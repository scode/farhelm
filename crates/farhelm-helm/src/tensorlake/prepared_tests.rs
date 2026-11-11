//! Preparation proofs use the build model's actual resource graph and barriers.
//! A fake installer establishes orchestration contracts; real cloned supervisors
//! are still required to prove boot and identity isolation.

use super::*;
use crate::tensorlake::prepared::{PreparedInstallFacts, PreparedSnapshot, SnapshotPreparer};
use farhelm_proto::tensorlake::recipes::PreparedStaleReason;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// A bounded installer stand-in whose held call proves the preparation phase.
/// Refusing copied state returns before the engine can create a checkpoint.
struct FakePreparer {
    started: watch::Sender<usize>,
    hold: watch::Sender<bool>,
    calls: AtomicUsize,
    copied_state: AtomicBool,
}

impl FakePreparer {
    /// Start unheld and successful; each test explicitly establishes its failure
    /// or held-install premise before admitting the engine task.
    fn new() -> Arc<Self> {
        Arc::new(Self {
            started: watch::channel(0).0,
            hold: watch::channel(false).0,
            calls: AtomicUsize::new(0),
            copied_state: AtomicBool::new(false),
        })
    }

    /// Wait for installation entry, rather than inferring it from elapsed time.
    async fn entered(&self, count: usize) {
        tokio::time::timeout(
            Duration::from_secs(5),
            self.started.subscribe().wait_for(|n| *n >= count),
        )
        .await
        .unwrap()
        .unwrap();
    }
}

#[async_trait]
impl SnapshotPreparer for FakePreparer {
    async fn prepare(
        &self,
        _: &str,
        _: &TensorlakeSshAccess,
    ) -> anyhow::Result<PreparedInstallFacts> {
        let count = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        self.started.send_replace(count);
        self.hold.subscribe().wait_for(|held| !*held).await.unwrap();
        anyhow::ensure!(
            !self.copied_state.load(Ordering::SeqCst),
            "copied supervisor.db refuses preparation"
        );
        Ok(facts())
    }
}

/// Valid absolute coordinates carry no identity and need no process environment.
fn facts() -> PreparedInstallFacts {
    PreparedInstallFacts {
        home: "/home/sandbox".into(),
        farhelm_path: "/home/sandbox/.local/lib/farhelm/farhelm".into(),
        state_dir: "/home/sandbox/.local/share/farhelm".into(),
        farhelm_version: farhelm_proto::BUILD_VERSION.into(),
    }
}

/// A complete persisted checkpoint with controllable age and fixed provenance.
fn record(at: i64) -> PreparedSnapshot {
    PreparedSnapshot {
        id: "prepared".into(),
        prepared_at: at,
        farhelm_version: farhelm_proto::BUILD_VERSION.into(),
        source_snapshot: "built".into(),
        disk_gb: 20,
        facts: facts(),
    }
}

/// Exactly 24 hours remains reusable; neither rollback nor unrelated recipe
/// settings can become an extra invalidation trigger.
#[test]
fn freshness_has_exactly_three_reasons_and_a_strict_age_boundary() {
    let p = record(1_000);
    let version = farhelm_proto::BUILD_VERSION;
    assert_eq!(p.stale_reason(87_400, version, Some("built")), None);
    assert_eq!(p.stale_reason(999, version, Some("built")), None);
    assert_eq!(
        p.stale_reason(87_401, version, Some("built")),
        Some(PreparedStaleReason::Expired)
    );
    assert_eq!(
        p.stale_reason(1_000, "new", Some("built")),
        Some(PreparedStaleReason::FarhelmUpdated)
    );
    assert_eq!(
        p.stale_reason(1_000, version, Some("other")),
        Some(PreparedStaleReason::SnapshotRebuilt)
    );
    assert_eq!(
        p.stale_reason(1_000, version, None),
        Some(PreparedStaleReason::SnapshotRebuilt)
    );
    assert_eq!(
        p.stale_reason(i64::MAX, "new", None),
        Some(PreparedStaleReason::FarhelmUpdated)
    );
    assert_eq!(
        p.stale_reason(i64::MAX, version, None),
        Some(PreparedStaleReason::SnapshotRebuilt)
    );
    assert_eq!(
        record(i64::MIN).stale_reason(i64::MAX, version, Some("built")),
        Some(PreparedStaleReason::Expired)
    );
}

/// Keep the real builder premise: preparation must clone the snapshot this
/// engine actually built, not a fabricated filesystem or identity proof.
async fn preparation_fixture() -> (Fixture, Arc<FakePreparer>, StoredRecipe) {
    let mut f = fixture().await;
    f.build().await;
    let preparer = FakePreparer::new();
    f.builds = Arc::new(RecipeBuilds::new(
        f.store.clone(),
        f.api.clone(),
        f.shell.clone(),
        f.builds.ssh_keys.clone(),
        BuildTimings::default(),
        preparer.clone(),
        farhelm_proto::BUILD_VERSION.into(),
    ));
    let frozen = f
        .store
        .tensorlake_recipe_row(RECIPE)
        .await
        .unwrap()
        .unwrap()
        .0;
    (f, preparer, frozen)
}

/// An owned waiter lets tests observe a held installer while the registry owns
/// the task; cancelling the waiter must not be the job's lifetime authority.
fn ensure(
    f: &Fixture,
    frozen: &StoredRecipe,
) -> tokio::task::JoinHandle<anyhow::Result<PreparedSnapshot>> {
    let builds = Arc::clone(&f.builds);
    let frozen = frozen.clone();
    let events = Arc::clone(&f.events);
    tokio::spawn(async move {
        builds
            .ensure_prepared_snapshot(RECIPE, &frozen, events, |_| async {})
            .await
    })
}

/// Concurrent consumers subscribe to one installation; the resulting fresh
/// record is reused without billed API calls or another progress wait.
#[farhelm_testtrace::test(start_paused = true)]
async fn concurrent_consumers_join_and_fresh_reuse_creates_nothing() {
    let (f, preparer, frozen) = preparation_fixture().await;
    preparer.hold.send_replace(true);
    let first = ensure(&f, &frozen);
    preparer.entered(1).await;
    let (joined, mut second_waiter_joined) = watch::channel(false);
    let builds = Arc::clone(&f.builds);
    let definition = frozen.clone();
    let events = Arc::clone(&f.events);
    let second = tokio::spawn(async move {
        builds
            .ensure_prepared_snapshot(RECIPE, &definition, events, move |message| {
                assert!(message.starts_with("Preparing snapshot:"));
                joined.send_replace(true);
                std::future::ready(())
            })
            .await
    });
    // The callback runs only after subscribing to this held preparation.
    // Spawning alone would also pass if the second task ran after completion.
    const SECOND_WAITER_JOIN_BOUND: Duration = Duration::from_secs(5);
    tokio::time::timeout(
        SECOND_WAITER_JOIN_BOUND,
        second_waiter_joined.wait_for(|joined| *joined),
    )
    .await
    .expect("the second waiter must observe live preparation progress")
    .unwrap();
    assert!(
        *preparer.hold.borrow(),
        "the installer is still held at subscription"
    );
    // A source guard is available during installation, so Rebuild and another
    // consumer are not serialized behind the full installation duration.
    let guard = tokio::time::timeout(Duration::from_secs(5), f.builds.lock_snapshot_use(RECIPE))
        .await
        .unwrap()
        .unwrap();
    drop(guard);
    preparer.hold.send_replace(false);
    let a = first.await.unwrap().unwrap();
    assert_eq!(a, second.await.unwrap().unwrap());
    assert_eq!(preparer.calls.load(Ordering::SeqCst), 1);
    let calls = f.api.ops().len();
    let mut edited = tests_support::save();
    edited.size.cpus = frozen.size.cpus + 1;
    edited.packages = vec!["jq".into()];
    f.store
        .update_tensorlake_recipe(RECIPE.into(), edited)
        .await
        .unwrap()
        .unwrap();
    let reused = f
        .builds
        .ensure_prepared_snapshot(RECIPE, &frozen, f.events.clone(), |_| async {
            panic!("a fresh checkpoint has no preparation wait")
        })
        .await
        .unwrap();
    assert_eq!(a, reused);
    assert_eq!(f.api.ops().len(), calls);
    assert!(f.state().await.preparer_sandbox_id.is_none());
    assert_eq!(f.snapshot_ids().len(), 2, "only built and prepared remain");
}

/// A later host disk increase cannot inflate the shared preparation source.
/// Publication retains the new snapshot before deleting the old checkpoint.
#[farhelm_testtrace::test(start_paused = true)]
async fn preparation_uses_source_disk_and_replacement_retains_current_snapshots() {
    let (f, preparer, mut frozen) = preparation_fixture().await;
    let source = f.state().await.snapshot.unwrap();
    frozen.size.disk_gb = source.disk_gb + 20;
    frozen.size.cpus = 4;
    frozen.size.memory_gb = 8;
    let old = ensure(&f, &frozen).await.unwrap().unwrap();
    *f.api.verify_prepared_retirement.lock().unwrap() = Some((old.id.clone(), f.store.clone()));
    f.set_state(|state| state.prepared.as_mut().unwrap().prepared_at = 0)
        .await;
    let new = ensure(&f, &frozen).await.unwrap().unwrap();
    assert_ne!(old.id, new.id);
    assert_eq!(preparer.calls.load(Ordering::SeqCst), 2);
    {
        let model = f.api.model();
        let sandboxes: Vec<_> = model
            .sandboxes
            .iter()
            .filter(|s| {
                s.name
                    .as_deref()
                    .is_some_and(|n| n.starts_with(PREPARER_PREFIX))
            })
            .collect();
        assert_eq!(sandboxes.len(), 2);
        for sandbox in sandboxes {
            assert_eq!(sandbox.resources.disk_mb, u64::from(source.disk_gb) * 1024);
            assert_eq!(sandbox.resources.cpus, 4.0);
            assert_eq!(sandbox.resources.memory_mb, 8192);
            assert_eq!(sandbox.snapshot_id.as_deref(), Some(source.id.as_str()));
            assert_eq!(sandbox.timeout_secs, 900);
            assert!(!sandbox.auto_resume);
        }
    }
    assert!(!f.snapshot_ids().contains(&old.id));
    assert!(f.snapshot_ids().contains(&new.id));
    assert!(f.snapshot_ids().contains(&source.id));
    assert!(f.state().await.retired_snapshots.is_empty());
}

/// A copied-state refusal must prevent checkpoint creation, retain the prior
/// stale metadata, and expose a lazy retry rather than returning that old record.
#[farhelm_testtrace::test(start_paused = true)]
async fn copied_state_failure_prevents_checkpoint_and_keeps_previous_record() {
    let (f, preparer, frozen) = preparation_fixture().await;
    let old = ensure(&f, &frozen).await.unwrap().unwrap();
    f.set_state(|state| state.prepared.as_mut().unwrap().prepared_at = 0)
        .await;
    let before = f
        .api
        .ops()
        .iter()
        .filter(|op| op.starts_with("create_snapshot "))
        .count();
    preparer.copied_state.store(true, Ordering::SeqCst);
    let error = ensure(&f, &frozen).await.unwrap().unwrap_err();
    assert!(error.to_string().contains("supervisor.db"));
    assert_eq!(
        f.api
            .ops()
            .iter()
            .filter(|op| op.starts_with("create_snapshot "))
            .count(),
        before
    );
    let state = f.state().await;
    assert_eq!(state.prepared.as_ref().unwrap().id, old.id);
    assert_eq!(
        state.last_prepare_failure.unwrap().step,
        PrepareStep::Installing
    );
    assert!(
        f.builds
            .fresh_prepared_snapshot(RECIPE)
            .await
            .unwrap()
            .is_none()
    );
    assert!(state.preparer_sandbox_id.is_none());
}

/// Rebuild must remain a real build during preparation. A checkpoint produced
/// from the superseded source is recorded then renewed before the waiter returns.
#[farhelm_testtrace::test(start_paused = true)]
async fn rebuilding_during_installation_rechecks_the_prepared_source() {
    let (f, preparer, frozen) = preparation_fixture().await;
    preparer.hold.send_replace(true);
    let pending = ensure(&f, &frozen);
    preparer.entered(1).await;
    let old_source = f.state().await.snapshot.unwrap().id;
    f.build().await;
    let new_source = f.state().await.snapshot.unwrap().id;
    assert_ne!(old_source, new_source, "Rebuild did actual build work");
    preparer.hold.send_replace(false);
    let result = pending.await.unwrap().unwrap();
    assert_eq!(result.source_snapshot, new_source);
    assert_eq!(preparer.calls.load(Ordering::SeqCst), 2);
}

/// An older waiter can resume after another waiter started renewal. Hold that
/// exact observation gap: the old attempt's closed watch must lead into joining
/// the new attempt, not interpreting its durable start as an interruption.
#[farhelm_testtrace::test(start_paused = true)]
async fn an_older_waiter_joins_renewal_already_started_by_another_waiter() {
    let (f, preparer, frozen) = preparation_fixture().await;
    preparer.hold.send_replace(true);
    let builds = Arc::clone(&f.builds);
    let definition = frozen.clone();
    let events = Arc::clone(&f.events);
    let installer = Arc::clone(&preparer);
    let first = tokio::spawn(async move {
        builds
            .ensure_prepared_snapshot(RECIPE, &definition, events, move |message| {
                // A has finished installing; hold B before the first waiter can
                // launch it, so the second waiter must observe B's live marker.
                if message.contains("finishing up") {
                    installer.hold.send_replace(true);
                }
                std::future::ready(())
            })
            .await
    });
    preparer.entered(1).await;
    let (release, release_rx) = watch::channel(false);
    let (callbacks, mut observed) = watch::channel(0usize);
    let builds = Arc::clone(&f.builds);
    let definition = frozen.clone();
    let events = Arc::clone(&f.events);
    let second = tokio::spawn(async move {
        let mut first_callback = true;
        builds
            .ensure_prepared_snapshot(RECIPE, &definition, events, move |_| {
                callbacks.send_modify(|count| *count += 1);
                let hold = std::mem::take(&mut first_callback);
                let mut release = release_rx.clone();
                async move {
                    if hold {
                        release.wait_for(|released| *released).await.unwrap();
                    }
                }
            })
            .await
    });
    tokio::time::timeout(
        Duration::from_secs(5),
        observed.wait_for(|count| *count >= 1),
    )
    .await
    .unwrap()
    .unwrap();
    f.build().await;
    let current_source = f.state().await.snapshot.unwrap().id;
    preparer.hold.send_replace(false);
    preparer.entered(2).await;
    assert!(
        f.state().await.prepare_started_at.is_some(),
        "renewal B is running"
    );
    release.send_replace(true);
    // Initial A progress, A's final progress, then live B progress. The old
    // false-interruption path exits before that third callback can happen.
    tokio::time::timeout(
        Duration::from_secs(5),
        observed.wait_for(|count| *count >= 3),
    )
    .await
    .unwrap()
    .unwrap();
    preparer.hold.send_replace(false);
    let first = first.await.unwrap().unwrap();
    let second = second.await.unwrap().unwrap();
    assert_eq!(first.id, second.id);
    assert_eq!(first.source_snapshot, current_source);
    assert_eq!(preparer.calls.load(Ordering::SeqCst), 2);
}

/// A start marker never grants ownership of an unrelated same-name sandbox.
/// Only a persisted outstanding request permits recovery and termination.
#[farhelm_testtrace::test(start_paused = true)]
async fn interrupted_preparation_requires_pending_create_for_name_recovery() {
    let (f, _, _) = preparation_fixture().await;
    let tag = helm_tag(&f.store.registry_namespace().await.unwrap());
    let name = preparer_name(&tag, RECIPE);
    seed_sandbox(&mut f.api.model(), "unrelated", &name, "running");
    f.set_state(|state| state.prepare_started_at = Some(1))
        .await;
    let key = ApiKey::new("test-key".into());
    f.builds
        .reconcile_preparer(RECIPE, &key, &name)
        .await
        .unwrap();
    assert!(
        !f.api
            .ops()
            .iter()
            .any(|op| op == "terminate_sandbox unrelated")
    );
    f.set_state(|state| state.prepare_pending_create_at = Some(now_secs()))
        .await;
    f.builds
        .reconcile_preparer(RECIPE, &key, &name)
        .await
        .unwrap();
    assert!(
        f.api
            .ops()
            .iter()
            .any(|op| op == "terminate_sandbox unrelated")
    );
    assert!(f.state().await.prepare_pending_create_at.is_none());
    assert!(f.state().await.preparer_sandbox_id.is_none());
}

/// Delete cancels installation outside source exclusion and then clears both
/// resource roles; no prepared or built checkpoint can keep billing unrecorded.
#[farhelm_testtrace::test(start_paused = true)]
async fn recipe_deletion_cancels_preparation_and_cleans_both_roles() {
    let (f, preparer, frozen) = preparation_fixture().await;
    let old = ensure(&f, &frozen).await.unwrap().unwrap();
    f.set_state(|state| state.prepared.as_mut().unwrap().prepared_at = 0)
        .await;
    preparer.hold.send_replace(true);
    let waiting = ensure(&f, &frozen);
    preparer.entered(2).await;
    f.shell.hold.send_replace(true);
    f.start().await;
    f.shell.started(6).await;
    let guard = f.builds.clean_for_delete(RECIPE).await.unwrap();
    assert!(!f.state().await.needs_cleanup());
    assert!(!f.snapshot_ids().contains(&old.id));
    assert!(f.snapshot_ids().is_empty());
    assert!(f.builds.lock_snapshot_use(RECIPE).await.is_err());
    assert!(waiting.await.unwrap().is_err());
    drop(guard);
}

/// Cancellation cannot drop create while Tensorlake has already committed the
/// resource. The pending marker remains until its ID is durably recorded.
#[farhelm_testtrace::test(start_paused = true)]
async fn delete_waits_for_preparer_create_and_durable_record() {
    let (f, preparer, frozen) = preparation_fixture().await;
    f.api.create_committed.send_replace(false);
    f.api.hold_create.send_replace(true);
    let waiting = ensure(&f, &frozen);
    f.api
        .create_committed
        .subscribe()
        .wait_for(|created| *created)
        .await
        .unwrap();
    let state = f.state().await;
    assert!(state.prepare_pending_create_at.is_some());
    assert!(state.preparer_sandbox_id.is_none());
    let mut cancellation = f
        .builds
        .registry()
        .preparing
        .get(RECIPE)
        .unwrap()
        .cancel
        .subscribe();
    let builds = Arc::clone(&f.builds);
    let deletion = tokio::spawn(async move { builds.clean_for_delete(RECIPE).await.map(drop) });
    tokio::time::timeout(
        Duration::from_secs(5),
        cancellation.wait_for(|cancelled| *cancelled),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        !deletion.is_finished(),
        "create is still held after cancellation"
    );
    f.api.hold_create.send_replace(false);
    deletion.await.unwrap().unwrap();
    assert!(waiting.await.unwrap().is_err());
    assert_eq!(preparer.calls.load(Ordering::SeqCst), 0);
    assert!(!f.state().await.needs_cleanup());
    assert!(
        f.api
            .model()
            .sandboxes
            .iter()
            .all(|sandbox| sandbox.status == "terminated")
    );
}

/// A current prepared snapshot is protected even when its source is a builder
/// being swept, and cannot be selected as another recipe's removable base.
#[farhelm_testtrace::test(start_paused = true)]
async fn prepared_snapshot_is_protected_by_sweeps_and_owned_base_refusal() {
    let (f, _, _) = preparation_fixture().await;
    let builder = f.builder_ids()[0].clone();
    let id = f.api.model().plant_snapshot(&builder);
    let mut prepared = record(now_secs());
    prepared.id = id.clone();
    f.set_state(move |state| state.prepared = Some(prepared))
        .await;
    let key = ApiKey::new("test-key".into());
    f.builds
        .sweep_builder(RECIPE, &key, &builder)
        .await
        .unwrap();
    assert!(f.snapshot_ids().contains(&id));
    let mut save = tests_support::save();
    save.base = RecipeBase::TensorlakeSnapshot { snapshot_id: id };
    assert!(matches!(
        f.store
            .create_tensorlake_recipe("other".into(), save)
            .await
            .unwrap(),
        Err(crate::tensorlake::recipes::Refusal::Invalid(_))
    ));
}

/// Store deletion must retain every new ownership form, including a pending
/// create with no observed ID and an interrupted task that has not created yet.
#[farhelm_testtrace::test(start_paused = true)]
async fn every_preparation_ownership_record_requires_cleanup() {
    let f = fixture().await;
    for state in [
        RecipeState {
            prepared: Some(record(1)),
            ..RecipeState::default()
        },
        RecipeState {
            preparer_sandbox_id: Some("id".into()),
            ..RecipeState::default()
        },
        RecipeState {
            prepare_pending_create_at: Some(1),
            ..RecipeState::default()
        },
        RecipeState {
            prepare_started_at: Some(1),
            ..RecipeState::default()
        },
    ] {
        let expected = state.clone();
        f.set_state(move |stored| *stored = state).await;
        assert_eq!(
            f.state().await,
            expected,
            "premise: this ownership form is durable"
        );
        assert_eq!(
            f.store
                .delete_tensorlake_recipe(RECIPE.into())
                .await
                .unwrap(),
            Err(crate::tensorlake::recipes::Refusal::Billed)
        );
        assert_eq!(
            f.state().await,
            expected,
            "refusal preserves the complete ownership row"
        );
    }
    f.set_state(|state| *state = RecipeState::default()).await;
    assert_eq!(
        f.store
            .delete_tensorlake_recipe(RECIPE.into())
            .await
            .unwrap(),
        Ok(())
    );
    assert!(
        f.store
            .tensorlake_recipe_row(RECIPE)
            .await
            .unwrap()
            .is_none()
    );
}

/// An ID held only by the failed task is not ownership evidence for termination.
/// Keep the pending marker so deletion can adopt durably before cleaning it up.
#[farhelm_testtrace::test(start_paused = true)]
async fn failed_preparer_record_leaves_pending_ownership_for_delete() {
    let (f, preparer, frozen) = preparation_fixture().await;
    f.builds.fail_preparer_record.store(true, Ordering::SeqCst);
    assert!(ensure(&f, &frozen).await.unwrap().is_err());
    let state = f.state().await;
    assert!(state.prepare_pending_create_at.is_some());
    assert!(state.preparer_sandbox_id.is_none());
    assert_eq!(preparer.calls.load(Ordering::SeqCst), 0);
    assert!(f.api.model().sandboxes.iter().any(|sandbox| {
        sandbox.status == "running"
            && sandbox
                .name
                .as_deref()
                .is_some_and(|name| name.starts_with(PREPARER_PREFIX))
    }));
    let guard = f.builds.clean_for_delete(RECIPE).await.unwrap();
    assert!(!f.state().await.needs_cleanup());
    assert!(
        f.api
            .model()
            .sandboxes
            .iter()
            .all(|sandbox| sandbox.status == "terminated")
    );
    drop(guard);
}

/// A temporarily absent name cannot erase billing ownership before the shared
/// create settle window, even when no sandbox ID was ever observed.
#[farhelm_testtrace::test(start_paused = true)]
async fn pending_preparation_absence_waits_for_the_settle_boundary() {
    let (f, _, _) = preparation_fixture().await;
    let tag = helm_tag(&f.store.registry_namespace().await.unwrap());
    let name = preparer_name(&tag, RECIPE);
    let key = ApiKey::new("test-key".into());
    f.set_state(|state| state.prepare_pending_create_at = Some(now_secs()))
        .await;
    assert!(
        f.builds
            .reconcile_preparer(RECIPE, &key, &name)
            .await
            .is_err()
    );
    assert!(f.state().await.prepare_pending_create_at.is_some());
    f.set_state(|state| state.prepare_pending_create_at = Some(now_secs() - CREATE_SETTLE_SECS))
        .await;
    f.builds
        .reconcile_preparer(RECIPE, &key, &name)
        .await
        .unwrap();
    assert!(f.state().await.prepare_pending_create_at.is_none());
}

/// A host's source-use guard blocks retirement, while independent build work
/// can finish its expensive phases. Releasing it allows atomic publication.
#[farhelm_testtrace::test(start_paused = true)]
async fn source_use_excludes_build_publication_and_retirement() {
    let (f, _, _) = preparation_fixture().await;
    let old = f.state().await.snapshot.unwrap().id;
    // Initial retired cleanup also takes source exclusion. Let that finish,
    // then hold an actual build command before acquiring the consumer's guard.
    let next_command = *f.shell.started.borrow() + 1;
    f.shell.hold.send_replace(true);
    f.start().await;
    f.shell.started(next_command).await;
    let guard = f.builds.lock_snapshot_use(RECIPE).await.unwrap();
    let mut progress = f
        .builds
        .registry()
        .active
        .get(RECIPE)
        .unwrap()
        .progress
        .subscribe();
    f.shell.hold.send_replace(false);
    tokio::time::timeout(
        Duration::from_secs(30),
        progress.wait_for(|progress| progress.step == BuildStep::Finishing),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(f.state().await.snapshot.unwrap().id, old);
    assert!(!f.api.ops().contains(&format!("delete_snapshot {old}")));
    drop(guard);
    f.finish().await;
    assert_ne!(f.state().await.snapshot.unwrap().id, old);
    assert!(f.api.ops().contains(&format!("delete_snapshot {old}")));
}
