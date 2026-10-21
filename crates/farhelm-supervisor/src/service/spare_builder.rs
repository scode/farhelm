//! One owed spare attempt per repository, one running host build, no retry queue.
//!
//! Fresh accepted origins register their frozen policy. Replays and Restart do
//! not. The ticker wakes pending clone gates; this worker alone mutates records
//! after startup. Registration shares only the final token-check/rename gate,
//! never a clone wait, cache flock or durability barrier.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Context as _;
use tokio::sync::{Notify, watch};
use tracing::warn;

use super::core::Supervisor;
use crate::bounded_command::{GroupChild, OutputCaps};
use crate::launch::PreparationState;

/// Only original fresh-create provenance can authorize background work. Root,
/// shell and switch are frozen inputs, not current helm configuration.
#[derive(Clone)]
pub(super) struct Trigger {
    pub(super) repo: farhelm_proto::GithubRepo,
    pub(super) root: PathBuf,
    pub(super) shell: String,
    pub(super) enabled: bool,
    pub(super) session: String,
    pub(super) generation: i64,
    pub(super) working_copy: String,
}

/// One coalesced action. Pending registrations replace the gate; once claimed,
/// same-policy creates owe no trailing build. Off/root changes invalidate either.
#[derive(Clone)]
pub(super) struct Desired {
    trigger: Trigger,
    token: uuid::Uuid,
    running: bool,
    /// Captured only after the registry's original root identity verifies.
    /// This is a transient worker projection, not another persisted identity.
    root_identity: Option<crate::working_copies::DirectoryEvidence>,
}

/// The mutex holds only registration and the final no-replace rename. Disk
/// preparation and record fsync run outside it on the single worker.
#[derive(Default)]
pub(super) struct Builder {
    desired: Mutex<HashMap<String, Desired>>,
    pub(super) wake: Notify,
}

impl Builder {
    /// Read-only test observation of the actual coalesced policy, rather than
    /// a second scheduler model. Production callers only register and wake.
    #[cfg(test)]
    pub(super) fn policy(
        &self,
        repo: &farhelm_proto::GithubRepo,
    ) -> Option<(bool, PathBuf, String)> {
        self.desired.lock().unwrap().get(&key(repo)).map(|desired| {
            (
                desired.trigger.enabled,
                desired.trigger.root.clone(),
                desired.trigger.session.clone(),
            )
        })
    }
    /// Registration order is this process's policy order. There is no persisted
    /// high-water mark and no startup rebuild of old completed creates.
    pub(super) fn register(&self, trigger: Trigger) {
        let key = key(&trigger.repo);
        let mut desired = self.desired.lock().expect("spare policy mutex poisoned");
        if !desired.get(&key).is_some_and(|old| {
            old.running
                && old.trigger.root == trigger.root
                && old.trigger.enabled == trigger.enabled
        }) {
            // Pending creates still coalesce into one entry, but the newest
            // physical create supplies its gate. An obsolete corrupt/refused
            // origin must not poison a later valid create of the same repo.
            desired.insert(
                key,
                Desired {
                    trigger,
                    token: uuid::Uuid::new_v4(),
                    running: false,
                    root_identity: None,
                },
            );
        }
        drop(desired);
        self.wake.notify_one();
    }

    /// A missing entry is not publication authority. Completed/failed attempts
    /// are removed rather than becoming automatic retry obligations.
    fn current(&self, desired: &Desired) -> bool {
        self.desired
            .lock()
            .expect("spare policy mutex poisoned")
            .get(&key(&desired.trigger.repo))
            .is_some_and(|current| current.token == desired.token)
    }

    /// Claim only the exact gate observed before asynchronous provenance reads.
    /// A new pending registration can replace it while those reads are in flight.
    fn claim(&self, desired: &Desired) -> bool {
        let mut entries = self.desired.lock().expect("spare policy mutex poisoned");
        let Some(current) = entries.get_mut(&key(&desired.trigger.repo)) else {
            return false;
        };
        if current.token != desired.token || current.running {
            return false;
        }
        current.running = true;
        true
    }

    /// Old completion cannot consume a newer policy or replenishment token.
    fn consume(&self, desired: &Desired) {
        let mut entries = self.desired.lock().expect("spare policy mutex poisoned");
        if entries
            .get(&key(&desired.trigger.repo))
            .is_some_and(|entry| entry.token == desired.token)
        {
            entries.remove(&key(&desired.trigger.repo));
        }
    }

    /// Publication retires its token before fsync/cleanup. A later same-policy
    /// create can therefore owe the replacement without old cleanup deleting the
    /// ready clone it is about to take. Off/root changes still discard it. Absence
    /// here preserves a captured successful publication; before publication,
    /// absence is never authority to rename a building into the ready slot.
    fn keep_ready(&self, desired: &Desired, published: bool) -> bool {
        self.desired
            .lock()
            .expect("spare policy mutex poisoned")
            .get(&key(&desired.trigger.repo))
            .map_or(published, |latest| {
                latest.trigger.enabled && latest.trigger.root == desired.trigger.root
            })
    }
}

/// Repository identity is already normalized by protocol admission.
fn key(repo: &farhelm_proto::GithubRepo) -> String {
    format!("{}/{}", repo.owner, repo.name)
}

/// Serve owns the stop sender. The task observes stop while waiting/running and
/// reaps its direct child; its Arc keeps state-directory ownership through cleanup.
pub(super) struct Handle {
    stop: watch::Sender<bool>,
    task: Option<tokio::task::JoinHandle<()>>,
    builder: Arc<Builder>,
}

impl Drop for Handle {
    fn drop(&mut self) {
        // A record fsync can still be in flight. Invalidate its publication
        // authority immediately, before cooperative child/disk cleanup finishes.
        self.builder
            .desired
            .lock()
            .expect("spare policy mutex poisoned")
            .clear();
        let _ = self.stop.send(true);
    }
}

impl Handle {
    /// A dead worker is reported once, not silently replaced with an automatic
    /// retry loop. No other background worker can publish concurrently.
    pub(super) async fn watch(&mut self) {
        let Some(task) = self.task.as_mut() else {
            std::future::pending::<()>().await;
            return;
        };
        let result = task.await;
        self.task = None;
        if let Err(error) = result {
            warn!(%error, "spare builder worker failed; new spares unavailable");
        }
    }
}

/// Start after startup recovery, never in a losing supervisor constructor.
pub(super) fn start(sup: &Arc<Supervisor>, farhelm_exe: PathBuf) -> Handle {
    let weak = Arc::downgrade(sup);
    let builder = sup.spare_builder.clone();
    let owner = builder.clone();
    let (stop, mut stopped) = watch::channel(false);
    let task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = stopped.changed() => break,
                _ = builder.wake.notified() => {}
            }
            while !*stopped.borrow() {
                let Some(sup) = weak.upgrade() else { return };
                let Some(desired) = next(&sup).await else {
                    break;
                };
                if !builder.claim(&desired) {
                    continue;
                }
                if let Err(error) = attempt(&sup, &farhelm_exe, &desired, &mut stopped).await {
                    warn!(repository = %key(&desired.trigger.repo), error = %format!("{error:#}"),
                        "background spare failed; ordinary visible clone remains available");
                }
                builder.consume(&desired);
            }
        }
    });
    Handle {
        stop,
        task: Some(task),
        builder: owner,
    }
}

/// Positive clone completion comes from existing durable preparation evidence,
/// fenced through the accepted origin and generation. Failed clone is eligible
/// once too: the visible command has returned and released its cache guard.
pub(super) async fn next(sup: &Supervisor) -> Option<Desired> {
    let candidates: Vec<_> = sup
        .spare_builder
        .desired
        .lock()
        .expect("spare policy mutex poisoned")
        .values()
        .filter(|desired| !desired.running)
        .cloned()
        .collect();
    for mut desired in candidates {
        if !desired.trigger.enabled {
            return Some(desired);
        }
        let trigger = &desired.trigger;
        let origin = match sup
            .store
            .preparation_origin(&trigger.session, trigger.generation)
            .await
        {
            Ok(Some(origin)) if origin.id == trigger.working_copy => origin,
            Ok(None) => {
                sup.spare_builder.consume(&desired);
                continue;
            }
            Ok(Some(_)) => {
                sup.spare_builder.consume(&desired);
                continue;
            }
            Err(_) => continue,
        };
        let state = crate::launch::read_preparation_state(
            &crate::launch::preparation_state_path(&sup.state_dir, &origin.id),
            &origin.id,
        );
        let state = match state {
            Ok(Some(record)) => record.state,
            // Unknown evidence refuses this attempt, not the next fresh create.
            // It is never an excuse to retry a failed clone automatically.
            Ok(None) | Err(_) => {
                sup.spare_builder.consume(&desired);
                continue;
            }
        };
        if clone_finished(&state) {
            let root = crate::working_copies::verified_root_dir(&origin)
                .and_then(|root| Ok(crate::working_copies::DirectoryEvidence::capture(&root)?));
            if let Ok(identity) = root {
                desired.root_identity = Some(identity);
                return Some(desired);
            }
            sup.spare_builder.consume(&desired);
        } else if matches!(state, PreparationState::Failed { .. }) {
            sup.spare_builder.consume(&desired);
        }
    }
    None
}

/// State vocabulary is deliberately closed: validation/identity failures did
/// not run clone, and an unreadable/missing state never means completion.
fn clone_finished(state: &PreparationState) -> bool {
    matches!(
        state,
        PreparationState::HookStarted | PreparationState::Ready
    ) || matches!(state, PreparationState::Failed { stage, .. }
            if stage == "clone" || stage == "post-clone" || stage == "ready")
}

/// Run the only filesystem writer off-runtime and retain supervisor ownership
/// until it returns, including a canceled requester or slow filesystem call.
async fn disk<T: Send + 'static>(
    sup: &Supervisor,
    work: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
) -> anyhow::Result<T> {
    let ownership = sup.ownership.clone();
    tokio::task::spawn_blocking(move || {
        let _ownership = ownership;
        work()
    })
    .await
    .context("spare filesystem worker panicked")?
}

/// Cancellation is a policy event, not a failed build to retry. Record writes
/// happen outside the gate; the final rename below shares registration's lock.
async fn attempt(
    sup: &Supervisor,
    farhelm_exe: &std::path::Path,
    desired: &Desired,
    stopped: &mut watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(600);
    let cache = crate::repo_cache::path(&sup.state_dir, &desired.trigger.repo);
    let cache_to_open = cache.clone();
    let lock = disk(sup, move || {
        Ok(crate::repo_cache::open_lock(&cache_to_open)?)
    })
    .await?;
    let Some(lock) = wait_cache(lock, &sup.spare_builder, desired, stopped, deadline).await? else {
        return Ok(());
    };
    let cache_for_prepare = cache.clone();
    let trigger = desired.trigger.clone();
    let root_identity = desired.root_identity;
    let prepared = disk(sup, move || {
        let _lock = lock;
        crate::spare_checkouts::discard_except(
            &cache_for_prepare,
            &trigger.repo,
            trigger.enabled.then_some(trigger.root.as_path()),
        )?;
        if !trigger.enabled {
            return Ok(None);
        }
        crate::spare_checkouts::prepare(
            &cache_for_prepare,
            &trigger.repo,
            &trigger.root,
            root_identity.context("accepted checkout root identity unavailable")?,
        )
    })
    .await?;
    let Some(prepared) = prepared else {
        return Ok(());
    };
    let result = async {
        if *stopped.borrow() || !sup.spare_builder.current(desired) {
            return Ok(false);
        }
        let directory = sup.state_dir.join("spare-builds");
        tokio::fs::create_dir_all(&directory).await?;
        let path = directory.join(format!("{}.json", uuid::Uuid::new_v4()));
        let remaining = deadline
            .checked_duration_since(tokio::time::Instant::now())
            .context("spare build deadline exhausted before login")?;
        let spec = crate::spare_build::BuildSpec {
            cache: cache.clone(),
            repo: desired.trigger.repo.clone(),
            target: prepared.target.clone(),
            shell: desired.trigger.shell.clone(),
            farhelm_exe: farhelm_exe.to_owned(),
            farhelm_bin_dir: sup.session_bin_dir().to_owned(),
            deadline_millis: u64::try_from(remaining.as_millis())
                .unwrap_or(600_000)
                .clamp(1, 600_000),
        };
        crate::files::write_private_file(&path, &serde_json::to_vec(&spec)?).await?;
        let run = run_guard(sup, farhelm_exe, &path, desired, stopped, deadline).await;
        if !run? {
            return Ok(false);
        }
        let builder = sup.spare_builder.clone();
        let token = desired.token;
        let repository = key(&desired.trigger.repo);
        let cache = cache.clone();
        disk(sup, move || {
            let published = publish_prepared(
                &builder,
                &repository,
                token,
                &prepared,
                &cache,
                &crate::files::RealFs,
            )?;
            if published {
                prepared.sync_published()?;
            }
            Ok(published)
        })
        .await
    }
    .await;
    // A failed/canceled building is cleanup, never another queued attempt. Keep
    // a successfully published ready slot only when current on/root policy agrees.
    let trigger = desired.trigger.clone();
    let keep_ready = !*stopped.borrow()
        && sup
            .spare_builder
            .keep_ready(desired, result.as_ref().is_ok_and(|published| *published));
    let cleanup = disk(sup, move || {
        discard_if_cache_free(
            &cache,
            &trigger.repo,
            keep_ready.then_some(trigger.root.as_path()),
        )
    })
    .await;
    if let Err(error) = &cleanup {
        warn!(error = %format!("{error:#}"), "spare cleanup retained ownership evidence for startup");
    }
    result.map(|_| ()).and(cleanup)
}

/// Cleanup is one nonblocking attempt under the same cache inode as Git. A busy
/// visible clone retains every recorded identity and generated directory for a
/// later create/startup; it cannot strand shutdown behind a credential prompt.
fn discard_if_cache_free(
    cache: &std::path::Path,
    repo: &farhelm_proto::GithubRepo,
    root: Option<&std::path::Path>,
) -> anyhow::Result<()> {
    let lock = crate::repo_cache::open_lock(cache)?;
    anyhow::ensure!(
        lock.try_acquire()?,
        "cache is busy; spare cleanup retained for startup"
    );
    crate::spare_checkouts::discard_except(cache, repo, root)
}

/// Wait only through nonblocking flock probes. A visible clone may own this
/// inode while waiting for credentials, so cancellation/deadline must not leave
/// a detached spawn_blocking waiter retaining supervisor ownership indefinitely.
async fn wait_cache(
    lock: crate::launch::PreparationLock,
    builder: &Builder,
    desired: &Desired,
    stopped: &mut watch::Receiver<bool>,
    deadline: tokio::time::Instant,
) -> anyhow::Result<Option<crate::launch::PreparationLock>> {
    loop {
        if *stopped.borrow() || !builder.current(desired) {
            return Ok(None);
        }
        anyhow::ensure!(
            tokio::time::Instant::now() < deadline,
            "spare build deadline exhausted waiting for cache"
        );
        if lock.try_acquire()? {
            return Ok(Some(lock));
        }
        tokio::select! {
            _ = stopped.changed() => return Ok(None),
            _ = builder.wake.notified() => {},
            _ = tokio::time::sleep_until((tokio::time::Instant::now() + Duration::from_millis(100)).min(deadline)) => {},
        }
    }
}

/// Persist outside the short gate, then make the policy decision and move one
/// indivisible publication. The file seam observes the actual record write.
fn publish_prepared(
    builder: &Builder,
    repository: &str,
    token: uuid::Uuid,
    prepared: &crate::spare_checkouts::PreparedBuild,
    cache: &std::path::Path,
    seam: &dyn crate::files::FaultSeam,
) -> anyhow::Result<bool> {
    prepared.record_ready(cache, seam)?;
    let mut entries = builder.desired.lock().expect("spare policy mutex poisoned");
    if entries
        .get(repository)
        .is_some_and(|entry| entry.token == token)
    {
        prepared.publish()?;
        // The useful build is complete at rename, not after unrelated slow
        // durability/cleanup work. A create taking this slot must owe its next
        // replacement rather than coalescing into the already published build.
        entries.remove(repository);
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Keep one finish future alive across coalesced wakes: restarting finish would
/// discard pipe readers and violate GroupChild's once-only stream contract.
async fn run_guard(
    sup: &Supervisor,
    farhelm_exe: &std::path::Path,
    path: &std::path::Path,
    desired: &Desired,
    stopped: &mut watch::Receiver<bool>,
    deadline: tokio::time::Instant,
) -> anyhow::Result<bool> {
    if tokio::time::Instant::now() >= deadline {
        remove_artifacts(path).await;
        anyhow::bail!("spare build deadline exhausted before guard spawn");
    }
    let caps = OutputCaps {
        stdout: 1024 * 1024,
        stderr: Some(1024 * 1024),
    };
    let mut command =
        tokio::process::Command::from(crate::launch::background_guard_command(farhelm_exe));
    command.arg("internal").arg("build-spare-guard").arg(path);
    let mut child = match GroupChild::spawn_detached_tethered(&mut command, caps) {
        Ok(child) => child,
        Err(error) => {
            // Spawn failure has no live reader. Every later error must first
            // reap the child; neither path leaves ephemeral input until restart.
            remove_artifacts(path).await;
            return Err(error.into());
        }
    };
    let ended = {
        let finish = child.finish(deadline, caps);
        tokio::pin!(finish);
        loop {
            tokio::select! {
                result = &mut finish => break Some(result),
                _ = stopped.changed() => break None,
                _ = sup.spare_builder.wake.notified() => {
                    if !sup.spare_builder.current(desired) { break None; }
                }
            }
        }
    };
    if !child.is_reaped() {
        child.kill_and_reap().await?;
    }
    let result = match ended {
        None => Ok(false),
        Some(Err(error)) => Err(error.into()),
        Some(Ok(ended)) => {
            let success = crate::spare_build::completed(&ended, path);
            if !success {
                match &ended {
                    crate::bounded_command::Ended::Exited(output) => warn!(status = %output.status,
                stderr = %String::from_utf8_lossy(&output.stderr[..output.stderr.len().min(4096)]),
                "spare guard did not produce accepted clone completion"),
                    other => warn!(?other, "spare guard exceeded its execution bounds"),
                }
            }
            Ok(success)
        }
    };
    // This point positively follows direct-child reaping. A kill/reap failure
    // above retains all artifacts for startup rather than racing a live reader.
    remove_artifacts(path).await;
    result
}

/// Remove only this run's private names, after positive child reaping or failed
/// spawn. A kill/reap error must retain these while a reader might still exist.
async fn remove_artifacts(path: &std::path::Path) {
    for artifact in [
        path.to_owned(),
        crate::spare_build::result_path(path),
        crate::spare_build::receipt_path(path),
    ] {
        match tokio::fs::remove_file(artifact).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => warn!(%error, "spare private artifact retained after child retirement"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::{FaultSeam, RealFs};
    use crate::working_copies::DirectoryEvidence;
    use std::fs;
    use std::sync::mpsc;

    /// A real owned namespace/cache fixture for publication races. Its clone
    /// content is placed by hand because the contested boundary is record/rename.
    struct Fixture {
        _temporary: tempfile::TempDir,
        cache: PathBuf,
        trigger: Trigger,
        prepared: crate::spare_checkouts::PreparedBuild,
    }

    impl Fixture {
        /// Capture the actual root object and use the production producer path;
        /// no parallel record implementation can make the race pass accidentally.
        fn new() -> Self {
            let temporary = tempfile::tempdir().unwrap();
            let root = temporary.path().canonicalize().unwrap();
            let repo = farhelm_proto::parse_github_repo("example/repo").unwrap();
            let cache = crate::repo_cache::path(&root.join("state"), &repo);
            let identity = DirectoryEvidence::capture(&fs::File::open(&root).unwrap()).unwrap();
            let prepared = crate::spare_checkouts::prepare(&cache, &repo, &root, identity)
                .unwrap()
                .unwrap();
            fs::write(prepared.target.path().join("complete"), "clone content").unwrap();
            let trigger = Trigger {
                repo,
                root,
                shell: "/bin/sh".into(),
                enabled: true,
                session: uuid::Uuid::new_v4().to_string(),
                generation: 0,
                working_copy: uuid::Uuid::new_v4().to_string(),
            };
            Self {
                _temporary: temporary,
                cache,
                trigger,
                prepared,
            }
        }

        /// The ready slot is distinct from the private building being cloned.
        fn ready(&self) -> PathBuf {
            self.trigger
                .root
                .join(crate::working_copies::SPARE_DIR_NAME)
                .join(&self.trigger.repo.owner)
                .join(&self.trigger.repo.name)
        }
    }

    /// The file seam blocks the actual directory durability barrier, after the
    /// record's rename. Registration must remain independent of that filesystem.
    struct HeldSync {
        reached: mpsc::Sender<()>,
        release: Mutex<mpsc::Receiver<()>>,
    }
    impl FaultSeam for HeldSync {
        fn fsync_dir(&self, directory: &fs::File) -> std::io::Result<()> {
            self.reached.send(()).unwrap();
            self.release
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(10))
                .unwrap();
            directory.sync_all()
        }
    }

    /// A positively contested visible-clone lock cannot keep runtime admission
    /// alive after off/root invalidation or supervisor stop. Polling the actual
    /// future once establishes its pending boundary without a readiness sleep.
    #[farhelm_testtrace::test]
    async fn held_cache_admission_cancels_without_a_blocking_waiter() {
        use std::future::Future as _;
        for cancellation in ["off", "root", "stop"] {
            let fixture = Fixture::new();
            let held = crate::repo_cache::acquire(&fixture.cache).unwrap();
            let contender = crate::repo_cache::open_lock(&fixture.cache).unwrap();
            assert!(
                !contender.try_acquire().unwrap(),
                "premise: same inode is actually contested"
            );
            let builder = Builder::default();
            builder.register(fixture.trigger.clone());
            let desired = builder
                .desired
                .lock()
                .unwrap()
                .values()
                .next()
                .unwrap()
                .clone();
            let (stop, mut stopped) = watch::channel(false);
            let waiting = wait_cache(
                contender,
                &builder,
                &desired,
                &mut stopped,
                tokio::time::Instant::now() + Duration::from_secs(600),
            );
            tokio::pin!(waiting);
            std::future::poll_fn(|context| {
                assert!(
                    waiting.as_mut().poll(context).is_pending(),
                    "premise: admission waits on held cache"
                );
                std::task::Poll::Ready(())
            })
            .await;
            match cancellation {
                "off" => {
                    let mut off = fixture.trigger.clone();
                    off.enabled = false;
                    builder.register(off);
                }
                "root" => {
                    let mut moved = fixture.trigger.clone();
                    moved.root = moved.root.join("next");
                    builder.register(moved);
                }
                "stop" => {
                    stop.send(true).unwrap();
                }
                _ => unreachable!(),
            }
            assert!(
                tokio::time::timeout(Duration::from_secs(1), &mut waiting)
                    .await
                    .expect("cancel must not wait for the lock owner")
                    .unwrap()
                    .is_none()
            );
            // Retaining the original guard proves cancellation did not merely
            // coincide with the visible owner releasing its credential prompt.
            assert!(
                !crate::repo_cache::open_lock(&fixture.cache)
                    .unwrap()
                    .try_acquire()
                    .unwrap()
            );
            drop(held);
        }
    }

    /// Admission consumes the same bounded attempt as login/Git, rather than
    /// accumulating an unbounded pre-guard wait. The held flock remains held.
    #[farhelm_testtrace::test]
    async fn held_cache_admission_obeys_attempt_deadline() {
        use std::future::Future as _;
        let fixture = Fixture::new();
        let held = crate::repo_cache::acquire(&fixture.cache).unwrap();
        let contender = crate::repo_cache::open_lock(&fixture.cache).unwrap();
        assert!(!contender.try_acquire().unwrap());
        let builder = Builder::default();
        builder.register(fixture.trigger.clone());
        let desired = builder
            .desired
            .lock()
            .unwrap()
            .values()
            .next()
            .unwrap()
            .clone();
        let (_stop, mut stopped) = watch::channel(false);
        let waiting = wait_cache(
            contender,
            &builder,
            &desired,
            &mut stopped,
            tokio::time::Instant::now() + Duration::from_secs(1),
        );
        tokio::pin!(waiting);
        std::future::poll_fn(|context| {
            assert!(waiting.as_mut().poll(context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        let error = tokio::time::timeout(Duration::from_secs(5), &mut waiting)
            .await
            .expect("attempt deadline must finish admission")
            .err()
            .expect("held cache must not be admitted");
        assert!(error.to_string().contains("deadline exhausted"));
        assert!(
            !crate::repo_cache::open_lock(&fixture.cache)
                .unwrap()
                .try_acquire()
                .unwrap()
        );
        drop(held);
    }

    /// Busy cleanup fails without waiting or forgetting filesystem authority.
    /// Releasing the actual contested inode then permits the same path to clean.
    #[farhelm_testtrace::test]
    fn held_cache_cleanup_retains_record_and_building() {
        let fixture = Fixture::new();
        let held = crate::repo_cache::acquire(&fixture.cache).unwrap();
        assert!(
            !crate::repo_cache::open_lock(&fixture.cache)
                .unwrap()
                .try_acquire()
                .unwrap()
        );
        let error = discard_if_cache_free(&fixture.cache, &fixture.trigger.repo, None).unwrap_err();
        assert!(error.to_string().contains("cache is busy"));
        assert!(fixture.prepared.target.path().join("complete").is_file());
        assert!(fixture.cache.with_extension("spare.json").is_file());
        drop(held);
        discard_if_cache_free(&fixture.cache, &fixture.trigger.repo, None).unwrap();
        assert!(!fixture.prepared.target.path().exists());
        assert!(!fixture.cache.with_extension("spare.json").exists());
    }

    /// Off during record fsync must refuse the still-unpublished old token,
    /// without making fresh create admission wait for that fsync to finish.
    #[farhelm_testtrace::test]
    fn publication_rechecks_off_after_record_fsync() {
        let fixture = Fixture::new();
        let builder = Arc::new(Builder::default());
        builder.register(fixture.trigger.clone());
        let desired = builder
            .desired
            .lock()
            .unwrap()
            .values()
            .next()
            .unwrap()
            .clone();
        let (reached, observed) = mpsc::channel();
        let (release, held) = mpsc::channel();
        let seam = HeldSync {
            reached,
            release: Mutex::new(held),
        };
        std::thread::scope(|scope| {
            let build = builder.clone();
            let fixture_ref = &fixture;
            let seam_ref = &seam;
            let token = desired.token;
            let writing = scope.spawn(move || {
                publish_prepared(
                    &build,
                    &key(&fixture_ref.trigger.repo),
                    token,
                    &fixture_ref.prepared,
                    &fixture_ref.cache,
                    seam_ref,
                )
            });
            observed.recv_timeout(Duration::from_secs(10)).unwrap();
            assert!(
                !fixture.ready().exists(),
                "premise: slot still private during fsync"
            );
            let mut off = fixture.trigger.clone();
            off.enabled = false;
            builder.register(off);
            assert!(
                !builder.current(&desired),
                "registration completed while fsync is held"
            );
            release.send(()).unwrap();
            assert!(!writing.join().unwrap().unwrap());
        });
        assert!(!fixture.ready().exists());
        crate::spare_checkouts::discard_except(&fixture.cache, &fixture.trigger.repo, None)
            .unwrap();
        assert!(!fixture.prepared.target.path().exists());
    }

    /// If rename wins the serialization point first, the later off action owes
    /// discard rather than retroactively treating that publication as canceled.
    #[farhelm_testtrace::test]
    fn off_after_publication_discards_recorded_ready() {
        let fixture = Fixture::new();
        let builder = Builder::default();
        builder.register(fixture.trigger.clone());
        let desired = builder
            .desired
            .lock()
            .unwrap()
            .values()
            .next()
            .unwrap()
            .clone();
        assert!(
            publish_prepared(
                &builder,
                &key(&fixture.trigger.repo),
                desired.token,
                &fixture.prepared,
                &fixture.cache,
                &RealFs
            )
            .unwrap()
        );
        assert!(fixture.ready().join("complete").is_file());
        let mut off = fixture.trigger.clone();
        off.enabled = false;
        builder.register(off);
        assert!(!builder.current(&desired));
        crate::spare_checkouts::discard_except(&fixture.cache, &fixture.trigger.repo, None)
            .unwrap();
        assert!(!fixture.ready().exists());
    }

    /// Taking a just-published slot can precede old fsync/cleanup. Registration
    /// after the actual rename must owe a fresh gate, and old cleanup/consume
    /// must preserve both that gate and the same-policy ready content.
    #[farhelm_testtrace::test]
    fn create_after_publication_owes_replacement_before_old_cleanup() {
        let fixture = Fixture::new();
        let builder = Builder::default();
        builder.register(fixture.trigger.clone());
        let old = builder
            .desired
            .lock()
            .unwrap()
            .values()
            .next()
            .unwrap()
            .clone();
        assert!(builder.claim(&old));
        assert!(
            publish_prepared(
                &builder,
                &key(&fixture.trigger.repo),
                old.token,
                &fixture.prepared,
                &fixture.cache,
                &RealFs
            )
            .unwrap()
        );
        assert!(
            builder.desired.lock().unwrap().is_empty(),
            "publication retires the useful attempt"
        );
        let mut later = fixture.trigger.clone();
        later.session = uuid::Uuid::new_v4().to_string();
        builder.register(later);
        let next = builder
            .desired
            .lock()
            .unwrap()
            .values()
            .next()
            .unwrap()
            .clone();
        assert_ne!(old.token, next.token);
        assert!(builder.keep_ready(&old, true));
        discard_if_cache_free(
            &fixture.cache,
            &fixture.trigger.repo,
            Some(&fixture.trigger.root),
        )
        .unwrap();
        assert!(fixture.ready().join("complete").is_file());
        builder.consume(&old);
        assert!(builder.current(&next));
        // Model the take's real atomic slot removal with the strict claim name.
        // Runtime cleanup must retain it while replacement preparation proceeds.
        let claim = crate::spare_checkouts::claim_name(
            &fixture.trigger.repo.name,
            &next.trigger.session,
            0,
        )
        .unwrap();
        let claimed = fixture
            .ready()
            .parent()
            .unwrap()
            .join(claim.to_str().unwrap());
        fs::rename(fixture.ready(), &claimed).unwrap();
        let expected =
            DirectoryEvidence::capture(&fs::File::open(&fixture.trigger.root).unwrap()).unwrap();
        let replacement = crate::spare_checkouts::prepare(
            &fixture.cache,
            &fixture.trigger.repo,
            &fixture.trigger.root,
            expected,
        )
        .unwrap()
        .unwrap();
        assert!(replacement.target.path().is_dir());
        assert!(claimed.join("complete").is_file());
        assert!(builder.claim(&next));
    }

    /// Repeated same-policy creates owe only the existing attempt. Root/off
    /// replacement supplies a new gate and cannot be consumed by old completion.
    #[farhelm_testtrace::test]
    fn coalescing_preserves_one_attempt_and_replaces_changed_policy_gate() {
        let fixture = Fixture::new();
        let builder = Builder::default();
        builder.register(fixture.trigger.clone());
        let original = builder
            .desired
            .lock()
            .unwrap()
            .values()
            .next()
            .unwrap()
            .clone();
        assert!(builder.claim(&original));
        let mut same = fixture.trigger.clone();
        same.session = uuid::Uuid::new_v4().to_string();
        builder.register(same);
        assert!(builder.current(&original));
        assert_eq!(builder.desired.lock().unwrap().len(), 1);
        let mut moved = fixture.trigger.clone();
        moved.root = moved.root.join("next-root");
        moved.session = uuid::Uuid::new_v4().to_string();
        let new_session = moved.session.clone();
        builder.register(moved);
        builder.consume(&original);
        assert!(!builder.current(&original));
        let entries = builder.desired.lock().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries.values().next().unwrap().trigger.session,
            new_session
        );
    }

    /// An old asynchronous provenance read cannot claim or consume a newer
    /// pending same-policy create. Once claimed, repeated creates coalesce and
    /// consuming that one attempt does not leave a hidden retry obligation.
    #[farhelm_testtrace::test]
    fn pending_same_policy_replaces_gate_before_atomic_claim() {
        let fixture = Fixture::new();
        let builder = Builder::default();
        builder.register(fixture.trigger.clone());
        let old = builder
            .desired
            .lock()
            .unwrap()
            .values()
            .next()
            .unwrap()
            .clone();
        let mut newer = fixture.trigger.clone();
        newer.session = uuid::Uuid::new_v4().to_string();
        builder.register(newer);
        assert!(!builder.claim(&old));
        builder.consume(&old);
        let current = builder
            .desired
            .lock()
            .unwrap()
            .values()
            .next()
            .unwrap()
            .clone();
        assert_ne!(current.token, old.token);
        assert!(builder.claim(&current));
        assert!(!builder.claim(&current), "an attempt is claimed only once");
        builder.register(fixture.trigger.clone());
        assert!(builder.current(&current));
        builder.consume(&current);
        assert!(
            builder.desired.lock().unwrap().is_empty(),
            "no trailing attempt after failure/completion"
        );
    }

    /// Clone failure is a completed visible attempt, while a validation failure
    /// or an unstarted/live clone provides no completed-cache-work evidence.
    #[farhelm_testtrace::test]
    fn clone_gate_includes_failed_clone_without_promoting_other_failures() {
        for state in [
            PreparationState::NotStarted,
            PreparationState::CloneStarted,
            PreparationState::Failed {
                stage: "validate".into(),
                detail: "bad input".into(),
            },
            PreparationState::Failed {
                stage: "cwd-identity".into(),
                detail: "replaced".into(),
            },
        ] {
            assert!(!clone_finished(&state));
        }
        for state in [
            PreparationState::HookStarted,
            PreparationState::Ready,
            PreparationState::Failed {
                stage: "clone".into(),
                detail: "network".into(),
            },
            PreparationState::Failed {
                stage: "post-clone".into(),
                detail: "hook".into(),
            },
        ] {
            assert!(clone_finished(&state));
        }
    }
}
