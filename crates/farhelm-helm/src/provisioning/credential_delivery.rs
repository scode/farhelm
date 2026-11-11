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
            || self.claude_login_credential.as_deref() == Some(name)
            || self.codex_login_credential.as_deref() == Some(name)
            || [&self.claude, &self.codex]
                .into_iter()
                .flatten()
                .any(|endpoint| endpoint.token_credential.as_deref() == Some(name))
    }
}

/// Explicit modes distinguish completed cleanup from older no-work records.
/// Claude's login shares its settings file with endpoint keys, so that file's
/// fingerprint includes the login identity without retaining its secret.
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
    claude: Option<EndpointStamp>,
    codex: Option<EndpointStamp>,
    /// Codex refreshes this file itself. Leaving subscription mode must retain
    /// its last successful seed, or returning would overwrite refreshed bytes.
    codex_auth: Option<(String, u64, String)>,
    github: Option<GithubStamp>,
}

/// Secret-bearing values exist only while a serialized operation uses them.
struct Slot {
    endpoint: Option<ResolvedEndpoint>,
    /// Only Claude stores its subscription token in the settings file.
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
    active: watch::Sender<usize>,
    live: Mutex<Live>,
}

impl Entry {
    /// Identity changes allocate another entry; an old lease cannot end it.
    fn new(row: &HostRow) -> Self {
        Self {
            entry_uuid: row.entry_uuid.clone(),
            serial: Arc::default(),
            active: watch::channel(0).0,
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
    fn begin_push(&self, name: &str) -> Option<u64> {
        let mut live = self.live.lock().unwrap();
        if !live
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
            .send_modify(|count| *count = count.saturating_sub(1));
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
            entry.active.send_modify(|count| {
                if *count == 0 {
                    live.binding = None;
                }
                *count += 1;
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
            entry.active.send_replace(0);
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
            self.apply(row, bindings, None, guard).await
        })
        .await
        .unwrap_or_else(|_| Err(TIMED_OUT.into()));
        entry.warning(attempt, result.err(), events);
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
                guard,
            )
            .await
        })
        .await
        .unwrap_or_else(|_| Err(TIMED_OUT.into()));
        entry.warning(attempt, result.clone().err(), events);
        result
    }

    /// One deadline covers all candidates, lock waits and transport operations.
    /// A hook that has not read any credential yet can safely be skipped: it
    /// will read the committed replacement or publish its own failure warning.
    pub(crate) async fn push(&self, name: &str, events: &FleetEvents) -> Vec<DeliveryFailure> {
        let deadline = Instant::now() + self.budget;
        let entries = self
            .entries
            .lock()
            .unwrap()
            .values()
            .filter(|entry| {
                let active = *entry.active.borrow() != 0;
                active
                    && entry
                        .live
                        .lock()
                        .unwrap()
                        .binding
                        .as_ref()
                        .is_some_and(|(_, bindings)| bindings.uses(name))
            })
            .cloned()
            .collect::<Vec<_>>();
        stream::iter(entries).map(|entry| async move {
            let attempt = entry.begin_push(name)?;
            let mut active = entry.active.subscribe();
            let result = tokio::select! {
                biased;
                _ = active.wait_for(|count| *count == 0) => return None,
                result = timeout_at(deadline, async {
                    let guard = Arc::clone(&entry.serial).lock_owned().await;
                    let binding = entry.live.lock().unwrap().binding.clone();
                    let Some((row, bindings)) = binding.filter(|(_, binding)| binding.uses(name)) else { return Ok(()); };
                    self.apply(&row, bindings, None, guard).await
                }) => result.unwrap_or_else(|_| Err(TIMED_OUT.into())),
            };
            let row = entry.live.lock().unwrap().binding.as_ref()
                .filter(|(_, binding)| binding.uses(name)).map(|(row, _)| row.clone())?;
            entry.warning(attempt, result.clone().err(), events);
            result.err().map(|reason| DeliveryFailure {
                host: row.alias.clone().or_else(|| row.environment_binding().map(|binding| binding.handle.name().to_owned())).unwrap_or_else(|| "host".into()),
                reason,
            })
        }).buffer_unordered(PUSH_CONCURRENCY).filter_map(|result| async { result }).collect().await
    }

    /// Resolve revisions under the host lock and record each successful file
    /// before attempting the next. Equal fingerprints perform no SSH at all.
    async fn apply(
        &self,
        row: &HostRow,
        bindings: Bindings,
        home: Option<PathBuf>,
        mut guard: OwnedMutexGuard<()>,
    ) -> Result<(), String> {
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
                return Err("environment credentials require a remote route".into());
            }
        };
        for (is_claude, folder, file, endpoint, login) in [
            (
                true,
                ".claude",
                "settings.json",
                bindings.claude,
                bindings.claude_login_credential,
            ),
            (false, ".codex", "config.toml", bindings.codex, None),
        ] {
            // Resolve at this slot: a missing retained name in a later slot
            // must not withhold an earlier replacement or its durable success.
            let slot = if let Some(login) = login {
                self.resolve_claude_login(login).await?
            } else {
                self.resolve_endpoint(endpoint).await?
            };
            let previous_stamp = if is_claude {
                &record.claude
            } else {
                &record.codex
            };
            if previous_stamp.as_ref() == Some(&slot.stamp) {
                continue;
            }
            // Retry only this file, merging the editor's newest bytes once. A second
            // conflict returns before the success stamp, so reconnect can retry later.
            for attempt in 0..2 {
                if slot.endpoint.is_none()
                    && slot.subscription.is_none()
                    && slot.stamp != EndpointStamp::Cleared
                {
                    break;
                }
                let backend = self
                    .backend
                    .as_ref()
                    .ok_or("credential delivery is unavailable")?;
                if record.home.is_none() {
                    record.home = Some(backend.host_home(&target).await.map_err(|error| {
                        error.redacted_reason("could not read the sandbox's home directory")
                    })?);
                    guard = self.write_record(row, &record, guard).await?;
                }
                let directory = record.home.as_ref().unwrap().join(folder);
                let path = directory.join(file);
                let previous = backend
                    .read_small_file(&target, &path)
                    .await
                    .map_err(|error| error.redacted_reason("could not read the agent settings"))?;
                let expected = previous.as_deref().map(super::backend::hex_sha256);
                let bytes = if let Some(endpoint) = &slot.endpoint {
                    Some(if is_claude {
                        super::agent_endpoints::claude(previous.as_deref(), endpoint)
                    } else {
                        super::agent_endpoints::codex(previous.as_deref(), endpoint)
                    }?)
                } else if let Some(token) = &slot.subscription {
                    Some(super::agent_endpoints::claude_subscription(
                        previous.as_deref(),
                        token,
                    )?)
                } else if is_claude {
                    super::agent_endpoints::clear_claude(previous.as_deref())?
                } else {
                    super::agent_endpoints::clear_codex(previous.as_deref())?
                };
                if let Some(bytes) = bytes {
                    backend
                        .ensure_directories(
                            &target,
                            &[DirectorySpec {
                                path: directory,
                                mode: 0o700,
                                shared: true,
                            }],
                        )
                        .await
                        .map_err(|error| {
                            error.redacted_reason("could not prepare the agent settings directory")
                        })?;
                    let temporary = path
                        .with_file_name(format!(".{file}.farhelm-tmp-{}", uuid::Uuid::new_v4()));
                    match backend
                        .install_bytes_checked(
                            &target,
                            &bytes,
                            &path,
                            &temporary,
                            0o600,
                            Some(expected.as_deref()),
                        )
                        .await
                    {
                        Err(error)
                            if error.context == super::backend::DESTINATION_CHANGED
                                && attempt == 0 =>
                        {
                            continue;
                        }
                        result => {
                            result.map_err(|error| {
                                error.redacted_reason("could not install the agent settings")
                            })?;
                        }
                    }
                }
                break;
            }
            if is_claude {
                record.claude = Some(slot.stamp);
            } else {
                record.codex = Some(slot.stamp);
            }
            guard = self.write_record(row, &record, guard).await?;
        }
        if let Some(name) = bindings.codex_login_credential {
            let lookup = name.clone();
            let (auth, revision, incarnation) = self
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
                .await
                .map_err(|_| "could not resolve the recipe's Codex login")?;
            let stamp = (name, revision, incarnation);
            if record.codex_auth.as_ref() != Some(&stamp) {
                // Config cleanup established the ordinary home first. Never
                // read auth.json: its refreshed contents belong to Codex.
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
                        &target,
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
                    .install_bytes(&target, auth.as_bytes(), &path, &temporary, 0o600)
                    .await
                    .map_err(|error| error.redacted_reason("could not install the Codex login"))?;
                record.codex_auth = Some(stamp);
                guard = self.write_record(row, &record, guard).await?;
            }
        }
        // An active connection can still name a credential removed after a
        // pending recipe edit. That later slot must not prevent independent
        // endpoint rotations and their successful stamps from reaching disk.
        let github = self
            .resolve_github(bindings.github_token_credential)
            .await?;
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
                        &target,
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
                    .install_bytes(&target, token.as_bytes(), &path, &temporary, 0o600)
                    .await
                    .map_err(|error| error.redacted_reason("could not install the GitHub token"))?;
                if !matches!(record.github, Some(GithubStamp::Token { .. })) {
                    let available = backend.setup_github_git(&target).await.map_err(|error| {
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
                    .remove_github_token(&target, &path)
                    .await
                    .map_err(|error| {
                        error.redacted_reason("could not remove the managed GitHub token")
                    })?;
            }
            record.github = Some(github.stamp);
            self.write_record(row, &record, guard).await?;
        }
        Ok(())
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
    async fn resolve_endpoint(&self, endpoint: Option<StoredEndpoint>) -> Result<Slot, String> {
        let Some(endpoint) = endpoint else {
            return Ok(Slot {
                endpoint: None,
                subscription: None,
                stamp: EndpointStamp::Cleared,
            });
        };
        let name = endpoint.token_credential.clone();
        let resolved = self
            .store
            .resolve_endpoint(endpoint)
            .await
            .map_err(|_| "could not resolve the recipe's endpoint credentials")?;
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
