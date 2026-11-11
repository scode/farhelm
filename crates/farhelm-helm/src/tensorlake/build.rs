//! Recipe snapshots: a built software layer, then a lazy Farhelm installation
//! checkpoint that new hosts clone. Each layer has independent task admission.
//!
//! A build creates a builder sandbox from the recipe's base
//! ([`TENSORLAKE_BASE_IMAGE`] for Tensorlake Ubuntu) with the recipe's size,
//! checks the base's requirements ([`build_script::ready`]), installs its apt
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
//! until it is gone, in the recipe's `state_json` ([`RecipeState`]).
//!
//! The shared state also holds Podman artifacts by place. Every Tensorlake
//! read-modify-write preserves those fields in the same immediate transaction.
//! This registry admits both engines: Tensorlake jobs remain recipe-scoped;
//! Podman jobs join per recipe/place and cancel only between owned commands.
//! One recipe-wide deletion guard excludes new work in either engine, while
//! stable source locks protect selection through a clone's first Running.
//!
//!
//! - the builder's sandbox ID is persisted right after create returns. Its
//!   name alone is not enough: once terminated, a sandbox can no longer be
//!   found by name reliably, and its snapshots are attributable only by its
//!   ID;
//! - a crash before that point leaves the pending-create marker written
//!   before the request, which lets the next build recover by name. A name
//!   match without that marker is not evidence that this helm created it;
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
//!
//! Preparation follows the same ownership rules in a separate sandbox. Its
//! installer ([`SnapshotPreparer`]) receives no recipe credentials or host ID and
//! returns only after supervisor state is absent. A per-recipe mutex excludes
//! source retirement through clone create, durable ID recording and Running.
//! Installation does not hold that mutex: Rebuild may publish meanwhile, and
//! ensure-prepared rechecks the source before returning. This mutex is not a
//! durable dependency registry; interrupted creates retain ordinary vendor
//! failure/retry behavior when source retirement overlaps them.

use std::{
    collections::{HashMap, HashSet},
    future::Future,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::Context as _;
use farhelm_proto::tensorlake::recipes::{
    BuildStep, BuildView, PrepareStep, PrepareView, PreparedSnapshotView, RecipeBase, RecipeSize,
    SnapshotView,
};
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use super::api::{
    ApiKey, CreateSandbox, Resources, SandboxSource, SandboxStatus, SnapshotStatus, SnapshotType,
    TensorlakeApi, TensorlakeError,
};
use super::build_script;
use super::config::helm_tag;
use super::prepared::{PreparedSnapshot, SnapshotPreparer};
use super::recipes::{StoredRecipe, decode_spec, now_secs};
use super::shell::{SandboxShell, ShellFailure, ShellOutput};
use super::ssh::TensorlakeSshAccess;
use super::ssh_key::SshKeyStore;
use crate::{
    feed::FleetEvents,
    store::{HelmStore, HostRow},
};

/// The standard Tensorlake Ubuntu base. The one place its image
/// is named: goal D1's fallback, should Tensorlake's ssh to CAS-format
/// sandboxes regress, changes this to `tensorlake/legacy/ubuntu-systemd`.
pub(crate) const TENSORLAKE_BASE_IMAGE: &str = "tensorlake/ubuntu-systemd";

/// The name prefix of every builder sandbox. Specific on purpose: the
/// repository's browser-test harness names its sandboxes `farhelm-sbx-`, and
/// the orphan list must never take one of those for a builder.
pub(crate) const BUILDER_PREFIX: &str = "farhelm-build-";

/// Separate namespace so preparation cannot occupy the recipe's build name.
pub(crate) const PREPARER_PREFIX: &str = "farhelm-prepare-";

/// A deterministic name permits recovery only with its pending-create marker.
pub(crate) fn preparer_name(tag: &str, recipe: &str) -> String {
    format!("{PREPARER_PREFIX}{tag}-{recipe}")
}

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
    /// For the builder name to become free, including an unrelated occupant
    /// this helm must leave alone.
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
    /// Missing in older state, whose builds always used Tensorlake Ubuntu.
    pub(crate) base: RecipeBase,
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

// The build engine keeps its existing import path for shared recipe state.
pub(crate) use crate::environment::recipes::RecipeState;

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
        self.write_recipe_state(name, None, change).await
    }

    /// A place-backed engine records helper intent before run. Validate its
    /// current place in that same write transaction, so place removal cannot
    /// leave newly recorded ownership at an already forgotten endpoint. Entry
    /// UUID, kind and destination bind the row paired with its probe/commands;
    /// decorative edits and supervisor install paths do not select Podman I/O.
    pub(crate) async fn update_recipe_place_state<T: Send + 'static>(
        &self,
        name: &str,
        place: &HostRow,
        change: impl FnOnce(&StoredRecipe, &mut RecipeState) -> T + Send + 'static,
    ) -> anyhow::Result<Option<T>> {
        self.write_recipe_state(name, Some(place.clone()), change)
            .await
    }

    /// Definition and both providers' state still share one transactional
    /// writer. The optional guard adds no independent state or cleanup engine.
    async fn write_recipe_state<T: Send + 'static>(
        &self,
        name: &str,
        place: Option<HostRow>,
        change: impl FnOnce(&StoredRecipe, &mut RecipeState) -> T + Send + 'static,
    ) -> anyhow::Result<Option<T>> {
        let name = name.to_string();
        self.conn()
            .call("Tensorlake recipe state write task panicked", move |conn| {
                use rusqlite::OptionalExtension as _;
                let tx = conn
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                    .context("starting the Tensorlake recipe state write")?;
                if let Some(place) = place {
                    let (kind, environment, provider, identity, destination): (String, Option<String>, Option<String>, String, Option<String>) = tx.query_row(
                        "SELECT kind, tensorlake_sandbox, environment_provider, entry_uuid, destination FROM hosts WHERE id = ?1",
                        [place.id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
                    ).context("the container place was removed before its ownership could be recorded")?;
                    let kind = crate::store::HostKind::from_columns(&kind, environment.as_deref(), provider.as_deref())?;
                    anyhow::ensure!(kind.can_host_environments() && kind == place.kind && identity == place.entry_uuid && destination == place.destination,
                        "the container place changed before its ownership could be recorded; retry at the current place");
                }
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

    /// Both current snapshot layers are protected, whatever sandbox produced
    /// them. A builder sweep must not erase a separately recorded installation.
    async fn recorded_tensorlake_snapshots(&self) -> anyhow::Result<HashSet<String>> {
        Ok(self
            .tensorlake_recipe_rows()
            .await?
            .into_iter()
            .flat_map(|(_, _, state)| {
                state
                    .snapshot
                    .map(|snapshot| snapshot.id)
                    .into_iter()
                    .chain(state.prepared.map(|snapshot| snapshot.id))
            })
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
    preparing: HashMap<String, Arc<ActivePreparation>>,
    /// Never remove a mutex while callers can retain its owned guard: a new
    /// mutex for the same recipe would defeat source-retirement exclusion.
    snapshot_use: HashMap<String, Arc<tokio::sync::Mutex<()>>>,
    /// Recipes a delete is cleaning up; no build may start for them.
    deleting: HashSet<String>,
    /// Place work shares deletion admission with Tensorlake. Job-owned watch
    /// senders bound retained progress and let concurrent launches join one
    /// outcome rather than starting another build after the first one fails.
    podman_jobs: HashMap<(String, crate::store::HostId), PodmanJob>,
}

/// A Rebuild may accept launch joiners, while a preparation-only job cannot
/// claim that a Rebuild click will replace software. Cancellation is checked
/// between bounded commands, never by aborting a mutating command halfway.
struct PodmanJob {
    progress: watch::Receiver<PodmanProgress>,
    rebuild: bool,
    cancel: Arc<std::sync::atomic::AtomicBool>,
}

/// One owned place job's latest named step and eventual result.
#[derive(Clone, Default)]
pub(crate) struct PodmanProgress {
    pub(crate) line: String,
    pub(crate) result: Option<Result<crate::environment::recipes::PreparedImage, String>>,
}

/// A panic or runtime cancellation cannot leave admission permanently busy.
/// Durable container/image ownership remains for explicit later cleanup.
struct PodmanJobAdmission {
    owner: Arc<RecipeBuilds>,
    key: (String, crate::store::HostId),
}

impl Drop for PodmanJobAdmission {
    fn drop(&mut self) {
        self.owner.registry().podman_jobs.remove(&self.key);
    }
}

/// Why a build did not start.
#[derive(Debug)]
pub(crate) enum StartRefusal {
    Missing,
    /// The shared catalog can contain bases this build engine cannot use.
    UnsupportedBase,
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

/// Both recipe snapshot jobs share this registry, while retaining independent
/// single-flight admission, progress and sandbox ownership. One per helm.
pub(crate) struct RecipeBuilds {
    store: HelmStore,
    api: Arc<dyn TensorlakeApi>,
    shell: Arc<dyn SandboxShell>,
    preparer: Arc<dyn SnapshotPreparer>,
    helm_version: String,
    /// `None` for a helm without a state directory, which keeps no key.
    ssh_keys: Option<Arc<SshKeyStore>>,
    timings: BuildTimings,
    registry: Mutex<Registry>,
    /// Makes the next write of a new builder's ID fail, for tests of the
    /// window between a create and its record.
    #[cfg(test)]
    fail_builder_record: std::sync::atomic::AtomicBool,
    /// The preparation counterpart exercises ownership after an acknowledged
    /// create whose durable ID write fails.
    #[cfg(test)]
    fail_preparer_record: std::sync::atomic::AtomicBool,
}

/// What one build works with, fixed when it starts.
struct BuildContext {
    recipe: String,
    builder: String,
    key: ApiKey,
    access: TensorlakeSshAccess,
    size: RecipeSize,
    base: RecipeBase,
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
    /// Settings reads shared durable state and this registry synchronously.
    /// The Place match and recipe-state layout are exhaustive provider homes;
    /// image existence belongs to actual reuse, never a list's probe fan-out.
    #[warn(clippy::wildcard_enum_match_arm)]
    pub(crate) fn place_view(
        &self,
        name: &str,
        state: &RecipeState,
        place: farhelm_proto::Place,
    ) -> farhelm_proto::recipes::RecipePlaceView {
        let (built_at, prepared_at, stale, present, building, step, failure, cleanup_pending) =
            match place {
                farhelm_proto::Place::TensorlakeAccount => {
                    let (building, step) = {
                        let registry = self.registry();
                        let step = registry
                            .active
                            .get(name)
                            .map(|job| job.progress.borrow().step.label().to_owned())
                            .or_else(|| {
                                registry
                                    .preparing
                                    .get(name)
                                    .map(|job| job.progress.borrow().step.label().to_owned())
                            });
                        (
                            registry.active.contains_key(name)
                                || registry.preparing.contains_key(name),
                            step,
                        )
                    };
                    let stale = state.prepared.as_ref().and_then(|prepared| {
                        prepared.stale_reason(
                            now_secs(),
                            &self.helm_version,
                            state.snapshot.as_ref().map(|image| image.id.as_str()),
                        )
                    });
                    let failure = state
                        .last_prepare_failure
                        .as_ref()
                        .map(|failure| failure.message.clone())
                        .or_else(|| {
                            state
                                .last_failure
                                .as_ref()
                                .map(|failure| failure.message.clone())
                        });
                    (
                        state.snapshot.as_ref().map(|image| image.built_at),
                        state.prepared.as_ref().map(|image| image.prepared_at),
                        stale,
                        state.prepared.is_some(),
                        building,
                        step,
                        failure,
                        self.view(name, state).2,
                    )
                }
                farhelm_proto::Place::PodmanHost(host) => {
                    let current = state.podman.get(&host);
                    let (building, step) = {
                        let registry = self.registry();
                        let job = registry.podman_jobs.get(&(name.to_owned(), host));
                        (
                            job.is_some(),
                            job.map(|job| job.progress.borrow().line.clone())
                                .filter(|line| !line.is_empty()),
                        )
                    };
                    let stale = current
                        .and_then(|state| state.prepared.as_ref().map(|prepared| (state, prepared)))
                        .and_then(|(state, prepared)| {
                            crate::environment::recipes::prepared_stale_reason(
                                prepared.prepared_at,
                                &prepared.farhelm_version,
                                &prepared.source_image,
                                now_secs(),
                                &self.helm_version,
                                state.image.as_ref().map(|image| image.id.as_str()),
                            )
                        });
                    let cleanup_pending = !building
                        && current.is_some_and(|state| {
                            state.builder_container_id.is_some()
                                || state.preparer_container_id.is_some()
                                || state.build_started_at.is_some()
                                || state.prepare_started_at.is_some()
                                || !state.retired_images.is_empty()
                        });
                    (
                        current.and_then(|state| state.image.as_ref().map(|image| image.built_at)),
                        current.and_then(|state| {
                            state.prepared.as_ref().map(|image| image.prepared_at)
                        }),
                        stale,
                        current.is_some_and(|state| state.prepared.is_some()),
                        building,
                        step,
                        current.and_then(|state| state.last_failure.clone()),
                        cleanup_pending,
                    )
                }
            };
        let freshness = stale.map(|reason| {
            match reason {
                farhelm_proto::recipes::PreparedStaleReason::FarhelmUpdated => {
                    "Farhelm updated; preparation runs at next launch"
                }
                farhelm_proto::recipes::PreparedStaleReason::SnapshotRebuilt => {
                    "the software image changed; preparation runs at next launch"
                }
                farhelm_proto::recipes::PreparedStaleReason::Expired => {
                    "preparation is more than 24 hours old; it runs at next launch"
                }
            }
            .to_owned()
        });
        farhelm_proto::recipes::RecipePlaceView {
            recipe: name.to_owned(),
            place,
            ready: built_at.is_some() && present && freshness.is_none(),
            building,
            reason: failure.or(freshness),
            built_at,
            prepared_at,
            step,
            cleanup_pending,
        }
    }
    /// Admit place work under the same lock as recipe deletion. The owned
    /// task survives its initiating waiter, and every joiner observes its
    /// actual outcome. A new later attempt may retry a finished failure.
    pub(crate) fn start_podman<F, Fut>(
        self: &Arc<Self>,
        name: &str,
        place: crate::store::HostId,
        rebuild: bool,
        work: F,
    ) -> anyhow::Result<watch::Receiver<PodmanProgress>>
    where
        F: FnOnce(watch::Sender<PodmanProgress>, Arc<std::sync::atomic::AtomicBool>) -> Fut
            + Send
            + 'static,
        Fut: Future<Output = anyhow::Result<crate::environment::recipes::PreparedImage>>
            + Send
            + 'static,
    {
        let key = (name.to_owned(), place);
        let mut registry = self.registry();
        anyhow::ensure!(
            !registry.deleting.contains(name),
            "the recipe is being deleted"
        );
        if let Some(job) = registry.podman_jobs.get(&key) {
            anyhow::ensure!(
                !rebuild || job.rebuild,
                "the recipe is already preparing here; wait for it before rebuilding"
            );
            return Ok(job.progress.clone());
        }
        let (progress, receiver) = watch::channel(PodmanProgress::default());
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        registry.podman_jobs.insert(
            key.clone(),
            PodmanJob {
                progress: receiver.clone(),
                rebuild,
                cancel: cancel.clone(),
            },
        );
        let owner = Arc::clone(self);
        tokio::spawn(async move {
            let _admission = PodmanJobAdmission { owner, key };
            let result = work(progress.clone(), cancel)
                .await
                .map_err(|error| super::shell::shown(&error.to_string()));
            progress.send_modify(|view| view.result = Some(result));
        });
        Ok(receiver)
    }

    /// Select/create and publication share stable place-specific source
    /// exclusion. Deletion still keys by recipe across every provider/place.
    pub(crate) async fn lock_podman_use(
        &self,
        name: &str,
        place: crate::store::HostId,
    ) -> anyhow::Result<tokio::sync::OwnedMutexGuard<()>> {
        let guard = self
            .snapshot_mutex(&format!("\0podman:{place}:{name}"))
            .lock_owned()
            .await;
        anyhow::ensure!(
            !self.registry().deleting.contains(name),
            "the recipe is being deleted"
        );
        anyhow::ensure!(
            self.store.tensorlake_recipe_row(name).await?.is_some(),
            "the recipe no longer exists"
        );
        Ok(guard)
    }

    /// Deletion alone may acquire source exclusion after admission has closed.
    /// A caller cannot turn this into a new launch by omitting the guard.
    pub(crate) async fn lock_podman_cleanup(
        &self,
        name: &str,
        place: crate::store::HostId,
        deletion: &DeleteGuard,
    ) -> anyhow::Result<tokio::sync::OwnedMutexGuard<()>> {
        deletion.check(self, name)?;
        Ok(self
            .snapshot_mutex(&format!("\0podman:{place}:{name}"))
            .lock_owned()
            .await)
    }

    /// Source cleanup waits until each owned task's sender closes, including
    /// its failure cleanup. Commands already running settle before cancellation.
    pub(crate) async fn cancel_podman(
        &self,
        name: &str,
        deletion: &DeleteGuard,
    ) -> anyhow::Result<()> {
        deletion.check(self, name)?;
        let jobs: Vec<_> = self
            .registry()
            .podman_jobs
            .iter()
            .filter(|((recipe, _), _)| recipe == name)
            .map(|(_, job)| {
                job.cancel.store(true, std::sync::atomic::Ordering::SeqCst);
                job.progress.clone()
            })
            .collect();
        for mut job in jobs {
            while job.changed().await.is_ok() {}
        }
        Ok(())
    }

    /// Leftover deletion must not race a helper's live preparation owner.
    pub(crate) fn podman_job_active(&self, name: &str, place: crate::store::HostId) -> bool {
        self.registry()
            .podman_jobs
            .contains_key(&(name.to_owned(), place))
    }
    /// Inject the exact installer and version selected at helm assembly. Detached
    /// tests use an explicit unavailable installer; production has no late fallback.
    pub(crate) fn new(
        store: HelmStore,
        api: Arc<dyn TensorlakeApi>,
        shell: Arc<dyn SandboxShell>,
        ssh_keys: Option<Arc<SshKeyStore>>,
        timings: BuildTimings,
        preparer: Arc<dyn SnapshotPreparer>,
        helm_version: String,
    ) -> Self {
        Self {
            store,
            api,
            shell,
            ssh_keys,
            timings,
            preparer,
            helm_version,
            registry: Mutex::new(Registry::default()),
            #[cfg(test)]
            fail_builder_record: std::sync::atomic::AtomicBool::new(false),
            #[cfg(test)]
            fail_preparer_record: std::sync::atomic::AtomicBool::new(false),
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
        let preparing = self.registry().preparing.contains_key(name);
        let cleanup_pending = (live.is_none()
            && (state.builder_sandbox_id.is_some() || state.pending_create_at.is_some()))
            || (!preparing
                && (state.preparer_sandbox_id.is_some()
                    || state.prepare_pending_create_at.is_some()))
            || (live.is_none() && !preparing && !state.retired_snapshots.is_empty());
        let snapshot = state.snapshot.as_ref().map(|snapshot| SnapshotView {
            id: snapshot.id.clone(),
            built_at: snapshot.built_at,
            base: snapshot.base.clone(),
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
    /// Refuses a base that Tensorlake cannot build before resource work, a
    /// recipe that does not exist, and a helm
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
        if !spec.base.supports(farhelm_proto::ProviderKind::Tensorlake) {
            return Err(StartRefusal::UnsupportedBase);
        }
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
            base: spec.base,
            packages: spec.packages,
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

    /// Return a recipe's built snapshot, building it when absent
    /// and joining an already running rebuild. A host keeps its confirmed
    /// definition; edits made while confirmation was open cannot change the
    /// base, packages or size of a build it starts. Unsupported bases refuse
    /// even when an older Tensorlake snapshot remains recorded. Existing snapshots keep the
    /// recipe's explicit-rebuild semantics (base or package edits do not rebuild).
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
        anyhow::ensure!(
            frozen
                .base
                .supports(farhelm_proto::ProviderKind::Tensorlake),
            "this recipe's base cannot run on Tensorlake"
        );
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
                    StartRefusal::UnsupportedBase =>
                        "this recipe's base cannot run on Tensorlake".into(),
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
            base: ctx.base.clone(),
            disk_gb: ctx.size.disk_gb,
            packages: ctx.packages.clone(),
            claude_version: claude,
            codex_version: codex,
        };
        // One write records the new snapshot and retires the old one, so the
        // old one is deleted only after the new one is recorded. The snapshot's
        // frozen base and packages remain visible if the recipe changed during
        // the build; edits require another explicit rebuild.
        let publication = self.snapshot_mutex(&ctx.recipe).lock_owned().await;
        self.store
            .update_tensorlake_recipe_state(&ctx.recipe, move |_, state| {
                if let Some(old) = state.snapshot.take() {
                    state.retired_snapshots.push(old.id);
                }
                state.snapshot = Some(built);
                state.build_started_at = None;
                state.last_failure = None;
            })
            .await
            .map_err(|error| storage(BuildStep::Finishing, error))?;
        drop(publication);
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

    /// Step 1: clean recorded resources and recover an outstanding create.
    ///
    /// A same-name sandbox without a create record is left alone. The name
    /// must still become free before this build can create its own builder.
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
                    format!(
                        "a sandbox named {} still occupies the recipe's build name. Wait for it to finish or remove it in Tensorlake, then retry",
                        ctx.builder
                    ),
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
        let source = match &ctx.base {
            RecipeBase::ManagedUbuntu => SandboxSource::Image(TENSORLAKE_BASE_IMAGE.to_string()),
            RecipeBase::TensorlakeImage { image } => SandboxSource::Image(image.clone()),
            RecipeBase::TensorlakeSnapshot { snapshot_id } => {
                SandboxSource::Snapshot(snapshot_id.clone())
            }
            RecipeBase::ContainerImage { .. } => {
                return Err(failed(step, "this recipe's base cannot run on Tensorlake"));
            }
        };
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
            source,
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
    /// while its snapshots may remain. Only an outstanding pending-create
    /// marker then permits looking up the builder's name. A recorded ID
    /// authorizes only that ID, never a different sandbox now at the name;
    /// build starts alone authorize neither lookup nor adoption.
    ///
    /// A sandbox recovered from that request is recorded BEFORE it is
    /// terminated, because it then no longer answers to its name and only
    /// the recorded ID attributes its snapshots; the record stays until the
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
        if state.pending_create_at.is_none() {
            return Ok(());
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
        self.sweep_owned_sandbox(recipe, key, id, false).await
    }

    /// Both job roles sweep only their own recorded sandbox. The same source
    /// exclusion used by creates prevents a sweep from retiring a source mid-boot.
    async fn sweep_owned_sandbox(
        &self,
        recipe: &str,
        key: &ApiKey,
        id: &str,
        preparing: bool,
    ) -> Result<(), TensorlakeError> {
        self.sweep_owned_sandbox_with_limit(recipe, key, id, preparing, None)
            .await
    }

    /// Bound only resource discovery/deletion, never the ownership write that
    /// settles it. A timed-out resource phase keeps the old durable owner.
    async fn sweep_owned_sandbox_with_limit(
        &self,
        recipe: &str,
        key: &ApiKey,
        id: &str,
        preparing: bool,
        limit: Option<Duration>,
    ) -> Result<(), TensorlakeError> {
        let resources = async {
            let source = self.snapshot_mutex(recipe).lock_owned().await;
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
                    TensorlakeError::Transport(
                        "Farhelm could not read its recorded snapshots".into(),
                    )
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
            Ok(source)
        };
        let _source = match limit {
            Some(limit) => tokio::time::timeout(limit, resources).await.map_err(|_| {
                TensorlakeError::Transport("sandbox cleanup did not finish in time".into())
            })??,
            None => resources.await?,
        };
        let id = id.to_string();
        self.store
            .update_tensorlake_recipe_state(recipe, move |_, state| {
                if !preparing && state.builder_sandbox_id.as_deref() == Some(id.as_str()) {
                    state.builder_sandbox_id = None;
                }
                if preparing && state.preparer_sandbox_id.as_deref() == Some(id.as_str()) {
                    state.preparer_sandbox_id = None;
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
        let _source = self.snapshot_mutex(recipe).lock_owned().await;
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

    /// Delete the billed resources recorded for recipe `name`, for the
    /// recipe's deletion: cancel both job roles and wait outside source exclusion,
    /// reconcile their recorded owners and outstanding creates, then delete both
    /// current snapshots and retired snapshots. Each record
    /// is cleared as its deletion succeeds; on failure the rest stays, so a
    /// retry finishes the job.
    /// A missing ID and pending marker, including a legacy or power-loss
    /// gap, leave any same-name sandbox for explicit cleanup.
    ///
    /// The returned guard keeps builds of `name` from starting until it is
    /// dropped, so the caller can delete the row with nothing new recorded.
    #[cfg(test)]
    pub(crate) async fn clean_for_delete(
        self: &Arc<Self>,
        name: &str,
    ) -> Result<DeleteGuard, CleanupRefusal> {
        let guard = self.begin_delete(name)?;
        self.clean_tensorlake(name, &guard).await?;
        Ok(guard)
    }

    /// Admission is shared by every provider, before any cleanup observes work.
    pub(crate) fn begin_delete(
        self: &Arc<Self>,
        name: &str,
    ) -> Result<DeleteGuard, CleanupRefusal> {
        if !self.registry().deleting.insert(name.to_owned()) {
            return Err(CleanupRefusal::Deleting);
        }
        Ok(DeleteGuard {
            builds: Arc::clone(self),
            name: name.to_owned(),
        })
    }

    /// Tensorlake keeps its current cancellation/reconciliation semantics under
    /// admission held by the common catalog deletion operation.
    pub(crate) async fn clean_tensorlake(
        self: &Arc<Self>,
        name: &str,
        deletion: &DeleteGuard,
    ) -> Result<(), CleanupRefusal> {
        deletion
            .check(self, name)
            .map_err(CleanupRefusal::Storage)?;
        let tasks = {
            let registry = self.registry();
            let build = registry.active.get(name).and_then(|active| {
                active.cancel.send_replace(true);
                active.task.lock().expect("build task slot poisoned").take()
            });
            let prepare = registry.preparing.get(name).and_then(|active| {
                active.cancel.send_replace(true);
                active
                    .task
                    .lock()
                    .expect("preparation task slot poisoned")
                    .take()
            });
            (build, prepare)
        };
        for task in [tasks.0, tasks.1].into_iter().flatten() {
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
        // An interrupted start still has progress state to clear, even when
        // no create record authorizes looking up or deleting its name.
        if !state.tensorlake_needs_cleanup() {
            return Ok(());
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
        let publication = self.snapshot_mutex(name).lock_owned().await;
        self.store
            .update_tensorlake_recipe_state(name, |_, state| {
                if let Some(snapshot) = state.snapshot.take() {
                    state.retired_snapshots.push(snapshot.id);
                }
                if let Some(snapshot) = state.prepared.take() {
                    state.retired_snapshots.push(snapshot.id);
                }
            })
            .await
            .map_err(CleanupRefusal::Storage)?;
        drop(publication);
        self.reconcile_builder(name, &key, &builder_name(&tag, name))
            .await
            .map_err(|failure| match failure {
                ReconcileFailure::Storage(error) => CleanupRefusal::Storage(error),
                other => CleanupRefusal::Tensorlake(other.to_string()),
            })?;
        self.reconcile_preparer(name, &key, &preparer_name(&tag, name))
            .await
            .map_err(|error| CleanupRefusal::Tensorlake(super::shell::shown(&error.to_string())))?;
        self.delete_retired(name, &key).await.map_err(tensorlake)?;
        self.store
            .update_tensorlake_recipe_state(name, |_, state| {
                state.build_started_at = None;
                state.prepare_started_at = None;
            })
            .await
            .map_err(CleanupRefusal::Storage)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Lazy preparation: separate admission, shared snapshot ownership
// ---------------------------------------------------------------------------

/// Persisted failure survives a helm restart without losing the earlier snapshot.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct PrepareFailureRecord {
    pub(crate) step: PrepareStep,
    pub(crate) message: String,
    pub(crate) at: i64,
}

/// Live task outcome includes persistence failure because that failure itself
/// may be impossible to record in the recipe's durable failure history.
#[derive(Clone, Copy)]
struct PrepareProgress {
    step: PrepareStep,
    started_at: i64,
    /// A failed ownership write cannot always be persisted as last_failure.
    /// Carry that failure to live waiters rather than spinning lazy retries.
    persistence_failed: bool,
}

/// An owned preparation task; request cancellation only detaches its waiter.
struct ActivePreparation {
    progress: watch::Sender<PrepareProgress>,
    cancel: watch::Sender<bool>,
    task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

/// Frozen sizing and access for one preparation. Its source is selected later,
/// under exclusion, so admission cannot freeze an already-retired snapshot ID.
struct PrepareContext {
    recipe: String,
    sandbox: String,
    key: ApiKey,
    access: TensorlakeSshAccess,
    size: RecipeSize,
    events: Arc<FleetEvents>,
    progress: watch::Sender<PrepareProgress>,
    cancel: watch::Receiver<bool>,
    started_at: i64,
}

impl PrepareContext {
    /// Publish phase changes through the same invalidation feed as builds.
    fn enter(&self, step: PrepareStep) {
        self.progress.send_replace(PrepareProgress {
            step,
            started_at: self.started_at,
            persistence_failed: false,
        });
        self.events.bump();
    }
}

/// Cancellation delegates cleanup to the deleting caller; failure attempts a
/// bounded sweep and keeps any ownership that could not be settled.
enum PrepareStop {
    Cancelled,
    Failed(PrepareStep, String),
}

/// All incoming preparer/API messages pass through the existing bounded peer
/// text filter. In particular, an anyhow chain is never logged or displayed.
fn prepare_failed(step: PrepareStep, error: impl std::fmt::Display) -> PrepareStop {
    PrepareStop::Failed(step, super::shell::shown(&error.to_string()))
}

impl RecipeBuilds {
    /// Obtain stable per-recipe exclusion without lending the registry mutex
    /// across a wait. The unchecked form also serves deletion after admission.
    fn snapshot_mutex(&self, name: &str) -> Arc<tokio::sync::Mutex<()>> {
        Arc::clone(
            self.registry()
                .snapshot_use
                .entry(name.to_string())
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(()))),
        )
    }

    /// Protect selection and create through the clone's first Running observation.
    /// Acquire only after ensuring/joining snapshots; publication needs this lock.
    /// This is in-memory exclusion, not a crash-safe source dependency record.
    pub(crate) async fn lock_snapshot_use(
        &self,
        name: &str,
    ) -> anyhow::Result<tokio::sync::OwnedMutexGuard<()>> {
        let guard = self.snapshot_mutex(name).lock_owned().await;
        anyhow::ensure!(
            !self.registry().deleting.contains(name),
            "the recipe is being deleted"
        );
        anyhow::ensure!(
            self.store.tensorlake_recipe_row(name).await?.is_some(),
            "the recipe no longer exists"
        );
        Ok(guard)
    }

    /// Local-only reuse decision, also used while a consumer holds source exclusion.
    /// Failure history does not add an invalidation rule to the three cache facts.
    pub(crate) async fn fresh_prepared_snapshot(
        &self,
        name: &str,
    ) -> anyhow::Result<Option<PreparedSnapshot>> {
        anyhow::ensure!(
            !self.registry().deleting.contains(name),
            "the recipe is being deleted"
        );
        let (_, state) = self
            .store
            .tensorlake_recipe_row(name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("the recipe no longer exists"))?;
        let source = state.snapshot.as_ref().map(|snapshot| snapshot.id.as_str());
        Ok(state
            .prepared
            .as_ref()
            .filter(|snapshot| {
                snapshot
                    .stale_reason(now_secs(), &self.helm_version, source)
                    .is_none()
            })
            .cloned())
    }

    /// Project preparation independently of explicit build progress. An old record
    /// remains visible during renewal and after failure, with its real freshness.
    pub(crate) fn preparation_view(
        &self,
        name: &str,
        state: &RecipeState,
    ) -> (Option<PreparedSnapshotView>, PrepareView) {
        let live = self
            .registry()
            .preparing
            .get(name)
            .map(|active| *active.progress.borrow());
        let preparation = match (live, state.prepare_started_at, &state.last_prepare_failure) {
            (Some(p), _, _) => PrepareView::Preparing {
                step: p.step,
                started_at: p.started_at,
            },
            (None, Some(started_at), _) => PrepareView::Interrupted { started_at },
            (None, None, Some(f)) => PrepareView::Failed {
                step: f.step,
                message: f.message.clone(),
                at: f.at,
            },
            _ => PrepareView::Idle,
        };
        let source = state.snapshot.as_ref().map(|snapshot| snapshot.id.as_str());
        (
            state
                .prepared
                .as_ref()
                .map(|snapshot| snapshot.view(now_secs(), &self.helm_version, source)),
            preparation,
        )
    }

    /// Ensure a fresh installation checkpoint, joining one task per recipe.
    /// Rebuild remains separate: if it changes the source during installation,
    /// the completed checkpoint is recorded but renewed before returning to Add.
    pub(crate) async fn ensure_prepared_snapshot<F, Fut>(
        self: &Arc<Self>,
        name: &str,
        frozen: &StoredRecipe,
        events: Arc<FleetEvents>,
        progress: F,
    ) -> anyhow::Result<PreparedSnapshot>
    where
        F: FnMut(String) -> Fut,
        Fut: Future<Output = ()>,
    {
        self.ensure_prepared_snapshot_inner(name, frozen, events, progress, false)
            .await
    }

    /// Prepare an existing software snapshot without starting a billed build.
    /// Session launch can renew the installation checkpoint, but a missing
    /// software snapshot is a separate explicit Build operation in Settings.
    pub(crate) async fn ensure_prepared_snapshot_from_built<F, Fut>(
        self: &Arc<Self>,
        name: &str,
        frozen: &StoredRecipe,
        events: Arc<FleetEvents>,
        progress: F,
    ) -> anyhow::Result<PreparedSnapshot>
    where
        F: FnMut(String) -> Fut,
        Fut: Future<Output = ()>,
    {
        self.ensure_prepared_snapshot_inner(name, frozen, events, progress, true)
            .await
    }

    /// Share preparation admission and source-change handling between Add host
    /// and session launch. Only Add host may start a missing software build.
    async fn ensure_prepared_snapshot_inner<F, Fut>(
        self: &Arc<Self>,
        name: &str,
        frozen: &StoredRecipe,
        events: Arc<FleetEvents>,
        mut progress: F,
        built_only: bool,
    ) -> anyhow::Result<PreparedSnapshot>
    where
        F: FnMut(String) -> Fut,
        Fut: Future<Output = ()>,
    {
        anyhow::ensure!(
            frozen
                .base
                .supports(farhelm_proto::ProviderKind::Tensorlake),
            "this recipe's base cannot run on Tensorlake"
        );
        loop {
            if built_only {
                anyhow::ensure!(
                    self.store
                        .tensorlake_recipe_row(name)
                        .await?
                        .is_some_and(|(_, state)| state.snapshot.is_some()),
                    "this recipe has no built snapshot; build it in Settings → Tensorlake first"
                );
            } else {
                self.ensure_snapshot(name, frozen, Arc::clone(&events), &mut progress)
                    .await?;
            }
            if let Some(snapshot) = self.fresh_prepared_snapshot(name).await? {
                return Ok(snapshot);
            }
            let mut receiver = self
                .start_preparation(name, frozen, Arc::clone(&events))
                .await?;
            loop {
                let step = receiver.borrow_and_update().step;
                progress(format!(
                    "Preparing snapshot: {} (step {} of {})",
                    step.label(),
                    step.number(),
                    PrepareStep::COUNT
                ))
                .await;
                if receiver.changed().await.is_err() {
                    break;
                }
            }
            anyhow::ensure!(
                !receiver.borrow().persistence_failed,
                "Farhelm could not save preparation progress; run setup again"
            );
            let (_, state) = self
                .store
                .tensorlake_recipe_row(name)
                .await?
                .ok_or_else(|| anyhow::anyhow!("the recipe no longer exists"))?;
            if let Some(failure) = state.last_prepare_failure {
                anyhow::bail!("{}", failure.message);
            }
            // This watch describes the attempt we joined. The durable start
            // marker may already belong to a newer preparation another waiter
            // started after Rebuild; the next iteration must join that job.
            anyhow::ensure!(
                state.prepared.is_some(),
                "snapshot preparation ended without a snapshot; run setup again"
            );
            // Freshness is rechecked at the top, outside source exclusion. A
            // concurrent Rebuild may require another preparation, never a stale return.
        }
    }

    /// Admit preparation independently from Build/Rebuild, subscribing while the
    /// registry lock still protects the task so a fast completion cannot be missed.
    async fn start_preparation(
        self: &Arc<Self>,
        name: &str,
        frozen: &StoredRecipe,
        events: Arc<FleetEvents>,
    ) -> anyhow::Result<watch::Receiver<PrepareProgress>> {
        {
            let registry = self.registry();
            anyhow::ensure!(
                !registry.deleting.contains(name),
                "the recipe is being deleted"
            );
            if let Some(active) = registry.preparing.get(name) {
                return Ok(active.progress.subscribe());
            }
        }
        let key = self.api_key().await?.ok_or_else(|| {
            anyhow::anyhow!("save a Tensorlake API key in Settings → Tensorlake first")
        })?;
        let access = self.ssh_access().await.map_err(anyhow::Error::msg)?;
        let tag = helm_tag(&self.store.registry_namespace().await?);
        let mut registry = self.registry();
        anyhow::ensure!(
            !registry.deleting.contains(name),
            "the recipe is being deleted"
        );
        if let Some(active) = registry.preparing.get(name) {
            return Ok(active.progress.subscribe());
        }
        let started_at = now_secs();
        let (progress, receiver) = watch::channel(PrepareProgress {
            step: PrepareStep::Creating,
            started_at,
            persistence_failed: false,
        });
        let (cancel, cancel_rx) = watch::channel(false);
        let active = Arc::new(ActivePreparation {
            progress: progress.clone(),
            cancel,
            task: Mutex::new(None),
        });
        let ctx = PrepareContext {
            recipe: name.to_string(),
            sandbox: preparer_name(&tag, name),
            key,
            access,
            size: frozen.size,
            events,
            progress,
            cancel: cancel_rx,
            started_at,
        };
        let engine = Arc::clone(self);
        let mine = Arc::clone(&active);
        let task = tokio::spawn(async move {
            engine.run_preparation(&ctx).await;
            let mut registry = engine.registry();
            if registry
                .preparing
                .get(&ctx.recipe)
                .is_some_and(|current| Arc::ptr_eq(current, &mine))
            {
                registry.preparing.remove(&ctx.recipe);
            }
            drop(registry);
            ctx.events.bump();
        });
        *active.task.lock().expect("preparation task slot poisoned") = Some(task);
        registry.preparing.insert(name.to_string(), active);
        Ok(receiver)
    }

    /// Cancel only remote work and waits. Create plus its ownership writes are
    /// awaited unconditionally so recipe deletion never loses a billed sandbox.
    async fn prepare_remote<T>(
        &self,
        ctx: &PrepareContext,
        future: impl Future<Output = T>,
    ) -> Result<T, PrepareStop> {
        let mut cancel = ctx.cancel.clone();
        tokio::select! { biased;
            _ = cancel.wait_for(|cancelled| *cancelled) => Err(PrepareStop::Cancelled),
            output = future => Ok(output),
        }
    }

    /// Record lifecycle boundaries even on failure; interrupted ownership is
    /// reconciled lazily by the next preparation or explicit recipe deletion.
    async fn run_preparation(&self, ctx: &PrepareContext) {
        let at = ctx.started_at;
        if !matches!(
            self.store
                .update_tensorlake_recipe_state(&ctx.recipe, move |_, state| {
                    state.prepare_started_at = Some(at);
                    state.last_prepare_failure = None;
                })
                .await,
            Ok(Some(()))
        ) {
            ctx.progress
                .send_modify(|progress| progress.persistence_failed = true);
            return;
        }
        ctx.events.bump();
        let mut sandbox = None;
        let failure = match self.prepare_steps(ctx, &mut sandbox).await {
            Ok(()) => return,
            Err(PrepareStop::Cancelled) => None,
            Err(PrepareStop::Failed(step, message)) => {
                if let Some(id) = sandbox {
                    // A failed best-effort sweep retains the durable owner.
                    let _ = self
                        .sweep_owned_sandbox_with_limit(
                            &ctx.recipe,
                            &ctx.key,
                            &id,
                            true,
                            Some(self.timings.failure_sweep),
                        )
                        .await;
                }
                Some(PrepareFailureRecord {
                    step,
                    message,
                    at: now_secs(),
                })
            }
        };
        if self
            .store
            .update_tensorlake_recipe_state(&ctx.recipe, move |_, state| {
                state.prepare_started_at = None;
                if failure.is_some() {
                    state.last_prepare_failure = failure;
                }
            })
            .await
            .is_err()
        {
            ctx.progress
                .send_modify(|progress| progress.persistence_failed = true);
        }
    }

    /// Installation never holds source exclusion. Only selection/create/Running
    /// and publication need to serialize with source retirement.
    async fn prepare_steps(
        &self,
        ctx: &PrepareContext,
        sandbox: &mut Option<String>,
    ) -> Result<(), PrepareStop> {
        let creating = PrepareStep::Creating;
        // These helpers include durable ownership writes. Await them whole:
        // cancellation must never drop a record after an observed vendor change.
        self.reconcile_preparer(&ctx.recipe, &ctx.key, &ctx.sandbox)
            .await
            .map_err(|error| prepare_failed(creating, error))?;
        let _ = self.delete_retired(&ctx.recipe, &ctx.key).await;
        let deadline = tokio::time::Instant::now() + self.timings.name_free;
        loop {
            match self
                .prepare_remote(ctx, self.api.get_sandbox(&ctx.key, &ctx.sandbox))
                .await?
            {
                Err(TensorlakeError::NotFound { .. }) => break,
                Ok(found) if found.status == SandboxStatus::Terminated => break,
                Ok(_) if tokio::time::Instant::now() < deadline => {}
                Ok(_) => {
                    return Err(prepare_failed(
                        creating,
                        "a sandbox still occupies the recipe's preparation name; remove it in Tensorlake or retry later",
                    ));
                }
                Err(error) => return Err(prepare_failed(creating, error)),
            }
            self.prepare_remote(ctx, tokio::time::sleep(self.timings.poll))
                .await?;
        }
        let source_guard = self
            .prepare_remote(ctx, self.snapshot_mutex(&ctx.recipe).lock_owned())
            .await?;
        // Another waiter may have completed preparation between the caller's
        // freshness read and admission. Avoid a second billed create.
        let state = self
            .store
            .tensorlake_recipe_row(&ctx.recipe)
            .await
            .map_err(|_| prepare_failed(creating, "Farhelm could not read preparation progress"))?
            .ok_or_else(|| prepare_failed(creating, "the recipe was deleted"))?
            .1;
        let source = state
            .snapshot
            .ok_or_else(|| prepare_failed(creating, "the recipe has no built snapshot"))?;
        if state.prepared.as_ref().is_some_and(|prepared| {
            prepared
                .stale_reason(now_secs(), &self.helm_version, Some(&source.id))
                .is_none()
        }) {
            let _ = self
                .store
                .update_tensorlake_recipe_state(&ctx.recipe, |_, state| {
                    state.prepare_started_at = None;
                })
                .await;
            return Ok(());
        }
        let id = self.create_preparer(ctx, &source, sandbox).await?;
        ctx.enter(PrepareStep::Starting);
        let deadline = tokio::time::Instant::now() + self.timings.start;
        loop {
            let info = self
                .prepare_remote(ctx, self.api.get_sandbox(&ctx.key, &id))
                .await?
                .map_err(|error| prepare_failed(PrepareStep::Starting, error))?;
            match info.status {
                SandboxStatus::Running => break,
                SandboxStatus::Terminated => {
                    return Err(prepare_failed(
                        PrepareStep::Starting,
                        "Tensorlake ended the preparation sandbox",
                    ));
                }
                _ => {}
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(prepare_failed(
                    PrepareStep::Starting,
                    "the preparation sandbox did not start in time",
                ));
            }
            self.prepare_remote(ctx, tokio::time::sleep(self.timings.poll))
                .await?;
        }
        drop(source_guard);
        ctx.enter(PrepareStep::Installing);
        let facts = self
            .prepare_remote(ctx, self.preparer.prepare(&ctx.sandbox, &ctx.access))
            .await?
            .map_err(|error| prepare_failed(PrepareStep::Installing, error))?;
        if facts.farhelm_version != self.helm_version
            || !facts.home.is_absolute()
            || !facts.farhelm_path.is_absolute()
            || !facts.state_dir.is_absolute()
        {
            return Err(prepare_failed(
                PrepareStep::Installing,
                "the preparer returned incompatible installation coordinates or version",
            ));
        }
        ctx.enter(PrepareStep::Snapshotting);
        let snapshot = self
            .prepare_remote(
                ctx,
                self.api
                    .create_snapshot(&ctx.key, &id, SnapshotType::Filesystem),
            )
            .await?
            .map_err(|error| prepare_failed(PrepareStep::Snapshotting, error))?
            .snapshot_id;
        let deadline = tokio::time::Instant::now() + self.timings.snapshot;
        loop {
            let info = self
                .prepare_remote(ctx, self.api.get_snapshot(&ctx.key, &snapshot))
                .await?
                .map_err(|error| prepare_failed(PrepareStep::Snapshotting, error))?;
            match info.status {
                SnapshotStatus::Completed => break,
                SnapshotStatus::Failed => {
                    return Err(prepare_failed(
                        PrepareStep::Snapshotting,
                        "Tensorlake could not complete the prepared snapshot",
                    ));
                }
                _ => {}
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(prepare_failed(
                    PrepareStep::Snapshotting,
                    "the prepared snapshot did not complete in time",
                ));
            }
            self.prepare_remote(ctx, tokio::time::sleep(self.timings.poll))
                .await?;
        }
        ctx.enter(PrepareStep::Finishing);
        let prepared = PreparedSnapshot {
            id: snapshot,
            prepared_at: now_secs(),
            farhelm_version: self.helm_version.clone(),
            source_snapshot: source.id,
            disk_gb: source.disk_gb,
            facts,
        };
        let publication = self
            .prepare_remote(ctx, self.snapshot_mutex(&ctx.recipe).lock_owned())
            .await?;
        self.store
            .update_tensorlake_recipe_state(&ctx.recipe, move |_, state| {
                if let Some(old) = state.prepared.replace(prepared) {
                    state.retired_snapshots.push(old.id);
                }
                state.prepare_started_at = None;
                state.last_prepare_failure = None;
            })
            .await
            .map_err(|_| {
                prepare_failed(
                    PrepareStep::Finishing,
                    "Farhelm could not save the prepared snapshot",
                )
            })?
            .ok_or_else(|| prepare_failed(PrepareStep::Finishing, "the recipe was deleted"))?;
        drop(publication);
        let _ = self
            .sweep_owned_sandbox(&ctx.recipe, &ctx.key, &id, true)
            .await;
        let _ = self.delete_retired(&ctx.recipe, &ctx.key).await;
        Ok(())
    }

    /// The pending marker and observed ID writes bracket an uncancellable create.
    /// Only that outstanding marker authorizes later recovery by deterministic name.
    async fn create_preparer(
        &self,
        ctx: &PrepareContext,
        source: &BuiltSnapshot,
        sandbox: &mut Option<String>,
    ) -> Result<String, PrepareStop> {
        let step = PrepareStep::Creating;
        let sent = now_secs();
        self.store
            .update_tensorlake_recipe_state(&ctx.recipe, move |_, state| {
                state.prepare_pending_create_at = Some(sent);
            })
            .await
            .map_err(|_| prepare_failed(step, "Farhelm could not record the preparation create"))?
            .ok_or_else(|| prepare_failed(step, "the recipe was deleted"))?;
        let request = CreateSandbox {
            name: ctx.sandbox.clone(),
            source: SandboxSource::Snapshot(source.id.clone()),
            resources: Resources {
                cpus: f64::from(ctx.size.cpus),
                memory_mb: u64::from(ctx.size.memory_gb) * 1024,
                disk_mb: u64::from(source.disk_gb) * 1024,
            },
            timeout_secs: BUILDER_IDLE_TIMEOUT_SECS,
            auto_resume: false,
            max_pending_secs: 900,
        };
        let id = match self.api.create_sandbox(&ctx.key, &request).await {
            Ok(created) => created.sandbox_id,
            Err(error) if create_was_refused(&error) => {
                let _ = self
                    .store
                    .update_tensorlake_recipe_state(&ctx.recipe, |_, state| {
                        state.prepare_pending_create_at = None;
                    })
                    .await;
                return Err(prepare_failed(step, error));
            }
            Err(error) => match self.api.get_sandbox(&ctx.key, &ctx.sandbox).await {
                Ok(found) => found.id,
                _ => return Err(prepare_failed(step, error)),
            },
        };
        self.record_preparer(&ctx.recipe, &id).await.map_err(|_| {
            prepare_failed(step, "Farhelm could not record the preparation sandbox")
        })?;
        *sandbox = Some(id.clone());
        Ok(id)
    }

    /// Recording an observed owner settles the pending request in the same write.
    async fn record_preparer(&self, recipe: &str, id: &str) -> anyhow::Result<()> {
        #[cfg(test)]
        if self
            .fail_preparer_record
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            anyhow::bail!("injected failure recording the preparer");
        }
        let id = id.to_string();
        self.store
            .update_tensorlake_recipe_state(recipe, move |_, state| {
                state.preparer_sandbox_id = Some(id);
                state.prepare_pending_create_at = None;
            })
            .await?
            .ok_or_else(|| anyhow::anyhow!("the recipe was deleted"))
    }

    /// Reconcile only a recorded ID or outstanding create; a preparation start
    /// alone is never evidence that a same-name sandbox belongs to this helm.
    async fn reconcile_preparer(
        &self,
        recipe: &str,
        key: &ApiKey,
        name: &str,
    ) -> anyhow::Result<()> {
        let state = self
            .store
            .tensorlake_recipe_row(recipe)
            .await?
            .ok_or_else(|| anyhow::anyhow!("the recipe was deleted"))?
            .1;
        if let Some(id) = state.preparer_sandbox_id {
            self.sweep_owned_sandbox(recipe, key, &id, true).await?;
        }
        if let Some(sent) = state.prepare_pending_create_at {
            match self.api.get_sandbox(key, name).await {
                Ok(found) => {
                    self.record_preparer(recipe, &found.id).await?;
                    self.sweep_owned_sandbox(recipe, key, &found.id, true)
                        .await?;
                }
                Err(TensorlakeError::NotFound { .. })
                    if now_secs().saturating_sub(sent) >= CREATE_SETTLE_SECS =>
                {
                    self.store
                        .update_tensorlake_recipe_state(recipe, |_, state| {
                            state.prepare_pending_create_at = None;
                        })
                        .await?;
                }
                Err(TensorlakeError::NotFound { .. }) => anyhow::bail!(
                    "a preparation sandbox Farhelm asked Tensorlake for may still be starting; try again in a few minutes"
                ),
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
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

/// Holds off both snapshot jobs for a recipe being deleted; see
/// [`RecipeBuilds::clean_for_delete`].
pub(crate) struct DeleteGuard {
    builds: Arc<RecipeBuilds>,
    name: String,
}

impl DeleteGuard {
    /// A guard from another registry or recipe grants no cleanup authority.
    pub(crate) fn check(&self, builds: &RecipeBuilds, name: &str) -> anyhow::Result<()> {
        anyhow::ensure!(
            std::ptr::eq(self.builds.as_ref(), builds) && self.name == name,
            "recipe cleanup requires this recipe's shared deletion admission"
        );
        Ok(())
    }
}

impl Drop for DeleteGuard {
    fn drop(&mut self) {
        self.builds.registry().deleting.remove(&self.name);
    }
}

#[cfg(test)]
#[path = "build_tests.rs"]
mod tests;
