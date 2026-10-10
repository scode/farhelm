//! Prompt report pickup, with the periodic drain retained as the safety net.
//!
//! The backend only marks work owed. One independent task runs the existing
//! whole-directory drain, waiting for its serialization lock rather than losing
//! a wake to a drain already in flight. Notify's stored permit keeps the trailing
//! edge of a burst; the pause bounds retry-generated filesystem events too.

use super::core::Supervisor;
use crate::hook_report::REPORTS_DIR;
use notify::{EventKind, RecursiveMode, Watcher};
use std::os::unix::fs::DirBuilderExt as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::{Notify, oneshot};
use tracing::warn;

/// Retried reports are linked back into place, producing another event. This
/// floor prevents an unavailable store or tmux from creating a tight retry loop.
const DRAIN_GAP: Duration = Duration::from_millis(100);

/// Owns the cooperative stop for the report task and its backend.
///
/// The ticker handle holds this beside its periodic task. Dropping it wakes an
/// idle task immediately; an admission already underway completes its durable
/// write and in-memory mirror before the backend is dropped. notify requests
/// native shutdown on drop; its inotify thread releases resources asynchronously.
pub(super) struct ReportWatchHandle {
    _stop: oneshot::Sender<()>,
    task: Option<tokio::task::JoinHandle<()>>,
    /// Tests can impose an event while the real drain is held at its snapshot.
    #[cfg(test)]
    pending: Arc<Notify>,
}

impl ReportWatchHandle {
    /// Surface an unexpected task panic once without stopping periodic pickup.
    /// Backend errors already logged their fallback before ending normally.
    pub(super) async fn watch(&mut self) {
        let Some(task) = self.task.as_mut() else {
            std::future::pending::<()>().await;
            return;
        };
        let result = task.await;
        self.task = None;
        if let Err(error) = result {
            warn!(%error, "conversation report watch task failed; periodic pickup remains active");
        }
    }
}

#[cfg(test)]
impl ReportWatchHandle {
    /// Join the cooperative stop so shutdown assertions cannot mistake a panic
    /// or a pending admission for Rust-task completion. Native backend teardown
    /// can finish after this join; Linux resource tests observe that separately.
    pub(super) async fn shutdown(mut self) {
        drop(self._stop);
        if let Some(task) = self.task.take() {
            task.await.expect("report watch task must not panic");
        }
    }
}

/// Attach the platform's recommended recursive watcher before serving reports.
///
/// Linux uses inotify and macOS uses notify's default FSEvents backend. Any setup
/// or runtime backend failure disables this acceleration permanently; the timer
/// continues running the same drain. A post-attachment wake covers files written
/// between startup reconciliation and attaching the watch.
pub(super) fn start_report_watch(sup: &Arc<Supervisor>) -> ReportWatchHandle {
    let (stop_tx, mut stop_rx) = oneshot::channel();
    let pending = Arc::new(Notify::new());
    let failed = Arc::new(AtomicBool::new(false));
    #[cfg(target_os = "linux")]
    let topology_changed = Arc::new(AtomicBool::new(false));
    let root = sup.state_dir.join(REPORTS_DIR);
    let setup = (|| -> anyhow::Result<notify::RecommendedWatcher> {
        if let Some(fault) = sup.seams.faults.report_watch_creation() {
            fault()?;
        }
        // Both the hook and supervisor use 0700. DirBuilder's recursive mode
        // accepts an existing directory, including the hook winning this race.
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&root)?;
        let wake = Arc::clone(&pending);
        let backend_failed = Arc::clone(&failed);
        let suppress_events = sup.seams.faults.suppress_report_watch_events().cloned();
        #[cfg(target_os = "linux")]
        let topology = Arc::clone(&topology_changed);
        #[cfg(target_os = "linux")]
        let event_root = root.clone();
        let mut watcher = notify::recommended_watcher(
            move |event: notify::Result<notify::Event>| {
                match event {
                    Err(error) => {
                        if !backend_failed.swap(true, Ordering::AcqRel) {
                            warn!(%error, "conversation report watch failed; periodic pickup remains active");
                        }
                        wake.notify_one();
                    }
                    // Reads generate access events on some backends. Waking on
                    // those would let our own directory scan perpetually re-fire.
                    Ok(_)
                        if suppress_events
                            .as_ref()
                            .is_some_and(|flag| flag.load(Ordering::Acquire)) => {}
                    Ok(event) if !matches!(event.kind, EventKind::Access(_)) => {
                        #[cfg(target_os = "linux")]
                        if event.need_rescan()
                            || (matches!(
                                event.kind,
                                EventKind::Create(_)
                                    | EventKind::Modify(notify::event::ModifyKind::Name(_))
                            ) && event
                                .paths
                                .iter()
                                .any(|path| path.parent() == Some(event_root.as_path())))
                        {
                            topology.store(true, Ordering::Release);
                        }
                        wake.notify_one();
                    }
                    Ok(_) => {}
                }
            },
        )?;
        watcher.watch(&root, RecursiveMode::Recursive)?;
        Ok(watcher)
    })();
    let watcher = match setup {
        Ok(watcher) => watcher,
        Err(error) => {
            if !failed.swap(true, Ordering::AcqRel) {
                warn!(%error, "could not start conversation report watch; periodic pickup remains active");
            }
            return ReportWatchHandle {
                _stop: stop_tx,
                task: None,
                #[cfg(test)]
                pending,
            };
        }
    };
    let weak = Arc::downgrade(sup);
    let mut lifetime = sup.report_watch_lifetime.subscribe();
    pending.notify_one();
    #[cfg(test)]
    let test_pending = Arc::clone(&pending);
    let task = tokio::spawn(async move {
        // Owning the backend here requests native shutdown on every exit,
        // including a closed lifetime channel with no filesystem traffic.
        // notify's inotify thread finishes asynchronously after backend drop.
        #[cfg(target_os = "linux")]
        let mut watcher = watcher;
        #[cfg(not(target_os = "linux"))]
        let _watcher = watcher;
        #[cfg(target_os = "linux")]
        let mut vanished_retries = 0_u32;
        loop {
            tokio::select! {
                biased;
                _ = &mut stop_rx => break,
                _ = lifetime.changed() => break,
                _ = pending.notified() => {}
            }
            if failed.load(Ordering::Acquire) {
                break;
            }
            #[cfg(target_os = "linux")]
            if topology_changed.swap(false, Ordering::AcqRel) {
                // notify's inotify backend emits a new directory event BEFORE
                // subscribing to its children. Acknowledged recursive watch
                // registration closes that gap before this drain lists files:
                // earlier publications are on disk, later ones are subscribed.
                // FSEvents watches trees directly and needs no re-registration.
                let registration_root = root.clone();
                let registered = tokio::task::spawn_blocking(move || {
                    let result = watcher.watch(&registration_root, RecursiveMode::Recursive);
                    (watcher, result)
                })
                .await;
                match registered {
                    Ok((returned, Ok(()))) => {
                        watcher = returned;
                        vanished_retries = 0;
                    }
                    // A folder removed mid-walk (an orphan cleanup, a Delete,
                    // a hook re-creating a deleted session's folder) fails
                    // the walk without saying anything about the watch. The
                    // walk stopped partway, so later folders may be missing
                    // their subscription: register again after this drain
                    // rather than giving up fast pickup for the supervisor's
                    // lifetime. The cap stops a root that is itself gone from
                    // retrying forever.
                    Ok((returned, Err(error)))
                        if vanished_during_walk(&error)
                            && vanished_retries < MAX_VANISHED_RETRIES =>
                    {
                        watcher = returned;
                        vanished_retries += 1;
                        topology_changed.store(true, Ordering::Release);
                        pending.notify_one();
                    }
                    Ok((_, Err(error))) => {
                        if !failed.swap(true, Ordering::AcqRel) {
                            warn!(%error, "conversation report watch registration failed; periodic pickup remains active");
                        }
                        break;
                    }
                    Err(error) => {
                        if !failed.swap(true, Ordering::AcqRel) {
                            warn!(%error, "conversation report watch registration task failed; periodic pickup remains active");
                        }
                        break;
                    }
                }
            }
            // Registration waits on the native backend. A stop or failure
            // during that wait must not start a new admission afterward;
            // only a drain already underway may finish cooperatively.
            if !matches!(stop_rx.try_recv(), Err(oneshot::error::TryRecvError::Empty))
                || lifetime.has_changed().is_err()
                || failed.load(Ordering::Acquire)
            {
                break;
            }
            let Some(sup) = weak.upgrade() else { break };
            let entries = sup
                .sessions
                .lock()
                .await
                .values()
                .cloned()
                .collect::<Vec<_>>();
            sup.apply_report_files(&entries, true).await;
            drop(sup);
            // Stop remains responsive during the recovery gap. Do not consume
            // pending here: events during admission or the pause owe a pass.
            tokio::select! {
                biased;
                _ = &mut stop_rx => break,
                _ = lifetime.changed() => break,
                _ = tokio::time::sleep(DRAIN_GAP) => {}
            }
        }
    });
    ReportWatchHandle {
        _stop: stop_tx,
        task: Some(task),
        #[cfg(test)]
        pending: test_pending,
    }
}

/// How many re-registrations in a row may fail on a vanished folder before
/// the watch gives up. Each retry follows a drain and its pause, so this
/// bounds the retries at a few seconds of churn, not a tight loop.
#[cfg(target_os = "linux")]
const MAX_VANISHED_RETRIES: u32 = 20;

/// Whether a recursive registration failed only because a path it was
/// walking disappeared, which says nothing about whether watching works.
///
/// notify's inotify backend walks the whole tree on every registration and
/// fails the call on the first folder that is gone by the time it subscribes.
#[cfg(target_os = "linux")]
fn vanished_during_walk(error: &notify::Error) -> bool {
    match &error.kind {
        notify::ErrorKind::PathNotFound | notify::ErrorKind::WatchNotFound => true,
        notify::ErrorKind::Io(io) => io.kind() == std::io::ErrorKind::NotFound,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec: only a path that vanished mid-walk counts as a retryable
    /// registration failure; running out of watches or any other error
    /// still ends the watch.
    ///
    /// Why: a retry on a real limit would spin without ever working, and
    /// treating a deleted folder as fatal would give up fast pickup for the
    /// supervisor's whole life over routine folder cleanup.
    #[cfg(target_os = "linux")]
    #[test]
    fn only_a_vanished_path_makes_registration_retryable() {
        assert!(vanished_during_walk(&notify::Error::path_not_found()));
        assert!(vanished_during_walk(&notify::Error::io(
            std::io::Error::from(std::io::ErrorKind::NotFound)
        )));
        assert!(!vanished_during_walk(&notify::Error::new(
            notify::ErrorKind::MaxFilesWatch
        )));
        assert!(!vanished_during_walk(&notify::Error::io(
            std::io::Error::from(std::io::ErrorKind::PermissionDenied)
        )));
    }
    use crate::service::core::tests::{StateDir, dummy_exe};
    use crate::service::core::{FaultHooks, SupervisorSeams, SupervisorTimeouts};
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::Path;

    /// No timer or server is started here: only the watch may drain these
    /// orphan directories. StateDir owns any private tmux cleanup on failure.
    async fn supervisor(state: &StateDir, faults: FaultHooks) -> Arc<Supervisor> {
        Supervisor::new_with_seams(
            state.path(),
            dummy_exe(),
            SupervisorTimeouts::default(),
            SupervisorSeams {
                faults,
                ticker_interval: Duration::from_secs(600),
                ..Default::default()
            },
        )
        .await
        .expect("supervisor")
    }

    /// Observe orphan cleanup without driving any reconciliation ourselves.
    /// The bounded poll reports the exact still-present directory on timeout.
    async fn removed(path: &Path) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while path.exists() {
            assert!(
                tokio::time::Instant::now() < deadline,
                "watch did not drain {}",
                path.display()
            );
            // sleep-ok: polling the watch task's externally observable orphan cleanup.
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// A directory created by the first hook must be observed recursively.
    /// No ticker runs, so removing its orphan report proves actual backend pickup.
    #[farhelm_testtrace::test]
    async fn first_new_directory_is_drained_and_root_is_private() {
        let state = StateDir::new();
        let scanned = Arc::new(Notify::new());
        let sup = supervisor(
            &state,
            FaultHooks {
                report_drain_listed: Some(Arc::new({
                    let scanned = Arc::clone(&scanned);
                    move || {
                        scanned.notify_one();
                        Box::pin(async {})
                    }
                })),
                ..Default::default()
            },
        )
        .await;
        let root = state.path().join(REPORTS_DIR);
        assert!(!root.exists(), "fixture starts without hook reports");
        let watch = start_report_watch(&sup);
        assert!(watch.task.is_some(), "real backend must start");
        assert_eq!(root.metadata().unwrap().permissions().mode() & 0o777, 0o700);
        tokio::time::timeout(Duration::from_secs(5), scanned.notified())
            .await
            .unwrap();
        let dir = root.join("orphan-first");
        std::fs::create_dir(&dir).unwrap();
        removed(&dir).await;
        watch.shutdown().await;
    }

    /// A drain cannot see a directory created after its snapshot. An imposed
    /// event while that drain holds the lock must retain a second pass, even
    /// if the backend coalesces the directory and slot writes into one wake.
    #[farhelm_testtrace::test]
    async fn event_during_a_drain_keeps_another_pass_owed() {
        let state = StateDir::new();
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let first = Arc::new(AtomicBool::new(true));
        let sup = supervisor(
            &state,
            FaultHooks {
                suppress_report_watch_events: Some(Arc::new(AtomicBool::new(true))),
                report_drain_listed: Some(Arc::new({
                    let entered = Arc::clone(&entered);
                    let release = Arc::clone(&release);
                    move || {
                        let entered = Arc::clone(&entered);
                        let release = Arc::clone(&release);
                        let pause = first.swap(false, Ordering::SeqCst);
                        Box::pin(async move {
                            if pause {
                                entered.notify_one();
                                release.notified().await;
                            }
                        })
                    }
                })),
                ..Default::default()
            },
        )
        .await;
        let watch = start_report_watch(&sup);
        tokio::time::timeout(Duration::from_secs(5), entered.notified())
            .await
            .unwrap();
        let dir = state.path().join(REPORTS_DIR).join("orphan-later");
        assert!(
            !dir.exists(),
            "the held drain's snapshot excludes this directory"
        );
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("latest.json"), b"{}").unwrap();
        watch.pending.notify_one();
        release.notify_one();
        removed(&dir).await;
        watch.shutdown().await;
    }

    /// Setup failure must leave actual scheduled pickup working. The seam
    /// consumes no shared kernel resources and changes no environment; no
    /// explicit reconciliation or second watcher can explain the cleanup.
    #[farhelm_testtrace::test]
    async fn creation_failure_leaves_periodic_drain_usable() {
        let state = StateDir::new();
        let failed = Arc::new(AtomicBool::new(false));
        let sup = Supervisor::new_with_seams(
            state.path(),
            dummy_exe(),
            SupervisorTimeouts::default(),
            SupervisorSeams {
                faults: FaultHooks {
                    report_watch_creation: Some(Arc::new({
                        let failed = Arc::clone(&failed);
                        move || {
                            failed.store(true, Ordering::Release);
                            anyhow::bail!("injected watch failure")
                        }
                    })),
                    ..Default::default()
                },
                ticker_interval: Duration::from_millis(100),
                ..Default::default()
            },
        )
        .await
        .expect("supervisor");
        let dir = state.path().join(REPORTS_DIR).join("orphan-fallback");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("latest.json"), b"{}").unwrap();
        assert!(
            dir.exists(),
            "report is present before any periodic work starts"
        );
        let ticker = super::super::ticker::start_ticker(&sup);
        assert!(
            failed.load(Ordering::Acquire),
            "watch setup hit the failure seam"
        );
        removed(&dir).await;
        ticker.shutdown().await;
    }

    /// Snapshot this isolated nextest process's native watch resources, so the
    /// shutdown proof measures only the backend created by its own fixture.
    /// Linux truncates the notify thread's name to the kernel's comm limit.
    #[cfg(target_os = "linux")]
    fn native_resources() -> (
        std::collections::BTreeSet<String>,
        std::collections::BTreeSet<String>,
    ) {
        let threads = std::fs::read_dir("/proc/self/task")
            .unwrap()
            .filter_map(|entry| {
                let entry = entry.unwrap();
                let name = std::fs::read_to_string(entry.path().join("comm")).ok()?;
                name.starts_with("notify-rs inoti")
                    .then(|| entry.file_name().to_string_lossy().into_owned())
            })
            .collect();
        let descriptors = std::fs::read_dir("/proc/self/fd")
            .unwrap()
            .filter_map(|entry| {
                let entry = entry.unwrap();
                let target = std::fs::read_link(entry.path()).ok()?;
                (target == Path::new("anon_inode:inotify"))
                    .then(|| entry.file_name().to_string_lossy().into_owned())
            })
            .collect();
        (threads, descriptors)
    }

    /// Backend drop sends native shutdown but does not join notify's thread.
    /// Observe eventual disappearance of this fixture's TID and inotify FD,
    /// rather than equating a Rust task join with native resource release.
    #[cfg(target_os = "linux")]
    async fn native_resources_released(
        owned: &(
            std::collections::BTreeSet<String>,
            std::collections::BTreeSet<String>,
        ),
    ) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let current = native_resources();
            if current.0.is_disjoint(&owned.0) && current.1.is_disjoint(&owned.1) {
                return;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "native watch resources remain: owned={owned:?}, current={current:?}"
            );
            // sleep-ok: bounded observation of asynchronous notify backend teardown.
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// An idle backend must end when the supervisor disappears, without a
    /// filesystem event or a periodic weak-reference poll to wake it.
    #[farhelm_testtrace::test]
    async fn dropping_supervisor_releases_idle_watch() {
        let state = StateDir::new();
        let sup = supervisor(&state, Default::default()).await;
        #[cfg(target_os = "linux")]
        let baseline = native_resources();
        let mut watch = start_report_watch(&sup);
        assert!(watch.task.is_some());
        #[cfg(target_os = "linux")]
        let owned = {
            let current = native_resources();
            let owned: (
                std::collections::BTreeSet<String>,
                std::collections::BTreeSet<String>,
            ) = (
                current.0.difference(&baseline.0).cloned().collect(),
                current.1.difference(&baseline.1).cloned().collect(),
            );
            assert_eq!(owned.0.len(), 1, "one fixture-owned native thread");
            assert_eq!(owned.1.len(), 1, "one fixture-owned inotify descriptor");
            owned
        };
        drop(sup);
        tokio::time::timeout(Duration::from_secs(5), watch.task.as_mut().unwrap())
            .await
            .unwrap()
            .unwrap();
        watch.task = None;
        watch.shutdown().await;
        #[cfg(target_os = "linux")]
        native_resources_released(&owned).await;
    }

    /// The hook may win creation of the private root. Attaching the watch to
    /// that existing directory must preserve it and still drain later writes.
    #[farhelm_testtrace::test]
    async fn existing_hook_root_is_accepted() {
        let state = StateDir::new();
        let scanned = Arc::new(Notify::new());
        let sup = supervisor(
            &state,
            FaultHooks {
                report_drain_listed: Some(Arc::new({
                    let scanned = Arc::clone(&scanned);
                    move || {
                        scanned.notify_one();
                        Box::pin(async {})
                    }
                })),
                ..Default::default()
            },
        )
        .await;
        let root = state.path().join(REPORTS_DIR);
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        let watch = start_report_watch(&sup);
        assert!(watch.task.is_some());
        tokio::time::timeout(Duration::from_secs(5), scanned.notified())
            .await
            .unwrap();
        let dir = root.join("orphan-existing");
        std::fs::create_dir(&dir).unwrap();
        removed(&dir).await;
        watch.shutdown().await;
    }
}
