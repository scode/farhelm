//! `/api/sessions` — the list, the routing every operation on one session
//! goes through, and the handlers themselves.
//!
//! The bulk of what a client does with a helm happens here, and the two
//! halves are worth separating in the reader's head: WHERE a request goes,
//! and what it does once it gets there.
//!
//! ## Routing is the interesting half
//!
//! A helm holds no authoritative session state, so every operation on a
//! session has to find the host that owns it and reach that host's LIVE
//! connection. `resolve_owner` answers the first question from the two
//! places a session can be known (helm.db's cache, and the manager's
//! in-memory list for a connected host with no identity to cache under);
//! `route_session` and `host_client` answer the second, and both take the
//! host's state and its client from ONE borrow of the actor's published
//! status. What that buys is COHERENCE, not freshness: the pair is
//! guaranteed to describe the same incarnation, so an operation is never
//! aimed at a client from one connection while a state from another said it
//! was fine. The connection can still be replaced the instant after the
//! borrow — nothing at this layer can prevent that, and nothing needs to:
//! the operation then fails against a dead client, which is an honest error,
//! rather than succeeding against the wrong machine.
//!
//! Every non-connected state refuses identically, through `refusal_text`.
//! That uniformity is deliberate: unreachable is only the most common of
//! the ways a host can fail to be connected — skew, identity mismatch, an
//! unverified identity, a duplicate, a retired row, and a first connection
//! still in flight are the others — and a caller that special-cases some of
//! them mis-handles the rest.
//!
//! ## The list never fans out
//!
//! `list_sessions` is served entirely from what the helm has already
//! recorded — helm.db plus the manager's memory — so a slow or flapping
//! host cannot slow a list poll down. The cost is that a session created
//! by ANOTHER client appears only after its host's next refresh. Sessions
//! created through this helm do not pay it: `record_session` seeds them at
//! create time, which is also what makes them routable immediately.
//!
//! ## Mutations write back what the host just said
//!
//! Create, restart, and rename all record their reply
//! (`record_session`), and delete forgets (`forget_session`). Without
//! that, the list — which is served from the recording, not from the host —
//! would show the user their own successful action as a no-op for up to a
//! refresh interval.

use crate::manager;
use crate::{
    AppState, CreateExtras, SupervisorClient, SupervisorError, aggregate, http_error, store,
};
use anyhow::Context;
use axum::extract::{Path as AxPath, Query, State};
use axum::response::IntoResponse;
use farhelm_proto::ErrorKind;
use serde::Deserialize;
use std::sync::Arc;
use tracing::warn;

/// Query parameters for `GET /api/sessions` — the helm-level page walk
/// (PLAN_M6.md item 5).
///
/// Both absent is a fresh walk of the first [`aggregate::DEFAULT_PAGE_LIMIT`]
/// entries, which is what every pre-M6 caller sends and what the UI in this
/// tree still sends. That is the whole compatibility story for this route's
/// query string: it gained two optional parameters and no required one.
#[derive(Deserialize)]
pub(crate) struct ListQuery {
    /// An opaque resume key from a previous reply's `next_cursor`. Replay
    /// it verbatim; never construct or interpret one. An undecodable value
    /// is a 400 rather than a silent restart from the front, because a
    /// restart would re-serve a page the caller already had while looking
    /// exactly like progress.
    cursor: Option<String>,
    /// Maximum entries in this page. Deliberately uncapped: the merged list
    /// is local data this process has already read, so a large page costs
    /// serialization rather than a fan-out of host round trips. A limit of
    /// zero is refused — it could never make progress through the pages.
    limit: Option<usize>,
}

/// `GET /api/sessions` — one page of the MERGED, multi-host session list
/// (PLAN_M6.md item 5).
///
/// The rows are every registered host's sessions in one creation-time
/// order, each tagged with the host it lives on and marked `stale` when
/// that host is not currently connected — SPEC.md's "sessions on an
/// unreachable host stay in the list, clearly marked" is this handler plus
/// the cache behind it, and nothing else.
///
/// The body keeps its M2 shape (`sessions`/`total`/`truncated`) with the
/// host fields added to each row and `next_cursor` added alongside, so the
/// UI that predates multi-host keeps decoding it unchanged. `total` now
/// counts the merged view rather than one supervisor's list, and
/// `truncated` now means "there is a next page" rather than "entries were
/// held back" — see [`aggregate::SessionPageBody`] for both.
///
/// Served from what the helm has already RECORDED, never by asking hosts
/// (see [`aggregate`]'s module docs for why the two cursor layers are
/// decoupled): helm.db for every host that caches, and the manager's
/// in-memory list for a connected host that has no identity to bind a cache
/// write to. Either way nothing here makes a network call, so a slow or
/// flapping host cannot slow a list poll down.
///
/// One consequence is worth stating rather than discovering: a session
/// created on ANOTHER client appears here only after its host's next
/// refresh, so this list trails such a create by up to one refresh
/// interval. A session created through this helm is recorded by the create
/// itself, and is routable immediately either way — routing does not go
/// through this handler.
pub(crate) async fn list_sessions(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListQuery>,
) -> impl IntoResponse {
    let limit = match q.limit {
        None => aggregate::DEFAULT_PAGE_LIMIT,
        Some(0) => {
            return http_error(anyhow::Error::new(SupervisorError {
                kind: ErrorKind::InvalidRequest,
                message: "session list limit must be at least 1; a limit of 0 could never make \
                          progress through the pages"
                    .to_string(),
            }));
        }
        Some(limit) if limit > crate::aggregate::MAX_PAGE_LIMIT => {
            return http_error(anyhow::Error::new(SupervisorError {
                kind: ErrorKind::InvalidRequest,
                message: format!(
                    "session list limit must be at most {}; a page is real work on this side, \
                     and an unbounded one is a request to do all of it at once",
                    crate::aggregate::MAX_PAGE_LIMIT
                ),
            }));
        }
        Some(limit) => limit,
    };
    match aggregate::session_page(&state.manager, &state.store, q.cursor.as_deref(), limit).await {
        Ok(page) => axum::Json(page).into_response(),
        Err(e) => http_error(e),
    }
}

/// Find the live connection for the host that owns `session_id`, or refuse
/// naming the state that host is actually in (PLAN_M6.md item 5).
///
/// The single owner-lookup path every session operation goes through. Two
/// properties are the whole point:
///
/// - **The state and the client are read TOGETHER**, from one borrow of the
///   actor's published status ([`manager::ConnectionManager::status`]). Two
///   separate reads can straddle a transition and hand back a fresh
///   `Connected` beside a `None` client, or a live-looking client beside a
///   dead state — which is exactly how an operation gets routed onto a
///   corpse.
/// - **Every non-connected state refuses identically**, with the state
///   named. Unreachable is not special; it is merely the common case. A
///   skewed, mismatched, unverified, duplicate, or retired host refuses the
///   same way, as does one whose first connection has not finished, because
///   the alternative is a caller that handles the states it thought of and
///   silently mis-handles the rest. Nothing queues — SPEC.md v1 refuses
///   rather than deferring.
///
/// A session nothing knows about is a 404. A session created HERE is
/// routable immediately — `create_session` seeds it into its host's cache
/// in the same handler — so that 404 means "no host has ever reported this
/// id", not "you were too quick". A session created by another client on
/// another host is the one case that waits, for up to one refresh interval,
/// which is the price of a list that never fans out to N hosts per request.
pub(crate) async fn route_session(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<(manager::SessionClaim, Arc<SupervisorClient>)> {
    let (host, status) = resolve_owner(state, session_id).await?;
    // The claim comes out of the SAME status this routed by, so an
    // operation whose reply is recorded afterwards (restart, rename) files
    // it against the connection it actually used — see
    // `manager::SessionClaim`.
    let identity = match &status.state {
        manager::HostState::Connected { identity, .. } => identity.clone(),
        _ => None,
    };
    let claim = manager::SessionClaim {
        host,
        incarnation: status.incarnation,
        identity,
    };
    let client = status.client.ok_or_else(|| {
        anyhow::Error::new(SupervisorError {
            kind: ErrorKind::Conflict,
            message: refusal_text(host, &status.state),
        })
    })?;
    Ok((claim, client))
}

/// Which host owns `session_id`, and that host's live status — read
/// together, from the two places a session can be known.
///
/// helm.db answers for every host that caches; the manager's in-memory
/// lists answer for a connected host that reports no identity and has none
/// on record, which therefore caches nothing (see
/// [`manager::HostSnapshot::live_sessions`]). Both are consulted because
/// either alone leaves a whole class of session unroutable: without the
/// first, nothing survives a helm restart; without the second, an
/// identity-less host reads as connected and empty while its sessions are
/// unreachable.
///
/// The in-memory lookup and the status it returns come from ONE hold of the
/// manager's actor map ([`manager::ConnectionManager::live_owner`]), not
/// from a snapshot followed by a second call. Split across two reads, a
/// reconnect landing in between pairs one install's session claim with the
/// next install's client — the same hazard the status accessor exists to
/// prevent for the cached case, and it deserves the same answer rather than
/// a second, weaker one.
///
/// The lookup is deliberately independent of whether the session's cached
/// METADATA still decodes: routing asks where to send an operation, not
/// what the session is, so a poisoned `info_json` must not make a live
/// session unreachable.
///
/// FAILS CLOSED where two hosts claim one id, with the ambiguity named —
/// including a collision a create discovered and recorded
/// ([`AppState::contested_sessions`]). helm.db makes that unconstructible
/// within itself, but a create can still mint an id another host already
/// holds, and picking one would mean a stop aimed at one machine landing on
/// another. A contested entry clears itself as soon as the fleet agrees
/// again, so a collision that resolved needs no intervention.
async fn resolve_owner(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<(store::HostId, manager::HostStatus)> {
    // Contested claims come first, from live refresh state rather than a
    // remembered incident: a host that STILL reports an id another host's
    // cache holds is a standing disagreement, and there is no honest owner
    // to route to while it stands. A claimant that stopped reporting the
    // id, was removed, or had its cache purged by an adoption is simply not
    // in this answer — the contest clears itself with the evidence that
    // made it.
    let contested = state.manager.contested_claimants(session_id);
    let cached = state.store.host_of_session(session_id).await?;
    if let Some(claimant) = contested.first()
        && let Some(owner) = cached
        && owner != *claimant
    {
        return Err(anyhow::Error::new(
            store::HostStoreError::SessionOwnerAmbiguous {
                session: session_id.to_string(),
                first: owner.min(*claimant),
                second: owner.max(*claimant),
            },
        ));
    }

    let live = state.manager.live_owner(session_id)?;
    match (cached, live) {
        (None, None) => Err(anyhow::Error::new(SupervisorError {
            kind: ErrorKind::NotFound,
            message: format!("no such session: {session_id}"),
        })),
        // The in-memory answer carries its own status from the same lock
        // hold, so it is used as it stands rather than looked up again.
        (None, Some((host, status))) => Ok((host, status)),
        (Some(host), Some((live_host, _))) if host != live_host => Err(anyhow::Error::new(
            store::HostStoreError::SessionOwnerAmbiguous {
                session: session_id.to_string(),
                first: host.min(live_host),
                second: host.max(live_host),
            },
        )),
        (Some(host), _) => {
            let status = state.manager.status(host).ok_or_else(|| {
                anyhow::Error::new(SupervisorError {
                    kind: ErrorKind::Conflict,
                    message: format!(
                        "session {session_id} lives on host {host}, which is no longer registered"
                    ),
                })
            })?;
            // RE-READ the cached owner after capturing the status, and
            // refuse if it moved. An adoption landing between the two reads
            // purges one host's cache and connects another, so the pair
            // taken naively can be "host A owns it" beside "host B's live
            // connection" — an operation sent to the wrong machine, with
            // nothing about either read looking wrong. Refusing is the only
            // safe answer available at this layer: the caller retries and
            // gets a coherent pair.
            let still = state.store.host_of_session(session_id).await?;
            if still != Some(host) {
                return Err(anyhow::Error::new(SupervisorError {
                    kind: ErrorKind::Conflict,
                    message: format!(
                        "session {session_id} changed hosts while this request was being routed; \
                         retry it"
                    ),
                }));
            }
            Ok((host, status))
        }
    }
}

/// The refusal sentence a non-connected host produces, for a session
/// operation and for a create alike.
///
/// Written once because SPEC.md requires the host's state to be IN the
/// error and requires errors to be actionable: two hand-written versions
/// would drift, and the one that drifted would be the one a user actually
/// read. The phase label is the same vocabulary the hosts list chips and
/// the log lines use ([`manager::HostState::phase`]), so a user comparing
/// an error against the hosts panel sees the same word in both.
fn refusal_text(host: store::HostId, state: &manager::HostState) -> String {
    let detail = match state {
        manager::HostState::Connecting { last_error, .. } => last_error
            .clone()
            .unwrap_or_else(|| "the first connection attempt has not finished yet".to_string()),
        manager::HostState::Unreachable { last_error, .. } => last_error.clone(),
        manager::HostState::VersionSkew {
            peer_protocol,
            our_protocol,
            remediation,
            ..
        } => format!(
            "the host speaks protocol {peer_protocol} and this helm speaks {our_protocol}; \
             {remediation}"
        ),
        manager::HostState::IdentityMismatch { recorded, reported } => format!(
            "the host now reports identity {reported} where {recorded} was recorded; adopt the \
             new identity or fix the destination"
        ),
        manager::HostState::IdentityUnverified { recorded } => format!(
            "the host answered without an identity, so this helm cannot confirm it is still the \
             install recorded as {recorded}; fix the host so it reports its identity, or \
             retarget or remove this entry"
        ),
        manager::HostState::Duplicate { twin, .. } => {
            format!("this entry duplicates host {twin}; edit or remove it")
        }
        manager::HostState::Retired { reason } => reason.clone(),
        // Unreachable in practice — a connected host has a client and
        // never reaches this function — but stated rather than
        // `unreachable!()`: a panic on the refusal path would turn a
        // routing race into a dropped connection.
        manager::HostState::Connected { .. } => "the host connected while this was decided".into(),
    };
    format!(
        "host {host} is {phase}, so this operation is refused and nothing was queued: {detail}",
        phase = state.phase()
    )
}

#[derive(Deserialize)]
pub(crate) struct CreateReq {
    cwd: String,
    invocation: String,
    title: Option<String>,
    /// Which registered host to create on — a `HostView::id` from
    /// `GET /api/hosts` (PLAN_M6.md item 5).
    ///
    /// Optional, defaulting to the reserved LOCAL row. That is the tail of
    /// SPEC.md's own creation default ("the host of the currently open
    /// session, else the helm's own host"): the first half needs to know
    /// what the user is looking at and is therefore the client's to supply,
    /// while the fallback is a server-side fact the helm can state itself.
    /// Keeping it optional is also what leaves every hand-written caller —
    /// a curl, a script, a test — meaning the obvious thing on a
    /// single-machine setup.
    host: Option<store::HostId>,
    #[serde(default = "default_cols")]
    cols: u16,
    #[serde(default = "default_rows")]
    rows: u16,
    /// The caller's idempotency key for this create (PLAN_M3.md item 6),
    /// passed straight through to the supervisor. Optional — like `title`,
    /// an absent field decodes as `None` — so every pre-M3 caller (curl, an
    /// older UI build, the CLI's startup create) keeps working unchanged,
    /// with each request its own create.
    intent_key: Option<String>,
    /// Override of the integrated-agent kind (PLAN_M3.md item 7), forwarded
    /// verbatim to `ControlMsg::CreateSession::agent_kind` — see that
    /// field's doc comment (farhelm-proto's `lib.rs`) for the full
    /// three-state semantics. Absent, like `intent_key`, decodes as `None`
    /// and preserves pre-M3 behavior: the supervisor derives the kind from
    /// `invocation`'s basename. On the wire a present value is one of the
    /// snake_case strings `"claude"`, `"codex"`, `"generic"` — the same
    /// representation `AgentKind`'s `#[serde(rename_all = "snake_case")]`
    /// produces on the supervisor protocol, so a JSON body needs no
    /// translation between the two.
    agent_kind: Option<farhelm_proto::AgentKind>,
    /// Override of the resume invocation template (PLAN_M3.md item 7),
    /// forwarded verbatim to `ControlMsg::CreateSession::resume_template` —
    /// see that field's doc comment for the placeholder-placement rule and
    /// the integrated/non-integrated distinction it enforces. Absent
    /// decodes as `None`, same posture as `intent_key`: for a session
    /// whose EFFECTIVE kind (after any `agent_kind` override) is
    /// integrated (claude/codex), the supervisor derives the template
    /// from `invocation`'s first token instead; a generic-kind session
    /// derives none — only this explicit override can give one a
    /// (verbatim, placeholder-free) resume invocation.
    resume_template: Option<Vec<String>>,
}

// Dimensions for a caller that has no terminal yet — the CLI, a script,
// a UI dialog that has not laid out a pane. 80x24 is a guess and it does
// not have to be a good one: the first attach resizes the window to the
// real client size, so these only decide how the agent's first few lines
// wrap before anyone is looking.
pub(crate) fn default_cols() -> u16 {
    80
}
pub(crate) fn default_rows() -> u16 {
    24
}

/// The live connection for one NAMED host, plus the claim that pins WHICH
/// connection it was — or a refusal naming the state that host is in.
///
/// The host-scoped twin of [`route_session`], which answers the same
/// question for a host it first has to derive from a session id. Both exist
/// so that the state and the client are read TOGETHER, from one borrow of
/// the actor's published status ([`manager::ConnectionManager::status`]):
/// two separate reads can straddle a transition and hand back a fresh
/// `Connected` beside a `None` client, or a live-looking client beside a
/// dead state, which is exactly how an operation gets routed onto a corpse.
///
/// An UNREGISTERED host is a 404 and a registered-but-not-connected host is
/// a conflict, because they are different things to the caller: the first
/// is a name that was never valid, the second is a condition that clears on
/// its own once the host comes back.
///
/// Synchronous, unlike [`route_session`]: naming a host outright skips the
/// helm.db lookup the owner search needs, and the manager's published status
/// is behind a plain lock. Nothing here awaits, so nothing here should
/// pretend it might.
fn host_client(
    state: &AppState,
    host: store::HostId,
) -> anyhow::Result<(manager::SessionClaim, Arc<SupervisorClient>)> {
    let status = state.manager.status(host).ok_or_else(|| {
        anyhow::Error::new(SupervisorError {
            kind: ErrorKind::NotFound,
            message: format!("no such host: {host}"),
        })
    })?;
    // The claim is taken from the SAME read that produced the client, so
    // the seed that follows can prove it is still talking about this
    // connection — see `manager::SessionClaim`.
    let identity = match &status.state {
        manager::HostState::Connected { identity, .. } => identity.clone(),
        // Unreachable in practice: a client is published exactly while the
        // state is `Connected`, and the `ok_or_else` below is what turns
        // every other state into a refusal. Written as a value rather than
        // `unreachable!()` because a panic on the create path would be a
        // far worse answer than a seed that later declines itself.
        _ => None,
    };
    let claim = manager::SessionClaim {
        host,
        incarnation: status.incarnation,
        identity,
    };
    let client = status.client.ok_or_else(|| {
        anyhow::Error::new(SupervisorError {
            kind: ErrorKind::Conflict,
            message: refusal_text(host, &status.state),
        })
    })?;
    Ok((claim, client))
}

/// The connection to create on: the body's `host`, or the reserved local
/// row when the body named none (see [`CreateReq::host`]).
///
/// A create against a host in ANY non-connected state is a PRECONDITION
/// FAILURE, exactly as SPEC.md's creation section demands — a visible error
/// naming the host's state, and no session anywhere. That refusal is
/// [`host_client`]'s, shared with the lifecycle routes on purpose:
/// "unreachable host" is listed in SPEC.md beside "nonexistent directory"
/// as one of the preconditions that fail a create, and every other
/// non-connected state is the same failure with a different cause.
///
/// The default is resolved by KIND rather than by a remembered id: the
/// reserved local row is the one host every helm has, and a body that names
/// no host means "here" — see [`CreateReq::host`].
fn create_target(
    state: &AppState,
    host: Option<store::HostId>,
) -> anyhow::Result<(manager::SessionClaim, Arc<SupervisorClient>)> {
    let snapshots = state.manager.snapshots();
    let host =
        match host {
            Some(host) => host,
            None => snapshots
                .iter()
                .find(|snapshot| snapshot.kind == store::HostKind::Local)
                .context(
                    "this helm has no local host row, so a create naming no host has no default \
                     target",
                )?
                .id,
        };
    host_client(state, host)
}

/// Record what a host just told us about one session, where the serving
/// path will find it.
///
/// Called by every mutation whose reply carries a fresh `SessionInfo`, and
/// each has its own reason:
///
/// - **Create.** Without this a create is followed by a window — up to one
///   refresh interval — in which every operation on the session it just
///   returned 404s, because routing resolves owners from what the helm has
///   recorded and the helm has recorded nothing yet. Not a theoretical gap:
///   the create dialog's own flow is "create, then open the terminal",
///   which lands in exactly that window.
/// - **Restart and rename.** The list is served from what the helm has
///   recorded, so a mutation whose result was not recorded leaves the row
///   showing the PREVIOUS state for a poll interval. A user who restarts an
///   exited session and watches the list keep saying `exited` has been shown
///   their own successful action as a failure (observed in the browser
///   suite, which is this behavior's regression test). Recording the reply
///   the host just sent costs nothing and closes it.
///
/// Goes through the MANAGER rather than straight to the store, and that is
/// not indirection for its own sake. The manager is what knows the two
/// things this write depends on: which storage a host uses (a host with no
/// identity caches nothing and serves from memory, and its created sessions
/// have to land there or they are invisible too), and whether the
/// connection the create used is still the current one. It is also what
/// serializes this write against the host's own refresh, so a drain that
/// predates the create cannot commit its wholesale replacement afterwards
/// and erase it.
///
/// BEST EFFORT for a stale claim, and deliberately not fatal: the session
/// exists and the caller must be told about it, since reporting a create
/// that actually succeeded as a failure is the one outcome SPEC.md's
/// creation contract rules out. Every such failure is self-healing within
/// one refresh — the host has the session and will report it.
///
/// AMBIGUITY IS THE EXCEPTION, and it is reported rather than swallowed:
/// if the session id is already cached under a DIFFERENT host there is no
/// honest owner, and routing would silently pick the other one. The
/// standing collision itself is not remembered HERE — it is refresh state
/// on the hosts that report it (`manager::ActorStatus::contested`), so it
/// clears itself when they stop.
async fn record_session(
    state: &AppState,
    claim: &manager::SessionClaim,
    session: &farhelm_proto::SessionInfo,
) {
    let Err(error) = state.manager.remember_session(claim, session).await else {
        return;
    };
    // The id is the PEER's text — escaped and bounded before it reaches a
    // log line, like every other peer-supplied value this process writes.
    let session_id = manager::peer_text(&session.id);
    if let Some(store::HostStoreError::SessionOwnerAmbiguous { first, second, .. }) = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<store::HostStoreError>())
    {
        warn!(
            host = claim.host,
            session_id = session_id.as_str(),
            first,
            second,
            "the host reported a session id another host already claims; it will not be routed \
             while both keep claiming it"
        );
        return;
    }
    warn!(
        host = claim.host,
        session_id = session_id.as_str(),
        error = %error,
        "could not record the session for routing; it will be picked up at the next refresh"
    );
}

/// Forget a deleted session everywhere the serving path looks for it.
///
/// The delete's half of [`record_session`]'s principle, and the quadrant
/// that was missing: a reply with no `SessionInfo` still carries the fact
/// that a session is gone, and the merged list is served from what the helm
/// has recorded. Leaving the row behind means the list shows a deleted
/// session until the owning host's next refresh — and a client that deletes
/// and immediately re-creates then sees BOTH, which is indistinguishable
/// from a duplicate. That is precisely how the browser suite found it, in
/// its own shared-session reset.
///
/// Best effort on the same terms as a seed: the delete SUCCEEDED and the
/// caller must be told so. Everything here is self-healing within one
/// refresh.
async fn forget_session(state: &AppState, claim: &manager::SessionClaim, session_id: &str) {
    if let Err(error) = state.manager.forget_session(claim, session_id).await {
        warn!(
            host = claim.host,
            session_id = manager::peer_text(session_id).as_str(),
            error = %error,
            "could not forget the deleted session; it will disappear at the next refresh"
        );
    }
}

/// `POST /api/sessions` — the creation API SPEC_impl.md calls the one true
/// path. The UI's create dialog and any script land on the same supervisor
/// call this reaches; there is no side door, and as of PLAN_M6.md item 5
/// there is no argv path either.
///
/// The body's `host` selects which registered host to create on, defaulting
/// to the local row; a host that is not connected fails the create as a
/// precondition (see [`create_target`]).
///
/// A body carrying `intent_key` gets server-enforced idempotency
/// (PLAN_M3.md item 6): a retry of the same request under the same key
/// yields the same session rather than a second one, and a key reused for
/// a DIFFERENT request comes back 409 through `http_error`. A body carrying
/// `agent_kind` and/or `resume_template` (PLAN_M3.md item 7) reaches the
/// supervisor's create validation unchanged, including its refusal of an
/// integrated kind paired with a placeholder-free template — that refusal
/// surfaces as `ErrorKind::InvalidRequest` and comes back 400 through the
/// same `http_error` mapping every other create precondition failure uses.
///
/// The reply is the created `SessionInfo`, unchanged. It carries no host
/// fields (contrast the list's rows): the caller already knows which host
/// it asked for, and inventing a second place where a session's host is
/// reported would be a second thing to keep true.
///
/// The new session is seeded into its host's cache before this answers
/// ([`seed_created_session`]), so it is routable — stop, rename, terminal —
/// the moment the caller has its id, rather than after the owning host's
/// next refresh. It joins the LIST on that next refresh like any other
/// session; the two are separate promises and only the first one is
/// something a client can be surprised by.
pub(crate) async fn create_session(
    State(state): State<Arc<AppState>>,
    axum::Json(req): axum::Json<CreateReq>,
) -> impl IntoResponse {
    let (claim, client) = match create_target(&state, req.host) {
        Ok(target) => target,
        Err(e) => return http_error(e),
    };
    match client
        .create_session_with_extras(
            &req.cwd,
            &req.invocation,
            req.title,
            req.cols,
            req.rows,
            CreateExtras {
                intent_key: req.intent_key,
                agent_kind: req.agent_kind,
                resume_template: req.resume_template,
            },
        )
        .await
    {
        Ok(session) => {
            record_session(&state, &claim, &session).await;
            axum::Json(session).into_response()
        }
        Err(e) => http_error(e),
    }
}

/// `POST /api/sessions/{id}/stop` — kill the agent's process tree, leaving
/// the session listed and its terminal viewable (SPEC.md's "stop", the
/// recoverable operation the UI does not confirm). The body carries no
/// information beyond success — an empty JSON object, so the response
/// shape stays uniform with `delete_session` below and callers do not
/// need to special-case "no content" bodies. An `id` the merged view does
/// not know is a 404 from [`route_session`] before any host is contacted,
/// and a session whose host is not connected is a 409 naming that state.
pub(crate) async fn stop_session(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
) -> impl IntoResponse {
    let (_claim, client) = match route_session(&state, &id).await {
        Ok(routed) => routed,
        Err(e) => return http_error(e),
    };
    match client.stop_session(&id).await {
        Ok(()) => axum::Json(serde_json::json!({})).into_response(),
        Err(e) => http_error(e),
    }
}

/// `GET /api/sessions/{id}` — one session's current state, as a merged-list
/// row (`SessionInfo` fields plus `host`, `host_name`, `stale`).
///
/// Exists for the recovery paths rather than for browsing: after a restart
/// (or after a restart whose reply was lost) a client needs THIS session's
/// current status and offer, and finding it must not depend on where it
/// happens to sit in its host's list.
///
/// ## Read, not operation — which is why it is not refused
///
/// This is the ONE `/api/sessions/{id}` route that a non-connected host
/// does not refuse, and the exception is SPEC.md's own: "opening such a
/// session shows its metadata — title, directory, last-known status —
/// behind a clear host-unreachable notice". Refusing here would leave the
/// UI nothing to put behind that notice. Every route that CHANGES
/// something still refuses (see [`route_session`]).
///
/// The two answers are deliberately different data, from one status read so
/// they cannot disagree:
///
/// - **Connected host: live, and the WHOLE list.** The owner's session list
///   is drained to exhaustion following its own cursor (the same bounded
///   walk the cache refresh uses), never one page. Asking for one page made
///   a session that happened to sit past the supervisor's default page
///   simply 404 — on a busy host, and only for the sessions a busy host has
///   most of. PLAN_M6.md is also explicit that the cache is for the stale
///   list and is not a general serving layer, so a reachable host's detail
///   must never come from it: a detail poll lagging the refresh cadence
///   would show a restart offer that no longer exists.
/// - **Non-connected host: last-known, `stale: true`.** The cached row,
///   which is exactly what the notice is drawn around.
///
/// ## Owner lookup does not depend on the cached row decoding
///
/// The owner is resolved from the cache's COLUMNS (and the manager's
/// in-memory lists), never from the stored metadata — so a row whose
/// `info_json` no longer decodes still routes, and a live session is served
/// from its host regardless of what its cached copy looks like. The
/// undecodable case only costs something for a host that is DOWN, where
/// there is genuinely nothing left to show and 404 is the honest answer.
///
/// Honest limitation, stated because it is not fixed here: the supervisor's
/// protocol has no per-session query, so the live path walks a list. What
/// this route buys is ONE place for every client's recovery lookup to live,
/// so the fix — a `GetSession` message — lands behind it rather than in each
/// caller.
pub(crate) async fn get_session(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
) -> impl IntoResponse {
    let (host, status) = match resolve_owner(&state, &id).await {
        Ok(owner) => owner,
        Err(e) => return http_error(e),
    };
    let Some(snapshot) = state
        .manager
        .snapshots()
        .into_iter()
        .find(|snapshot| snapshot.id == host)
    else {
        return http_error(anyhow::Error::new(SupervisorError {
            kind: ErrorKind::NotFound,
            message: format!("no such session: {id}"),
        }));
    };
    let host_name = aggregate::host_display_name(snapshot.kind, snapshot.destination.as_deref());

    // The client comes from the SAME status read that resolved the owner,
    // so "ask the host" and "say this row is live" cannot disagree.
    let Some(client) = status.client else {
        let cached = match state.store.cached_session(host, &id).await {
            Ok(cached) => cached,
            Err(e) => return http_error(e),
        };
        return match cached {
            Some(info) => axum::Json(aggregate::SessionRow {
                info,
                host,
                host_name,
                stale: true,
            })
            .into_response(),
            // The host is down and its cached copy is unreadable (or gone).
            // There is nothing to put behind the notice, and inventing a
            // placeholder would be worse than saying so.
            None => http_error(anyhow::Error::new(SupervisorError {
                kind: ErrorKind::NotFound,
                message: format!("no such session: {id}"),
            })),
        };
    };
    match manager::drain_sessions(&client).await {
        Ok(sessions) => match sessions.into_iter().find(|s| s.id == id) {
            Some(info) => axum::Json(aggregate::SessionRow {
                info,
                host,
                host_name,
                stale: false,
            })
            .into_response(),
            // The host is up and says this session is gone: it was deleted
            // between the last cache refresh and now, so 404 is the truth
            // rather than the stale row.
            None => http_error(anyhow::Error::new(SupervisorError {
                kind: ErrorKind::NotFound,
                message: format!("no such session: {id}"),
            })),
        },
        Err(e) => http_error(e),
    }
}

/// The body of `POST /api/sessions/{id}/restart`.
///
/// `mode` is required, and deliberately has no default: a restart that
/// guessed a mode could resume a conversation the caller never asked to
/// resume, or launch a fresh agent where the caller expected a resume.
/// The supervisor validates it against the session's CURRENT offer anyway
/// (PLAN_M3.md item 9), so a wrong value is refused rather than obeyed —
/// but an ABSENT one should not be silently turned into a choice at all.
///
/// `stop_if_running` defaults to false, the safe direction: an old-shaped
/// or hand-written body never kills a live agent by omission.
#[derive(Deserialize)]
pub(crate) struct RestartReq {
    mode: farhelm_proto::RestartMode,
    #[serde(default)]
    stop_if_running: bool,
}

/// `POST /api/sessions/{id}/restart` — relaunch the session's agent
/// (SPEC.md's restart; the resume offered when opening an interrupted
/// session is this same operation, not a separate one).
///
/// Pure passthrough, including of the refusals that carry this endpoint's
/// real contract: a `mode` that no longer matches the session's offer and
/// a live agent without `stop_if_running` both come back as 409s through
/// `http_error`, and a vanished working directory as a 400 naming the
/// directory. The success body is the session's freshly recomputed
/// `SessionInfo` — the same shape `POST /api/sessions` answers with — so a
/// caller can re-render the row (its new offer included) without listing
/// again. Routed by owner like every other lifecycle operation, so a
/// session on a non-connected host is refused with that host's state named
/// rather than reaching a supervisor at all.
pub(crate) async fn restart_session(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
    axum::Json(req): axum::Json<RestartReq>,
) -> impl IntoResponse {
    let (claim, client) = match route_session(&state, &id).await {
        Ok(routed) => routed,
        Err(e) => return http_error(e),
    };
    match client
        .restart_session(&id, req.mode, req.stop_if_running)
        .await
    {
        Ok(session) => {
            record_session(&state, &claim, &session).await;
            axum::Json(session).into_response()
        }
        Err(e) => http_error(e),
    }
}

/// The body of `POST /api/sessions/{id}/rename`: the verb-POST convention
/// `/stop` and `/restart` already use (PLAN_M5.md item 4), rather than a
/// PATCH with a partial `SessionInfo` — there is exactly one field to
/// change, and a verb route says so without inventing a partial-update
/// shape this API has nowhere else.
///
/// `title` has no default and no client-side shape check: an absent field
/// is a 422 from axum's `Json` extractor (a body that parses as JSON but
/// fails to deserialize into this struct — axum 0.8's
/// `JsonRejection::JsonDataError` status, distinct from the 400 a body
/// that is not even valid JSON gets) before this handler ever runs, and
/// every value that DOES parse — including control characters and the
/// empty string — is forwarded as-is (see `rename_session`'s docs for why
/// this handler does not pre-filter what only the supervisor is
/// authoritative over).
#[derive(Deserialize)]
pub(crate) struct RenameReq {
    title: String,
}

/// `POST /api/sessions/{id}/rename` — SPEC.md's rename verb (PLAN_M5.md
/// item 4), closing one of the two v1 client-surface operations
/// unimplemented since M1 (archive is the other, deliberately M7's).
///
/// Pure passthrough, deliberately: `req.title` reaches
/// `SupervisorClient::rename_session` VERBATIM, with no trimming and no
/// local validation. The supervisor is the sole authority on what title is
/// acceptable — control characters are refused, and a title over the 64
/// KiB field cap is refused, but every value that clears both (including
/// an explicit empty title) is accepted — so a helm-side check would only
/// be a second copy of that rule with its own chance to drift; a refused
/// title comes back through the same `ErrorKind`→status table every other
/// route uses (`InvalidRequest` 400, `NotFound` 404 for an unknown
/// session), and the accepted case answers with the session's freshly
/// recomputed `SessionInfo`, matching `get_session`'s and `restart_session`'s
/// success shape so a caller can re-render the row without listing again.
pub(crate) async fn rename_session(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
    axum::Json(req): axum::Json<RenameReq>,
) -> impl IntoResponse {
    let (claim, client) = match route_session(&state, &id).await {
        Ok(routed) => routed,
        Err(e) => return http_error(e),
    };
    match client.rename_session(&id, &req.title).await {
        Ok(session) => {
            record_session(&state, &claim, &session).await;
            axum::Json(session).into_response()
        }
        Err(e) => http_error(e),
    }
}

/// `DELETE /api/sessions/{id}` — remove a session and all its stored state
/// (SPEC.md's "delete"). This handler enforces nothing about liveness: it
/// deletes unconditionally, in any state. SPEC.md's confirm-when-alive
/// rule is normatively a CLIENT responsibility — no UI calls this route
/// yet, and when the UI PR adds the delete action, confirming before it
/// sends this request is that PR's job, not something to retrofit here.
/// Same empty-object success body as `stop_session`; an unknown `id` maps
/// to 404.
///
/// A successful delete FORGETS the session from the helm's own records
/// before it answers ([`forget_session`]), so the merged list stops showing
/// it at once rather than at the owning host's next refresh. Without that,
/// a delete followed immediately by a create shows both rows — which is
/// what the browser suite's own shared-session reset does on every test.
pub(crate) async fn delete_session(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
) -> impl IntoResponse {
    let (claim, client) = match route_session(&state, &id).await {
        Ok(routed) => routed,
        Err(e) => return http_error(e),
    };
    match client.delete_session(&id).await {
        Ok(()) => {
            forget_session(&state, &claim, &id).await;
            axum::Json(serde_json::json!({})).into_response()
        }
        Err(e) => http_error(e),
    }
}

/// `POST /api/sessions/{id}/tabs` — open a terminal tab: a plain shell in
/// the session's working directory (PLAN_M4.md item 2, plumbed through by
/// item 5). No request body: unlike `create_session`, a tab has nothing
/// for a caller to specify.
///
/// The success body is `{"tab": TabInfo}` rather than the bare object
/// `stop`/`delete` use, because there is something to hand back — the
/// minted tab id a client needs before it can attach
/// (`?tab=<id>` on `term_ws`). Every refusal the supervisor can give
/// (vanished working directory, no tmux session to open a window on, a
/// shell dead by reply time) reaches the browser through the same
/// `http_error` mapping every other endpoint uses, verbatim.
pub(crate) async fn open_tab(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
) -> impl IntoResponse {
    let (_claim, client) = match route_session(&state, &id).await {
        Ok(routed) => routed,
        Err(e) => return http_error(e),
    };
    match client.open_tab(&id).await {
        Ok(tab) => axum::Json(serde_json::json!({ "tab": tab })).into_response(),
        Err(e) => http_error(e),
    }
}

/// `DELETE /api/sessions/{id}/tabs/{tab_id}` — close a terminal tab: kill
/// its shell and everything it left behind, then drop the window
/// (PLAN_M4.md item 2). Same empty-object success body as `stop_session`/
/// `delete_session`; an unknown `tab_id` maps to 404 like any other
/// unknown identifier, and a tab whose shell had already exited still
/// closes successfully — `close_tab`'s own idempotency, passed straight
/// through.
pub(crate) async fn close_tab(
    State(state): State<Arc<AppState>>,
    AxPath((id, tab_id)): AxPath<(String, String)>,
) -> impl IntoResponse {
    let (_claim, client) = match route_session(&state, &id).await {
        Ok(routed) => routed,
        Err(e) => return http_error(e),
    };
    match client.close_tab(&id, &tab_id).await {
        Ok(()) => axum::Json(serde_json::json!({})).into_response(),
        Err(e) => http_error(e),
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_owner, store};
    use crate::rest_harness::{self, WsTestClient, silent_supervisor};
    use std::time::Duration;

    /// `POST /api/sessions` end to end through the real axum handler and
    /// middleware stack, with a scripted supervisor peer standing in for
    /// `farhelm-supervisor`.
    ///
    /// PLAN_M1.md makes this endpoint the single creation path: every
    /// caller — the M1 CLI flags today, the M2 GUI session-creation dialog
    /// next — lands on the same API, never bypassing it. Despite that, no
    /// *successful* request previously exercised the handler: the
    /// Playwright e2e suite deliberately covers only a failure path
    /// (create in a nonexistent working directory), and the Rust e2e
    /// tests call `SupervisorClient::create_session` directly, which
    /// bypasses both the handler and the `CreateReq` struct's
    /// `#[serde(default)]` cols/rows fields entirely. That left the 80x24
    /// default — the size an agent's first output wraps to before any
    /// browser has attached and reported a real size — pinned nowhere.
    /// This test closes that gap: it omits cols/rows/title from the
    /// request body, asserts the peer received exactly the defaults, and
    /// checks the JSON reply shape a caller actually depends on.
    ///
    /// This same minimal body is also the pre-M3 caller posture for
    /// `agent_kind`/`resume_template` (PLAN_M3.md item 7): the UI and CLI
    /// currently send neither field, so this test also pins that an
    /// absent override decodes and forwards as `None` rather than
    /// inventing a value — the fields are deliberately accepted here for
    /// non-UI API callers that basename recognition cannot classify, not
    /// because every production caller is expected to omit them forever.
    #[tokio::test]
    async fn create_session_request_with_omitted_dimensions_uses_80x24_defaults() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, Frame, SessionInfo};
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::CreateSession {
                req_id,
                cwd,
                invocation,
                // Bound rather than swept into the `..` below: which MODE
                // the helm forwards is exactly what the assertions here are
                // about, and a `..` would let a future change start sending
                // a profile selection alongside the invocation — the
                // ambiguous request the supervisor refuses — with this test
                // still green.
                profile_id,
                title,
                cols,
                rows,
                agent_kind,
                resume_template,
                // Not under test here (the assertions below only check
                // cwd/invocation/profile/title/cols/rows/agent_kind/
                // resume_template); PLAN_M3.md's `intent_key` is exercised
                // by `create_session_forwards_the_bodys_extras_to_the_supervisor`
                // instead.
                ..
            } = request
            else {
                panic!("expected CreateSession, got {request:?}");
            };
            // The contract under test: a caller that omits cols/rows must
            // still reach the supervisor with the documented 80x24
            // defaults. (Without the serde defaults the request would not
            // reach the supervisor at all — axum rejects a body missing
            // non-optional fields during deserialization.)
            assert_eq!((cols, rows), (80, 24), "serde defaults must be 80x24");
            assert_eq!(cwd, "/some/dir");
            // The RAW create mode, spelled out: the helm has no profile
            // catalog of its own, so every create it forwards names an
            // invocation and no profile (PLAN_M6_75.md item 3's
            // exclusivity — a request naming both is refused).
            assert_eq!(invocation, Some("some-agent".to_string()));
            assert_eq!(
                profile_id, None,
                "the raw mode names no profile — a request naming both is refused outright"
            );
            assert_eq!(title, None);
            assert_eq!(agent_kind, None);
            assert_eq!(resume_template, None);
            writer
                .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                    req_id,
                    session: SessionInfo {
                        id: "sess-1".into(),
                        title: "some-agent".into(),
                        created_at: 1_700_000_000,
                        cwd: "/some/dir".into(),
                        invocation: "some-agent".into(),
                        // Matches real `create_session` output: `Unknown`,
                        // not a live status (creation does not establish the
                        // agent's later exec succeeded).
                        status: farhelm_proto::SessionStatus::Unknown,
                        annotation: None,
                        restart_offer: farhelm_proto::RestartOffer::default(),
                        tabs: Vec::new(),
                        source_profile: None,
                    },
                }))
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions")
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                serde_json::json!({"cwd": "/some/dir", "invocation": "some-agent"}).to_string(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let session: SessionInfo = serde_json::from_slice(&body).unwrap();
        assert_eq!(session.id, "sess-1");
        assert_eq!(session.cwd, "/some/dir");

        peer.await.unwrap();
    }

    /// The create body's `intent_key`, `agent_kind`, and `resume_template`
    /// all reach the supervisor verbatim (PLAN_M3.md items 6 and 7).
    ///
    /// Worth its own test because the helm is a pure pass-through here and
    /// pass-throughs are exactly what silently stop passing things
    /// through: nothing else in this crate would notice if a field were
    /// dropped, and for `intent_key` specifically the symptom in production
    /// would not be an error but a SECOND session appearing on a retry —
    /// the failure the whole feature exists to prevent, visible only under
    /// a lost reply.
    #[tokio::test]
    async fn create_session_forwards_the_bodys_extras_to_the_supervisor() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{AgentKind, ControlMsg, Frame, SessionInfo};
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::CreateSession {
                req_id,
                intent_key,
                agent_kind,
                resume_template,
                ..
            } = request
            else {
                panic!("expected CreateSession, got {request:?}");
            };
            assert_eq!(
                intent_key.as_deref(),
                Some("intent-from-the-browser"),
                "the key belongs to whoever can retry, so it must arrive unaltered"
            );
            assert_eq!(agent_kind, Some(AgentKind::Claude));
            assert_eq!(
                resume_template,
                Some(vec!["claude".to_string(), "{conversation}".to_string()])
            );
            writer
                .write_frame(&Frame::control(&ControlMsg::SessionCreated {
                    req_id,
                    session: SessionInfo {
                        id: "sess-1".into(),
                        title: "t".into(),
                        created_at: 1_700_000_000,
                        cwd: "/some/dir".into(),
                        invocation: "some-agent".into(),
                        status: farhelm_proto::SessionStatus::Unknown,
                        annotation: None,
                        restart_offer: farhelm_proto::RestartOffer::default(),
                        tabs: Vec::new(),
                        source_profile: None,
                    },
                }))
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions")
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                serde_json::json!({
                    "cwd": "/some/dir",
                    "invocation": "some-agent",
                    "intent_key": "intent-from-the-browser",
                    "agent_kind": "claude",
                    "resume_template": ["claude", "{conversation}"],
                })
                .to_string(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        peer.await.unwrap();
    }

    /// `POST /api/sessions/{id}/stop` end to end: a scripted peer replies
    /// `SessionStopped`, and the route must answer 200 with an empty JSON
    /// object — the uniform success body `stop`/`delete` share so a caller
    /// does not need to special-case "no content".
    #[tokio::test]
    async fn stop_session_happy_path_returns_200_with_empty_object_body() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::StopSession { req_id, session_id } = request else {
                panic!("expected StopSession, got {request:?}");
            };
            assert_eq!(session_id, "sess-1");
            writer
                .write_control(&ControlMsg::SessionStopped { req_id })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/stop")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value, serde_json::json!({}));

        peer.await.unwrap();
    }

    /// Stopping an unknown id must surface as a 404 carrying the
    /// helm's OWN message, without the request ever reaching a supervisor.
    ///
    /// This contract INVERTED with PLAN_M6.md item 5, and the inversion is
    /// the point of keeping the test. Before owner routing, an unknown id
    /// was the supervisor's question to answer, and this test pinned the
    /// verbatim passthrough of its 404. Now the helm resolves a session's
    /// owning host in its merged view first, so an id nobody owns has no
    /// host to ask — answering it locally is not an optimization but the
    /// only honest thing available, since "which supervisor would you even
    /// forward this to" has no answer.
    ///
    /// Both halves are asserted: the status and body a caller sees, and —
    /// through [`silent_supervisor`] — that the connected host was not
    /// asked. Without the second half a helm that forwarded the request AND
    /// answered locally would pass.
    #[tokio::test]
    async fn stop_session_unknown_id_returns_404_with_supervisor_message() {
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(silent_supervisor(peer_side));

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-missing/stop")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&body),
            "no such session: sess-missing",
            "the helm's own refusal must name the id it could not place"
        );

        peer.await.unwrap();
    }

    /// `DELETE /api/sessions/{id}` happy path, mirroring the stop test
    /// above: a scripted `SessionDeleted` reply must reach the caller as
    /// 200 with the same empty-object body shape.
    #[tokio::test]
    async fn delete_session_happy_path_returns_200_with_empty_object_body() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::DeleteSession { req_id, session_id } = request else {
                panic!("expected DeleteSession, got {request:?}");
            };
            assert_eq!(session_id, "sess-1");
            writer
                .write_control(&ControlMsg::SessionDeleted { req_id })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("DELETE")
            .uri("/api/sessions/sess-1")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value, serde_json::json!({}));

        peer.await.unwrap();
    }

    /// Deleting an unknown id must 404 from the helm's own owner lookup,
    /// the delete-side twin of
    /// `stop_session_unknown_id_returns_404_with_supervisor_message` — see
    /// that test's docs for why this contract inverted with M6's routing,
    /// and why the silent supervisor is half the assertion.
    #[tokio::test]
    async fn delete_session_unknown_id_returns_404_with_supervisor_message() {
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(silent_supervisor(peer_side));

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("DELETE")
            .uri("/api/sessions/sess-missing")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&body),
            "no such session: sess-missing",
            "the helm's own refusal must name the id it could not place"
        );

        peer.await.unwrap();
    }

    /// `GET /api/sessions`'s JSON shape, which the UI decodes and which
    /// PLAN_M6.md item 5 extended without breaking.
    ///
    /// Two halves, both load-bearing for the UI PRs that follow. The M2
    /// envelope (`sessions`/`total`/`truncated`) is still there under the
    /// same names, so the list UI in this tree keeps decoding it unchanged;
    /// and each row now carries `host`/`host_name`/`stale` as ADDITIVE
    /// siblings of the session's own fields, never nested under a wrapper —
    /// which is the whole reason `SessionRow` flattens `SessionInfo`
    /// instead of embedding it.
    ///
    /// Asserted on raw JSON rather than a decoded type, because the UI
    /// decodes JSON: a serialization change that a round trip through the
    /// same Rust types would hide is exactly what would break the list in
    /// the browser.
    #[tokio::test]
    async fn list_sessions_returns_the_merged_listing_object_shape() {
        let harness =
            rest_harness::helm_listing(vec![rest_harness::session("sess-1", 1_700_000_000)]).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("GET")
            .uri("/api/sessions")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = tower::ServiceExt::oneshot(app, request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["total"], 1, "total counts the merged view");
        assert_eq!(
            value["truncated"], false,
            "one page held everything, so there is no next page"
        );
        assert_eq!(value["next_cursor"], serde_json::Value::Null);

        let row = &value["sessions"][0];
        assert_eq!(row["id"], "sess-1");
        assert_eq!(
            row["title"], "sess-1",
            "the session's own fields stay at the row's top level"
        );
        assert_eq!(
            row["host"],
            rest_harness::local_id(&harness.store).await,
            "every row names the host it lives on"
        );
        assert_eq!(
            row["host_name"], "this machine",
            "the reserved local row is described, never addressed"
        );
        assert_eq!(
            row["stale"], false,
            "a connected host's rows are live knowledge"
        );
    }

    /// `GET /api/sessions/{id}` — the session-detail route a session view
    /// actually fetches — must pass a NON-EMPTY `tabs` list through
    /// intact. `farhelm-proto`'s own tests already pin `SessionInfo`'s
    /// JSON shape exhaustively (order, nesting, everything); what the helm
    /// still owes is exactly one HTTP-boundary check that THIS route does
    /// not drop or mangle the field on its way from the supervisor's
    /// `ListSessions` reply to the JSON body a browser decodes — this
    /// replaces an earlier version of the same check aimed at the bulk
    /// LISTING route, which no session view reads tabs from.
    #[tokio::test]
    async fn get_session_passes_a_non_empty_tabs_list_through() {
        let harness = rest_harness::helm_listing(vec![farhelm_proto::SessionInfo {
            tabs: vec![
                farhelm_proto::TabInfo { id: "tab-1".into() },
                farhelm_proto::TabInfo { id: "tab-2".into() },
            ],
            ..rest_harness::session("sess-1", 1_700_000_000)
        }])
        .await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("GET")
            .uri("/api/sessions/sess-1")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = tower::ServiceExt::oneshot(app, request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            value["tabs"],
            serde_json::json!([{"id": "tab-1"}, {"id": "tab-2"}])
        );
        assert_eq!(
            value["stale"], false,
            "a connected host's detail is live, and says so"
        );
        assert_eq!(
            value["host"],
            rest_harness::local_id(&harness.store).await,
            "the detail route carries the same host fields a list row does"
        );
    }

    /// The helm is a passthrough for classification, and PLAN_M3.md item 2
    /// is the first change that makes that claim testable with something
    /// the helm could plausibly get wrong: `interrupted` is a status
    /// variant no earlier milestone had, and the stop annotation is a
    /// field nothing used to populate. Neither is invented, renamed, or
    /// dropped here — the supervisor is authoritative (SPEC.md), and this
    /// pins that the JSON the browser receives says exactly what the
    /// supervisor said.
    ///
    /// Asserted on the raw JSON rather than a decoded `SessionInfo`,
    /// because the UI decodes JSON, not proto types: a serialization
    /// change that a round trip through the same Rust types would hide is
    /// precisely what would break the badge in the browser.
    ///
    /// As of PLAN_M6.md item 5 the claim is stronger than it was: these
    /// rows now reach the browser by way of helm.db's session cache, so
    /// they survive a serialize/store/deserialize round trip on the way.
    /// A status variant or annotation field that failed to persist would
    /// fail here too, which is exactly the coverage a durable cache of
    /// supervisor-authored data needs.
    #[tokio::test]
    async fn list_sessions_passes_interrupted_status_and_stop_annotation_through() {
        let session = |id: &str, status, annotation: Option<&str>| farhelm_proto::SessionInfo {
            status,
            annotation: annotation.map(str::to_string),
            // `created_at` is shared, so the merged order falls to the id
            // tiebreak — which is what fixes "lost" ahead of "stopped"
            // below rather than leaving the two positions to chance.
            ..rest_harness::session(id, 1_700_000_000)
        };
        let harness = rest_harness::helm_listing(vec![
            session("lost", farhelm_proto::SessionStatus::Interrupted, None),
            session(
                "stopped",
                farhelm_proto::SessionStatus::Exited { exit_code: Some(0) },
                Some(farhelm_proto::STOP_ANNOTATION),
            ),
        ])
        .await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("GET")
            .uri("/api/sessions")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = tower::ServiceExt::oneshot(app, request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["sessions"][0]["status"]["state"], "interrupted");
        assert_eq!(value["sessions"][0]["annotation"], serde_json::Value::Null);
        assert_eq!(value["sessions"][1]["status"]["state"], "exited");
        assert_eq!(value["sessions"][1]["annotation"], "stopped by user");
    }

    /// The DNS-rebinding origin guard is route-agnostic middleware, and the
    /// Playwright suite (`terminal.spec.ts`, "requests from a foreign
    /// origin are refused") already proves it holds through the real
    /// stack. What that suite does NOT cover is this PR's own change: that
    /// the guard sits in front of the new mutating routes too, not just
    /// `GET /api/sessions`, and that a refused request never reaches the
    /// supervisor at all. A loopback `Host` (same-origin by that half of
    /// the check) paired with a foreign `Origin` isolates exactly the
    /// half the browser itself supplies from the requesting page's origin
    /// — same setup as `foreign_or_missing_authorities_are_refused`
    /// above, aimed at the stop route instead of the pure function.
    ///
    /// Proof that no frame reached the supervisor comes from EOF, not a
    /// timeout: `oneshot` consumes the router (and with it the only
    /// remaining `Arc<SupervisorClient>`) once the response is produced, so
    /// the transport closes right after — the scripted peer reading a clean
    /// `Ok(None)` at that point means nothing but the handshake was ever
    /// written to it. A frame arriving instead (a bypassed guard) would read
    /// as `Ok(Some(_))`, which is what this actually distinguishes.
    #[tokio::test]
    async fn foreign_origin_is_refused_on_the_stop_route() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake};
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            // A bounded silence, not an EOF: the harness keeps this
            // connection open for the whole test (see `rest_harness`), so
            // "nothing reached the supervisor" is only observable as
            // nothing ARRIVING. The window is generous because a false
            // pass needs the frame to be merely late, and a stop that the
            // middleware failed to refuse would be sent immediately.
            let leaked = tokio::time::timeout(Duration::from_secs(2), reader.read_frame()).await;
            assert!(
                leaked.is_err(),
                "stop request must never reach the supervisor for a foreign origin, but one \
                 arrived: {leaked:?}"
            );
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/stop")
            .header("host", "127.0.0.1:7433")
            .header("origin", "http://evil.example")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);

        peer.await.unwrap();
    }

    /// `http_error`'s status mapping, pinned through the real handler and
    /// middleware stack rather than by calling `http_error` directly: what
    /// actually matters is that a `ControlMsg::Error`'s `kind` survives
    /// `SupervisorClient::request`'s downcast and reaches the HTTP status,
    /// not just that the mapping function has the right match arms.
    ///
    /// `InvalidRequest` is exercised here (400) rather than `NotFound`
    /// (404): both go through the identical downcast path in `http_error`,
    /// and the supervisor-side classification for a bad cwd — the
    /// realistic `InvalidRequest` case — is itself pinned end-to-end
    /// against a real supervisor in `farhelm/tests/e2e.rs`
    /// (`create_in_missing_directory_errors`). This test's job is narrower
    /// and complementary: prove the *client-and-HTTP* half of the chain
    /// (scripted `Error` reply in, status code out) without needing a real
    /// supervisor, tmux, or filesystem precondition to produce one.
    #[tokio::test]
    async fn create_session_error_reply_maps_to_bad_request_status() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, ErrorKind, Frame};
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::CreateSession { req_id, .. } = request else {
                panic!("expected CreateSession, got {request:?}");
            };
            writer
                .write_frame(&Frame::control(&ControlMsg::Error {
                    req_id,
                    message: "working directory does not exist: /nope".into(),
                    kind: ErrorKind::InvalidRequest,
                }))
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions")
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                serde_json::json!({"cwd": "/nope", "invocation": "some-agent"}).to_string(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(
            String::from_utf8_lossy(&body).contains("does not exist"),
            "body must still carry the supervisor's concrete message"
        );

        peer.await.unwrap();
    }

    /// `POST /api/sessions/{id}/restart` end to end (PLAN_M3.md item 9):
    /// the body's `mode` and `stop_if_running` reach the supervisor
    /// unaltered, and the success body is the session's own recomputed
    /// `SessionInfo` — including the freshly computed `restart_offer` a
    /// caller re-renders its row from without listing again.
    ///
    /// Both body fields are asserted at the WIRE, not merely accepted by
    /// the handler: `stop_if_running` is the user's consent to kill a
    /// running agent and `mode` is the choice the supervisor validates
    /// against the current offer, so a route that dropped or defaulted
    /// either would be a silent safety regression rather than a visible
    /// failure.
    #[tokio::test]
    async fn restart_session_passes_mode_and_consent_through_and_returns_the_session() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::RestartSession {
                req_id,
                session_id,
                mode,
                stop_if_running,
            } = request
            else {
                panic!("expected RestartSession, got {request:?}");
            };
            assert_eq!(session_id, "sess-1");
            assert_eq!(mode, farhelm_proto::RestartMode::Resume);
            assert!(
                stop_if_running,
                "the user's consent to stop a live agent must reach the supervisor"
            );
            writer
                .write_control(&ControlMsg::SessionRestarted {
                    req_id,
                    session: farhelm_proto::SessionInfo {
                        id: "sess-1".into(),
                        title: "t".into(),
                        created_at: 1_700_000_000,
                        cwd: "/some/dir".into(),
                        invocation: "some-agent".into(),
                        status: farhelm_proto::SessionStatus::Unknown,
                        annotation: None,
                        restart_offer: farhelm_proto::RestartOffer::Resume,
                        tabs: Vec::new(),
                        source_profile: None,
                    },
                })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/restart")
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                serde_json::json!({ "mode": "resume", "stop_if_running": true }).to_string(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["id"], "sess-1");
        assert_eq!(
            value["restart_offer"], "resume",
            "the reply carries the offer the session has NOW, which is what a client re-renders"
        );

        peer.await.unwrap();
    }

    /// A stale-offer refusal must reach the browser as a 409 carrying the
    /// supervisor's own prose — that message names the CURRENT offer, and
    /// re-presenting it is the client's prescribed response (the wire
    /// vocabulary's staleness contract). A route that flattened it to a
    /// generic 500 would leave the UI with nothing to say.
    #[tokio::test]
    async fn restart_session_conflict_reaches_the_caller_as_409_with_its_message() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, ErrorKind};
        use tower::ServiceExt;

        const SENTINEL: &str = "SENTINEL-restart-4b1e: the offer is now resume";

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::RestartSession { req_id, .. } = request else {
                panic!("expected RestartSession, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::Error {
                    req_id,
                    message: SENTINEL.to_string(),
                    kind: ErrorKind::Conflict,
                })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/restart")
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                serde_json::json!({ "mode": "fresh" }).to_string(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&body).trim(), SENTINEL);

        peer.await.unwrap();
    }

    /// `POST /api/sessions/{id}/rename` end to end (PLAN_M5.md item 4), for
    /// every shape a title can take: an ordinary title, the empty string
    /// (an explicit empty title is a legal rename, symmetric with an
    /// explicit empty title on create — PLAN_M5.md item 3), leading/
    /// trailing whitespace, and embedded control characters (the very
    /// thing the supervisor's own validation refuses, so this helm-level
    /// hop must not pre-filter or normalize it away before the refusal can
    /// even run). One route, four shapes, because the property under
    /// test — "no trimming, no validation, no rewriting" — is the same
    /// claim for each and a shared body keeps the cases from drifting
    /// into subtly different assertions.
    ///
    /// The success body is checked as a FULL `SessionInfo`, field for
    /// field against the scripted reply, not just `id`/`title`: a route
    /// that echoed a stale or partially-rebuilt session (the bug
    /// `SessionRenamed`'s own docs warn against — see
    /// `ControlMsg::SessionRenamed`) would still pass an id/title-only
    /// check while failing every other field.
    #[tokio::test]
    async fn rename_session_forwards_the_title_verbatim() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, RestartOffer, SessionInfo, SessionStatus, TabInfo};
        use tower::ServiceExt;

        let cases = [
            "an ordinary title",
            "",
            "  leading and trailing spaces  ",
            "bell\u{7}esc\u{1b}nl\ntab\t",
        ];

        for title in cases {
            let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
            let expected_title = title.to_string();
            // Distinctive on every field, not just `title`: a handler
            // that echoed back a stale or default-filled `SessionInfo`
            // must fail the full-struct comparison below even if the
            // title alone looked right.
            let expected_session = SessionInfo {
                id: "sess-1".into(),
                title: expected_title.clone(),
                created_at: 1_700_000_000,
                cwd: "/distinctive/dir".into(),
                invocation: "distinctive-agent --flag".into(),
                status: SessionStatus::Running,
                annotation: None,
                restart_offer: RestartOffer::Resume,
                tabs: vec![TabInfo { id: "tab-1".into() }],
                source_profile: None,
            };
            let reply_session = expected_session.clone();
            let peer = tokio::spawn(async move {
                let (r, w) = tokio::io::split(peer_side);
                let mut reader = FrameReader::new(r);
                let mut writer = FrameWriter::new(w);
                handshake(&mut reader, &mut writer, "supervisor")
                    .await
                    .unwrap();
                let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::RenameSession {
                    req_id, title: got, ..
                } = request
                else {
                    panic!("expected RenameSession, got {request:?}");
                };
                assert_eq!(
                    got, expected_title,
                    "the title must reach the supervisor byte-for-byte unchanged"
                );
                writer
                    .write_control(&ControlMsg::SessionRenamed {
                        req_id,
                        session: reply_session,
                    })
                    .await
                    .unwrap();
            });

            let harness = rest_harness::spliced_helm(client_side).await;
            let app = harness.router();
            let request = axum::http::Request::builder()
                .method("POST")
                .uri("/api/sessions/sess-1/rename")
                .header("host", "127.0.0.1:7433")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::json!({ "title": title }).to_string(),
                ))
                .unwrap();

            let response = app.oneshot(request).await.unwrap();
            assert_eq!(
                response.status(),
                axum::http::StatusCode::OK,
                "for title {title:?}"
            );
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            let got_session: SessionInfo = serde_json::from_slice(&body).unwrap();
            assert_eq!(
                got_session, expected_session,
                "the success body must be the supervisor's FULL SessionInfo, not a partial \
                 echo, for title {title:?}"
            );

            peer.await.unwrap();
        }
    }

    /// A body whose `title` field is MISSING entirely must be refused
    /// before this route's handler ever runs — 422 from axum 0.8's `Json`
    /// extractor rejecting a body that parses as JSON but fails to
    /// deserialize into `RenameReq` (a missing required field), distinct
    /// from the 400 a body that is not valid JSON at all would get
    /// (`RenameReq`'s own docs name the same distinction) — and
    /// distinctly from a body whose `title` is PRESENT but explicitly
    /// empty, which must reach the supervisor and be accepted (SPEC.md
    /// names control characters, not absence of content, as rename's
    /// refusal — PLAN_M5.md item 3; `rename_session_forwards_the_title_verbatim`
    /// also carries the empty-string case among its shapes). Both halves
    /// live in this one test, rather than as two that could quietly drift
    /// apart, because "missing" and "explicit empty" are exactly the pair
    /// a route that collapsed `Option<String>` handling could confuse.
    #[tokio::test]
    async fn rename_session_missing_title_is_422_but_an_explicit_empty_title_is_accepted() {
        use tower::ServiceExt;

        // Half 1: `title` absent. No frame may reach the supervisor at
        // all — a rejected extractor never calls `rename_session`'s body.
        {
            let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
            let peer = tokio::spawn(async move {
                let (r, w) = tokio::io::split(peer_side);
                let mut reader = farhelm_proto::io::FrameReader::new(r);
                let mut writer = farhelm_proto::io::FrameWriter::new(w);
                farhelm_proto::io::handshake(&mut reader, &mut writer, "supervisor")
                    .await
                    .unwrap();
                reader
            });

            let harness = rest_harness::spliced_helm(client_side).await;
            let app = harness.router();
            let request = axum::http::Request::builder()
                .method("POST")
                .uri("/api/sessions/sess-1/rename")
                .header("host", "127.0.0.1:7433")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(serde_json::json!({}).to_string()))
                .unwrap();

            let response = app.oneshot(request).await.unwrap();
            assert_eq!(
                response.status(),
                axum::http::StatusCode::UNPROCESSABLE_ENTITY,
                "a body missing the required `title` field must be a 422 from the JSON extractor"
            );

            // Dropping `app`/`response` above already dropped this
            // block's only `SupervisorClient` handle, which closes the
            // transport — so the peer seeing EOF proves nothing about
            // whether a frame was sent first; only the SHAPE of what (if
            // anything) arrives does. A still-open connection with
            // nothing to read, or a clean EOF with nothing read, are both
            // consistent with "no frame was ever sent"; an actual frame
            // is the one outcome that is not.
            let mut reader = peer.await.unwrap();
            match tokio::time::timeout(Duration::from_millis(200), reader.read_frame()).await {
                Err(_) | Ok(Ok(None)) => {}
                Ok(Ok(Some(frame))) => panic!(
                    "a rejected extractor must never let a RenameSession reach the \
                     supervisor, but this frame arrived: {frame:?}"
                ),
                Ok(Err(e)) => {
                    panic!("unexpected transport error while checking for a stray frame: {e}")
                }
            }
        }

        // Half 2: `title` present and explicitly empty. Must reach the
        // supervisor (not be treated as if it were absent) and succeed.
        {
            use farhelm_proto::ControlMsg;
            use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

            let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
            let peer = tokio::spawn(async move {
                let (r, w) = tokio::io::split(peer_side);
                let mut reader = FrameReader::new(r);
                let mut writer = FrameWriter::new(w);
                handshake(&mut reader, &mut writer, "supervisor")
                    .await
                    .unwrap();
                let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::RenameSession { req_id, title, .. } = request else {
                    panic!("expected RenameSession, got {request:?}");
                };
                assert_eq!(
                    title, "",
                    "an explicit empty title must reach the supervisor, not be treated as \
                     though it were absent"
                );
                writer
                    .write_control(&ControlMsg::SessionRenamed {
                        req_id,
                        session: farhelm_proto::SessionInfo {
                            id: "sess-1".into(),
                            title: String::new(),
                            created_at: 1_700_000_000,
                            cwd: "/some/dir".into(),
                            invocation: "some-agent".into(),
                            status: farhelm_proto::SessionStatus::Unknown,
                            annotation: None,
                            restart_offer: farhelm_proto::RestartOffer::default(),
                            tabs: Vec::new(),
                            source_profile: None,
                        },
                    })
                    .await
                    .unwrap();
            });

            let harness = rest_harness::spliced_helm(client_side).await;
            let app = harness.router();
            let request = axum::http::Request::builder()
                .method("POST")
                .uri("/api/sessions/sess-1/rename")
                .header("host", "127.0.0.1:7433")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::json!({ "title": "" }).to_string(),
                ))
                .unwrap();

            let response = app.oneshot(request).await.unwrap();
            assert_eq!(
                response.status(),
                axum::http::StatusCode::OK,
                "an explicit empty title must be ACCEPTED, distinctly from the missing-field \
                 case in the first half of this test"
            );

            peer.await.unwrap();
        }
    }

    /// Renaming an unknown session must 404 from the helm's own owner
    /// lookup, without reaching a supervisor — the rename-side twin of
    /// `stop_session_unknown_id_returns_404_with_supervisor_message`, whose
    /// docs carry the reasoning.
    #[tokio::test]
    async fn rename_session_unknown_id_returns_404_with_supervisor_message() {
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(silent_supervisor(peer_side));

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-missing/rename")
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                serde_json::json!({ "title": "doesn't matter" }).to_string(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&body),
            "no such session: sess-missing",
            "the helm's own refusal must name the id it could not place"
        );

        peer.await.unwrap();
    }

    /// A title the supervisor refuses (control characters, per PLAN_M5.md
    /// item 3's validation) must surface as a 400 carrying the
    /// supervisor's own refusal text — the UI's only source for that
    /// message, since this route performs no local validation of its own
    /// to phrase a redundant one from.
    #[tokio::test]
    async fn rename_session_invalid_title_returns_400_with_supervisor_message() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, ErrorKind};
        use tower::ServiceExt;

        const SENTINEL: &str = "SENTINEL-rename-e91f: title must not contain control characters";

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::RenameSession { req_id, .. } = request else {
                panic!("expected RenameSession, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::Error {
                    req_id,
                    message: SENTINEL.to_string(),
                    kind: ErrorKind::InvalidRequest,
                })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/rename")
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                serde_json::json!({ "title": "bad\u{7}title" }).to_string(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&body),
            SENTINEL,
            "body must carry the supervisor's own refusal text verbatim"
        );

        peer.await.unwrap();
    }

    /// `POST /api/sessions/{id}/tabs` happy path (PLAN_M4.md item 5): the
    /// scripted `TabOpened` reply's `TabInfo` must round-trip through the
    /// success body under a `tab` key — the shape a client needs before it
    /// can attach the new tab via `?tab=<id>` on `term_ws`.
    #[tokio::test]
    async fn open_tab_happy_path_returns_200_with_tab() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, TabInfo};
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::OpenTab { req_id, session_id } = request else {
                panic!("expected OpenTab, got {request:?}");
            };
            assert_eq!(session_id, "sess-1");
            writer
                .write_control(&ControlMsg::TabOpened {
                    req_id,
                    tab: TabInfo { id: "tab-1".into() },
                })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/tabs")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["tab"]["id"], "tab-1");

        peer.await.unwrap();
    }

    /// `DELETE /api/sessions/{id}/tabs/{tab_id}` happy path, mirroring
    /// `stop_session_happy_path_returns_200_with_empty_object_body`: a
    /// scripted `TabClosed` reply must reach the caller as 200 with the
    /// same empty-object body every no-payload success shares. The peer
    /// asserts both path segments landed in the right `CloseTab` fields —
    /// a route that swapped `id`/`tab_id` would still 200 here, just
    /// against the wrong tab.
    #[tokio::test]
    async fn close_tab_happy_path_returns_200_with_empty_object_body() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use tower::ServiceExt;

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::CloseTab {
                req_id,
                session_id,
                tab_id,
            } = request
            else {
                panic!("expected CloseTab, got {request:?}");
            };
            assert_eq!(session_id, "sess-1");
            assert_eq!(tab_id, "tab-1");
            writer
                .write_control(&ControlMsg::TabClosed { req_id })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("DELETE")
            .uri("/api/sessions/sess-1/tabs/tab-1")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value, serde_json::json!({}));

        peer.await.unwrap();
    }

    /// `POST /api/sessions/{id}/tabs` must map a supervisor `Error` reply
    /// to the right HTTP status AND carry its message through verbatim.
    /// `http_error`'s own unit tests already pin the full four-`ErrorKind`
    /// table exhaustively, so this route owes only ONE representative
    /// case through the real handler — `NotFound`, the same choice
    /// `stop_session_unknown_id_returns_404_with_supervisor_message` made
    /// for the same reason. The body assertion is the COMPLETE sentinel,
    /// not a substring: a handler that truncated or rewrapped the
    /// supervisor's message would still pass a status-only check here,
    /// which is exactly the gap an exact-body assertion closes.
    #[tokio::test]
    async fn open_tab_error_reply_maps_to_404_with_the_supervisors_message() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, ErrorKind};
        use tower::ServiceExt;

        const SENTINEL: &str = "SENTINEL-open-tab-3f1a2c: no such session";

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::OpenTab { req_id, .. } = request else {
                panic!("expected OpenTab, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::Error {
                    req_id,
                    message: SENTINEL.to_string(),
                    kind: ErrorKind::NotFound,
                })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sessions/sess-1/tabs")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&body),
            SENTINEL,
            "body must carry the supervisor's own message verbatim, not a substring of it"
        );

        peer.await.unwrap();
    }

    /// `DELETE /api/sessions/{id}/tabs/{tab_id}`'s twin of
    /// `open_tab_error_reply_maps_to_404_with_the_supervisors_message` —
    /// same reasoning (one representative `ErrorKind`, exact-body
    /// assertion), aimed at `close_tab` instead so a route wired to the
    /// wrong client method (or dropping `http_error` entirely) cannot hide
    /// behind the open-tab coverage above.
    #[tokio::test]
    async fn close_tab_error_reply_maps_to_404_with_the_supervisors_message() {
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        use farhelm_proto::{ControlMsg, ErrorKind};
        use tower::ServiceExt;

        const SENTINEL: &str = "SENTINEL-close-tab-9d4e17: no such tab";

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::CloseTab { req_id, .. } = request else {
                panic!("expected CloseTab, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::Error {
                    req_id,
                    message: SENTINEL.to_string(),
                    kind: ErrorKind::NotFound,
                })
                .await
                .unwrap();
        });

        let harness = rest_harness::spliced_helm(client_side).await;
        let app = harness.router();

        let request = axum::http::Request::builder()
            .method("DELETE")
            .uri("/api/sessions/sess-1/tabs/tab-1")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&body),
            SENTINEL,
            "body must carry the supervisor's own message verbatim, not a substring of it"
        );

        peer.await.unwrap();
    }

    /// Issue one request against `app` and return its status and JSON body.
    ///
    /// The tests below make several requests each and none of them is about
    /// HTTP mechanics, so the builder boilerplate lives here once.
    async fn get_json(
        harness: &rest_harness::Harness,
        uri: &str,
    ) -> (axum::http::StatusCode, serde_json::Value) {
        let request = axum::http::Request::builder()
            .method("GET")
            .uri(uri)
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = tower::ServiceExt::oneshot(harness.router(), request)
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value = serde_json::from_slice(&body)
            .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(&body).into()));
        (status, value)
    }

    /// POST with a JSON body, returning the status and the body as text —
    /// the shape every refusal assertion below needs, since a refusal's
    /// body is prose rather than JSON.
    async fn post_text(
        harness: &rest_harness::Harness,
        uri: &str,
        body: serde_json::Value,
    ) -> (axum::http::StatusCode, String) {
        let request = axum::http::Request::builder()
            .method("POST")
            .uri(uri)
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(body.to_string()))
            .unwrap();
        let response = tower::ServiceExt::oneshot(harness.router(), request)
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8_lossy(&body).into_owned())
    }

    /// The `id` of every row of a session-list body, in order.
    fn row_ids(value: &serde_json::Value) -> Vec<String> {
        value["sessions"]
            .as_array()
            .expect("sessions is an array")
            .iter()
            .map(|row| row["id"].as_str().expect("id is a string").to_string())
            .collect()
    }

    /// A three-host fleet where every host has sessions, sharing one
    /// interleaved creation order — the fixture the merge, ordering, and
    /// staleness assertions all need.
    ///
    /// The interleaving is the point: `created_at` values alternate between
    /// hosts, so a merge that concatenated per-host lists (or sorted only
    /// within a host) would produce a visibly different order rather than
    /// happening to agree.
    async fn three_host_fleet() -> (rest_harness::Harness, store::HostId, store::HostId) {
        let (builder, alpha) = rest_harness::FleetBuilder::new()
            .await
            .local(rest_harness::HostScript {
                identity: Some("identity-local".to_string()),
                sessions: vec![rest_harness::session("local-mid", 200)],
                ..rest_harness::HostScript::default()
            })
            .await
            .ssh(
                "user@alpha",
                rest_harness::HostScript {
                    identity: Some("identity-alpha".to_string()),
                    sessions: vec![
                        rest_harness::session("alpha-new", 300),
                        rest_harness::session("alpha-old", 100),
                    ],
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let (builder, beta) = builder
            .ssh(
                "user@beta",
                rest_harness::HostScript {
                    identity: Some("identity-beta".to_string()),
                    sessions: vec![
                        rest_harness::session("beta-newest", 400),
                        rest_harness::session("beta-oldest", 50),
                    ],
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        let local = rest_harness::local_id(&harness.store).await;
        for host in [local, alpha, beta] {
            harness.await_refreshed(host).await;
        }
        (harness, alpha, beta)
    }

    /// The merged list is ONE list: every connected host's sessions in a
    /// single creation-time order, each row naming its host.
    ///
    /// SPEC.md promises "one flat list across all registered hosts, with
    /// each row saying which host it lives on", and the ordering half is
    /// what makes it a list rather than a concatenation. The fixture
    /// interleaves creation times across hosts specifically so a
    /// per-host-then-append implementation fails here instead of passing by
    /// coincidence.
    #[tokio::test]
    async fn the_session_list_merges_every_host_into_one_creation_order() {
        let (harness, alpha, beta) = three_host_fleet().await;
        let local = rest_harness::local_id(&harness.store).await;

        let (status, value) = get_json(&harness, "/api/sessions").await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            row_ids(&value),
            vec![
                "beta-newest",
                "alpha-new",
                "local-mid",
                "alpha-old",
                "beta-oldest"
            ],
            "the merge is creation-time descending across hosts, not host by host"
        );
        assert_eq!(value["total"], 5, "total is the merged count");

        let rows = value["sessions"].as_array().unwrap();
        assert_eq!(rows[0]["host"], beta);
        assert_eq!(rows[0]["host_name"], "user@beta");
        assert_eq!(rows[1]["host"], alpha);
        assert_eq!(rows[1]["host_name"], "user@alpha");
        assert_eq!(rows[2]["host"], local);
        assert_eq!(
            rows[2]["host_name"], "this machine",
            "the helm's own machine is described rather than addressed"
        );
        assert!(
            rows.iter().all(|row| row["stale"] == false),
            "every host is connected, so nothing is last-known knowledge"
        );
    }

    /// A host going dark must not remove its sessions from the list: they
    /// stay, marked stale, while every other host's rows keep their place
    /// in the same order.
    ///
    /// This is SPEC.md's central multi-host promise — "sessions on an
    /// unreachable host stay in the list from the helm's last-known
    /// knowledge, clearly marked stale, rather than vanishing" — at the
    /// REST boundary, where the UI actually reads it.
    #[tokio::test]
    async fn a_down_hosts_sessions_stay_listed_and_marked_stale() {
        let (harness, alpha, beta) = three_host_fleet().await;

        harness.fleet.take_down(beta);
        harness
            .await_state(beta, |state| state.phase() == "unreachable-reprobing")
            .await;

        let (status, value) = get_json(&harness, "/api/sessions").await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            row_ids(&value),
            vec![
                "beta-newest",
                "alpha-new",
                "local-mid",
                "alpha-old",
                "beta-oldest"
            ],
            "a down host's rows keep their place in the merged order"
        );
        let rows = value["sessions"].as_array().unwrap();
        for row in rows {
            let expected_stale = row["host"] == beta;
            assert_eq!(
                row["stale"], expected_stale,
                "only the down host's rows are stale: {row}"
            );
        }
        assert_eq!(
            rows[1]["host"], alpha,
            "one host going down must not disturb another's rows"
        );
    }

    /// The helm-level cursor walks the MERGED order to exhaustion, page by
    /// page, crossing host boundaries mid-page without any host being asked
    /// anything.
    ///
    /// The decoupling PLAN_M6.md item 5 requires is what makes this
    /// possible at all: the pages come from helm.db, so a page boundary can
    /// fall anywhere in the merged order rather than being pinned to where
    /// some host's own wire page happened to end.
    #[tokio::test]
    async fn the_helm_cursor_walks_the_merged_order_across_host_boundaries() {
        let (harness, _alpha, _beta) = three_host_fleet().await;

        let mut walked: Vec<String> = Vec::new();
        let mut uri = "/api/sessions?limit=2".to_string();
        for _ in 0..10 {
            let (status, value) = get_json(&harness, &uri).await;
            assert_eq!(status, axum::http::StatusCode::OK);
            assert_eq!(value["total"], 5, "every page reports the merged total");
            walked.extend(row_ids(&value));
            match value["next_cursor"].as_str() {
                None => break,
                Some(cursor) => uri = format!("/api/sessions?limit=2&cursor={cursor}"),
            }
        }
        assert_eq!(
            walked,
            vec![
                "beta-newest",
                "alpha-new",
                "local-mid",
                "alpha-old",
                "beta-oldest"
            ],
            "the walk must reproduce the whole merged order exactly once"
        );

        let (status, body) = get_json(&harness, "/api/sessions?cursor=not-a-cursor").await;
        assert_eq!(
            status,
            axum::http::StatusCode::BAD_REQUEST,
            "a tampered cursor is a clean refusal: {body}"
        );
        let (status, _) = get_json(&harness, "/api/sessions?limit=0").await;
        assert_eq!(
            status,
            axum::http::StatusCode::BAD_REQUEST,
            "a zero limit could never make progress through the pages"
        );
    }

    /// A session operation must reach the host that OWNS the session, and
    /// only that host.
    ///
    /// The assertion needs two live hosts, because a single-host fleet
    /// cannot distinguish "routed correctly" from "sent to the only
    /// connection there is" — which is exactly the bug this whole lookup
    /// exists to prevent once a fleet has more than one member.
    #[tokio::test]
    async fn a_session_operation_routes_to_the_host_that_owns_it() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        // The host that must NOT be asked, and the one that must.
        let (alpha_client, alpha_peer) = tokio::io::duplex(64 * 1024);
        let alpha_task = tokio::spawn(silent_supervisor(alpha_peer));
        let (beta_client, beta_peer) = tokio::io::duplex(64 * 1024);
        let beta_task = tokio::spawn(async move {
            let (r, w) = tokio::io::split(beta_peer);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::StopSession { req_id, session_id } = request else {
                panic!("expected StopSession, got {request:?}");
            };
            assert_eq!(session_id, "beta-1");
            writer
                .write_control(&ControlMsg::SessionStopped { req_id })
                .await
                .unwrap();
        });

        let (builder, alpha) = rest_harness::FleetBuilder::new()
            .await
            .ssh(
                "user@alpha",
                rest_harness::HostScript {
                    identity: Some("identity-alpha".to_string()),
                    sessions: vec![rest_harness::session("alpha-1", 100)],
                    peer: Some(alpha_client),
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let (builder, beta) = builder
            .ssh(
                "user@beta",
                rest_harness::HostScript {
                    identity: Some("identity-beta".to_string()),
                    sessions: vec![rest_harness::session("beta-1", 200)],
                    peer: Some(beta_client),
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        harness.await_refreshed(alpha).await;
        harness.await_refreshed(beta).await;

        let (status, body) =
            post_text(&harness, "/api/sessions/beta-1/stop", serde_json::json!({})).await;
        assert_eq!(status, axum::http::StatusCode::OK, "{body}");

        beta_task.await.unwrap();
        alpha_task.await.unwrap();
    }

    /// Every non-connected state refuses a session operation, names itself
    /// in the error, and queues nothing.
    ///
    /// Three states are reached the way they are reached in life — a host
    /// switched off, a host upgraded past this helm's protocol, a host
    /// reinstalled under a new identity — and the assertion is deliberately
    /// uniform across them. SPEC.md refuses lifecycle operations against an
    /// unreachable host; PLAN_M6.md item 5 makes explicit that unreachable
    /// is not special, only common, and that all of these refuse alike. A
    /// helm that special-cased one of them would pass a test written per
    /// state and fail this one.
    ///
    /// The other three states — `connecting`, `duplicate`, `retired` — are
    /// covered by `refusal_text_names_every_non_connected_state` rather than
    /// here. Reaching them through the integration path is either
    /// impractical (a connecting host has to be caught mid-ladder) or
    /// meaningless for a SESSION operation (a duplicate entry and a retired
    /// one connect nothing, so they can never have cached a session to
    /// operate on). What actually has to hold for all six is that the
    /// refusal names the state, and that is what the sibling test pins —
    /// against the same function this path uses.
    ///
    /// Each host CONNECTS first, so its session is genuinely in the merged
    /// view before the host breaks — otherwise there would be nothing to
    /// operate on and the 409 under test would be a 404 instead.
    #[tokio::test]
    async fn every_non_connected_state_refuses_a_session_operation_naming_itself() {
        struct Case {
            /// How the far side changes under the host's feet.
            break_it: fn(&rest_harness::ScriptedFleet, store::HostId),
            /// The phase label the refusal must carry — the same
            /// vocabulary `/api/hosts` chips and the log lines use.
            phase: &'static str,
        }

        let cases = [
            Case {
                break_it: |fleet, host| fleet.take_down(host),
                phase: "unreachable-reprobing",
            },
            Case {
                break_it: |fleet, host| {
                    fleet.edit(host, |script| {
                        script.protocol = farhelm_proto::PROTOCOL_VERSION + 1;
                    });
                    fleet.kill_connection(host);
                },
                phase: "version-skew",
            },
            Case {
                break_it: |fleet, host| {
                    fleet.edit(host, |script| {
                        script.identity = Some("a-different-install".to_string());
                    });
                    fleet.kill_connection(host);
                },
                phase: "identity-mismatch",
            },
        ];

        for case in cases {
            let (builder, host) = rest_harness::FleetBuilder::new()
                .await
                .ssh(
                    "user@breaks",
                    rest_harness::HostScript {
                        identity: Some("identity-original".to_string()),
                        sessions: vec![rest_harness::session("owned", 100)],
                        ..rest_harness::HostScript::default()
                    },
                )
                .await;
            let harness = builder.start().await;
            harness.await_refreshed(host).await;

            (case.break_it)(&harness.fleet, host);
            harness
                .await_state(host, |state| state.phase() == case.phase)
                .await;

            let (status, body) =
                post_text(&harness, "/api/sessions/owned/stop", serde_json::json!({})).await;
            assert_eq!(
                status,
                axum::http::StatusCode::CONFLICT,
                "a {} host must refuse rather than 404 or 500: {body}",
                case.phase
            );
            assert!(
                body.contains(case.phase),
                "the refusal must name the host's state ({}): {body}",
                case.phase
            );
            assert!(
                body.contains("nothing was queued"),
                "the refusal must say nothing was deferred: {body}"
            );

            // Still listed, and still marked as what it is: refusing an
            // operation must not make the session disappear.
            let (_, value) = get_json(&harness, "/api/sessions").await;
            assert_eq!(row_ids(&value), vec!["owned"]);
            assert_eq!(value["sessions"][0]["stale"], true);
        }
    }

    /// Every one of the six non-connected states must name itself in the
    /// refusal, including the three the integration path above cannot
    /// practically reach.
    ///
    /// Asserted against `refusal_text` directly — the single function every
    /// refusal in this crate is built from — because what matters is that
    /// no state falls through to a generic message. A seventh state added
    /// later without a case here fails this test rather than silently
    /// refusing operations with nothing a user can act on.
    #[test]
    fn refusal_text_names_every_non_connected_state() {
        use crate::manager::{HostState, UnreachableCause};

        let cases = [
            (
                HostState::Connecting {
                    attempt: 2,
                    last_error: Some("ssh: connect to host timed out".to_string()),
                },
                "connecting",
                "timed out",
            ),
            (
                HostState::Unreachable {
                    cause: UnreachableCause::TransportFailure,
                    last_error: "no route to host".to_string(),
                },
                "unreachable-reprobing",
                "no route to host",
            ),
            (
                HostState::VersionSkew {
                    peer_protocol: 9,
                    peer_build: "0.0.2".to_string(),
                    our_protocol: 8,
                    our_build: "0.0.1".to_string(),
                    remediation: "update this helm".to_string(),
                },
                "version-skew",
                "update this helm",
            ),
            (
                HostState::IdentityMismatch {
                    recorded: "identity-old".to_string(),
                    reported: "identity-new".to_string(),
                },
                "identity-mismatch",
                "identity-new",
            ),
            (
                HostState::Duplicate {
                    twin: 7,
                    identity: "identity-shared".to_string(),
                },
                "duplicate",
                "host 7",
            ),
            (
                HostState::Retired {
                    reason: "its connection actor panicked".to_string(),
                },
                "retired",
                "panicked",
            ),
        ];
        assert_eq!(
            cases.len(),
            6,
            "all six non-connected states are covered; a seventh needs a case here"
        );
        for (state, phase, detail) in cases {
            let text = super::refusal_text(42, &state);
            assert!(
                text.contains(phase),
                "the refusal must name the phase {phase:?}: {text}"
            );
            assert!(
                text.contains(detail),
                "the refusal must carry the state's own evidence ({detail:?}): {text}"
            );
            assert!(
                text.contains("nothing was queued"),
                "every refusal must say nothing was deferred: {text}"
            );
            assert!(
                text.contains("host 42"),
                "every refusal must name the host: {text}"
            );
        }
    }

    /// Creating on a non-connected host is a PRECONDITION FAILURE: a
    /// visible error naming the host's state, and no session anywhere.
    ///
    /// SPEC.md lists "unreachable host" beside "nonexistent directory" as a
    /// precondition that fails a create outright, and the silent supervisor
    /// is what turns "no session anywhere" into an assertion rather than a
    /// claim — a helm that refused the caller but still sent the create
    /// would leave a real agent running that nobody asked for.
    #[tokio::test]
    async fn creating_on_a_non_connected_host_is_refused_with_no_session() {
        let (alpha_client, alpha_peer) = tokio::io::duplex(64 * 1024);
        let alpha_task = tokio::spawn(silent_supervisor(alpha_peer));

        let (builder, down) = rest_harness::FleetBuilder::new()
            .await
            .local(rest_harness::HostScript {
                identity: Some("identity-local".to_string()),
                peer: Some(alpha_client),
                ..rest_harness::HostScript::default()
            })
            .await
            .ssh(
                "user@down",
                rest_harness::HostScript {
                    reachable: false,
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        let local = rest_harness::local_id(&harness.store).await;
        harness.await_refreshed(local).await;
        harness
            .await_state(down, |state| state.phase() == "unreachable-reprobing")
            .await;

        let (status, body) = post_text(
            &harness,
            "/api/sessions",
            serde_json::json!({
                "cwd": "/tmp",
                "invocation": "agent",
                "host": down,
            }),
        )
        .await;
        assert_eq!(
            status,
            axum::http::StatusCode::CONFLICT,
            "a create against a down host fails as a precondition: {body}"
        );
        assert!(
            body.contains("unreachable-reprobing"),
            "the error must name the host's state: {body}"
        );

        // The connected host must not have been used as a fallback: a
        // create that silently landed somewhere else would be worse than
        // one that failed.
        alpha_task.await.unwrap();
    }

    /// A create that names no host lands on the reserved LOCAL row, and one
    /// that names a host lands there instead.
    ///
    /// The default is the tail of SPEC.md's own creation default ("…else
    /// the helm's own host"), and keeping it a default rather than a
    /// requirement is what leaves a curl or a script meaning the obvious
    /// thing. Both halves are asserted against a two-host fleet, since a
    /// single-host fleet cannot tell a default from an accident.
    #[tokio::test]
    async fn a_create_defaults_to_the_local_host_and_honors_an_explicit_one() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        /// Answer one `CreateSession` with a session whose id says which
        /// host answered.
        async fn create_once(peer_side: tokio::io::DuplexStream, id: &'static str) {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::CreateSession { req_id, .. } = request else {
                panic!("expected CreateSession, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::SessionCreated {
                    req_id,
                    session: rest_harness::session(id, 1),
                })
                .await
                .unwrap();
        }

        for (explicit, expected) in [(false, "created-on-local"), (true, "created-on-remote")] {
            let (local_client, local_peer) = tokio::io::duplex(64 * 1024);
            let local_task = tokio::spawn(create_once(local_peer, "created-on-local"));
            let (remote_client, remote_peer) = tokio::io::duplex(64 * 1024);
            let remote_task = tokio::spawn(create_once(remote_peer, "created-on-remote"));

            let (builder, remote) = rest_harness::FleetBuilder::new()
                .await
                .local(rest_harness::HostScript {
                    identity: Some("identity-local".to_string()),
                    peer: Some(local_client),
                    ..rest_harness::HostScript::default()
                })
                .await
                .ssh(
                    "user@remote",
                    rest_harness::HostScript {
                        identity: Some("identity-remote".to_string()),
                        peer: Some(remote_client),
                        ..rest_harness::HostScript::default()
                    },
                )
                .await;
            let harness = builder.start().await;
            let local = rest_harness::local_id(&harness.store).await;
            harness.await_refreshed(local).await;
            harness.await_refreshed(remote).await;

            let mut body = serde_json::json!({ "cwd": "/tmp", "invocation": "agent" });
            if explicit {
                body["host"] = serde_json::json!(remote);
            }
            let (status, text) = post_text(&harness, "/api/sessions", body).await;
            assert_eq!(status, axum::http::StatusCode::OK, "{text}");
            let created: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(
                created["id"],
                expected,
                "a create with host {} must land on {expected}",
                if explicit { "named" } else { "omitted" }
            );

            // Whichever peer was not chosen is still parked on its read;
            // aborting is how this test declines to wait for it.
            local_task.abort();
            remote_task.abort();
        }
    }

    /// A terminal socket for a session on a non-connected host must be
    /// refused the same way every other operation is — and must SAY so, as
    /// the ordinary `detached` notice, rather than closing bare.
    ///
    /// SPEC.md wants "no terminal to show and no pretense of one", and a
    /// silent close is exactly a pretense the browser would blame on the
    /// network. Riding the existing notice shape is also what lets the UI
    /// render this without a new message type.
    #[tokio::test]
    async fn a_terminal_socket_on_a_down_host_is_refused_with_the_hosts_state() {
        let (builder, host) = rest_harness::FleetBuilder::new()
            .await
            .ssh(
                "user@breaks",
                rest_harness::HostScript {
                    identity: Some("identity-original".to_string()),
                    sessions: vec![rest_harness::session("owned", 100)],
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let mut harness = builder.start().await;
        harness.await_refreshed(host).await;
        harness.fleet.take_down(host);
        harness
            .await_state(host, |state| state.phase() == "unreachable-reprobing")
            .await;

        let addr = harness.serve().await;
        let mut ws = WsTestClient::connect(addr, "/api/sessions/owned/term").await;
        let (opcode, payload) = ws.recv().await.expect("a notice, not a bare close");
        assert_eq!(opcode, 1, "the refusal arrives as a text notice");
        let notice: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(notice["type"], "detached");
        let reason = notice["reason"].as_str().unwrap();
        assert!(
            reason.contains("unreachable-reprobing"),
            "the notice must name the host's state: {reason}"
        );
        assert!(
            ws.recv().await.is_none(),
            "the socket closes once its refusal is delivered"
        );
    }

    /// A session created HERE must be operable at once — the create's own
    /// reply is not a promise the helm may then take a refresh interval to
    /// honour.
    ///
    /// This is a regression test for a real gap, not a hypothetical: owner
    /// routing resolves hosts from the cache, and for a while `create`
    /// never seeded it, so the create dialog's own flow — create, then open
    /// the terminal — 404'd until the owning host's next refresh. Every
    /// verb is exercised because they route through one lookup and the
    /// failure was in the lookup, not in any one of them.
    ///
    /// No refresh tick is allowed to rescue it: the harness's cadence
    /// refreshes once at connect and then not for an hour, so anything that
    /// works here worked because the create seeded it.
    #[tokio::test]
    async fn a_session_created_here_is_routable_before_any_refresh() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            // Create, then answer every later request for the session it
            // just minted. The point is that these are REACHED at all.
            loop {
                let Ok(Some(frame)) = reader.read_frame().await else {
                    return;
                };
                match parse_control(&frame) {
                    Ok(ControlMsg::CreateSession { req_id, .. }) => writer
                        .write_control(&ControlMsg::SessionCreated {
                            req_id,
                            session: rest_harness::session("brand-new", 900),
                        })
                        .await
                        .unwrap(),
                    Ok(ControlMsg::StopSession { req_id, session_id }) => {
                        assert_eq!(session_id, "brand-new");
                        writer
                            .write_control(&ControlMsg::SessionStopped { req_id })
                            .await
                            .unwrap();
                    }
                    Ok(ControlMsg::RenameSession {
                        req_id, session_id, ..
                    }) => {
                        assert_eq!(session_id, "brand-new");
                        writer
                            .write_control(&ControlMsg::SessionRenamed {
                                req_id,
                                session: rest_harness::session("brand-new", 900),
                            })
                            .await
                            .unwrap();
                    }
                    _ => return,
                }
            }
        });

        // The scripted host's own list is EMPTY, so nothing but the create
        // can put this session where routing will find it.
        let harness = rest_harness::spliced_helm_listing(client_side, Vec::new()).await;

        let (status, body) = post_text(
            &harness,
            "/api/sessions",
            serde_json::json!({ "cwd": "/tmp", "invocation": "agent" }),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK, "{body}");
        let created: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(created["id"], "brand-new");

        for (uri, request_body) in [
            ("/api/sessions/brand-new/stop", serde_json::json!({})),
            (
                "/api/sessions/brand-new/rename",
                serde_json::json!({ "title": "renamed" }),
            ),
        ] {
            let (status, body) = post_text(&harness, uri, request_body).await;
            assert_eq!(
                status,
                axum::http::StatusCode::OK,
                "{uri} must route immediately after the create that made it: {body}"
            );
        }

        // Deliberately NOT asserted here: the detail route asks the owning
        // host live rather than reading the cache, so what it reports is
        // the scripted list (empty) and not the seed. That is the correct
        // division — the seed exists to make the session ROUTABLE, and the
        // host remains authority for what it is — and asserting otherwise
        // would pin the cache as a detail-serving layer, which PLAN_M6.md
        // explicitly rules out.
        peer.abort();
    }

    /// A connected host reporting NO identity caches nothing, and its
    /// sessions must still list and route — then vanish when it drops.
    ///
    /// The gap this closes was total and silent: the manager deliberately
    /// skips persisting an identity-less host's refreshes (the cache write
    /// is identity-bound), while aggregation and owner lookup read only
    /// persisted rows — so such a host read as connected and EMPTY, with
    /// its sessions absent from the list and unroutable for every
    /// operation.
    ///
    /// The disappearance half is equally deliberate and is asserted here so
    /// nobody "fixes" it later: with no durable copy there is nothing to
    /// vouch for these rows once the connection is gone, so they must not
    /// linger as stale entries the helm cannot stand behind.
    #[tokio::test]
    async fn an_identity_less_hosts_sessions_serve_while_connected_and_vanish_after() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::StopSession { req_id, session_id } = request else {
                panic!("expected StopSession, got {request:?}");
            };
            assert_eq!(session_id, "unbound-1");
            writer
                .write_control(&ControlMsg::SessionStopped { req_id })
                .await
                .unwrap();
        });

        let (builder, host) = rest_harness::FleetBuilder::new()
            .await
            .ssh(
                "user@no-identity",
                rest_harness::HostScript {
                    // A supervisor with no standing to mint one reports
                    // none; the wire allows it and the store cannot bind a
                    // cache write to it.
                    identity: None,
                    sessions: vec![rest_harness::session("unbound-1", 100)],
                    peer: Some(client_side),
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        harness.await_refreshed(host).await;

        let (status, value) = get_json(&harness, "/api/sessions").await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            row_ids(&value),
            vec!["unbound-1"],
            "an identity-less host's sessions must appear in the merged list"
        );
        assert_eq!(value["total"], 1, "and must be counted in the total");
        assert_eq!(value["sessions"][0]["host"], host);
        assert_eq!(
            value["sessions"][0]["stale"], false,
            "it is connected, so these are live rows"
        );

        // Nothing is persisted — the identity binding has nothing to bind
        // to — which is exactly why the manager has to hold them.
        assert!(
            harness
                .store
                .cached_sessions(host)
                .await
                .expect("cache read")
                .is_empty(),
            "an identity-less host must write no cache at all"
        );

        let (status, body) = post_text(
            &harness,
            "/api/sessions/unbound-1/stop",
            serde_json::json!({}),
        )
        .await;
        assert_eq!(
            status,
            axum::http::StatusCode::OK,
            "an identity-less host's sessions must route like any other: {body}"
        );
        peer.await.unwrap();

        harness.fleet.take_down(host);
        harness
            .await_state(host, |state| state.phase() == "unreachable-reprobing")
            .await;

        let (_, value) = get_json(&harness, "/api/sessions").await;
        assert!(
            row_ids(&value).is_empty(),
            "with no durable copy there is nothing to serve stale: {value}"
        );
        assert_eq!(value["total"], 0);
        let (status, _) = get_json(&harness, "/api/sessions/unbound-1").await;
        assert_eq!(
            status,
            axum::http::StatusCode::NOT_FOUND,
            "and nothing to show behind a host-unreachable notice either"
        );
    }

    /// A hostile or buggy supervisor claiming another host's session id must
    /// not be able to steer an operation to the wrong machine — and while
    /// the claim STANDS, no operation goes anywhere at all.
    ///
    /// Two rules, and the second is the one worth being explicit about.
    /// helm.db refuses the second claim outright, so the LIST is coherent:
    /// the first host keeps the session and the impostor's row is dropped.
    /// But a session two hosts both report is genuinely ambiguous, and the
    /// helm has no basis for deciding which of them the user meant — so
    /// ROUTING fails closed for as long as both keep reporting it, rather
    /// than quietly choosing the one that happened to cache first.
    ///
    /// The contest is refresh STATE, not a remembered incident: when the
    /// impostor stops reporting the id, the next drain rebuilds its
    /// contested set without it and routing resumes with no intervention.
    /// That second half is asserted here because it is what makes the
    /// refusal a temporary, self-clearing condition rather than a session
    /// bricked by someone else's bug.
    #[tokio::test]
    async fn a_second_host_claiming_a_session_id_never_steals_its_routing() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let (owner_client, owner_peer) = tokio::io::duplex(64 * 1024);
        let owner_task = tokio::spawn(async move {
            let (r, w) = tokio::io::split(owner_peer);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::StopSession { req_id, session_id } = request else {
                panic!("expected StopSession, got {request:?}");
            };
            assert_eq!(session_id, "contested");
            writer
                .write_control(&ControlMsg::SessionStopped { req_id })
                .await
                .unwrap();
        });
        let (impostor_client, impostor_peer) = tokio::io::duplex(64 * 1024);
        let impostor_task = tokio::spawn(silent_supervisor(impostor_peer));

        let (builder, owner) = rest_harness::FleetBuilder::new()
            .await
            .ssh(
                "user@owner",
                rest_harness::HostScript {
                    identity: Some("identity-owner".to_string()),
                    sessions: vec![rest_harness::session("contested", 100)],
                    peer: Some(owner_client),
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let (builder, impostor) = builder
            .ssh(
                "user@impostor",
                rest_harness::HostScript {
                    identity: Some("identity-impostor".to_string()),
                    // The same id, from a machine that does not own it.
                    sessions: vec![rest_harness::session("contested", 100)],
                    peer: Some(impostor_client),
                    // Held down until the owner has cached, so "first claim
                    // holds" has a defined first. Two hosts racing to claim
                    // one id is a real situation and either may win it, but
                    // a test whose subject is what happens to the LOSER
                    // cannot also leave who loses to chance.
                    reachable: false,
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        harness.await_refreshed(owner).await;

        harness
            .fleet
            .edit(impostor, |script| script.reachable = true);
        harness
            .manager
            .retry_now(impostor)
            .await
            .expect("the impostor is registered");
        harness.await_refreshed(impostor).await;

        let (_, value) = get_json(&harness, "/api/sessions").await;
        assert_eq!(
            row_ids(&value),
            vec!["contested"],
            "the contested id appears exactly once, not once per claimant: {value}"
        );
        assert_eq!(
            value["sessions"][0]["host"], owner,
            "the first claim holds; the later claimant's row is dropped"
        );

        // While BOTH report it, there is no honest owner to route to.
        let (status, body) = post_text(
            &harness,
            "/api/sessions/contested/stop",
            serde_json::json!({}),
        )
        .await;
        assert_eq!(
            status,
            axum::http::StatusCode::CONFLICT,
            "a session two hosts both claim must not be routed to either: {body}"
        );
        assert!(
            body.contains(&owner.to_string()) && body.contains(&impostor.to_string()),
            "and the refusal must name both candidates so the user can fix it: {body}"
        );

        // The impostor stops claiming it. Nothing is told to forget
        // anything — the contest is rebuilt from the next drain's evidence,
        // and that evidence no longer contains the id.
        harness
            .fleet
            .edit(impostor, |script| script.sessions = Vec::new());
        harness.fleet.kill_connection(impostor);
        harness
            .await_refreshed_as(impostor, "identity-impostor", 0)
            .await;

        let (status, body) = post_text(
            &harness,
            "/api/sessions/contested/stop",
            serde_json::json!({}),
        )
        .await;
        assert_eq!(
            status,
            axum::http::StatusCode::OK,
            "a contest clears itself when the claimant stops claiming: {body}"
        );
        owner_task.await.unwrap();
        // The impostor must never have been asked anything about it.
        impostor_task.await.unwrap();

        let (status, value) = get_json(&harness, "/api/sessions/contested").await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            value["host"], owner,
            "the detail route and the routing decision must name the SAME host"
        );
    }

    /// A refresh whose drain PREDATES a create must not erase the create.
    ///
    /// The window is wide and entirely ordinary: a refresh drains a host's
    /// whole list over the network, a create lands during that drain and is
    /// recorded, and the drain then commits a wholesale replacement built
    /// from a snapshot in which the new session did not exist. The caller
    /// has already been told its session exists; the list and the routing
    /// would then contradict the answer they just gave it.
    ///
    /// Driven by a BARRIER rather than by timing: the scripted host's second
    /// list reply is held until the create has completed, so the
    /// interleaving under test is the one that actually happens rather than
    /// whichever one a sleep happened to produce.
    #[tokio::test]
    async fn a_refresh_that_predates_a_create_cannot_erase_it() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            loop {
                let Ok(Some(frame)) = reader.read_frame().await else {
                    return;
                };
                let Ok(ControlMsg::CreateSession { req_id, .. }) = parse_control(&frame) else {
                    return;
                };
                writer
                    .write_control(&ControlMsg::SessionCreated {
                        req_id,
                        session: rest_harness::session("created-mid-drain", 900),
                    })
                    .await
                    .unwrap();
            }
        });

        // Refreshing briskly, so a second walk really is in flight while the
        // create runs. The host's canned list never mentions the new
        // session — which is the point: it describes the world before it.
        let harness = rest_harness::FleetBuilder::new()
            .await
            .refresh_every(std::time::Duration::from_millis(20))
            .local(rest_harness::HostScript {
                identity: Some("local-identity".to_string()),
                sessions: vec![rest_harness::session("pre-existing", 100)],
                peer: Some(client_side),
                ..rest_harness::HostScript::default()
            })
            .await
            .start()
            .await;
        let local = rest_harness::local_id(&harness.store).await;
        harness.await_refreshed(local).await;

        // Arm the barrier, then wait until the held walk has actually
        // STARTED: from here on, whatever it eventually replies describes a
        // world that predates the create below.
        let release = harness.fleet.hold_next_list(local);
        harness.fleet.await_list_requests(2).await;

        let (status, body) = post_text(
            &harness,
            "/api/sessions",
            serde_json::json!({ "cwd": "/tmp", "invocation": "agent" }),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK, "{body}");

        // The host now genuinely has the session, as a real one would the
        // moment it answered the create. Only the HELD reply — built before
        // any of this — still describes the world without it.
        harness.fleet.edit(local, |script| {
            script.sessions = vec![
                rest_harness::session("created-mid-drain", 900),
                rest_harness::session("pre-existing", 100),
            ];
        });

        // Let the stale walk commit — or rather, discover that it may not.
        let _ = release.send(());
        // The held walk has committed (or declined to) by the time the NEXT
        // one has started, which is a state the fleet reports rather than a
        // duration this test has to guess at.
        harness.fleet.await_list_requests(3).await;

        let (status, value) = get_json(&harness, "/api/sessions").await;
        assert_eq!(status, axum::http::StatusCode::OK);
        let mut ids = row_ids(&value);
        ids.sort();
        assert_eq!(
            ids,
            vec!["created-mid-drain", "pre-existing"],
            "a refresh built before the create must not erase it: {value}"
        );

        // And it is routable, which is the promise the create made.
        let (host, _) = resolve_owner(&harness.state, "created-mid-drain")
            .await
            .expect("the created session must still have an owner");
        assert_eq!(host, local);
        peer.abort();
    }

    /// A byte-bounded persisted scan must FENCE the merge: a live host's
    /// rows may not carry the cursor past cached rows nobody has been shown.
    ///
    /// The interleaving is specific and the loss is permanent. The store's
    /// scan stops on its byte bound having returned FEWER rows than the
    /// page asked for, so the merge still has capacity — and fills it from
    /// an identity-less host's in-memory list, whose rows sort after the
    /// cached ones the scan never reached. The page's cursor then names a
    /// live row, and the next page resumes after it: every cached row
    /// between the byte cut and that position is skipped, forever, with
    /// nothing about either page looking wrong.
    ///
    /// The fixture is exactly that shape — one fat cached row, an unseen
    /// cached successor, and a live row that sorts between them by time.
    #[tokio::test]
    async fn a_byte_cut_persisted_scan_fences_the_merge() {
        let fat = farhelm_proto::SessionInfo {
            // Alone larger than the page budget, so the scan stops right
            // after it with a successor still unread.
            title: "x".repeat(5 * 1024 * 1024),
            ..rest_harness::session("cached-fat", 500)
        };
        let (builder, cached_host) = rest_harness::FleetBuilder::new()
            .await
            .ssh(
                "user@cached",
                rest_harness::HostScript {
                    identity: Some("identity-cached".to_string()),
                    // The successor sorts LAST, so a merge that ran past
                    // the fence would leave it behind.
                    sessions: vec![fat, rest_harness::session("cached-next", 100)],
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let (builder, live_host) = builder
            .ssh(
                "user@live",
                rest_harness::HostScript {
                    // No identity: this host caches nothing and serves from
                    // the manager's memory, which is the other side of the
                    // merge.
                    identity: None,
                    sessions: vec![rest_harness::session("live-middle", 300)],
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        harness.await_refreshed(cached_host).await;
        harness.await_refreshed(live_host).await;

        // Walk the whole list one page at a time. Every row must appear
        // exactly once, in order — the property a fence-less merge breaks
        // silently.
        let mut walked: Vec<String> = Vec::new();
        let mut uri = "/api/sessions?limit=10".to_string();
        for _ in 0..10 {
            let (status, value) = get_json(&harness, &uri).await;
            assert_eq!(status, axum::http::StatusCode::OK);
            walked.extend(row_ids(&value));
            match value["next_cursor"].as_str() {
                None => break,
                Some(cursor) => uri = format!("/api/sessions?limit=10&cursor={cursor}"),
            }
        }
        assert_eq!(
            walked,
            vec!["cached-fat", "live-middle", "cached-next"],
            "every row exactly once, in the merged order — a fence-less merge loses the cached \
             row after the byte cut"
        );
    }

    /// The helm cursor must survive sessions coming and going between
    /// pages, and `truncated` must be true on every page but the last.
    ///
    /// The stability half is the whole reason the cursor encodes a KEY: an
    /// offset would shift under both mutations and silently re-serve or skip
    /// a row, with nothing a caller could observe. The `truncated` half is
    /// what the pre-M6 UI reads to draw "showing N of M", so it has to mean
    /// something exact — "there is a next page" — rather than approximately.
    #[tokio::test]
    async fn a_page_walk_survives_creation_and_deletion_between_pages() {
        let (harness, alpha, _beta) = three_host_fleet().await;

        let (_, first) = get_json(&harness, "/api/sessions?limit=2").await;
        assert_eq!(row_ids(&first), vec!["beta-newest", "alpha-new"]);
        assert_eq!(
            first["truncated"], true,
            "entries remain, so this is not the final page"
        );
        let cursor = first["next_cursor"]
            .as_str()
            .expect("more pages")
            .to_string();

        // The row the cursor NAMES is deleted, and a brand-new session
        // appears at the very front of the order — the two mutations a walk
        // must be indifferent to.
        harness.fleet.edit(alpha, |script| {
            script.sessions = vec![
                rest_harness::session("alpha-brand-new", 9_999),
                rest_harness::session("alpha-old", 100),
            ];
        });
        harness.fleet.kill_connection(alpha);
        harness
            .await_state(alpha, |state| {
                matches!(
                    state,
                    crate::manager::HostState::Connected {
                        last_refresh: crate::manager::RefreshHealth::Ok { sessions: 2 },
                        ..
                    }
                )
            })
            .await;

        let (_, second) =
            get_json(&harness, &format!("/api/sessions?limit=2&cursor={cursor}")).await;
        assert_eq!(
            row_ids(&second),
            vec!["local-mid", "alpha-old"],
            "the walk resumes strictly after the deleted row's key, and never rewinds to the \
             newly created one"
        );
        let cursor = second["next_cursor"]
            .as_str()
            .expect("one more")
            .to_string();
        let (_, third) =
            get_json(&harness, &format!("/api/sessions?limit=2&cursor={cursor}")).await;
        assert_eq!(row_ids(&third), vec!["beta-oldest"]);
        assert_eq!(
            third["truncated"], false,
            "the final page says so, which is what stops a walking caller"
        );
        assert_eq!(third["next_cursor"], serde_json::Value::Null);
    }

    /// An over-large `?limit=` is refused rather than silently clamped.
    ///
    /// Silently clamping would leave a caller that asked for fifty thousand
    /// and got five thousand with no way to tell it had not got what it
    /// asked for — the reply looks identical to a genuinely short page.
    #[tokio::test]
    async fn an_over_large_page_limit_is_refused() {
        let (harness, _alpha, _beta) = three_host_fleet().await;
        let (status, body) = get_json(
            &harness,
            &format!(
                "/api/sessions?limit={}",
                crate::aggregate::MAX_PAGE_LIMIT + 1
            ),
        )
        .await;
        assert_eq!(
            status,
            axum::http::StatusCode::BAD_REQUEST,
            "an unbounded page is a request to do all the work at once: {body}"
        );
        let (status, _) = get_json(
            &harness,
            &format!("/api/sessions?limit={}", crate::aggregate::MAX_PAGE_LIMIT),
        )
        .await;
        assert_eq!(
            status,
            axum::http::StatusCode::OK,
            "the cap itself is legal"
        );
    }

    /// A create naming a host id nothing holds must 404, and must not fall
    /// back to any other host.
    ///
    /// The fallback is the dangerous half: a create that quietly landed on
    /// the local machine because the named host was gone would put a live
    /// agent somewhere the user never asked for, and the reply would look
    /// like success. The silent supervisor is what turns "no fallback" into
    /// an assertion rather than a claim.
    #[tokio::test]
    async fn creating_on_an_unknown_host_is_refused_without_falling_back() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(silent_supervisor(peer_side));
        let harness = rest_harness::spliced_helm(client_side).await;

        let (status, body) = post_text(
            &harness,
            "/api/sessions",
            serde_json::json!({ "cwd": "/tmp", "invocation": "agent", "host": 9999 }),
        )
        .await;
        assert_eq!(
            status,
            axum::http::StatusCode::NOT_FOUND,
            "a create naming a host nothing holds is a 404, not a create somewhere else: {body}"
        );
        peer.await.unwrap();
    }

    /// The stale list must survive a HELM restart: a fresh helm over the
    /// same helm.db, with the host still down and no ensure file, serves
    /// its sessions from the database alone.
    ///
    /// PLAN_M6.md's testing decisions are explicit that the restart leg runs
    /// WITHOUT the ensure file, because an ensure file would rebuild the
    /// registry entry and mask a broken persistence path — the assertion is
    /// that the destination, the identity, and the stale sessions all come
    /// from helm.db.
    #[tokio::test]
    async fn the_stale_list_survives_a_helm_restart_from_helm_db_alone() {
        let (builder, host) = rest_harness::FleetBuilder::new()
            .await
            .ssh(
                "user@remembered",
                rest_harness::HostScript {
                    identity: Some("identity-remembered".to_string()),
                    sessions: vec![rest_harness::session("survivor", 100)],
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let first = builder.start().await;
        first.await_refreshed(host).await;
        let (_, before) = get_json(&first, "/api/sessions").await;
        assert_eq!(row_ids(&before), vec!["survivor"]);

        // A NEW helm over the same database, with the host now down — the
        // manager, its actors, and the router are all built from scratch.
        let restarted = first.restart_with(|fleet| fleet.take_down(host)).await;
        restarted
            .await_state(host, |state| state.phase() == "unreachable-reprobing")
            .await;

        let (status, value) = get_json(&restarted, "/api/sessions").await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            row_ids(&value),
            vec!["survivor"],
            "the stale list must come back from helm.db alone: {value}"
        );
        assert_eq!(value["sessions"][0]["stale"], true);
        assert_eq!(value["sessions"][0]["host_name"], "user@remembered");

        let (_, hosts) = get_json(&restarted, "/api/hosts").await;
        let row = hosts["hosts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == host)
            .expect("the registry entry survived too");
        assert_eq!(
            row["identity"], "identity-remembered",
            "the identity is durable, not re-learned from a host that is down"
        );
    }

    /// A session created on an IDENTITY-LESS host must be routable at once,
    /// exactly like one created on a host that caches.
    ///
    /// Such a host writes no cache at all, so the create's durable seed has
    /// nowhere to go — and the version of this that only seeded the store
    /// skipped it silently, leaving every immediate operation 404ing on
    /// precisely the hosts whose sessions are hardest to see. The promise is
    /// "created here is routable now", and it cannot hold for one storage
    /// shape and not the other.
    #[tokio::test]
    async fn a_session_created_on_an_identity_less_host_is_routable_at_once() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            loop {
                let Ok(Some(frame)) = reader.read_frame().await else {
                    return;
                };
                match parse_control(&frame) {
                    Ok(ControlMsg::CreateSession { req_id, .. }) => writer
                        .write_control(&ControlMsg::SessionCreated {
                            req_id,
                            session: rest_harness::session("unbound-new", 900),
                        })
                        .await
                        .unwrap(),
                    Ok(ControlMsg::StopSession { req_id, session_id }) => {
                        assert_eq!(session_id, "unbound-new");
                        writer
                            .write_control(&ControlMsg::SessionStopped { req_id })
                            .await
                            .unwrap();
                    }
                    _ => return,
                }
            }
        });

        let (builder, host) = rest_harness::FleetBuilder::new()
            .await
            .ssh(
                "user@no-identity",
                rest_harness::HostScript {
                    identity: None,
                    sessions: Vec::new(),
                    peer: Some(client_side),
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        harness.await_refreshed(host).await;

        let (status, body) = post_text(
            &harness,
            "/api/sessions",
            serde_json::json!({ "cwd": "/tmp", "invocation": "agent", "host": host }),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK, "{body}");

        let (status, body) = post_text(
            &harness,
            "/api/sessions/unbound-new/stop",
            serde_json::json!({}),
        )
        .await;
        assert_eq!(
            status,
            axum::http::StatusCode::OK,
            "a session created on an identity-less host must route immediately too: {body}"
        );

        // And it is in the list, in order, without waiting for a refresh.
        let (_, value) = get_json(&harness, "/api/sessions").await;
        assert_eq!(row_ids(&value), vec!["unbound-new"]);
        assert_eq!(value["total"], 1);
        peer.abort();
    }

    /// A restart's reply must reach the LIST immediately, not at the owning
    /// host's next refresh tick — and a delete must leave it immediately
    /// too.
    ///
    /// The browser suite caught both as user-visible lies. A restart of an
    /// exited session succeeded while the list went on saying `exited` for a
    /// poll interval; and its own shared-session reset (delete, then create)
    /// left the deleted row listed beside the new one, so a strict locator
    /// found two rows where the test meant one. The merged view serves what
    /// the helm has RECORDED, so every mutation that changes what a session
    /// is — or whether it is — records the result.
    #[tokio::test]
    async fn a_restart_and_a_rename_reach_the_list_without_waiting_for_a_refresh() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let exited = farhelm_proto::SessionInfo {
            status: farhelm_proto::SessionStatus::Exited { exit_code: Some(1) },
            ..rest_harness::session("sess-1", 500)
        };
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let renamed = farhelm_proto::SessionInfo {
            title: "renamed-later".to_string(),
            ..rest_harness::session("sess-1", 500)
        };
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            loop {
                let Ok(Some(frame)) = reader.read_frame().await else {
                    return;
                };
                match parse_control(&frame) {
                    // The host now reports it ALIVE — the whole point of
                    // the restart the caller just made.
                    Ok(ControlMsg::RestartSession { req_id, .. }) => writer
                        .write_control(&ControlMsg::SessionRestarted {
                            req_id,
                            session: rest_harness::session("sess-1", 500),
                        })
                        .await
                        .unwrap(),
                    Ok(ControlMsg::RenameSession { req_id, .. }) => writer
                        .write_control(&ControlMsg::SessionRenamed {
                            req_id,
                            session: renamed.clone(),
                        })
                        .await
                        .unwrap(),
                    Ok(ControlMsg::DeleteSession { req_id, .. }) => writer
                        .write_control(&ControlMsg::SessionDeleted { req_id })
                        .await
                        .unwrap(),
                    _ => return,
                }
            }
        });

        // The cached row says `exited`, and the harness refreshes once an
        // hour — so anything the list shows differently was recorded by the
        // mutation itself.
        let harness = rest_harness::spliced_helm_listing(client_side, vec![exited]).await;
        let (_, before) = get_json(&harness, "/api/sessions").await;
        assert_eq!(before["sessions"][0]["status"]["state"], "exited");

        let (status, body) = post_text(
            &harness,
            "/api/sessions/sess-1/restart",
            serde_json::json!({ "mode": "fresh" }),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK, "{body}");
        let (_, after) = get_json(&harness, "/api/sessions").await;
        assert_eq!(
            after["sessions"][0]["status"]["state"], "running",
            "a completed restart must not leave the list showing the state it restarted FROM"
        );

        let (status, body) = post_text(
            &harness,
            "/api/sessions/sess-1/rename",
            serde_json::json!({ "title": "renamed-later" }),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK, "{body}");
        let (_, after) = get_json(&harness, "/api/sessions").await;
        assert_eq!(
            after["sessions"][0]["title"], "renamed-later",
            "and a completed rename must not either"
        );

        // A delete is the quadrant the browser suite found missing: the row
        // must be gone from the list the moment the delete answers, not at
        // the next refresh.
        let request = axum::http::Request::builder()
            .method("DELETE")
            .uri("/api/sessions/sess-1")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = tower::ServiceExt::oneshot(harness.router(), request)
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let (_, after) = get_json(&harness, "/api/sessions").await;
        assert!(
            row_ids(&after).is_empty(),
            "a deleted session must leave the list at once, or a delete-then-create shows both: \
             {after}"
        );
        peer.abort();
    }

    /// A mutation reply that says `Unknown` must not erase a status the
    /// helm already knew.
    ///
    /// This is the lost-restart-reply case, reduced to the fact that
    /// produced it. The browser suite's `a restart whose response is lost
    /// still recovers the terminal` restarts a LIVE session with the reply
    /// dropped on the client side, then reads the list and expects `alive`.
    /// The restart itself really happened — the supervisor relaunched, and
    /// the helm received and recorded the reply — but that reply carries
    /// `SessionStatus::Unknown` BY CONTRACT: at the instant it is built the
    /// pane exists and the agent's own `exec` inside it has not been
    /// observed, and `SessionStatus::Unknown`'s own docs are explicit that
    /// `ListSessions` is the only reply computing a real answer. Recording
    /// it verbatim answered a successful restart with "the helm has no
    /// idea", for a session it had definite knowledge about a moment
    /// earlier.
    ///
    /// Both directions are pinned, because the rule is narrow on purpose: a
    /// DEFINITE status in a reply is authoritative and wins immediately
    /// (that is what makes a restart show `alive` without a refresh), and
    /// only `Unknown` defers to what was already known. Every other field
    /// of the reply is taken as given in both cases — the status alone is
    /// knowledge the reply does not have.
    #[tokio::test]
    async fn a_reply_carrying_unknown_never_erases_a_known_status() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let alive = rest_harness::session("sess-1", 500);
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            loop {
                let Ok(Some(frame)) = reader.read_frame().await else {
                    return;
                };
                let Ok(ControlMsg::RestartSession { req_id, .. }) = parse_control(&frame) else {
                    return;
                };
                // Exactly what a real supervisor sends: a fresh offer and
                // a deliberately unknown status (`publish_relaunched`).
                writer
                    .write_control(&ControlMsg::SessionRestarted {
                        req_id,
                        session: farhelm_proto::SessionInfo {
                            status: farhelm_proto::SessionStatus::Unknown,
                            restart_offer: farhelm_proto::RestartOffer::FreshOnly,
                            title: "restarted".to_string(),
                            ..rest_harness::session("sess-1", 500)
                        },
                    })
                    .await
                    .unwrap();
            }
        });

        // The cached row is ALIVE, and the harness refreshes once an hour —
        // so nothing but this restart can change what the list says.
        let harness = rest_harness::spliced_helm_listing(client_side, vec![alive]).await;
        let (_, before) = get_json(&harness, "/api/sessions").await;
        assert_eq!(before["sessions"][0]["status"]["state"], "running");

        let (status, body) = post_text(
            &harness,
            "/api/sessions/sess-1/restart",
            serde_json::json!({ "mode": "fresh" }),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK, "{body}");

        let (_, after) = get_json(&harness, "/api/sessions").await;
        assert_eq!(
            after["sessions"][0]["status"]["state"], "running",
            "a reply that says 'not yet known' must not replace knowledge with its absence: \
             {after}"
        );
        assert_eq!(
            after["sessions"][0]["title"], "restarted",
            "every other field of the reply is authoritative and lands at once"
        );
        assert_eq!(
            after["sessions"][0]["restart_offer"], "fresh_only",
            "including the freshly recomputed offer the restart exists to produce"
        );
        peer.abort();
    }

    /// A mutation whose reply could not improve the cached status must
    /// WAKE the owning host's refresh, so the definite answer arrives in one
    /// round trip rather than one refresh interval.
    ///
    /// This is the other half of the no-degrade rule, and without it that
    /// rule pays for its own correctness with a visible lag. Restarting an
    /// EXITED session is the case that shows it: the reply says `Unknown`
    /// (deliberately — the pane exists, the agent's exec has not been
    /// observed), the merge declines to record it over the cached `exited`,
    /// and the list therefore goes on saying `exited` after a restart that
    /// succeeded. A user watching that sees their own successful action
    /// look like a failed one, for as long as the cadence says — which is
    /// exactly what the browser suite caught, on one engine and not the
    /// other, because a one-shot assertion races the interval.
    ///
    /// The harness refreshes once an HOUR and this test never advances the
    /// clock, so the transition asserted below cannot have come from the
    /// ordinary cadence: only the wake can have produced it. The woken drain
    /// must also be a POST-seed one — it samples the seed epoch when it
    /// starts, so a pre-seed snapshot would correctly decline to commit and
    /// leave the lag in place.
    #[tokio::test]
    async fn a_restart_that_cannot_improve_the_status_wakes_the_refresh() {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};

        let exited = farhelm_proto::SessionInfo {
            status: farhelm_proto::SessionStatus::Exited { exit_code: Some(1) },
            ..rest_harness::session("sess-1", 500)
        };
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            loop {
                let Ok(Some(frame)) = reader.read_frame().await else {
                    return;
                };
                let Ok(ControlMsg::RestartSession { req_id, .. }) = parse_control(&frame) else {
                    return;
                };
                // What a real supervisor sends: the relaunch happened, and
                // its liveness is not yet knowable.
                writer
                    .write_control(&ControlMsg::SessionRestarted {
                        req_id,
                        session: farhelm_proto::SessionInfo {
                            status: farhelm_proto::SessionStatus::Unknown,
                            ..rest_harness::session("sess-1", 500)
                        },
                    })
                    .await
                    .unwrap();
            }
        });

        let harness = rest_harness::spliced_helm_listing(client_side, vec![exited]).await;
        let (_, before) = get_json(&harness, "/api/sessions").await;
        assert_eq!(before["sessions"][0]["status"]["state"], "exited");

        // The host is ALIVE from here on — which the helm can only learn by
        // listing again.
        harness
            .fleet
            .edit(rest_harness::local_id(&harness.store).await, |script| {
                script.sessions = vec![rest_harness::session("sess-1", 500)];
            });

        let (status, body) = post_text(
            &harness,
            "/api/sessions/sess-1/restart",
            serde_json::json!({ "mode": "fresh" }),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK, "{body}");

        // Wait for the SECOND list request — the woken drain. Counting
        // requests rather than watching for a refresh state is what makes
        // this deterministic: the connect-time refresh already produced a
        // successful one-session result, so a state-shaped wait is
        // satisfied by the pre-restart pass and proves nothing. No clock is
        // advanced anywhere in this test, so a second request can only have
        // come from the wake — and the bound turns a missing one into a
        // failed test rather than a hung CI run.
        tokio::time::timeout(
            std::time::Duration::from_secs(30),
            harness.fleet.await_list_requests(2),
        )
        .await
        .expect("the write must wake a refresh; no second list request ever arrived");
        // The wait above resolves when the fake RECEIVES the second request;
        // the helm still has to process the reply and commit it, and a loaded
        // runner can stretch that gap past a one-shot assertion (seen twice
        // in full-workspace runs, never in isolation). Polling briefly does
        // not weaken the proof: the cadence is an hour of real time, so
        // within this window the woken drain is still the only thing that
        // can have produced the transition.
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        let after = loop {
            let (_, after) = get_json(&harness, "/api/sessions").await;
            if after["sessions"][0]["status"]["state"] == "running"
                || tokio::time::Instant::now() >= deadline
            {
                break after;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        };
        assert_eq!(
            after["sessions"][0]["status"]["state"], "running",
            "the definite status must arrive in one round trip, not one refresh interval: {after}"
        );
        peer.abort();
    }

    /// A stale session's DETAIL is served, not refused — the one
    /// `/api/sessions/{id}` route a down host does not turn away.
    ///
    /// SPEC.md: "opening such a session shows its metadata — title,
    /// directory, last-known status — behind a clear host-unreachable
    /// notice". Refusing here would leave the UI nothing to draw behind
    /// that notice, so the read is served from the cache and marked
    /// `stale`, while every mutating route on the same session still
    /// refuses (pinned above).
    #[tokio::test]
    async fn a_stale_sessions_detail_is_served_from_the_cache_and_marked_stale() {
        let (builder, host) = rest_harness::FleetBuilder::new()
            .await
            .ssh(
                "user@breaks",
                rest_harness::HostScript {
                    identity: Some("identity-original".to_string()),
                    sessions: vec![farhelm_proto::SessionInfo {
                        title: "the work in progress".to_string(),
                        cwd: "/home/user/project".to_string(),
                        ..rest_harness::session("owned", 100)
                    }],
                    ..rest_harness::HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        harness.await_refreshed(host).await;
        harness.fleet.take_down(host);
        harness
            .await_state(host, |state| state.phase() == "unreachable-reprobing")
            .await;

        let (status, value) = get_json(&harness, "/api/sessions/owned").await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(value["title"], "the work in progress");
        assert_eq!(value["cwd"], "/home/user/project");
        assert_eq!(value["host"], host);
        assert_eq!(
            value["stale"], true,
            "the metadata is last-known knowledge and must say so"
        );
    }
}
