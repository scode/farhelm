//! Per-host credential convergence on the ordinary provisioning transport.
//!
//! A connection registers before reading credentials and keeps its lease until
//! it ends. Replacement therefore includes a hook that has read an old value
//! but has not published Connected yet. Only nonsecret bindings are retained;
//! pending recipe edits wait for the next setup or connection.

use super::backend::{COMMAND_TIMEOUT, ProvisioningBackend};
use super::plan::{DirectorySpec, ProvisioningTarget};
use crate::feed::FleetEvents;
use crate::store::{HelmStore, HostId, HostRow};
use crate::tensorlake::recipes::{ResolvedEndpoint, StoredEndpoint, StoredRecipe};
use farhelm_proto::credentials::DeliveryFailure;
use farhelm_supervisor::recipe_credentials::{self, RecipeCredentials};
use futures_util::{StreamExt, stream};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{OwnedMutexGuard, watch};
use tokio::time::{Instant, timeout_at};

/// Bound concurrent SSH work without restarting the save's shared deadline for
/// hosts waiting for a slot. Provisioning uses the same four-host work budget.
const PUSH_CONCURRENCY: usize = 4;
const STORAGE: &str = "could not read or record the host's credential delivery state";
const TIMED_OUT: &str = "credential delivery did not finish within the SSH timeout";
const CODEX_LOGIN_LOOKUP: &str = "could not resolve the recipe's Codex login";

/// The reason a replacement's reply gives a host still being delivered to
/// when the reply was due (see [`CredentialDelivery::push`]). It says where
/// the late outcome shows, as the other bounded routes' timeout replies do.
fn still_delivering() -> String {
    format!(
        "the host did not answer within {} seconds; delivery is still being carried out, and \
         the host's details will show a warning if it fails",
        crate::sessions::HOST_REPLY_BOUND.as_secs()
    )
}

/// An active connection's choices, rather than a copy of the whole recipe.
#[derive(Clone, Deserialize)]
pub(crate) struct Bindings {
    claude: Option<StoredEndpoint>,
    codex: Option<StoredEndpoint>,
    claude_login_credential: Option<String>,
    codex_login_credential: Option<String>,
    github_token_credential: Option<String>,
}

#[cfg(test)]
#[path = "credential_delivery_tests.rs"]
mod tests;

impl Bindings {
    /// Setup's confirmed recipe stays frozen, while its named values stay live.
    pub(super) fn from_recipe(recipe: &StoredRecipe) -> Self {
        Self {
            claude: recipe.claude.clone(),
            codex: recipe.codex.clone(),
            claude_login_credential: recipe.claude_login_credential.clone(),
            codex_login_credential: recipe.codex_login_credential.clone(),
            github_token_credential: recipe.github_token_credential.clone(),
        }
    }

    /// A replacement must not activate another recipe's pending selection edit.
    fn uses(&self, name: &str) -> bool {
        self.github_token_credential.as_deref() == Some(name)
            || self.uses_claude(name)
            || self.uses_codex_login(name)
            || self.uses_codex_endpoint(name)
    }

    /// A Codex endpoint cleanup warning belongs to its endpoint token, not a Subscription seed.
    fn uses_codex_endpoint(&self, name: &str) -> bool {
        self.codex
            .as_ref()
            .and_then(|endpoint| endpoint.token_credential.as_deref())
            == Some(name)
    }

    /// Subscription login writes the separate auth.json file.
    fn uses_codex_login(&self, name: &str) -> bool {
        self.codex_login_credential.as_deref() == Some(name)
    }

    /// Pending Claude cleanup affects a push only if Claude needs that value.
    fn uses_claude(&self, name: &str) -> bool {
        self.claude_login_credential.as_deref() == Some(name)
            || self
                .claude
                .as_ref()
                .and_then(|endpoint| endpoint.token_credential.as_deref())
                == Some(name)
    }
}

/// Explicit modes distinguish completed cleanup from older no-work records.
/// A stamp identifies the credential revision without retaining its value.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
enum EndpointStamp {
    Absent,
    Cleared,
    Endpoint {
        base_url: String,
        credential: Option<(String, u64, String)>,
    },
    Subscription {
        name: String,
        revision: u64,
        incarnation: String,
    },
}

/// A successful token stamp also attests that gh's helper was configured.
/// A missing gh leaves this unknown/previous, so the next named event retries
/// setup while the already-installed token remains available to new children.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
enum GithubStamp {
    Absent,
    Token {
        name: String,
        revision: u64,
        incarnation: String,
    },
}

/// Only successful slots are durable. Outer None is unknown, so a partial
/// failure retries unfinished work without forgetting another slot's success.
#[derive(Default, Deserialize, Serialize)]
struct Record {
    home: Option<PathBuf>,
    /// Last legacy settings stamp. A cleared stamp settles migration for
    /// this host, including hosts with no Farhelm-authored settings to clean.
    claude: Option<EndpointStamp>,
    /// The installed launch environment's own stamp. A failed upload can
    /// retry without reopening settled legacy settings.
    claude_environment: Option<EndpointStamp>,
    /// Legacy settings ownership remains separate from successful endpoint file delivery.
    codex: Option<EndpointStamp>,
    codex_environment: Option<EndpointStamp>,
    /// Codex refreshes this file itself. Leaving subscription mode must retain
    /// its last successful seed, or returning would overwrite refreshed bytes.
    codex_auth: Option<(String, u64, String)>,
    github: Option<GithubStamp>,
}

/// Keep each warning with its agent. A lookup failure holds the whole launch credentials file,
/// while a pending cleanup holds only the entry that depends on it.
#[derive(Debug, Default)]
struct DeliveryWarnings {
    /// A launch lookup failed, so the pass left the combined file, both launch stamps and both legacy settings
    /// files as they were.
    lookup_aborted: bool,
    /// Claude's reason: its failed launch lookup when `lookup_aborted`, otherwise its pending legacy cleanup.
    claude: Option<String>,
    /// Codex's reason, read the same way as `claude`.
    codex: Option<String>,
    /// The separate Codex login lookup failed too. Only recorded when `lookup_aborted`; otherwise that failure ends
    /// the pass as a hard error.
    codex_login_lookup_failed: bool,
}

/// A hard failure still ends the pass. Keep preceding lookup warnings separate
/// so a save reply can include only the ones relevant to its delivery.
#[derive(Debug)]
struct DeliveryError {
    reason: String,
    lookup_warnings: Option<DeliveryWarnings>,
}

impl From<String> for DeliveryError {
    fn from(reason: String) -> Self {
        Self {
            reason,
            lookup_warnings: None,
        }
    }
}

impl DeliveryError {
    /// Host details show both the failed lookup and the failure that stopped the pass.
    fn message(&self) -> String {
        self.with_warning(
            self.lookup_warnings
                .as_ref()
                .and_then(DeliveryWarnings::message),
        )
    }

    /// Keep the hard failure in every reply, with any earlier warnings selected by that reply.
    fn with_warning(&self, earlier: Option<String>) -> String {
        match earlier {
            Some(earlier) => format!("{earlier}. {}", self.reason),
            None => self.reason.clone(),
        }
    }
}

impl DeliveryWarnings {
    /// Host details and setup errors can report all affected slots from one pass.
    fn message(&self) -> Option<String> {
        self.message_for(true, true, true)
    }

    /// The host and save reply share wording, but a save selects only the slots it needed.
    fn message_for(&self, claude: bool, codex: bool, codex_login: bool) -> Option<String> {
        let mut reasons = [
            self.claude.as_deref().filter(|_| claude),
            self.codex.as_deref().filter(|_| codex),
            (codex_login && self.codex_login_lookup_failed).then_some(CODEX_LOGIN_LOOKUP),
        ]
        .into_iter()
        .flatten();
        let mut message = reasons.next()?.to_owned();
        for reason in reasons {
            message.push_str(". ");
            message.push_str(reason);
        }
        Some(message)
    }

    /// A push for either entry depends on publication of the whole file if a lookup fails.
    /// A Codex login push depends only on its own warning, and a GitHub push depends on its own slot.
    fn push_failure(
        &self,
        uses_claude: bool,
        uses_codex: bool,
        uses_codex_login: bool,
    ) -> Option<String> {
        let shared_blocked = self.lookup_aborted && (uses_claude || uses_codex);
        self.message_for(
            shared_blocked || uses_claude,
            shared_blocked || uses_codex,
            uses_codex_login,
        )
    }
}

/// A tokenless Codex endpoint still needs its descriptor. Only a snapshot with neither agent has no file.
fn encode_agent_environment(credentials: &RecipeCredentials) -> Result<Option<Vec<u8>>, String> {
    if credentials.env.is_empty() && credentials.codex_endpoint.is_none() {
        return Ok(None);
    }
    let bytes = serde_json::to_vec(credentials)
        .map_err(|_| "could not encode the managed launch credentials")?;
    if bytes.len() > recipe_credentials::MAX_FILE_BYTES {
        return Err("the managed launch credentials are too large".into());
    }
    Ok(Some(bytes))
}

/// Setup can install the old file shape before the supervisor answers. Codex migration needs a successful
/// hello so a supervisor that rejects the expanded file never receives it.
#[derive(Clone, Copy)]
enum DeliveryPhase {
    Setup,
    Connected,
}

/// The settings path and ownership stamp have to agree before cleanup touches a user file.
#[derive(Clone, Copy)]
enum LegacySettings {
    Claude,
    Codex,
}

/// Keep a failed legacy cleanup attributable to the binding that needs it.
fn cleanup_pending(agent: &str, reason: &str) -> String {
    format!("{agent} recipe credential cleanup is pending ({reason})")
}

impl LegacySettings {
    fn stamp(self, record: &mut Record) -> &mut Option<EndpointStamp> {
        match self {
            Self::Claude => &mut record.claude,
            Self::Codex => &mut record.codex,
        }
    }
}

/// Secret-bearing values exist only while a serialized operation uses them.
struct Slot {
    endpoint: Option<ResolvedEndpoint>,
    /// Claude's setup-token supplies the environment in Subscription mode.
    subscription: Option<String>,
    stamp: EndpointStamp,
}

/// As with endpoints, raw token bytes exist only inside one guarded delivery.
struct GithubSlot {
    token: Option<String>,
    stamp: GithubStamp,
}

/// Runtime selections and warnings belong to one registered host identity.
/// Secret bytes and delivery successes live elsewhere, so reconnecting cannot
/// accidentally reuse a cached value or mistake a warning for durable state.
#[derive(Default)]
struct Live {
    binding: Option<(HostRow, Bindings)>,
    warning: Option<String>,
    /// A timed-out older operation cannot replace a newer attempt's warning.
    attempt: u64,
}

/// One serialization boundary shared by every connection incarnation. Counted
/// leases allow an older incarnation to retire without cancelling its successor.
struct Entry {
    entry_uuid: String,
    serial: Arc<tokio::sync::Mutex<()>>,
    active: watch::Sender<LeaseState>,
    live: Mutex<Live>,
}

/// A connected lifetime runs from its first lease to its last. The generation
/// distinguishes lifetimes even if the receiver misses the zero count.
#[derive(Clone, Copy, Default)]
struct LeaseState {
    count: usize,
    generation: u64,
}

impl LeaseState {
    /// Overlapping leases share one lifetime. A new lifetime cannot
    /// authorize work that belonged to the previous one.
    fn is_current(self, generation: u64) -> bool {
        self.count != 0 && self.generation == generation
    }
}

impl Entry {
    /// Identity changes allocate another entry; an old lease cannot end it.
    fn new(row: &HostRow) -> Self {
        Self {
            entry_uuid: row.entry_uuid.clone(),
            serial: Arc::default(),
            active: watch::channel(LeaseState::default()).0,
            live: Mutex::default(),
        }
    }

    /// Warning ordering is local to attempts, independent of durable records.
    fn begin(&self) -> u64 {
        let mut live = self.live.lock().unwrap();
        Self::next_attempt(&mut live)
    }

    /// Check a queued candidate again before superseding any warning. A new
    /// connection may have cleared its selections while this host waited for a
    /// push slot; only that connection can report its still-unknown delivery.
    fn begin_push(&self, name: &str, generation: u64) -> Option<u64> {
        let mut live = self.live.lock().unwrap();
        if !self.active.borrow().is_current(generation)
            || !live
                .binding
                .as_ref()
                .is_some_and(|(_, binding)| binding.uses(name))
        {
            return None;
        }
        Some(Self::next_attempt(&mut live))
    }

    /// Allocate under the same mutex as the selection check. Clearing a
    /// binding for a new connection must happen before its newer attempt starts.
    fn next_attempt(live: &mut Live) -> u64 {
        live.attempt = live
            .attempt
            .checked_add(1)
            .expect("delivery attempt exhausted");
        live.attempt
    }

    /// Publish only the newest attempt's result and invalidate the host feed
    /// when its visible warning actually changes.
    fn warning(&self, attempt: u64, reason: Option<String>, events: &FleetEvents) {
        let mut live = self.live.lock().unwrap();
        if live.attempt == attempt && live.warning != reason {
            live.warning = reason;
            events.bump();
        }
    }
}

/// Owned by the actor through acquisition, Connected publication and serving.
/// Dropping it synchronously cancels pushes before their next backend poll.
pub(crate) struct ConnectionLease(Arc<Entry>);

impl Drop for ConnectionLease {
    fn drop(&mut self) {
        self.0
            .active
            .send_modify(|state| state.count = state.count.saturating_sub(1));
    }
}

/// Shared by setup and the Tensorlake connect hook, with no manager dependency.
/// Detached settings-only test services deliberately have no remote backend.
pub(crate) struct CredentialDelivery {
    store: HelmStore,
    places: Arc<crate::environment::podman_probe::PodmanPlaces>,
    backend: Option<Arc<dyn ProvisioningBackend>>,
    entries: Mutex<HashMap<HostId, Arc<Entry>>>,
    budget: Duration,
}

impl CredentialDelivery {
    /// Bind delivery to the same already-selected backend as host setup. The
    /// manager must receive this object before its first actor can connect.
    #[cfg(test)]
    pub(super) fn new(store: HelmStore, backend: Arc<dyn ProvisioningBackend>) -> Arc<Self> {
        Self::with_places(
            store,
            backend,
            crate::environment::podman_probe::PodmanPlaces::new(Path::new("")),
        )
    }

    /// Production supplies the single cache before actors or setup can use
    /// it; delivery must resolve the same executable observation as dialing.
    pub(super) fn with_places(
        store: HelmStore,
        backend: Arc<dyn ProvisioningBackend>,
        places: Arc<crate::environment::podman_probe::PodmanPlaces>,
    ) -> Arc<Self> {
        Arc::new(Self {
            store,
            places,
            backend: Some(backend),
            entries: Mutex::default(),
            budget: COMMAND_TIMEOUT,
        })
    }

    /// No environment selection or production fallback belongs in a test seam.
    #[cfg(test)]
    pub(crate) fn detached(store: HelmStore) -> Arc<Self> {
        Arc::new(Self {
            store,
            places: crate::environment::podman_probe::PodmanPlaces::new(Path::new("")),
            backend: None,
            entries: Mutex::default(),
            budget: COMMAND_TIMEOUT,
        })
    }

    /// Settings, actor transport and installation share this exact cache.
    pub(crate) fn places(&self) -> Arc<crate::environment::podman_probe::PodmanPlaces> {
        Arc::clone(&self.places)
    }

    /// Provider readiness uses the transport selected for this same delivery
    /// service. Detached settings-only fixtures deliberately have none.
    pub(crate) fn backend(&self) -> Option<Arc<dyn ProvisioningBackend>> {
        self.backend.clone()
    }

    /// Numeric IDs alone cannot authorize work after a host entry is replaced.
    /// A new identity gets a separate lease/lock entry; guarded writes still
    /// check the persisted identity before accepting any success record.
    fn entry(&self, row: &HostRow) -> Arc<Entry> {
        let mut entries = self.entries.lock().unwrap();
        let entry = entries
            .entry(row.id)
            .or_insert_with(|| Arc::new(Entry::new(row)));
        if entry.entry_uuid != row.entry_uuid {
            *entry = Arc::new(Entry::new(row));
        }
        Arc::clone(entry)
    }

    /// Registration precedes every await in the connect hook. Clearing a prior
    /// connection's binding here prevents it authorizing a new connection's push.
    pub(crate) fn enter(&self, row: &HostRow) -> ConnectionLease {
        let entry = self.entry(row);
        {
            let mut live = entry.live.lock().unwrap();
            entry.active.send_modify(|state| {
                if state.count == 0 {
                    live.binding = None;
                    state.generation = state
                        .generation
                        .checked_add(1)
                        .expect("lease generation exhausted");
                }
                state.count += 1;
            });
        }
        ConnectionLease(entry)
    }

    /// Host presentation is independent of routability: a usable supervisor
    /// can still have a failed credential delivery that the user needs to see.
    pub(crate) fn warning(&self, host: HostId) -> Option<String> {
        self.entries
            .lock()
            .unwrap()
            .get(&host)
            .and_then(|entry| entry.live.lock().unwrap().warning.clone())
    }

    /// Host removal bounds retained warning/lock state and cancels any push
    /// still holding the removed entry. Its later lease Drop is harmless.
    pub(super) fn forget(&self, host: HostId) {
        if let Some(entry) = self.entries.lock().unwrap().remove(&host) {
            entry.active.send_modify(|state| state.count = 0);
        }
    }

    /// Persist preparation's home without rerunning user-manager setup later.
    pub(super) async fn remember_home(&self, row: &HostRow, home: PathBuf) -> Result<(), String> {
        let entry = self.entry(row);
        let guard = Arc::clone(&entry.serial).lock_owned().await;
        let mut record = self.read_record(row).await?;
        record.home = Some(home);
        self.write_record(row, &record, guard).await?;
        Ok(())
    }

    /// Delivery failures remain warnings; they never refuse a usable connection.
    pub(crate) async fn connect(&self, row: &HostRow, events: &FleetEvents) {
        let entry = self.entry(row);
        let attempt = entry.begin();
        let result = timeout_at(Instant::now() + self.budget, async {
            let guard = Arc::clone(&entry.serial).lock_owned().await;
            let bindings = self.read_bindings(row).await?;
            entry.live.lock().unwrap().binding = Some((row.clone(), bindings.clone()));
            self.apply(row, bindings, None, DeliveryPhase::Connected, guard)
                .await
                .map(|warnings| warnings.message())
                .map_err(|error| error.message())
        })
        .await
        .unwrap_or_else(|_| Err(TIMED_OUT.into()));
        entry.warning(attempt, result.unwrap_or_else(Some), events);
    }

    /// Use the confirmed setup choices, after install coordinates are admitted.
    /// The next connection independently refreshes the recipe selection.
    pub(super) async fn setup(
        &self,
        row: &HostRow,
        recipe: &StoredRecipe,
        home: &Path,
        events: &FleetEvents,
    ) -> Result<(), String> {
        let entry = self.entry(row);
        let attempt = entry.begin();
        let result = timeout_at(Instant::now() + self.budget, async {
            let guard = Arc::clone(&entry.serial).lock_owned().await;
            self.apply(
                row,
                Bindings::from_recipe(recipe),
                Some(home.to_path_buf()),
                DeliveryPhase::Setup,
                guard,
            )
            .await
            .map_err(|error| error.message())?
            .message()
            .map_or(Ok(()), Err)
        })
        .await
        .unwrap_or_else(|_| Err(TIMED_OUT.into()));
        entry.warning(attempt, result.clone().err(), events);
        result
    }

    /// Deliver a committed replacement to every active connection using it,
    /// and report by `reply_by` the hosts it has not reached.
    ///
    /// One deadline, the SSH command timeout from now, covers all candidates,
    /// lock waits and transport operations. A hook that has not read any
    /// credential yet can safely be skipped: it will read the committed
    /// replacement or publish its own failure warning.
    ///
    /// The replacement route passes the helm's host reply bound counted from
    /// its own start (decided by the GPUI migration's orchestrator on
    /// 2026-11-15), which ends before that deadline: the UI channel admits one
    /// command at a time, so a save must not hold every later command for the
    /// whole SSH budget (SPEC_impl.md, "The UI channel"). A host whose delivery
    /// has not finished by `reply_by` is reported as not delivered, with a
    /// reason saying it did not answer in time. That is not a refusal, since
    /// the value is already saved, and the delivery is not cancelled either:
    /// like the other bounded routes' work, it keeps running on its own task
    /// until it finishes or meets its own deadline, and its outcome lands in the
    /// host's warning, which a later success clears. So a host named in the
    /// returned list may still receive the value.
    pub(crate) async fn push(
        self: &Arc<Self>,
        name: &str,
        reply_by: Instant,
        events: &Arc<FleetEvents>,
    ) -> Vec<DeliveryFailure> {
        let deadline = Instant::now() + self.budget;
        let candidates = self
            .entries
            .lock()
            .unwrap()
            .values()
            .filter_map(|entry| {
                let lease = *entry.active.borrow();
                (lease.count != 0
                    && entry
                        .live
                        .lock()
                        .unwrap()
                        .binding
                        .as_ref()
                        .is_some_and(|(_, bindings)| bindings.uses(name)))
                .then(|| (Arc::clone(entry), lease.generation))
            })
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return Vec::new();
        }
        // The deliveries run on a task of their own, so that answering at
        // `reply_by` abandons only this wait. Each sends its candidate's index
        // with its outcome, which is how the hosts still running are found.
        let (sender, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
        let work = {
            let (this, name, events) = (Arc::clone(self), name.to_owned(), Arc::clone(events));
            let candidates = candidates.clone();
            tokio::spawn(async move {
                let (this, name, events) = (&this, name.as_str(), &*events);
                stream::iter(candidates.into_iter().enumerate())
                    .map(|(index, (entry, generation))| async move {
                        (
                            index,
                            this.push_one(&entry, generation, name, deadline, events)
                                .await,
                        )
                    })
                    .buffer_unordered(PUSH_CONCURRENCY)
                    .for_each(|outcome| {
                        // The reply may already have gone; the outcome then
                        // shows only in the host's warning.
                        let _ = sender.send(outcome);
                        std::future::ready(())
                    })
                    .await;
            })
        };
        let mut finished = vec![false; candidates.len()];
        let mut failures = Vec::new();
        loop {
            match timeout_at(reply_by, outcomes.recv()).await {
                Ok(Some((index, failure))) => {
                    finished[index] = true;
                    failures.extend(failure);
                }
                // Every candidate reported, or the task ended early, which
                // only a panic or the runtime's shutdown does. A panic is
                // re-raised as it would have been inline; at shutdown the
                // failures known so far are returned.
                Ok(None) => {
                    if let Err(join) = work.await
                        && join.is_panic()
                    {
                        std::panic::resume_unwind(join.into_panic());
                    }
                    return failures;
                }
                Err(_) => break,
            }
        }
        // A host whose last lease ended has had its push cancelled, and one
        // whose new connection no longer uses this name reports nothing; both
        // are left out here as they would be had they finished.
        failures.extend(
            candidates
                .iter()
                .zip(finished)
                .filter(|(_, done)| !done)
                .filter_map(|((entry, generation), _)| {
                    Some(DeliveryFailure {
                        host: Self::reported_host(entry, name, *generation)?,
                        reason: still_delivering(),
                    })
                }),
        );
        failures
    }

    /// One host's share of [`Self::push`], under the push's shared deadline:
    /// the failure to report, or `None` when it was delivered or has nothing
    /// to report (its lease ended, or its connection stopped using `name`).
    async fn push_one(
        &self,
        entry: &Entry,
        generation: u64,
        name: &str,
        deadline: Instant,
        events: &FleetEvents,
    ) -> Option<DeliveryFailure> {
        let attempt = entry.begin_push(name, generation)?;
        let mut active = entry.active.subscribe();
        let result = tokio::select! {
            biased;
            _ = active.wait_for(|state| !state.is_current(generation)) => return None,
            result = timeout_at(deadline, async {
                let guard = Arc::clone(&entry.serial).lock_owned().await;
                let binding = entry.live.lock().unwrap().binding.clone();
                let Some((row, bindings)) = binding.filter(|(_, binding)| binding.uses(name)) else {
                    return (None, None);
                };
                let uses_claude = bindings.uses_claude(name);
                let uses_codex = bindings.uses_codex_endpoint(name);
                let uses_codex_login = bindings.uses_codex_login(name);
                match self.apply(&row, bindings, None, DeliveryPhase::Connected, guard).await {
                    Ok(warnings) => {
                        let failure = warnings.push_failure(uses_claude, uses_codex, uses_codex_login);
                        (warnings.message(), failure)
                    }
                    Err(error) => {
                        let earlier = error.lookup_warnings.as_ref().and_then(|warnings| {
                            warnings.push_failure(uses_claude, uses_codex, uses_codex_login)
                        });
                        (Some(error.message()), Some(error.with_warning(earlier)))
                    }
                }
            }) => result.unwrap_or_else(|_| (Some(TIMED_OUT.into()), Some(TIMED_OUT.into()))),
        };
        let host = Self::reported_host(entry, name, generation)?;
        let (warning, failure) = result;
        entry.warning(attempt, warning, events);
        failure.map(|reason| DeliveryFailure { host, reason })
    }

    /// The name a delivery failure gives its host (its alias, else its
    /// environment's name). Only the selected lifetime reports this push.
    /// A successor or a connection with another selection reports its own
    /// delivery.
    fn reported_host(entry: &Entry, name: &str, generation: u64) -> Option<String> {
        let live = entry.live.lock().unwrap();
        if !entry.active.borrow().is_current(generation) {
            return None;
        }
        let (row, _) = live
            .binding
            .as_ref()
            .filter(|(_, binding)| binding.uses(name))?;
        Some(
            row.alias
                .clone()
                .or_else(|| {
                    row.environment_binding()
                        .map(|binding| binding.handle.name().to_owned())
                })
                .unwrap_or_else(|| "host".into()),
        )
    }

    /// Resolve revisions under the host lock and record each successful file
    /// before attempting the next. Equal fingerprints perform no SSH at all.
    /// A lookup or legacy cleanup warning can leave launch credentials pending
    /// while independent Codex login and GitHub deliveries finish.
    async fn apply(
        &self,
        row: &HostRow,
        bindings: Bindings,
        home: Option<PathBuf>,
        phase: DeliveryPhase,
        mut guard: OwnedMutexGuard<()>,
    ) -> Result<DeliveryWarnings, DeliveryError> {
        let mut record = self.read_record(row).await?;
        if let Some(home) = home
            && record.home.as_ref() != Some(&home)
        {
            record.home = Some(home);
            guard = self.write_record(row, &record, guard).await?;
        }
        // Delivery and supervisor dialing must select the same live route.
        // A container's place destination can change after this lease began.
        let target = match crate::access::resolve_host_access(&self.store, row.id, &self.places)
            .await
            .map_err(|error| error.to_string())?
        {
            crate::access::HostAccess::Remote(route) => ProvisioningTarget::Remote(route),
            crate::access::HostAccess::Local => {
                return Err("environment credentials require a remote route"
                    .to_owned()
                    .into());
            }
        };
        // Endpoint values are resolved inside the same lease and lock as the snapshot they replace.
        let claude = if let Some(login) = bindings.claude_login_credential {
            self.resolve_claude_login(login).await
        } else {
            self.resolve_endpoint(bindings.claude, "Claude").await
        };
        let codex = match phase {
            DeliveryPhase::Setup => None,
            DeliveryPhase::Connected => Some(self.resolve_endpoint(bindings.codex, "Codex").await),
        };
        let (next_guard, mut warnings) = self
            .apply_agent_environment(row, &target, &mut record, claude, codex, guard)
            .await?;
        guard = next_guard;
        let later_result = async {
            if let Some(name) = bindings.codex_login_credential {
                guard = self
                    .apply_codex_login(row, &target, &mut record, name, guard, &mut warnings)
                    .await?;
            }
            self.apply_github_token(
                row,
                &target,
                &mut record,
                bindings.github_token_credential,
                guard,
            )
            .await
        }
        .await;
        match later_result {
            Ok(()) => Ok(warnings),
            Err(reason) => Err(DeliveryError {
                reason,
                lookup_warnings: warnings.lookup_aborted.then_some(warnings),
            }),
        }
    }

    /// Seed a selected login independently of the launch credentials. Once a launch lookup has stalled that file,
    /// a failed login lookup also leaves its slot pending so the pass can still reach GitHub under the same host
    /// lock. Without a stalled launch file, a failed login lookup is a hard error that ends the pass, as before.
    async fn apply_codex_login(
        &self,
        row: &HostRow,
        target: &ProvisioningTarget,
        record: &mut Record,
        name: String,
        mut guard: OwnedMutexGuard<()>,
        warnings: &mut DeliveryWarnings,
    ) -> Result<OwnedMutexGuard<()>, String> {
        let lookup = name.clone();
        let result = self
            .store
            .conn()
            .call("resolve Codex login", move |conn| {
                crate::credentials::secret(
                    conn,
                    &lookup,
                    farhelm_proto::credentials::CredentialKind::CodexLogin,
                )?
                .ok_or_else(|| anyhow::anyhow!("Codex login no longer exists"))
            })
            .await;
        let (auth, revision, incarnation) = match result {
            Ok(login) => login,
            Err(_) if warnings.lookup_aborted => {
                warnings.codex_login_lookup_failed = true;
                return Ok(guard);
            }
            Err(_) => return Err(CODEX_LOGIN_LOOKUP.into()),
        };
        let stamp = (name, revision, incarnation);
        if record.codex_auth.as_ref() == Some(&stamp) {
            return Ok(guard);
        }
        if record.home.is_none() {
            record.home = Some(self.settings_home(target).await?);
            guard = self.write_record(row, record, guard).await?;
        }
        // Never read auth.json: Codex owns its refreshed contents. The last delivered stamp alone authorizes a
        // replacement.
        let directory = record
            .home
            .as_ref()
            .ok_or("the host has no known home directory")?
            .join(".codex");
        let path = directory.join("auth.json");
        let backend = self
            .backend
            .as_ref()
            .ok_or("credential delivery is unavailable")?;
        backend
            .ensure_directories(
                target,
                &[DirectorySpec {
                    path: directory,
                    mode: 0o700,
                    shared: true,
                }],
            )
            .await
            .map_err(|error| {
                error.redacted_reason("could not prepare the Codex login directory")
            })?;
        let temporary =
            path.with_file_name(format!(".auth.json.farhelm-tmp-{}", uuid::Uuid::new_v4()));
        backend
            .install_bytes(target, auth.as_bytes(), &path, &temporary, 0o600)
            .await
            .map_err(|error| error.redacted_reason("could not install the Codex login"))?;
        record.codex_auth = Some(stamp);
        self.write_record(row, record, guard).await
    }

    /// Resolve GitHub last so a stale selection here cannot withhold earlier slot deliveries or their stamps.
    async fn apply_github_token(
        &self,
        row: &HostRow,
        target: &ProvisioningTarget,
        record: &mut Record,
        credential: Option<String>,
        guard: OwnedMutexGuard<()>,
    ) -> Result<(), String> {
        let github = self.resolve_github(credential).await?;
        if record.github.as_ref() != Some(&github.stamp) {
            let state_dir = row
                .remote_state_dir
                .as_deref()
                .map(Path::new)
                .filter(|path| path.is_absolute())
                .ok_or("the host has no absolute supervisor state directory")?;
            let path = farhelm_supervisor::github_token::host_file(state_dir);
            let backend = self
                .backend
                .as_ref()
                .ok_or("credential delivery is unavailable")?;
            if let Some(token) = github.token {
                backend
                    .ensure_directories(
                        target,
                        &[DirectorySpec {
                            path: state_dir.to_path_buf(),
                            mode: 0o700,
                            shared: true,
                        }],
                    )
                    .await
                    .map_err(|error| {
                        error.redacted_reason("could not prepare the GitHub token directory")
                    })?;
                let temporary = path.with_file_name(format!(
                    ".github-token.farhelm-tmp-{}",
                    uuid::Uuid::new_v4()
                ));
                backend
                    .install_bytes(target, token.as_bytes(), &path, &temporary, 0o600)
                    .await
                    .map_err(|error| error.redacted_reason("could not install the GitHub token"))?;
                if !matches!(record.github, Some(GithubStamp::Token { .. })) {
                    let available = backend.setup_github_git(target).await.map_err(|error| {
                        error.redacted_reason("could not configure GitHub's git helper")
                    })?;
                    if !available {
                        return Err(
                            "GitHub CLI (gh) is missing; private clones need gh installed".into(),
                        );
                    }
                }
            } else {
                backend
                    .remove_owned_file(target, &path)
                    .await
                    .map_err(|error| {
                        error.redacted_reason("could not remove the managed GitHub token")
                    })?;
            }
            record.github = Some(github.stamp);
            self.write_record(row, record, guard).await?;
        }
        Ok(())
    }

    /// Publish both agents' entries atomically after their catalog lookups succeed. The previous
    /// file and stamps are the last known good snapshot if a lookup fails. Once both values are
    /// known, legacy settings retire independently, and a failed cleanup withholds its own entry.
    async fn apply_agent_environment(
        &self,
        row: &HostRow,
        target: &ProvisioningTarget,
        record: &mut Record,
        claude: Result<Slot, String>,
        codex: Option<Result<Slot, String>>,
        mut guard: OwnedMutexGuard<()>,
    ) -> Result<(OwnedMutexGuard<()>, DeliveryWarnings), String> {
        let (claude, codex) = match (claude, codex.transpose()) {
            (Ok(claude), Ok(codex)) => (claude, codex),
            (claude, codex) => {
                return Ok((
                    guard,
                    DeliveryWarnings {
                        lookup_aborted: true,
                        claude: claude.err(),
                        codex: codex.err(),
                        codex_login_lookup_failed: false,
                    },
                ));
            }
        };
        if codex.is_none() && record.codex_environment.is_some() {
            // A reconnect owns the next write to a previously installed combined snapshot. Setup has neither
            // verified the supervisor's protocol nor resolved the Codex entry to carry forward.
            return Ok((guard, DeliveryWarnings::default()));
        }
        let mut warnings = DeliveryWarnings::default();
        let mut credentials =
            super::agent_endpoints::claude_environment(claude.endpoint, claude.subscription);
        let mut claude_stamp = Some(claude.stamp);
        let mut codex_stamp = codex.map(|slot| {
            super::agent_endpoints::add_codex_endpoint(&mut credentials, slot.endpoint);
            slot.stamp
        });
        // Check the largest candidate before clearing legacy keys, even if a cleanup later omits an entry.
        let mut bytes = encode_agent_environment(&credentials)?;
        let mut withheld = false;
        let (next_guard, warning) = self
            .retire_legacy_settings(row, target, record, LegacySettings::Claude, guard)
            .await?;
        guard = next_guard;
        if let Some(reason) = warning {
            warnings.claude = Some(reason);
            claude_stamp = None;
            credentials
                .env
                .retain(|name, _| name == recipe_credentials::CODEX_ENDPOINT_TOKEN_ENV);
            withheld = true;
        }
        if codex_stamp.is_some() {
            let (next_guard, warning) = self
                .retire_legacy_settings(row, target, record, LegacySettings::Codex, guard)
                .await?;
            guard = next_guard;
            if let Some(reason) = warning {
                warnings.codex = Some(reason);
                codex_stamp = None;
                credentials.codex_endpoint = None;
                credentials
                    .env
                    .remove(recipe_credentials::CODEX_ENDPOINT_TOKEN_ENV);
                withheld = true;
            }
        }
        if record.claude_environment != claude_stamp || record.codex_environment != codex_stamp {
            if withheld {
                bytes = encode_agent_environment(&credentials)?;
            }
            let state_dir = row
                .remote_state_dir
                .as_deref()
                .map(Path::new)
                .filter(|path| path.is_absolute())
                .ok_or("the host has no absolute supervisor state directory")?;
            let path = recipe_credentials::host_file(state_dir);
            let backend = self
                .backend
                .as_ref()
                .ok_or("credential delivery is unavailable")?;
            if let Some(bytes) = bytes {
                backend
                    .ensure_directories(
                        target,
                        &[DirectorySpec {
                            path: state_dir.to_path_buf(),
                            mode: 0o700,
                            shared: true,
                        }],
                    )
                    .await
                    .map_err(|error| {
                        error.redacted_reason(
                            "could not prepare the managed launch credentials directory",
                        )
                    })?;
                let temporary = path.with_file_name(format!(
                    ".agent-credentials.farhelm-tmp-{}",
                    uuid::Uuid::new_v4()
                ));
                backend
                    .install_bytes(target, &bytes, &path, &temporary, 0o600)
                    .await
                    .map_err(|error| {
                        error.redacted_reason("could not install the managed launch credentials")
                    })?;
            } else {
                backend
                    .remove_owned_file(target, &path)
                    .await
                    .map_err(|error| {
                        error.redacted_reason("could not remove the managed launch credentials")
                    })?;
            }
            record.claude_environment = claude_stamp;
            record.codex_environment = codex_stamp;
            guard = self.write_record(row, record, guard).await?;
        }

        Ok((guard, warnings))
    }

    /// Clear only a settings file for which a prior Farhelm left a stamp. Record each success before attempting the
    /// other file so cancellation or a refusal cannot send a finished migration back through a user's settings.
    async fn retire_legacy_settings(
        &self,
        row: &HostRow,
        target: &ProvisioningTarget,
        record: &mut Record,
        agent: LegacySettings,
        mut guard: OwnedMutexGuard<()>,
    ) -> Result<(OwnedMutexGuard<()>, Option<String>), String> {
        let stamp = agent.stamp(record);
        if *stamp == Some(EndpointStamp::Cleared) {
            return Ok((guard, None));
        }
        // Only unreleased builds between #3081 and #3086 wrote a Codex Absent stamp without clearing anything; no
        // release did. Released Farhelm records a managed Codex settings write only as Endpoint, so settings found under
        // Absent are the user's own and stay as they are.
        let needs_cleanup = match agent {
            LegacySettings::Claude => stamp.is_some(),
            LegacySettings::Codex => matches!(stamp, Some(EndpointStamp::Endpoint { .. })),
        };
        if needs_cleanup {
            let (name, subdir, file) = match agent {
                LegacySettings::Claude => ("Claude", ".claude", "settings.json"),
                LegacySettings::Codex => ("Codex", ".codex", "config.toml"),
            };
            if record.home.is_none() {
                match self.settings_home(target).await {
                    Ok(home) => {
                        record.home = Some(home);
                        guard = self.write_record(row, record, guard).await?;
                    }
                    Err(reason) => {
                        return Ok((guard, Some(cleanup_pending(name, &reason))));
                    }
                }
            }
            let directory = record.home.as_ref().expect("settings home").join(subdir);
            let merge = match agent {
                LegacySettings::Claude => super::agent_endpoints::clear_claude,
                LegacySettings::Codex => super::agent_endpoints::clear_codex,
            };
            if let Err(reason) = self.merge_settings(target, &directory, file, merge).await {
                return Ok((guard, Some(cleanup_pending(name, &reason))));
            }
        }
        *agent.stamp(record) = Some(EndpointStamp::Cleared);
        guard = self.write_record(row, record, guard).await?;
        Ok((guard, None))
    }

    /// Probe only for a stamped settings migration or a Subscription seed. An endpoint snapshot has no home dependency.
    async fn settings_home(&self, target: &ProvisioningTarget) -> Result<PathBuf, String> {
        let backend = self
            .backend
            .as_ref()
            .ok_or("credential delivery is unavailable")?;
        backend
            .host_home(target)
            .await
            .map_err(|error| error.redacted_reason("could not read the sandbox's home directory"))
    }

    /// Retry a concurrent settings save once with the editor's latest bytes.
    /// The caller records its stamp only after the checked install succeeds
    /// or the merged settings already satisfy the request.
    async fn merge_settings(
        &self,
        target: &ProvisioningTarget,
        directory: &Path,
        file: &str,
        merge: impl Fn(Option<&[u8]>) -> Result<Option<Vec<u8>>, String>,
    ) -> Result<(), String> {
        let backend = self
            .backend
            .as_ref()
            .ok_or("credential delivery is unavailable")?;
        let path = directory.join(file);
        for attempt in 0..2 {
            let previous = backend
                .read_small_file(target, &path)
                .await
                .map_err(|error| error.redacted_reason("could not read the agent settings"))?;
            let Some(bytes) = merge(previous.as_deref())? else {
                return Ok(());
            };
            let expected = previous.as_deref().map(super::backend::hex_sha256);
            backend
                .ensure_directories(
                    target,
                    &[DirectorySpec {
                        path: directory.to_path_buf(),
                        mode: 0o700,
                        shared: true,
                    }],
                )
                .await
                .map_err(|error| {
                    error.redacted_reason("could not prepare the agent settings directory")
                })?;
            let temporary =
                path.with_file_name(format!(".{file}.farhelm-tmp-{}", uuid::Uuid::new_v4()));
            match backend
                .install_bytes_checked(
                    target,
                    &bytes,
                    &path,
                    &temporary,
                    0o600,
                    Some(expected.as_deref()),
                )
                .await
            {
                Err(error)
                    if error.context == super::backend::DESTINATION_CHANGED && attempt == 0 =>
                {
                    continue;
                }
                result => {
                    return result.map(|_| ()).map_err(|error| {
                        error.redacted_reason("could not install the agent settings")
                    });
                }
            }
        }
        unreachable!("the second attempt always settles the settings merge")
    }

    /// Load only credential selections, without build state or vendor auth.
    async fn read_bindings(&self, row: &HostRow) -> Result<Bindings, String> {
        let name = row
            .environment_binding()
            .ok_or("the host is not an environment")?
            .handle
            .recipe()
            .to_owned();
        self.store
            .conn()
            .call("credential recipe selection", move |conn| {
                let json: String = conn.query_row(
                    "SELECT spec_json FROM tensorlake_recipes WHERE name = ?1",
                    [name],
                    |row| row.get(0),
                )?;
                Ok(serde_json::from_str(&json)?)
            })
            .await
            .map_err(|_| "could not read the host's recipe credential selections".into())
    }

    /// Each catalog read binds a value to its revision. The shared resolver
    /// already keeps secrets transient; warnings discard storage diagnostics.
    async fn resolve_endpoint(
        &self,
        endpoint: Option<StoredEndpoint>,
        agent: &str,
    ) -> Result<Slot, String> {
        let Some(endpoint) = endpoint else {
            return Ok(Slot {
                endpoint: None,
                subscription: None,
                stamp: EndpointStamp::Cleared,
            });
        };
        let name = endpoint.token_credential.clone();
        let resolved =
            self.store.resolve_endpoint(endpoint).await.map_err(|_| {
                format!("could not resolve the recipe's {agent} endpoint credential")
            })?;
        let credential = name
            .zip(resolved.revision)
            .zip(resolved.incarnation.clone())
            .map(|((name, revision), incarnation)| (name, revision, incarnation));
        Ok(Slot {
            stamp: EndpointStamp::Endpoint {
                base_url: resolved.base_url.clone(),
                credential,
            },
            endpoint: Some(resolved),
            subscription: None,
        })
    }

    /// Bind the setup-token to its catalog incarnation and revision under the
    /// host lock. Replacement uses the same connection lease and durable-file
    /// boundary as endpoints, never a cached secret or a separate agent home.
    async fn resolve_claude_login(&self, name: String) -> Result<Slot, String> {
        let lookup = name.clone();
        let (token, revision, incarnation) = self
            .store
            .conn()
            .call("resolve Claude login", move |conn| {
                crate::credentials::secret(
                    conn,
                    &lookup,
                    farhelm_proto::credentials::CredentialKind::ClaudeLogin,
                )?
                .ok_or_else(|| anyhow::anyhow!("Claude login no longer exists"))
            })
            .await
            .map_err(|_| "could not resolve the recipe's Claude login")?;
        Ok(Slot {
            endpoint: None,
            subscription: Some(token),
            stamp: EndpointStamp::Subscription {
                name,
                revision,
                incarnation,
            },
        })
    }

    /// Resolve the later GitHub slot only after earlier successful endpoint
    /// writes. Missing retained names remain a warning without undoing them.
    async fn resolve_github(&self, name: Option<String>) -> Result<GithubSlot, String> {
        Ok(if let Some(name) = name {
            let lookup = name.clone();
            let (token, revision, incarnation) = self
                .store
                .conn()
                .call("resolve GitHub credential", move |conn| {
                    crate::credentials::secret(
                        conn,
                        &lookup,
                        farhelm_proto::credentials::CredentialKind::GithubToken,
                    )?
                    .ok_or_else(|| anyhow::anyhow!("GitHub credential no longer exists"))
                })
                .await
                .map_err(|_| "could not resolve the recipe's GitHub credential")?;
            GithubSlot {
                token: Some(token),
                stamp: GithubStamp::Token {
                    name,
                    revision,
                    incarnation,
                },
            }
        } else {
            GithubSlot {
                token: None,
                stamp: GithubStamp::Absent,
            }
        })
    }

    /// Missing state is unknown, not successful absence. Refuse corrupt state
    /// and stale host identities with fixed text rather than leaking JSON or
    /// accepting a relative home for subsequent remote file operations.
    async fn read_record(&self, row: &HostRow) -> Result<Record, String> {
        let (id, identity) = (row.id, row.entry_uuid.clone());
        self.store.conn().call("read credential delivery state", move |conn| {
            let json: Option<Option<String>> = conn.query_row("SELECT tensorlake_credential_delivery FROM hosts WHERE id = ?1 AND entry_uuid = ?2 AND tensorlake_sandbox IS NOT NULL", params![id, identity], |row| row.get(0)).optional()?;
            match json {
                None => anyhow::bail!("host no longer exists"),
                Some(None) => Ok(Record::default()),
                Some(Some(json)) => {
                    let record: Record = serde_json::from_str(&json)?;
                    anyhow::ensure!(record.home.as_ref().is_none_or(|home| home.is_absolute()), "host home is not absolute");
                    Ok(record)
                },
            }
        }).await.map_err(|_| STORAGE.into())
    }

    /// Db::call's blocking closure survives cancellation. Move serialization
    /// into it, and return it only after the write finishes; a timed-out caller
    /// cannot let a later operation overtake its still-queued record update.
    async fn write_record(
        &self,
        row: &HostRow,
        record: &Record,
        guard: OwnedMutexGuard<()>,
    ) -> Result<OwnedMutexGuard<()>, String> {
        let (id, identity) = (row.id, row.entry_uuid.clone());
        let json = serde_json::to_string(record).map_err(|_| STORAGE.to_string())?;
        self.store.conn().call("record credential delivery", move |conn| {
            let guard = guard;
            anyhow::ensure!(conn.execute("UPDATE hosts SET tensorlake_credential_delivery = ?3 WHERE id = ?1 AND entry_uuid = ?2 AND tensorlake_sandbox IS NOT NULL", params![id, identity, json])? == 1, "host no longer exists");
            Ok(guard)
        }).await.map_err(|_| STORAGE.into())
    }
}
