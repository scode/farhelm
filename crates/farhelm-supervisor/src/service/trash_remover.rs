//! One live trash-removal run, independent of directory and browse admission.
//!
//! The registry is the recovery queue. This small in-memory queue contains only
//! this startup's attempts and newly accepted retirements; failed IDs are never
//! reinserted until another supervisor starts.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use tokio::sync::{Notify, oneshot, watch};
use tracing::warn;

use super::core::Supervisor;

/// Pending IDs belong only to this live run; active and ended describe the
/// worker, never the durable presence of a failed removal record.
#[derive(Default)]
struct Pending {
    ids: VecDeque<String>,
    started: Vec<oneshot::Sender<()>>,
    active: bool,
    ended: bool,
}

/// Queue changes and the active-to-idle transition share one short mutex. A
/// request arriving at that boundary either joins a live run or waits only for
/// the idle worker to start, never for an earlier folder's removal to finish.
#[derive(Default)]
pub(super) struct Remover {
    pending: Mutex<Pending>,
    wake: Notify,
    /// Hold or fail the actual removal entry point without replacing scheduling,
    /// retirement, ownership or the confined production traversal.
    #[cfg(test)]
    pub(super) before_remove: Mutex<Option<RemovalHook>>,
}

#[cfg(test)]
/// The real blocking entry point, after pickup has published a running attempt.
pub(super) type RemovalHook = Arc<dyn Fn(&str) -> anyhow::Result<()> + Send + Sync>;

impl Remover {
    /// Queue IDs inside the owned retirement closure. The optional receipt
    /// bridges the idle-to-running handoff before the reply becomes visible;
    /// otherwise a fast listing could say false before any removal began.
    pub(super) fn enqueue(
        &self,
        ids: Vec<String>,
    ) -> anyhow::Result<Option<oneshot::Receiver<()>>> {
        if ids.is_empty() {
            return Ok(None);
        }
        let mut pending = self.pending.lock().expect("trash queue poisoned");
        anyhow::ensure!(
            !pending.ended,
            "background trash remover ended; retired folders will resume at the next startup"
        );
        pending.ids.extend(ids);
        let receipt = if pending.active {
            None
        } else {
            let (sent, received) = oneshot::channel();
            pending.started.push(sent);
            Some(received)
        };
        self.wake.notify_one();
        Ok(receipt)
    }

    /// True for the worker's current sequential run, including its transition
    /// between folders; an idle or failed worker and retained failures are false.
    pub(super) fn removing(&self) -> bool {
        self.pending.lock().expect("trash queue poisoned").active
    }

    /// Claim work before acknowledging waiting replies. Finishing the run and
    /// observing the queue empty are atomic against a new enqueue.
    fn next(&self) -> Option<String> {
        let mut pending = self.pending.lock().expect("trash queue poisoned");
        let id = pending.ids.pop_front();
        pending.active = id.is_some();
        if id.is_some() {
            for started in pending.started.drain(..) {
                let _ = started.send(());
            }
        }
        id
    }
}

/// Report task death once and retain serve's stop authority. Cooperative stop
/// lets an already-started blocking traversal keep its state-directory claim.
pub(super) struct Handle {
    stop: watch::Sender<bool>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl Drop for Handle {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
    }
}

impl Handle {
    /// A failed worker is visible and never restarted into a retry loop.
    pub(super) async fn watch(&mut self) {
        let Some(task) = self.task.as_mut() else {
            std::future::pending::<()>().await;
            return;
        };
        let result = task.await;
        self.task = None;
        if let Err(error) = result {
            warn!(%error, "background trash remover failed; remaining folders wait for next startup");
        }
    }
}

/// Clear live state and release start receipts even if the async task unwinds.
struct Lifetime(Arc<Remover>);

impl Drop for Lifetime {
    fn drop(&mut self) {
        let mut pending = self.0.pending.lock().expect("trash queue poisoned");
        pending.active = false;
        pending.ended = true;
        pending.started.clear();
    }
}

/// Recover short journaled renames before admissions open, retaining ownership
/// even if serve's awaiter disappears. Dropping a blocking task's handle does
/// not stop its filesystem or database work; a successor must still be fenced.
pub(super) fn recover(
    sup: &Arc<Supervisor>,
) -> tokio::task::JoinHandle<anyhow::Result<Vec<String>>> {
    let owner = Arc::clone(sup);
    tokio::task::spawn_blocking(move || {
        let db = &owner.store.conn;
        let paths = crate::store::session_directory_paths(&db.lock())?;
        Ok(crate::checkout_trash::resume_retirements(db, &paths))
    })
}

/// Start only after startup's short rename recovery, before accepting requests.
/// An idle task holds a Weak supervisor; each actual traversal owns an Arc so
/// dropping a caller or serve cannot release its state-directory claim early.
pub(super) fn start(sup: &Arc<Supervisor>) -> Handle {
    let weak = Arc::downgrade(sup);
    let remover = sup.trash_remover.clone();
    let (stop, mut stopped) = watch::channel(false);
    let task = tokio::spawn(async move {
        let _lifetime = Lifetime(remover.clone());
        loop {
            tokio::select! {
                _ = stopped.changed() => return,
                _ = remover.wake.notified() => {}
            }
            while !*stopped.borrow() {
                let Some(id) = remover.next() else { break };
                let Some(sup) = weak.upgrade() else { return };
                let selected = id.clone();
                let result = tokio::task::spawn_blocking(move || {
                    #[cfg(test)]
                    {
                        let hook = sup
                            .trash_remover
                            .before_remove
                            .lock()
                            .expect("trash hook poisoned")
                            .clone();
                        if let Some(hook) = hook {
                            hook(&id)?;
                        }
                    }
                    crate::checkout_trash::remove_retired(&sup.store.conn, &id)
                })
                .await;
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        warn!(checkout_id = ?selected, error = %format!("{error:#}"), "background trash removal failed; record retained until next startup")
                    }
                    Err(error) => {
                        warn!(checkout_id = ?selected, %error, "background trash removal task failed; record retained until next startup")
                    }
                }
            }
        }
    });
    Handle {
        stop,
        task: Some(task),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::core::CreateInputs;
    use crate::service::core::tests::{StateDir, dummy_exe};
    use crate::working_copies::{self, PlannedWorkingCopy};
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    /// Real allocation and archival give the worker the same authority as a
    /// session Delete. Each ID owns a distinct observable marker and directory.
    fn archive(sup: &Supervisor, root: &Path, id: &str) -> PathBuf {
        let conn = sup.store.conn.lock();
        working_copies::record_planned(
            &conn,
            &PlannedWorkingCopy {
                id: id.into(),
                canonical_root: root.to_str().unwrap().into(),
                repo_owner: "fixture".into(),
                repo_name: "repo".into(),
                original_basename: id.into(),
                origin_session_id: id.into(),
                root_identity: None,
                preparation_snapshot: None,
            },
        )
        .unwrap();
        working_copies::allocate(&conn, id, None).unwrap();
        std::fs::write(root.join(id).join("marker"), id).unwrap();
        let working_copies::ArchiveOutcome::Archived { destination } =
            working_copies::archive_move(&conn, id).unwrap()
        else {
            panic!("fixture must archive")
        };
        working_copies::remove_member(&conn, id, id).unwrap();
        working_copies::retire(&conn, id).unwrap();
        root.join(working_copies::ARCHIVE_DIR_NAME)
            .join(destination)
    }

    /// Exercise real launch admission rather than its private predicate. A
    /// different preflight failure cannot stand in for the trash safety fence.
    async fn create_refused(
        sup: &Arc<Supervisor>,
        cwd: &str,
        checkout: Option<farhelm_proto::ResolvedGithubCheckout>,
    ) {
        let refusal = sup
            .create_session(
                CreateInputs {
                    prompt: None,
                    named_start: None,
                    cwd,
                    parent: None,
                    github_checkout: checkout,
                    launch: farhelm_proto::SessionLaunch::plain_command("agent"),
                    title: None,
                    cols: 80,
                    rows: 24,
                },
                None,
            )
            .await
            .unwrap_err();
        assert!(format!("{refusal:#}").contains("being removed from the trash"));
        assert!(sup.store.load_all().await.unwrap().is_empty());
    }

    /// Panic cleanup must release a blocking test gate too. A dropped receiver
    /// is an ordinary seam failure, never an unbounded test-runtime teardown.
    struct Release(Option<std::sync::mpsc::Sender<()>>);
    impl Drop for Release {
        fn drop(&mut self) {
            if let Some(sender) = self.0.take() {
                let _ = sender.send(());
            }
        }
    }

    /// Losing startup's awaiter cannot release the state-directory claim while
    /// its recovery still owes a rename. Holding the real database mutex pins
    /// that boundary without a scheduling delay or replacement recovery routine.
    #[farhelm_testtrace::test]
    fn startup_recovery_keeps_ownership_after_its_awaiter_disappears() {
        let state = StateDir::new();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let sup = runtime
            .block_on(Supervisor::new_with_exe(state.path(), dummy_exe()))
            .unwrap();
        let root = state.path().join("checkouts");
        std::fs::create_dir(&root).unwrap();
        let original = archive(&sup, &root, "recover");
        let destination = format!(".rm:{}", original.file_name().unwrap().to_str().unwrap());
        let db = sup.store.conn.clone();
        let row = working_copies::get_working_copy(&db.lock(), "recover")
            .unwrap()
            .unwrap();
        working_copies::repoint_retired(&db.lock(), &row, &destination).unwrap();
        let hidden = original.with_file_name(&destination);
        let ownership = Arc::downgrade(sup.ownership.as_ref().unwrap());
        let held = db.lock();
        {
            let _entered = runtime.enter();
            drop(recover(&sup));
        }
        drop(sup);
        assert!(
            original.exists(),
            "premise: recovery cannot pass the database lock"
        );
        assert!(
            !hidden.exists(),
            "premise: the journaled rename is still owed"
        );
        assert!(
            ownership.upgrade().is_some(),
            "recovery still owns the state directory"
        );
        drop(held);
        // Runtime drop joins real blocking work. Only after the rename finishes
        // may this in-process owner release its claim to a successor.
        drop(runtime);
        assert!(!original.exists());
        assert_eq!(std::fs::read(hidden.join("marker")).unwrap(), b"recover");
        assert!(ownership.upgrade().is_none());
        assert_eq!(
            working_copies::get_working_copy(&db.lock(), "recover")
                .unwrap()
                .unwrap()
                .archive_destination
                .as_deref(),
            Some(destination.as_str())
        );
    }

    /// A held remover must not hold either shared admission resource. A second
    /// retirement replies while the first marker remains, proving both prompt
    /// acceptance and sequential recursion rather than concurrent unlinks.
    #[farhelm_testtrace::test]
    async fn retirement_replies_while_removal_leaves_admission_and_browse_workers_free() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .unwrap();
        let ancestor = state.path().join("ancestor");
        let root = ancestor.join("checkouts");
        std::fs::create_dir_all(&root).unwrap();
        let first = archive(&sup, &root, "first");
        let second = archive(&sup, &root, "second");
        assert_eq!(std::fs::read(first.join("marker")).unwrap(), b"first");
        assert_eq!(std::fs::read(second.join("marker")).unwrap(), b"second");
        let (entered, ready) = oneshot::channel();
        let (last_entered, last_ready) = oneshot::channel();
        let entered = Mutex::new(Some(entered));
        let last_entered = Mutex::new(Some(last_entered));
        let (release, released) = std::sync::mpsc::channel();
        let release = Release(Some(release));
        let released = Mutex::new(released);
        *sup.trash_remover.before_remove.lock().unwrap() = Some(Arc::new(move |id| {
            if id == "first" {
                let _ = entered.lock().unwrap().take().unwrap().send(());
                released
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(15))?;
            } else {
                let _ = last_entered.lock().unwrap().take().unwrap().send(());
            }
            Ok(())
        }));
        let mut worker = start(&sup);
        let browse = sup
            .directory_browse_workers
            .clone()
            .acquire_many_owned(2)
            .await
            .unwrap();
        let owner = sup.clone();
        let request =
            tokio::spawn(async move { owner.delete_checkout_trash(vec!["first".into()]).await });
        tokio::time::timeout(Duration::from_secs(10), ready)
            .await
            .unwrap()
            .unwrap();
        let reply = tokio::time::timeout(Duration::from_secs(5), request)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(reply.removed, ["first"]);
        assert!(sup.working_copy_operations.try_lock().is_ok());
        drop(browse);
        let listing = sup.list_checkout_trash(false).await.unwrap();
        assert!(listing.removing);
        assert_eq!(listing.checkouts.len(), 1);
        let reply = tokio::time::timeout(
            Duration::from_secs(5),
            sup.delete_checkout_trash(vec!["second".into()]),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(reply.removed, ["second"]);
        for (id, original) in [("first", &first), ("second", &second)] {
            let row = working_copies::get_working_copy(&sup.store.conn.lock(), id)
                .unwrap()
                .unwrap();
            assert!(!original.exists());
            assert_eq!(
                std::fs::read(
                    root.join(working_copies::ARCHIVE_DIR_NAME)
                        .join(row.archive_destination.unwrap())
                        .join("marker")
                )
                .unwrap(),
                id.as_bytes()
            );
        }
        // The second tree is queued behind the held first traversal. Its
        // continued existence must not admit a new borrower, including aliases
        // and a fresh checkout configured to allocate inside it.
        let queued = working_copies::get_working_copy(&sup.store.conn.lock(), "second")
            .unwrap()
            .unwrap();
        let queued = root
            .join(working_copies::ARCHIVE_DIR_NAME)
            .join(queued.archive_destination.unwrap());
        let alias = root.join("alias");
        std::os::unix::fs::symlink(&queued, &alias).unwrap();
        assert_eq!(alias.canonicalize().unwrap(), queued);
        for cwd in [&queued, &alias] {
            create_refused(&sup, cwd.to_str().unwrap(), None).await;
        }
        let canonical_root = queued.to_str().unwrap().to_string();
        let mut fresh = farhelm_proto::ResolvedGithubCheckout {
            client_identity: "trash-fence-fixture".into(),
            agent_wrapper_supplied: false,
            wrapper_approved: false,
            repo: farhelm_proto::GithubRepo {
                owner: "fixture".into(),
                name: "repo".into(),
            },
            root: canonical_root.clone(),
            post_clone: None,
            claude_trust: false,
            keep_spare: false,
            preview: farhelm_proto::CheckoutPreviewBinding {
                canonical_root,
                basename: "repo-1".into(),
                cwd: queued.join("repo-1").to_str().unwrap().into(),
                config_revision: 1,
            },
        };
        create_refused(&sup, "", Some(fresh.clone())).await;
        assert!(!queued.join("repo-1").exists());
        // A final root symlink would be refused by ownership checks. Moving
        // its ancestor instead keeps that same root fully authorized while
        // changing every canonical candidate's prefix.
        let moved = state.path().join("moved-ancestor");
        std::fs::rename(&ancestor, &moved).unwrap();
        std::os::unix::fs::symlink(&moved, &ancestor).unwrap();
        let canonical = queued.canonicalize().unwrap();
        assert_ne!(canonical, queued);
        let row = working_copies::get_working_copy(&sup.store.conn.lock(), "second")
            .unwrap()
            .unwrap();
        working_copies::verified_root_dir(&row).unwrap();
        assert_eq!(std::fs::read(canonical.join("marker")).unwrap(), b"second");
        for cwd in [&queued, &canonical, &alias] {
            create_refused(&sup, cwd.to_str().unwrap(), None).await;
        }
        fresh.root = canonical.to_str().unwrap().into();
        fresh.preview.canonical_root = fresh.root.clone();
        fresh.preview.cwd = canonical.join("repo-1").to_str().unwrap().into();
        create_refused(&sup, "", Some(fresh)).await;
        assert!(!canonical.join("repo-1").exists());
        drop(release);
        tokio::time::timeout(Duration::from_secs(10), last_ready)
            .await
            .unwrap()
            .unwrap();
        worker.stop.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(10), worker.watch())
            .await
            .unwrap();
        assert!(!sup.list_checkout_trash(false).await.unwrap().removing);
        for id in ["first", "second"] {
            assert!(
                working_copies::get_working_copy(&sup.store.conn.lock(), id)
                    .unwrap()
                    .is_none()
            );
        }
    }

    /// One failed removal must neither retry on the next wake nor poison the
    /// next folder. Its warning and retained record survive an otherwise idle run.
    #[farhelm_testtrace::test]
    async fn failed_removal_is_attempted_once_and_the_next_folder_still_finishes() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .unwrap();
        let root = state.path().join("checkouts");
        std::fs::create_dir(&root).unwrap();
        archive(&sup, &root, "failed");
        archive(&sup, &root, "next");
        let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = attempts.clone();
        let (entered, ready) = oneshot::channel();
        let entered = Mutex::new(Some(entered));
        let (release, released) = std::sync::mpsc::channel();
        let release = Release(Some(release));
        let released = Mutex::new(released);
        *sup.trash_remover.before_remove.lock().unwrap() = Some(Arc::new(move |id| {
            if id == "failed" {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                return Err(std::io::Error::from_raw_os_error(libc::EACCES).into());
            }
            let _ = entered.lock().unwrap().take().unwrap().send(());
            released
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(15))?;
            Ok(())
        }));
        let mut worker = start(&sup);
        sup.delete_checkout_trash(vec!["failed".into(), "next".into()])
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(10), ready)
            .await
            .unwrap()
            .unwrap();
        let duplicate = sup
            .delete_checkout_trash(vec!["failed".into()])
            .await
            .unwrap();
        assert!(duplicate.removed.is_empty());
        assert_eq!(duplicate.issues.len(), 1);
        worker.stop.send(true).unwrap();
        drop(release);
        tokio::time::timeout(Duration::from_secs(10), worker.watch())
            .await
            .unwrap();
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);
        let listing = sup.list_checkout_trash(false).await.unwrap();
        assert!(!listing.removing && listing.checkouts.is_empty());
        let failed = working_copies::get_working_copy(&sup.store.conn.lock(), "failed")
            .unwrap()
            .unwrap();
        assert_eq!(
            std::fs::read(
                root.join(working_copies::ARCHIVE_DIR_NAME)
                    .join(failed.archive_destination.unwrap())
                    .join("marker")
            )
            .unwrap(),
            b"failed"
        );
        assert!(
            working_copies::get_working_copy(&sup.store.conn.lock(), "next")
                .unwrap()
                .is_none()
        );
        assert_eq!(
            farhelm_testtrace::current_capture()
                .unwrap()
                .matching("background trash removal failed; record retained until next startup")
                .unwrap()
                .len(),
            1
        );
    }
}
