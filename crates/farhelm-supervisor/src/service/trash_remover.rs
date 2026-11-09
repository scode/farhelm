//! One live trash-removal run, independent of directory and browse admission.
//!
//! The registry is the recovery queue. This small in-memory queue contains only
//! this startup's attempts and newly accepted retirements. Failures stay in
//! memory until an explicit Retry or the next supervisor start tries again.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use farhelm_proto::{CheckoutTrashListing, CheckoutTrashStuck};
use tokio::sync::{Notify, oneshot, watch};
use tracing::warn;

use super::core::Supervisor;

/// Queued/active IDs and failure observations belong only to this live run.
/// The revision separates failed attempts and detects completion during a
/// listing; none of this state adds deletion authority to the registry.
#[derive(Default)]
struct Pending {
    ids: VecDeque<String>,
    inflight: HashSet<String>,
    failures: HashMap<String, Failure>,
    revision: u64,
    retiring: bool,
    started: Vec<oneshot::Sender<()>>,
    active: bool,
    ended: bool,
    ended_at: i64,
}

/// A process-local identity separates retries of distinct failed attempts,
/// even when both failures happened during the same UTC second.
struct Failure {
    revision: u64,
    report: CheckoutTrashStuck,
}

/// What a request could have meant to retry before it waited for admission.
/// A later failure must never turn an old duplicate into another attempt.
pub(super) enum Observed {
    Inflight,
    Failed(u64),
    Unseen,
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

/// A borrowed publication marker, never a filesystem lock or admission claim.
/// Admission serializes its owners, so one bool covers the journal-to-enqueue
/// gap. Dropping it before admission advances the snapshot even when work exits
/// without enqueueing; no queue mutex stays held through that work's I/O.
pub(super) struct Retirement<'a>(&'a Remover);

impl Drop for Retirement<'_> {
    fn drop(&mut self) {
        let mut pending = self.0.pending.lock().expect("trash queue poisoned");
        pending.retiring = false;
        pending.revision += 1;
    }
}

impl Remover {
    /// Mark owned retirement only after it acquires directory admission. Keep
    /// the returned guard through enqueue and drop it before releasing admission;
    /// starting it before admission would invalidate the single-owner bool.
    pub(super) fn retiring(&self) -> Retirement<'_> {
        self.pending.lock().expect("trash queue poisoned").retiring = true;
        Retirement(self)
    }

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
        let mut changed = false;
        for id in ids {
            if pending.inflight.insert(id.clone()) {
                changed = true;
                pending.failures.remove(&id);
                pending.ids.push_back(id);
            }
        }
        if changed {
            pending.revision += 1;
        }
        if pending.ids.is_empty() {
            return Ok(None);
        }
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

    /// True for queued work and the worker's sequential run, including its
    /// transition between folders. Retained failures and an ended worker are
    /// false, so polling starts before pickup and stops after task death.
    pub(super) fn removing(&self) -> bool {
        let pending = self.pending.lock().expect("trash queue poisoned");
        !pending.ended && (pending.active || !pending.ids.is_empty())
    }

    /// Freeze retry authority before directory admission can delay a request.
    pub(super) fn observe(&self, ids: &[String]) -> HashMap<String, Observed> {
        let pending = self.pending.lock().expect("trash queue poisoned");
        ids.iter()
            .map(|id| {
                let observed = if pending.inflight.contains(id) {
                    Observed::Inflight
                } else if let Some(failure) = pending.failures.get(id) {
                    Observed::Failed(failure.revision)
                } else {
                    Observed::Unseen
                };
                (id.clone(), observed)
            })
            .collect()
    }

    /// Admission serializes request checks and enqueue. Coalescing uses the
    /// original observation as well as current state, so a newly failed Retry
    /// cannot authorize another request that observed the previous failure.
    pub(super) fn select(
        &self,
        ids: &[String],
        observed: HashMap<String, Observed>,
    ) -> (Vec<String>, Vec<String>) {
        let pending = self.pending.lock().expect("trash queue poisoned");
        let mut eligible = Vec::new();
        let mut coalesced = Vec::new();
        let mut seen = HashSet::new();
        for id in ids.iter().filter(|id| seen.insert(id.as_str())) {
            let observed = observed.get(id).expect("every requested ID was observed");
            let same_failure = match observed {
                Observed::Inflight => false,
                Observed::Failed(revision) => pending
                    .failures
                    .get(id)
                    .is_some_and(|failure| failure.revision == *revision),
                Observed::Unseen => !pending.failures.contains_key(id),
            };
            if same_failure && !pending.inflight.contains(id) {
                eligible.push(id.clone());
            } else {
                coalesced.push(id.clone());
            }
        }
        (eligible, coalesced)
    }

    /// Distinguish Retry refusals from ordinary first-time Delete diagnostics.
    pub(super) fn failed_ids(&self) -> HashSet<String> {
        self.pending
            .lock()
            .expect("trash queue poisoned")
            .failures
            .keys()
            .cloned()
            .collect()
    }

    /// A read-start revision tells a racing completion apart from a retained
    /// record that this run never queued (for example after a registry error).
    pub(super) fn revision(&self) -> u64 {
        self.pending.lock().expect("trash queue poisoned").revision
    }

    /// Merge worker state with a filesystem snapshot without losing polling's
    /// completion edge. A captured journal with no current state might have
    /// completed during the read; keep it running if state changed or an owned
    /// retirement still owes its enqueue. Only a stable untracked row or task
    /// death is unavailable until an explicit Retry. The retirement marker
    /// alone does not turn an empty listing into removal activity.
    pub(super) fn annotate(&self, listing: &mut CheckoutTrashListing, revision: u64) {
        let mut pending = self.pending.lock().expect("trash queue poisoned");
        let changed = pending.revision != revision;
        listing.removing = !pending.ended && (pending.active || !pending.ids.is_empty());
        for entry in &mut listing.checkouts {
            if pending.inflight.contains(&entry.id) && !pending.ended {
                entry.removing = true;
                entry.stuck = None;
            } else if let Some(failure) = pending.failures.get(&entry.id) {
                entry.removing = false;
                entry.stuck = Some(failure.report.clone());
            } else if entry.removing && pending.ended {
                entry.removing = false;
                entry.stuck = Some(CheckoutTrashStuck {
                    reason:
                        "background trash remover ended; try again after the supervisor restarts"
                            .into(),
                    at: pending.ended_at,
                });
            } else if entry.removing && !changed && !pending.retiring {
                // No attempt raced this read, so another poll cannot reveal
                // its outcome. Keep the record visible and explicitly retryable.
                record_failure(
                    &mut pending,
                    &entry.id,
                    "background removal is unavailable; retry to try freeing this folder again"
                        .into(),
                );
                entry.removing = false;
                entry.stuck = Some(pending.failures[&entry.id].report.clone());
            }
            listing.removing |= entry.removing;
        }
    }

    /// Record failures before releasing an attempt's in-flight identity.
    /// Listing and a new Retry therefore observe one consistent outcome.
    pub(super) fn failed(&self, id: &str, error: &anyhow::Error) {
        let reason = if error.chain().any(|cause| {
            cause
                .downcast_ref::<std::io::Error>()
                .is_some_and(|error| error.kind() == std::io::ErrorKind::PermissionDenied)
                || cause
                    .downcast_ref::<rustix::io::Errno>()
                    .is_some_and(|error| matches!(error.raw_os_error(), libc::EACCES | libc::EPERM))
        }) {
            "permission denied while removing files; check ownership and directory permissions"
                .to_string()
        } else {
            format!("{error:#}").chars().take(512).collect()
        };
        let mut pending = self.pending.lock().expect("trash queue poisoned");
        pending.inflight.remove(id);
        record_failure(&mut pending, id, reason);
    }

    /// Successful removal or explicit forgetting clears this run's failure
    /// state after its caller has already dropped the durable record.
    pub(super) fn succeeded(&self, id: &str) {
        let mut pending = self.pending.lock().expect("trash queue poisoned");
        pending.inflight.remove(id);
        pending.failures.remove(id);
        pending.revision += 1;
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

/// Failure timestamps use the same UTC-second convention as archive times.
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|time| time.as_secs() as i64)
        .unwrap_or(0)
}

/// Keep failure identity and content changes atomic under the queue mutex.
fn record_failure(pending: &mut Pending, id: &str, reason: String) {
    pending.revision += 1;
    pending.failures.insert(
        id.into(),
        Failure {
            revision: pending.revision,
            report: CheckoutTrashStuck { reason, at: now() },
        },
    );
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
        pending.ended_at = now();
        let unfinished = pending.inflight.drain().collect::<Vec<_>>();
        pending.ids.clear();
        for id in unfinished {
            record_failure(
                &mut pending,
                &id,
                "background trash remover ended; try again after the supervisor restarts".into(),
            );
        }
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
        Ok(crate::checkout_trash::resume_retirements(
            db,
            &paths,
            &mut |id, error| {
                owner.trash_remover.failed(id, error);
            },
        ))
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
                    Ok(Ok(())) => remover.succeeded(&selected),
                    Ok(Err(error)) => {
                        remover.failed(&selected, &error);
                        warn!(checkout_id = ?selected, error = %format!("{error:#}"), "background trash removal failed; record retained until next startup")
                    }
                    Err(error) => {
                        let error = anyhow::Error::from(error);
                        remover.failed(&selected, &error);
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
    use crate::service::core::tests::{StateDir, dummy_exe};
    use crate::service::core::{CreateInputs, StateDirOwnership};
    use crate::working_copies::{self, PlannedWorkingCopy};
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    /// The queue's settled state is the completion oracle, not hook entry or a
    /// guessed filesystem delay. Yielding keeps both runtime and worker free.
    async fn wait_for_idle(remover: &Remover) {
        tokio::time::timeout(Duration::from_secs(10), async {
            while remover.removing() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("trash worker did not settle");
    }

    /// Observe the owned closure's marker while the database mutex holds its
    /// first read. This is an explicit unsettled boundary, not worker readiness.
    async fn wait_for_retirement(remover: &Remover, expected: bool, marker: &Path) {
        tokio::time::timeout(Duration::from_secs(10), async {
            while remover.pending.lock().unwrap().retiring != expected {
                tokio::task::yield_now().await;
            }
        }).await.unwrap_or_else(|_| {
            panic!("retirement marker did not become {expected}; fixture marker exists={}, current retirement={}",
                marker.exists(), remover.pending.lock().unwrap().retiring);
        });
    }

    /// Restore the owned directory even after assertion failure. A held handle
    /// follows the fixture across retirement's rename without guessing its path.
    struct ReadonlyDirectory(std::fs::File, std::fs::Permissions);
    impl ReadonlyDirectory {
        /// Keep directory reads possible so the fixture distinguishes known
        /// size from failed removal; only writes needed for unlink are denied.
        fn new(path: &Path) -> Self {
            use std::os::unix::fs::PermissionsExt;
            let directory = std::fs::File::open(path).unwrap();
            let original = directory.metadata().unwrap().permissions();
            directory
                .set_permissions(std::fs::Permissions::from_mode(0o500))
                .unwrap();
            Self(directory, original)
        }
    }
    impl Drop for ReadonlyDirectory {
        fn drop(&mut self) {
            self.0.set_permissions(self.1.clone()).unwrap();
        }
    }

    /// A queued startup set must start polling before the worker exists. The
    /// same snapshot must also keep polling if completion raced a browse read,
    /// while a dead worker must settle as unavailable instead of spinning UI.
    #[farhelm_testtrace::test]
    fn queued_snapshot_and_ended_worker_have_truthful_listing_states() {
        let remover = Arc::new(Remover::default());
        remover.enqueue(vec!["id".into(), "id".into()]).unwrap();
        assert!(remover.removing());
        assert_eq!(remover.pending.lock().unwrap().ids.len(), 1);
        let mut captured: CheckoutTrashListing = serde_json::from_value(serde_json::json!({
            "checkouts":[{"id":"id", "name":"repo", "repository":"fixture/repo",
                "path":"/checkouts/.farhelm-trash/.rm:repo", "archived_at":null,
                "bytes":null, "removing":true}], "issues":[], "reports_retired":true
        }))
        .unwrap();
        let revision = remover.revision();
        assert_eq!(remover.next().as_deref(), Some("id"));
        remover.succeeded("id");
        assert!(remover.next().is_none());
        assert!(!remover.removing());
        remover.annotate(&mut captured, revision);
        assert!(
            captured.removing && captured.checkouts[0].removing,
            "a completed row captured by an earlier read needs one final poll"
        );
        drop(Lifetime(remover.clone()));
        remover.annotate(&mut captured, revision);
        assert!(!captured.removing && !captured.checkouts[0].removing);
        assert!(
            captured.checkouts[0]
                .stuck
                .as_ref()
                .unwrap()
                .reason
                .contains("ended")
        );
        assert!(remover.enqueue(vec!["id".into()]).is_err());
    }

    /// A readable journal with no queued attempt must not keep clients polling
    /// forever after an earlier whole-registry recovery read failed.
    #[farhelm_testtrace::test]
    fn an_untracked_retirement_reports_unavailable_without_polling() {
        let remover = Arc::new(Remover::default());
        let mut listing: CheckoutTrashListing = serde_json::from_value(serde_json::json!({
            "checkouts":[{"id":"id", "name":"repo", "repository":"fixture/repo",
                "path":"/checkouts/.farhelm-trash/.rm:repo", "archived_at":null,
                "bytes":null, "removing":true}], "issues":[], "reports_retired":true
        }))
        .unwrap();
        remover.annotate(&mut listing, remover.revision());
        assert!(!listing.removing && !listing.checkouts[0].removing);
        assert!(
            listing.checkouts[0]
                .stuck
                .as_ref()
                .unwrap()
                .reason
                .contains("unavailable")
        );
        assert_eq!(
            remover
                .select(&["id".into()], remover.observe(&["id".into()]))
                .0,
            ["id"]
        );
        remover.enqueue(vec!["id".into()]).unwrap();
        drop(Lifetime(remover.clone()));
        remover.annotate(&mut listing, remover.revision());
        assert!(!listing.removing);
        assert!(
            listing.checkouts[0]
                .stuck
                .as_ref()
                .unwrap()
                .reason
                .contains("ended")
        );
    }

    /// A journal seen before enqueue has not failed. The marker and its drop
    /// revision preserve the final poll, but a later stable unqueued read must
    /// settle. Unwinding must clear the marker without poisoning the mutex.
    #[farhelm_testtrace::test]
    fn retirement_handoff_and_drop_preserve_provisional_snapshots() {
        let remover = Remover::default();
        let mut captured: CheckoutTrashListing = serde_json::from_value(serde_json::json!({
            "checkouts":[{"id":"id", "name":"repo", "repository":"fixture/repo",
                "path":"/checkouts/.farhelm-trash/.rm:repo", "archived_at":null,
                "bytes":null, "removing":true}], "issues":[], "reports_retired":true
        }))
        .unwrap();
        let retirement = remover.retiring();
        let revision = remover.revision();
        let mut empty = CheckoutTrashListing::default();
        remover.annotate(&mut empty, revision);
        assert!(
            !empty.removing,
            "a marker without a captured row is not queued work"
        );
        remover.annotate(&mut captured, revision);
        assert!(captured.removing && captured.checkouts[0].removing);
        assert!(captured.checkouts[0].stuck.is_none());
        assert!(
            remover.failed_ids().is_empty(),
            "the handoff is not a failure"
        );
        drop(retirement);
        remover.annotate(&mut captured, revision);
        assert!(
            captured.removing && captured.checkouts[0].stuck.is_none(),
            "a read spanning drop needs its final poll even without enqueue"
        );
        remover.annotate(&mut captured, remover.revision());
        assert!(
            !captured.removing && captured.checkouts[0].stuck.is_some(),
            "a stable truly-unqueued journal must stop polling"
        );
        let revision = remover.revision();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _retirement = remover.retiring();
            panic!("fixture: owned retirement unwinds before enqueue");
        }));
        assert!(result.is_err());
        assert!(!remover.pending.lock().unwrap().retiring);
        assert!(remover.revision() > revision);
    }

    /// Cancelling the async request cannot drop the owned retirement's marker
    /// or admission. Once its held database read can finish, that same closure
    /// must enqueue its accepted ID and clear the marker before releasing
    /// admission, even though nobody remains to await its pickup receipt.
    #[farhelm_testtrace::test]
    #[allow(
        clippy::await_holding_lock,
        reason = "the held database read deliberately gates owned retirement across request cancellation"
    )]
    async fn cancelled_retirement_keeps_its_marker_until_owned_enqueue_finishes() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .unwrap();
        let root = state.path().join("checkouts");
        std::fs::create_dir(&root).unwrap();
        let original = archive(&sup, &root, "handoff");
        let batch = crate::checkout_trash::delete_timed(
            &sup.store.conn,
            &["handoff".into()],
            &[],
            crate::checkout_trash::DeleteTiming::default(),
        )
        .unwrap();
        assert_eq!(batch.queued, ["handoff"]);
        let revision = sup.trash_remover.revision();
        let mut captured = crate::checkout_trash::list(&sup.store.conn, false).unwrap();
        let marker = Path::new(&captured.checkouts[0].path).join("marker");
        assert_eq!(std::fs::read(&marker).unwrap(), b"handoff");
        assert!(!original.exists());
        let db = sup.store.conn.clone();
        let held = db.lock();
        let owner = sup.clone();
        let request =
            tokio::spawn(async move { owner.delete_checkout_trash(vec!["handoff".into()]).await });
        wait_for_retirement(&sup.trash_remover, true, &marker).await;
        sup.trash_remover.annotate(&mut captured, revision);
        assert!(captured.removing && captured.checkouts[0].stuck.is_none());
        assert!(sup.trash_remover.failed_ids().is_empty());
        request.abort();
        assert!(request.await.unwrap_err().is_cancelled());
        assert!(
            sup.trash_remover.pending.lock().unwrap().retiring,
            "the owned closure is still held at its first database read"
        );
        assert!(sup.working_copy_operations.try_lock().is_err());
        drop(held);
        wait_for_retirement(&sup.trash_remover, false, &marker).await;
        assert!(
            sup.trash_remover.removing(),
            "work reached the queue before marker drop"
        );
        // Waiting for admission's actual release also proves the closure has
        // passed enqueue and its marker's ordered drop, rather than inferring
        // completion from the vanished async awaiter.
        drop(
            tokio::time::timeout(Duration::from_secs(10), sup.working_copy_operations.lock())
                .await
                .unwrap(),
        );
        assert!(!sup.trash_remover.pending.lock().unwrap().retiring);
        assert_eq!(sup.trash_remover.pending.lock().unwrap().ids.len(), 1);
        let mut worker = start(&sup);
        wait_for_idle(&sup.trash_remover).await;
        assert!(!marker.exists());
        assert!(
            sup.list_checkout_trash(false)
                .await
                .unwrap()
                .checkouts
                .is_empty()
        );
        worker.stop.send(true).unwrap();
        worker.watch().await;
    }

    /// A failed Retry cannot lend its successor to another request that saw
    /// the same old failure, even when that request waited for admission.
    #[farhelm_testtrace::test]
    fn retry_authority_is_bound_to_the_observed_failure() {
        let remover = Remover::default();
        let ids = vec!["id".into()];
        let before_initial = remover.observe(&ids);
        remover.enqueue(ids.clone()).unwrap();
        let while_queued = remover.observe(&ids);
        assert_eq!(remover.next(), Some("id".into()));
        remover.failed("id", &anyhow::anyhow!("first failure"));
        assert!(remover.select(&ids, before_initial).0.is_empty());
        assert!(remover.select(&ids, while_queued).0.is_empty());
        let first_retry = remover.observe(&ids);
        let delayed_duplicate = remover.observe(&ids);
        assert_eq!(remover.select(&ids, first_retry).0, ids);
        remover.enqueue(ids.clone()).unwrap();
        let while_running = remover.observe(&ids);
        assert_eq!(remover.next(), Some("id".into()));
        remover.failed("id", &anyhow::anyhow!("second failure"));
        assert!(remover.select(&ids, delayed_duplicate).0.is_empty());
        assert!(remover.select(&ids, while_running).0.is_empty());
        assert_eq!(remover.select(&ids, remover.observe(&ids)).0, ids);
    }

    /// Two requests that saw the same failure remain one Retry even if the
    /// first one fails again before the second resumes from directory admission.
    #[farhelm_testtrace::test]
    async fn delayed_duplicate_retry_does_not_queue_a_later_failed_attempt() {
        use std::future::Future;
        use std::task::Poll;

        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .unwrap();
        let root = state.path().join("checkouts");
        std::fs::create_dir(&root).unwrap();
        archive(&sup, &root, "duplicate");
        let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = attempts.clone();
        *sup.trash_remover.before_remove.lock().unwrap() = Some(Arc::new(move |_| {
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Err(std::io::Error::from_raw_os_error(libc::EACCES).into())
        }));
        let mut worker = start(&sup);
        sup.delete_checkout_trash(vec!["duplicate".into()])
            .await
            .unwrap();
        wait_for_idle(&sup.trash_remover).await;
        let admission = sup.working_copy_operations.lock().await;
        let mut first = Box::pin(sup.delete_checkout_trash(vec!["duplicate".into()]));
        let mut second = Box::pin(sup.delete_checkout_trash(vec!["duplicate".into()]));
        // Poll each request to the held admission boundary, proving both
        // already observed the same failure without timing their scheduling.
        for request in [&mut first, &mut second] {
            std::future::poll_fn(|cx| {
                assert!(
                    request.as_mut().poll(cx).is_pending(),
                    "admission is still held"
                );
                Poll::Ready(())
            })
            .await;
        }
        drop(admission);
        assert_eq!(first.await.unwrap().removed, ["duplicate"]);
        wait_for_idle(&sup.trash_remover).await;
        assert_eq!(
            attempts.load(std::sync::atomic::Ordering::SeqCst),
            2,
            "the initial attempt and first explicit Retry failed"
        );
        let reply = second.await.unwrap();
        assert_eq!(reply.removed, ["duplicate"]);
        assert!(reply.issues.is_empty());
        wait_for_idle(&sup.trash_remover).await;
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 2);
        worker.stop.send(true).unwrap();
        worker.watch().await;
    }

    /// Mode bits must cause a real filesystem refusal. Restoring only the
    /// fixture permissions and explicitly retrying must then free the files.
    #[farhelm_testtrace::test]
    async fn permission_failure_is_listed_and_explicit_retry_frees_the_folder() {
        if unsafe { libc::geteuid() } == 0 {
            eprintln!(
                "SKIPPED: root bypasses directory permissions; no real removal-refusal evidence"
            );
            return;
        }
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .unwrap();
        let root = state.path().join("checkouts");
        std::fs::create_dir(&root).unwrap();
        let original = archive(&sup, &root, "permission");
        let locked = original.join("locked");
        std::fs::create_dir(&locked).unwrap();
        std::fs::write(locked.join("kept"), b"retained work").unwrap();
        let permissions = ReadonlyDirectory::new(&locked);
        assert_eq!(
            std::fs::remove_file(locked.join("kept"))
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::PermissionDenied,
            "premise: the owned directory actually refuses unlink"
        );
        let before = now();
        let mut worker = start(&sup);
        assert_eq!(
            sup.delete_checkout_trash(vec!["permission".into()])
                .await
                .unwrap()
                .removed,
            ["permission"]
        );
        wait_for_idle(&sup.trash_remover).await;
        let listing = sup.list_checkout_trash(true).await.unwrap();
        assert!(listing.reports_retired && !listing.removing);
        assert!(listing.issues.is_empty());
        let entry = &listing.checkouts[0];
        assert_eq!(entry.id, "permission");
        assert!(!entry.name.starts_with(".rm:"));
        assert!(entry.path.contains("/.rm:"));
        assert!(entry.archived_at.is_some() && entry.bytes.is_some());
        let stuck = entry.stuck.as_ref().unwrap();
        assert!(stuck.reason.contains("permission denied"));
        assert!(stuck.at >= before && stuck.at <= now());
        assert!(!entry.removing);
        assert_eq!(
            std::fs::read(Path::new(&entry.path).join("locked/kept")).unwrap(),
            b"retained work"
        );

        // Moving the recorded root away makes it inaccessible without changing
        // fixture permissions or adopting a replacement. Known metadata survives.
        let unavailable = state.path().join("unavailable");
        std::fs::rename(&root, &unavailable).unwrap();
        let missing = sup.list_checkout_trash(true).await.unwrap();
        assert_eq!(missing.checkouts.len(), 1);
        assert_eq!(missing.checkouts[0].name, entry.name);
        assert_eq!(missing.checkouts[0].archived_at, entry.archived_at);
        assert_eq!(missing.checkouts[0].repository, entry.repository);
        assert_eq!(missing.checkouts[0].stuck, entry.stuck);
        assert!(missing.checkouts[0].bytes.is_none() && missing.issues.is_empty());
        std::fs::rename(&unavailable, &root).unwrap();
        drop(permissions);
        let retry = sup
            .delete_checkout_trash(vec!["permission".into()])
            .await
            .unwrap();
        assert_eq!(retry.removed, ["permission"]);
        assert!(retry.issues.is_empty());
        wait_for_idle(&sup.trash_remover).await;
        let listing = sup.list_checkout_trash(true).await.unwrap();
        assert!(listing.reports_retired && listing.checkouts.is_empty());
        assert!(!Path::new(&entry.path).exists());
        worker.stop.send(true).unwrap();
        worker.watch().await;
    }

    /// A failed startup rename keeps accepted intent durable. After restoring
    /// the owned obstruction, a new supervisor with no failure memory must
    /// resume that same journal and finish freeing its folder.
    #[farhelm_testtrace::test]
    async fn startup_rename_failure_keeps_the_journal_for_the_next_start() {
        if unsafe { libc::geteuid() } == 0 {
            eprintln!(
                "SKIPPED: root bypasses directory permissions; no startup rename-refusal evidence"
            );
            return;
        }
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .unwrap();
        let root = state.path().join("checkouts");
        std::fs::create_dir(&root).unwrap();
        let original = archive(&sup, &root, "startup");
        let row = working_copies::get_working_copy(&sup.store.conn.lock(), "startup")
            .unwrap()
            .unwrap();
        let destination = format!(".rm:{}", original.file_name().unwrap().to_str().unwrap());
        working_copies::repoint_retired(&sup.store.conn.lock(), &row, &destination).unwrap();
        let permissions = ReadonlyDirectory::new(original.parent().unwrap());
        assert_eq!(
            std::fs::rename(&original, original.with_file_name(&destination))
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::PermissionDenied,
            "premise: the pending rename really is refused without moving"
        );
        let before = now();
        assert!(recover(&sup).await.unwrap().unwrap().is_empty());
        assert!(original.exists());
        let row = working_copies::get_working_copy(&sup.store.conn.lock(), "startup")
            .unwrap()
            .unwrap();
        assert!(
            crate::checkout_trash::is_removing(&row),
            "a definite rename refusal must not lose previously accepted cleanup"
        );
        assert_eq!(
            row.archive_destination.as_deref(),
            Some(destination.as_str())
        );
        let listing = sup.list_checkout_trash(false).await.unwrap();
        assert_eq!(listing.checkouts.len(), 1);
        let entry = &listing.checkouts[0];
        assert!(
            entry
                .stuck
                .as_ref()
                .unwrap()
                .reason
                .contains("permission denied")
        );
        assert!(entry.stuck.as_ref().unwrap().at >= before);
        assert!(!entry.removing && !listing.removing);
        let unavailable = state.path().join("unavailable");
        std::fs::rename(&root, &unavailable).unwrap();
        let missing = sup.list_checkout_trash(true).await.unwrap();
        assert_eq!(missing.checkouts[0].id, "startup");
        assert_eq!(missing.checkouts[0].stuck, entry.stuck);
        assert!(missing.checkouts[0].bytes.is_none() && missing.issues.is_empty());
        std::fs::rename(&unavailable, &root).unwrap();
        drop(permissions);
        drop(sup);
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .unwrap();
        assert!(
            sup.trash_remover.failed_ids().is_empty(),
            "a new supervisor did not inherit an in-memory failure"
        );
        let resumed = recover(&sup).await.unwrap().unwrap();
        assert_eq!(resumed, ["startup"]);
        assert!(!original.exists());
        assert_eq!(
            std::fs::read(original.with_file_name(&destination).join("marker")).unwrap(),
            b"startup"
        );
        sup.trash_remover.enqueue(resumed).unwrap();
        assert!(
            sup.trash_remover.removing(),
            "startup queued its accepted removal before pickup"
        );
        let mut worker = start(&sup);
        wait_for_idle(&sup.trash_remover).await;
        assert!(
            sup.list_checkout_trash(false)
                .await
                .unwrap()
                .checkouts
                .is_empty()
        );
        assert!(!original.with_file_name(&destination).exists());
        worker.stop.send(true).unwrap();
        worker.watch().await;
    }

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

    /// Observe detached recovery through its claim, since its awaiter is gone.
    /// The runtime must stay alive while waiting: shutdown can discard blocking
    /// work that has not started, so dropping it is not a completion barrier.
    async fn wait_for_recovery_claim_release(
        ownership: &std::sync::Weak<StateDirOwnership>,
        original: &Path,
        hidden: &Path,
    ) {
        tokio::time::timeout(Duration::from_secs(10), async {
            while ownership.upgrade().is_some() {
                // sleep-ok: bounded polling of detached recovery's own ownership release.
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "startup recovery still owns its claim: original exists={}, hidden exists={}",
                original.exists(),
                hidden.exists()
            )
        });
    }

    /// Losing startup's awaiter cannot release the state-directory claim while
    /// recovery still owes a rename. The real database mutex holds that unsettled
    /// boundary; after release, the detached recovery must finish the rename and
    /// give its claim back without anyone awaiting its task handle.
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
        runtime.block_on(wait_for_recovery_claim_release(
            &ownership, &original, &hidden,
        ));
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
        assert_eq!(listing.checkouts.len(), 2);
        assert!(
            listing
                .checkouts
                .iter()
                .find(|entry| entry.id == "first")
                .unwrap()
                .removing
        );
        let duplicate = sup
            .delete_checkout_trash(vec!["first".into()])
            .await
            .unwrap();
        assert_eq!(duplicate.removed, ["first"]);
        assert!(duplicate.issues.is_empty());
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
            .delete_checkout_trash(vec!["next".into()])
            .await
            .unwrap();
        assert_eq!(duplicate.removed, ["next"]);
        assert!(duplicate.issues.is_empty());
        worker.stop.send(true).unwrap();
        drop(release);
        tokio::time::timeout(Duration::from_secs(10), worker.watch())
            .await
            .unwrap();
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);
        let listing = sup.list_checkout_trash(false).await.unwrap();
        assert!(!listing.removing);
        assert_eq!(listing.checkouts.len(), 1);
        assert_eq!(listing.checkouts[0].id, "failed");
        assert!(
            listing.checkouts[0]
                .stuck
                .as_ref()
                .unwrap()
                .reason
                .contains("permission denied")
        );
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
