//! `/api/hosts` — the host registry as a REST surface (PLAN_M6.md item 5).
//!
//! Everything the UI's hosts panel needs and nothing it does not: the
//! registry's own facts, each host's live connection state with the
//! evidence that makes it actionable (both versions on skew, both
//! identities on a mismatch, the twin on a duplicate), and the five
//! mutations SPEC.md's host management consists of — add, retarget,
//! remove, adopt, retry.
//!
//! ## Two reads, joined
//!
//! A host's facts come from two places that are authoritative about
//! different things, and this module joins them rather than picking one.
//! [`crate::manager::ConnectionManager::snapshots`] is authority for the
//! live state AND for the destination shown beside it — those two are
//! published together under one lock precisely so a freshly retargeted host
//! can never be drawn with the old connection's state. helm.db is authority
//! for what a connection cannot know: the recorded identity (which first
//! contact writes without any actor being reconciled) and the
//! `remote_farhelm`/`remote_state_dir` fields the user set at registration.
//! Reading identity from the snapshot instead would show `NULL` for every
//! host until the next reconcile, which is most of them, most of the time.
//!
//! ## Add always registers
//!
//! `POST /api/hosts` writes the row whether or not anything answers at that
//! destination, and lets the connection state say what was found. That is
//! what makes the registry meaningful for a host that is simply switched
//! off at the moment it is added — and what makes `--ensure-hosts`
//! (`crate::ensure`) a floor rather than a probe.

use crate::aggregate::host_display_name;
use crate::manager::{HostState, RefreshHealth, UnreachableCause};
use crate::store::{HostId, HostKind, HostRow, HostStoreError};
use crate::{AppState, http_error};
use axum::extract::{Path as AxPath, State};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// One registered host as `GET /api/hosts` reports it.
///
/// Flat rather than nested under a `registry`/`connection` split: every
/// consumer of this shape renders one row per host, and a split would only
/// make the common case — "name it and chip its state" — reach through two
/// objects.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct HostView {
    pub(crate) id: HostId,
    /// `"local"` for the one reserved row, `"ssh"` for a registered
    /// destination. The distinction is not cosmetic: it is what tells a
    /// client which rows offer edit and remove at all.
    pub(crate) kind: &'static str,
    /// `None` for the local row, always `Some` for an ssh row.
    pub(crate) destination: Option<String>,
    /// [`host_display_name`]'s rendering — the same string session rows
    /// carry in `host_name`, so a row and its host chip never disagree.
    pub(crate) name: String,
    /// The identity the REGISTRY holds for this host: the one recorded at
    /// first contact, or accepted by an adoption. `None` if the host has
    /// never been reached, or reports none at all.
    ///
    /// Deliberately not "what the host is reporting". During an
    /// identity-mismatch those are two different values, and the difference
    /// is the whole decision the user is being asked to make — the reported
    /// one rides `state` (as `identity-mismatch`'s `reported`), where it is
    /// paired with the `recorded` value it contradicts. A renderer that
    /// took this field for "what is out there" would show the dead install's
    /// identity beside a prompt about the new one.
    pub(crate) identity: Option<String>,
    pub(crate) remote_farhelm: Option<String>,
    pub(crate) remote_state_dir: Option<String>,
    pub(crate) state: HostStateView,
    /// Which CONNECTION this host is on — an opaque, monotonic token that
    /// changes whenever the host's client does, including when it goes away
    /// (see `manager::ActorStatus::incarnation`).
    ///
    /// Served so a client can tell one connection to a host from the next
    /// without interpreting the state payload — the UI's provisioning panel
    /// binds a displayed plan to it, so a plan prepared against one
    /// connection is not left actionable on another. Opaque as a USAGE
    /// convention: compare it, never interpret it — the number is a counter
    /// over this helm's own connections and means nothing across a restart,
    /// which is exactly why a client must not persist one.
    ///
    /// `0` is "never connected".
    pub(crate) incarnation: u64,
}

/// How one host's [`HostState`] crosses the REST boundary.
///
/// Internally tagged by `phase`, whose values are exactly
/// [`HostState::phase`]'s stable labels — the same strings the diagnostic
/// trail logs. That equality is deliberate and load-bearing: a UI chip, a
/// log line, and this JSON must key off ONE vocabulary, or a state that
/// gains a payload field starts reading as a different phase somewhere.
///
/// Every variant carries its own evidence rather than a shared error
/// string, because SPEC.md requires errors to be actionable: "unreachable"
/// and "the host runs an older farhelm" call for opposite responses, and a
/// renderer cannot invent the difference from prose.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "phase")]
pub(crate) enum HostStateView {
    /// Inside the active-retry window — attempts are happening right now.
    #[serde(rename = "connecting")]
    Connecting {
        attempt: u32,
        last_error: Option<String>,
    },
    /// The active window is spent; background re-probing continues
    /// forever, so no user action is required for the host to come back.
    #[serde(rename = "unreachable-reprobing")]
    Unreachable {
        /// `"local-supervisor-not-running"` is the one cause the user can
        /// fix with a command on the machine they are already at, and the
        /// UI's manual-start hint keys off it. Everything else is
        /// `"transport-failure"` with the transport's own text beside it.
        cause: &'static str,
        last_error: String,
    },
    #[serde(rename = "connected")]
    Connected {
        identity: Option<String>,
        build_version: String,
        refresh: RefreshView,
    },
    /// Refused at the hello: both versions are named so the user can see
    /// WHICH side is behind, and `remediation` is the sentence to act on.
    #[serde(rename = "version-skew")]
    VersionSkew {
        peer_protocol: u32,
        peer_build: String,
        our_protocol: u32,
        our_build: String,
        remediation: String,
    },
    /// Frozen awaiting a user decision: adopt the reported identity
    /// (`POST /api/hosts/{id}/adopt`) or fix the destination. Nothing is
    /// connected and nothing is written while this holds.
    #[serde(rename = "identity-mismatch")]
    IdentityMismatch { recorded: String, reported: String },
    /// Frozen because the destination answered without an identity at all
    /// while this entry has one on record — so there is nothing to compare
    /// and, deliberately, nothing to adopt.
    ///
    /// A renderer must NOT offer the adopt verb here (it would be refused,
    /// and the offer itself would be a lie about what is on the table): the
    /// remedies are fixing the host so it reports its identity, retargeting
    /// the entry, or removing it. Re-probed automatically, so a host that
    /// starts identifying itself again recovers unaided.
    #[serde(rename = "identity-unverified")]
    IdentityUnverified { recorded: String },
    /// This ENTRY reaches a host another entry already owns. The host
    /// itself appears exactly once, under `twin`; this row is the entry the
    /// user must edit or remove.
    #[serde(rename = "duplicate")]
    Duplicate { twin: HostId, identity: String },
    /// No actor is running for this row — it retired with its registry row,
    /// or its task panicked. Reported rather than hidden so an operation
    /// refused against it has something honest to name.
    #[serde(rename = "retired")]
    Retired { reason: String },
}

impl HostStateView {
    /// This state's stable phase label — the same eight strings
    /// [`HostState::phase`] produces and the same strings the `#[serde(
    /// rename)]`s above put on the wire.
    ///
    /// A third spelling of one vocabulary, which this type's own docs warn
    /// against, and it earns the exception by being the only way a
    /// non-serializing consumer can reach the word. The agent relay needs
    /// the label as a Rust string (`crate::agent_requests`), and the
    /// alternatives were worse: re-deriving it from the manager's snapshot
    /// would bypass the view every other surface renders from, and
    /// round-tripping through `serde_json` to read a tag back out is a
    /// vocabulary coupling expressed as an allocation. What keeps the copy
    /// honest is `phase_matches_the_serialized_tag`, which compares this
    /// function's answer to the encoder's for every variant.
    pub(crate) fn phase(&self) -> &'static str {
        match self {
            HostStateView::Connecting { .. } => "connecting",
            HostStateView::Unreachable { .. } => "unreachable-reprobing",
            HostStateView::Connected { .. } => "connected",
            HostStateView::VersionSkew { .. } => "version-skew",
            HostStateView::IdentityMismatch { .. } => "identity-mismatch",
            HostStateView::IdentityUnverified { .. } => "identity-unverified",
            HostStateView::Duplicate { .. } => "duplicate",
            HostStateView::Retired { .. } => "retired",
        }
    }
}

/// How a connected host's most recent cache refresh went.
///
/// Beside the connection rather than inside it, mirroring
/// [`RefreshHealth`]: a failed refresh does not disconnect a host, and
/// collapsing the two would make a host that is answering perfectly well
/// read as unreachable.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "status")]
pub(crate) enum RefreshView {
    /// Connected, first refresh still in flight.
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "ok")]
    Ok { sessions: usize },
    /// The last refresh failed; the PREVIOUS cache is still in place, so
    /// this host's rows are still listed.
    #[serde(rename = "failed")]
    Failed { error: String },
}

impl From<&RefreshHealth> for RefreshView {
    fn from(health: &RefreshHealth) -> RefreshView {
        match health {
            RefreshHealth::Pending => RefreshView::Pending,
            RefreshHealth::Ok { sessions } => RefreshView::Ok {
                sessions: *sessions,
            },
            RefreshHealth::Failed { error } => RefreshView::Failed {
                error: error.clone(),
            },
        }
    }
}

impl From<&HostState> for HostStateView {
    fn from(state: &HostState) -> HostStateView {
        match state {
            HostState::Connecting {
                attempt,
                last_error,
            } => HostStateView::Connecting {
                attempt: *attempt,
                last_error: last_error.clone(),
            },
            HostState::Unreachable { cause, last_error } => HostStateView::Unreachable {
                cause: match cause {
                    UnreachableCause::LocalSupervisorNotRunning => "local-supervisor-not-running",
                    UnreachableCause::TransportFailure => "transport-failure",
                },
                last_error: last_error.clone(),
            },
            HostState::Connected {
                identity,
                build_version,
                last_refresh,
            } => HostStateView::Connected {
                identity: identity.clone(),
                build_version: build_version.clone(),
                refresh: last_refresh.into(),
            },
            HostState::VersionSkew {
                peer_protocol,
                peer_build,
                our_protocol,
                our_build,
                remediation,
            } => HostStateView::VersionSkew {
                peer_protocol: *peer_protocol,
                peer_build: peer_build.clone(),
                our_protocol: *our_protocol,
                our_build: our_build.clone(),
                remediation: remediation.clone(),
            },
            HostState::IdentityMismatch { recorded, reported } => HostStateView::IdentityMismatch {
                recorded: recorded.clone(),
                reported: reported.clone(),
            },
            HostState::IdentityUnverified { recorded } => HostStateView::IdentityUnverified {
                recorded: recorded.clone(),
            },
            HostState::Duplicate { twin, identity } => HostStateView::Duplicate {
                twin: *twin,
                identity: identity.clone(),
            },
            HostState::Retired { reason } => HostStateView::Retired {
                reason: reason.clone(),
            },
        }
    }
}

/// Join the manager's live snapshots with helm.db's registry rows into the
/// list `GET /api/hosts` answers with — see the module docs for why the
/// join exists at all.
///
/// Driven by the SNAPSHOTS, not by the registry read: the snapshot set is
/// the set of hosts that have an actor, which is what every other surface
/// (routing, the merged list) also means by "a host". A registry row with
/// no actor yet is one reconcile away and would otherwise appear with no
/// state to show.
///
/// `pub(crate)` for the agent relay (`crate::agent_requests`), which
/// answers an agent's `hosts` verb from this exact listing rather than a
/// second one of its own — the point of routing an agent's questions
/// through the helm is that the agent and the user see one fleet, and two
/// assemblies would eventually disagree. It takes `&AppState` and no axum
/// extractor, so there is nothing to unwrap for a non-HTTP caller.
pub(crate) async fn host_views(state: &AppState) -> anyhow::Result<Vec<HostView>> {
    let rows: std::collections::HashMap<HostId, HostRow> = state
        .store
        .list_hosts()
        .await?
        .into_iter()
        .map(|row| (row.id, row))
        .collect();
    Ok(state
        .manager
        .snapshots()
        .into_iter()
        .map(|snapshot| {
            let registry = rows.get(&snapshot.id);
            HostView {
                id: snapshot.id,
                kind: match snapshot.kind {
                    HostKind::Local => "local",
                    HostKind::Ssh => "ssh",
                },
                name: host_display_name(snapshot.kind, snapshot.destination.as_deref()),
                destination: snapshot.destination.clone(),
                identity: registry.and_then(|row| row.host_identity.clone()),
                remote_farhelm: registry.and_then(|row| row.remote_farhelm.clone()),
                remote_state_dir: registry.and_then(|row| row.remote_state_dir.clone()),
                state: (&snapshot.state).into(),
                incarnation: snapshot.incarnation,
            }
        })
        .collect())
}

/// One host's view by id, or a 404-shaped error — the reply every mutation
/// that returns a host renders through, so add and retarget answer in
/// exactly the shape a list row has.
async fn host_view(state: &AppState, host: HostId) -> anyhow::Result<HostView> {
    host_views(state)
        .await?
        .into_iter()
        .find(|view| view.id == host)
        .ok_or_else(|| anyhow::Error::new(HostStoreError::HostNotFound(host)))
}

/// `GET /api/hosts` — every registered host with its live connection state.
///
/// The whole registry every time, never paginated: SPEC.md's promise is
/// that per-host connection state is ALWAYS visible, and a fleet whose host
/// count needed paging is not a fleet a person is managing by hand.
pub(crate) async fn list_hosts(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match host_views(&state).await {
        Ok(hosts) => axum::Json(serde_json::json!({ "hosts": hosts })).into_response(),
        Err(e) => http_error(e),
    }
}

/// The body of `POST /api/hosts` and `POST /api/hosts/{id}/destination`.
///
/// `ssh` rather than `destination` deliberately: this is byte-for-byte the
/// entry shape `--ensure-hosts` files carry (`crate::ensure`), and the two
/// paths registering hosts by two different spellings of the same field
/// would be a needless second vocabulary for one concept.
///
/// The two optional fields are meaningful only on add. A retarget carries
/// just `ssh` — the remote binary and state directory are properties of the
/// INSTALL, not of the address it is reached at, so retargeting a host to a
/// new address deliberately keeps them (see `set_destination`).
/// `deny_unknown_fields` for the same reason the ensure file's entries have
/// it: every field here decides how a host is REACHED, so a typo that is
/// silently ignored produces a registry row that dials the wrong thing (or
/// the right thing the wrong way) and reports success. The two paths
/// register hosts from the same shape and now refuse the same mistakes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HostSpec {
    pub(crate) ssh: String,
    #[serde(default)]
    pub(crate) remote_farhelm: Option<String>,
    #[serde(default)]
    pub(crate) remote_state_dir: Option<String>,
}

/// `POST /api/hosts` — register an ssh destination.
///
/// Always registers (module docs); the reply's `state` is where "there was
/// nothing there" shows up, as an ordinary connecting-then-unreachable
/// host rather than as a failed request. The one refusal is a destination
/// that is not usable AS a destination — empty, option-shaped, or carrying
/// a NUL — which the store rejects before writing anything (400), and a
/// destination another row already holds (409).
///
/// Reconciles before replying so the new row already has an actor and
/// therefore a state to report. That is a synchronous reconcile, not a
/// connection: a host that is down does not delay this reply.
///
/// FAILS CLOSED. The registry write commits first and the live actors have
/// to follow, so a reconcile that fails would otherwise leave a registered
/// host with no actor — invisible in `/api/hosts` (which is driven by the
/// actor set), un-dialed, and indistinguishable from a host that was never
/// added, while a re-add of the same destination is refused as a duplicate.
/// The row is therefore rolled back. A rollback that ALSO fails is reported
/// as such rather than papered over: at that point the database itself is
/// refusing writes and the honest thing is to name the divergence and the
/// remedy.
pub(crate) async fn add_host(
    State(state): State<Arc<AppState>>,
    axum::Json(spec): axum::Json<HostSpec>,
) -> impl IntoResponse {
    let added = state
        .store
        .add_ssh_host(
            &spec.ssh,
            spec.remote_farhelm.as_deref(),
            spec.remote_state_dir.as_deref(),
        )
        .await;
    let host = match added {
        Ok(host) => host,
        Err(e) => return http_error(e),
    };
    if let Err(reconcile) = state.manager.sync_registry().await {
        return match state.store.remove_ssh_host(host).await {
            Ok(()) => http_error(reconcile.context(
                "the host was not registered: its actor could not be started, so the registry                  entry was rolled back",
            )),
            Err(rollback) => http_error(reconcile.context(format!(
                "host {host} is registered but has no connection actor, and the entry could not                  be rolled back either ({rollback:#}); restart the helm to reconcile them"
            ))),
        };
    }
    tracing::info!(host, destination = spec.ssh.as_str(), "host registered");
    match host_view(&state, host).await {
        Ok(view) => axum::Json(view).into_response(),
        Err(e) => http_error(e),
    }
}

/// `POST /api/hosts/{id}/destination` — retarget a registered host.
///
/// A verb-shaped route over the field being changed, following the same
/// reasoning `POST /api/sessions/{id}/rename` records: there is exactly one
/// field to change, and a partial-PATCH shape would invent an update
/// vocabulary this API has nowhere else. The body is [`HostSpec`], of which
/// only `ssh` is read — the remote binary and state directory describe the
/// INSTALL, not the address it is reached at, so retargeting keeps them.
///
/// The reconcile that follows is what makes the edit TAKE: it tears down
/// the connection to the old address and hands the actor a fresh
/// active-retry window against the new one. Without it a user fixing a
/// wrong destination would watch their fix do nothing until the old
/// connection happened to drop.
///
/// FAILS CLOSED, by CONVERGING rather than rolling back — the asymmetry with
/// `add_host` is deliberate. The durable write here is what the user asked
/// for and is already correct; the only thing at risk is an actor still
/// dialing the old address. `retry_now` fixes exactly that and cannot fail
/// (it reads no registry of its own — the ACTOR reloads its row at its own
/// boundary), so the connection converges on the registry even when the
/// reconcile did not. What is left over is cosmetic and self-correcting: the
/// hosts list may show the old destination until the next successful
/// reconcile, which the error says.
///
/// ## The retarget and this host's own writers are one queue
///
/// The registry write and the reconcile that invalidates every outstanding
/// connection claim are taken under this host's cache-write lock
/// (`ConnectionManager::host_write_lock`), the lock the host's own cache
/// writers hold, so a cache write in flight for the old connection cannot
/// straddle the move. Provisioning retains the same lock for its whole
/// confirmed run, so a retarget also cannot move the registry out from under
/// a frozen host plan. The remembered default profile is NOT touched by a
/// retarget — it is a bare id per registry row (SPEC.md, Sessions /
/// Creation).
///
/// Refuses the reserved local row (409 — it has no destination to change),
/// an unknown id (404), an unusable destination (400), and a destination
/// another host holds (409).
pub(crate) async fn set_destination(
    State(state): State<Arc<AppState>>,
    AxPath(host): AxPath<HostId>,
    axum::Json(spec): axum::Json<HostSpec>,
) -> impl IntoResponse {
    // Held across the write AND the reconcile: see this function's docs for
    // the in-flight write it fences out. Dropped before the reply is built,
    // which needs nothing from it.
    let serialized = state.manager.host_write_lock(host).await;
    if let Err(e) = state.store.update_ssh_destination(host, &spec.ssh).await {
        return http_error(e);
    }
    if let Err(reconcile) = state.manager.sync_registry().await {
        // Converge rather than roll back: the durable write is what the
        // user asked for, and `retry_now` makes the ACTOR reload its row and
        // dial whatever the registry says now.
        //
        // Its failures are reported rather than swallowed. `unwrap_or(false)`
        // read as "it either worked or it did not, never mind which", which
        // is exactly the wrong posture on the path that exists to guarantee
        // convergence: a revival that failed because the registry could not
        // be read leaves the actor pointed at the OLD address, and saying so
        // is the difference between a user who retries and a user who trusts
        // a stale hosts list.
        let converged = match state.manager.retry_now(host).await {
            Ok(found) => format!("reconnect requested: {found}"),
            Err(error) => format!("the reconnect could not be requested either ({error:#})"),
        };
        return http_error(reconcile.context(format!(
            "host {host}'s destination was changed, but its live connection could not be \
             reconciled ({converged}); the hosts list may keep showing the old destination until \
             the next successful reconcile"
        )));
    }
    // Released here rather than at the end of the function: the reply is
    // built from the registry and the actor set, both already settled, and
    // holding a host's write lock across a read would stall its next refresh
    // commit for no reason.
    drop(serialized);
    tracing::info!(host, destination = spec.ssh.as_str(), "host retargeted");
    match host_view(&state, host).await {
        Ok(view) => axum::Json(view).into_response(),
        Err(e) => http_error(e),
    }
}

/// `DELETE /api/hosts/{id}` — forget a registered host.
///
/// SPEC.md's remove-merely-forgets contract: the row and its cached
/// sessions go, the actor stops, and nothing about the remote install is
/// touched — re-adding the same destination later rediscovers everything
/// under a fresh host id. Refuses the reserved local row (409) and an
/// unknown id (404).
///
/// FAILS CLOSED through an INFALLIBLE teardown rather than a reconcile.
/// `stop_actor` takes the id this handler already committed and needs no
/// registry read, so it cannot fail — which matters because the general
/// reconcile can, and a failure there would leave an actor dialing a host
/// the registry no longer has, with its cached sessions already cascaded
/// away. There is no rollback to consider in the other direction: the row
/// is gone, and that is what the user asked for.
///
/// Same empty-object success body as the session verbs, so a caller never
/// has to special-case a bodiless response.
///
/// Removal takes the same host write authority as provisioning. Once the
/// confirmed run releases it, removal deletes the row and purges that host's
/// retained progress, confirmation ids, and detached-task handle together.
pub(crate) async fn remove_host(
    State(state): State<Arc<AppState>>,
    AxPath(host): AxPath<HostId>,
) -> impl IntoResponse {
    let serialized = state.manager.host_write_lock(host).await;
    if let Err(e) = state.store.remove_ssh_host(host).await {
        return http_error(e);
    }
    state.provisioning.forget_host(host).await;
    let stopped = state.manager.stop_actor(host).await;
    drop(serialized);
    tracing::info!(
        host,
        stopped,
        "host removed from the registry and its connection actor stopped"
    );
    axum::Json(serde_json::json!({})).into_response()
}

/// The body of `POST /api/hosts/{id}/adopt`: the identity the user was
/// actually shown when they approved the adoption.
///
/// Not optional, and not merely a confirmation checkbox. An empty-bodied
/// adopt means "adopt whatever this host is reporting right now", which is a
/// different promise than the one the UI made: a re-probe landing between
/// the mismatch being displayed and this request arriving can change the
/// reported identity, so a user shown "adopt install B?" would silently
/// adopt install C. Naming what was approved turns that race into a 409 the
/// client answers by asking again.
#[derive(Deserialize)]
pub(crate) struct AdoptReq {
    /// The `state.reported` value from the `HostView` the decision was made
    /// against.
    pub(crate) reported: String,
}

/// `POST /api/hosts/{id}/adopt` — accept the identity a mismatched host is
/// reporting, purging the superseded identity's cached sessions.
///
/// The user-decision half of SPEC.md's never-silently-merge rule: the helm
/// refuses to merge two installs on its own, and this is the explicit
/// acknowledgment that performs the merge the user chose. Only meaningful
/// for a host currently in `identity-mismatch`; anything else is a 409
/// naming the phase it is actually in, because adopting a host that is not
/// asking is a client bug rather than a no-op.
///
/// The body names the identity that was approved, and the check happens
/// under the manager's own lock (see [`AdoptReq`] and
/// `ConnectionManager::adopt`) — so there is no window between "what the
/// user was shown" and "what gets adopted".
///
/// Can also fail after being offered: a rival entry may claim the very
/// identity being adopted in the window between the mismatch being
/// displayed and this call arriving (409), and the row may have been
/// retargeted since (409). The correct client response to all of these is to
/// re-render the host, which now shows a different decision.
///
/// Every refusal is TYPED at the manager and mapped by `http_error`; this
/// handler makes no precheck of its own. It used to, and that was a race
/// wearing a status code: the check and the adoption were two separate lock
/// holds, so a phase change in between produced a 500 from a path that had
/// just been told the host was fine.
pub(crate) async fn adopt_host(
    State(state): State<Arc<AppState>>,
    AxPath(host): AxPath<HostId>,
    axum::Json(req): axum::Json<AdoptReq>,
) -> impl IntoResponse {
    match state.manager.adopt(host, &req.reported).await {
        Ok(()) => axum::Json(serde_json::json!({})).into_response(),
        Err(e) => http_error(e),
    }
}

/// `POST /api/hosts/{id}/retry` — reconnect this host now.
///
/// The "try again" control SPEC.md's always-visible connection state
/// implies, mapped onto the manager's `retry_now`: drop whatever connection
/// the host has, reload its registry row, and attempt it again from
/// scratch. Deliberately ONE attempt rather than a fresh retry ladder — a
/// user clicking retry is not evidence the host is back, and unfolding a
/// whole ladder per click is exactly the hammering the two-regime cadence
/// avoids.
///
/// A RETIRED host is the ONE documented exception to that one-attempt rule,
/// and it is the case that makes this verb load-bearing rather than a
/// convenience. Its actor is gone, so there is nothing to make one more
/// attempt WITH; retry respawns it from the current registry row, and the
/// fresh actor starts on the full active-retry ladder like any other. The
/// exception is a consequence of the rule rather than a softening of it:
/// the rule is about not hammering a host that is merely down, and a
/// retired entry is not attempting anything at all. Without it, an actor
/// that panicked left its host permanently dark behind a button that
/// visibly did nothing.
///
/// 404 for an unknown id, decided by the manager rather than by a
/// pre-check here: `retry_now` reports whether it found a host, so the two
/// cannot disagree about what "unknown" means.
pub(crate) async fn retry_host(
    State(state): State<Arc<AppState>>,
    AxPath(host): AxPath<HostId>,
) -> impl IntoResponse {
    match state.manager.retry_now(host).await {
        Ok(true) => {
            tracing::info!(host, "host reconnect requested");
            axum::Json(serde_json::json!({})).into_response()
        }
        Ok(false) => http_error(anyhow::Error::new(HostStoreError::HostNotFound(host))),
        Err(e) => http_error(e),
    }
}

#[cfg(test)]
mod tests {
    use super::{HostStateView, RefreshView};
    use crate::rest_harness::{self, FleetBuilder, Harness, HostScript};
    use axum::http::StatusCode;
    use serde_json::Value;
    use std::sync::Arc;

    /// Spec: [`HostStateView::phase`] answers, for every variant, exactly
    /// the string the serializer puts in the `phase` tag.
    ///
    /// This is the guard that lets `phase()` exist at all. Its docstring
    /// admits to being a third copy of one vocabulary — the manager's
    /// `HostState::phase`, the serde renames, and itself — and a copy is
    /// only safe while something compares it to the original. Without this
    /// test, a renamed phase would reach the browser under its new word and
    /// reach an agent under its old one, with both sides looking correct.
    #[test]
    fn phase_matches_the_serialized_tag() {
        for state in [
            HostStateView::Connecting {
                attempt: 1,
                last_error: None,
            },
            HostStateView::Unreachable {
                cause: "transport-failure",
                last_error: "no route".to_string(),
            },
            HostStateView::Connected {
                identity: None,
                build_version: "0.0.0".to_string(),
                refresh: RefreshView::Pending,
            },
            HostStateView::VersionSkew {
                peer_protocol: 1,
                peer_build: "0.0.0".to_string(),
                our_protocol: 2,
                our_build: "0.0.1".to_string(),
                remediation: "upgrade".to_string(),
            },
            HostStateView::IdentityMismatch {
                recorded: "a".to_string(),
                reported: "b".to_string(),
            },
            HostStateView::IdentityUnverified {
                recorded: "a".to_string(),
            },
            HostStateView::Duplicate {
                twin: 1,
                identity: "a".to_string(),
            },
            HostStateView::Retired {
                reason: "removed".to_string(),
            },
        ] {
            let encoded = serde_json::to_value(&state).unwrap();
            assert_eq!(
                encoded["phase"],
                state.phase(),
                "the phase word and the wire tag disagree for {state:?}"
            );
        }
    }

    /// Issue one request against the harness's real router and return the
    /// status plus the body — decoded as JSON where it is JSON, and as its
    /// own text where it is a refusal (which is prose, per SPEC.md's
    /// requirement that errors be readable).
    async fn call(
        harness: &Harness,
        method: &str,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value, String) {
        let mut builder = axum::http::Request::builder()
            .method(method)
            .uri(uri)
            .header("host", "127.0.0.1:7433");
        let request = match body {
            Some(body) => {
                builder = builder.header("content-type", "application/json");
                builder
                    .body(axum::body::Body::from(body.to_string()))
                    .unwrap()
            }
            None => builder.body(axum::body::Body::empty()).unwrap(),
        };
        let response = tower::ServiceExt::oneshot(harness.router(), request)
            .await
            .unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, json, text)
    }

    /// A helm with just its reserved local row, connected — the starting
    /// point for every host-management test, since add/edit/remove are all
    /// about what happens AROUND the local row.
    async fn lone_local_helm() -> Harness {
        let harness = FleetBuilder::new()
            .await
            .local(HostScript {
                identity: Some("identity-local".to_string()),
                ..HostScript::default()
            })
            .await
            .start()
            .await;
        let local = rest_harness::local_id(&harness.store).await;
        harness.await_refreshed(local).await;
        harness
    }

    /// The list must always contain the helm's own machine, named as
    /// itself, with no destination and no management affordances implied.
    ///
    /// SPEC.md: the machine running the helm is always in the list, "never
    /// a ghost, never needing registration". Asserted on the JSON because
    /// that is what a hosts panel decodes.
    #[tokio::test]
    async fn the_local_host_is_always_listed_and_carries_no_destination() {
        let harness = lone_local_helm().await;
        let (status, body, _) = call(&harness, "GET", "/api/hosts", None).await;
        assert_eq!(status, StatusCode::OK);

        let hosts = body["hosts"].as_array().expect("hosts is an array");
        assert_eq!(hosts.len(), 1, "a fresh registry has exactly the local row");
        let local = &hosts[0];
        assert_eq!(local["kind"], "local");
        assert_eq!(local["destination"], Value::Null);
        assert_eq!(local["name"], "this machine");
        assert_eq!(
            local["identity"], "identity-local",
            "the identity comes from the registry, which learns it at first contact"
        );
        assert_eq!(local["state"]["phase"], "connected");
        assert_eq!(
            local["state"]["refresh"]["status"], "ok",
            "a connected host reports how its last cache refresh went"
        );
    }

    /// The hosts view publishes the CONNECTION token, and a reconnection
    /// changes it.
    ///
    /// Spec: every host row carries `incarnation`, equal to what the manager
    /// has published for that host, and a host that drops and comes back
    /// carries a larger one.
    ///
    /// The UI's provisioning panel binds a displayed plan to this token so a
    /// plan prepared against one connection is not left actionable on the
    /// next; the change-on-reconnect half is what makes that binding mean
    /// anything.
    #[tokio::test]
    async fn the_hosts_view_publishes_the_connection_token() {
        let harness = lone_local_helm().await;
        let local = rest_harness::local_id(&harness.store).await;

        let (status, body, _) = call(&harness, "GET", "/api/hosts", None).await;
        assert_eq!(status, StatusCode::OK);
        let before = body["hosts"][0]["incarnation"]
            .as_u64()
            .expect("every host row carries the connection it is on");
        assert_eq!(
            before,
            harness
                .manager
                .status(local)
                .expect("the local host has an actor")
                .incarnation,
            "the published number is the one a claim is validated against, not a second counter"
        );
        assert!(
            before > 0,
            "a connected host is on some connection; 0 means never connected"
        );

        harness.fleet.kill_connection(local);
        harness.await_refreshed_after(local, before).await;

        let (_, body, _) = call(&harness, "GET", "/api/hosts", None).await;
        let after = body["hosts"][0]["incarnation"]
            .as_u64()
            .expect("still a number");
        assert!(
            after > before,
            "a reconnection is a different connection ({before} then {after}), and a client \
             holding the old number must be refused rather than served"
        );
    }

    /// Adding a host ALWAYS registers it, and the connection state — not
    /// the request's status — is what says whether anything answered.
    ///
    /// This is the property that makes the registry meaningful for a
    /// machine that is simply switched off when it is added, and it is what
    /// `--ensure-hosts` depends on: a startup floor that only registered
    /// reachable hosts would guarantee nothing.
    #[tokio::test]
    async fn adding_a_host_that_is_down_still_registers_it() {
        let harness = lone_local_helm().await;
        // Down BEFORE the POST, which is the case that matters: a host
        // scripted down only afterwards would have connected first, so the
        // registration would prove nothing about a machine that was
        // switched off at the moment it was added. The id is not known
        // until the POST answers, so the script is planted against the id
        // the registry is about to mint.
        harness.fleet.take_down_next_host();

        let (status, body, text) = call(
            &harness,
            "POST",
            "/api/hosts",
            Some(serde_json::json!({ "ssh": "user@switched-off" })),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "a host that answers nothing is registered anyway: {text}"
        );
        let id = body["id"].as_i64().expect("the new host's id");
        assert_eq!(body["kind"], "ssh");
        assert_eq!(body["destination"], "user@switched-off");
        assert_eq!(
            body["name"], "user@switched-off",
            "an ssh host is named by the destination it was registered at"
        );
        assert_eq!(
            body["identity"],
            Value::Null,
            "a host that has never been reached has no identity to report"
        );

        harness
            .await_state(id, |state| state.phase() == "unreachable-reprobing")
            .await;
        let (_, body, _) = call(&harness, "GET", "/api/hosts", None).await;
        let added = body["hosts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|host| host["id"] == id)
            .expect("the added host is in the list");
        assert_eq!(added["state"]["phase"], "unreachable-reprobing");
        assert!(
            added["state"]["last_error"].is_string(),
            "an unreachable host carries the transport's own words: {added}"
        );
    }

    /// The registry's refusals must reach the caller as the statuses their
    /// meanings imply, not all as one generic failure.
    ///
    /// Each of these is a different thing for a UI to say: fix what you
    /// typed (400), that address is already registered (409), that row
    /// cannot be changed at all (409), there is no such host (404). Pinned
    /// together because they share one mapping (`http_error`'s
    /// `HostStoreError` arm) and a regression there would move all of them
    /// at once.
    #[tokio::test]
    async fn host_management_refusals_map_to_their_own_statuses() {
        let harness = lone_local_helm().await;
        let local = rest_harness::local_id(&harness.store).await;
        let (_, first, _) = call(
            &harness,
            "POST",
            "/api/hosts",
            Some(serde_json::json!({ "ssh": "user@taken" })),
        )
        .await;
        let taken = first["id"].as_i64().unwrap();

        // An option-shaped destination is refused at registration rather
        // than becoming a host that can never connect.
        for unusable in [
            // Option-shaped: ssh's own getopt would read it as an option.
            "-oProxyCommand=touch /tmp/pwned",
            "",
            // Embedded NUL: argv is NUL-terminated, so a destination
            // carrying one is either an opaque spawn failure or — worse —
            // silently truncated, registering one address and dialing
            // another.
            "good.example\u{0}evil.example",
        ] {
            let (status, _, text) = call(
                &harness,
                "POST",
                "/api/hosts",
                Some(serde_json::json!({ "ssh": unusable })),
            )
            .await;
            assert_eq!(
                status,
                StatusCode::BAD_REQUEST,
                "{unusable:?} must be refused at the registry boundary: {text}"
            );
        }

        // The same rule on the EDIT path: a registry that validated only
        // one of its two writers would be validating nothing.
        let (status, _, text) = call(
            &harness,
            "POST",
            &format!("/api/hosts/{taken}/destination"),
            Some(serde_json::json!({ "ssh": "good.example\u{0}evil.example" })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");

        let (status, _, text) = call(
            &harness,
            "POST",
            "/api/hosts",
            Some(serde_json::json!({ "ssh": "user@taken" })),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{text}");

        // The reserved local row is not user management surface at all.
        let (status, _, text) =
            call(&harness, "DELETE", &format!("/api/hosts/{local}"), None).await;
        assert_eq!(status, StatusCode::CONFLICT, "{text}");
        let (status, _, text) = call(
            &harness,
            "POST",
            &format!("/api/hosts/{local}/destination"),
            Some(serde_json::json!({ "ssh": "user@nope" })),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{text}");

        // Every verb that names a host must answer 404 for an id nothing
        // holds — a well-formed request against a host that is not there,
        // which is a different thing from a malformed one. Adopt carries a
        // well-formed body here for exactly that reason: its extractor runs
        // before the handler, so an empty body would test the extractor
        // rather than the lookup.
        for (method, uri, body) in [
            ("DELETE", "/api/hosts/9999".to_string(), None),
            (
                "POST",
                "/api/hosts/9999/adopt".to_string(),
                Some(serde_json::json!({ "reported": "anything" })),
            ),
            ("POST", "/api/hosts/9999/retry".to_string(), None),
        ] {
            let (status, _, text) = call(&harness, method, &uri, body).await;
            assert_eq!(
                status,
                StatusCode::NOT_FOUND,
                "{method} {uri} must 404 for a host nothing holds: {text}"
            );
        }

        // Retry on a host that DOES exist is accepted — the 404s above are
        // about the id, not about the verb being unimplemented.
        let (status, _, text) =
            call(&harness, "POST", &format!("/api/hosts/{taken}/retry"), None).await;
        assert_eq!(status, StatusCode::OK, "{text}");
    }

    /// Retargeting a host must take effect on the CONNECTION, not just in
    /// the registry.
    ///
    /// A user fixing a mistyped destination and watching nothing happen
    /// until the old connection incidentally drops is the failure this
    /// pins against: the edit tears the current connection down and dials
    /// the new address.
    #[tokio::test]
    async fn retargeting_a_host_reconnects_it_to_the_new_destination() {
        let harness = lone_local_helm().await;
        let (_, added, _) = call(
            &harness,
            "POST",
            "/api/hosts",
            Some(serde_json::json!({ "ssh": "user@typo" })),
        )
        .await;
        let host = added["id"].as_i64().unwrap();
        harness.await_refreshed(host).await;

        let (status, body, text) = call(
            &harness,
            "POST",
            &format!("/api/hosts/{host}/destination"),
            Some(serde_json::json!({ "ssh": "user@correct" })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{text}");
        assert_eq!(body["destination"], "user@correct");
        assert_eq!(
            body["name"], "user@correct",
            "the displayed name follows the destination, with no separate naming surface"
        );
        harness.await_refreshed(host).await;

        // The assertion that actually distinguishes "retargeted" from
        // "reconnected to the same place": states and identities look
        // identical either way, so only the DIALED destinations can tell a
        // manager that honoured the edit from one that did not.
        let dialed: Vec<Option<String>> = harness
            .fleet
            .dialed_rows(host)
            .into_iter()
            .map(|row| row.destination)
            .collect();
        assert_eq!(
            dialed.first().and_then(|d| d.as_deref()),
            Some("user@typo"),
            "the first dial went to the address the host was registered at"
        );
        assert_eq!(
            dialed.last().and_then(|d| d.as_deref()),
            Some("user@correct"),
            "the edit must be DIALED, not merely recorded: {dialed:?}"
        );
    }

    /// Removing a host forgets it AND its cached sessions, while the host
    /// itself is untouched — SPEC.md's remove-merely-forgets contract, as
    /// the REST layer expresses it.
    ///
    /// The cache half is the part worth asserting: a removal that dropped
    /// the registry row but left cache rows behind would leave sessions in
    /// the merged list with no host to name.
    #[tokio::test]
    async fn removing_a_host_forgets_it_and_its_cached_sessions() {
        let harness = lone_local_helm().await;
        let (_, added, _) = call(
            &harness,
            "POST",
            "/api/hosts",
            Some(serde_json::json!({ "ssh": "user@doomed" })),
        )
        .await;
        let host = added["id"].as_i64().unwrap();
        harness.fleet.edit(host, |script| {
            script.identity = Some("identity-doomed".to_string());
            script.sessions = vec![rest_harness::session("doomed-1", 100)];
        });
        harness
            .manager
            .retry_now(host)
            .await
            .expect("the host exists");
        // The INTENDED refresh, not merely a refreshed state: this host
        // already connected once under the default script (no identity, no
        // sessions), and waiting for the loose condition would let the
        // assertion below run against that empty first pass.
        harness.await_refreshed_as(host, "identity-doomed", 1).await;

        let (_, sessions, _) = call(&harness, "GET", "/api/sessions", None).await;
        assert_eq!(sessions["total"], 1, "the host's session is listed first");

        let (status, _, text) = call(&harness, "DELETE", &format!("/api/hosts/{host}"), None).await;
        assert_eq!(status, StatusCode::OK, "{text}");

        let (_, hosts, _) = call(&harness, "GET", "/api/hosts", None).await;
        assert_eq!(
            hosts["hosts"].as_array().unwrap().len(),
            1,
            "only the local row remains"
        );
        let (_, sessions, _) = call(&harness, "GET", "/api/sessions", None).await;
        assert_eq!(
            sessions["total"], 0,
            "the removed host's cached sessions went with it"
        );
    }

    /// A host reinstalled at a known address must FREEZE awaiting a
    /// decision, surface both identities, and resolve only when the user
    /// adopts — at which point the dead install's cached sessions are gone.
    ///
    /// SPEC.md forbids silently merging two installs, and adopt is the
    /// explicit acknowledgment that performs the merge the user chose. The
    /// purge is the part that would be easy to get wrong and hard to
    /// notice: carrying the old identity's sessions forward would
    /// misattribute one install's history to another.
    #[tokio::test]
    async fn adopting_resolves_an_identity_mismatch_and_purges_the_old_cache() {
        let harness = lone_local_helm().await;
        let (_, added, _) = call(
            &harness,
            "POST",
            "/api/hosts",
            Some(serde_json::json!({ "ssh": "user@reinstalled" })),
        )
        .await;
        let host = added["id"].as_i64().unwrap();
        harness.fleet.edit(host, |script| {
            script.identity = Some("identity-before".to_string());
            script.sessions = vec![rest_harness::session("from-the-old-install", 100)];
        });
        harness
            .manager
            .retry_now(host)
            .await
            .expect("the host exists");
        // The pre-reinstall state has to be REACHED before it is broken —
        // see `await_refreshed_as`. Breaking it while the host was still on
        // its default script would leave nothing cached to purge, which is
        // the half of this test that matters.
        harness.await_refreshed_as(host, "identity-before", 1).await;

        // The machine is wiped and reinstalled: same address, new identity.
        harness.fleet.edit(host, |script| {
            script.identity = Some("identity-after".to_string());
            script.sessions = vec![rest_harness::session("from-the-new-install", 200)];
        });
        harness.fleet.kill_connection(host);
        harness
            .await_state(host, |state| state.phase() == "identity-mismatch")
            .await;

        let (_, hosts, _) = call(&harness, "GET", "/api/hosts", None).await;
        let frozen = &hosts["hosts"].as_array().unwrap()[1];
        assert_eq!(frozen["state"]["phase"], "identity-mismatch");
        assert_eq!(frozen["state"]["recorded"], "identity-before");
        assert_eq!(
            frozen["state"]["reported"], "identity-after",
            "both identities are named, because the decision needs both"
        );

        let (status, _, text) = call(
            &harness,
            "POST",
            &format!("/api/hosts/{host}/adopt"),
            Some(serde_json::json!({ "reported": "identity-after" })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{text}");
        harness.await_refreshed_as(host, "identity-after", 1).await;

        let (_, sessions, _) = call(&harness, "GET", "/api/sessions", None).await;
        let ids: Vec<&str> = sessions["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["id"].as_str().unwrap())
            .collect();
        assert_eq!(
            ids,
            vec!["from-the-new-install"],
            "the superseded identity's cached sessions describe a dead install and must be gone"
        );
    }

    /// The adoption's invalidation arrives AFTER the state it invalidates
    /// has been settled, never before.
    ///
    /// Spec: by the time `/api/events` carries the bump an adoption
    /// publishes, the host's own state has already stopped being an identity
    /// mismatch — so a client that re-reads on that notification cannot see
    /// the newly adopted identity sitting beside an offer to adopt it.
    ///
    /// The ordering is the entire content of this test, and the bug it
    /// closes is a loop rather than a cosmetic glitch: the top-level identity
    /// is read from the registry (already updated by the commit) while the
    /// phase comes from the actor (not yet touched), so a client woken by the
    /// bump would render "this host reports identity-after, which you have
    /// not accepted" and offer the adopt verb again — for an adoption that
    /// has already happened. Pressing it is refused, because the host is no
    /// longer mismatched, so the offer is one the UI cannot honour.
    ///
    /// Observed from INSIDE the bump rather than from a subscriber, and that
    /// is not a shortcut. There is no await point between the state publish
    /// and the bump — which is exactly what makes the ordering safe — so a
    /// woken subscriber is polled after both have happened and cannot tell
    /// the two orders apart. See `manager::FleetEvents::on_bump`.
    #[tokio::test]
    async fn an_adoption_settles_the_hosts_state_before_it_invalidates() {
        let harness = lone_local_helm().await;
        let (_, added, _) = call(
            &harness,
            "POST",
            "/api/hosts",
            Some(serde_json::json!({ "ssh": "user@reinstalled" })),
        )
        .await;
        let host = added["id"].as_i64().unwrap();
        harness.fleet.edit(host, |script| {
            script.identity = Some("identity-before".to_string());
        });
        harness
            .manager
            .retry_now(host)
            .await
            .expect("the host exists");
        harness.await_refreshed_as(host, "identity-before", 0).await;

        harness.fleet.edit(host, |script| {
            script.identity = Some("identity-after".to_string());
        });
        harness.fleet.kill_connection(host);
        harness
            .await_state(host, |state| state.phase() == "identity-mismatch")
            .await;

        // Every phase published to the feed from here on, recorded at the
        // instant the invalidation goes out. A `Weak` because the closure
        // outlives nothing and the manager owns the events it is installed
        // on — a strong handle would be a cycle that never drops.
        let seen: std::sync::Arc<std::sync::Mutex<Vec<String>>> = Default::default();
        harness.manager.events().observe_bumps({
            let manager = std::sync::Arc::downgrade(&harness.manager);
            let seen = std::sync::Arc::clone(&seen);
            move || {
                if let Some(manager) = manager.upgrade()
                    && let Some(status) = manager.status(host)
                {
                    seen.lock()
                        .expect("phase log poisoned")
                        .push(status.state.phase().to_string());
                }
            }
        });

        let (status, _, text) = call(
            &harness,
            "POST",
            &format!("/api/hosts/{host}/adopt"),
            Some(serde_json::json!({ "reported": "identity-after" })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{text}");

        let seen = seen.lock().expect("phase log poisoned").clone();
        assert!(
            !seen.is_empty(),
            "the adoption must publish its own invalidation rather than leaving it to the \
             reconnect"
        );
        assert!(
            !seen.contains(&"identity-mismatch".to_string()),
            "the state a client is woken to re-read must not still be offering the adoption it \
             was just told about: {seen:?}"
        );
    }

    /// Adopting a host that is not asking for a decision is a client bug,
    /// not a no-op: it must be refused with the phase the host is actually
    /// in, so a UI that fired the wrong verb learns which one.
    #[tokio::test]
    async fn adopting_a_host_that_is_not_mismatched_is_refused_naming_its_phase() {
        let harness = lone_local_helm().await;
        let local = rest_harness::local_id(&harness.store).await;
        let (status, _, text) = call(
            &harness,
            "POST",
            &format!("/api/hosts/{local}/adopt"),
            Some(serde_json::json!({ "reported": "whatever" })),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{text}");
        assert!(
            text.contains("connected"),
            "the refusal must name the phase the host is really in: {text}"
        );
    }

    /// Adopting must name the identity the user was SHOWN, and a host that
    /// has since started reporting a different one refuses.
    ///
    /// The REST half of the manager's own superseded-adoption check. PR 7's
    /// UI builds against this body, so the contract — that an absent or
    /// stale `reported` never silently adopts whatever is current — belongs
    /// here as well as one layer down.
    #[tokio::test]
    async fn adopting_without_the_displayed_identity_is_refused() {
        let harness = lone_local_helm().await;
        let (_, added, _) = call(
            &harness,
            "POST",
            "/api/hosts",
            Some(serde_json::json!({ "ssh": "user@reinstalled" })),
        )
        .await;
        let host = added["id"].as_i64().unwrap();
        harness.fleet.edit(host, |script| {
            script.identity = Some("identity-before".to_string());
        });
        harness.manager.retry_now(host).await.expect("host exists");
        harness.await_refreshed_as(host, "identity-before", 0).await;
        harness.fleet.edit(host, |script| {
            script.identity = Some("identity-after".to_string());
        });
        harness.fleet.kill_connection(host);
        harness
            .await_state(host, |state| state.phase() == "identity-mismatch")
            .await;

        let (status, _, text) = call(
            &harness,
            "POST",
            &format!("/api/hosts/{host}/adopt"),
            Some(serde_json::json!({ "reported": "identity-the-user-saw-earlier" })),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{text}");
        assert!(
            text.contains("identity-after"),
            "the refusal must name what the host reports now, so the client can re-ask: {text}"
        );

        // A body missing the field entirely is a decode failure, never a
        // silent "adopt whatever is current".
        let (status, _, _) = call(
            &harness,
            "POST",
            &format!("/api/hosts/{host}/adopt"),
            Some(serde_json::json!({})),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "an adopt with no approved identity must not be accepted at all"
        );
    }

    /// A retired host must come back through RETRY, and retry must be an
    /// immediate attempt rather than an early wake-up.
    ///
    /// Retirement is what an actor's task ENDING looks like — it panicked,
    /// or its row was pulled out from under it — and nothing in the manager
    /// restarts one on a timer, because a timer cannot resurrect a task. So
    /// without this, an actor that died left its host permanently dark
    /// behind a control that visibly did nothing. The immediacy half is
    /// asserted by pushing the re-probe cadence an hour out: anything that
    /// reconnects within the test's own patience did so because retry made
    /// it happen.
    #[tokio::test]
    async fn retrying_a_retired_host_respawns_its_actor_and_reconnects() {
        let harness = lone_local_helm().await;
        let (_, added, _) = call(
            &harness,
            "POST",
            "/api/hosts",
            Some(serde_json::json!({ "ssh": "user@doomed-actor" })),
        )
        .await;
        let host = added["id"].as_i64().unwrap();
        harness.fleet.edit(host, |script| {
            script.identity = Some("identity-doomed-actor".to_string());
        });
        harness.manager.retry_now(host).await.expect("host exists");
        harness
            .await_refreshed_as(host, "identity-doomed-actor", 0)
            .await;

        // Take the actor down the way a bug would: the transport panics on
        // the next dial, the task dies, and the entry stays in the map with
        // its last published status — which is exactly what `retired` is.
        harness
            .fleet
            .edit(host, |script| script.panic_on_dial = true);
        harness.fleet.kill_connection(host);
        harness
            .await_state(host, |state| state.phase() == "retired")
            .await;
        let (_, hosts, _) = call(&harness, "GET", "/api/hosts", None).await;
        let retired = hosts["hosts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == host)
            .expect("a retired entry stays visible rather than vanishing");
        assert_eq!(
            retired["state"]["phase"], "retired",
            "an entry with no actor must say so rather than reading connected"
        );

        // The bug is fixed (or was transient) and the user presses retry.
        harness
            .fleet
            .edit(host, |script| script.panic_on_dial = false);

        let (status, _, text) =
            call(&harness, "POST", &format!("/api/hosts/{host}/retry"), None).await;
        assert_eq!(status, StatusCode::OK, "{text}");
        harness
            .await_refreshed_as(host, "identity-doomed-actor", 0)
            .await;
    }

    /// Retry against a host that is merely UNREACHABLE must produce an
    /// attempt now, not at the next re-probe.
    ///
    /// The harness's re-probe interval is an hour, so an attempt observed
    /// here can only have come from the retry — which is the whole point of
    /// the control existing beside an always-visible connection state.
    #[tokio::test]
    async fn retrying_an_unreachable_host_dials_immediately() {
        let harness = lone_local_helm().await;
        let (_, added, _) = call(
            &harness,
            "POST",
            "/api/hosts",
            Some(serde_json::json!({ "ssh": "user@down" })),
        )
        .await;
        let host = added["id"].as_i64().unwrap();
        harness.fleet.take_down(host);
        harness
            .await_state(host, |state| state.phase() == "unreachable-reprobing")
            .await;
        let before = harness.fleet.dials(host);

        // The host comes back, but nothing would notice for an hour.
        harness.fleet.edit(host, |script| {
            script.reachable = true;
            script.identity = Some("identity-back".to_string());
        });
        let (status, _, text) =
            call(&harness, "POST", &format!("/api/hosts/{host}/retry"), None).await;
        assert_eq!(status, StatusCode::OK, "{text}");

        harness.await_refreshed_as(host, "identity-back", 0).await;
        assert!(
            harness.fleet.dials(host) > before,
            "retry must dial, not merely shorten a wait"
        );
    }

    /// The optional registration fields must reach the DIAL, not just the
    /// row: a `remote_farhelm`/`remote_state_dir` that were recorded and
    /// then ignored would look identical in every API response and fail
    /// only against a real host.
    #[tokio::test]
    async fn a_hosts_remote_fields_reach_the_transport_that_dials_it() {
        let harness = lone_local_helm().await;
        let (status, body, text) = call(
            &harness,
            "POST",
            "/api/hosts",
            Some(serde_json::json!({
                "ssh": "user@remote",
                "remote_farhelm": "/opt/farhelm",
                "remote_state_dir": "/srv/state",
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{text}");
        let host = body["id"].as_i64().unwrap();
        assert_eq!(body["remote_farhelm"], "/opt/farhelm");
        assert_eq!(body["remote_state_dir"], "/srv/state");

        harness
            .await_state(host, |state| {
                !matches!(
                    state,
                    crate::manager::HostState::Connecting { attempt: 0, .. }
                )
            })
            .await;
        let dialed = harness
            .fleet
            .dialed_rows(host)
            .into_iter()
            .next()
            .expect("the host was dialed at least once");
        assert_eq!(dialed.destination.as_deref(), Some("user@remote"));
        assert_eq!(
            dialed.remote_farhelm.as_deref(),
            Some("/opt/farhelm"),
            "the registered remote binary must be what the transport is handed"
        );
        assert_eq!(dialed.remote_state_dir.as_deref(), Some("/srv/state"));
    }

    /// A version-skewed host's JSON must carry everything the remediation
    /// needs: both protocol numbers, both build strings, and the sentence
    /// telling the user what to do.
    ///
    /// SPEC.md requires errors to be actionable rather than merely
    /// diagnostic, and a chip that said only "version skew" would leave the
    /// user to guess WHICH side is behind. PR 7's host panel renders these
    /// fields directly, so their presence is a contract rather than a
    /// convenience.
    #[tokio::test]
    async fn a_version_skewed_host_reports_both_versions_and_its_remediation() {
        let harness = lone_local_helm().await;
        let (_, added, _) = call(
            &harness,
            "POST",
            "/api/hosts",
            Some(serde_json::json!({ "ssh": "user@ancient" })),
        )
        .await;
        let host = added["id"].as_i64().unwrap();
        harness.fleet.edit(host, |script| {
            script.protocol = farhelm_proto::PROTOCOL_VERSION + 1;
            script.build = "peer-build-from-the-future".to_string();
        });
        harness.fleet.kill_connection(host);
        harness
            .await_state(host, |state| state.phase() == "version-skew")
            .await;

        let (_, hosts, _) = call(&harness, "GET", "/api/hosts", None).await;
        let skewed = hosts["hosts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == host)
            .expect("the skewed host is listed");
        let state = &skewed["state"];
        assert_eq!(state["phase"], "version-skew");
        assert_eq!(state["peer_protocol"], farhelm_proto::PROTOCOL_VERSION + 1);
        assert_eq!(state["our_protocol"], farhelm_proto::PROTOCOL_VERSION);
        assert_eq!(state["peer_build"], "peer-build-from-the-future");
        assert_eq!(state["our_build"], farhelm_proto::BUILD_VERSION);
        assert!(
            state["remediation"]
                .as_str()
                .is_some_and(|text| !text.is_empty()),
            "the state must carry the sentence a user acts on, not just the diagnosis: {state}"
        );
    }

    /// Editing a RETIRED host must bring it back, not wedge it.
    ///
    /// The shape this replaces overwrote the retired state with
    /// `connecting` and nudged a receiver that had already been dropped, so
    /// an edit to a host whose actor had died left it reading "connecting"
    /// forever — a phase that would never advance, in front of a retry
    /// control that could not help because the edit had already erased the
    /// evidence that a respawn was needed. The stuck state is now
    /// unconstructible: a reconcile that finds a dead handle respawns it,
    /// whether or not the row changed.
    #[tokio::test]
    async fn editing_a_retired_host_respawns_it_rather_than_wedging_it() {
        let harness = lone_local_helm().await;
        let (_, added, _) = call(
            &harness,
            "POST",
            "/api/hosts",
            Some(serde_json::json!({ "ssh": "user@will-die" })),
        )
        .await;
        let host = added["id"].as_i64().unwrap();
        harness.fleet.edit(host, |script| {
            script.identity = Some("identity-before-death".to_string());
        });
        harness.manager.retry_now(host).await.expect("host exists");
        harness
            .await_refreshed_as(host, "identity-before-death", 0)
            .await;

        // Its actor dies the way a bug would kill it.
        harness
            .fleet
            .edit(host, |script| script.panic_on_dial = true);
        harness.fleet.kill_connection(host);
        harness
            .await_state(host, |state| state.phase() == "retired")
            .await;

        // The user, seeing a dead host, fixes what they think is wrong: the
        // address. The edit must take AND revive.
        //
        // Same identity across the edit, deliberately: the same machine is
        // simply being reached at a corrected address, so the revived actor
        // must CONNECT rather than freeze — a changed identity here would
        // (correctly) produce a mismatch and test something else entirely.
        harness
            .fleet
            .edit(host, |script| script.panic_on_dial = false);
        let (status, body, text) = call(
            &harness,
            "POST",
            &format!("/api/hosts/{host}/destination"),
            Some(serde_json::json!({ "ssh": "user@corrected" })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{text}");
        assert_eq!(body["destination"], "user@corrected");

        harness
            .await_refreshed_as(host, "identity-before-death", 0)
            .await;
        assert_eq!(
            harness
                .fleet
                .dialed_rows(host)
                .last()
                .and_then(|row| row.destination.clone())
                .as_deref(),
            Some("user@corrected"),
            "the revived actor must dial the EDITED address, not the old one"
        );
    }

    /// A claim captured on one incarnation must not validate against a
    /// LATER one, across a retire, an edit, and a respawn.
    ///
    /// The per-actor counter this replaced reset to zero on every respawn,
    /// so a claim taken on a retired actor's Nth connection validated
    /// perfectly against its replacement's Nth — a delayed mutation reply
    /// filing a session against a host that had since died, been retargeted,
    /// and come back. The token is now drawn from a manager-wide counter
    /// that is never reused, and the write also re-checks that the map's
    /// entry is the very handle the claim was taken through.
    #[tokio::test]
    async fn a_claim_from_a_previous_incarnation_never_validates() {
        let harness = lone_local_helm().await;
        let (_, added, _) = call(
            &harness,
            "POST",
            "/api/hosts",
            Some(serde_json::json!({ "ssh": "user@reborn" })),
        )
        .await;
        let host = added["id"].as_i64().unwrap();
        harness.fleet.edit(host, |script| {
            script.identity = Some("identity-reborn".to_string());
        });
        harness.manager.retry_now(host).await.expect("host exists");
        harness.await_refreshed_as(host, "identity-reborn", 0).await;

        // The claim a mutation would have captured, taken now.
        let stale = crate::manager::SessionClaim {
            host,
            incarnation: harness.manager.status(host).expect("connected").incarnation,
            identity: Some("identity-reborn".to_string()),
        };

        // The actor dies, the row is edited, and a replacement comes up —
        // the exact sequence that used to hand the replacement the same
        // token the original had.
        harness
            .fleet
            .edit(host, |script| script.panic_on_dial = true);
        harness.fleet.kill_connection(host);
        harness
            .await_state(host, |state| state.phase() == "retired")
            .await;
        harness
            .fleet
            .edit(host, |script| script.panic_on_dial = false);
        let (status, _, text) = call(
            &harness,
            "POST",
            &format!("/api/hosts/{host}/destination"),
            Some(serde_json::json!({ "ssh": "user@reborn-elsewhere" })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{text}");
        harness.await_refreshed_as(host, "identity-reborn", 0).await;

        let error = harness
            .manager
            .remember_session(&stale, &rest_harness::session("from-a-past-life", 100))
            .await
            .expect_err("a claim from a previous incarnation must never validate");
        assert!(
            format!("{error:#}").contains("connection changed"),
            "the refusal must say why it declined: {error:#}"
        );
        assert!(
            harness
                .store
                .cached_sessions(host)
                .await
                .expect("cache read")
                .is_empty(),
            "and nothing may have been written"
        );
    }

    /// A retry racing a removal must leave no ghost: either the host is
    /// gone, or it is gone — never an actor dialing a host the registry has
    /// forgotten.
    ///
    /// The two paths share the reconcile discipline precisely so this
    /// interleaving has one outcome. Without it, a revival that read the
    /// registry just before a removal committed would install an actor into
    /// a map the removal had already drained: nothing would ever stop it,
    /// and nothing would ever show it.
    #[tokio::test]
    async fn a_retry_racing_a_removal_leaves_no_ghost() {
        let harness = lone_local_helm().await;
        let (_, added, _) = call(
            &harness,
            "POST",
            "/api/hosts",
            Some(serde_json::json!({ "ssh": "user@doomed-twice" })),
        )
        .await;
        let host = added["id"].as_i64().unwrap();
        harness
            .fleet
            .edit(host, |script| script.panic_on_dial = true);
        harness.fleet.kill_connection(host);
        harness
            .await_state(host, |state| state.phase() == "retired")
            .await;
        harness
            .fleet
            .edit(host, |script| script.panic_on_dial = false);

        // Both at once. Whichever order they resolve in, the registry is
        // the arbiter and the actor map must agree with it.
        let manager = Arc::clone(&harness.manager);
        let retrying = tokio::spawn(async move { manager.retry_now(host).await });
        let (status, _, text) = call(&harness, "DELETE", &format!("/api/hosts/{host}"), None).await;
        assert_eq!(status, StatusCode::OK, "{text}");
        let _ = retrying.await.expect("the retry task did not panic");

        // Whatever the interleaving, the removal is what the user asked for
        // and it is what holds.
        let (_, hosts, _) = call(&harness, "GET", "/api/hosts", None).await;
        let ids: Vec<i64> = hosts["hosts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["id"].as_i64().unwrap())
            .collect();
        assert!(
            !ids.contains(&host),
            "a removed host must not come back as a ghost: {ids:?}"
        );
        assert!(
            harness.manager.state(host).is_none(),
            "and no actor may remain behind the registry's back"
        );
    }

    /// A second entry reaching an already-registered install surfaces as an
    /// entry needing resolution, while the HOST appears exactly once.
    ///
    /// SPEC.md's shown-once rule and the user's ability to fix a mistyped
    /// address have to coexist, and this is how: the duplicate ENTRY is
    /// visible (so it can be edited or removed) but connects nothing and
    /// contributes no sessions, so the host behind it is still listed once,
    /// under the entry that owns it.
    #[tokio::test]
    async fn a_second_entry_for_one_install_is_marked_duplicate_and_serves_nothing() {
        let harness = lone_local_helm().await;
        let (_, first, _) = call(
            &harness,
            "POST",
            "/api/hosts",
            Some(serde_json::json!({ "ssh": "user@box" })),
        )
        .await;
        let original = first["id"].as_i64().unwrap();
        harness.fleet.edit(original, |script| {
            script.identity = Some("identity-shared".to_string());
            script.sessions = vec![rest_harness::session("only-once", 100)];
        });
        harness
            .manager
            .retry_now(original)
            .await
            .expect("the host exists");
        // The original must be holding the shared identity WITH its session
        // cached before the twin is introduced; otherwise the twin can win
        // the identity claim and the roles under test swap.
        harness
            .await_refreshed_as(original, "identity-shared", 1)
            .await;

        // The same machine, registered again under its other address.
        let (_, second, _) = call(
            &harness,
            "POST",
            "/api/hosts",
            Some(serde_json::json!({ "ssh": "box.internal" })),
        )
        .await;
        let twin = second["id"].as_i64().unwrap();
        harness.fleet.edit(twin, |script| {
            script.identity = Some("identity-shared".to_string());
            script.sessions = vec![rest_harness::session("only-once", 100)];
        });
        harness
            .manager
            .retry_now(twin)
            .await
            .expect("the host exists");
        harness
            .await_state(twin, |state| state.phase() == "duplicate")
            .await;

        let (_, hosts, _) = call(&harness, "GET", "/api/hosts", None).await;
        let duplicate = hosts["hosts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|host| host["id"] == twin)
            .expect("the duplicate entry stays visible so it can be resolved");
        assert_eq!(duplicate["state"]["phase"], "duplicate");
        assert_eq!(
            duplicate["state"]["twin"], original,
            "the entry names the row that owns the host, which is where it appears"
        );

        let (_, sessions, _) = call(&harness, "GET", "/api/sessions", None).await;
        assert_eq!(
            sessions["total"], 1,
            "the host's session is listed once, under the entry that owns it"
        );
        assert_eq!(sessions["sessions"][0]["host"], original);
    }
}
