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
//! Create, restart, rename, and archive all record their reply
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
/// (PLAN_M6.md item 5) and, as of PLAN_M6_75.md item 5, its filters.
///
/// Everything except the archive inclusion switch is absent-by-default.
/// A caller sending no query sees the ordinary, non-archived fleet view;
/// `include_archived=true` widens that view — rows AND `total` both — without
/// changing any of the search dimensions.
///
/// The filter parameters are SPEC.md's session-list dimensions: host,
/// parent, directory, profile, status, and title. Their match semantics live on
/// [`store::SessionFilter`], which is also where both the persisted and the
/// in-memory sources read them from, so there is one definition rather than
/// one per source. A parameter present but EMPTY is treated as absent
/// (`?title=` is what a cleared search box sends, and refusing it would make
/// clearing the box an error).
#[derive(Deserialize)]
pub(crate) struct ListQuery {
    /// An opaque resume key from a previous reply's `next_cursor`. Replay
    /// it verbatim; never construct or interpret one. An undecodable value
    /// is a 400 rather than a silent restart from the front, because a
    /// restart would re-serve a page the caller already had while looking
    /// exactly like progress.
    ///
    /// A cursor and a filter travel TOGETHER on every page of one walk, and
    /// this is ENFORCED rather than asked for: every cursor is bound to the
    /// filter it was minted under, and replaying one under a changed, added
    /// or cleared parameter is a 400 saying to start a fresh walk. The cursor
    /// names a position in the order, not in a result set, so a walk whose
    /// later pages moved the filter would resume mid-order through a
    /// different sequence and drop every earlier match without a trace.
    cursor: Option<String>,
    /// Maximum entries in this page.
    ///
    /// Bounded twice, and the second bound is newer than this field's
    /// original "a page is local data, so ask for what you like" reasoning:
    /// [`aggregate::MAX_PAGE_LIMIT`] caps every request, and a FILTERED
    /// request is capped lower again ([`aggregate::MAX_FILTERED_PAGE_LIMIT`]).
    /// The difference is real work rather than tidiness — an unfiltered page
    /// reads exactly the rows it returns, while a filtered one walks the
    /// order until it has filled the page, so a big limit paired with a
    /// selective filter is a request to scan the whole cache and decode
    /// every row of it.
    ///
    /// A limit of zero is refused too — it could never make progress through
    /// the pages.
    limit: Option<usize>,
    /// Include archived sessions.
    ///
    /// The one parameter that moves `total`: it selects which view is being
    /// served rather than narrowing one, so with it off the reply's rows and
    /// its total are both about the non-archived list, and with it on both
    /// are about the whole fleet (see [`aggregate::SessionPageBody::total`]).
    #[serde(default)]
    include_archived: bool,
    /// Only sessions on this registered host (a `HostView::id`).
    host: Option<store::HostId>,
    /// Only direct children of this session id.
    parent: Option<String>,
    /// Only sessions whose working directory CONTAINS this text, ignoring
    /// case.
    directory: Option<String>,
    /// Only sessions created from this profile, named either by its id or
    /// by the name they snapshotted at creation — which is what keeps a
    /// DELETED profile's sessions findable. See [`store::SessionFilter`].
    profile: Option<String>,
    /// Only sessions in this status, spelled exactly as the wire spells it
    /// (`running`, `waiting`, `idle`, `exited`, `error`, `interrupted`,
    /// `unknown`). An unrecognized word is a 400 rather than an empty list:
    /// a typo that answers "no sessions" is a lie the user will believe.
    status: Option<String>,
    /// Only sessions whose title CONTAINS this text, ignoring case.
    title: Option<String>,
}

/// Build the merged view's predicate from one request's query string, or
/// refuse it.
///
/// The one place the wire's spelling meets [`store::SessionFilter`].
///
/// The EXACTLY-EMPTY value is dropped rather than matched against, so a
/// cleared search box widens the list instead of narrowing it to sessions
/// whose title contains the empty string (which is all of them, but by
/// accident rather than by intent — and would count as "filtered" for the
/// two-totals reply).
///
/// Nothing else is dropped, and specifically not surrounding whitespace:
/// a directory or a title may legitimately contain it, and a session in
/// `/srv/my project/` or titled `fix  the  spacing` must stay findable by
/// typing what is actually there. Trimming would also make two different
/// searches — `" "` and `""` — into the same request, which is the one case
/// a user can see: typing a space would silently clear the filter. The cost
/// of not trimming is a search for `"drain "` that finds nothing, which the
/// user can see and fix.
fn list_filter(q: &ListQuery) -> anyhow::Result<store::SessionFilter> {
    let present = |value: &Option<String>| -> Option<String> {
        value
            .as_deref()
            .filter(|text| !text.is_empty())
            .map(str::to_string)
    };
    let mut filter = store::SessionFilter::default().include_archived(q.include_archived);
    if let Some(host) = q.host {
        filter = filter.host(host);
    }
    if let Some(parent) = present(&q.parent) {
        filter = filter.parent(&parent);
    }
    if let Some(directory) = present(&q.directory) {
        filter = filter.directory(&directory);
    }
    if let Some(profile) = present(&q.profile) {
        filter = filter.profile(&profile);
    }
    if let Some(title) = present(&q.title) {
        filter = filter.title(&title);
    }
    if let Some(status) = present(&q.status) {
        let known = store::parse_status_key(&status).ok_or_else(|| {
            anyhow::Error::new(SupervisorError {
                kind: ErrorKind::InvalidRequest,
                message: format!(
                    "{status:?} is not a session status; this helm knows running, waiting, idle, \
                     exited, error, interrupted, and unknown"
                ),
            })
        })?;
        filter = filter.status(known);
    }
    Ok(filter)
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
/// host fields added to each row and `next_cursor` and `matching` added
/// alongside, so the UI that predates multi-host keeps decoding it
/// unchanged. `matching` is present whenever a predicate is active. That
/// includes the ordinary request, whose implicit archive exclusion is a
/// real predicate even though its false value is omitted from the query
/// string. Only `include_archived=true` with no search dimensions is fully
/// unfiltered and makes no matching claim (see
/// [`aggregate::SessionPageBody::matching`] for why equating that claim with
/// `total` would have been a small lie). `total` now counts the merged view
/// rather than one supervisor's list — the view the request asked for,
/// archived rows included only under `include_archived=true` — and
/// `truncated` now means "there is a next page"
/// rather than "entries were held back" — see [`aggregate::SessionPageBody`]
/// for all of them, including why the filter's count is a SECOND number
/// beside `total` rather than a redefinition of it.
///
/// The filter parameters narrow the list
/// server-side, before the page cut, which is what makes "N matching of M"
/// coherent with a paged list at all: the alternative — a client filtering
/// the page it was handed — hides matches beyond the cut while reporting a
/// count that includes them.
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
    let filter = match list_filter(&q) {
        Ok(filter) => filter,
        Err(e) => return http_error(e),
    };
    // The filtered cap, checked after the filter is known rather than
    // guessed at from the query string: a filtered page does not read the
    // rows it returns, it reads the order until it has filled itself, so the
    // limit multiplies a scan rather than a slice. Refused rather than
    // clamped, like the cap above and for the same reason — a caller that
    // asked for 5,000 and got 500 with no way to tell has been answered
    // dishonestly.
    if !filter.is_empty() && limit > aggregate::MAX_FILTERED_PAGE_LIMIT {
        return http_error(anyhow::Error::new(SupervisorError {
            kind: ErrorKind::InvalidRequest,
            message: format!(
                "a filtered session list may ask for at most {} entries per page (an unfiltered \
                 one may ask for {}); a filtered page scans the order rather than slicing it, so \
                 the limit costs a walk rather than a copy",
                aggregate::MAX_FILTERED_PAGE_LIMIT,
                aggregate::MAX_PAGE_LIMIT
            ),
        }));
    }
    match aggregate::session_page(
        &state.manager,
        &state.store,
        &state.counts,
        q.cursor.as_deref(),
        limit,
        &filter,
    )
    .await
    {
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
    /// The agent command line, in RAW mode. Absent selects PROFILE mode,
    /// where `profile_id` supplies it (PLAN_M6_75.md item 3's two mutually
    /// exclusive creation modes, as they reach this API).
    ///
    /// Optional only in the type: exactly one of `invocation` and
    /// `profile_id` must be present, and a body naming both or neither is a
    /// 400 (see [`create_mode`]). Kept as two fields rather than one tagged
    /// union because that is the wire's own shape, and translating between
    /// two spellings of the same choice would be one more place for them to
    /// disagree.
    invocation: Option<String>,
    /// The profile to create from, in PROFILE mode — a `Profile::id` from
    /// this host's own catalog (`GET /api/hosts/{id}/profiles`), since
    /// profiles are per-supervisor and an id means nothing on another host.
    ///
    /// A successful profile-backed create is also what UPDATES this host's
    /// remembered default (see [`create_session`]): "last used" means a
    /// session was actually created from it, not that a picker was opened
    /// on it.
    profile_id: Option<String>,
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
    /// Which CONNECTION the caller prepared this create against — a
    /// `HostView::incarnation` read from `GET /api/hosts`.
    ///
    /// Optional, and absent means no claim is made (see
    /// [`crate::precondition`], which carries the whole reasoning). Present,
    /// and the create is refused with a 409 unless the host is still on that
    /// connection when routing resolves it.
    ///
    /// It matters most in PROFILE mode, and that is worth stating rather than
    /// leaving to be inferred: a profile id is minted per supervisor and every
    /// fresh supervisor seeds the same starter profiles, so a create aimed at
    /// `starter-claude` and landing on a host that was retargeted or adopted
    /// mid-flight does not fail — it RESOLVES over there, launching a profile
    /// the user never chose, and then records it as their remembered default.
    /// Raw-mode creates accept the field too: an invocation is not
    /// install-specific in the same way, but "run this on THAT machine" is
    /// still a thing a caller can mean and be wrong about.
    expected_incarnation: Option<u64>,
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
///
/// Visible to the crate because profile CRUD (`crate::profiles`) routes the
/// same way and for the same reasons: a profile lives on ONE supervisor, so
/// every profile request is a request to a named host's live connection, and
/// a second lookup path for it would be a second place to get the
/// state-and-client pairing wrong.
pub(crate) fn host_client(
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
///
/// ## Profile mode, and the remembered default
///
/// A body naming `profile_id` instead of `invocation` creates from the
/// target host's own profile catalog (PLAN_M6_75.md item 3's second creation
/// mode). Two consequences live here rather than on the supervisor:
///
/// - The helm REMEMBERS the profile as that host's last-used one, in
///   helm.db, but only after the create SUCCEEDS. A create that failed its
///   preconditions did not establish a preference — remembering an
///   attempted profile would make a typo the default the next dialog
///   suggests.
/// - The write is best-effort and never turns a successful create into a
///   failure. The session exists; reporting otherwise is the one outcome
///   SPEC.md's creation contract rules out, and a lost preference costs the
///   user one extra click.
///
/// A profile that no longer exists fails the create visibly, with no session
/// anywhere, and this handler does nothing to soften that: SPEC.md's rule is
/// to ask rather than guess, and a fallback to some other profile here would
/// be exactly the guess it forbids.
///
/// ## Naming the install this create was written for
///
/// An optional `expected_incarnation` says which connection the caller
/// prepared this body against, and the create is refused (409, with
/// `crate::precondition`'s marker) unless the host is still on it. Absent
/// means no claim, which is every pre-existing caller.
///
/// It exists for profile mode above all, and for a reason that makes the
/// ordinary reading of "stale request" wrong here: profile ids are minted per
/// supervisor and every fresh supervisor seeds the same starter profiles, so a
/// create aimed at a profile the user picked, landing on a host that was
/// retargeted or adopted in another tab, does not fail. It RESOLVES on the
/// successor, launches a profile nobody chose, and records that as the
/// remembered default. See [`crate::precondition`].
pub(crate) async fn create_session(
    State(state): State<Arc<AppState>>,
    axum::Json(mut req): axum::Json<CreateReq>,
) -> impl IntoResponse {
    let mode = match create_mode(&mut req) {
        Ok(mode) => mode,
        Err(e) => return http_error(e),
    };
    let (claim, client) = match create_target(&state, req.host) {
        Ok(target) => target,
        Err(e) => return http_error(e),
    };
    // Checked HERE, and once, because routing and claim-taking are one read
    // for a create: `create_target` resolves the host, takes the connection,
    // and mints the claim from the same borrow of the actor's status, so there
    // is no interval between "which install" and "which connection" for
    // anything to land in. What carries the check forward past the forward
    // itself is the claim: every write this create goes on to make — the cache
    // seed, the remembered default — revalidates it under the host's own
    // write lock, so a connection replaced while the supervisor was answering
    // records nothing. See `crate::precondition` on how the two compose.
    if let Err(e) = crate::precondition::incarnation_holds(&claim, req.expected_incarnation) {
        return http_error(e);
    }
    let created = match &mode {
        CreateMode::Raw(invocation) => {
            client
                .create_session_with_extras(
                    &req.cwd,
                    invocation,
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
        }
        CreateMode::Profile(profile_id) => {
            client
                .create_session_from_profile(
                    &req.cwd,
                    profile_id,
                    req.title,
                    req.cols,
                    req.rows,
                    req.intent_key,
                )
                .await
        }
    };
    match created {
        Ok(session) => {
            record_session(&state, &claim, &session).await;
            if let CreateMode::Profile(profile_id) = &mode {
                remember_default_profile(&state, &claim, profile_id, &session).await;
            }
            axum::Json(session).into_response()
        }
        Err(e) => http_error(e),
    }
}

/// Which of the two creation modes a request body selected — the choice
/// PLAN_M6_75.md item 3 made mutually exclusive on the wire, resolved once
/// at the REST edge.
///
/// Owned rather than borrowed from the body, because the mode outlives the
/// request that produced it: it decides which call to make, and is consulted
/// AGAIN after the reply lands (only a PROFILE create writes a remembered
/// default), by which point the body's other fields have been moved into the
/// call. Taken out of the body rather than cloned — nothing else reads them
/// afterwards.
enum CreateMode {
    Raw(String),
    Profile(String),
}

/// Resolve a create body's mode, refusing the two ambiguous shapes.
///
/// Both refusals are `InvalidRequest` — a 400 — and both are worth making
/// loudly rather than picking a winner. A body naming BOTH has no honest
/// reading (does the profile's invocation win, or the caller's?), which is
/// the same reasoning that made the two mutually exclusive on the wire; a
/// body naming NEITHER says nothing about what to run at all. Silently
/// preferring one, or defaulting to some shell, would launch something the
/// caller never asked for.
/// The snapshot overrides are RAW-MODE ONLY, and a profile-mode body
/// carrying either is refused rather than quietly served: a profile already
/// states its kind and its resume template, the wire refuses a request that
/// names both, and this API's shape makes it easy to send both by accident.
/// Discarding them silently — which is what forwarding a profile create and
/// dropping the fields amounts to — would launch a session under settings
/// the caller believes it chose. The refusal names the fields so the caller
/// knows which half to remove.
fn create_mode(req: &mut CreateReq) -> anyhow::Result<CreateMode> {
    match (req.invocation.take(), req.profile_id.take()) {
        (Some(_), Some(_)) => Err(anyhow::Error::new(SupervisorError {
            kind: ErrorKind::InvalidRequest,
            message: "a create names either an invocation or a profile, never both: a profile \
                      already says what to run, and there is no honest way to merge the two"
                .to_string(),
        })),
        (None, None) => Err(anyhow::Error::new(SupervisorError {
            kind: ErrorKind::InvalidRequest,
            message: "a create must name either an invocation or a profile; this body names \
                      neither, so there is nothing to launch"
                .to_string(),
        })),
        (Some(invocation), None) => Ok(CreateMode::Raw(invocation)),
        (None, Some(_)) if req.agent_kind.is_some() || req.resume_template.is_some() => {
            Err(anyhow::Error::new(SupervisorError {
                kind: ErrorKind::InvalidRequest,
                message: "a profile-backed create cannot also send agent_kind or \
                          resume_template: the profile states both, and the wire refuses a \
                          request that names a profile alongside either override — edit the \
                          profile, or create from a raw invocation instead"
                    .to_string(),
            }))
        }
        (None, Some(profile_id)) => Ok(CreateMode::Profile(profile_id)),
    }
}

/// Record `profile_id` as this host's last-used profile, and invalidate.
///
/// Routed through the MANAGER rather than straight to the store, for
/// `record_session`'s reasons plus one that is specific to profiles: an id
/// only means anything on the supervisor that minted it, and every fresh
/// supervisor seeds the same STARTER profiles, so ids genuinely collide
/// across installs. A write that landed after the host was retargeted or
/// adopted away could therefore record an id that resolves on the new
/// install to a profile the user never picked — and the create dialog would
/// then offer it as their own last choice. The claim is what makes that
/// unconstructible; see [`manager::ConnectionManager::remember_profile_default`].
///
/// Best effort, on the same terms as [`record_session`]: the session has
/// been created and the caller is about to be told so, and a preference that
/// failed to persist costs one extra click at the next create dialog — where
/// reporting a successful create as a failure would cost a session the user
/// then has to find and clean up by hand.
///
/// Bumps the fleet's revision when the stored ROW actually CHANGED, which is
/// what makes a create-dialog default arrive in a second client without
/// polling. Creating from the same profile twice in a row changes nothing and
/// wakes nobody.
///
/// "The row" rather than "the profile id", and the distinction has teeth: a
/// create that rewrites the same id against an identity the stored row does
/// not carry REPAIRS a binding that had made the default unreadable, so the
/// default goes from absent back to present without the id ever moving. That
/// is a change other clients must be told about, and the store is what decides
/// it (`crate::store::HelmStore::remember_profile_default`).
async fn remember_default_profile(
    state: &AppState,
    claim: &manager::SessionClaim,
    profile_id: &str,
    session: &farhelm_proto::SessionInfo,
) {
    match state
        .manager
        .remember_profile_default_from_session(claim, profile_id, session)
        .await
    {
        Ok(true) => state.manager.events().bump(),
        Ok(false) => {}
        Err(error) => warn!(
            host = claim.host,
            profile_id = manager::peer_text(profile_id).as_str(),
            error = %error,
            "the session was created but its profile could not be remembered as this host's \
             default; the next create dialog will suggest the previous one"
        ),
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
    // The registry-identity join, same terms as the merged listing's (see
    // `aggregate::row_of`): helm.db is the identity authority, and the
    // snapshot cannot carry a fresh copy. Read BEFORE the owner/status
    // capture below for the same ordering reason the listing documents at
    // its identity join: a retarget straddling the two reads must produce
    // a stale identity on fresh session content (mismatch → the create
    // default falls back locally) rather than a fresh identity on stale
    // content (a false match onto the wrong machine). The whole registry
    // is read because the owner is not known yet — one indexed lookup
    // against a list this small is cheaper than being wrong.
    let identities: Vec<store::HostRow> = match state.store.list_hosts().await {
        Ok(rows) => rows,
        Err(e) => return http_error(e),
    };
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
    let host_identity = identities
        .into_iter()
        .find(|row| row.id == host)
        .and_then(|row| row.host_identity);

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
                host_identity,
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
                host_identity,
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
/// item 4).
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

/// `POST /api/sessions/{id}/archive` — stop the session's agent and tabs,
/// remove its terminal, and retain its metadata and attachments.
///
/// The supervisor returns the durable post-teardown state, including for an
/// idempotent retry. Recording that exact reply before answering makes the
/// default list hide the row immediately and publishes the ordinary
/// changed-only fleet event. Owner routing happens first, so an archive on
/// an unreachable host is refused with that host's state rather than
/// pretending the retained session is missing.
pub(crate) async fn archive_session(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
) -> impl IntoResponse {
    let (claim, client) = match route_session(&state, &id).await {
        Ok(routed) => routed,
        Err(e) => return http_error(e),
    };
    match client.archive_session(&id).await {
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

// Tests occupy three quarters of this module, so they live in a sibling file. `#[path]`
// keeps them under `sessions` with private-item access and no visibility changes.
#[cfg(test)]
#[path = "sessions_tests.rs"]
mod tests;
