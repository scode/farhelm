//! Snapshot builds: turning a managed recipe into a Tensorlake snapshot that
//! hosts start from.
//!
//! A build creates a builder sandbox from [`TENSORLAKE_BASE_IMAGE`] with the
//! recipe's size, checks it boots systemd, installs the recipe's apt
//! packages and the two agents (the scripts are in `build_script.rs`),
//! empties `/etc/machine-id`, takes a filesystem snapshot and waits for it
//! to complete, records the snapshot ID, terminates the builder and deletes
//! the snapshots whose source is the builder (its suspend snapshots). A
//! rebuild does the same and then deletes the recipe's previous snapshot;
//! sandboxes already started from it keep working (checked live).
//!
//! # Billing is the invariant
//!
//! Every builder and snapshot bills until deleted, so the engine's rule is
//! that a durable record of each exists from the moment Tensorlake has it
//! until it is gone, in the recipe's `state_json` ([`RecipeState`]):
//!
//! - the builder's sandbox ID is persisted right after create returns. Its
//!   name alone is not enough: once terminated, a sandbox can no longer be
//!   found by name reliably, and its snapshots are attributable only by its
//!   ID;
//! - a crash before that point leaves a sandbox with the fixed builder name,
//!   which the next build's cleaning step looks up by name;
//! - the new snapshot's ID is recorded in the same write that moves the old
//!   one to `retired_snapshots`, so the old one is deleted only after the new
//!   one is recorded, and a crash between the two leaves both recorded;
//! - a builder ID or retired snapshot is removed from the state only after
//!   Tensorlake confirmed the deletion (or answered "not found").
//!
//! Nothing runs at helm start (no reconcile pass). A build the helm's exit
//! interrupted shows as interrupted (a persisted `build_started_at` with no
//! live build), and the next build or the recipe's deletion finishes the
//! cleanup from what the state records.
//!
//! # One build per recipe
//!
//! [`RecipeBuilds`] is the in-memory registry: at most one build per recipe
//! (a second Build click joins the running one), different recipes build
//! concurrently with one builder each. A build is a task the registry owns,
//! not the HTTP request that started it. Progress is a `watch` channel the
//! recipes GET reads, plus a feed bump per step so clients re-read.
//!
//! Deleting a recipe cancels its build and then cleans up. Cancellation is
//! cooperative: only the waits on Tensorlake and on the sandbox (API calls,
//! script runs, poll sleeps) are cancelled, never a state write, so a
//! cancelled build has recorded everything it created before the delete
//! reads the state.

use std::{
    collections::{HashMap, HashSet},
    future::Future,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::Context as _;
use farhelm_proto::tensorlake::recipes::{
    BuildStep, BuildView, RecipeSize, RecipeSource, SnapshotView,
};
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use super::api::{
    ApiKey, CreateSandbox, Resources, SandboxSource, SandboxStatus, SnapshotStatus, SnapshotType,
    TensorlakeApi, TensorlakeError,
};
use super::build_script;
use super::config::helm_tag;
use super::recipes::{StoredRecipe, decode_spec, now_secs};
use super::shell::{SandboxShell, ShellFailure, ShellOutput};
use super::ssh::TensorlakeSshAccess;
use super::ssh_key::SshKeyStore;
use crate::{feed::FleetEvents, store::HelmStore};

/// The image every managed recipe builds on. The one place the base image
/// is named: goal D1's fallback, should Tensorlake's ssh to CAS-format
/// sandboxes regress, changes this to `tensorlake/legacy/ubuntu-systemd`.
pub(crate) const TENSORLAKE_BASE_IMAGE: &str = "tensorlake/ubuntu-systemd";

/// The name prefix of every builder sandbox. Specific on purpose: the
/// repository's browser-test harness names its sandboxes `farhelm-sbx-`, and
/// the orphan list must never take one of those for a builder.
pub(crate) const BUILDER_PREFIX: &str = "farhelm-build-";

/// The builder's idle timeout, in seconds. A builder only goes idle if the
/// helm died mid-build (an open ssh channel counts as activity); then this
/// bounds how long it bills as a running sandbox before Tensorlake suspends
/// it, leaving a suspend snapshot the next build sweeps.
pub(crate) const BUILDER_IDLE_TIMEOUT_SECS: u64 = 900;

/// How long after a create was sent a lookup of its pre-chosen name that
/// finds nothing proves the create made nothing.
///
/// Right after a create whose answer was lost, an empty lookup proves
/// little: the request may still be on its way or waiting at Tensorlake.
/// The client gives up on a create after ten minutes, and Tensorlake makes a
/// sandbox durable (and visible by name) before it waits for capacity, so a
/// sandbox that does not answer to its name five minutes after that never
/// will. Until then the pending record stays, and a delete is refused.
pub(crate) const CREATE_SETTLE_SECS: i64 = 15 * 60;

/// The builder sandbox's name for a recipe: `farhelm-build-<helm tag>-<recipe>`.
///
/// Fixed per recipe and helm, so a build can find a builder an earlier crash
/// left before its ID was recorded, and so the orphan list can attribute it.
/// The recipe name's grammar (`check_recipe_name`) is a subset of a
/// Tensorlake name's, so no sanitizing happens and two recipes can never
/// share a builder.
pub(crate) fn builder_name(tag: &str, recipe: &str) -> String {
    let name = format!("{BUILDER_PREFIX}{tag}-{recipe}");
    debug_assert!(name.len() <= 63, "{name}");
    name
}

/// How long each wait of a build may take. Fields rather than constants so
/// tests can use the same engine with its real values under paused time.
#[derive(Clone, Debug)]
pub(crate) struct BuildTimings {
    /// Between two status reads of a sandbox or snapshot.
    pub(crate) poll: Duration,
    /// For the builder to run after create. Tensorlake may wait for capacity.
    pub(crate) start: Duration,
    /// For a leftover builder's name to be free again after it terminated.
    pub(crate) name_free: Duration,
    /// For ssh to reach the builder: its sshd can lag its "running" state.
    pub(crate) connect_window: Duration,
    /// Between two connection attempts within that window.
    pub(crate) connect_pause: Duration,
    /// Each script's bound.
    pub(crate) ready: Duration,
    pub(crate) packages: Duration,
    pub(crate) agent: Duration,
    pub(crate) prepare: Duration,
    /// For the snapshot to complete.
    pub(crate) snapshot: Duration,
    /// For the best-effort cleanup after a failed build.
    pub(crate) failure_sweep: Duration,
}

impl Default for BuildTimings {
    fn default() -> Self {
        Self {
            poll: Duration::from_secs(2),
            start: Duration::from_secs(15 * 60),
            name_free: Duration::from_secs(2 * 60),
            connect_window: Duration::from_secs(60),
            connect_pause: Duration::from_secs(3),
            // systemd gets 180 s inside the script.
            ready: Duration::from_secs(4 * 60),
            packages: Duration::from_secs(45 * 60),
            agent: Duration::from_secs(10 * 60),
            prepare: Duration::from_secs(2 * 60),
            snapshot: Duration::from_secs(30 * 60),
            failure_sweep: Duration::from_secs(2 * 60),
        }
    }
}

// ---------------------------------------------------------------------------
// Persisted state
// ---------------------------------------------------------------------------

/// The snapshot a build recorded. Times are seconds since the epoch.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub(crate) struct BuiltSnapshot {
    pub(crate) id: String,
    pub(crate) built_at: i64,
    pub(crate) base_image: String,
    /// The builder's disk, which is the smallest disk a sandbox started from
    /// this snapshot can have.
    pub(crate) disk_gb: u32,
    pub(crate) packages: Vec<String>,
    pub(crate) claude_version: Option<String>,
    pub(crate) codex_version: Option<String>,
}

/// How the last build failed, for the list's status line.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct FailureRecord {
    pub(crate) step: BuildStep,
    pub(crate) message: String,
    pub(crate) at: i64,
}

/// A recipe's `state_json`: the helm's own build bookkeeping, written only
/// by the build engine, the recipe's deletion, and a save that stops the
/// recipe being managed (which retires its snapshot). `{}` decodes to the
/// default; unknown fields are ignored so an older helm can read a newer
/// state.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub(crate) struct RecipeState {
    /// The snapshot hosts start from.
    pub(crate) snapshot: Option<BuiltSnapshot>,
    /// The builder sandbox, from right after create (or from when one is
    /// found by its name) until it is terminated and its snapshots are
    /// deleted.
    pub(crate) builder_sandbox_id: Option<String>,
    /// When a builder create was sent whose outcome Farhelm does not know
    /// yet: written before the request goes out, cleared when the builder's
    /// ID is recorded, when Tensorlake refused the create outright, or when
    /// a lookup of the builder's name finds nothing at least
    /// [`CREATE_SETTLE_SECS`] after the create was sent. While set, the next
    /// build and the recipe's deletion must look the name up.
    pub(crate) pending_create_at: Option<i64>,
    /// Set when a build starts and cleared when it ends either way; set with
    /// no live build means the helm stopped mid-build.
    pub(crate) build_started_at: Option<i64>,
    pub(crate) last_failure: Option<FailureRecord>,
    /// Snapshots this recipe no longer uses whose deletion has not succeeded
    /// yet: superseded by a rebuild, or left when the recipe stopped being
    /// managed.
    pub(crate) retired_snapshots: Vec<String>,
}

impl RecipeState {
    /// Whether the state records anything that may still bill on Tensorlake,
    /// or a build that may have created something unrecorded (an
    /// interrupted one, whose builder may exist under its name only).
    pub(crate) fn needs_cleanup(&self) -> bool {
        self.snapshot.is_some()
            || self.builder_sandbox_id.is_some()
            || self.pending_create_at.is_some()
            || self.build_started_at.is_some()
            || !self.retired_snapshots.is_empty()
    }
}

/// Decode a stored state. A state that does not decode is an error rather
/// than the default: the default would forget recorded IDs of billed
/// resources.
pub(super) fn decode_state(name: &str, json: &str) -> anyhow::Result<RecipeState> {
    serde_json::from_str(json).map_err(|_| {
        anyhow::anyhow!("Tensorlake recipe {name:?} has build state this build cannot read")
    })
}

impl HelmStore {
    /// Every recipe with its build state, ordered by name.
    pub(crate) async fn tensorlake_recipe_rows(
        &self,
    ) -> anyhow::Result<Vec<(String, StoredRecipe, RecipeState)>> {
        self.conn()
            .call("Tensorlake recipe state read task panicked", |conn| {
                let mut stmt = conn
                    .prepare(
                        "SELECT name, spec_json, state_json FROM tensorlake_recipes ORDER BY name",
                    )
                    .context("preparing the Tensorlake recipe read")?;
                let rows = stmt
                    .query_map([], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    })
                    .context("reading Tensorlake recipes")?
                    .collect::<Result<Vec<_>, _>>()
                    .context("reading Tensorlake recipes")?;
                rows.into_iter()
                    .map(|(name, spec, state)| {
                        let spec = decode_spec(&name, &spec)?;
                        let state = decode_state(&name, &state)?;
                        Ok((name, spec, state))
                    })
                    .collect()
            })
            .await
    }

    /// One recipe with its build state, `None` when no recipe has the name.
    pub(crate) async fn tensorlake_recipe_row(
        &self,
        name: &str,
    ) -> anyhow::Result<Option<(StoredRecipe, RecipeState)>> {
        let name = name.to_string();
        self.conn()
            .call("Tensorlake recipe state read task panicked", move |conn| {
                use rusqlite::OptionalExtension as _;
                let row: Option<(String, String)> = conn
                    .query_row(
                        "SELECT spec_json, state_json FROM tensorlake_recipes WHERE name = ?1",
                        [&name],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()
                    .context("reading a Tensorlake recipe")?;
                row.map(|(spec, state)| {
                    Ok((decode_spec(&name, &spec)?, decode_state(&name, &state)?))
                })
                .transpose()
            })
            .await
    }

    /// Read-modify-write one recipe's `state_json` in an immediate
    /// transaction, with its current spec for decisions that depend on it.
    /// `None` when no recipe has the name. Never writes `spec_json`.
    pub(crate) async fn update_tensorlake_recipe_state<T: Send + 'static>(
        &self,
        name: &str,
        change: impl FnOnce(&StoredRecipe, &mut RecipeState) -> T + Send + 'static,
    ) -> anyhow::Result<Option<T>> {
        let name = name.to_string();
        self.conn()
            .call("Tensorlake recipe state write task panicked", move |conn| {
                use rusqlite::OptionalExtension as _;
                let tx = conn
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                    .context("starting the Tensorlake recipe state write")?;
                let row: Option<(String, String)> = tx
                    .query_row(
                        "SELECT spec_json, state_json FROM tensorlake_recipes WHERE name = ?1",
                        [&name],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()
                    .context("reading a Tensorlake recipe")?;
                let Some((spec, state)) = row else {
                    return Ok(None);
                };
                let spec = decode_spec(&name, &spec)?;
                let mut state = decode_state(&name, &state)?;
                let result = change(&spec, &mut state);
                let json =
                    serde_json::to_string(&state).context("encoding Tensorlake build state")?;
                tx.execute(
                    "UPDATE tensorlake_recipes SET state_json = ?2 WHERE name = ?1",
                    rusqlite::params![name, json],
                )
                .context("storing Tensorlake build state")?;
                tx.commit()
                    .context("committing the Tensorlake build state")?;
                Ok(Some(result))
            })
            .await
    }

    /// Every snapshot ID a recipe records as its current snapshot. A sweep
    /// never deletes one of these, whatever sandbox it came from.
    async fn recorded_tensorlake_snapshots(&self) -> anyhow::Result<HashSet<String>> {
        Ok(self
            .tensorlake_recipe_rows()
            .await?
            .into_iter()
            .filter_map(|(_, _, state)| state.snapshot.map(|snapshot| snapshot.id))
            .collect())
    }
}

// ---------------------------------------------------------------------------
// The registry
// ---------------------------------------------------------------------------

/// A running build's progress, as the GET reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Progress {
    step: BuildStep,
    started_at: i64,
}

/// One running build.
struct ActiveBuild {
    progress: watch::Sender<Progress>,
    /// Set to true to cancel the build at its next wait.
    cancel: watch::Sender<bool>,
    /// The build's task, taken by whoever waits for it to end (a delete).
    task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

#[derive(Default)]
struct Registry {
    active: HashMap<String, Arc<ActiveBuild>>,
    /// Recipes a delete is cleaning up; no build may start for them.
    deleting: HashSet<String>,
}

/// Why a build did not start.
#[derive(Debug)]
pub(crate) enum StartRefusal {
    Missing,
    /// The recipe reuses an image or snapshot; Farhelm builds nothing for it.
    NotManaged,
    NoApiKey,
    /// Farhelm's ssh key is missing or unreadable; the text says which.
    NoSshKey(String),
    /// The recipe is being deleted.
    Deleting,
    Storage(anyhow::Error),
}

/// Why a delete's cleanup did not finish.
#[derive(Debug)]
pub(crate) enum CleanupRefusal {
    Missing,
    /// Another delete of the same recipe is running.
    Deleting,
    NoApiKey,
    /// Tensorlake refused or failed a deletion; the text is for display.
    Tensorlake(String),
    Storage(anyhow::Error),
}

/// The helm's snapshot builds. One per helm, shared by the routes.
pub(crate) struct RecipeBuilds {
    store: HelmStore,
    api: Arc<dyn TensorlakeApi>,
    shell: Arc<dyn SandboxShell>,
    /// `None` for a helm without a state directory, which keeps no key.
    ssh_keys: Option<Arc<SshKeyStore>>,
    timings: BuildTimings,
    registry: Mutex<Registry>,
    /// Makes the next write of a new builder's ID fail, for tests of the
    /// window between a create and its record.
    #[cfg(test)]
    fail_builder_record: std::sync::atomic::AtomicBool,
}

/// What one build works with, fixed when it starts.
struct BuildContext {
    recipe: String,
    builder: String,
    key: ApiKey,
    access: TensorlakeSshAccess,
    size: RecipeSize,
    packages: Vec<String>,
    events: Arc<FleetEvents>,
    progress: watch::Sender<Progress>,
    cancel: watch::Receiver<bool>,
    started_at: i64,
}

impl BuildContext {
    /// Enter `step`: publish it and tell clients to re-read.
    fn enter(&self, step: BuildStep) {
        self.progress.send_replace(Progress {
            step,
            started_at: self.started_at,
        });
        self.events.bump();
    }
}

/// Why a build stopped early.
enum Stop {
    /// A delete cancelled it; the delete cleans up.
    Cancelled,
    Failed(BuildStep, String),
}

/// What a step's failure says: a step failure with its message.
fn failed(step: BuildStep, message: impl std::fmt::Display) -> Stop {
    Stop::Failed(step, message.to_string())
}

/// A storage failure inside a build fails the build at `step`; the error is
/// logged in full and shown as a fixed sentence.
fn storage(step: BuildStep, error: anyhow::Error) -> Stop {
    tracing::warn!("Tensorlake snapshot build: storage failed: {error:#}");
    failed(step, "Farhelm could not save the build's progress")
}

impl RecipeBuilds {
    pub(crate) fn new(
        store: HelmStore,
        api: Arc<dyn TensorlakeApi>,
        shell: Arc<dyn SandboxShell>,
        ssh_keys: Option<Arc<SshKeyStore>>,
        timings: BuildTimings,
    ) -> Self {
        Self {
            store,
            api,
            shell,
            ssh_keys,
            timings,
            registry: Mutex::new(Registry::default()),
            #[cfg(test)]
            fail_builder_record: std::sync::atomic::AtomicBool::new(false),
        }
    }

    fn registry(&self) -> std::sync::MutexGuard<'_, Registry> {
        self.registry.lock().expect("build registry poisoned")
    }

    /// The build fields of a recipe's view: its snapshot, the live build or
    /// how the last one ended, and whether something awaits deletion.
    pub(crate) fn view(
        &self,
        name: &str,
        state: &RecipeState,
    ) -> (Option<SnapshotView>, BuildView, bool) {
        let live = self
            .registry()
            .active
            .get(name)
            .map(|active| *active.progress.borrow());
        let build = match (live, state.build_started_at, &state.last_failure) {
            (Some(progress), _, _) => BuildView::Building {
                step: progress.step,
                started_at: progress.started_at,
            },
            (None, Some(started_at), _) => BuildView::Interrupted { started_at },
            (None, None, Some(failure)) => BuildView::Failed {
                step: failure.step,
                message: failure.message.clone(),
                at: failure.at,
            },
            (None, None, None) => BuildView::Idle,
        };
        let cleanup_pending = live.is_none()
            && (state.builder_sandbox_id.is_some()
                || state.pending_create_at.is_some()
                || !state.retired_snapshots.is_empty());
        let snapshot = state.snapshot.as_ref().map(|snapshot| SnapshotView {
            id: snapshot.id.clone(),
            built_at: snapshot.built_at,
            base_image: snapshot.base_image.clone(),
            disk_gb: snapshot.disk_gb,
            packages: snapshot.packages.clone(),
            claude_version: snapshot.claude_version.clone(),
            codex_version: snapshot.codex_version.clone(),
        });
        (snapshot, build, cleanup_pending)
    }

    /// Whether a build of `name` is running.
    pub(crate) fn is_building(&self, name: &str) -> bool {
        self.registry().active.contains_key(name)
    }

    /// The saved API key, if any.
    async fn api_key(&self) -> anyhow::Result<Option<ApiKey>> {
        Ok(self.store.tensorlake_config().await?.api_key())
    }

    /// Start a build of the recipe `name`, or join the one already running
    /// (a second Build, Rebuild or Retry click starts nothing).
    ///
    /// Refuses a recipe that does not exist or is not managed, and a helm
    /// without an API key or ssh key. `events` is bumped at every step.
    pub(crate) async fn start_or_join(
        self: &Arc<Self>,
        name: &str,
        events: Arc<FleetEvents>,
    ) -> Result<(), StartRefusal> {
        self.start_definition(name, events, None).await
    }

    /// Build the definition a host's confirmation froze, while retaining the
    /// same per-recipe admission as Settings' rebuild action. The public
    /// rebuild route passes no override and reads the current definition.
    async fn start_definition(
        self: &Arc<Self>,
        name: &str,
        events: Arc<FleetEvents>,
        frozen: Option<StoredRecipe>,
    ) -> Result<(), StartRefusal> {
        if self.is_building(name) {
            return Ok(());
        }
        let (stored, _) = self
            .store
            .tensorlake_recipe_row(name)
            .await
            .map_err(StartRefusal::Storage)?
            .ok_or(StartRefusal::Missing)?;
        let spec = frozen.unwrap_or(stored);
        let RecipeSource::Packages { packages } = spec.source else {
            return Err(StartRefusal::NotManaged);
        };
        let key = self
            .api_key()
            .await
            .map_err(StartRefusal::Storage)?
            .ok_or(StartRefusal::NoApiKey)?;
        let access = self.ssh_access().await.map_err(StartRefusal::NoSshKey)?;
        let tag = helm_tag(
            &self
                .store
                .registry_namespace()
                .await
                .map_err(StartRefusal::Storage)?,
        );

        let mut registry = self.registry();
        if registry.deleting.contains(name) {
            return Err(StartRefusal::Deleting);
        }
        if registry.active.contains_key(name) {
            return Ok(());
        }
        let started_at = now_secs();
        let (progress, _) = watch::channel(Progress {
            step: BuildStep::Cleaning,
            started_at,
        });
        let (cancel, cancel_rx) = watch::channel(false);
        let active = Arc::new(ActiveBuild {
            progress: progress.clone(),
            cancel,
            task: Mutex::new(None),
        });
        let context = BuildContext {
            recipe: name.to_string(),
            builder: builder_name(&tag, name),
            key,
            access,
            size: spec.size,
            packages,
            events,
            progress,
            cancel: cancel_rx,
            started_at,
        };
        let engine = Arc::clone(self);
        let mine = Arc::clone(&active);
        // The task removes its own entry when it ends; it cannot get there
        // before the entry exists, because removing needs the registry lock
        // held here until the insert below.
        let task = tokio::spawn(async move {
            engine.run(&context).await;
            let mut registry = engine.registry();
            if registry
                .active
                .get(&context.recipe)
                .is_some_and(|current| Arc::ptr_eq(current, &mine))
            {
                registry.active.remove(&context.recipe);
            }
            drop(registry);
            context.events.bump();
        });
        *active.task.lock().expect("build task slot poisoned") = Some(task);
        registry.active.insert(name.to_string(), active);
        Ok(())
    }

    /// Return a managed recipe's prepared snapshot, building it when absent
    /// and joining an already running rebuild. A host keeps its confirmed
    /// definition; edits made while confirmation was open cannot change the
    /// packages or size of a build it starts. Existing snapshots keep the
    /// recipe's explicit-rebuild semantics (package edits alone do not rebuild).
    /// Progress follows the same watch channel Settings reads, without a
    /// second builder, detached waiter, or polling timer.
    pub(crate) async fn ensure_snapshot<F, Fut>(
        self: &Arc<Self>,
        name: &str,
        frozen: &StoredRecipe,
        events: Arc<FleetEvents>,
        mut progress: F,
    ) -> anyhow::Result<String>
    where
        F: FnMut(String) -> Fut,
        Fut: Future<Output = ()>,
    {
        let (_, state) = self
            .store
            .tensorlake_recipe_row(name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("the recipe no longer exists"))?;
        if !self.is_building(name)
            && let Some(snapshot) = state.snapshot
        {
            anyhow::ensure!(
                snapshot.disk_gb <= frozen.size.disk_gb,
                "the recipe's snapshot needs a larger disk; increase the recipe's disk size or rebuild its snapshot"
            );
            return Ok(snapshot.id);
        }
        self.start_definition(name, events, Some(frozen.clone()))
            .await
            .map_err(|reason| {
                anyhow::anyhow!(match reason {
                    StartRefusal::Missing => "the recipe no longer exists".into(),
                    StartRefusal::NotManaged => "this recipe does not build a snapshot".into(),
                    StartRefusal::NoApiKey =>
                        "save a Tensorlake API key in Settings → Tensorlake first".into(),
                    StartRefusal::NoSshKey(message) => message,
                    StartRefusal::Deleting => "the recipe is being deleted".into(),
                    StartRefusal::Storage(_) => "Farhelm could not read the recipe".into(),
                })
            })?;
        let receiver = self
            .registry()
            .active
            .get(name)
            .map(|active| active.progress.subscribe());
        if let Some(mut receiver) = receiver {
            loop {
                let step = receiver.borrow_and_update().step;
                progress(format!("Snapshot build: {step:?}")).await;
                if receiver.changed().await.is_err() {
                    break;
                }
            }
        }
        let (_, state) = self
            .store
            .tensorlake_recipe_row(name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("the recipe no longer exists"))?;
        if let Some(failure) = state.last_failure {
            anyhow::bail!("{}", failure.message);
        }
        let snapshot = state.snapshot.ok_or_else(|| {
            anyhow::anyhow!("the snapshot build ended without a snapshot; run setup again")
        })?;
        anyhow::ensure!(
            snapshot.disk_gb <= frozen.size.disk_gb,
            "the recipe's snapshot needs a larger disk; increase the recipe's disk size or rebuild its snapshot"
        );
        Ok(snapshot.id)
    }

    /// Farhelm's ssh access, or why it has none. The key is never created
    /// here: creating it is the user's explicit action in Settings.
    async fn ssh_access(&self) -> Result<TensorlakeSshAccess, String> {
        let Some(keys) = &self.ssh_keys else {
            return Err("this helm keeps no SSH key for Tensorlake".into());
        };
        match keys.read().await? {
            Some(key) => Ok(TensorlakeSshAccess::new(
                keys.tensorlake_dir(),
                key.private_key_path,
            )),
            None => Err(
                "create Farhelm's SSH key in Settings → Tensorlake and register it with Tensorlake first"
                    .into(),
            ),
        }
    }

    /// One whole build, ending with the state written either way.
    async fn run(&self, ctx: &BuildContext) {
        let at = ctx.started_at;
        let started = self
            .store
            .update_tensorlake_recipe_state(&ctx.recipe, move |_, state| {
                state.build_started_at = Some(at);
                state.last_failure = None;
            })
            .await;
        match started {
            Ok(Some(())) => {}
            Ok(None) => return,
            Err(error) => {
                tracing::warn!("Tensorlake snapshot build could not start: {error:#}");
                return;
            }
        }
        ctx.events.bump();
        let mut builder = None;
        let outcome = self.steps(ctx, &mut builder).await;
        let failure = match outcome {
            Ok(()) => return,
            Err(Stop::Cancelled) => None,
            Err(Stop::Failed(step, message)) => {
                if let Some(id) = &builder {
                    // Best effort and bounded: what fails here stays recorded
                    // for the next build or the recipe's deletion. `builder`
                    // is only ever a durably recorded ID, so the sweep can
                    // never terminate a sandbox nothing records.
                    let sweep = self.sweep_builder(&ctx.recipe, &ctx.key, id);
                    let problem =
                        match tokio::time::timeout(self.timings.failure_sweep, sweep).await {
                            Ok(Ok(())) => None,
                            Ok(Err(error)) => Some(error.to_string()),
                            Err(_) => Some("it did not finish in time".to_string()),
                        };
                    if let Some(problem) = problem {
                        tracing::warn!(
                            recipe = %ctx.recipe,
                            "could not clean up after a failed snapshot build: {problem}"
                        );
                    }
                }
                Some(FailureRecord {
                    step,
                    message,
                    at: now_secs(),
                })
            }
        };
        if let Err(error) = self
            .store
            .update_tensorlake_recipe_state(&ctx.recipe, move |_, state| {
                state.build_started_at = None;
                if failure.is_some() {
                    state.last_failure = failure;
                }
            })
            .await
        {
            tracing::warn!("Tensorlake snapshot build could not record its end: {error:#}");
        }
    }

    /// Wait for `future` unless the build is cancelled first.
    async fn remote<T>(
        &self,
        ctx: &BuildContext,
        future: impl Future<Output = T>,
    ) -> Result<T, Stop> {
        let mut cancel = ctx.cancel.clone();
        tokio::select! {
            biased;
            _ = cancel.wait_for(|cancelled| *cancelled) => Err(Stop::Cancelled),
            output = future => Ok(output),
        }
    }

    /// Sleep one poll interval, cancellably.
    async fn pause(&self, ctx: &BuildContext, length: Duration) -> Result<(), Stop> {
        self.remote(ctx, tokio::time::sleep(length)).await
    }

    /// The build's steps, in order. `builder` is set as soon as the builder
    /// exists, for the failure path's cleanup.
    async fn steps(&self, ctx: &BuildContext, builder: &mut Option<String>) -> Result<(), Stop> {
        ctx.enter(BuildStep::Cleaning);
        self.clean_leftovers(ctx).await?;

        ctx.enter(BuildStep::Creating);
        let id = self.create_builder(ctx, builder).await?;

        ctx.enter(BuildStep::Starting);
        self.wait_running(ctx, &id).await?;

        ctx.enter(BuildStep::Connecting);
        self.connect(ctx).await?;

        ctx.enter(BuildStep::Packages);
        self.script(
            ctx,
            BuildStep::Packages,
            "packages",
            &build_script::packages(&ctx.packages),
            self.timings.packages,
        )
        .await?;

        ctx.enter(BuildStep::ClaudeCode);
        let claude = self
            .script(
                ctx,
                BuildStep::ClaudeCode,
                "claude",
                &build_script::claude_code(),
                self.timings.agent,
            )
            .await?
            .last_line();

        ctx.enter(BuildStep::Codex);
        let codex = self
            .script(
                ctx,
                BuildStep::Codex,
                "codex",
                &build_script::codex(),
                self.timings.agent,
            )
            .await?
            .last_line();

        ctx.enter(BuildStep::Snapshotting);
        self.script(
            ctx,
            BuildStep::Snapshotting,
            "prepare",
            &build_script::prepare(),
            self.timings.prepare,
        )
        .await?;
        let snapshot = self.take_snapshot(ctx, &id).await?;

        ctx.enter(BuildStep::Finishing);
        let built = BuiltSnapshot {
            id: snapshot,
            built_at: now_secs(),
            base_image: TENSORLAKE_BASE_IMAGE.to_string(),
            disk_gb: ctx.size.disk_gb,
            packages: ctx.packages.clone(),
            claude_version: claude,
            codex_version: codex,
        };
        // One write records the new snapshot and retires the old one, so the
        // old one is deleted only after the new one is recorded. A recipe
        // that stopped being managed while this build ran keeps no snapshot:
        // the new one is retired at once.
        self.store
            .update_tensorlake_recipe_state(&ctx.recipe, move |spec, state| {
                if let Some(old) = state.snapshot.take() {
                    state.retired_snapshots.push(old.id);
                }
                if matches!(spec.source, RecipeSource::Packages { .. }) {
                    state.snapshot = Some(built);
                } else {
                    state.retired_snapshots.push(built.id);
                }
                state.build_started_at = None;
                state.last_failure = None;
            })
            .await
            .map_err(|error| storage(BuildStep::Finishing, error))?;
        // The snapshot is recorded: from here on the build has succeeded, and
        // a failed deletion only leaves something for the next build.
        if let Err(error) = self
            .remote(ctx, self.sweep_builder(&ctx.recipe, &ctx.key, &id))
            .await?
        {
            tracing::warn!(recipe = %ctx.recipe, "could not delete the build sandbox: {error}");
        }
        if let Err(error) = self
            .remote(ctx, self.delete_retired(&ctx.recipe, &ctx.key))
            .await?
        {
            tracing::warn!(recipe = %ctx.recipe, "could not delete a superseded snapshot: {error}");
        }
        Ok(())
    }

    /// Step 1: remove what earlier builds left: the recorded builder, any
    /// sandbox with the builder's name, and retired snapshots; then the name
    /// must be free for the create.
    async fn clean_leftovers(&self, ctx: &BuildContext) -> Result<(), Stop> {
        let step = BuildStep::Cleaning;
        self.remote(
            ctx,
            self.reconcile_builder(&ctx.recipe, &ctx.key, &ctx.builder),
        )
        .await?
        .map_err(|error| failed(step, error))?;
        // A retired snapshot that still cannot be deleted does not block the
        // build: it stays listed, and the list says something awaits cleanup.
        if let Err(error) = self
            .remote(ctx, self.delete_retired(&ctx.recipe, &ctx.key))
            .await?
        {
            tracing::warn!(recipe = %ctx.recipe, "could not delete a retired snapshot: {error}");
        }
        let deadline = tokio::time::Instant::now() + self.timings.name_free;
        loop {
            match self
                .remote(ctx, self.api.get_sandbox(&ctx.key, &ctx.builder))
                .await?
            {
                Err(TensorlakeError::NotFound { .. }) => return Ok(()),
                Ok(found) if found.status == SandboxStatus::Terminated => return Ok(()),
                Ok(_) => {}
                Err(error) => return Err(failed(step, error)),
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(failed(
                    step,
                    format!("the previous build sandbox {} did not go away", ctx.builder),
                ));
            }
            self.pause(ctx, self.timings.poll).await?;
        }
    }

    /// Step 2: create the builder and record its ID at once.
    ///
    /// Neither the create nor recording its outcome is cancellable: a
    /// create whose request is dropped can still make a sandbox, and only a
    /// recorded ID, or the pending marker written before the request goes
    /// out, attributes it. A delete waits for this step instead.
    ///
    /// An answer that is not a refusal (a transport failure, a reply that
    /// does not decode, a server error) does not mean nothing was created,
    /// so the builder is looked up by name and adopted if it exists; if it
    /// cannot be found yet, the pending marker stays for the next build or
    /// the delete to resolve. `builder` is set only once the ID is recorded:
    /// when recording fails, the builder is left running and the pending
    /// marker stays, so the next build or the delete finds it by name and
    /// adopts it before terminating it. Terminating it from the ID held only
    /// here would lose its snapshots' attribution if the sweep then failed.
    async fn create_builder(
        &self,
        ctx: &BuildContext,
        builder: &mut Option<String>,
    ) -> Result<String, Stop> {
        let step = BuildStep::Creating;
        let sent = now_secs();
        match self
            .store
            .update_tensorlake_recipe_state(&ctx.recipe, move |_, state| {
                state.pending_create_at = Some(sent);
            })
            .await
        {
            Ok(Some(())) => {}
            Ok(None) => return Err(failed(step, "the recipe was deleted")),
            Err(error) => return Err(storage(step, error)),
        }
        let request = CreateSandbox {
            name: ctx.builder.clone(),
            source: SandboxSource::Image(TENSORLAKE_BASE_IMAGE.to_string()),
            resources: Resources {
                cpus: f64::from(ctx.size.cpus),
                memory_mb: u64::from(ctx.size.memory_gb) * 1024,
                disk_mb: u64::from(ctx.size.disk_gb) * 1024,
            },
            timeout_secs: BUILDER_IDLE_TIMEOUT_SECS,
            auto_resume: false,
            max_pending_secs: 900,
        };
        let id = match self.api.create_sandbox(&ctx.key, &request).await {
            Ok(created) => created.sandbox_id,
            Err(error) if create_was_refused(&error) => {
                // Tensorlake answered and made nothing. If this write is
                // lost the marker only costs a later lookup.
                let _ = self
                    .store
                    .update_tensorlake_recipe_state(&ctx.recipe, |_, state| {
                        state.pending_create_at = None;
                    })
                    .await;
                return Err(failed(step, error));
            }
            Err(error) => match self.api.get_sandbox(&ctx.key, &ctx.builder).await {
                Ok(found) => found.id,
                _ => return Err(failed(step, error)),
            },
        };
        self.record_builder(&ctx.recipe, &id)
            .await
            .map_err(|error| storage(step, error))?;
        *builder = Some(id.clone());
        Ok(id)
    }

    /// Record `id` as the recipe's builder, which settles a pending create.
    async fn record_builder(&self, recipe: &str, id: &str) -> anyhow::Result<()> {
        #[cfg(test)]
        if self
            .fail_builder_record
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            anyhow::bail!("injected failure recording the builder");
        }
        let id = id.to_string();
        self.store
            .update_tensorlake_recipe_state(recipe, move |_, state| {
                state.builder_sandbox_id = Some(id);
                state.pending_create_at = None;
            })
            .await?
            .ok_or_else(|| anyhow::anyhow!("the recipe was deleted"))
    }

    /// Settle what earlier builds may have left of the recipe's builder,
    /// for the next build and for the recipe's deletion alike.
    ///
    /// A recorded builder is swept first, so its record is never replaced
    /// while its snapshots may remain. Then the builder's name is looked up:
    /// a sandbox found there (a create whose answer was lost, or one that
    /// finished after a crash) is recorded BEFORE it is terminated, because
    /// once terminated it no longer answers to its name and only the
    /// recorded ID attributes its snapshots; the record stays until the
    /// whole sweep succeeded. Nothing under the name settles a pending
    /// create only once [`CREATE_SETTLE_SECS`] have passed since it was
    /// sent.
    async fn reconcile_builder(
        &self,
        recipe: &str,
        key: &ApiKey,
        builder: &str,
    ) -> Result<(), ReconcileFailure> {
        let state = self
            .store
            .tensorlake_recipe_row(recipe)
            .await
            .map_err(ReconcileFailure::Storage)?
            .ok_or_else(|| ReconcileFailure::Storage(anyhow::anyhow!("the recipe was deleted")))?
            .1;
        if let Some(id) = &state.builder_sandbox_id {
            self.sweep_builder(recipe, key, id)
                .await
                .map_err(ReconcileFailure::Tensorlake)?;
        }
        match self.api.get_sandbox(key, builder).await {
            Ok(found) => {
                self.record_builder(recipe, &found.id)
                    .await
                    .map_err(ReconcileFailure::Storage)?;
                self.sweep_builder(recipe, key, &found.id)
                    .await
                    .map_err(ReconcileFailure::Tensorlake)?;
            }
            Err(TensorlakeError::NotFound { .. }) => {
                if let Some(sent) = state.pending_create_at {
                    if now_secs() < sent + CREATE_SETTLE_SECS {
                        return Err(ReconcileFailure::StillStarting);
                    }
                    self.store
                        .update_tensorlake_recipe_state(recipe, |_, state| {
                            state.pending_create_at = None;
                        })
                        .await
                        .map_err(ReconcileFailure::Storage)?;
                }
            }
            Err(error) => return Err(ReconcileFailure::Tensorlake(error)),
        }
        Ok(())
    }

    /// Step 3: wait for the builder to run.
    async fn wait_running(&self, ctx: &BuildContext, id: &str) -> Result<(), Stop> {
        let step = BuildStep::Starting;
        let deadline = tokio::time::Instant::now() + self.timings.start;
        loop {
            let info = self
                .remote(ctx, self.api.get_sandbox(&ctx.key, id))
                .await?
                .map_err(|error| failed(step, error))?;
            match info.status {
                SandboxStatus::Running => return Ok(()),
                SandboxStatus::Terminated => {
                    let reason = info
                        .termination_reason
                        .or(info.error_details)
                        .unwrap_or_else(|| "no reason given".into());
                    return Err(failed(
                        step,
                        format!("Tensorlake ended the build sandbox ({reason})"),
                    ));
                }
                _ => {}
            }
            if tokio::time::Instant::now() >= deadline {
                let waiting = info
                    .pending_reason
                    .map(|reason| format!(" ({reason})"))
                    .unwrap_or_default();
                return Err(failed(
                    step,
                    format!(
                        "the build sandbox did not start within {} minutes{waiting}",
                        self.timings.start.as_secs() / 60
                    ),
                ));
            }
            self.pause(ctx, self.timings.poll).await?;
        }
    }

    /// Step 4: reach the builder and check it can be a host. Only "could
    /// not reach it" is retried, within a short window (sshd can come up a
    /// moment after the sandbox reports running); a refused key fails at
    /// once, since retrying cannot register it.
    async fn connect(&self, ctx: &BuildContext) -> Result<(), Stop> {
        let step = BuildStep::Connecting;
        let deadline = tokio::time::Instant::now() + self.timings.connect_window;
        loop {
            match self
                .run_script(ctx, "ready", &build_script::ready(), self.timings.ready)
                .await?
            {
                Ok(_) => return Ok(()),
                Err(ShellFailure::Unreachable(_)) if tokio::time::Instant::now() < deadline => {
                    self.pause(ctx, self.timings.connect_pause).await?;
                }
                Err(error) => return Err(failed(step, error)),
            }
        }
    }

    /// Run one script in the builder, cancellably.
    async fn run_script(
        &self,
        ctx: &BuildContext,
        label: &str,
        script: &str,
        timeout: Duration,
    ) -> Result<Result<ShellOutput, ShellFailure>, Stop> {
        self.remote(
            ctx,
            self.shell
                .run(&ctx.access, &ctx.builder, label, script, timeout),
        )
        .await
    }

    /// Run one script, failing the build at `step` if it fails.
    async fn script(
        &self,
        ctx: &BuildContext,
        step: BuildStep,
        label: &str,
        script: &str,
        timeout: Duration,
    ) -> Result<ShellOutput, Stop> {
        self.run_script(ctx, label, script, timeout)
            .await?
            .map_err(|error| failed(step, error))
    }

    /// Step 8: take a filesystem snapshot and wait until Tensorlake has it
    /// durably (`completed`, not merely `local_ready`).
    ///
    /// The snapshot's ID is not recorded until it completes; a build that
    /// stops before then leaves it attributable by its source, the builder,
    /// whose sweep deletes it.
    async fn take_snapshot(&self, ctx: &BuildContext, id: &str) -> Result<String, Stop> {
        let step = BuildStep::Snapshotting;
        let snapshot = self
            .remote(
                ctx,
                self.api
                    .create_snapshot(&ctx.key, id, SnapshotType::Filesystem),
            )
            .await?
            .map_err(|error| failed(step, error))?
            .snapshot_id;
        let deadline = tokio::time::Instant::now() + self.timings.snapshot;
        loop {
            let info = self
                .remote(ctx, self.api.get_snapshot(&ctx.key, &snapshot))
                .await?
                .map_err(|error| failed(step, error))?;
            match info.status {
                SnapshotStatus::Completed => return Ok(snapshot),
                SnapshotStatus::Failed => {
                    return Err(failed(step, "Tensorlake could not complete the snapshot"));
                }
                SnapshotStatus::LocalReady | SnapshotStatus::Unknown => {}
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(failed(
                    step,
                    format!(
                        "the snapshot did not complete within {} minutes",
                        self.timings.snapshot.as_secs() / 60
                    ),
                ));
            }
            self.pause(ctx, self.timings.poll).await?;
        }
    }

    /// Terminate builder `id` and delete every snapshot taken from it except
    /// the ones a recipe records, then clear the recipe's record of the
    /// builder. A not-found answer counts as done everywhere. On any failure
    /// the record stays, so the next build or the delete tries again.
    async fn sweep_builder(
        &self,
        recipe: &str,
        key: &ApiKey,
        id: &str,
    ) -> Result<(), TensorlakeError> {
        match self.api.terminate_sandbox(key, id).await {
            Ok(()) | Err(TensorlakeError::NotFound { .. }) => {}
            Err(error) => return Err(error),
        }
        let keep = self
            .store
            .recorded_tensorlake_snapshots()
            .await
            .map_err(|error| {
                tracing::warn!("Tensorlake snapshot sweep: storage failed: {error:#}");
                TensorlakeError::Transport("Farhelm could not read its recorded snapshots".into())
            })?;
        let snapshots = self.api.list_snapshots(key).await?;
        let mut first_error = None;
        for snapshot in snapshots.snapshots.iter().filter(|snapshot| {
            snapshot.sandbox_id.as_deref() == Some(id) && !keep.contains(&snapshot.id)
        }) {
            match self.api.delete_snapshot(key, &snapshot.id).await {
                Ok(()) | Err(TensorlakeError::NotFound { .. }) => {}
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        let id = id.to_string();
        self.store
            .update_tensorlake_recipe_state(recipe, move |_, state| {
                if state.builder_sandbox_id.as_deref() == Some(id.as_str()) {
                    state.builder_sandbox_id = None;
                }
            })
            .await
            .map_err(|error| {
                tracing::warn!("Tensorlake snapshot sweep: storage failed: {error:#}");
                TensorlakeError::Transport("Farhelm could not record the deletion".into())
            })?;
        Ok(())
    }

    /// Delete the recipe's retired snapshots, removing each from the record
    /// once Tensorlake confirms (or says it is gone). Failures stay listed
    /// and are returned, the first one only.
    pub(crate) async fn delete_retired(
        &self,
        recipe: &str,
        key: &ApiKey,
    ) -> Result<(), TensorlakeError> {
        let retired = match self.store.tensorlake_recipe_row(recipe).await {
            Ok(Some((_, state))) => state.retired_snapshots,
            Ok(None) => return Ok(()),
            Err(error) => {
                tracing::warn!("Tensorlake snapshot cleanup: storage failed: {error:#}");
                return Err(TensorlakeError::Transport(
                    "Farhelm could not read its recorded snapshots".into(),
                ));
            }
        };
        let mut first_error = None;
        for id in retired {
            match self.api.delete_snapshot(key, &id).await {
                Ok(()) | Err(TensorlakeError::NotFound { .. }) => {
                    let _ = self
                        .store
                        .update_tensorlake_recipe_state(recipe, move |_, state| {
                            state.retired_snapshots.retain(|retired| *retired != id);
                        })
                        .await;
                }
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// Delete everything Tensorlake bills for the recipe `name`, for the
    /// recipe's deletion: cancel a running build and wait for it to stop,
    /// sweep the recorded builder and any sandbox with the builder's name,
    /// and delete the recipe's snapshot and retired snapshots. Each record
    /// is cleared as its deletion succeeds; on failure the rest stays, so a
    /// retry finishes the job.
    ///
    /// The returned guard keeps builds of `name` from starting until it is
    /// dropped, so the caller can delete the row with nothing new recorded.
    pub(crate) async fn clean_for_delete(
        self: &Arc<Self>,
        name: &str,
    ) -> Result<DeleteGuard, CleanupRefusal> {
        let task = {
            let mut registry = self.registry();
            if !registry.deleting.insert(name.to_string()) {
                return Err(CleanupRefusal::Deleting);
            }
            registry.active.get(name).and_then(|active| {
                active.cancel.send_replace(true);
                active.task.lock().expect("build task slot poisoned").take()
            })
        };
        let guard = DeleteGuard {
            builds: Arc::clone(self),
            name: name.to_string(),
        };
        if let Some(task) = task {
            // The task stops at its next wait and writes its end; a panic in
            // it has nothing left to wait for.
            let _ = task.await;
        }
        let (_, state) = self
            .store
            .tensorlake_recipe_row(name)
            .await
            .map_err(CleanupRefusal::Storage)?
            .ok_or(CleanupRefusal::Missing)?;
        // `needs_cleanup` covers a pending create and an interrupted build,
        // so a builder known only by its name is still looked up.
        if !state.needs_cleanup() {
            return Ok(guard);
        }
        let key = self
            .api_key()
            .await
            .map_err(CleanupRefusal::Storage)?
            .ok_or(CleanupRefusal::NoApiKey)?;
        let tag = helm_tag(
            &self
                .store
                .registry_namespace()
                .await
                .map_err(CleanupRefusal::Storage)?,
        );
        let tensorlake = |error: TensorlakeError| CleanupRefusal::Tensorlake(error.to_string());
        // The recipe's own snapshot goes to the retired list first, so the
        // sweeps below may delete it too (nothing records it any more) and
        // `delete_retired` removes whatever is left.
        self.store
            .update_tensorlake_recipe_state(name, |_, state| {
                if let Some(snapshot) = state.snapshot.take() {
                    state.retired_snapshots.push(snapshot.id);
                }
            })
            .await
            .map_err(CleanupRefusal::Storage)?;
        self.reconcile_builder(name, &key, &builder_name(&tag, name))
            .await
            .map_err(|failure| match failure {
                ReconcileFailure::Storage(error) => CleanupRefusal::Storage(error),
                other => CleanupRefusal::Tensorlake(other.to_string()),
            })?;
        self.delete_retired(name, &key).await.map_err(tensorlake)?;
        self.store
            .update_tensorlake_recipe_state(name, |_, state| {
                state.build_started_at = None;
            })
            .await
            .map_err(CleanupRefusal::Storage)?;
        Ok(guard)
    }
}

/// Whether a create failure is Tensorlake refusing the request, which
/// creates nothing, as opposed to an answer that leaves the outcome open.
pub(crate) fn create_was_refused(error: &TensorlakeError) -> bool {
    matches!(
        error,
        TensorlakeError::Auth { .. }
            | TensorlakeError::Refused { .. }
            | TensorlakeError::Conflict { .. }
            | TensorlakeError::NotFound { .. }
    )
}

/// Why settling a recipe's builder did not finish.
#[derive(Debug, thiserror::Error)]
enum ReconcileFailure {
    /// A create Farhelm sent may still produce a sandbox.
    #[error(
        "a build sandbox Farhelm asked Tensorlake for may still be starting; try again in a few minutes"
    )]
    StillStarting,
    #[error("Farhelm could not delete the previous build sandbox: {0}")]
    Tensorlake(TensorlakeError),
    #[error("Farhelm could not record the previous build sandbox")]
    Storage(anyhow::Error),
}

/// Holds off builds of a recipe being deleted; see
/// [`RecipeBuilds::clean_for_delete`].
pub(crate) struct DeleteGuard {
    builds: Arc<RecipeBuilds>,
    name: String,
}

impl Drop for DeleteGuard {
    fn drop(&mut self) {
        self.builds.registry().deleting.remove(&self.name);
    }
}

#[cfg(test)]
#[path = "build_tests.rs"]
mod tests;
