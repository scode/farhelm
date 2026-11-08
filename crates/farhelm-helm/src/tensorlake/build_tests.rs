//! Snapshot build proofs: the engine against an in-memory Tensorlake and a
//! scripted sandbox shell, the store's billing invariants, and the build
//! route.
//!
//! The in-memory API drives the same [`FakeModel`] the browser suite's fake
//! persists, so both levels share Tensorlake's rules (name uniqueness, 404s,
//! snapshots that complete after a poll). Engine tests run under paused
//! tokio time: the engine's polls and deadlines advance instantly instead of
//! sleeping, and the store's blocking calls hold the clock while they run.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex as StdMutex;

use super::*;
use crate::tensorlake::api::{
    CreatedSandbox, CreatedSnapshot, SandboxInfo, SandboxPage, SnapshotInfo, SnapshotList,
};
use crate::tensorlake::e2e::{FakeModel, FakeSandbox, default_stdout};
use crate::tensorlake::recipes::tests_support;
use crate::tensorlake::ssh::AUTH_REFUSED_MESSAGE;
use async_trait::async_trait;
use farhelm_proto::tensorlake::SaveSettings;

// ---------------------------------------------------------------------------
// Fakes
// ---------------------------------------------------------------------------

/// Tensorlake in memory: the shared model, a log of every call as
/// `"<op> <argument>"`, and failures injected per logged call, either for
/// every call (`fail`) or for the next one only (`fail_once`).
///
/// A create can also be made to commit and then lose its answer: `create_reply`
/// replaces the next create's reply with an error after the sandbox exists,
/// and `hold_create` keeps a committed create from answering until released
/// (`create_committed` says when the sandbox exists).
struct MemoryApi {
    model: StdMutex<FakeModel>,
    ops: StdMutex<Vec<String>>,
    fail: StdMutex<HashMap<String, TensorlakeError>>,
    fail_once: StdMutex<HashMap<String, TensorlakeError>>,
    create_reply: StdMutex<Option<TensorlakeError>>,
    hold_create: watch::Sender<bool>,
    create_committed: watch::Sender<bool>,
    /// Inspect durable replacement at the actual deletion boundary, not just
    /// after both publication and retirement have already completed.
    verify_prepared_retirement: StdMutex<Option<(String, HelmStore)>>,
}

impl Default for MemoryApi {
    fn default() -> Self {
        Self {
            model: StdMutex::default(),
            ops: StdMutex::default(),
            fail: StdMutex::default(),
            fail_once: StdMutex::default(),
            create_reply: StdMutex::default(),
            hold_create: watch::channel(false).0,
            create_committed: watch::channel(false).0,
            verify_prepared_retirement: StdMutex::new(None),
        }
    }
}

impl MemoryApi {
    /// Log the call and answer an injected failure if one is set for it.
    fn call(&self, op: &str, argument: &str) -> Result<(), TensorlakeError> {
        let entry = format!("{op} {argument}");
        self.ops.lock().unwrap().push(entry.clone());
        if let Some(error) = self.fail_once.lock().unwrap().remove(&entry) {
            return Err(error);
        }
        match self.fail.lock().unwrap().get(&entry) {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }

    fn fail_once(&self, entry: &str, error: TensorlakeError) {
        self.fail_once.lock().unwrap().insert(entry.into(), error);
    }

    fn ops(&self) -> Vec<String> {
        self.ops.lock().unwrap().clone()
    }

    fn model(&self) -> std::sync::MutexGuard<'_, FakeModel> {
        self.model.lock().unwrap()
    }

    /// Position of the first logged call equal to `entry`.
    fn position(&self, entry: &str) -> usize {
        self.ops()
            .iter()
            .position(|op| op == entry)
            .unwrap_or_else(|| panic!("{entry:?} was never called: {:?}", self.ops()))
    }
}

#[async_trait]
impl TensorlakeApi for MemoryApi {
    async fn list_sandboxes(
        &self,
        _: &ApiKey,
        _: Option<&str>,
    ) -> Result<SandboxPage, TensorlakeError> {
        self.call("list_sandboxes", "")?;
        Ok(SandboxPage {
            sandboxes: self.model().live_sandboxes(),
            next_cursor: None,
        })
    }
    async fn get_sandbox(&self, _: &ApiKey, sandbox: &str) -> Result<SandboxInfo, TensorlakeError> {
        self.call("get_sandbox", sandbox)?;
        self.model().get(sandbox)
    }
    async fn create_sandbox(
        &self,
        _: &ApiKey,
        request: &CreateSandbox,
    ) -> Result<CreatedSandbox, TensorlakeError> {
        self.call("create_sandbox", &request.name)?;
        let created = self.model().create(request)?;
        self.create_committed.send_replace(true);
        self.hold_create
            .subscribe()
            .wait_for(|held| !*held)
            .await
            .unwrap();
        match self.create_reply.lock().unwrap().take() {
            Some(error) => Err(error),
            None => Ok(created),
        }
    }
    async fn suspend_sandbox(&self, _: &ApiKey, _: &str) -> Result<(), TensorlakeError> {
        unreachable!("builds never suspend")
    }
    async fn resume_sandbox(&self, _: &ApiKey, _: &str) -> Result<(), TensorlakeError> {
        unreachable!("builds never resume")
    }
    async fn terminate_sandbox(&self, _: &ApiKey, sandbox: &str) -> Result<(), TensorlakeError> {
        self.call("terminate_sandbox", sandbox)?;
        self.model().terminate(sandbox)
    }
    async fn create_snapshot(
        &self,
        _: &ApiKey,
        sandbox: &str,
        snapshot_type: SnapshotType,
    ) -> Result<CreatedSnapshot, TensorlakeError> {
        assert_eq!(snapshot_type, SnapshotType::Filesystem);
        self.call("create_snapshot", sandbox)?;
        self.model().snapshot(sandbox, 1)
    }
    async fn get_snapshot(&self, _: &ApiKey, id: &str) -> Result<SnapshotInfo, TensorlakeError> {
        self.call("get_snapshot", id)?;
        self.model().poll_snapshot(id)
    }
    async fn list_snapshots(&self, _: &ApiKey) -> Result<SnapshotList, TensorlakeError> {
        self.call("list_snapshots", "")?;
        Ok(self.model().list_snapshots())
    }
    async fn delete_snapshot(&self, _: &ApiKey, id: &str) -> Result<(), TensorlakeError> {
        let verify = self.verify_prepared_retirement.lock().unwrap().clone();
        if let Some((retired, store)) = verify.filter(|(retired, _)| retired.as_str() == id) {
            let state = store
                .tensorlake_recipe_row(RECIPE)
                .await
                .unwrap()
                .unwrap()
                .1;
            assert_ne!(
                state.prepared.as_ref().unwrap().id,
                retired,
                "replacement must be durable before old checkpoint deletion"
            );
            assert!(state.retired_snapshots.contains(&retired));
        }
        self.call("delete_snapshot", id)?;
        self.model().delete_snapshot(id)
    }
}

/// A sandbox shell that answers from a script per step label, defaulting to
/// success with the e2e fake's stdout. It refuses a sandbox the model does
/// not have running, can hold every command until released, and can plant a
/// snapshot of the sandbox when a given step runs (standing in for a
/// suspend snapshot or a checkpoint the build never recorded).
struct ScriptedShell {
    api: Arc<MemoryApi>,
    answers: StdMutex<HashMap<String, VecDeque<Result<ShellOutput, ShellFailure>>>>,
    calls: StdMutex<Vec<(String, String)>>,
    started: watch::Sender<usize>,
    hold: watch::Sender<bool>,
    plant_on: StdMutex<Option<String>>,
    planted: StdMutex<Vec<String>>,
}

impl ScriptedShell {
    fn new(api: Arc<MemoryApi>) -> Arc<Self> {
        Arc::new(Self {
            api,
            answers: StdMutex::default(),
            calls: StdMutex::default(),
            started: watch::channel(0).0,
            hold: watch::channel(false).0,
            plant_on: StdMutex::default(),
            planted: StdMutex::default(),
        })
    }

    /// Queue `answer` for the next run of `label`.
    fn answer(&self, label: &str, answer: Result<ShellOutput, ShellFailure>) {
        self.answers
            .lock()
            .unwrap()
            .entry(label.into())
            .or_default()
            .push_back(answer);
    }

    fn labels(&self) -> Vec<String> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .map(|(_, label)| label.clone())
            .collect()
    }

    /// Wait until `count` commands have started.
    async fn started(&self, count: usize) {
        self.started
            .subscribe()
            .wait_for(|started| *started >= count)
            .await
            .unwrap();
    }
}

#[async_trait]
impl SandboxShell for ScriptedShell {
    async fn run(
        &self,
        _: &TensorlakeSshAccess,
        sandbox: &str,
        label: &str,
        _: &str,
        _: Duration,
    ) -> Result<ShellOutput, ShellFailure> {
        self.calls
            .lock()
            .unwrap()
            .push((sandbox.to_string(), label.to_string()));
        self.started.send_modify(|started| *started += 1);
        self.hold.subscribe().wait_for(|held| !*held).await.unwrap();
        if !self.api.model().is_running(sandbox) {
            return Err(ShellFailure::Unreachable(format!(
                "{sandbox} is not running"
            )));
        }
        if self.plant_on.lock().unwrap().as_deref() == Some(label) {
            let mut model = self.api.model();
            let id = model.get(sandbox).unwrap().id;
            let planted = model.plant_snapshot(&id);
            self.planted.lock().unwrap().push(planted);
        }
        let queued = self
            .answers
            .lock()
            .unwrap()
            .get_mut(label)
            .and_then(VecDeque::pop_front);
        queued.unwrap_or_else(|| {
            Ok(ShellOutput {
                stdout: default_stdout(label).to_string(),
            })
        })
    }
}

// ---------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------

const RECIPE: &str = "rust-dev";

#[path = "prepared_tests.rs"]
mod prepared_tests;

/// An add must join Settings' active build and use the resulting snapshot;
/// asking for it again must reuse it without another billed builder. The
/// first progress callback proves the add is waiting before release.
#[farhelm_testtrace::test]
async fn ensure_snapshot_joins_a_build_and_then_reuses_it() {
    let f = fixture().await;
    f.shell.hold.send_replace(true);
    f.start().await;
    f.shell.started(1).await;
    let frozen = f
        .store
        .tensorlake_recipe_row(RECIPE)
        .await
        .unwrap()
        .unwrap()
        .0;
    let builds = Arc::clone(&f.builds);
    let events = Arc::clone(&f.events);
    let definition = frozen.clone();
    let (observed, mut observation) = watch::channel(false);
    let waiting = tokio::spawn(async move {
        builds
            .ensure_snapshot(RECIPE, &definition, events, move |_| {
                observed.send_replace(true);
                std::future::ready(())
            })
            .await
            .unwrap()
    });
    tokio::time::timeout(Duration::from_secs(5), observation.wait_for(|seen| *seen))
        .await
        .unwrap()
        .unwrap();
    f.shell.hold.send_replace(false);
    let id = waiting.await.unwrap();
    let reused = f
        .builds
        .ensure_snapshot(RECIPE, &frozen, Arc::clone(&f.events), |_| async {
            panic!("a ready snapshot needs no build progress")
        })
        .await
        .unwrap();
    assert_eq!(id, reused);
    assert_eq!(
        f.api
            .ops()
            .iter()
            .filter(|op| op.starts_with("create_sandbox "))
            .count(),
        1
    );
}

struct Fixture {
    _dir: tempfile::TempDir,
    store: HelmStore,
    api: Arc<MemoryApi>,
    shell: Arc<ScriptedShell>,
    builds: Arc<RecipeBuilds>,
    events: Arc<FleetEvents>,
    builder: String,
}

/// A store with an API key, a stand-in ssh key on disk (nothing runs ssh),
/// the managed recipe [`RECIPE`], and an engine over the fakes.
async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let store = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
    store
        .save_tensorlake_config(SaveSettings {
            api_key: Some("test-key".into()),
            ..SaveSettings::default()
        })
        .await
        .ok()
        .expect("save the API key");
    let tensorlake_dir = dir.path().join("tensorlake");
    std::fs::create_dir_all(tensorlake_dir.join("ssh-key")).unwrap();
    std::fs::write(tensorlake_dir.join("ssh-key/id_ed25519"), "private").unwrap();
    std::fs::write(
        tensorlake_dir.join("ssh-key/id_ed25519.pub"),
        "ssh-ed25519 AAAA test\n",
    )
    .unwrap();
    tests_support::create(&store, RECIPE).await;
    let api = Arc::new(MemoryApi::default());
    let shell = ScriptedShell::new(Arc::clone(&api));
    let builds = Arc::new(RecipeBuilds::new(
        store.clone(),
        api.clone(),
        shell.clone(),
        Some(Arc::new(SshKeyStore::new(tensorlake_dir))),
        BuildTimings::default(),
        Arc::new(super::super::prepared::UnavailablePreparer),
        farhelm_proto::BUILD_VERSION.to_string(),
    ));
    let tag = helm_tag(&store.registry_namespace().await.unwrap());
    Fixture {
        _dir: dir,
        store,
        api,
        shell,
        builds,
        events: Arc::new(FleetEvents::new()),
        builder: builder_name(&tag, RECIPE),
    }
}

impl Fixture {
    async fn start(&self) {
        self.builds
            .start_or_join(RECIPE, Arc::clone(&self.events))
            .await
            .expect("the build starts");
    }

    /// Wait for the recipe's running build, if any, to end.
    async fn finish(&self) {
        let task = self
            .builds
            .registry()
            .active
            .get(RECIPE)
            .and_then(|active| active.task.lock().unwrap().take());
        if let Some(task) = task {
            task.await.unwrap();
        }
    }

    async fn build(&self) {
        self.start().await;
        self.finish().await;
    }

    async fn state(&self) -> RecipeState {
        self.store
            .tensorlake_recipe_row(RECIPE)
            .await
            .unwrap()
            .unwrap()
            .1
    }

    async fn set_state(&self, change: impl FnOnce(&mut RecipeState) + Send + 'static) {
        self.store
            .update_tensorlake_recipe_state(RECIPE, move |_, state| change(state))
            .await
            .unwrap()
            .unwrap();
    }

    async fn build_view(&self) -> BuildView {
        self.builds.view(RECIPE, &self.state().await).1
    }

    /// The sandbox IDs the model has under the builder's name.
    fn builder_ids(&self) -> Vec<String> {
        self.api
            .model()
            .sandboxes
            .iter()
            .filter(|s| s.name.as_deref() == Some(self.builder.as_str()))
            .map(|s| s.id.clone())
            .collect()
    }

    fn snapshot_ids(&self) -> Vec<String> {
        self.api
            .model()
            .snapshots
            .iter()
            .map(|s| s.id.clone())
            .collect()
    }
}

/// A seeded sandbox the model knows; `name` may be anything, `status` is
/// `running` or `terminated`.
fn seed_sandbox(model: &mut FakeModel, id: &str, name: &str, status: &str) {
    model.sandboxes.push(FakeSandbox {
        id: id.into(),
        name: Some(name.into()),
        status: status.into(),
        image: Some(TENSORLAKE_BASE_IMAGE.into()),
        snapshot_id: None,
        resources: Resources {
            cpus: 2.0,
            memory_mb: 4096,
            disk_mb: 20480,
        },
        timeout_secs: BUILDER_IDLE_TIMEOUT_SECS,
        auto_resume: false,
    });
}

// ---------------------------------------------------------------------------
// Names
// ---------------------------------------------------------------------------

/// Spec: a builder is `farhelm-build-<tag>-<recipe>`, at most 63 bytes for
/// the longest tag and recipe name, never exactly 21 letters and digits
/// (Tensorlake refuses that shape, which is an ID's), and never in the
/// browser harness's `farhelm-sbx-` namespace.
///
/// Why: the name is the builder's identity across crashes and the orphan
/// list's attribution; a name Tensorlake refuses would fail every build, and
/// one in the harness's namespace would let the two mistake each other's
/// sandboxes.
#[farhelm_testtrace::test]
fn builder_names_fit_tensorlakes_rules_and_stay_out_of_the_harness_namespace() {
    let longest = builder_name("zzzz", &"r".repeat(32));
    assert_eq!(longest.len(), 51);
    assert!(longest.len() <= 63);
    let short = builder_name("k7q2", "a");
    assert_eq!(short, "farhelm-build-k7q2-a");
    for name in [&longest, &short] {
        assert!(!(name.len() == 21 && name.bytes().all(|b| b.is_ascii_alphanumeric())));
        assert!(!name.starts_with("farhelm-sbx-"));
    }
}

// ---------------------------------------------------------------------------
// The engine
// ---------------------------------------------------------------------------

/// Spec: a build looks for leftovers, creates the builder from the base
/// image with the recipe's size, a 900-second idle timeout and
/// `auto_resume` off, runs ready, packages, claude, codex and prepare in that
/// order against the builder's name, snapshots it, and on completion
/// records the snapshot (with the agents' version lines, packages and disk),
/// terminates the builder and deletes nothing it records; the state ends
/// with no builder, no build running and no failure.
///
/// Why: this is the happy path every Rebuild takes; the create request is
/// what bills, and the end state is the promise that nothing but the
/// recorded snapshot is left.
#[farhelm_testtrace::test(start_paused = true)]
async fn a_build_records_its_snapshot_and_leaves_nothing_else_billing() {
    let f = fixture().await;
    f.build().await;
    let state = f.state().await;
    let snapshot = state.snapshot.clone().expect("a snapshot is recorded");
    assert_eq!(
        (
            snapshot.base.clone(),
            snapshot.disk_gb,
            snapshot.packages.clone(),
            snapshot.claude_version.as_deref(),
            snapshot.codex_version.as_deref(),
        ),
        (
            RecipeBase::TensorlakeUbuntu,
            20,
            vec!["git".to_string(), "ripgrep".to_string()],
            Some("1.0.0 (Claude Code)"),
            Some("codex-cli 0.1.0"),
        )
    );
    assert_eq!(
        (
            state.builder_sandbox_id,
            state.build_started_at,
            state.last_failure,
            state.retired_snapshots
        ),
        (None, None, None, vec![])
    );
    let model = f.api.model().clone();
    let [builder] = &model.sandboxes[..] else {
        panic!("one builder: {:?}", model.sandboxes);
    };
    assert_eq!(builder.name.as_deref(), Some(f.builder.as_str()));
    assert_eq!(builder.image.as_deref(), Some(TENSORLAKE_BASE_IMAGE));
    assert_eq!(
        builder.resources,
        Resources {
            cpus: 2.0,
            memory_mb: 4096,
            disk_mb: 20480
        }
    );
    assert_eq!((builder.timeout_secs, builder.auto_resume), (900, false));
    assert_eq!(builder.status, "terminated");
    assert_eq!(f.snapshot_ids(), std::slice::from_ref(&snapshot.id));
    assert_eq!(
        f.shell.labels(),
        ["ready", "packages", "claude", "codex", "prepare"]
    );
    assert!(
        f.shell
            .calls
            .lock()
            .unwrap()
            .iter()
            .all(|(sandbox, _)| *sandbox == f.builder)
    );
    assert!(
        f.api.position(&format!("get_snapshot {}", snapshot.id))
            < f.api.position(&format!("terminate_sandbox {}", builder.id))
    );
    assert_eq!(f.build_view().await, BuildView::Idle);
    assert!(
        !f.builds.view(RECIPE, &f.state().await).2,
        "nothing awaits cleanup"
    );
}

/// Every base starts the builder from exactly the selected image or snapshot
/// and runs the same package and official-agent layers. Snapshot provenance
/// must describe what was built so later recipe edits can be shown as stale.
#[farhelm_testtrace::test(start_paused = true)]
async fn all_bases_build_their_selected_source_and_record_the_layers() {
    for choice in 0..3 {
        let f = fixture().await;
        let users_snapshot = f.api.model().plant_snapshot("usersbuilder");
        let base = match choice {
            0 => RecipeBase::TensorlakeUbuntu,
            1 => RecipeBase::Image {
                image: "team/base".into(),
            },
            _ => RecipeBase::Snapshot {
                snapshot_id: users_snapshot.clone(),
            },
        };
        let mut save = tests_support::save();
        save.base = base.clone();
        save.packages = if choice == 1 {
            vec![]
        } else {
            vec!["jq".into()]
        };
        let packages = save.packages.clone();
        f.store
            .update_tensorlake_recipe(RECIPE.into(), save)
            .await
            .unwrap()
            .unwrap();
        f.build().await;
        let built = f
            .state()
            .await
            .snapshot
            .expect("every base produces a snapshot");
        assert_eq!(built.base, base);
        assert_eq!(built.packages, packages);
        assert!(built.claude_version.is_some() && built.codex_version.is_some());
        assert_eq!(
            f.shell.labels(),
            ["ready", "packages", "claude", "codex", "prepare"]
        );
        let model = f.api.model();
        let builder = &model.sandboxes[0];
        match base {
            RecipeBase::TensorlakeUbuntu => {
                assert_eq!(builder.image.as_deref(), Some(TENSORLAKE_BASE_IMAGE))
            }
            RecipeBase::Image { image } => assert_eq!(builder.image, Some(image)),
            RecipeBase::Snapshot { snapshot_id } => {
                assert_eq!(builder.snapshot_id, Some(snapshot_id))
            }
        }
        assert_eq!(builder.status, "terminated");
        assert!(
            model
                .snapshots
                .iter()
                .any(|snapshot| snapshot.id == users_snapshot),
            "the user's base is never cleanup-owned"
        );
    }
}

/// A save racing a build cannot relabel its filesystem. The completed result
/// keeps the frozen base and package layer, and an explicit rebuild catches up
/// to the edited definition and only then deletes the previous result.
#[farhelm_testtrace::test(start_paused = true)]
async fn edits_during_build_keep_frozen_provenance_until_rebuilt() {
    let f = fixture().await;
    f.shell.hold.send_replace(true);
    f.start().await;
    f.shell.started(1).await;
    let mut save = tests_support::save();
    save.base = RecipeBase::Image {
        image: "team/next".into(),
    };
    save.packages = vec!["jq".into()];
    f.store
        .update_tensorlake_recipe(RECIPE.into(), save)
        .await
        .unwrap()
        .unwrap();
    f.shell.hold.send_replace(false);
    f.finish().await;
    let previous = f.state().await.snapshot.unwrap();
    assert_eq!(previous.base, RecipeBase::TensorlakeUbuntu);
    assert_eq!(previous.packages, vec!["git", "ripgrep"]);
    let (spec, _) = f
        .store
        .tensorlake_recipe_row(RECIPE)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(spec.base, previous.base);
    assert_ne!(spec.packages, previous.packages);
    f.build().await;
    let rebuilt = f.state().await.snapshot.unwrap();
    assert_eq!(rebuilt.base, spec.base);
    assert_eq!(rebuilt.packages, spec.packages);
    assert_ne!(rebuilt.id, previous.id);
    assert!(!f.snapshot_ids().contains(&previous.id));
}

/// A failed requirement check must reach neither packages nor either agent
/// installer. The failure remains on the existing step and the builder is swept,
/// while any earlier ready snapshot stays usable.
#[farhelm_testtrace::test(start_paused = true)]
async fn base_requirement_failures_stop_before_installing_and_keep_the_old_snapshot() {
    for requirement in ["systemd", "sudo", "apt", "bash", "curl", "git"] {
        let f = fixture().await;
        let old = f.api.model().plant_snapshot("oldbuilder");
        let id = old.clone();
        f.set_state(move |state| {
            state.snapshot = Some(BuiltSnapshot {
                id,
                ..Default::default()
            })
        })
        .await;
        f.shell.answer(
            "ready",
            Err(ShellFailure::Exited {
                code: 1,
                stderr: format!("missing {requirement}"),
            }),
        );
        f.build().await;
        let state = f.state().await;
        let failure = state.last_failure.unwrap();
        assert_eq!(failure.step, BuildStep::Connecting);
        assert!(failure.message.contains(requirement));
        assert_eq!(f.shell.labels(), ["ready"]);
        assert_eq!(state.snapshot.unwrap().id, old);
        assert!(f.api.model().live_sandboxes().is_empty());
    }
}

/// Spec: a rebuild records its new snapshot and only then deletes the old
/// one; the old one is gone from Tensorlake and from the state.
///
/// Why: "Rebuild snapshot" swaps snapshots; deleting the old one first would
/// leave the recipe with none if the build then failed, and keeping it would
/// bill forever.
#[farhelm_testtrace::test(start_paused = true)]
async fn a_rebuild_deletes_the_old_snapshot_only_after_recording_the_new_one() {
    let f = fixture().await;
    f.build().await;
    let old = f.state().await.snapshot.unwrap().id;
    f.build().await;
    let state = f.state().await;
    let new = state.snapshot.unwrap().id;
    assert_ne!(old, new);
    assert!(state.retired_snapshots.is_empty());
    assert_eq!(f.snapshot_ids(), std::slice::from_ref(&new));
    let second_builder = f.builder_ids()[1].clone();
    assert!(
        f.api.position(&format!("get_snapshot {new}"))
            < f.api.position(&format!("delete_snapshot {old}"))
    );
    assert!(
        f.api
            .position(&format!("terminate_sandbox {second_builder}"))
            < f.api.position(&format!("delete_snapshot {old}")),
        "the old snapshot goes after the swap, at the end of the build"
    );
}

/// Spec: when a step fails, the builder is terminated, its snapshots are
/// deleted, the failure is recorded with its step and the script's message,
/// and the recipe's previous snapshot is untouched.
///
/// Why: a failed Rebuild must leave existing and new hosts on the old
/// snapshot, must show the user why, and must not leave the failed builder
/// or its checkpoints billing.
#[farhelm_testtrace::test(start_paused = true)]
async fn a_failed_step_cleans_up_and_keeps_the_previous_snapshot() {
    let f = fixture().await;
    f.build().await;
    let previous = f.state().await.snapshot.unwrap();
    *f.shell.plant_on.lock().unwrap() = Some("packages".into());
    f.shell.answer(
        "packages",
        Err(ShellFailure::Exited {
            code: 1,
            stderr: "E: Unable to locate package nope".into(),
        }),
    );
    f.build().await;
    let state = f.state().await;
    assert_eq!(state.snapshot, Some(previous.clone()));
    let failure = state.last_failure.clone().expect("the failure is recorded");
    assert_eq!(failure.step, BuildStep::Packages);
    assert_eq!(failure.message, "E: Unable to locate package nope");
    assert_eq!(
        (state.builder_sandbox_id, state.build_started_at),
        (None, None)
    );
    let failed_builder = f.builder_ids()[1].clone();
    assert_eq!(
        f.api.model().get(&failed_builder).unwrap().status,
        SandboxStatus::Terminated
    );
    let planted = f.shell.planted.lock().unwrap()[0].clone();
    assert!(
        !f.snapshot_ids().contains(&planted),
        "the builder's snapshot was swept"
    );
    assert_eq!(f.snapshot_ids(), [previous.id]);
    assert!(matches!(
        f.build_view().await,
        BuildView::Failed {
            step: BuildStep::Packages,
            ..
        }
    ));
}

/// Spec: a build first sweeps a builder the state records but that cannot
/// be found by name (terminated): its suspend snapshot and an unrecorded
/// checkpoint are deleted, while another recipe's recorded snapshot taken
/// from that same sandbox is kept; the record is cleared.
///
/// Why: this is the crash between terminating a builder and sweeping its
/// snapshots, the reason the builder's ID is persisted at all; and the keep
/// set is what stops a sweep from deleting a snapshot a recipe relies on.
#[farhelm_testtrace::test(start_paused = true)]
async fn recovery_sweeps_a_recorded_builder_and_keeps_recorded_snapshots() {
    let f = fixture().await;
    tests_support::create(&f.store, "py-data").await;
    let (suspend, checkpoint, recorded) = {
        let mut model = f.api.model();
        seed_sandbox(&mut model, "oldbuilder00000000001", "renamed", "terminated");
        (
            model.plant_snapshot("oldbuilder00000000001"),
            model.plant_snapshot("oldbuilder00000000001"),
            model.plant_snapshot("oldbuilder00000000001"),
        )
    };
    f.set_state(|state| state.builder_sandbox_id = Some("oldbuilder00000000001".into()))
        .await;
    let kept = recorded.clone();
    f.store
        .update_tensorlake_recipe_state("py-data", move |_, state| {
            state.snapshot = Some(BuiltSnapshot {
                id: kept,
                ..BuiltSnapshot::default()
            });
        })
        .await
        .unwrap();
    f.build().await;
    let snapshots = f.snapshot_ids();
    assert!(!snapshots.contains(&suspend) && !snapshots.contains(&checkpoint));
    assert!(
        snapshots.contains(&recorded),
        "another recipe's snapshot is kept"
    );
    assert!(
        f.api.position("terminate_sandbox oldbuilder00000000001")
            < f.api.position(&format!("create_sandbox {}", f.builder))
    );
    let state = f.state().await;
    assert_eq!(state.builder_sandbox_id, None);
    assert!(state.snapshot.is_some());
}

/// Spec: an outstanding create recovers its builder by name after a crash
/// before the ID was persisted, and sweeps it before the next create.
///
/// Why: the pending marker attributes that lost response to this recipe;
/// the name alone must never authorize deleting a colliding sandbox.
#[farhelm_testtrace::test(start_paused = true)]
async fn a_leftover_found_only_by_name_is_swept_before_the_create() {
    let f = fixture().await;
    let leftover_snapshot = {
        let mut model = f.api.model();
        seed_sandbox(&mut model, "leftover0000000000001", &f.builder, "running");
        model.plant_snapshot("leftover0000000000001")
    };
    f.set_state(|state| state.pending_create_at = Some(now_secs()))
        .await;
    assert!(f.state().await.pending_create_at.is_some());
    f.build().await;
    assert_eq!(
        f.api.model().get("leftover0000000000001").unwrap().status,
        SandboxStatus::Terminated
    );
    assert!(!f.snapshot_ids().contains(&leftover_snapshot));
    assert!(
        f.api.position("terminate_sandbox leftover0000000000001")
            < f.api.position(&format!("create_sandbox {}", f.builder))
    );
    assert!(f.state().await.snapshot.is_some());
}

/// A fresh build must refuse an occupied name without modifying its sandbox
/// or snapshots. Its own persisted start is progress, not proof of a create;
/// treating it as ownership would defeat the check on every ordinary build.
#[farhelm_testtrace::test(start_paused = true)]
async fn a_new_build_leaves_an_unrequested_same_name_sandbox_untouched() {
    let f = fixture().await;
    let before = {
        let mut model = f.api.model();
        seed_sandbox(&mut model, "foreign00000000000001", &f.builder, "running");
        model.plant_snapshot("foreign00000000000001");
        model.clone()
    };
    let state = f.state().await;
    assert!(state.build_started_at.is_none());
    assert!(state.builder_sandbox_id.is_none() && state.pending_create_at.is_none());
    assert_eq!(
        f.api.model().get(&f.builder).unwrap().status,
        SandboxStatus::Running
    );

    f.build().await;

    let state = f.state().await;
    let failure = state
        .last_failure
        .as_ref()
        .expect("the occupied name refuses the build");
    assert_eq!(failure.step, BuildStep::Cleaning);
    assert!(failure.message.contains(&f.builder), "{failure:?}");
    assert!(
        failure.message.contains("remove it in Tensorlake"),
        "{failure:?}"
    );
    assert!(state.builder_sandbox_id.is_none() && state.pending_create_at.is_none());
    assert!(state.snapshot.is_none());
    assert_eq!(*f.api.model(), before);
    let calls = f.api.ops();
    assert!(!calls.is_empty(), "the name was checked before refusal");
    assert!(
        calls
            .iter()
            .all(|op| op == &format!("get_sandbox {}", f.builder)),
        "{calls:?}"
    );
    assert!(f.shell.labels().is_empty());
}

/// An interrupted start without an ID or pending create permits deleting
/// the recipe row, but no vendor call. Older state or power loss can leave
/// this shape; guessing ownership from its name would destroy another helm's
/// builder and its snapshots.
#[farhelm_testtrace::test(start_paused = true)]
async fn deleting_an_interrupted_start_leaves_an_unrequested_builder_untouched() {
    let f = fixture().await;
    let before = {
        let mut model = f.api.model();
        seed_sandbox(&mut model, "foreign00000000000001", &f.builder, "running");
        model.plant_snapshot("foreign00000000000001");
        model.clone()
    };
    f.set_state(|state| state.build_started_at = Some(now_secs()))
        .await;
    let state = f.state().await;
    assert!(state.needs_cleanup() && state.build_started_at.is_some());
    assert!(state.builder_sandbox_id.is_none() && state.pending_create_at.is_none());
    assert_eq!(
        f.api.model().get(&f.builder).unwrap().status,
        SandboxStatus::Running
    );

    let guard = f
        .builds
        .clean_for_delete(RECIPE)
        .await
        .expect("only progress needs clearing");

    assert!(!f.state().await.needs_cleanup());
    assert_eq!(
        f.store
            .delete_tensorlake_recipe(RECIPE.into())
            .await
            .unwrap(),
        Ok(())
    );
    assert!(f.api.ops().is_empty(), "{:?}", f.api.ops());
    assert_eq!(*f.api.model(), before);
    drop(guard);
}

/// A saved ID authorizes sweeping that builder's leftovers, not adopting a
/// different sandbox now at its old name. Otherwise the post-sweep lookup
/// would silently turn legitimate cleanup into another helm's data loss.
#[farhelm_testtrace::test(start_paused = true)]
async fn sweeping_a_saved_builder_does_not_adopt_a_different_name_occupant() {
    let f = fixture().await;
    let expected = {
        let mut model = f.api.model();
        seed_sandbox(
            &mut model,
            "oldbuilder00000000001",
            "old-builder",
            "terminated",
        );
        let old_snapshot = model.plant_snapshot("oldbuilder00000000001");
        seed_sandbox(&mut model, "foreign00000000000001", &f.builder, "running");
        model.plant_snapshot("foreign00000000000001");
        let mut expected = model.clone();
        expected
            .snapshots
            .retain(|snapshot| snapshot.id != old_snapshot);
        expected
    };
    f.set_state(|state| state.builder_sandbox_id = Some("oldbuilder00000000001".into()))
        .await;
    let state = f.state().await;
    assert_eq!(
        state.builder_sandbox_id.as_deref(),
        Some("oldbuilder00000000001")
    );
    assert!(state.pending_create_at.is_none());
    assert_eq!(
        f.api.model().get(&f.builder).unwrap().id,
        "foreign00000000000001"
    );

    f.build().await;

    let state = f.state().await;
    assert_eq!(
        state.last_failure.as_ref().unwrap().step,
        BuildStep::Cleaning
    );
    assert!(state.builder_sandbox_id.is_none() && state.pending_create_at.is_none());
    assert!(state.snapshot.is_none());
    assert_eq!(*f.api.model(), expected);
    let calls = f.api.ops();
    assert!(calls.contains(&"terminate_sandbox oldbuilder00000000001".into()));
    assert!(!calls.contains(&"terminate_sandbox foreign00000000000001".into()));
    assert!(
        !calls.iter().any(|op| op.starts_with("create_sandbox ")),
        "{calls:?}"
    );
    assert!(f.shell.labels().is_empty());
}

/// Spec: a retired snapshot whose deletion fails stays listed and the view
/// says cleanup is pending, without failing the build; a later build retries
/// it; one Tensorlake no longer has counts as deleted.
///
/// Why: no path may forget a billed snapshot, and a transient Tensorlake
/// failure must not block building.
#[farhelm_testtrace::test(start_paused = true)]
async fn a_retired_snapshot_that_cannot_be_deleted_stays_listed_and_is_retried() {
    let f = fixture().await;
    let stuck = f.api.model().plant_snapshot("whatever000000000001");
    let retired = vec!["alreadygone000000001".to_string(), stuck.clone()];
    f.set_state(move |state| state.retired_snapshots = retired)
        .await;
    f.api.fail.lock().unwrap().insert(
        format!("delete_snapshot {stuck}"),
        TensorlakeError::Status {
            status: 500,
            message: "try later".into(),
        },
    );
    f.build().await;
    let state = f.state().await;
    assert!(state.snapshot.is_some(), "the build itself succeeded");
    assert_eq!(state.retired_snapshots, std::slice::from_ref(&stuck));
    assert!(f.builds.view(RECIPE, &state).2, "cleanup is pending");
    f.api.fail.lock().unwrap().clear();
    f.build().await;
    assert!(f.state().await.retired_snapshots.is_empty());
    assert!(!f.snapshot_ids().contains(&stuck));
}

/// Spec: starting a build while one runs joins it: one builder is created,
/// and the view shows the running step until it ends.
///
/// Why: Build, Rebuild and Retry are the same route and the user (or two
/// clients) may click twice; two builders would bill twice and race on the
/// recipe's state.
#[farhelm_testtrace::test(start_paused = true)]
async fn a_second_start_joins_the_running_build() {
    let f = fixture().await;
    f.shell.hold.send_replace(true);
    f.start().await;
    f.shell.started(1).await;
    f.start().await;
    assert!(matches!(
        f.build_view().await,
        BuildView::Building {
            step: BuildStep::Connecting,
            ..
        }
    ));
    f.shell.hold.send_replace(false);
    f.finish().await;
    let creates = f
        .api
        .ops()
        .iter()
        .filter(|op| op.starts_with("create_sandbox"))
        .count();
    assert_eq!(creates, 1);
    assert!(f.state().await.snapshot.is_some());
}

/// Spec: cleaning up for a delete during a build cancels it at its current
/// wait, terminates the builder it had recorded, sweeps its snapshots,
/// deletes the recipe's snapshot, clears every record, and keeps new builds
/// out until the guard is dropped; the store then deletes the row.
///
/// Why: deleting a recipe must not leave a builder billing, and a build
/// started between the cleanup and the row's deletion would create
/// resources nothing records.
#[farhelm_testtrace::test(start_paused = true)]
async fn deleting_during_a_build_cancels_it_and_cleans_up() {
    let f = fixture().await;
    f.build().await;
    let previous = f.state().await.snapshot.unwrap().id;
    f.shell.hold.send_replace(true);
    f.start().await;
    f.shell.started(6).await;
    let builder = f
        .state()
        .await
        .builder_sandbox_id
        .expect("the builder is recorded");
    let guard = f
        .builds
        .clean_for_delete(RECIPE)
        .await
        .expect("the cleanup succeeds");
    assert!(matches!(
        f.builds.start_or_join(RECIPE, Arc::clone(&f.events)).await,
        Err(StartRefusal::Deleting)
    ));
    assert!(!f.builds.is_building(RECIPE));
    assert_eq!(
        f.api.model().get(&builder).unwrap().status,
        SandboxStatus::Terminated
    );
    assert!(f.snapshot_ids().is_empty(), "{:?}", f.snapshot_ids());
    assert!(!f.snapshot_ids().contains(&previous));
    assert!(!f.state().await.needs_cleanup(), "{:?}", f.state().await);
    assert_eq!(
        f.shell.labels().len(),
        6,
        "the held command was the last one"
    );
    assert_eq!(
        f.store
            .delete_tensorlake_recipe(RECIPE.into())
            .await
            .unwrap(),
        Ok(())
    );
    drop(guard);
}

/// Spec: the store refuses to delete a recipe whose state records a
/// snapshot, a builder, a retired snapshot or an unfinished build.
///
/// Why: the row is the only record of those IDs; deleting it first would
/// leave them billing with nothing that knows them.
#[farhelm_testtrace::test(start_paused = true)]
async fn the_store_refuses_to_forget_a_recipe_with_billed_resources() {
    let f = fixture().await;
    let states: [fn(&mut RecipeState); 4] = [
        |s| s.snapshot = Some(BuiltSnapshot::default()),
        |s| s.builder_sandbox_id = Some("b".into()),
        |s| s.retired_snapshots = vec!["r".into()],
        |s| s.build_started_at = Some(1),
    ];
    for set in states {
        f.set_state(move |state| {
            *state = RecipeState::default();
            set(state);
        })
        .await;
        assert_eq!(
            f.store
                .delete_tensorlake_recipe(RECIPE.into())
                .await
                .unwrap(),
            Err(super::super::recipes::Refusal::Billed)
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
}

/// Spec: changing base preserves the current snapshot until a successful
/// replacement build, just as changing packages does.
///
/// Why: an edit must not remove a usable snapshot or hide billed ownership.
#[farhelm_testtrace::test(start_paused = true)]
async fn switching_to_an_image_keeps_the_snapshot_until_rebuilt() {
    let f = fixture().await;
    f.set_state(|state| {
        state.snapshot = Some(BuiltSnapshot {
            id: "snap1".into(),
            ..BuiltSnapshot::default()
        })
    })
    .await;
    let mut save = tests_support::save();
    save.size.cpus = 4;
    f.store
        .update_tensorlake_recipe(RECIPE.into(), save.clone())
        .await
        .unwrap()
        .ok()
        .unwrap();
    assert!(f.state().await.snapshot.is_some());
    save.base = RecipeBase::Image {
        image: "team-base".into(),
    };
    f.store
        .update_tensorlake_recipe(RECIPE.into(), save)
        .await
        .unwrap()
        .ok()
        .unwrap();
    let state = f.state().await;
    assert_eq!(state.snapshot.unwrap().id, "snap1");
    assert!(state.retired_snapshots.is_empty());
}

/// Spec: a recorded `build_started_at` with no live build reads as
/// interrupted; with a recorded failure and no start it reads as failed.
///
/// Why: nothing reconciles at helm start, so the view is the only place a
/// build cut short by a restart becomes visible, with Retry.
#[farhelm_testtrace::test(start_paused = true)]
async fn a_build_cut_short_by_a_restart_reads_as_interrupted() {
    let f = fixture().await;
    f.set_state(|state| {
        state.build_started_at = Some(5);
        state.builder_sandbox_id = Some("b".into());
    })
    .await;
    assert_eq!(
        f.build_view().await,
        BuildView::Interrupted { started_at: 5 }
    );
    assert!(
        f.builds.view(RECIPE, &f.state().await).2,
        "the recorded builder awaits cleanup"
    );
}

/// Spec: a refused key fails the build at the connection step at once, with
/// the registration hint, after one attempt.
///
/// Why: an unregistered key is the most likely first-time failure, and
/// retrying cannot fix it.
#[farhelm_testtrace::test(start_paused = true)]
async fn a_refused_key_fails_at_once_with_the_registration_hint() {
    let f = fixture().await;
    f.shell.answer("ready", Err(ShellFailure::AuthRefused));
    f.build().await;
    let failure = f.state().await.last_failure.unwrap();
    assert_eq!(
        (failure.step, failure.message.as_str()),
        (BuildStep::Connecting, AUTH_REFUSED_MESSAGE)
    );
    assert_eq!(f.shell.labels(), ["ready"]);
    assert_eq!(f.api.model().sandboxes[0].status, "terminated");
}

/// Spec: an unreachable sandbox is retried within the connection window and
/// the build then goes on.
///
/// Why: the sandbox's sshd can come up a moment after Tensorlake reports it
/// running; failing on the first refusal would fail healthy builds.
#[farhelm_testtrace::test(start_paused = true)]
async fn the_first_connection_is_retried_while_sshd_comes_up() {
    let f = fixture().await;
    for _ in 0..2 {
        f.shell.answer(
            "ready",
            Err(ShellFailure::Unreachable("Connection refused".into())),
        );
    }
    f.build().await;
    assert_eq!(
        f.shell.labels()[..4],
        ["ready", "ready", "ready", "packages"]
    );
    assert!(f.state().await.snapshot.is_some());
}

/// Spec: a snapshot that Tensorlake reports failed fails the build at the
/// snapshot step and is swept with the builder.
///
/// Why: only a completed snapshot may be recorded; a failed one is not a
/// usable host image and still bills until deleted.
#[farhelm_testtrace::test(start_paused = true)]
async fn a_failed_snapshot_fails_the_build() {
    let f = fixture().await;
    f.api.model().snapshots_fail = true;
    f.build().await;
    let state = f.state().await;
    assert_eq!(state.last_failure.unwrap().step, BuildStep::Snapshotting);
    assert_eq!(state.snapshot, None);
    assert!(f.snapshot_ids().is_empty());
}

// ---------------------------------------------------------------------------
// The route
// ---------------------------------------------------------------------------

/// Spec: the build route requires an API key for every base, and a helm
/// without an API key, and one without Farhelm's ssh key, each with its
/// reason; with both keys it answers 202 with the recipe building, and a
/// second request joins that build (one builder created).
///
/// Why: these are the Build button's outcomes; a second click must never
/// create a second billed builder.
#[farhelm_testtrace::test]
async fn the_build_route_refuses_what_it_cannot_build_and_joins_a_running_build() {
    use crate::rest_harness::FleetBuilder;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt as _;

    let api = Arc::new(MemoryApi::default());
    let shell = ScriptedShell::new(Arc::clone(&api));
    shell.hold.send_replace(true);
    let harness = FleetBuilder::new()
        .await
        .tensorlake_api(api.clone())
        .tensorlake_shell(shell.clone())
        .start()
        .await;
    let call = |method: &'static str, path: String, body: Option<String>| {
        let router = harness.router();
        async move {
            let request = Request::builder()
                .method(method)
                .uri(path)
                .header("host", "127.0.0.1:7433")
                .header("content-type", "application/json")
                .body(body.map_or_else(Body::empty, Body::from))
                .unwrap();
            let response = router.oneshot(request).await.unwrap();
            let status = response.status();
            let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
                .await
                .unwrap();
            (status, String::from_utf8(bytes.to_vec()).unwrap())
        }
    };
    let mut reuse = tests_support::save();
    reuse.base = RecipeBase::Image {
        image: "team-base".into(),
    };
    for (name, save) in [("reuse", reuse), (RECIPE, tests_support::save())] {
        let body = serde_json::json!({ "name": name, "recipe": save });
        let (status, body) = call(
            "POST",
            "/api/tensorlake/recipes".into(),
            Some(body.to_string()),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    let build = |name: &str| format!("/api/tensorlake/recipes/{name}/build");
    assert_eq!(
        call("POST", build("reuse"), None).await,
        (
            StatusCode::BAD_REQUEST,
            "save a Tensorlake API key in Settings first".into()
        )
    );
    assert_eq!(
        call("POST", build(RECIPE), None).await,
        (
            StatusCode::BAD_REQUEST,
            "save a Tensorlake API key in Settings first".into()
        )
    );
    let (status, body) = call(
        "PUT",
        "/api/tensorlake".into(),
        Some(r#"{"api_key":"test-key"}"#.into()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = call("POST", build(RECIPE), None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("create Farhelm's SSH key"), "{body}");
    let (status, body) = call("POST", "/api/tensorlake/ssh-key".into(), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        call("POST", build("missing"), None).await.0,
        StatusCode::NOT_FOUND
    );

    let (status, body) = call("POST", build(RECIPE), None).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    let view: farhelm_proto::tensorlake::recipes::RecipeView = serde_json::from_str(&body).unwrap();
    assert!(matches!(view.build, BuildView::Building { .. }), "{body}");
    shell.started(1).await;
    let (status, _) = call("POST", build(RECIPE), None).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    shell.hold.send_replace(false);
    let builds = Arc::clone(harness.state.tensorlake.builds());
    let task = builds
        .registry()
        .active
        .get(RECIPE)
        .and_then(|active| active.task.lock().unwrap().take());
    if let Some(task) = task {
        task.await.unwrap();
    }
    assert_eq!(
        api.ops()
            .iter()
            .filter(|op| op.starts_with("create_sandbox"))
            .count(),
        1
    );
    let (_, listed) = call("GET", "/api/tensorlake/recipes".into(), None).await;
    let listed: farhelm_proto::tensorlake::recipes::RecipesView =
        serde_json::from_str(&listed).unwrap();
    let built = listed.recipes.iter().find(|r| r.name == RECIPE).unwrap();
    assert!(
        built.snapshot.is_some() && built.build == BuildView::Idle,
        "{built:?}"
    );

    // Deleting the built recipe deletes its snapshot first.
    let (status, body) = call("DELETE", format!("/api/tensorlake/recipes/{RECIPE}"), None).await;
    assert_eq!((status, body.as_str()), (StatusCode::OK, "{}"));
    assert!(api.model().snapshots.is_empty());
}

// ---------------------------------------------------------------------------
// Ownership across lost answers, cancellation and restarts
// ---------------------------------------------------------------------------

/// Spec: a delete that arrives while a first build's create has reached
/// Tensorlake but not answered waits for the create to be answered and
/// recorded, then terminates that builder; the row can then be deleted.
///
/// Why: a create dropped mid-flight can still make a sandbox, and on a
/// first build nothing else records it; deleting the recipe then would
/// leave a builder billing with nothing that knows it.
#[farhelm_testtrace::test(start_paused = true)]
async fn a_delete_during_a_create_waits_for_its_answer_and_cleans_up() {
    let f = fixture().await;
    f.api.hold_create.send_replace(true);
    f.start().await;
    f.api
        .create_committed
        .subscribe()
        .wait_for(|committed| *committed)
        .await
        .unwrap();
    let builds = Arc::clone(&f.builds);
    let delete = tokio::spawn(async move { builds.clean_for_delete(RECIPE).await.map(drop) });
    // The delete publishes its cancellation before it waits for the build;
    // the create stays held throughout, which is the boundary under test.
    let mut cancel = f
        .builds
        .registry()
        .active
        .get(RECIPE)
        .expect("premise: the build is running")
        .cancel
        .subscribe();
    let cancelled = tokio::time::timeout(
        Duration::from_secs(30),
        cancel.wait_for(|cancelled| *cancelled),
    )
    .await;
    assert!(
        matches!(cancelled, Ok(Ok(_))),
        "the delete never cancelled the build (delete finished: {}, create held: {}, build running: {})",
        delete.is_finished(),
        *f.api.hold_create.borrow(),
        f.builds.is_building(RECIPE),
    );
    assert!(!delete.is_finished(), "the delete waits for the create");
    f.api.hold_create.send_replace(false);
    delete.await.unwrap().expect("the cleanup succeeds");
    let [builder] = &f.builder_ids()[..] else {
        panic!("one builder: {:?}", f.api.model().sandboxes);
    };
    assert_eq!(
        f.api.model().get(builder).unwrap().status,
        SandboxStatus::Terminated
    );
    assert!(!f.state().await.needs_cleanup(), "{:?}", f.state().await);
    assert!(
        f.shell.labels().is_empty(),
        "the build stopped before connecting"
    );
    assert_eq!(
        f.store
            .delete_tensorlake_recipe(RECIPE.into())
            .await
            .unwrap(),
        Ok(())
    );
}

/// Spec: when a create succeeds but its reply cannot be read, the builder
/// is found by its name, recorded, and the build goes on and ends with the
/// builder terminated.
///
/// Why: a success reply that does not decode still means the sandbox
/// exists; giving up on it would leave a builder nothing records.
#[farhelm_testtrace::test(start_paused = true)]
async fn an_unreadable_create_reply_adopts_the_builder_by_name() {
    let f = fixture().await;
    *f.api.create_reply.lock().unwrap() = Some(TensorlakeError::Decode);
    f.build().await;
    let state = f.state().await;
    assert!(state.snapshot.is_some(), "{state:?}");
    assert_eq!(
        (state.builder_sandbox_id, state.pending_create_at),
        (None, None)
    );
    let builder = f.builder_ids()[0].clone();
    assert_eq!(
        f.api.model().get(&builder).unwrap().status,
        SandboxStatus::Terminated
    );
    let create = f.api.position(&format!("create_sandbox {}", f.builder));
    assert!(
        f.api.ops()[create + 1..].contains(&format!("get_sandbox {}", f.builder)),
        "the builder was looked up by name after the create"
    );
}

/// Spec: with the create's reply lost and its fallback lookup failing, the
/// build fails with the pending record kept, and a delete then adopts the
/// builder found by name.
///
/// Why: this is the uncertain outcome itself; the lookup that resolves it
/// must happen at delete time, not only during the build.
#[farhelm_testtrace::test(start_paused = true)]
async fn an_unresolved_create_is_resolved_by_name_at_delete() {
    let f = fixture().await;
    *f.api.create_reply.lock().unwrap() = Some(TensorlakeError::Transport(
        "Tensorlake did not answer in time".into(),
    ));
    let lookup = format!("get_sandbox {}", f.builder);
    let api = Arc::clone(&f.api);
    // Fail the create's fallback lookup, which follows the create call.
    f.api.hold_create.send_replace(true);
    f.start().await;
    api.create_committed
        .subscribe()
        .wait_for(|committed| *committed)
        .await
        .unwrap();
    api.fail_once(
        &lookup,
        TensorlakeError::Status {
            status: 503,
            message: "down".into(),
        },
    );
    api.hold_create.send_replace(false);
    f.finish().await;
    let state = f.state().await;
    assert_eq!(state.builder_sandbox_id, None);
    assert!(state.pending_create_at.is_some(), "{state:?}");
    assert_eq!(state.last_failure.unwrap().step, BuildStep::Creating);
    let builder = f.builder_ids()[0].clone();
    assert_eq!(
        f.api.model().get(&builder).unwrap().status,
        SandboxStatus::Running
    );
    assert_eq!(
        f.store
            .delete_tensorlake_recipe(RECIPE.into())
            .await
            .unwrap(),
        Err(super::super::recipes::Refusal::Billed),
        "the store will not forget a pending create"
    );
    let _guard = f
        .builds
        .clean_for_delete(RECIPE)
        .await
        .expect("the cleanup succeeds");
    assert_eq!(
        f.api.model().get(&builder).unwrap().status,
        SandboxStatus::Terminated
    );
    assert!(!f.state().await.needs_cleanup(), "{:?}", f.state().await);
}

/// Spec: a pending create with nothing under the builder's name holds the
/// delete back (with a "try again" message) until the settle window since
/// the create has passed, after which the same lookup clears it.
///
/// Why: right after a lost create, an empty lookup does not prove nothing
/// will appear; treating it as proof would forget a sandbox that starts a
/// moment later.
#[farhelm_testtrace::test(start_paused = true)]
async fn a_pending_create_with_nothing_under_its_name_settles_only_after_the_window() {
    let f = fixture().await;
    let now = super::super::recipes::now_secs();
    f.set_state(move |state| state.pending_create_at = Some(now))
        .await;
    match f.builds.clean_for_delete(RECIPE).await {
        Err(CleanupRefusal::Tensorlake(message)) => {
            assert!(message.contains("may still be starting"), "{message}")
        }
        other => panic!("refused: {:?}", other.map(drop)),
    }
    assert!(f.state().await.pending_create_at.is_some());
    f.set_state(move |state| state.pending_create_at = Some(now - CREATE_SETTLE_SECS - 1))
        .await;
    let _guard = f.builds.clean_for_delete(RECIPE).await.expect("settled");
    assert!(!f.state().await.needs_cleanup());
}

/// Spec: when recording a new builder's ID fails, the build fails at the
/// create step and leaves that builder running, unrecorded except by the
/// pending-create marker; the next build finds it by name, records it,
/// terminates it and builds normally.
///
/// Why: only a recorded ID may enter a sweep. Terminating the builder from
/// an ID held in memory alone would, if the sweep then failed or the helm
/// stopped, leave its snapshots attributable to nothing, since a terminated
/// sandbox no longer answers to its name.
#[farhelm_testtrace::test(start_paused = true)]
async fn a_failed_builder_record_leaves_the_builder_for_reconciliation() {
    let f = fixture().await;
    f.builds
        .fail_builder_record
        .store(true, std::sync::atomic::Ordering::SeqCst);
    f.build().await;
    let state = f.state().await;
    assert_eq!(
        state.last_failure.as_ref().unwrap().step,
        BuildStep::Creating
    );
    assert_eq!(state.builder_sandbox_id, None);
    assert!(state.pending_create_at.is_some(), "{state:?}");
    assert!(f.builds.view(RECIPE, &state).2, "cleanup is pending");
    let unrecorded = f.builder_ids()[0].clone();
    assert_eq!(
        f.api.model().get(&unrecorded).unwrap().status,
        SandboxStatus::Running,
        "nothing terminated the unrecorded builder"
    );

    f.build().await;
    assert_eq!(
        f.api.model().get(&unrecorded).unwrap().status,
        SandboxStatus::Terminated
    );
    let last_create = f
        .api
        .ops()
        .iter()
        .rposition(|op| op.starts_with("create_sandbox"))
        .unwrap();
    assert!(
        f.api.position(&format!("terminate_sandbox {unrecorded}")) < last_create,
        "the retry swept the leftover before creating its own builder"
    );
    let state = f.state().await;
    assert!(state.snapshot.is_some(), "{state:?}");
    assert_eq!(
        (state.builder_sandbox_id, state.pending_create_at),
        (None, None)
    );
}

/// Spec: a pending create's builder found by its name is recorded before it is
/// terminated; when the snapshot listing then fails, the record stays (the
/// terminated builder no longer answers to its name), and a fresh engine
/// over the same store, as after a helm restart, deletes its snapshot.
///
/// Why: terminating first and recording never would leave the builder's
/// snapshots unattributable once its name stops resolving, billing for
/// good.
#[farhelm_testtrace::test(start_paused = true)]
async fn a_builder_found_by_name_is_recorded_before_it_is_terminated() {
    let f = fixture().await;
    let snapshot = {
        let mut model = f.api.model();
        seed_sandbox(&mut model, "leftover0000000000001", &f.builder, "running");
        model.plant_snapshot("leftover0000000000001")
    };
    f.set_state(|state| state.pending_create_at = Some(now_secs()))
        .await;
    assert!(f.state().await.pending_create_at.is_some());
    f.api.fail_once(
        "list_snapshots ",
        TensorlakeError::Status {
            status: 500,
            message: "try later".into(),
        },
    );
    f.build().await;
    let state = f.state().await;
    assert_eq!(
        state.last_failure.as_ref().unwrap().step,
        BuildStep::Cleaning
    );
    assert_eq!(
        state.builder_sandbox_id.as_deref(),
        Some("leftover0000000000001")
    );
    assert!(matches!(
        f.api.model().get(&f.builder),
        Err(TensorlakeError::NotFound { .. })
    ));
    assert!(
        f.snapshot_ids().contains(&snapshot),
        "premise: the sweep stopped"
    );

    let restarted = Arc::new(RecipeBuilds::new(
        f.store.clone(),
        f.api.clone(),
        f.shell.clone(),
        Some(Arc::new(SshKeyStore::new(f._dir.path().join("tensorlake")))),
        BuildTimings::default(),
        Arc::new(super::super::prepared::UnavailablePreparer),
        farhelm_proto::BUILD_VERSION.to_string(),
    ));
    restarted
        .start_or_join(RECIPE, Arc::clone(&f.events))
        .await
        .unwrap();
    let task = restarted
        .registry()
        .active
        .get(RECIPE)
        .and_then(|active| active.task.lock().unwrap().take());
    task.unwrap().await.unwrap();
    assert!(!f.snapshot_ids().contains(&snapshot));
    let state = f.state().await;
    assert!(
        state.snapshot.is_some() && state.last_failure.is_none(),
        "{state:?}"
    );
}

/// Spec: a snapshot base Farhelm built (any recipe's
/// current or retired one) is refused on create and on update with a
/// message naming the owning recipe; a snapshot of the user's own is
/// accepted and survives the save and the retired-snapshot cleanup.
///
/// Why: Farhelm deletes the snapshots it built when their recipe moves on;
/// a recipe built on one, itself included, would be left with a base that
/// no longer exists.
#[farhelm_testtrace::test(start_paused = true)]
async fn reusing_a_snapshot_farhelm_built_is_refused() {
    let f = fixture().await;
    let built = f.api.model().plant_snapshot("builder00000000000001");
    let retired = f.api.model().plant_snapshot("builder00000000000002");
    let users = f.api.model().plant_snapshot("someoneelse0000000001");
    let (b, r) = (built.clone(), retired.clone());
    f.set_state(move |state| {
        state.snapshot = Some(BuiltSnapshot {
            id: b,
            ..BuiltSnapshot::default()
        });
        state.retired_snapshots = vec![r];
    })
    .await;
    let reuse = |snapshot_id: &str| {
        let mut save = tests_support::save();
        save.base = RecipeBase::Snapshot {
            snapshot_id: snapshot_id.into(),
        };
        save
    };
    for id in [&built, &retired] {
        match f
            .store
            .update_tensorlake_recipe(RECIPE.into(), reuse(id))
            .await
            .unwrap()
        {
            Err(super::super::recipes::Refusal::Invalid(message)) => {
                assert!(
                    message.contains(&format!("Farhelm built for recipe {RECIPE}")),
                    "{message}"
                )
            }
            _ => panic!("reusing {id} must be refused"),
        }
        assert!(
            f.store
                .create_tensorlake_recipe("other".into(), reuse(id))
                .await
                .unwrap()
                .is_err()
        );
    }
    assert_eq!(
        f.state().await.snapshot.unwrap().id,
        built,
        "the refused save changed nothing"
    );
    f.store
        .update_tensorlake_recipe(RECIPE.into(), reuse(&users))
        .await
        .unwrap()
        .unwrap_or_else(|_| panic!("a snapshot of the user's own is accepted"));
    let key = ApiKey::new("test-key".into());
    f.builds.delete_retired(RECIPE, &key).await.unwrap();
    assert!(
        f.snapshot_ids().contains(&users),
        "the reuse source survives the cleanup"
    );
    assert!(f.snapshot_ids().contains(&built));
    assert!(!f.snapshot_ids().contains(&retired));
}

/// Spec: changing base keeps the old snapshot current, without attempting to
/// delete it or reporting it as retired cleanup.
///
/// Why: a save is not a build and cannot retire the replacement's fallback.
#[farhelm_testtrace::test(start_paused = true)]
async fn a_switch_to_an_image_does_not_attempt_snapshot_deletion() {
    let f = fixture().await;
    let snapshot = f.api.model().plant_snapshot("builder00000000000001");
    let id = snapshot.clone();
    f.set_state(move |state| {
        state.snapshot = Some(BuiltSnapshot {
            id,
            ..BuiltSnapshot::default()
        })
    })
    .await;
    f.api.fail.lock().unwrap().insert(
        format!("delete_snapshot {snapshot}"),
        TensorlakeError::Status {
            status: 500,
            message: "try later".into(),
        },
    );
    let mut save = tests_support::save();
    save.base = RecipeBase::Image {
        image: "team-base".into(),
    };
    f.store
        .update_tensorlake_recipe(RECIPE.into(), save)
        .await
        .unwrap()
        .ok()
        .unwrap();
    let key = ApiKey::new("test-key".into());
    f.builds.delete_retired(RECIPE, &key).await.unwrap();
    let state = f.state().await;
    assert!(state.retired_snapshots.is_empty());
    assert_eq!(state.snapshot.unwrap().id, snapshot);
    assert!(
        !f.api
            .ops()
            .iter()
            .any(|op| op.starts_with("delete_snapshot"))
    );
}
