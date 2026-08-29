//! Named per-message handlers for `handle_control`'s dispatch.
//!
//! One `async fn handle_<message>` per substantial `ControlMsg` variant
//! (M4.5's structural pass — functional no-op, see PLAN.md's milestone
//! ladder); `handle_control` itself is just the dispatch match. Each
//! handler owns exactly the connection-local state its message can
//! mutate and otherwise leans on `Supervisor` methods (`service::core`)
//! and the other `service` submodules for everything else.
//!
//! `handle_control` receives that connection-local state as one
//! [`ConnectionCtx`] and hands each arm only the fields its message may
//! touch — the bundle stops here deliberately, so a handler's signature
//! still says what it is allowed to reach. The heavier arms delegate
//! their substance too: `ListSessions` to `listing::list_page` and
//! `DeleteSession` to `Supervisor::teardown_session`, leaving the handler
//! with validation on the way in and reply shaping on the way out.

use super::connection::{
    ConnectionCtx, Forwarder, notify_detached, reply_frame, send_reply, set_attachment_paused,
    spawn_admitted,
};
use super::core::{
    CreateInputs, CreateMode, RequestError, SessionEntry, Supervisor, create_fingerprint,
    ensure_title_printable, error_kind, truncate_for_error, unknown_pane_owner_refusal,
};
use super::launch_artifacts::{
    cleanup_launch_artifacts, read_launch_sentinel, sentinel_could_still_apply,
    wrapper_failure_detail,
};
use super::listing::{
    LIST_BYTE_BUDGET, LIST_SESSION_CAP, ListQuery, build_list_reply, decode_list_cursor, list_page,
};
use super::snapshots::{capture_alt_screen_before_stop, publish_alt_screen_snapshot};
use super::status::{dead_pane_exit_code, entry_info, observe_entry};
use super::sweep::{StopFailure, SweepTarget, reap_process_tree, stop_live_agent};
use super::teardown::{ArchiveError, TeardownError};
use super::terminals::{
    ActiveAttach, AttachmentKey, DETACH_REASON_REPLACED, DETACH_REASON_TAKEOVER, InputRoute,
    MAX_LEASE_BYTES, Terminal, TerminalId, displaced_by_attach, resolve_terminal,
};
use super::uploads::{
    MAX_UPLOADS_PER_CONNECTION, UPLOAD_CHUNK_QUEUE, UPLOAD_SIGNAL_QUEUE, UploadCommand,
    UploadHandle, UploadOutcome, UploadRequest, UploadRoute, UploadSignal, commit_without_upload,
    run_upload,
};
use crate::store::DedupScope;
use crate::store::{
    IntentClaim, LastOutcome, ProfileCreation, ProfileNames, Transition, validate_profile_fields,
};
use crate::tmux::PaneProbe;

/// Authority-derived create policy.
///
/// One value controls both selector defaulting and reservation lifetime, so
/// a caller cannot accidentally combine interactive derivation with bounded
/// keys or spawn derivation with permanent tombstones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CreateAdmission {
    Interactive,
    Spawn,
}

impl CreateAdmission {
    fn dedup_scope(self) -> DedupScope {
        match self {
            CreateAdmission::Interactive => DedupScope::Permanent,
            CreateAdmission::Spawn => DedupScope::SessionLifetime,
        }
    }
}
use anyhow::Context;
use farhelm_proto::{
    AgentKind, ControlMsg, ErrorKind, Frame, MAX_PROFILES_PER_HOST, RestartMode, SessionInfo,
    TerminalSelector,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio::sync::{mpsc, oneshot, watch};
use tracing::{debug, warn};

/// Combined byte cap on every caller-supplied field a `CreateSession` can
/// persist or return, enforced before `create_session` does anything.
///
/// Without this, a request whose fields nearly fill `MAX_FRAME_LEN` can
/// succeed at creating the session, and only then discover that its
/// `SessionCreated` reply — the same fields again, plus the generated id
/// and the frame wrapper — exceeds the cap and gets degraded to an
/// `Error` by `reply_frame`. That leaves the session alive while the
/// caller is told the request failed, with no way to learn the id needed
/// to attach to (or tear down) the very session it just created. 64 KiB
/// is orders of magnitude beyond any real parent, cwd, launch selector,
/// title, or resume template. Capping the inputs this far below the frame
/// limit makes an oversized `SessionCreated` reply structurally
/// impossible and bounds the permanent fingerprint copy, before
/// `create_session` has touched storage, tmux, or the filesystem.
pub(crate) const CREATE_FIELD_CAP: usize = 64 * 1024;

/// Byte cap on `CreateSession`'s `intent_key` (PLAN_M3.md item 6),
/// enforced alongside `CREATE_FIELD_CAP` before any lookup or write.
///
/// Separate from that cap rather than folded into it, because what the two
/// protect is different in kind: the field cap bounds a REPLY that would
/// otherwise be undeliverable, while this one bounds a durable, deliberately
/// un-pruned table (see `store::Reservation`'s tombstone docs) whose primary
/// key is whatever the client sent. Without a bound, one client can spend
/// unbounded disk on keys nothing will ever replay. 512 bytes is two orders
/// of magnitude beyond a UUID — the shape the UI actually sends — while
/// still leaving room for a caller that prefers structured keys.
const INTENT_KEY_CAP: usize = 512;

/// Byte cap on `ControlMsg::ReportConversation`'s `conversation`, enforced
/// alongside the plausibility check and before any WRITE.
///
/// Not before every lookup: the credential check runs first and reads the
/// session row, because answering an unauthenticated peer's malformed
/// request with anything more specific than `Unauthorized` would tell it
/// which half it got wrong. What this cap precedes is the store WRITE and
/// the log line, which are the two places an unbounded field would cost
/// something.
///
/// The handler's own bound rather than an inherited one, for the reason
/// `MAX_LEASE_BYTES` is: a post-handshake control frame is bounded only by
/// `MAX_FRAME_LEN` (megabytes), and the hello-only caps in
/// `farhelm_proto::io` stop applying the moment the connection is
/// established. The reported id is stored in a column, logged, and
/// eventually placed on an agent's command line, so it wants a bound at
/// the doorway even though `agent_kind::is_plausible_conversation_id`
/// happens to enforce the same number today. Keeping them separate is what
/// stops a future relaxation of the record parser — whose input is a file
/// this process at least chose to open — from silently widening what an
/// in-session peer can push over the wire. Both vendors use UUIDs (36
/// bytes), so 128 is generous headroom either way.
const MAX_CONVERSATION_BYTES: usize = 128;

/// Byte cap on `ControlMsg::ReportConversation`'s `source` — the vendor's
/// own word for why the hook fired (`startup`, `resume`, `clear`,
/// `compact`, ...).
///
/// Unlike the conversation id, this field is never stored and never reaches
/// an argv; it only ever appears in log lines. That is exactly why it needs
/// a bound of its own rather than riding on the id's. A `source` is not
/// validated for shape — a vendor may add an event name at any time, and
/// refusing an unrecognized one would throw away a perfectly good report
/// over a diagnostic string — so the field is whatever the peer sends, and
/// the peer is any process inside the agent's tree holding the session
/// credential. Without a cap, one such process turns the supervisor log
/// into an unbounded write target.
///
/// Generous next to every real value (the longest vendor event is a
/// handful of characters) and small enough that a log line stays readable.
const MAX_SOURCE_BYTES: usize = 64;

/// `source` reduced to something safe to put in a log line: at most
/// [`MAX_SOURCE_BYTES`] bytes, with every control character replaced.
///
/// Sanitizing rather than refusing, deliberately. The report itself is the
/// valuable thing and the `source` is a diagnostic beside it; rejecting a
/// report because its event name was odd would trade a correct resume for a
/// tidy log. So an over-long or control-laced value is trimmed and passed
/// on, and the report is judged on its identity alone.
///
/// Control characters are what make this more than a length cap. The log is
/// line-oriented and read by humans and by whatever tails it; a newline in
/// this field lets a session-held credential forge log ENTRIES, and a
/// terminal escape lets it repaint the operator's screen. Replacement keeps
/// the value legible while making both impossible.
///
/// Truncation is on a CHARACTER boundary rather than a byte one — slicing a
/// `String` mid-UTF-8 would panic, and this input is attacker-chosen.
fn sanitized_source(source: &str) -> String {
    source
        .chars()
        .map(|c| if c.is_control() { '\u{fffd}' } else { c })
        .scan(0usize, |used, c| {
            *used += c.len_utf8();
            (*used <= MAX_SOURCE_BYTES).then_some(c)
        })
        .collect()
}

/// Cap on how many argv elements `CreateSession`'s `resume_template`
/// override may carry (PLAN_M3.md items 6 and 7).
///
/// Re-exported from `store` rather than declared here, because the same
/// bound applies to a profile's template and profile validation had to move
/// below the handler to be reachable from the store's own writes. One
/// constant is what keeps a create override and a profile definition from
/// growing different limits for the same field; see the definition for what
/// the number is and why.
use crate::store::RESUME_TEMPLATE_ELEMENT_CAP;

/// Decide which launch selector a `CreateSession` chose, or say why the
/// request has no single meaning (PLAN_M7.md item 2).
///
/// The three selectors are mutually exclusive on the wire by CONTRACT
/// rather than by construction (see `ControlMsg::CreateSession`'s own
/// docs), so this is where that contract is enforced — once, before
/// anything reads a mode's fields, so that no half-interpreted request can
/// reach a launch.
///
/// Every ambiguous shape is refused as `ErrorKind::InvalidRequest`. A
/// profile selection ALONGSIDE a
/// raw-mode override (`agent_kind` or `resume_template`) is just as
/// ambiguous as naming both an invocation and a profile. The profile
/// already states its kind and its resume template, and there is no honest
/// precedence rule to invent — a request that says "use this profile, but
/// actually a different kind" has not decided what it wants, and the
/// session's own durable snapshot would then no longer describe the profile
/// it claims to have come from.
///
/// The `Err` is the user-facing message verbatim (SPEC.md's concrete,
/// actionable errors), so each one names both what was sent and what would
/// have been acceptable.
///
/// The overrides are MOVED into the raw variant rather than passed onward
/// beside the mode: past this function they are meaningful only for a raw
/// create, and [`CreateMode`] is where that stops being a convention.
fn create_mode(
    invocation: Option<String>,
    profile_id: Option<String>,
    profile_name: Option<String>,
    agent_kind: Option<AgentKind>,
    resume_template: Option<Vec<String>>,
) -> Result<CreateMode, String> {
    match (invocation, profile_id, profile_name) {
        (None, None, None) => Err(
            "a create must name an invocation, profile id, or profile name; this request named \
             none"
                .to_string(),
        ),
        (Some(invocation), None, None) => Ok(CreateMode::Raw {
            invocation,
            agent_kind,
            resume_template,
        }),
        (None, profile_id, profile_name) if profile_id.is_some() != profile_name.is_some() => {
            if agent_kind.is_some() || resume_template.is_some() {
                return Err(
                    "a profile-backed create cannot also carry agent_kind or resume_template \
                     overrides; the profile supplies both"
                        .to_string(),
                );
            }
            match (profile_id, profile_name) {
                (Some(profile_id), None) => Ok(CreateMode::Profile { profile_id }),
                (None, Some(profile_name)) => Ok(CreateMode::ProfileName { profile_name }),
                _ => unreachable!("the match guard requires exactly one profile selector"),
            }
        }
        _ => Err(
            "a create names exactly one of invocation, profile id, or profile name; this \
             request named more than one"
                .to_string(),
        ),
    }
}

/// Resolves the creation mode, validates the caller-supplied fields against
/// the reply-size and idempotency-store caps, then hands off to
/// [`Supervisor::create_session`].
///
/// The refusal ORDER is shape, then size and key bounds. Neither claims a
/// reservation: malformed or oversized requests must remain correctable
/// under the same key. That is deliberately NOT true of launch preconditions past this
/// point (a working directory that does not exist, a profile id that no
/// longer does): those are durable outcomes replayed under the key, which
/// is the contract `Supervisor::create_session` states in full.
#[allow(clippy::too_many_arguments)]
async fn handle_create_session(
    sup: &Arc<Supervisor>,
    tx: &mpsc::Sender<Frame>,
    req_id: u64,
    parent: Option<String>,
    cwd: String,
    invocation: Option<String>,
    profile_id: Option<String>,
    profile_name: Option<String>,
    title: Option<String>,
    cols: u16,
    rows: u16,
    intent_key: Option<String>,
    admission: CreateAdmission,
    // Two consumers, and they must see the SAME values: item 6's
    // fingerprint (a retry differing only in an override is a
    // different request and is refused as a key reuse) and item
    // 7's snapshot resolution, which is what makes the overrides
    // shape the session itself.
    agent_kind: Option<AgentKind>,
    resume_template: Option<Vec<String>>,
) {
    let selectorless_spawn = admission == CreateAdmission::Spawn
        && invocation.is_none()
        && profile_id.is_none()
        && profile_name.is_none()
        && agent_kind.is_none()
        && resume_template.is_none();
    let mode = match if selectorless_spawn {
        Ok(CreateMode::DerivedProfile)
    } else {
        create_mode(
            invocation,
            profile_id,
            profile_name,
            agent_kind,
            resume_template,
        )
    } {
        Ok(mode) => mode,
        Err(message) => {
            send_reply(
                tx,
                &ControlMsg::Error {
                    req_id,
                    message,
                    kind: ErrorKind::InvalidRequest,
                },
            )
            .await;
            return;
        }
    };
    // One accounting for every caller-supplied field the supervisor can
    // copy into a durable fingerprint. Interactive rows are permanent, so
    // omitting parent, a profile selector, or a raw override would leave an
    // unbounded write path through a cap the other modes cannot dodge.
    let (mode_bytes, template_elements) = match &mode {
        CreateMode::Raw {
            invocation,
            resume_template,
            ..
        } => (
            invocation.len()
                + resume_template
                    .iter()
                    .flatten()
                    .map(|element| element.len())
                    .sum::<usize>(),
            resume_template.as_ref().map_or(0, Vec::len),
        ),
        CreateMode::Profile { profile_id } => (profile_id.len(), 0),
        CreateMode::ProfileName { profile_name } => (profile_name.len(), 0),
        CreateMode::DerivedProfile => (0, 0),
    };
    let field_len = parent.as_deref().map_or(0, str::len)
        + cwd.len()
        + mode_bytes
        + title.as_deref().map_or(0, str::len);
    let refusal = if field_len > CREATE_FIELD_CAP {
        Some(format!(
            "parent, cwd, invocation or profile, title, and resume template together are \
             {field_len} bytes, exceeding the {CREATE_FIELD_CAP}-byte limit"
        ))
    } else if template_elements > RESUME_TEMPLATE_ELEMENT_CAP {
        // Bounded separately from the byte total because the two
        // are independent: a template of ten thousand EMPTY
        // elements costs almost no bytes and is still nothing a
        // resume invocation could legitimately be.
        Some(format!(
            "resume template has {template_elements} elements, exceeding the \
             {RESUME_TEMPLATE_ELEMENT_CAP}-element limit"
        ))
    } else {
        // Both refusals below are about a key that could never do
        // its job: an empty one would collapse every create that
        // forgot to set it into a single intent, and an unbounded
        // one buys durable, un-pruned table space with a request
        // (see `INTENT_KEY_CAP`). Checked before the lookup so
        // neither ever reaches the store.
        match intent_key.as_deref() {
            Some("") => Some("intent key must not be empty".to_string()),
            Some(key) if key.len() > INTENT_KEY_CAP => Some(format!(
                "intent key is {} bytes, exceeding the {INTENT_KEY_CAP}-byte limit",
                key.len()
            )),
            _ => None,
        }
    };
    if let Some(message) = refusal {
        send_reply(
            tx,
            &ControlMsg::Error {
                req_id,
                message,
                kind: ErrorKind::InvalidRequest,
            },
        )
        .await;
        return;
    }
    // The resolved mode travels onward unchanged: the fingerprint binds
    // whichever selector the request chose (so a retried key cannot flip
    // selectors or name a DIFFERENT profile), and `create_session` resolves
    // profile ids during validation, where the unknown-profile precondition can fail the
    // create with no session and — like every other precondition — be
    // recorded against the intent key so a retry replays the same answer.
    //
    // Nothing about a profile id is resolved HERE, deliberately: doing it
    // before the reservation lookup would run a catalog read for a replay
    // that is only going to return the original attempt's answer, and would
    // put a second (and differently-ordered) precondition check on the
    // create path.
    let idempotency = intent_key.map(|intent_key| IntentClaim {
        intent_key,
        fingerprint: create_fingerprint(parent.as_deref(), &cwd, &mode, title.as_deref()),
        dedup_scope: admission.dedup_scope(),
    });
    match sup
        .create_session(
            CreateInputs {
                cwd: &cwd,
                parent,
                mode,
                title,
                cols,
                rows,
            },
            idempotency,
        )
        .await
    {
        Ok(session) => {
            send_reply(tx, &ControlMsg::SessionCreated { req_id, session }).await;
        }
        Err(e) => {
            send_reply(
                tx,
                &ControlMsg::Error {
                    req_id,
                    message: format!("{e:#}"),
                    kind: error_kind(&e),
                },
            )
            .await;
        }
    }
}

/// One session's `SessionInfo` for a single-session reply, observed
/// and recorded exactly as a `ListSessions` pass would (PLAN_M5.md
/// item 3).
///
/// The alternative — reading the entry's stored fields and probing
/// only liveness — was the first shape of this and is wrong in a way
/// that is easy to miss: a session whose agent never execed lists as
/// **error** through the sentinel path alone, so a reply that skipped
/// it would report `Exited` for a session every list reply calls
/// `Error`, and a rename issued moments after a conversation became
/// capturable would report `FreshOnly` where the list says `Resume`.
/// `SessionRenamed`'s protocol contract is that its `SessionInfo` is
/// built the way `ListSessions` builds one; this is that promise
/// being kept rather than approximated.
///
/// The capture pass runs first for the same reason the list runs it
/// there: an identity claimed on this very pass belongs in the
/// `restart_offer` this reply carries, not only in the next poll's.
/// Like the list path it is a `CaptureReason::Reply` pass — so it can
/// never be answered by a sweep older than this request — and cheap in the
/// steady state (see `Supervisor::capture_pass_for`).
///
/// The tmux round trip is skipped for a terminal-less entry (the
/// restart gap): its status comes entirely from its recorded outcome
/// and it has no tmux session to hold tabs, so asking would be
/// pointless and would let an unrelated tmux failure break a reply
/// that needs nothing from it.
///
/// Every failure here is the caller's to report, including the durable
/// write's — see the transition below for why this is stricter than the
/// list path it otherwise mirrors.
async fn session_info_now(
    sup: &Arc<Supervisor>,
    entry: &Arc<SessionEntry>,
) -> anyhow::Result<SessionInfo> {
    sup.capture_now().await;
    let pane_states = match entry.terminal {
        Some(_) => sup.tmux.pane_states().await?,
        None => HashMap::new(),
    };
    let observed = observe_entry(sup, entry, &pane_states).await?;
    if observed.settled_error {
        cleanup_launch_artifacts(&sup.state_dir, &entry.info.id, entry.generation).await;
    }
    if let Some(transition) = observed.transition {
        // One entry's transition through the same batching API the
        // list uses, so the store arbitrates it identically.
        //
        // A failed write PROPAGATES here, unlike on the list path where
        // it is logged and the reply goes out anyway. The difference is
        // what the two requests are: a list is a poll that retries in a
        // second, so serving it from what was observed costs nothing,
        // while this reply is the authoritative answer to a mutation and
        // its caller has no reason to poll again. Silently returning a
        // `SessionInfo` whose status this process could not record would
        // hand that caller a success built on a write that did not
        // happen; the handler above reports it as a failure that says
        // the rename itself landed.
        let committed = sup
            .store
            .transition_many(vec![(entry.info.id.clone(), entry.generation, transition)])
            .await
            .with_context(|| format!("recording session {}'s observed outcome", entry.info.id))?;
        let committed = committed.get(&entry.info.id);
        if let Some(outcome) = committed {
            *entry.outcome.lock().expect("outcome mutex poisoned") = outcome.clone();
        }
        // Both files are cosmetic once the durable outcome says what
        // happened, and a write that did NOT land must leave them for a
        // later pass to retry against — hence gating on what committed
        // rather than on the sentinel find alone.
        if observed.sentinel.is_some() && matches!(committed, Some(LastOutcome::Error { .. })) {
            cleanup_launch_artifacts(&sup.state_dir, &entry.info.id, entry.generation).await;
        }
    }
    // One session, so one catalog read — and only for a session that names
    // a profile at all. Failing the reply on a failed read rather than
    // passing an empty catalog, for `list_page`'s reason: an absent id is
    // how a DELETED profile reads, so an empty map would report this
    // session's profile as gone.
    let profiles = if entry.info.source_profile.is_some() {
        sup.store
            .profile_names()
            .await
            .context("reading the profile catalog to describe this session's source profile")?
    } else {
        ProfileNames::new()
    };
    Ok(entry_info(
        entry,
        &pane_states,
        observed.sentinel.as_deref(),
        &profiles,
    ))
}

/// Spawned onto its own task rather than awaited inline: this
/// handler is reached from `handle_connection`'s single serial read
/// loop, and `TmuxDriver::pane_states` is a real subprocess
/// round trip that can block for as long as tmux takes to
/// answer (a wedged or merely slow tmux, under load). Awaiting
/// it inline would stall every OTHER request on this
/// connection — attach, input, another session's list/stop/
/// delete — behind this one `ListSessions`. Spawning is safe:
/// this handler only reads `sup.sessions` under its own lock hold
/// and never touches `input_routes` (connection-local state,
/// never shared with a spawned task), the map-wide mutex
/// already tolerates concurrent requests interleaving (see the
/// `Supervisor` struct's lock-discipline docs), and replies are
/// correlated by `req_id` rather than by arrival or completion
/// order (already true of every request on this connection).
///
/// Tracked in `tasks` (a `JoinSet`) and admitted through
/// `spawn_admitted` rather than a bare `tokio::spawn`: see
/// `HANDLER_ADMISSION_PERMITS`/`HANDLER_SHUTDOWN_TIMEOUT`'s own
/// docs for why an unbounded, untracked spawn per slow request
/// is not safe to leave unmanaged.
///
/// ## Cursor/limit contract (PLAN_M6.md item 2, serving the item 1 vocabulary)
///
/// Real pagination, per `ControlMsg::ListSessions`/`SessionList`'s own
/// wire docs:
///
/// - `cursor: None` starts a walk at the front of `list_order_key`'s
///   order; `Some` resumes strictly after the key it decodes to,
///   regardless of whether a session bearing that exact key still exists
///   (a since-deleted session's cursor resumes cleanly). A cursor that
///   fails to decode — bad base64, malformed JSON, or valid JSON of the
///   wrong shape (`decode_list_cursor`'s own docs) — is refused with
///   `ErrorKind::InvalidRequest` before any list work is admitted: this
///   handler never guesses at a caller's intent for a value it cannot
///   honestly interpret, and never treats it as "start over" either
///   (which would silently re-serve sessions the caller's earlier pages
///   already returned).
/// - `limit: None` takes `LIST_SESSION_CAP` as the DEFAULT page size;
///   `Some(n)` for `n >= 1` is HONORED AS GIVEN, with no upper clamp —
///   PLAN_M6.md keeps `LIST_SESSION_CAP` alive only as that default, never
///   as a ceiling on an explicit request (an earlier build clamped `Some(n)`
///   above the cap down to it, which the plan never sanctioned; Theme C of
///   the M6.75 review-swarm batch removed that clamp). The byte budget
///   (`LIST_BYTE_BUDGET`, enforced in `build_list_reply`) remains the real
///   bound on what any one page can carry regardless of `n`: an
///   over-large `limit` simply degrades to a budget cut with a resume
///   cursor, same as any other page that does not fit whole.
///   `Some(0)` is refused outright, DELIBERATELY not clamped up to 1: a
///   page of zero can never make progress on its own, so a caller that
///   sent it almost certainly has a bug, and refusing surfaces that
///   immediately rather than silently substituting a value the caller
///   never asked for and may not expect. The alternative (clamp to 1) was
///   considered and rejected for exactly that silent-substitution reason;
///   either choice avoids the one truly forbidden shape — an EMPTY page
///   carrying a `next_cursor`, which would let a caller loop forever
///   making no progress while believing it was paging correctly.
/// - Every reply, cut or not, reflects the REAL state of the walk:
///   `next_cursor` is `Some` exactly when sessions remain beyond the last
///   entry actually returned (whether the count/cursor cut below left
///   them, or `build_list_reply`'s own byte-budget cut did — see that
///   function's docs for how the two compose), `None` only when the walk
///   this reply produced genuinely reached the end of the order. The one
///   exception is not a `next_cursor` shape at all: a single session too
///   large to fit under the byte budget even alone is refused outright
///   (`build_list_reply`'s own docs) rather than answered as a fake
///   exhausted page.
async fn handle_list_sessions(
    sup: &Arc<Supervisor>,
    tx: &mpsc::Sender<Frame>,
    tasks: &mut tokio::task::JoinSet<()>,
    req_id: u64,
    cursor: Option<String>,
    limit: Option<u32>,
) {
    // Limit and cursor are both validated BEFORE any list work is
    // admitted (mirroring where PLAN_M6.md item 1's interim refusal sat,
    // before item 2 replaced it with real pagination): neither check
    // touches `sup.sessions` or tmux, so there is no reason to pay
    // `spawn_admitted`'s cost for a request this handler is about to
    // refuse outright.
    let effective_limit = match limit {
        Some(0) => {
            send_reply(
                tx,
                &ControlMsg::Error {
                    req_id,
                    message: "ListSessions limit must be at least 1; a limit of 0 could never \
                              make progress through the pages"
                        .to_string(),
                    kind: ErrorKind::InvalidRequest,
                },
            )
            .await;
            return;
        }
        // No `.min(LIST_SESSION_CAP)` here (Theme C of the M6.75
        // review-swarm batch): PLAN_M6.md keeps the cap alive only as the
        // default page size, not as a ceiling on an explicit request. An
        // over-large `n` is bounded by the byte budget instead, same as
        // any other page that would not fit whole — see this function's
        // own cursor/limit contract docs above.
        Some(n) => n as usize,
        None => LIST_SESSION_CAP,
    };
    let cursor_key = match cursor.as_deref().map(decode_list_cursor) {
        Some(None) => {
            send_reply(
                tx,
                &ControlMsg::Error {
                    req_id,
                    message: "ListSessions cursor could not be decoded; cursors are opaque — \
                              replay one exactly as a reply carried it, or start a fresh \
                              walk with cursor: None"
                        .to_string(),
                    kind: ErrorKind::InvalidRequest,
                },
            )
            .await;
            return;
        }
        Some(Some(key)) => Some(key),
        None => None,
    };
    let sup2 = Arc::clone(sup);
    let tx = tx.clone();
    spawn_admitted(&sup.admission, tasks, async move {
        let sup = sup2;
        let page = match list_page(
            &sup,
            ListQuery {
                cursor: cursor_key,
                limit: effective_limit,
            },
        )
        .await
        {
            Ok(page) => page,
            // Every failure the walk can hit — an unreadable launch
            // sentinel, an unclassified tmux failure — is an `Internal`
            // carrying the original error verbatim: see `list_page`'s own
            // docs for why each of them fails the whole request rather
            // than degrading one entry.
            Err(e) => {
                send_reply(
                    &tx,
                    &ControlMsg::Error {
                        req_id,
                        message: format!("{e:#}"),
                        kind: ErrorKind::Internal,
                    },
                )
                .await;
                return;
            }
        };
        // `Err` is `build_list_reply`'s degenerate-budget case (Theme B of
        // the M6.75 review-swarm batch): the byte budget kept ZERO
        // entries while at least one candidate remained, so there is no
        // honest page to send — reporting `next_cursor: None` here would
        // lie that the walk was exhausted. Named by session id so the
        // caller (a human, in practice — no client is expected to react
        // programmatically to a page-shaped answer becoming impossible)
        // can tell which record is the problem.
        let reply = match build_list_reply(
            req_id,
            page.sessions,
            page.total,
            LIST_BYTE_BUDGET,
            page.more_beyond_page,
        ) {
            Ok(reply) => reply,
            Err(unfit_id) => ControlMsg::Error {
                req_id,
                message: format!(
                    "session {unfit_id} does not fit in a ListSessions reply even alone \
                     ({LIST_BYTE_BUDGET}-byte budget); the page cannot be represented"
                ),
                kind: ErrorKind::Internal,
            },
        };
        send_reply(&tx, &reply).await;
    })
    .await;
}

/// Spawned for the same reason as `ListSessions`: the process-
/// tree sweep (`service::sweep`'s `reap_process_tree`) is a grace-period
/// sleep plus repeated `/proc` walks and confirmation polls
/// that can take real wall-clock seconds (see that function's
/// own docs), and awaiting it inline would stall every OTHER
/// session's attach, input, and list/stop/delete behind this
/// one stop. Safe for the same locking reason: this handler's
/// `sessions` lookup is a single lock-guarded clone, and stop
/// leaves attachment entries intact (it briefly takes the
/// attachment-map lock to serialize the stop snapshot against
/// deletion, but never removes or replaces an entry) and never
/// touches `input_routes` at all (see `ControlMsg::StopSession`'s
/// own docs on why the existing attachment is left untouched).
/// Tracked and admitted exactly like `ListSessions` above — see
/// `HANDLER_ADMISSION_PERMITS`/`HANDLER_SHUTDOWN_TIMEOUT`.
async fn handle_stop_session(
    sup: &Arc<Supervisor>,
    tx: &mpsc::Sender<Frame>,
    tasks: &mut tokio::task::JoinSet<()>,
    req_id: u64,
    session_id: String,
) {
    let sup2 = Arc::clone(sup);
    let tx = tx.clone();
    spawn_admitted(&sup.admission, tasks, async move {
        let sup = sup2;
        // This session's lifecycle claim, held for the whole stop —
        // the intent, the sweep, and the outcome. Without it a
        // restart running concurrently would have this sweep reap
        // the agent IT just launched: the sweep is keyed on the
        // session's environment marker, which the new run carries
        // too. See `Supervisor::lifecycle_locks`.
        let _lifecycle = sup.lifecycle_locks.claim(&session_id).await;
        let entry = sup.sessions.lock().await.get(&session_id).cloned();
        let Some(entry) = entry else {
            send_reply(
                &tx,
                &ControlMsg::Error {
                    req_id,
                    message: format!("no such session: {}", truncate_for_error(&session_id)),
                    kind: ErrorKind::NotFound,
                },
            )
            .await;
            return;
        };
        // A dead or absent pane, or a terminal-less (restart-gap)
        // entry, all mean there is no live pid worth walking
        // ancestry from — but the environment-marker sweep still
        // runs regardless (`root_pid: None`), because SPEC.md
        // assigns reaping any leftover descendants of a PAST run
        // to the session's next stop or delete, and the marker
        // scan is the only mechanism that can still find such a
        // survivor once there is no live pane to walk from at
        // all. See `service::sweep`'s `kill_process_tree` docs.
        let pane_state = match entry.terminal.as_ref() {
            Some(terminal) => match sup
                .tmux
                .pane_process(&terminal.tmux_name, &terminal.pane)
                .await
            {
                Ok(PaneProbe::Owned(pane)) => Some(pane),
                Ok(PaneProbe::Gone) => None,
                // A RECOGNIZED owner folds into the dead-or-absent path,
                // which is what makes the record below honest rather than
                // merely convenient: another farhelm session holding this
                // pane id proves the recording predates the current tmux
                // server, so this session's agent DID exit on its own —
                // with the server that owned its pane. A plain exit is
                // exactly what happened, which is why this is not recorded
                // as an annotated stop. The marker sweep that path runs is
                // the SPEC.md-assigned mechanism for a past run's
                // survivors and needs no pane; the stranger's pid is never
                // touched. Erroring instead — what this used to do — left
                // stop permanently unavailable after the 2026-08-16 tmux
                // server death.
                //
                // An UNRECOGNIZED owner still errors. The recorded pane
                // may be this session's own live agent under a renamed
                // tmux session, and recording a plain exit for a process
                // that never exited is a lie the user would act on.
                Ok(PaneProbe::ForeignOwner { owner }) => {
                    if !sup.known_session_tmux_name(&owner).await {
                        send_reply(
                            &tx,
                            &ControlMsg::Error {
                                req_id,
                                message: unknown_pane_owner_refusal(
                                    &terminal.pane,
                                    &owner,
                                    &terminal.tmux_name,
                                ),
                                kind: ErrorKind::Internal,
                            },
                        )
                        .await;
                        return;
                    }
                    warn!(
                        session = %session_id, foreign_owner = %owner,
                        "this session's recorded pane now belongs to another tmux session; the \
                         stop records it as no longer running and sweeps by marker alone"
                    );
                    None
                }
                Err(e) => {
                    send_reply(
                        &tx,
                        &ControlMsg::Error {
                            req_id,
                            message: format!("{e:#}"),
                            kind: error_kind(&e),
                        },
                    )
                    .await;
                    return;
                }
            },
            None => None,
        };
        // One "alive" check deciding BOTH which lifecycle this stop
        // runs and whether an alt-screen capture is worth
        // attempting, rather than two independent `!pane.dead`
        // checks scattered across this handler. The stale pid a
        // dead pane still reports is deliberately never read; it
        // may already be recycled.
        let alive_pane = pane_state.filter(|pane| !pane.dead);

        // What this stop records, and why the two branches differ.
        //
        // Only a pane observed ALIVE gets the stop lifecycle and
        // its annotation: an agent that had already exited on its
        // own is not something the user
        // stopped, and claiming otherwise would credit them with
        // an ending they had nothing to do with. That case records
        // the plain exit instead — with whatever code the dead
        // pane still retains — because a stop is also the moment
        // this supervisor witnesses an exit nobody had listed yet.
        //
        // The sentinel is checked FIRST, though (item 3a of the
        // review-swarm fix batch): a dead-or-absent pane at stop
        // time can just as easily mean the launch never execed at
        // all, and this is exactly the commit boundary PLAN_M3.md
        // item 3 warns about — a stop (or any other first
        // observer) committing a plain `ObservedExit` before
        // anything ever reads the sentinel locks in "exited"
        // behind terminal-stickiness, permanently outrunning a
        // classification the file already had evidence for.
        // Checking here, before the write, is what keeps a stop
        // from being the race that loses.
        //
        // The alive path below is the stop lifecycle proper —
        // durable intent, sweep, outcome — and it is shared
        // verbatim with PLAN_M3.md item 9's restart, which stops a
        // still-running agent before relaunching it
        // (`stop_live_agent`). The dead-or-absent path is this
        // handler's alone: it records a CLASSIFICATION rather than
        // an intent, and has no annotation to write.
        let stop_error = if let Some(pane) = alive_pane {
            // Capture the alt-screen snapshot (if any) BEFORE the
            // kill destroys it, but do NOT write it to disk yet —
            // see `publish_alt_screen_snapshot`'s docs for why
            // publishing waits until the kill's own outcome is
            // known. `capture_alt_screen_before_stop` itself
            // decides (atomically, in tmux) whether that pane is
            // really on the alternate screen.
            //
            // Deliberately taken before the durable intent too,
            // which is a change from when this handler owned the
            // whole lifecycle inline: the capture is a READ (one
            // tmux `capture-pane`) that signals nothing, so it
            // cannot violate the "nothing dies before the intent is
            // recorded" rule the intent exists to enforce — and
            // moving it out of the middle is what lets intent,
            // sweep, and outcome stay one shared unit.
            let pending_snapshot = match entry.terminal.as_ref() {
                Some(terminal) => capture_alt_screen_before_stop(&sup, &session_id, terminal).await,
                None => None,
            };
            // Published into `Supervisor::pending_snapshots` (see
            // that field's own docs) BEFORE the kill runs:
            // `kill_process_tree` can take up to a couple of
            // seconds against an uncooperative tree, and tmux can
            // mark the pane dead well before that returns. Making
            // the capture visible to a concurrent `Attach` for this
            // whole window — not only after
            // `publish_alt_screen_snapshot` finally writes it to
            // disk — is what closes the "attach lands mid-stop,
            // sees a dead pane with nothing to show" gap. Cloned
            // rather than moved: this handler still needs its own
            // copy below regardless of what `Attach` does with the
            // map's copy concurrently.
            if let Some(bytes) = pending_snapshot.clone() {
                sup.pending_snapshots
                    .lock()
                    .await
                    .insert(session_id.clone(), bytes);
            }
            let stopped = stop_live_agent(&sup, &session_id, &entry, Some(pane.pid)).await;
            // Published only for the two outcomes that leave the
            // agent provably dead. A stop whose intent never
            // recorded (nothing was killed) or whose sweep could
            // not be confirmed must never plant a snapshot file
            // that a later, unrelated exit's own dead-pane replay
            // could be mistaken for.
            if matches!(stopped, Ok(()) | Err(StopFailure::UnrecordedOutcome(_)))
                && let Some(bytes) = pending_snapshot
            {
                publish_alt_screen_snapshot(&sup, &session_id, &bytes, crate::files::RealFs).await;
            }
            // Removed only now, AFTER publish has run (or been
            // skipped because there was never anything to publish):
            // a concurrent `Attach` must be able to see this entry
            // for the entire capture-to-published-file window, not
            // just up to this point — see
            // `Supervisor::pending_snapshots`'s docs.
            sup.pending_snapshots.lock().await.remove(&session_id);
            stopped.err().map(|failure| failure.message())
        } else {
            let current = entry
                .outcome
                .lock()
                .expect("outcome mutex poisoned")
                .clone();
            let sentinel = if sentinel_could_still_apply(&current) {
                read_launch_sentinel(&sup.state_dir, &session_id, entry.generation).await
            } else {
                Ok(None)
            };
            let classification = match sentinel {
                Ok(Some(detail)) => Transition::SentinelError {
                    detail,
                    pane: entry.terminal.as_ref().map(|t| t.pane.clone()),
                },
                // The wrapper-failure shape outranks the plain exit
                // for the same reason a sentinel does: an agent that
                // never started did not "run and finish".
                Ok(None) => match wrapper_failure_detail(
                    &sup.state_dir,
                    &session_id,
                    entry.generation,
                    entry.scope.is_some(),
                    pane_state.is_some_and(|state| state.dead),
                )
                .await
                {
                    Some(detail) => Transition::SentinelError {
                        detail,
                        pane: entry.terminal.as_ref().map(|t| t.pane.clone()),
                    },
                    None => Transition::ObservedExit {
                        exit_code: dead_pane_exit_code(&sup, entry.terminal.as_ref(), &session_id)
                            .await,
                    },
                },
                Err(e) => {
                    // Loud propagation (item 1's discipline,
                    // extended to this call site): refuse the
                    // whole stop rather than durably committing a
                    // plain exit this sentinel might contradict.
                    // Nothing was alive to signal anyway
                    // (`alive_pane` is already `None` in this
                    // branch), so nothing beyond the classification
                    // write itself is lost — the caller can retry
                    // once the sentinel is readable again.
                    send_reply(
                        &tx,
                        &ControlMsg::Error {
                            req_id,
                            message: format!(
                                "could not read this session's launch sentinel, so \
                                 nothing was recorded: {e:#}"
                            ),
                            kind: ErrorKind::Internal,
                        },
                    )
                    .await;
                    return;
                }
            };
            if let Err(e) = sup.record(&session_id, &entry, classification).await {
                // Recording what this stop witnessed is part of its
                // contract, not bookkeeping around it, and SPEC.md
                // requires the failure to surface rather than be
                // logged past. Nothing has been killed at this
                // point either way.
                send_reply(
                    &tx,
                    &ControlMsg::Error {
                        req_id,
                        message: format!("recording the stop failed, so nothing was killed: {e:#}"),
                        kind: ErrorKind::Internal,
                    },
                )
                .await;
                return;
            }
            // Sentinel lifecycle: if the classification just
            // recorded WAS a `SentinelError` and it committed, this
            // stop is the moment that classification became durable
            // — clean up both files right away rather than waiting
            // for a later list or reload to notice (item 4/25 of
            // the review-swarm fix batch; see
            // `cleanup_launch_artifacts`'s own docs).
            if matches!(
                &*entry.outcome.lock().expect("outcome mutex poisoned"),
                LastOutcome::Error { .. }
            ) {
                cleanup_launch_artifacts(&sup.state_dir, &session_id, entry.generation).await;
            }
            // The reap still runs with no live pid to walk from:
            // SPEC.md assigns reaping a PAST run's leftover
            // descendants to the session's next stop or delete, and
            // once there is no live pane the environment-marker
            // scan and this launch's cgroup are the only mechanisms
            // that can still find one. The scope in particular
            // outlives the agent for exactly as long as something
            // it spawned does, which is the case at hand.
            //
            // `AgentOnly` like the live-agent path above: this is
            // still a stop, and stop leaves tabs running whether
            // or not the agent was alive to begin with.
            if let Err(e) = reap_process_tree(
                &sup.seams.scopes,
                entry.scope.as_slice(),
                None,
                &session_id,
                &SweepTarget::AgentOnly,
            )
            .await
            {
                // The sweep itself failed (not just "nothing was
                // found to kill") — this is not a false success.
                // See `ControlMsg::StopSession`'s docs: a caller
                // must be able to tell "nothing was running" from
                // "the sweep could not confirm nothing is running".
                send_reply(
                    &tx,
                    &ControlMsg::Error {
                        req_id,
                        message: format!("{e:#}"),
                        kind: ErrorKind::Internal,
                    },
                )
                .await;
                return;
            }
            None
        };

        // Deliberately untouched: the DB row, the sessions map, and
        // any live attachment. The pane survives (remain-on-exit),
        // so an attached client's stream simply goes quiet after the
        // agent's death output — there is nothing here for it to be
        // notified of, unlike delete below.
        match stop_error {
            Some(message) => {
                send_reply(
                    &tx,
                    &ControlMsg::Error {
                        req_id,
                        message,
                        kind: ErrorKind::Internal,
                    },
                )
                .await
            }
            None => send_reply(&tx, &ControlMsg::SessionStopped { req_id }).await,
        }
    })
    .await;
}

/// Delete's mutation belongs to the supervisor, not to the connection that
/// requested it. The connection-owned task waits only to deliver the reply;
/// forced connection shutdown may abort that waiter, but cannot interrupt a
/// process sweep after it has begun or release its admission slot early.
async fn handle_delete_session(
    sup: &Arc<Supervisor>,
    tx: &mpsc::Sender<Frame>,
    tasks: &mut tokio::task::JoinSet<()>,
    req_id: u64,
    session_id: String,
) {
    let permit = Arc::clone(&sup.admission)
        .acquire_owned()
        .await
        .expect("admission semaphore is never closed");
    let mutation_sup = Arc::clone(sup);
    let mutation_id = session_id.clone();
    // Spawned before the connection-owned waiter exists: once this returns
    // a handle, disconnect shutdown can abort reply delivery but not the
    // teardown or release its admission slot early.
    let mutation = tokio::spawn(async move {
        let outcome = async {
            let _lifecycle = mutation_sup.lifecycle_locks.claim(&mutation_id).await;
            let entry = mutation_sup
                .sessions
                .lock()
                .await
                .get(&mutation_id)
                .cloned()
                .ok_or_else(|| {
                    RequestError::new(
                        ErrorKind::NotFound,
                        format!("no such session: {}", truncate_for_error(&mutation_id)),
                    )
                })?;
            mutation_sup
                .teardown_session(&entry, &mutation_id)
                .await
                .map_err(|error| {
                let message = match error {
                    TeardownError::PaneProbe(error) => {
                        format!("querying pane process: {error:#}")
                    }
                    TeardownError::TabRediscovery(error) => format!(
                        "could not determine this session's terminal tabs, so nothing was \
                         deleted: {error:#}"
                    ),
                    TeardownError::TabScopeEnumeration(error) => format!(
                        "this host has a systemd user manager but its terminal-tab scopes could \
                         not be enumerated, so nothing was deleted: {error:#}"
                    ),
                    TeardownError::Sweep(error) => format!("killing process tree: {error:#}"),
                    TeardownError::FailClosed(message) => message,
                };
                RequestError::new(ErrorKind::Internal, message)
                })
        }
        .await;
        (outcome, permit)
    });
    let tx = tx.clone();
    tasks.spawn(async move {
        match mutation.await {
            Ok((Ok(()), _permit)) => send_reply(&tx, &ControlMsg::SessionDeleted { req_id }).await,
            Ok((Err(error), _permit)) => {
                send_reply(
                    &tx,
                    &ControlMsg::Error {
                        req_id,
                        message: error.message,
                        kind: error.kind,
                    },
                )
                .await;
            }
            Err(join) => {
                send_reply(
                    &tx,
                    &ControlMsg::Error {
                        req_id,
                        message: format!("the session delete task failed: {join}"),
                        kind: ErrorKind::Internal,
                    },
                )
                .await;
            }
        }
    });
}

/// Archive is a whole-session teardown that keeps the row and attachment
/// directory, then returns the freshly derived row to the caller.
///
/// Like delete, the mutation is supervisor-owned while the connection owns
/// only reply delivery. The lifecycle claim makes archive, restart, stop,
/// rename, and delete resolve to one winner for this session. An
/// already-archived row skips teardown entirely and returns the same current
/// `SessionInfo`, which makes a retry after an ambiguous transport failure
/// idempotent.
async fn handle_archive_session(
    sup: &Arc<Supervisor>,
    tx: &mpsc::Sender<Frame>,
    tasks: &mut tokio::task::JoinSet<()>,
    req_id: u64,
    session_id: String,
) {
    let permit = Arc::clone(&sup.admission)
        .acquire_owned()
        .await
        .expect("admission semaphore is never closed");
    let mutation_sup = Arc::clone(sup);
    let mutation_id = session_id.clone();
    let mutation = tokio::spawn(async move {
        let outcome = async {
            let _lifecycle = mutation_sup.lifecycle_locks.claim(&mutation_id).await;
            let entry = mutation_sup
                .sessions
                .lock()
                .await
                .get(&mutation_id)
                .cloned()
                .ok_or_else(|| {
                    RequestError::new(
                        ErrorKind::NotFound,
                        format!("no such session: {}", truncate_for_error(&mutation_id)),
                    )
                })?;
            if entry.info.archived {
                return Ok(entry);
            }
            mutation_sup
                .teardown_for_archive(&entry, &mutation_id)
                .await
                .map_err(|error| {
                let message = match error {
                    ArchiveError::PaneProbe(error) => {
                        format!("querying pane process before archive: {error:#}")
                    }
                    ArchiveError::TabRediscovery(error) => format!(
                        "could not determine this session's terminal tabs, so nothing was \
                         archived: {error:#}"
                    ),
                    ArchiveError::TabScopeEnumeration(error) => format!(
                        "this host has a systemd user manager but its terminal-tab scopes could \
                         not be enumerated, so nothing was archived: {error:#}"
                    ),
                    ArchiveError::Sweep(error) => {
                        format!("killing process tree for archive: {error:#}")
                    }
                    ArchiveError::FailClosed(message) => message,
                };
                RequestError::new(ErrorKind::Internal, message)
                })
        }
        .await;
        (outcome, permit)
    });
    let reply_sup = Arc::clone(sup);
    let tx = tx.clone();
    tasks.spawn(async move {
        let entry = match mutation.await {
            Ok((Ok(entry), _permit)) => entry,
            Ok((Err(error), _permit)) => {
                send_reply(
                    &tx,
                    &ControlMsg::Error {
                        req_id,
                        message: error.message,
                        kind: error.kind,
                    },
                )
                .await;
                return;
            }
            Err(join) => {
                send_reply(
                    &tx,
                    &ControlMsg::Error {
                        req_id,
                        message: format!("the session archive task failed: {join}"),
                        kind: ErrorKind::Internal,
                    },
                )
                .await;
                return;
            }
        };
        match session_info_now(&reply_sup, &entry).await {
            Ok(session) => {
                send_reply(&tx, &ControlMsg::SessionArchived { req_id, session }).await;
            }
            Err(error) => {
                send_reply(
                    &tx,
                    &ControlMsg::Error {
                        req_id,
                        message: format!(
                            "the session was archived, but its fresh metadata could not be read: \
                             {error:#}"
                        ),
                        kind: ErrorKind::Internal,
                    },
                )
                .await;
            }
        }
    });
}

/// Every request-shape check the attach can make lives here,
/// ahead of the first lookup and far ahead of the takeover: a
/// malformed attach must cost the session's current client
/// nothing. See [`MAX_LEASE_BYTES`] for why an unbounded lease
/// is a memory question rather than a parsing one.
/// A channel carrying an upload counts as in use too: the two
/// route maps are separate, but the channel-id space they name
/// is one and the same, and a data frame that could mean
/// either terminal input or upload bytes is a frame nothing
/// can route correctly.
#[allow(clippy::too_many_arguments)]
async fn handle_attach(
    sup: &Arc<Supervisor>,
    tx: &mpsc::Sender<Frame>,
    input_routes: &mut HashMap<u32, InputRoute>,
    upload_routes: &HashMap<u32, UploadRoute>,
    req_id: u64,
    session_id: String,
    channel: u32,
    cols: u16,
    rows: u16,
    selector: TerminalSelector,
    lease: String,
    if_unowned: bool,
) {
    if channel == 0
        || input_routes.contains_key(&channel)
        || upload_routes.contains_key(&channel)
        || lease.len() > MAX_LEASE_BYTES
    {
        let message = if channel == 0 {
            "attachment channel 0 is reserved".to_string()
        } else if input_routes.contains_key(&channel) || upload_routes.contains_key(&channel) {
            format!("attachment channel {channel} is already in use")
        } else {
            format!(
                "attachment lease is {} bytes, over the {MAX_LEASE_BYTES}-byte cap",
                lease.len()
            )
        };
        send_reply(
            tx,
            &ControlMsg::Error {
                req_id,
                message,
                kind: ErrorKind::InvalidRequest,
            },
        )
        .await;
        return;
    }
    let terminal_id = TerminalId::from(selector);
    // A TAB attach holds the session's lifecycle claim across its
    // resolution and its takeover; an AGENT attach does not.
    //
    // The asymmetry is deliberate. A tab is a window another
    // client can CLOSE, so between resolving it and displacing the
    // incumbent it can stop existing — and a refused attach must
    // be side-effect-free even when the close wins the race, which
    // it cannot be if the terminal was resolved outside any claim.
    // Holding it is what makes `close_tab`'s own claim mean
    // something from this side. The agent terminal has no such
    // race (nothing removes it while its session lives), and
    // taking the claim for it would queue every ordinary attach
    // behind a multi-second stop or delete for no gain.
    let _lifecycle = match &terminal_id {
        TerminalId::Tab(_) => Some(sup.lifecycle_locks.claim(&session_id).await),
        TerminalId::Agent => None,
    };
    let entry = sup.sessions.lock().await.get(&session_id).cloned();
    let Some(entry) = entry else {
        send_reply(
            tx,
            &ControlMsg::Error {
                req_id,
                message: format!("no such session: {}", truncate_for_error(&session_id)),
                kind: ErrorKind::NotFound,
            },
        )
        .await;
        return;
    };
    if entry.info.archived {
        send_reply(
            tx,
            &ControlMsg::Error {
                req_id,
                message: format!(
                    "session {} is archived and has no terminal; restart it before attaching",
                    truncate_for_error(&session_id)
                ),
                kind: ErrorKind::InvalidRequest,
            },
        )
        .await;
        return;
    }
    // Which terminal this attach is FOR, resolved once here and
    // used for everything below: the tmux handles to drive, and
    // the second half of the attachment key.
    //
    // Resolved BEFORE the takeover below, deliberately: an attach
    // that cannot be honored must take nothing over, so naming a
    // terminal that does not exist can never cost this session's
    // current client its attachments. Also resolved before the
    // `attachments` lock is taken, because resolving a tab is a
    // tmux subprocess and that mutex is supervisor-wide.
    let terminal = match resolve_terminal(sup, &entry, &terminal_id).await {
        Ok(terminal) => terminal,
        Err(e) => {
            send_reply(
                tx,
                &ControlMsg::Error {
                    req_id,
                    message: e.message,
                    kind: e.kind,
                },
            )
            .await;
            return;
        }
    };
    let key = AttachmentKey::new(&session_id, terminal_id);

    // Reserve one writer slot BEFORE taking any lock, so this
    // handler's eventual reply — success or failure — can be enqueued
    // without awaiting. Everything below either holds
    // `attachments` or must not race the forwarder it is about to
    // spawn, and awaiting a bounded queue in either position is
    // what this reservation exists to make impossible. Waiting
    // here instead is exactly right: it backpressures the
    // connection's read loop before the attach has touched
    // anything.
    let Ok(permit) = tx.reserve().await else {
        // The connection is gone; there is nobody to attach for.
        return;
    };

    // The session's sink client, acquired BEFORE the attachments
    // lock and held across everything below. Three orderings ride
    // on this one line.
    //
    // It must precede the per-terminal clients opened further
    // down, because those turn the session's other panes off for
    // themselves and tmux answers by not reading a pane no client
    // wants (see `tmux::SessionSink`).
    //
    // It must precede the takeover sweep, so that a reattach —
    // where the incumbent holds the session's only other reference
    // — hands this attachment the SAME live sink instead of
    // letting the last reference drop and immediately spawning a
    // replacement.
    //
    // And it deliberately happens OUTSIDE the lock, unlike the
    // other tmux work here. Bringing a sink up costs a process
    // spawn and two control-mode round trips, and this mutex
    // serializes attach and input for EVERY session in this
    // supervisor (see the `Supervisor` struct's own note on that
    // coarseness) — so paying for it under the lock would make one
    // session's first attach measurably slow every other session's
    // traffic. Nothing about the sink needs the lock: the registry
    // has its own, and it is this `Arc` rather than any map entry
    // that keeps the sink alive.
    let sink = match sup.ensure_session_sink(&terminal.tmux_name).await {
        Ok(sink) => sink,
        Err(e) => {
            permit.send(reply_frame(&ControlMsg::Error {
                req_id,
                message: format!("{e:#}"),
                kind: error_kind(&e),
            }));
            return;
        }
    };

    // The whole takeover — kick every attachment this attach
    // displaces, set up tmux, install the new one — runs under
    // ONE lock hold. Without it, two concurrent attaches can both
    // pass the kick step and both install forwarders, leaving one
    // terminal with two live attachments (and, worse, a session
    // whose terminals are split between two clients, which SPEC.md
    // forbids). It also makes the winner the *last* attach rather
    // than whichever client's tmux calls happened to finish last.
    let mut attachments = loop {
        let attachments = sup.attachments.lock().await;
        if !sup.has_output_reap_for_key(&key) {
            break attachments;
        }
        drop(attachments);
        if let Err(error) = sup.wait_for_output_reap(&key).await {
            permit.send(reply_frame(&ControlMsg::Error {
                req_id,
                message: format!("terminal output cleanup is unconfirmed: {error}"),
                kind: ErrorKind::Internal,
            }));
            return;
        }
    };

    // Revalidate the entry this attach resolved BEFORE installing
    // anything on it (fix-batch item 6). The lookup above released
    // the `sessions` lock, and a restart landing in that window
    // replaces the entry (and respawns, or replaces, its pane) —
    // so an attachment installed from the stale entry would be
    // wired to the NEW run's terminal while nothing in this handler
    // ever checked that the user may drive it.
    //
    // A changed POINTER is not by itself a changed run, though, and
    // treating it as one made a rename racing an attach fail with a
    // spurious `Conflict` (PLAN_M5.md item 3 publishes a rebuilt
    // entry for a title change — same run, same pane, same
    // generation). Archive is the other same-generation replacement and
    // is refused explicitly: it retains the row while removing the terminal,
    // so calling it a restart or delete would describe neither winner. A
    // replacement is accepted when it still
    // describes what this attach resolved: same generation, same
    // terminal identity. Everything else is refused exactly as
    // before — a changed generation is a restart, a vanished entry
    // is a delete (or an id something else took), and a changed
    // terminal is a substrate this attach never validated.
    //
    // The accepted entry — not the one resolved earlier — is what
    // gets installed below, so the route pins the CURRENT title and
    // the shared cells behind it.
    let same_terminal = |a: Option<&Terminal>, b: Option<&Terminal>| match (a, b) {
        (Some(a), Some(b)) => a.tmux_name == b.tmux_name && a.pane == b.pane,
        (None, None) => true,
        _ => false,
    };
    let current = sup.sessions.lock().await.get(&session_id).cloned();
    let entry = match current {
        Some(current) if current.info.archived => {
            drop(attachments);
            permit.send(reply_frame(&ControlMsg::Error {
                req_id,
                message: format!(
                    "session {} was archived while this attach was being set up; restart it \
                     before attaching",
                    truncate_for_error(&session_id)
                ),
                kind: ErrorKind::InvalidRequest,
            }));
            return;
        }
        Some(current)
            if Arc::ptr_eq(&current, &entry)
                || (current.generation == entry.generation
                    && same_terminal(current.terminal.as_ref(), entry.terminal.as_ref())) =>
        {
            current
        }
        _ => {
            drop(attachments);
            permit.send(reply_frame(&ControlMsg::Error {
                req_id,
                message: format!(
                    "session {} changed while this attach was being set up (it was \
                     restarted or deleted); attach again",
                    truncate_for_error(&session_id)
                ),
                kind: ErrorKind::Conflict,
            }));
            return;
        }
    };

    // SPEC.md's one-attached-client rule, enforced across the
    // WHOLE session (PLAN_M4.md item 3): every channel held under
    // a different lease loses its attachment, whichever of the
    // session's terminals it was on. Running inside the same lock
    // hold as the installation below is what makes the wire
    // contract's "atomically" true — no observer can see a moment
    // where the old lease still holds one terminal while the new
    // one already holds another.
    //
    // The empty lease is not a lease (see `same_lease_client`), so
    // a legacy attach sweeps everything and is itself swept by
    // anything: exactly the pre-M4 behavior every un-leased client
    // was written against.
    //
    // Nothing here touches the losers' `input_routes`: those are
    // connection-local, and a connection that swept away its own
    // earlier channels (a browser reload attaching under a new
    // lease) simply carries stale routes until their next frame,
    // which the input arm drops on the same two-part ownership
    // check that has always guarded it. A stale route can never
    // deliver to the winner — the channel ids differ, and the
    // arm compares both.
    // A non-displacing attach refuses instead of sweeping (PLAN_M6.md
    // item 7; `ControlMsg::Attach::if_unowned`). Checked with the SAME
    // predicate the sweep below uses, so "would this attach take the
    // session from someone" cannot drift from "who does this attach take
    // it from", and inside the same lock hold for the reason that hold
    // exists at all: a check outside it could be answered before a
    // concurrent attach installed the owner it was asking about.
    //
    // The refusal names the takeover, because that is what happened —
    // see `ATTACH_REFUSED_TAKEN_OVER`. Nothing is installed and nothing
    // is torn down: the caller's channel stays unattached, which is what
    // makes an auto-reconnect that loses this race a no-op rather than an
    // eviction.
    if if_unowned
        && attachments
            .iter()
            .any(|(k, a)| displaced_by_attach(k, &a.lease, &session_id, &lease))
    {
        drop(attachments);
        permit.send(reply_frame(&ControlMsg::Error {
            req_id,
            message: farhelm_proto::ATTACH_REFUSED_TAKEN_OVER.to_string(),
            kind: ErrorKind::Conflict,
        }));
        return;
    }
    let displaced: Vec<(AttachmentKey, ActiveAttach)> = attachments
        .extract_if(|k, a| displaced_by_attach(k, &a.lease, &session_id, &lease))
        .collect();
    // Same terminal, same lease: an ordinary reconnect, and the
    // one-attachment-per-terminal enforcement point. It cannot
    // overlap the sweep above — a same-lease incumbent is never
    // swept — so removing it separately is not a second takeover,
    // just the cutover that has always happened here. Which is
    // exactly why it is told a DIFFERENT reason: see
    // `DETACH_REASON_REPLACED`.
    let incumbent = attachments
        .remove(&key)
        .map(|attachment| (key.clone(), attachment));

    // Stop every doomed forwarder BEFORE awaiting any of them. The joins run
    // concurrently, so signalling in the same loop would let a quick cleanup
    // finish and emit effects while later forwarders were still live.
    for (old_key, old) in displaced.iter().chain(incumbent.iter()) {
        sup.begin_forwarder_shutdown(old_key.clone(), old);
    }
    let mut notices = Vec::with_capacity(displaced.len() + usize::from(incumbent.is_some()));
    let doomed = displaced
        .into_iter()
        .map(|(key, old)| (key, old, DETACH_REASON_TAKEOVER))
        .chain(
            incumbent
                .into_iter()
                .map(|(key, old)| (key, old, DETACH_REASON_REPLACED)),
        );
    let mut forwarders = tokio::task::JoinSet::new();
    for (old_key, old, reason) in doomed {
        // `..` drops this attachment's input client (killing its
        // control-mode process via `kill_on_drop`) and its pause
        // sender, which is what every teardown path does.
        let ActiveAttach {
            channel,
            notify,
            forwarder,
            sink,
            ..
        } = old;
        // Await until the old forwarder either reaps its control client or
        // transfers that client to the published per-terminal reaper. The
        // request refuses the replacement below while that barrier remains.
        // The forwarder never takes this lock, so awaiting it here cannot
        // deadlock.
        forwarders.spawn(async move {
            let joined = forwarder.await;
            drop(sink);
            (old_key, joined, channel, notify, reason)
        });
    }
    let mut cleanup_error = None;
    while let Some(joined) = forwarders.join_next().await {
        match joined {
            Ok((old_key, result, channel, notify, reason)) => {
                if let Err(error) = sup.record_forwarder_join(old_key, result) {
                    cleanup_error.get_or_insert(error);
                }
                notices.push((channel, notify, reason));
            }
            Err(join) => {
                cleanup_error.get_or_insert_with(|| {
                    Arc::<str>::from(format!("terminal cleanup wrapper failed: {join}"))
                });
            }
        }
    }
    if let Some(error) = cleanup_error {
        drop(attachments);
        for (channel, notify, reason) in notices {
            notify_detached(&notify, channel, reason.to_string());
        }
        permit.send(reply_frame(&ControlMsg::Error {
            req_id,
            message: format!("the old terminal attachment could not be cleaned up: {error}"),
            kind: ErrorKind::Internal,
        }));
        return;
    }
    if sup.has_output_reap_for_key(&key) {
        drop(attachments);
        for (channel, notify, reason) in notices {
            notify_detached(&notify, channel, reason.to_string());
        }
        permit.send(reply_frame(&ControlMsg::Error {
            req_id,
            message: "the old terminal attachment is still being cleaned up; attach again"
                .to_string(),
            kind: ErrorKind::Conflict,
        }));
        return;
    }
    // Every notice is enqueued back to back, after the last
    // forwarder is gone, so a client that lost several terminals
    // at once sees them as one event and can coalesce the
    // identical reasons into a single banner (which is why the
    // protocol needs no session-scoped takeover message).
    //
    // One accepted caveat, stated so nobody "fixes" it: a client
    // whose own writer queue is completely full has its notice
    // handed to a spawned sender (`notify_detached`), which can
    // land after the winner's `Attached`. That is benign — the
    // losing channel is already dead, and a `Detached` is the
    // last frame it will ever carry — and the alternative,
    // awaiting a full queue here, would let one wedged peer
    // freeze every session's attach behind the `attachments`
    // mutex.
    for (channel, notify, reason) in notices {
        notify_detached(&notify, channel, reason.to_string());
    }

    // Size the window now, not during prep: resizing is a
    // mutation the incumbent would have seen, and a later prep
    // failure would have left its terminal reflowed to a size
    // nobody is using.
    if let Err(e) = sup
        .tmux
        .resize_window(&terminal.tmux_name, &terminal.pane, cols, rows)
        .await
    {
        warn!(session = %session_id, error = %e, "resize during attach failed");
    }

    // The incumbent must be fully gone before the replacement
    // control client starts. Overlap reproducibly froze the new
    // stream after replay, even though two steady-state control
    // clients both receive output in isolation. The replacement
    // attaches with output disabled, captures replay, and enables
    // live output through that SAME client; its final command
    // block is the exact replay/live boundary.
    //
    // This setup happens after takeover on purpose. There is no
    // safe way to preserve the incumbent while also avoiding
    // control-client overlap, so failure here leaves the session
    // detached and reports only this attach request as failed.
    //
    // One replay stream and one input client PER ATTACHED
    // TERMINAL, which is what makes flow control per terminal:
    // `pause-after`/`%pause` are properties of a control client,
    // so a client shared across terminals would let one stalled
    // viewer pause another terminal's stream (PLAN_M4.md item 3).
    // The overlap hazard above is per TERMINAL too — it is about
    // two clients streaming one pane, not about a session having
    // several. The stream is opened against the tmux SESSION,
    // because that is the only thing a control client can attach
    // to; it therefore hears every window's panes and filters down
    // to this one by pane id (see `tmux::OutputStream`), which is
    // what keeps a tab's output out of the agent's terminal.
    let stream_candidate = match sup
        .open_replay_stream_candidate(key.clone(), &terminal.tmux_name, &terminal.pane)
        .await
    {
        Ok(candidate) => candidate,
        Err(e) => {
            drop(attachments);
            permit.send(reply_frame(&ControlMsg::Error {
                req_id,
                message: format!("{e:#}"),
                kind: error_kind(&e),
            }));
            return;
        }
    };
    // A second, dedicated control-mode client for this
    // attachment's input (see `InputClient`) — opened here rather
    // than derived from `stream`, since the two are now
    // independent control connections rather than one shared
    // stdin. A failure here must tear down the replay stream just
    // opened above: leaving it live would attach this session to
    // a client nothing will ever read from or write to again.
    let input = match sup
        .tmux
        .open_input_client(&terminal.tmux_name, &terminal.pane)
        .await
    {
        Ok(input) => input,
        Err(e) => {
            drop(stream_candidate);
            drop(attachments);
            permit.send(reply_frame(&ControlMsg::Error {
                req_id,
                message: format!("{e:#}"),
                kind: error_kind(&e),
            }));
            return;
        }
    };
    let (modes, prefill, stream) = stream_candidate.install();

    // The `Attached` reply is enqueued HERE, before the forwarder
    // exists, using the capacity reserved before this handler took
    // any lock. Both halves matter. It must precede the replay so
    // the client's `attach()` can return and its consumer can
    // start draining — otherwise a large replay floods the helm's
    // bounded per-terminal queue while nobody is allowed to read
    // it yet, and a perfectly healthy attach trips the
    // stalled-terminal detach. And it must not AWAIT here, because
    // `attachments` is held: `permit` makes the enqueue
    // infallible and instant.
    permit.send(reply_frame(&ControlMsg::Attached { req_id, channel }));

    // Everything from here on — the replay prefill, the dead-pane
    // snapshot, and the live pump — happens inside the forwarder
    // task rather than here, and that placement is load-bearing.
    // A full replay is megabytes of 32 KiB frames, and this
    // handler runs under the supervisor-wide `attachments` mutex;
    // sending them here would mean AWAITING a bounded queue (see
    // CONNECTION_WRITER_QUEUE) with that lock held, letting one
    // slow client stall every other session's attach and input.
    // Ordering is unaffected: the forwarder is this channel's
    // only writer, so its prefill necessarily precedes its own
    // live output. The dead-pane stop snapshot moved with it —
    // see `Forwarder::send_dead_pane_snapshot`.
    let (pause_tx, pause_rx) = watch::channel(None);
    let (forwarder_shutdown, shutdown_rx) = watch::channel(false);
    let (forwarder_cleanup, cleanup_rx) = watch::channel(None);
    let forwarder = Forwarder {
        sup: Arc::clone(sup),
        session_id: session_id.clone(),
        terminal: key.terminal.clone(),
        channel,
        tx: tx.clone(),
        stream,
        pause_rx,
        stall_timeout: sup.timeouts.stall_detach,
        cleanup: forwarder_cleanup,
    };
    let task = tokio::spawn(forwarder.run(modes, prefill, shutdown_rx));

    attachments.insert(
        key.clone(),
        ActiveAttach {
            channel,
            lease,
            notify: tx.clone(),
            forwarder: task,
            forwarder_shutdown,
            forwarder_cleanup: cleanup_rx,
            input,
            pause: pause_tx,
            sink,
        },
    );
    drop(attachments);
    // The route carries the same key the attachment was installed
    // under, so input, resize, and detach all address this
    // attachment by lookup rather than by search.
    input_routes.insert(channel, InputRoute { entry, key });
}

/// `Detach` names no session and no terminal, but the route
/// this connection registered at attach time does — so the
/// attachment is addressed by key rather than found by
/// scanning every session's attachments. A channel with no
/// route was never attached here (or was already detached),
/// which `Detach` treats as the no-op its idempotence promises.
async fn handle_detach(
    sup: &Arc<Supervisor>,
    tx: &mpsc::Sender<Frame>,
    input_routes: &mut HashMap<u32, InputRoute>,
    channel: u32,
) {
    let Some(route) = input_routes.remove(&channel) else {
        return;
    };
    let mut attachments = sup.attachments.lock().await;
    // The same two-part ownership check the input and resize
    // paths make, and for the same reason: by now a takeover may
    // have installed somebody else's attachment under this key,
    // and a stale `Detach` must never tear that one down. Channel
    // ids are unique only within a connection, so `same_channel`
    // is what identifies the owner.
    let mine = attachments
        .get(&route.key)
        .is_some_and(|a| a.channel == channel && a.notify.same_channel(tx));
    if mine && let Some(a) = attachments.remove(&route.key) {
        // Request shutdown AND await, mirroring the takeover path: detach
        // followed by an immediate reattach (a browser reload is
        // exactly this) finds no incumbent to kick, so the only
        // thing keeping the old control-mode client from
        // overlapping the new one — the documented frozen-replay
        // hazard — is waiting for it here, before the lock is
        // released. Awaiting cannot deadlock: forwarders never
        // take this lock.
        sup.begin_forwarder_shutdown(route.key.clone(), &a);
        let ActiveAttach {
            forwarder, sink, ..
        } = a;
        if let Err(cleanup) = sup.record_forwarder_join(route.key.clone(), forwarder.await) {
            warn!(
                session = %route.key.session,
                terminal = ?route.key.terminal,
                error = %cleanup,
                "detached terminal output cleanup remains unconfirmed"
            );
        }

        // Dropping the final lease publishes this session's reaping barrier
        // synchronously. The runtime-owned reaper may finish afterward, but
        // the next attach already has a state it must wait on; a shared sink
        // simply retains its other lease and never enters that state.
        drop(sink);
    }
}

/// The channel's own route is both the lookup key and the
/// target: it names the (session, terminal) this channel
/// attached to, so a resize can only ever reflow the terminal
/// its sender actually holds — a client with two of a
/// session's terminals cannot reflow the one it did not name
/// (PLAN_M4.md item 3: resize goes per window, which is why
/// `Resize` carries no terminal selector at all). A channel
/// with no route on this connection is not attached here, and
/// a route naming a different session than the request does is
/// a client contradicting itself; both are ignored.
async fn handle_resize(
    sup: &Arc<Supervisor>,
    tx: &mpsc::Sender<Frame>,
    input_routes: &HashMap<u32, InputRoute>,
    session_id: String,
    channel: u32,
    cols: u16,
    rows: u16,
) {
    let Some(route) = input_routes.get(&channel) else {
        return;
    };
    if route.key.session != session_id {
        return;
    }
    // Same trust boundary as input, and the same two-part check
    // as the Data arm: `same_channel` identifies the owning
    // connection, and the channel id tells apart clients
    // multiplexed over ONE connection — every browser tab rides
    // the helm's single supervisor connection, so a
    // connection-level check alone would let a tab that just
    // lost a takeover reflow the winner's terminal.
    //
    // The check and the resize run UNDER one hold of the
    // attachments lock, like the Attach handler's tmux calls.
    // Checking ownership and then resizing after releasing the
    // lock is a TOCTOU: a takeover can interleave in that gap,
    // and the kicked client's already-authorized resize would
    // land after the winner's attach-time resize, reflowing the
    // winner's terminal with nothing to correct it.
    // Resolved BEFORE the lock, never under it. Resolving a TAB is
    // a tmux subprocess, and `attachments` is a supervisor-wide
    // mutex that every session's attach, input and resize queues
    // behind — running a subprocess inside it would serialize the
    // whole supervisor on one client's drag. The ownership check
    // that follows is what makes resolving early safe: a
    // resolution that went stale by the time the lock is held
    // simply does not act.
    //
    // Re-resolved rather than remembered, and no longer
    // infallible: a TAB is a tmux window that another client can
    // close while this one still holds an attachment to it, so
    // "the terminal this channel names" can genuinely stop
    // existing between attach and resize. Reflowing nothing is
    // the right answer then — the forwarder is already on its way
    // to detaching this channel — and it is the same
    // fire-and-forget silence a resize gets for any other reason
    // it cannot land.
    let terminal = match resolve_terminal(sup, &route.entry, &route.key.terminal).await {
        Ok(terminal) => terminal,
        Err(e) => {
            debug!(
                session = %session_id, channel,
                reason = %e.message,
                "ignoring a resize for a terminal that no longer resolves"
            );
            return;
        }
    };
    let attachments = sup.attachments.lock().await;
    let owns = attachments
        .get(&route.key)
        .is_some_and(|a| a.channel == channel && a.notify.same_channel(tx));
    if owns {
        // Fire-and-forget: a resize has no req_id to answer,
        // and a tmux failure here must not take the
        // connection (and every other session on it) down.
        //
        // Targeted at the terminal's own WINDOW (through its
        // pane), never at the session: with tabs, a bare session
        // target names whichever window tmux last made current,
        // so a client resizing its agent view could reflow
        // somebody's tab (PLAN_M4.md item 3: resize goes per
        // window). The pane is paired with its session in the
        // target, so even a resolution that went stale in an
        // unexpected way cannot reach another session's window.
        if let Err(e) = sup
            .tmux
            .resize_window(&terminal.tmux_name, &terminal.pane, cols, rows)
            .await
        {
            warn!(session = %session_id, error = %e, "resize failed");
        }
    }
}

/// Spawned for the same reason `StopSession` is: a restart that
/// has to stop a live agent first runs that handler's whole kill
/// sweep — a grace period plus repeated `/proc` walks, real
/// wall-clock seconds — and awaiting it inline would stall every
/// other session's attach, input, and list behind this one
/// request. Safe for the same reasons too: this handler resolves the
/// session through the same lock-guarded map clone, and it never
/// touches `input_routes` (connection-local state a spawned task
/// must not see). Tracked and admitted exactly like the other
/// slow handlers — see `HANDLER_ADMISSION_PERMITS`.
async fn handle_restart_session(
    sup: &Arc<Supervisor>,
    tx: &mpsc::Sender<Frame>,
    tasks: &mut tokio::task::JoinSet<()>,
    req_id: u64,
    session_id: String,
    mode: RestartMode,
    stop_if_running: bool,
) {
    let sup2 = Arc::clone(sup);
    let tx = tx.clone();
    spawn_admitted(&sup.admission, tasks, async move {
        let sup = sup2;
        match sup
            .restart_session(&session_id, mode, stop_if_running)
            .await
        {
            Ok(session) => {
                send_reply(&tx, &ControlMsg::SessionRestarted { req_id, session }).await;
            }
            Err(e) => {
                send_reply(
                    &tx,
                    &ControlMsg::Error {
                        req_id,
                        message: format!("{e:#}"),
                        kind: error_kind(&e),
                    },
                )
                .await;
            }
        }
    })
    .await;
}

/// Every request-shape check a rename can make lives here, ahead
/// of the lookup and the write, and it is deliberately
/// `validate_create`'s explicit-title rule and nothing more
/// (PLAN_M5.md item 3): [`ensure_title_printable`] is the shared
/// refusal, and the cap is create's own [`CREATE_FIELD_CAP`],
/// applied to the title alone because it is the only field this
/// request carries into the reply that echoes it back. An empty
/// title is ACCEPTED, exactly as an explicit empty title on
/// create is — rename inventing a stricter rule than create
/// would be an asymmetry SPEC.md nowhere asks for.
///
/// Validation precedes the session lookup, so a malformed rename
/// of a session that does not exist reports what the CALLER can
/// fix. Everything after it is [`Supervisor::rename_session`]'s.
///
/// Spawned like the other slow handlers: the commit takes the
/// session's lifecycle claim, so it can legitimately wait out a
/// multi-second stop or delete, and the reply makes a tmux round
/// trip of its own for the live status. Awaiting either inline
/// would stall every other request on this connection. It touches
/// neither `input_routes` (connection-local) nor `attachments`.
///
/// ## One admission slot, moved along the phases
///
/// A rename is the one request whose halves have different
/// cancellation rules — the commit must outlive its connection,
/// the reply must not — so it cannot simply wrap the whole thing
/// in [`spawn_admitted`]. What it does instead is acquire ONE
/// owned permit here (this read loop waits for capacity, exactly
/// as `spawn_admitted` would) and then move it: into
/// [`Supervisor::rename_session`]'s supervisor-owned commit task,
/// which hands it back with a successful result, and from there
/// into the reply build.
///
/// Exactly one acquisition per request is not a tidiness point.
/// An earlier shape took a second permit for the reply phase via
/// `spawn_admitted`, which deadlocks: with
/// `HANDLER_ADMISSION_PERMITS` renames in flight, each holds a
/// slot while waiting for another, and no commit task exists yet
/// to release one. Moving a single slot along the phases makes
/// that unrepresentable, and keeps both phases bounded by the same
/// semaphore every other slow handler answers to.
///
/// The slot is held until the reply has been HANDED OVER, on both
/// the success and the failure path — the same parity every other
/// handler has, since each of their replies awaits the bounded
/// writer queue under admission. A refused rename that freed its
/// slot before sending would let a peer that never reads reclaim
/// capacity per request while its error replies piled up.
///
/// The task itself is therefore tracked in this connection's
/// `JoinSet` WITHOUT going through `spawn_admitted` — it is
/// carrying admission it already holds.
async fn handle_rename_session(
    sup: &Arc<Supervisor>,
    tx: &mpsc::Sender<Frame>,
    tasks: &mut tokio::task::JoinSet<()>,
    req_id: u64,
    session_id: String,
    title: String,
) {
    let refusal = if title.len() > CREATE_FIELD_CAP {
        Some(format!(
            "title is {} bytes, exceeding the {CREATE_FIELD_CAP}-byte limit",
            title.len()
        ))
    } else {
        ensure_title_printable(&title).err().map(|e| e.message)
    };
    if let Some(message) = refusal {
        send_reply(
            tx,
            &ControlMsg::Error {
                req_id,
                message,
                kind: ErrorKind::InvalidRequest,
            },
        )
        .await;
        return;
    }
    // THE request's admission slot — acquired here, in the read loop, so
    // that is what waits for capacity, and moved through both phases
    // afterwards. See this function's docs for why there is exactly one.
    let permit = Arc::clone(&sup.admission)
        .acquire_owned()
        .await
        .expect("admission semaphore is never closed");
    let sup2 = Arc::clone(sup);
    let tx = tx.clone();
    // Tracked like every other spawned handler (so this connection's
    // shutdown drains or reaps it) but NOT re-admitted: the slot it needs
    // is already in hand.
    tasks.spawn(async move {
        let sup = sup2;
        // The slot comes back with the outcome whatever it is, and is held
        // for the rest of this task — through the reply's tmux round trip
        // on the success path, and through the error reply's wait on the
        // bounded writer queue on the other. Dropping it at either reply's
        // send is what keeps a peer that never reads from reclaiming
        // capacity per request (see `Supervisor::rename_session`).
        let (outcome, _permit) = sup.rename_session(&session_id, title, permit).await;
        let renamed = match outcome {
            Ok(renamed) => renamed,
            Err(e) => {
                send_reply(
                    &tx,
                    &ControlMsg::Error {
                        req_id,
                        message: e.message,
                        kind: e.kind,
                    },
                )
                .await;
                return;
            }
        };
        // Built after the commit, and reported as a failure if it cannot be
        // built even though the rename itself has already landed — with a
        // message that says exactly that. The alternative is fabricating the
        // dynamic fields this reply promises to have probed. The caller's
        // next list corrects the title on its own, so the honest error costs
        // a poll interval rather than the rename.
        match session_info_now(&sup, &renamed).await {
            Ok(session) => send_reply(&tx, &ControlMsg::SessionRenamed { req_id, session }).await,
            Err(e) => {
                send_reply(
                    &tx,
                    &ControlMsg::Error {
                        req_id,
                        message: format!(
                            "session {} was renamed, but reading back its current state \
                             failed: {e:#}",
                            truncate_for_error(&session_id)
                        ),
                        kind: error_kind(&e),
                    },
                )
                .await;
            }
        }
    });
}

/// Spawned for the same reason the other slow handlers are: an
/// open is a handful of tmux round trips plus a liveness probe,
/// and awaiting it inline would stall every other request on
/// this connection. It touches neither `input_routes` (which is
/// connection-local) nor `attachments`.
async fn handle_open_tab(
    sup: &Arc<Supervisor>,
    tx: &mpsc::Sender<Frame>,
    tasks: &mut tokio::task::JoinSet<()>,
    req_id: u64,
    session_id: String,
) {
    let sup2 = Arc::clone(sup);
    let tx = tx.clone();
    spawn_admitted(&sup.admission, tasks, async move {
        let sup = sup2;
        match sup.open_tab(&session_id).await {
            Ok(tab) => send_reply(&tx, &ControlMsg::TabOpened { req_id, tab }).await,
            Err(e) => {
                send_reply(
                    &tx,
                    &ControlMsg::Error {
                        req_id,
                        message: e.message,
                        kind: e.kind,
                    },
                )
                .await;
            }
        }
    })
    .await;
}

/// Spawned like `StopSession`, and for the identical reason:
/// closing a tab runs the same multi-second process-tree
/// escalation on that tab's own tree.
async fn handle_close_tab(
    sup: &Arc<Supervisor>,
    tx: &mpsc::Sender<Frame>,
    tasks: &mut tokio::task::JoinSet<()>,
    req_id: u64,
    session_id: String,
    tab_id: String,
) {
    let sup2 = Arc::clone(sup);
    let tx = tx.clone();
    spawn_admitted(&sup.admission, tasks, async move {
        let sup = sup2;
        match sup.close_tab(&session_id, &tab_id).await {
            Ok(()) => send_reply(&tx, &ControlMsg::TabClosed { req_id }).await,
            Err(e) => {
                send_reply(
                    &tx,
                    &ControlMsg::Error {
                        req_id,
                        message: e.message,
                        kind: e.kind,
                    },
                )
                .await;
            }
        }
    })
    .await;
}

/// Every request-shape check lives here, ahead of anything
/// that touches disk — and note what is NOT among them: the
/// filename. A name is never grounds for refusing an upload
/// (SPEC.md rejects directories, never a file for what it is
/// called), so it is sanitized rather than validated, just
/// below.
///
/// "In use" means a LIVE transfer (or an attachment): a
/// finished transfer's route lingers as a tombstone, and the
/// channel it names is legitimately reusable — this begin
/// replaces it. Admission counts live transfers for the same
/// reason.
#[allow(clippy::too_many_arguments)]
async fn handle_begin_upload(
    sup: &Arc<Supervisor>,
    tx: &mpsc::Sender<Frame>,
    priority: &mpsc::Sender<Frame>,
    input_routes: &HashMap<u32, InputRoute>,
    upload_routes: &mut HashMap<u32, UploadRoute>,
    req_id: u64,
    session_id: String,
    channel: u32,
    filename: String,
    size: u64,
) {
    let live_uploads = upload_routes
        .values()
        .filter(|route| route.is_live())
        .count();
    let refusal = if channel == 0 {
        Some("attachment channel 0 is reserved".to_string())
    } else if input_routes.contains_key(&channel)
        || upload_routes
            .get(&channel)
            .is_some_and(UploadRoute::is_live)
    {
        Some(format!("attachment channel {channel} is already in use"))
    } else if live_uploads >= MAX_UPLOADS_PER_CONNECTION {
        // The admission bound `BeginUpload`'s contract leaves to
        // the receiver; see `MAX_UPLOADS_PER_CONNECTION`.
        Some(format!(
            "this connection already has {MAX_UPLOADS_PER_CONNECTION} uploads in flight, \
             which is the most it may have at once"
        ))
    } else {
        None
    };
    if let Some(message) = refusal {
        send_reply(
            tx,
            &ControlMsg::Error {
                req_id,
                message,
                kind: ErrorKind::InvalidRequest,
            },
        )
        .await;
        return;
    }
    // The unknown-session refusal is made HERE rather than left to
    // the transfer's task, even though the task must re-check it
    // anyway (under the session's lifecycle claim, which is what
    // makes the check race-tight against a concurrent delete —
    // see `stage_upload`). Doing it up front is what keeps a
    // refused begin from having created a route at all, so the
    // channel number stays immediately reusable: a route is
    // otherwise only reclaimed once the task it points at has been
    // observed to finish (`prune_finished_uploads`), which is
    // eventual rather than immediate.
    if !sup.sessions.lock().await.contains_key(&session_id) {
        send_reply(
            tx,
            &ControlMsg::Error {
                req_id,
                message: format!("no such session: {}", truncate_for_error(&session_id)),
                kind: ErrorKind::NotFound,
            },
        )
        .await;
        return;
    }

    let transfer = sup.next_transfer.fetch_add(1, Ordering::Relaxed);
    let (commands_tx, commands_rx) = mpsc::channel(UPLOAD_CHUNK_QUEUE);
    let (signals_tx, signals_rx) = mpsc::channel(UPLOAD_SIGNAL_QUEUE);
    let (finished_tx, finished_rx) = oneshot::channel();
    let outcome = UploadOutcome::live();
    // Registered BEFORE the task exists, so a `DeleteSession` that
    // starts in the same instant cannot conclude there is nothing
    // in flight while this transfer is already staging into the
    // directory that delete is about to detach (see
    // `UploadHandle::finished`).
    sup.uploads.lock().await.insert(
        transfer,
        UploadHandle {
            session: session_id.clone(),
            signals: signals_tx.clone(),
            finished: finished_rx,
        },
    );
    // A plain spawn, deliberately untracked by this connection's
    // `JoinSet`: a transfer is not a request handler, it outlives
    // the frame that started it by design, and every wait it
    // performs is bounded — for a client command by the progress
    // timeout, for a disk operation by
    // `SupervisorTimeouts::upload_disk_stage`, and for the
    // session's lifecycle claim by a cancellation it always
    // selects on (see `run_upload`).
    tokio::spawn(run_upload(
        Arc::clone(sup),
        priority.clone(),
        UploadRequest {
            req_id,
            session_id: session_id.clone(),
            channel,
            // Resolved HERE, once, and only the bounded result is
            // kept: the raw proposal is caller-controlled and can
            // be as large as a frame, and a name generated twice
            // would differ between the log and the disk (see
            // `UploadRequest::name`).
            name: crate::attachments::publish_name(&filename),
            size,
            transfer,
        },
        commands_rx,
        signals_rx,
        finished_tx,
        outcome.clone(),
    ));
    let admitted = transfer;
    upload_routes.insert(
        channel,
        UploadRoute {
            transfer,
            session: session_id,
            commands: commands_tx,
            signals: signals_tx,
            outcome,
            answered: false,
            admitted,
        },
    );
}

/// The route is deliberately NOT removed here — it stays until
/// its transfer has fully finished (holding the channel and the
/// admission slot, see `UploadRoute`), and then lingers as a
/// tombstone. What is consumed is the client's ONE terminal
/// message per transfer: `answered` is what makes a second
/// commit an immediate error rather than another publication
/// queued behind the first, and what stops a pipelined flood of
/// commits from being treated as if each freed a slot.
async fn handle_commit_upload(
    tx: &mpsc::Sender<Frame>,
    upload_routes: &mut HashMap<u32, UploadRoute>,
    req_id: u64,
    channel: u32,
) {
    let refusal = match upload_routes.get_mut(&channel) {
        Some(route) if route.is_live() && !route.answered => {
            route.answered = true;
            // `try_send`, not an awaiting send: this runs on the
            // connection's read loop, which must stay free to
            // process the very `AbortUpload` that could end a
            // transfer whose queue is full.
            match route.commands.try_send(UploadCommand::Commit { req_id }) {
                Ok(()) => None,
                Err(mpsc::error::TrySendError::Full(_)) => {
                    let _ = route.signals.try_send(UploadSignal {
                        reason: "this connection queued more upload data than it could \
                                 be credited for"
                            .to_string(),
                        tell_client: true,
                        session_gone: false,
                    });
                    Some((
                        format!(
                            "channel {channel} has more data queued than its credit \
                             window allows, so this commit could not be accepted"
                        ),
                        ErrorKind::InvalidRequest,
                    ))
                }
                // The transfer ended between the client sending
                // this and it arriving; its tombstone knows why.
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    Some(commit_without_upload(Some(&*route), channel))
                }
            }
        }
        Some(route) if route.is_live() => Some((
            format!("channel {channel}'s upload has already been committed or aborted"),
            ErrorKind::InvalidRequest,
        )),
        other => Some(commit_without_upload(other.map(|route| &*route), channel)),
    };
    if let Some((message, kind)) = refusal {
        send_reply(
            tx,
            &ControlMsg::Error {
                req_id,
                message,
                kind,
            },
        )
        .await;
    }
}

/// An out-of-band signal rather than a dropped route, and the
/// difference is exactly what the client asked for: dropping
/// the route would let the transfer keep writing every chunk
/// already queued ahead of the abort, while a signal is
/// selected on FIRST and stops it where it stands.
///
/// The route itself stays until the transfer finishes, so the
/// channel and its admission slot are released by the task
/// rather than by this message — a client that aborted and
/// immediately reused the channel would otherwise receive the
/// dying transfer's own events on its new one.
///
/// No reply and no error for a channel that never carried an
/// upload, or whose transfer already ended: `AbortUpload` is
/// fire-and-forget and idempotent for `Detach`'s reason — a
/// client tearing down (a cancelled drop, a closed view) must
/// never have to reason about who won a race against a
/// concurrent completion.
fn handle_abort_upload(upload_routes: &mut HashMap<u32, UploadRoute>, channel: u32) {
    if let Some(route) = upload_routes.get_mut(&channel)
        && route.is_live()
    {
        route.answered = true;
        let _ = route.signals.try_send(UploadSignal {
            reason: "the client abandoned this upload".to_string(),
            tell_client: false,
            session_gone: false,
        });
        debug!(
            session = %route.session, transfer = route.transfer, channel,
            "client abandoned an attachment upload"
        );
    }
}

/// Send one correlated refusal — the shape the profile handlers below
/// repeat four times over.
///
/// Not a general replacement for the inline `ControlMsg::Error` replies
/// elsewhere in this module: those mostly carry a message assembled from
/// several pieces at the site that knows them. This exists because the
/// profile verbs share both their error kinds and their phrasing rules, and
/// four hand-written copies of the same three-line reply is how two of them
/// end up saying different things about the same refusal.
async fn refuse(tx: &mpsc::Sender<Frame>, req_id: u64, kind: ErrorKind, message: String) {
    send_reply(
        tx,
        &ControlMsg::Error {
            req_id,
            message,
            kind,
        },
    )
    .await;
}

/// The whole catalog, in the id-ascending order the wire contract promises
/// (`ControlMsg::ProfileList`).
///
/// Unpaginated and unfiltered: the catalog is bounded
/// ([`MAX_PROFILES_PER_HOST`]) precisely so that one reply is always
/// enough, and a picker that showed only some of the options would not be a
/// picker.
async fn handle_list_profiles(sup: &Arc<Supervisor>, tx: &mpsc::Sender<Frame>, req_id: u64) {
    match sup.store.profiles().await {
        Ok(profiles) => send_reply(tx, &ControlMsg::ProfileList { req_id, profiles }).await,
        Err(e) => {
            refuse(
                tx,
                req_id,
                ErrorKind::Internal,
                format!("could not read the profile catalog: {e:#}"),
            )
            .await;
        }
    }
}

/// Define a new profile, minting its id.
///
/// The two bounds are enforced in different places on purpose, and the
/// split is the whole reason the catalog cannot become unlistable: this
/// request's own size is a property of the request
/// ([`validate_profile_fields`]), while the catalog's size is a property of
/// the catalog and can only be checked truthfully inside the transaction
/// that inserts (`SessionStore::create_profile`).
///
/// The per-record check runs HERE as well as in the store, and the
/// duplication is deliberate rather than redundant. The store's copy is
/// what makes the rule true for every caller; this one is what turns a
/// violation into an `InvalidRequest` carrying the exact message, instead
/// of the `Internal` that a store refusal would otherwise become.
async fn handle_create_profile(
    sup: &Arc<Supervisor>,
    tx: &mpsc::Sender<Frame>,
    req_id: u64,
    name: String,
    invocation: String,
    agent_kind: AgentKind,
    resume_template: Option<Vec<String>>,
) {
    if let Err(message) =
        validate_profile_fields(&name, &invocation, agent_kind, resume_template.as_deref())
    {
        refuse(tx, req_id, ErrorKind::InvalidRequest, message).await;
        return;
    }
    match sup
        .store
        .create_profile(name, invocation, agent_kind, resume_template)
        .await
    {
        Ok(ProfileCreation::Created(profile)) => {
            send_reply(tx, &ControlMsg::ProfileCreated { req_id, profile }).await;
        }
        Ok(ProfileCreation::CatalogFull) => {
            refuse(
                tx,
                req_id,
                ErrorKind::InvalidRequest,
                format!(
                    "this host already holds the maximum of {MAX_PROFILES_PER_HOST} profiles; \
                     delete one before creating another"
                ),
            )
            .await;
        }
        Err(e) => {
            refuse(
                tx,
                req_id,
                ErrorKind::Internal,
                format!("could not store the new profile: {e:#}"),
            )
            .await;
        }
    }
}

/// Replace a profile's definition wholesale, keyed by its id.
///
/// Sessions already created from this profile are untouched — not as a
/// courtesy this handler extends, but because nothing here can reach them:
/// their launch and resume snapshots are their own columns and their
/// source-profile snapshot keeps the name it recorded (SPEC.md's snapshot
/// rule). What a rename changes for them is only what a later reply DERIVES
/// about the profile's existence.
async fn handle_update_profile(
    sup: &Arc<Supervisor>,
    tx: &mpsc::Sender<Frame>,
    req_id: u64,
    profile: farhelm_proto::Profile,
) {
    if let Err(message) = validate_profile_fields(
        &profile.name,
        &profile.invocation,
        profile.agent_kind,
        profile.resume_template.as_deref(),
    ) {
        refuse(tx, req_id, ErrorKind::InvalidRequest, message).await;
        return;
    }
    let asked_for = truncate_for_error(&profile.id).into_owned();
    match sup.store.update_profile(profile).await {
        Ok(Some(profile)) => {
            send_reply(tx, &ControlMsg::ProfileUpdated { req_id, profile }).await;
        }
        Ok(None) => {
            refuse(
                tx,
                req_id,
                ErrorKind::NotFound,
                format!("no profile {asked_for} exists on this host"),
            )
            .await;
        }
        Err(e) => {
            refuse(
                tx,
                req_id,
                ErrorKind::Internal,
                format!("could not store the edited profile: {e:#}"),
            )
            .await;
        }
    }
}

/// Remove a profile from the catalog.
///
/// An unknown id is `NotFound` rather than a silent success, per
/// `ControlMsg::DeleteProfile`'s own docs: a client asking to delete
/// something that is not there is working from a stale catalog and should
/// be told so.
async fn handle_delete_profile(
    sup: &Arc<Supervisor>,
    tx: &mpsc::Sender<Frame>,
    req_id: u64,
    profile_id: String,
) {
    match sup.store.delete_profile(&profile_id).await {
        Ok(true) => send_reply(tx, &ControlMsg::ProfileDeleted { req_id }).await,
        Ok(false) => {
            refuse(
                tx,
                req_id,
                ErrorKind::NotFound,
                format!(
                    "no profile {} exists on this host",
                    truncate_for_error(&profile_id)
                ),
            )
            .await;
        }
        Err(e) => {
            refuse(
                tx,
                req_id,
                ErrorKind::Internal,
                format!("could not delete the profile: {e:#}"),
            )
            .await;
        }
    }
}

/// Dispatch one control message from a connected client.
///
/// Failures belonging to one request—bad cwd, a tmux hiccup, an unknown
/// session—become `ControlMsg::Error` replies here. They must not escape
/// into the connection loop: one connection carries every session the
/// helm is driving, so request-local failure cannot be allowed to detach
/// unrelated terminals.
///
/// `ctx.tx` doubles as this connection's identity: `same_channel` against
/// it is how the handlers tell "the connection that owns this attachment"
/// from any other, which channel ids alone cannot do.
///
/// Every connection-local borrow arrives in [`ConnectionCtx`] rather than
/// as its own parameter; each arm below still hands the per-message
/// handler only the fields its message may touch. See that struct's docs
/// for why the bundle stops at this function.
pub(crate) async fn handle_control(sup: &Arc<Supervisor>, msg: ControlMsg, ctx: ConnectionCtx<'_>) {
    match msg {
        ControlMsg::CreateSession {
            req_id,
            parent,
            cwd,
            invocation,
            profile_id,
            profile_name,
            title,
            cols,
            rows,
            intent_key,
            agent_kind,
            resume_template,
        } => {
            handle_create_session(
                sup,
                ctx.tx,
                req_id,
                parent,
                cwd,
                invocation,
                profile_id,
                profile_name,
                title,
                cols,
                rows,
                intent_key,
                CreateAdmission::Interactive,
                agent_kind,
                resume_template,
            )
            .await
        }
        ControlMsg::ListSessions {
            req_id,
            cursor,
            limit,
        } => handle_list_sessions(sup, ctx.tx, ctx.tasks, req_id, cursor, limit).await,
        ControlMsg::StopSession { req_id, session_id } => {
            handle_stop_session(sup, ctx.tx, ctx.tasks, req_id, session_id).await
        }
        ControlMsg::DeleteSession { req_id, session_id } => {
            handle_delete_session(sup, ctx.tx, ctx.tasks, req_id, session_id).await
        }
        ControlMsg::ArchiveSession { req_id, session_id } => {
            handle_archive_session(sup, ctx.tx, ctx.tasks, req_id, session_id).await
        }
        ControlMsg::Attach {
            req_id,
            session_id,
            channel,
            cols,
            rows,
            terminal: selector,
            lease,
            if_unowned,
        } => {
            handle_attach(
                sup,
                ctx.tx,
                ctx.input_routes,
                ctx.upload_routes,
                req_id,
                session_id,
                channel,
                cols,
                rows,
                selector,
                lease,
                if_unowned,
            )
            .await
        }
        ControlMsg::PauseOutput { channel } => {
            set_attachment_paused(sup, ctx.tx, channel, true).await;
        }
        ControlMsg::ResumeOutput { channel } => {
            set_attachment_paused(sup, ctx.tx, channel, false).await;
        }
        ControlMsg::Detach { channel } => {
            handle_detach(sup, ctx.tx, ctx.input_routes, channel).await
        }
        ControlMsg::Resize {
            session_id,
            channel,
            cols,
            rows,
        } => {
            handle_resize(
                sup,
                ctx.tx,
                ctx.input_routes,
                session_id,
                channel,
                cols,
                rows,
            )
            .await
        }
        ControlMsg::Hello { .. } => {
            // A second hello is a protocol violation; ignore rather than
            // kill the connection over it.
        }
        ControlMsg::RestartSession {
            req_id,
            session_id,
            mode,
            stop_if_running,
        } => {
            handle_restart_session(
                sup,
                ctx.tx,
                ctx.tasks,
                req_id,
                session_id,
                mode,
                stop_if_running,
            )
            .await
        }
        ControlMsg::RenameSession {
            req_id,
            session_id,
            title,
        } => handle_rename_session(sup, ctx.tx, ctx.tasks, req_id, session_id, title).await,
        ControlMsg::OpenTab { req_id, session_id } => {
            handle_open_tab(sup, ctx.tx, ctx.tasks, req_id, session_id).await
        }
        ControlMsg::CloseTab {
            req_id,
            session_id,
            tab_id,
        } => handle_close_tab(sup, ctx.tx, ctx.tasks, req_id, session_id, tab_id).await,
        ControlMsg::BeginUpload {
            req_id,
            session_id,
            channel,
            filename,
            size,
        } => {
            handle_begin_upload(
                sup,
                ctx.tx,
                ctx.priority,
                ctx.input_routes,
                ctx.upload_routes,
                req_id,
                session_id,
                channel,
                filename,
                size,
            )
            .await
        }
        ControlMsg::CommitUpload { req_id, channel } => {
            handle_commit_upload(ctx.tx, ctx.upload_routes, req_id, channel).await
        }
        ControlMsg::AbortUpload { channel } => handle_abort_upload(ctx.upload_routes, channel),
        ControlMsg::ListProfiles { req_id } => handle_list_profiles(sup, ctx.tx, req_id).await,
        ControlMsg::CreateProfile {
            req_id,
            name,
            invocation,
            agent_kind,
            resume_template,
        } => {
            handle_create_profile(
                sup,
                ctx.tx,
                req_id,
                name,
                invocation,
                agent_kind,
                resume_template,
            )
            .await
        }
        ControlMsg::UpdateProfile { req_id, profile } => {
            handle_update_profile(sup, ctx.tx, req_id, profile).await
        }
        ControlMsg::DeleteProfile { req_id, profile_id } => {
            handle_delete_profile(sup, ctx.tx, req_id, profile_id).await
        }
        // A REQUEST this authority may not make, and therefore one that
        // needs a real reply rather than the catch-all below. A helm holds
        // full authority over every session, but reporting a conversation
        // identity is not an authority question at all — it is a claim to
        // BE a particular session's agent, which only that agent's own
        // credential can support (`handle_restricted_control`). Falling
        // through to the catch-all would log a line and send nothing,
        // leaving a helm that sent this waiting on a reply that never
        // comes, and leaving a test with nothing to assert on.
        ControlMsg::ReportConversation { req_id, .. } => {
            send_reply(
                ctx.tx,
                &ControlMsg::Error {
                    req_id,
                    message: "only the session's own agent may report its conversation".to_string(),
                    kind: ErrorKind::Unauthorized,
                },
            )
            .await;
        }
        // Response/event messages arriving at the supervisor are peer
        // bugs; log and continue. `AgentResponse` is one of them here by
        // construction: the connection loop completes it against that
        // connection's own pending table before dispatch runs (see
        // `handle_connection`'s control arm), so it can only reach this
        // point if that interception is bypassed — which this arm reports
        // like any other message that should not have arrived.
        other => warn!(?other, "unexpected control message at supervisor"),
    }
}

/// Dispatch the deliberately narrow operation slice granted to a
/// session-authenticated peer.
///
/// Presence of hello auth selected this path before any request was read.
/// The peer can create a child and report its own conversation identity,
/// and nothing else; keeping that split outside ordinary dispatch means a
/// future handler cannot accidentally become available to spawn merely by
/// being added to the full-authority match.
///
/// The three admitted operations are admitted for three different reasons,
/// which is worth keeping in view when a fourth is proposed. A create is an
/// action the peer takes on the host, so it is authorized and serialized
/// like any other lifecycle operation. A report is the peer describing
/// ITSELF — something no other authority can do, which is exactly why the
/// full-authority dispatch refuses it. An agent request is neither: this
/// supervisor does not answer it at all, it carries it to the helm and
/// brings the answer back (see [`super::agent_relay`]), so what is being
/// authorized here is the right to ASK AS this session, not any authority
/// over what the answer contains.
pub(crate) async fn handle_restricted_control(
    sup: &Arc<Supervisor>,
    msg: ControlMsg,
    tx: &mpsc::Sender<Frame>,
    auth: &farhelm_proto::SessionAuth,
) {
    match msg {
        ControlMsg::CreateSession {
            req_id,
            parent,
            cwd,
            invocation,
            profile_id,
            profile_name,
            title,
            cols,
            rows,
            intent_key,
            agent_kind,
            resume_template,
        } => {
            // The hello check admits the connection; this check authorizes
            // each create. Holding the parent's lifecycle claim across the
            // create serializes it with deletion, so an authenticated peer
            // cannot outlive the session whose authority it is using.
            let _parent_lifecycle = sup.lifecycle_locks.claim(&auth.session_id).await;
            match sup
                .store
                .authenticates_session(&auth.session_id, &auth.token)
                .await
            {
                Ok(true) => {}
                Ok(false) => {
                    send_reply(
                        tx,
                        &ControlMsg::Error {
                            req_id,
                            message:
                                "the session credential is invalid or its session no longer exists"
                                    .to_string(),
                            kind: ErrorKind::Unauthorized,
                        },
                    )
                    .await;
                    return;
                }
                Err(error) => {
                    send_reply(
                        tx,
                        &ControlMsg::Error {
                            req_id,
                            message: format!(
                                "could not validate the session credential: {error:#}"
                            ),
                            kind: ErrorKind::Internal,
                        },
                    )
                    .await;
                    return;
                }
            }
            if parent
                .as_deref()
                .is_some_and(|parent| parent != auth.session_id)
            {
                send_reply(
                    tx,
                    &ControlMsg::Error {
                        req_id,
                        message: format!(
                            "a session-authenticated peer may name only itself ({}) as parent",
                            truncate_for_error(&auth.session_id)
                        ),
                        kind: ErrorKind::Unauthorized,
                    },
                )
                .await;
                return;
            }
            handle_create_session(
                sup,
                tx,
                req_id,
                parent,
                cwd,
                invocation,
                profile_id,
                profile_name,
                title,
                cols,
                rows,
                intent_key,
                CreateAdmission::Spawn,
                agent_kind,
                resume_template,
            )
            .await;
        }
        ControlMsg::ReportConversation {
            req_id,
            conversation,
            source,
        } => {
            // NO lifecycle claim, deliberately, and the contrast with the
            // `CreateSession` arm directly above is the point rather than
            // an oversight. `restart_session` holds a session's lifecycle
            // claim for the whole restart, and Claude's hook fires at the
            // replacement process's startup — inside that window. Queuing
            // this behind the claim would put the tail of a restart in
            // front of a hook that has a 2 s budget and no retry, and the
            // vendor shows a blown budget to the user as a hook error.
            // `Supervisor::report_conversation` carries the full argument,
            // including what the generation fence has to cover instead.
            match sup
                .store
                .authenticates_session(&auth.session_id, &auth.token)
                .await
            {
                Ok(true) => {}
                Ok(false) => {
                    send_reply(
                        tx,
                        &ControlMsg::Error {
                            req_id,
                            message:
                                "the session credential is invalid or its session no longer exists"
                                    .to_string(),
                            kind: ErrorKind::Unauthorized,
                        },
                    )
                    .await;
                    return;
                }
                Err(error) => {
                    send_reply(
                        tx,
                        &ControlMsg::Error {
                            req_id,
                            message: format!(
                                "could not validate the session credential: {error:#}"
                            ),
                            kind: ErrorKind::Internal,
                        },
                    )
                    .await;
                    return;
                }
            }
            // The length bound is this handler's own, not one inherited
            // from the scan's parser: a post-handshake frame is capped
            // only by `MAX_FRAME_LEN`, and the hello-only caps in
            // `farhelm_proto::io` never applied here. Same job
            // `MAX_LEASE_BYTES` does for a lease name.
            if conversation.len() > MAX_CONVERSATION_BYTES
                || !crate::agent_kind::is_plausible_conversation_id(&conversation)
            {
                warn!(
                    session = %auth.session_id, bytes = conversation.len(),
                    "refused a reported conversation identity this build will not store"
                );
                send_reply(
                    tx,
                    &ControlMsg::Error {
                        req_id,
                        message: format!(
                            "not a conversation identity this build will store: {}",
                            truncate_for_error(&conversation)
                        ),
                        kind: ErrorKind::InvalidRequest,
                    },
                )
                .await;
                return;
            }
            // Bounded and stripped of control characters HERE, at the
            // doorway, so nothing downstream has to remember that this
            // field is attacker-chosen: `report_conversation` puts it in
            // several log lines and would otherwise be the place a
            // credential-holding process could write megabytes of
            // newline-laced text into the supervisor's log.
            let source = sanitized_source(&source);
            let reply = match sup
                .report_conversation(&auth.session_id, conversation, source)
                .await
            {
                Ok(()) => ControlMsg::ConversationReported { req_id },
                Err(e) => ControlMsg::Error {
                    req_id,
                    message: e.message,
                    kind: e.kind,
                },
            };
            send_reply(tx, &reply).await;
        }
        ControlMsg::AgentRequest {
            req_id,
            session_id,
            request,
        } => {
            // NO lifecycle claim, for `ReportConversation`'s reason and
            // one more: the two verbs this version carries are read-only
            // questions answered by another process entirely, so there is
            // nothing here to serialize against a delete — and holding a
            // claim across a round trip to the helm would put an unrelated
            // restart or stop behind a peer that may be waiting out the
            // full upcall budget.
            //
            // Every refusal below is an `AgentResponse` rather than a bare
            // `Error`: this exchange has two hops and refusals from both,
            // and one reply shape means the asking CLI decodes exactly one
            // thing (see `ControlMsg::AgentRequest`'s docs).
            let outcome = match sup
                .store
                .authenticates_session(&auth.session_id, &auth.token)
                .await
            {
                Ok(true) if session_id == auth.session_id => {
                    sup.relay_agent_request(session_id, request).await
                }
                // A credential for one session is not authority to ask
                // questions as another. The check is here rather than at
                // the far end because the helm never sees the credential:
                // by the time the request reaches it, `session_id` is the
                // only claim about who is asking, and it has to already be
                // true.
                Ok(true) => farhelm_proto::AgentOutcome::Err {
                    kind: ErrorKind::Unauthorized,
                    message: format!(
                        "a session-authenticated peer may ask only as itself ({})",
                        truncate_for_error(&auth.session_id)
                    ),
                },
                Ok(false) => farhelm_proto::AgentOutcome::Err {
                    kind: ErrorKind::Unauthorized,
                    message: "the session credential is invalid or its session no longer exists"
                        .to_string(),
                },
                Err(error) => farhelm_proto::AgentOutcome::Err {
                    kind: ErrorKind::Internal,
                    message: format!("could not validate the session credential: {error:#}"),
                },
            };
            send_reply(tx, &ControlMsg::AgentResponse { req_id, outcome }).await;
        }
        other => {
            send_reply(
                tx,
                &ControlMsg::Error {
                    req_id: other.request_req_id().unwrap_or(0),
                    message: "a session-authenticated peer may only create sessions, report its \
                              conversation, and ask the helm about the fleet"
                        .to_string(),
                    kind: ErrorKind::Unauthorized,
                },
            )
            .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::capture::{CaptureState, FirstInput};
    use super::super::connection::CONNECTION_WRITER_QUEUE;
    use super::super::core::tests::{StateDir, dummy_exe, entry_with, no_uploads};
    use super::super::core::{ArchiveStage, SupervisorSeams};
    use super::super::listing::encode_list_cursor;
    use super::super::terminals::Terminal;
    use super::*;
    use crate::agent_kind::IntegrationSnapshot;
    use farhelm_proto::{PROFILE_FIELD_CAP, RestartOffer, SessionStatus};

    /// Seed the durable half of a parent, which is the authority source a
    /// restricted connection must revalidate before every create.
    async fn authenticated_parent(
        sup: &Supervisor,
        cwd: &std::path::Path,
        id: &str,
    ) -> farhelm_proto::SessionAuth {
        let profile = sup
            .store
            .profiles()
            .await
            .expect("read starter profiles")
            .into_iter()
            .next()
            .expect("a fresh supervisor seeds starter profiles");
        let claimed = sup
            .store
            .insert_session(
                crate::store::StoredSession {
                    conversation_source: None,
                    id: id.to_string(),
                    parent: None,
                    archived: false,
                    title: id.to_string(),
                    created_at: crate::store::now_unix(),
                    last_activity_at: crate::store::now_unix(),
                    creation_seq: 0,
                    cwd: cwd.to_string_lossy().into_owned(),
                    invocation: "agent".to_string(),
                    tmux_name: format!("fh-{id}"),
                    pane: String::new(),
                    outcome: LastOutcome::Launching,
                    agent_kind: AgentKind::Generic,
                    resume_template: None,
                    canonical_cwd: None,
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                    source_profile: Some(crate::store::ProfileSnapshot {
                        id: profile.id,
                        name: profile.name,
                    }),
                },
                None,
            )
            .await
            .expect("seed authenticated parent");
        let crate::store::Claimed::Ours { session_token, .. } = claimed else {
            panic!("an unkeyed parent insert cannot be taken");
        };
        farhelm_proto::SessionAuth {
            session_id: id.to_string(),
            token: session_token,
        }
    }
    use std::time::Duration;

    /// The pre-storage create refusals, driven through the
    /// real dispatcher against a real supervisor rather than through
    /// `create_mode` alone: what matters is not only that the resolver
    /// returns an error, but that the error reaches the CALLER as a
    /// correlated `InvalidRequest` and that NOTHING was created on the way.
    ///
    /// The table covers every shape refusal: naming no selector, naming
    /// multiple selectors, and pairing either profile selector with a
    /// raw-mode override. The override rows are the subtle ones — a client
    /// that "helpfully" forwards a default `agent_kind` alongside a profile
    /// selection has written a request whose meaning nobody can defend,
    /// and the refusal stops an invented precedence rule at launch time.
    /// It also covers profile-name combinations, whose exact resolution is
    /// meaningful only after this shape boundary has admitted them.
    ///
    /// Every row carries an INTENT KEY, and the keys are asserted unclaimed
    /// afterwards. That is the half a refusal test usually forgets: a
    /// request refused for its shape must not spend the client's key on the
    /// way out, or the corrected retry — the entire point of an idempotency
    /// key — is answered with a stale `Conflict` forever. Checked with
    /// `reservation(key)`, not with "the table is empty": a claim that was
    /// wrongly settled is still a claim, and only asking for the key by
    /// name can tell the difference.
    #[tokio::test]
    async fn a_pre_storage_create_refusal_leaves_no_session_or_reservation() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        for (req_id, invocation, profile_id, profile_name, agent_kind, resume_template, expected) in [
            (
                1u64,
                Some("agent".to_string()),
                Some("prof-1".to_string()),
                None,
                None,
                None,
                "more than one",
            ),
            (2, None, None, None, None, None, "named none"),
            (
                3,
                None,
                Some("prof-1".to_string()),
                None,
                Some(AgentKind::Claude),
                None,
                "the profile supplies both",
            ),
            (
                4,
                None,
                Some("prof-1".to_string()),
                None,
                None,
                Some(vec!["claude".to_string(), "{conversation}".to_string()]),
                "the profile supplies both",
            ),
            (
                5,
                Some("agent".to_string()),
                None,
                Some("Claude Code".to_string()),
                None,
                None,
                "more than one",
            ),
            (
                6,
                None,
                Some("prof-1".to_string()),
                Some("Claude Code".to_string()),
                None,
                None,
                "more than one",
            ),
            (
                7,
                None,
                None,
                Some("Claude Code".to_string()),
                Some(AgentKind::Claude),
                None,
                "the profile supplies both",
            ),
            (
                8,
                None,
                None,
                Some("Claude Code".to_string()),
                None,
                Some(vec!["claude".to_string(), "{conversation}".to_string()]),
                "the profile supplies both",
            ),
        ] {
            handle_control(
                &sup,
                ControlMsg::CreateSession {
                    req_id,
                    parent: None,
                    profile_name,
                    // A real, usable directory, so nothing about the
                    // refusal can be attributed to the cwd check further
                    // in — the mode is what is under test.
                    cwd: state.path().to_string_lossy().to_string(),
                    invocation,
                    profile_id,
                    title: None,
                    cols: 80,
                    rows: 24,
                    intent_key: Some(format!("ambiguous-{req_id}")),
                    agent_kind,
                    resume_template,
                },
                ConnectionCtx {
                    tx: &tx,
                    priority: &tx,
                    input_routes: &mut input_routes,
                    upload_routes: &mut no_uploads(),
                    tasks: &mut tasks,
                },
            )
            .await;
            let frame = rx.try_recv().expect("a reply must have been sent");
            let decoded: ControlMsg = serde_json::from_slice(&frame.body).expect("decode");
            let ControlMsg::Error {
                req_id: got,
                kind,
                message,
            } = decoded
            else {
                panic!("a pre-storage create refusal must return an error: {decoded:?}");
            };
            assert_eq!(got, req_id);
            assert_eq!(
                kind,
                ErrorKind::InvalidRequest,
                "an ambiguous request is the caller's mistake, not a server fault"
            );
            assert!(
                message.contains(expected),
                "the refusal must say what was wrong: {message}"
            );
            let key = format!("ambiguous-{req_id}");
            assert!(
                sup.store
                    .reservation(&key)
                    .await
                    .expect("reservation lookup")
                    .is_none(),
                "a pre-storage refusal must leave its intent key unspent so a corrected or \
                 newly-supported retry can use it: {key}"
            );
        }
        assert!(
            sup.sessions.lock().await.is_empty(),
            "a refused request must create nothing"
        );
        assert!(
            sup.store.load_all().await.expect("load").is_empty(),
            "and must not have reached the store either"
        );
    }

    /// `profile_id` is charged against `CREATE_FIELD_CAP` like every other
    /// caller-supplied field, and the refusal that names it does not echo
    /// it whole.
    ///
    /// Two separate leaks, one request. The cap is what stops an 8 MiB
    /// "profile id" from reaching the fingerprint — a permanent,
    /// never-pruned row — through a mode the raw path's own cap does not
    /// cover. The truncation is what stops the REFUSAL from repeating that
    /// text back through the helm into an HTTP body: an error that quotes
    /// its input verbatim turns a rejected request into an amplifier, and
    /// this one is reachable by anyone who can reach the API.
    ///
    /// Both boundaries are exercised, not just the over-cap side: an id
    /// exactly AT the cap must get through to the next check, or the cap
    /// would be an off-by-one that quietly rejects legitimate requests.
    #[tokio::test]
    async fn an_oversized_profile_id_is_capped_and_never_echoed_whole() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        let cwd = state.path().to_string_lossy().to_string();

        let create_with_profile = |profile_id: String, req_id: u64| ControlMsg::CreateSession {
            req_id,
            parent: None,
            profile_name: None,
            cwd: cwd.clone(),
            invocation: None,
            profile_id: Some(profile_id),
            title: None,
            cols: 80,
            rows: 24,
            intent_key: None,
            agent_kind: None,
            resume_template: None,
        };
        let reply = |rx: &mut mpsc::Receiver<Frame>| -> (ErrorKind, String) {
            let frame = rx.try_recv().expect("a reply must have been sent");
            let decoded: ControlMsg = serde_json::from_slice(&frame.body).expect("decode");
            match decoded {
                ControlMsg::Error { kind, message, .. } => (kind, message),
                other => panic!("expected an Error reply, got {other:?}"),
            }
        };
        // Over the cap: refused for SIZE, before the mode's own refusal.
        handle_control(
            &sup,
            create_with_profile("p".repeat(CREATE_FIELD_CAP + 1 - cwd.len()), 1),
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;
        let (kind, message) = reply(&mut rx);
        assert_eq!(kind, ErrorKind::InvalidRequest);
        assert!(
            message.contains(&CREATE_FIELD_CAP.to_string()),
            "the refusal must name the limit that was exceeded: {message}"
        );
        assert!(
            message.len() < CREATE_FIELD_CAP,
            "a size refusal must not echo the oversized input back: {} bytes",
            message.len()
        );

        // At the cap: accepted by the size check, so it reaches the catalog
        // lookup — whose own refusal must still not echo an id this long.
        handle_control(
            &sup,
            create_with_profile("p".repeat(CREATE_FIELD_CAP - cwd.len()), 2),
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;
        let (kind, message) = reply(&mut rx);
        assert_eq!(kind, ErrorKind::NotFound);
        assert!(
            message.contains("no profile"),
            "an id exactly at the cap must pass the size check and reach the catalog lookup: \
             {message}"
        );
        assert!(
            message.len() < 4096,
            "the refusal must truncate the id it names rather than repeating it: {} bytes",
            message.len()
        );
    }

    /// `parent` is part of the permanent fingerprint and therefore part
    /// of `CREATE_FIELD_CAP`, including when its UTF-8 byte length differs
    /// from its character count. The exact boundary reaches catalog
    /// resolution; one byte more is refused before its key is claimed.
    #[tokio::test]
    async fn parent_bytes_are_capped_before_the_intent_key_is_spent() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        let cwd = state.path().to_string_lossy().to_string();
        let profile_name = "No Such Profile";
        let parent_bytes = CREATE_FIELD_CAP - cwd.len() - profile_name.len();
        let mut parent = format!(
            "{}{}",
            "😀".repeat(parent_bytes / "😀".len()),
            "x".repeat(parent_bytes % "😀".len())
        );
        assert_eq!(
            parent.len() + cwd.len() + profile_name.len(),
            CREATE_FIELD_CAP
        );

        for (req_id, key, value, expected, claimed) in [
            (
                1u64,
                "parent-at-cap",
                parent.clone(),
                "no profile named",
                true,
            ),
            {
                parent.push('x');
                (2, "parent-over-cap", parent, "parent, cwd", false)
            },
        ] {
            handle_control(
                &sup,
                ControlMsg::CreateSession {
                    req_id,
                    parent: Some(value),
                    cwd: cwd.clone(),
                    invocation: None,
                    profile_id: None,
                    profile_name: Some(profile_name.to_string()),
                    title: None,
                    cols: 80,
                    rows: 24,
                    intent_key: Some(key.to_string()),
                    agent_kind: None,
                    resume_template: None,
                },
                ConnectionCtx {
                    tx: &tx,
                    priority: &tx,
                    input_routes: &mut input_routes,
                    upload_routes: &mut no_uploads(),
                    tasks: &mut tasks,
                },
            )
            .await;
            let frame = rx.try_recv().expect("a refusal must be sent");
            let ControlMsg::Error { kind, message, .. } =
                serde_json::from_slice(&frame.body).expect("decode")
            else {
                panic!("the boundary request must be refused");
            };
            assert_eq!(kind, ErrorKind::InvalidRequest);
            assert!(
                message.contains(expected),
                "the boundary must fail at the expected next check: {message}"
            );
            assert_eq!(
                sup.store
                    .reservation(key)
                    .await
                    .expect("reservation lookup")
                    .is_some(),
                claimed,
                "only a request that reached catalog resolution spends {key}"
            );
        }
        assert!(sup.store.load_all().await.expect("load").is_empty());
    }

    /// Profile-name selection pays the same byte budget as profile-id and
    /// raw selection. The at-cap value reaches catalog resolution and
    /// records that durable answer; adding one byte is rejected before its
    /// key can be claimed.
    #[tokio::test]
    async fn profile_name_bytes_are_capped_before_the_intent_key_is_spent() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        let cwd = state.path().to_string_lossy().to_string();
        let profile_bytes = CREATE_FIELD_CAP - cwd.len();
        let mut profile_name = format!(
            "{}{}",
            "😀".repeat(profile_bytes / "😀".len()),
            "x".repeat(profile_bytes % "😀".len())
        );
        assert_eq!(profile_name.len() + cwd.len(), CREATE_FIELD_CAP);

        for (req_id, key, value, expected, claimed) in [
            (
                1u64,
                "profile-name-at-cap",
                profile_name.clone(),
                "no profile named",
                true,
            ),
            {
                profile_name.push('x');
                (
                    2,
                    "profile-name-over-cap",
                    profile_name,
                    "exceeding the",
                    false,
                )
            },
        ] {
            handle_control(
                &sup,
                ControlMsg::CreateSession {
                    req_id,
                    parent: None,
                    cwd: cwd.clone(),
                    invocation: None,
                    profile_id: None,
                    profile_name: Some(value),
                    title: None,
                    cols: 80,
                    rows: 24,
                    intent_key: Some(key.to_string()),
                    agent_kind: None,
                    resume_template: None,
                },
                ConnectionCtx {
                    tx: &tx,
                    priority: &tx,
                    input_routes: &mut input_routes,
                    upload_routes: &mut no_uploads(),
                    tasks: &mut tasks,
                },
            )
            .await;
            let frame = rx.try_recv().expect("a refusal must be sent");
            let ControlMsg::Error { kind, message, .. } =
                serde_json::from_slice(&frame.body).expect("decode")
            else {
                panic!("the boundary request must be refused");
            };
            assert_eq!(kind, ErrorKind::InvalidRequest);
            assert!(
                message.contains(expected),
                "the boundary must fail at the expected check: {message}"
            );
            assert_eq!(
                sup.store
                    .reservation(key)
                    .await
                    .expect("reservation lookup")
                    .is_some(),
                claimed,
                "only a request that reached catalog resolution spends {key}"
            );
        }
        assert!(sup.store.load_all().await.expect("load").is_empty());
    }

    /// Archive is a real lifecycle request: even a missing row is answered
    /// by the archive handler with the ordinary correlated not-found shape.
    #[tokio::test]
    async fn archive_of_a_missing_session_is_answered_by_the_real_handler() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();

        handle_control(
            &sup,
            ControlMsg::ArchiveSession {
                req_id: 41,
                session_id: "session-1".to_string(),
            },
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;
        tasks
            .join_next()
            .await
            .expect("archive handler was spawned")
            .expect("archive handler completed");

        let frame = rx.try_recv().expect("archive must receive a reply");
        let ControlMsg::Error {
            req_id,
            kind,
            message,
        } = serde_json::from_slice(&frame.body).expect("decode")
        else {
            panic!("missing archive target must return a correlated error");
        };
        assert_eq!(req_id, 41);
        assert_eq!(kind, ErrorKind::NotFound);
        assert!(message.contains("no such session") && message.contains("session-1"));
    }

    /// Repeating archive against the state already requested does no
    /// teardown and returns the current archived row.
    #[tokio::test]
    async fn archive_of_an_archived_session_is_an_idempotent_success() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .unwrap();
        let mut entry = entry_with(
            None,
            LastOutcome::Exited {
                exit_code: None,
                annotation: Some(farhelm_proto::STOP_ANNOTATION.to_string()),
            },
        );
        entry.info.archived = true;
        entry.info.annotation = Some(farhelm_proto::STOP_ANNOTATION.to_string());
        sup.sessions
            .lock()
            .await
            .insert("session-1".to_string(), Arc::new(entry));

        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        handle_control(
            &sup,
            ControlMsg::ArchiveSession {
                req_id: 42,
                session_id: "session-1".to_string(),
            },
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;
        tasks.join_next().await.unwrap().unwrap();

        let reply: ControlMsg = serde_json::from_slice(&rx.try_recv().unwrap().body).unwrap();
        let ControlMsg::SessionArchived { req_id, session } = reply else {
            panic!("double archive must return the archived row");
        };
        assert_eq!(req_id, 42);
        assert!(session.archived);
        assert!(session.tabs.is_empty());
        assert_eq!(
            session.annotation.as_deref(),
            Some(farhelm_proto::STOP_ANNOTATION)
        );
    }

    /// Aborting the connection-owned reply waiter cannot cancel archive's
    /// supervisor-owned mutation or release its lifecycle claim early.
    #[tokio::test]
    async fn archive_survives_connection_task_cancellation() {
        let state = StateDir::new();
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let gate_entered = Arc::clone(&entered);
        let gate_release = Arc::clone(&release);
        let sup = Supervisor::new_with_seams(
            state.path(),
            dummy_exe(),
            super::super::core::SupervisorTimeouts::default(),
            SupervisorSeams {
                archive_gate: Some(Arc::new(move |stage| {
                    let entered = Arc::clone(&gate_entered);
                    let release = Arc::clone(&gate_release);
                    Box::pin(async move {
                        if stage == ArchiveStage::Sweep {
                            entered.notify_one();
                            release.notified().await;
                        }
                        Ok(())
                    })
                })),
                ..SupervisorSeams::default()
            },
        )
        .await
        .unwrap();
        sup.store
            .insert_session(
                crate::store::StoredSession {
                    conversation_source: None,
                    id: "s1".to_string(),
                    parent: None,
                    archived: false,
                    title: "t".to_string(),
                    created_at: 1_700_000_000,
                    last_activity_at: 1_700_000_000,
                    creation_seq: 0,
                    cwd: "/tmp".to_string(),
                    invocation: "agent".to_string(),
                    tmux_name: "fh-s1".to_string(),
                    pane: String::new(),
                    outcome: LastOutcome::Running,
                    agent_kind: AgentKind::Generic,
                    resume_template: None,
                    canonical_cwd: None,
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                    source_profile: None,
                },
                None,
            )
            .await
            .unwrap();
        sup.sessions
            .lock()
            .await
            .insert("s1".to_string(), fake_entry("s1", 1_700_000_000));
        sup.tmux
            .create_session(
                "fh-s1",
                "/",
                80,
                24,
                &[],
                &["sleep".to_string(), "120".to_string()],
            )
            .await
            .expect("plant a durable-name tmux husk without an entry terminal");

        let (tx, _rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        handle_control(
            &sup,
            ControlMsg::ArchiveSession {
                req_id: 44,
                session_id: "s1".to_string(),
            },
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;
        tokio::time::timeout(Duration::from_secs(5), entered.notified())
            .await
            .expect("archive reached the blocked supervisor-owned task");
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
        release.notify_waiters();

        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            if sup.store.session("s1").await.unwrap().unwrap().archived {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "archive was cancelled with its reply waiter"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(
            !sup.tmux.has_session("fh-s1").await.unwrap(),
            "archive must kill the durable tmux name even when SessionEntry has no Terminal"
        );
    }

    /// Archive keeps the lifecycle claim for its complete teardown, so a
    /// restart queues behind it and observes the retained archived row.
    ///
    /// The archive seam parks the real handler before its first destructive
    /// step. If the handler omitted the claim or released it around the slow
    /// teardown, restart could act on the still-live pre-archive entry and
    /// report a competing mutation while archive was still in flight.
    #[tokio::test]
    async fn restart_waits_for_a_blocked_archive_lifecycle_claim() {
        let (_state, sup, entered, release) = blocked_archive_supervisor().await;
        let (mut archive_tasks, mut archive_rx) = dispatch_for_test(
            &sup,
            ControlMsg::ArchiveSession {
                req_id: 45,
                session_id: "s1".to_string(),
            },
        )
        .await;
        entered.notified().await;
        assert!(sup.lifecycle_locks.claimed_for_test("s1"));

        let (mut restart_tasks, mut restart_rx) = dispatch_for_test(
            &sup,
            ControlMsg::RestartSession {
                req_id: 46,
                session_id: "s1".to_string(),
                mode: RestartMode::Fresh,
                stop_if_running: false,
            },
        )
        .await;
        tokio::task::yield_now().await;
        assert!(
            matches!(
                restart_rx.try_recv(),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty)
            ),
            "restart must not answer while archive holds the lifecycle claim"
        );

        release.notify_one();
        archive_tasks.join_next().await.unwrap().unwrap();
        let archived: ControlMsg =
            serde_json::from_slice(&archive_rx.try_recv().unwrap().body).unwrap();
        assert!(matches!(
            archived,
            ControlMsg::SessionArchived { req_id: 45, .. }
        ));

        restart_tasks.join_next().await.unwrap().unwrap();
        let restarted: ControlMsg =
            serde_json::from_slice(&restart_rx.try_recv().unwrap().body).unwrap();
        let ControlMsg::SessionRestarted {
            req_id: 46,
            session,
        } = restarted
        else {
            panic!("restart after archive must succeed, got {restarted:?}");
        };
        assert!(!session.archived);
        assert!(
            !sup.store.session("s1").await.unwrap().unwrap().archived,
            "the serialized restart must clear the durable archive state"
        );
    }

    /// Archive keeps the lifecycle claim for its complete teardown, so a
    /// delete cannot remove the row from underneath archive publication.
    ///
    /// Once the blocked archive finishes, delete sees the archived entry
    /// and removes it normally. Without the shared claim, either handler can
    /// make the other's stale entry authoritative and resurrect a row.
    #[tokio::test]
    async fn delete_waits_for_a_blocked_archive_lifecycle_claim() {
        let (_state, sup, entered, release) = blocked_archive_supervisor().await;
        let (mut archive_tasks, mut archive_rx) = dispatch_for_test(
            &sup,
            ControlMsg::ArchiveSession {
                req_id: 47,
                session_id: "s1".to_string(),
            },
        )
        .await;
        entered.notified().await;
        assert!(sup.lifecycle_locks.claimed_for_test("s1"));

        let (mut delete_tasks, mut delete_rx) = dispatch_for_test(
            &sup,
            ControlMsg::DeleteSession {
                req_id: 48,
                session_id: "s1".to_string(),
            },
        )
        .await;
        tokio::task::yield_now().await;
        assert!(
            matches!(
                delete_rx.try_recv(),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty)
            ),
            "delete must not answer while archive holds the lifecycle claim"
        );

        release.notify_one();
        archive_tasks.join_next().await.unwrap().unwrap();
        let archived: ControlMsg =
            serde_json::from_slice(&archive_rx.try_recv().unwrap().body).unwrap();
        assert!(matches!(
            archived,
            ControlMsg::SessionArchived { req_id: 47, .. }
        ));

        delete_tasks.join_next().await.unwrap().unwrap();
        let deleted: ControlMsg =
            serde_json::from_slice(&delete_rx.try_recv().unwrap().body).unwrap();
        assert!(matches!(deleted, ControlMsg::SessionDeleted { req_id: 48 }));
        assert!(sup.store.session("s1").await.unwrap().is_none());
        assert!(!sup.sessions.lock().await.contains_key("s1"));
    }

    /// Build one terminal-less live row and park archive immediately before
    /// its process sweep. The durable tmux name makes teardown real while
    /// avoiding an attachment or pane fixture unrelated to claim ordering.
    async fn blocked_archive_supervisor() -> (
        StateDir,
        Arc<Supervisor>,
        Arc<tokio::sync::Notify>,
        Arc<tokio::sync::Notify>,
    ) {
        let state = StateDir::new();
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let gate_entered = Arc::clone(&entered);
        let gate_release = Arc::clone(&release);
        let sup = Supervisor::new_with_seams(
            state.path(),
            dummy_exe(),
            super::super::core::SupervisorTimeouts::default(),
            SupervisorSeams {
                archive_gate: Some(Arc::new(move |stage| {
                    let entered = Arc::clone(&gate_entered);
                    let release = Arc::clone(&gate_release);
                    Box::pin(async move {
                        if stage == ArchiveStage::Sweep {
                            entered.notify_one();
                            release.notified().await;
                        }
                        Ok(())
                    })
                })),
                ..SupervisorSeams::default()
            },
        )
        .await
        .unwrap();
        sup.store
            .insert_session(
                crate::store::StoredSession {
                    conversation_source: None,
                    id: "s1".to_string(),
                    parent: None,
                    archived: false,
                    title: "t".to_string(),
                    created_at: 1_700_000_000,
                    last_activity_at: 1_700_000_000,
                    creation_seq: 0,
                    cwd: "/tmp".to_string(),
                    invocation: "agent".to_string(),
                    tmux_name: "fh-s1".to_string(),
                    pane: String::new(),
                    outcome: LastOutcome::Running,
                    agent_kind: AgentKind::Generic,
                    resume_template: None,
                    canonical_cwd: None,
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                    source_profile: None,
                },
                None,
            )
            .await
            .unwrap();
        sup.sessions
            .lock()
            .await
            .insert("s1".to_string(), fake_entry("s1", 1_700_000_000));
        sup.tmux
            .create_session(
                "fh-s1",
                "/",
                80,
                24,
                &[],
                &["sleep".to_string(), "120".to_string()],
            )
            .await
            .expect("plant the durable tmux session archive will remove");
        (state, sup, entered, release)
    }

    /// Dispatch one request exactly as a connection would, retaining its
    /// task set and reply queue so race tests can observe the in-flight
    /// interval before joining it.
    async fn dispatch_for_test(
        sup: &Arc<Supervisor>,
        message: ControlMsg,
    ) -> (tokio::task::JoinSet<()>, mpsc::Receiver<Frame>) {
        let (tx, rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        handle_control(
            sup,
            message,
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;
        (tasks, rx)
    }

    /// An archived session still exists, so attach names that state as an
    /// invalid request instead of misreporting the row as not found.
    #[tokio::test]
    async fn attach_to_an_archived_session_is_refused_by_state() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .unwrap();
        let mut entry = entry_with(
            None,
            LastOutcome::Exited {
                exit_code: None,
                annotation: Some(farhelm_proto::STOP_ANNOTATION.to_string()),
            },
        );
        entry.info.archived = true;
        sup.sessions
            .lock()
            .await
            .insert("session-1".to_string(), Arc::new(entry));

        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        handle_control(
            &sup,
            ControlMsg::Attach {
                req_id: 43,
                session_id: "session-1".to_string(),
                channel: 1,
                cols: 80,
                rows: 24,
                terminal: TerminalSelector::Agent,
                lease: "archive-test".to_string(),
                if_unowned: false,
            },
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;

        let reply: ControlMsg = serde_json::from_slice(&rx.try_recv().unwrap().body).unwrap();
        let ControlMsg::Error { kind, message, .. } = reply else {
            panic!("archived attach must be refused");
        };
        assert_eq!(kind, ErrorKind::InvalidRequest);
        assert!(message.contains("archived") && message.contains("restart"));
    }

    /// Every profile CRUD verb, driven through the real dispatcher against
    /// a real catalog (PLAN_M6_75.md item 4): list, create, update, delete,
    /// each answered with its OWN reply variant and correlated to its
    /// `req_id`.
    ///
    /// Two things at once, and both are worth the one test. The obvious
    /// one is the round trip — what a create stores is what a later list
    /// returns. The other is that no request is ever met with SILENCE: a
    /// request carrying a `req_id` that the dispatcher merely logs leaves
    /// its caller waiting forever, and a hung request looks nothing like a
    /// failure from either end. Correlation is asserted on every reply for
    /// that reason, not only on the interesting ones.
    #[tokio::test]
    async fn every_profile_verb_round_trips_through_the_dispatcher() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        let mut dispatch = async |msg, rx: &mut mpsc::Receiver<Frame>| -> ControlMsg {
            handle_control(
                &sup,
                msg,
                ConnectionCtx {
                    tx: &tx,
                    priority: &tx,
                    input_routes: &mut input_routes,
                    upload_routes: &mut no_uploads(),
                    tasks: &mut tasks,
                },
            )
            .await;
            let frame = rx
                .try_recv()
                .expect("a profile request must be answered, never dropped");
            serde_json::from_slice(&frame.body).expect("decode")
        };

        // The starters are what a fresh supervisor answers a list with.
        let ControlMsg::ProfileList { req_id, profiles } =
            dispatch(ControlMsg::ListProfiles { req_id: 1 }, &mut rx).await
        else {
            panic!("a list must answer with the catalog");
        };
        assert_eq!(req_id, 1, "the reply must correlate");
        assert_eq!(
            profiles.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            vec!["claude", "claude-yolo", "codex", "codex-yolo"],
            "a fresh supervisor is not empty (SPEC.md)"
        );

        let ControlMsg::ProfileCreated { req_id, profile } = dispatch(
            ControlMsg::CreateProfile {
                req_id: 2,
                name: "Mine".to_string(),
                invocation: "claude --dangerously-skip-permissions".to_string(),
                agent_kind: AgentKind::Claude,
                resume_template: None,
            },
            &mut rx,
        )
        .await
        else {
            panic!("a well-formed create must be stored");
        };
        assert_eq!(req_id, 2);
        assert_eq!(profile.name, "Mine");

        let ControlMsg::ProfileUpdated {
            req_id,
            profile: updated,
        } = dispatch(
            ControlMsg::UpdateProfile {
                req_id: 3,
                profile: farhelm_proto::Profile {
                    name: "Mine, renamed".to_string(),
                    ..profile.clone()
                },
            },
            &mut rx,
        )
        .await
        else {
            panic!("an edit of an existing profile must be stored");
        };
        assert_eq!(req_id, 3);
        assert_eq!(updated.name, "Mine, renamed");
        assert_eq!(updated.id, profile.id, "an edit never re-mints the id");

        let ControlMsg::ProfileDeleted { req_id } = dispatch(
            ControlMsg::DeleteProfile {
                req_id: 4,
                profile_id: profile.id.clone(),
            },
            &mut rx,
        )
        .await
        else {
            panic!("a delete of an existing profile must succeed");
        };
        assert_eq!(req_id, 4);

        // Both mutating verbs answer NotFound for an id the catalog does
        // not hold — a client working from a stale catalog is told so
        // rather than being left to believe it changed something.
        for (req_id, msg) in [
            (
                5,
                ControlMsg::DeleteProfile {
                    req_id: 5,
                    profile_id: profile.id.clone(),
                },
            ),
            (
                6,
                ControlMsg::UpdateProfile {
                    req_id: 6,
                    profile: profile.clone(),
                },
            ),
        ] {
            let ControlMsg::Error {
                req_id: replied,
                kind,
                ..
            } = dispatch(msg, &mut rx).await
            else {
                panic!("a profile that is gone cannot be edited or deleted again");
            };
            assert_eq!(replied, req_id);
            assert_eq!(kind, ErrorKind::NotFound);
        }
    }

    /// A profile write is refused when it would not fit the bounds
    /// `ProfileList` depends on, or when it describes a profile no create
    /// could ever use (PLAN_M6_75.md item 4).
    ///
    /// The bound rows are the load-bearing ones: an unbounded catalog is
    /// one too large to LIST, and the listing is how a client would find
    /// the profile it needs to delete — so the catalog that outgrows its
    /// reply can never be trimmed back. The other rows are about failing
    /// EARLY: a name with a control character and an integrated kind with a
    /// placeholder-free resume template are both refused here rather than
    /// at every create that later names the profile, which is what keeps
    /// "pick a profile" from failing for reasons the picker could not show.
    ///
    /// The trailing `{cwd}`-as-program block (invocation and template) is
    /// asserted separately from the table above because it is the one
    /// case where the exact wording matters, not merely that a refusal
    /// happens: this handler's `message` is what the profile editor
    /// renders verbatim, so it has to carry the placeholder name, the
    /// reason ("PROGRAM"), and the remedy ("belongs in an argument slot")
    /// together.
    #[tokio::test]
    async fn a_profile_write_is_refused_when_it_breaks_a_bound_or_could_never_launch() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();

        for (req_id, name, invocation, agent_kind, resume_template, expected) in [
            (
                1,
                "x".repeat(PROFILE_FIELD_CAP + 1),
                "claude".to_string(),
                AgentKind::Generic,
                None,
                PROFILE_FIELD_CAP.to_string(),
            ),
            (
                6,
                "empty template".to_string(),
                "bash".to_string(),
                AgentKind::Generic,
                Some(Vec::new()),
                "present but empty".to_string(),
            ),
            (
                7,
                "empty program".to_string(),
                "bash".to_string(),
                AgentKind::Generic,
                Some(vec![String::new(), "-c".to_string()]),
                "names no program".to_string(),
            ),
            (
                11,
                "quoted-empty invocation".to_string(),
                "''".to_string(),
                AgentKind::Generic,
                None,
                "names no program".to_string(),
            ),
            (
                12,
                String::new(),
                "bash".to_string(),
                AgentKind::Generic,
                None,
                "must not be empty".to_string(),
            ),
            (
                13,
                "    ".to_string(),
                "bash".to_string(),
                AgentKind::Generic,
                None,
                "must not be empty".to_string(),
            ),
            (
                8,
                "nul in the template".to_string(),
                "bash".to_string(),
                AgentKind::Generic,
                Some(vec!["bash".to_string(), "sleep\u{0}30".to_string()]),
                "NUL byte".to_string(),
            ),
            (
                9,
                "nul in the invocation".to_string(),
                "bash\u{0}-c".to_string(),
                AgentKind::Generic,
                None,
                "NUL byte".to_string(),
            ),
            (
                10,
                "one element too many".to_string(),
                "bash".to_string(),
                AgentKind::Generic,
                Some(vec!["x".to_string(); RESUME_TEMPLATE_ELEMENT_CAP + 1]),
                RESUME_TEMPLATE_ELEMENT_CAP.to_string(),
            ),
            (
                2,
                "tab\tseparated".to_string(),
                "claude".to_string(),
                AgentKind::Generic,
                None,
                "control characters".to_string(),
            ),
            (
                3,
                "unparseable".to_string(),
                "claude --flag 'unterminated".to_string(),
                AgentKind::Generic,
                None,
                "does not parse".to_string(),
            ),
            (
                4,
                "empty".to_string(),
                "   ".to_string(),
                AgentKind::Generic,
                None,
                "is empty".to_string(),
            ),
            (
                5,
                "unresumable".to_string(),
                "claude".to_string(),
                AgentKind::Claude,
                Some(vec!["claude".to_string(), "--continue".to_string()]),
                crate::agent_kind::CONVERSATION_PLACEHOLDER.to_string(),
            ),
            // The placeholder in the PROGRAM slot, which satisfies every
            // other rule — non-empty vector, non-empty argv[0], placeholder
            // present so an integrated kind is happy — and would make a
            // restart try to execute the conversation id it just read off
            // disk.
            (
                14,
                "placeholder as the program".to_string(),
                "claude".to_string(),
                AgentKind::Claude,
                Some(vec![
                    crate::agent_kind::CONVERSATION_PLACEHOLDER.to_string(),
                    "--resume".to_string(),
                ]),
                "the PROGRAM".to_string(),
            ),
        ] {
            handle_control(
                &sup,
                ControlMsg::CreateProfile {
                    req_id,
                    name,
                    invocation,
                    agent_kind,
                    resume_template,
                },
                ConnectionCtx {
                    tx: &tx,
                    priority: &tx,
                    input_routes: &mut input_routes,
                    upload_routes: &mut no_uploads(),
                    tasks: &mut tasks,
                },
            )
            .await;
            let frame = rx.try_recv().expect("a reply must have been sent");
            let decoded: ControlMsg = serde_json::from_slice(&frame.body).expect("decode");
            let ControlMsg::Error {
                req_id: replied,
                kind,
                message,
            } = decoded
            else {
                panic!("request {req_id} must be refused, got {decoded:?}");
            };
            assert_eq!(replied, req_id, "the refusal must correlate");
            assert_eq!(kind, ErrorKind::InvalidRequest);
            assert!(
                message.contains(&expected),
                "request {req_id}'s refusal must say what was wrong: {message}"
            );
        }

        // `{cwd}` as the PROGRAM, checked at this handler boundary rather
        // than only in the store's own tests, because this is the reply
        // the profile editor actually renders — a single fragment check
        // here would leave the placeholder name, "PROGRAM", or the
        // "belongs in an argument slot" remedy free to silently drop out
        // of the message the user reads.
        for (req_id, invocation, resume_template) in [
            (
                40,
                format!("{} claude", crate::agent_kind::CWD_PLACEHOLDER),
                None,
            ),
            (
                41,
                "claude".to_string(),
                Some(vec![
                    crate::agent_kind::CWD_PLACEHOLDER.to_string(),
                    "claude".to_string(),
                    "--resume".to_string(),
                    crate::agent_kind::CONVERSATION_PLACEHOLDER.to_string(),
                ]),
            ),
        ] {
            handle_control(
                &sup,
                ControlMsg::CreateProfile {
                    req_id,
                    name: format!("cwd-as-program {req_id}"),
                    invocation,
                    agent_kind: AgentKind::Claude,
                    resume_template,
                },
                ConnectionCtx {
                    tx: &tx,
                    priority: &tx,
                    input_routes: &mut input_routes,
                    upload_routes: &mut no_uploads(),
                    tasks: &mut tasks,
                },
            )
            .await;
            let frame = rx.try_recv().expect("a reply must have been sent");
            let decoded: ControlMsg = serde_json::from_slice(&frame.body).expect("decode");
            let ControlMsg::Error {
                req_id: replied,
                kind,
                message,
            } = decoded
            else {
                panic!("request {req_id} must be refused, got {decoded:?}");
            };
            assert_eq!(replied, req_id, "the refusal must correlate");
            assert_eq!(kind, ErrorKind::InvalidRequest);
            assert!(
                message.contains(crate::agent_kind::CWD_PLACEHOLDER)
                    && message.contains("PROGRAM")
                    && message.contains("belongs in an argument slot"),
                "request {req_id}'s refusal must carry the complete editor-facing message: \
                 {message}"
            );
        }

        assert_eq!(
            sup.store.profiles().await.expect("catalog").len(),
            4,
            "not one refused write may have landed beside the starters"
        );

        // The shapes that must be ACCEPTED, which a refusal table alone
        // cannot show. A validator is wrong in two directions and only one
        // of them is loud: a rejected-but-legal profile is reported to the
        // user as "the thing I typed will not save", with nothing in any
        // log explaining why it should have worked.
        //
        // The wrapper is the case that motivated this: an empty element
        // AFTER the program is `$0` for an inner shell, which is how a
        // resume template passes the captured identity as `$1` instead of
        // splicing it into the script text. Rejecting it forced users into
        // exactly the substitution the argv-vector design exists to avoid.
        for (req_id, template) in [
            (
                30,
                vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "exec claude --resume \"$1\"".to_string(),
                    String::new(),
                    crate::agent_kind::CONVERSATION_PLACEHOLDER.to_string(),
                ],
            ),
            // Several empty arguments, none of them the program.
            (
                31,
                vec![
                    "sh".to_string(),
                    String::new(),
                    String::new(),
                    crate::agent_kind::CONVERSATION_PLACEHOLDER.to_string(),
                ],
            ),
        ] {
            handle_control(
                &sup,
                ControlMsg::CreateProfile {
                    req_id,
                    name: format!("wrapper {req_id}"),
                    invocation: "sh".to_string(),
                    agent_kind: AgentKind::Generic,
                    resume_template: Some(template),
                },
                ConnectionCtx {
                    tx: &tx,
                    priority: &tx,
                    input_routes: &mut input_routes,
                    upload_routes: &mut no_uploads(),
                    tasks: &mut tasks,
                },
            )
            .await;
            let frame = rx.try_recv().expect("a reply must have been sent");
            let decoded: ControlMsg = serde_json::from_slice(&frame.body).expect("decode");
            assert!(
                matches!(decoded, ControlMsg::ProfileCreated { req_id: replied, .. }
                    if replied == req_id),
                "an empty element after the program is a legal argv entry: {decoded:?}"
            );
        }

        // The other side of both bounds: a record exactly AT each limit
        // must be accepted, or a cap written with the wrong comparison
        // silently rejects legitimate profiles.
        let name = "at the cap";
        let invocation = "bash";
        let filler = PROFILE_FIELD_CAP - name.len() - invocation.len();
        for (req_id, resume_template) in [
            // Exactly `PROFILE_FIELD_CAP` bytes of name + invocation +
            // template, in one element.
            (20, vec!["x".repeat(filler)]),
            // Exactly `RESUME_TEMPLATE_ELEMENT_CAP` elements, well under
            // the byte cap: the two bounds are independent, so each needs
            // its own boundary case.
            (21, vec!["x".to_string(); RESUME_TEMPLATE_ELEMENT_CAP]),
        ] {
            handle_control(
                &sup,
                ControlMsg::CreateProfile {
                    req_id,
                    name: name.to_string(),
                    invocation: invocation.to_string(),
                    agent_kind: AgentKind::Generic,
                    resume_template: Some(resume_template),
                },
                ConnectionCtx {
                    tx: &tx,
                    priority: &tx,
                    input_routes: &mut input_routes,
                    upload_routes: &mut no_uploads(),
                    tasks: &mut tasks,
                },
            )
            .await;
            let frame = rx.try_recv().expect("a reply must have been sent");
            let decoded: ControlMsg = serde_json::from_slice(&frame.body).expect("decode");
            assert!(
                matches!(decoded, ControlMsg::ProfileCreated { req_id: replied, .. }
                    if replied == req_id),
                "a record exactly at the limit is legal: {decoded:?}"
            );
        }
    }

    /// The same rules apply to an EDIT, not only to a create.
    ///
    /// Both verbs share one validator, so this is a wiring test rather
    /// than a second copy of the table above — but the wiring is exactly
    /// what a bound needs: an update that accepted what a create refuses
    /// would let every refused shape into the catalog one edit at a time,
    /// and the catalog is where the rest of the system reads them from.
    ///
    /// Driven against a REAL existing profile, so a refusal cannot be
    /// passing for the unrelated reason that the id is unknown — and the
    /// stored record is read back afterwards to prove a refused edit
    /// changed nothing.
    ///
    /// The trailing `{cwd}`-as-program block mirrors the create-side test's
    /// full-wording assertion on `UpdateProfile`'s own validation call,
    /// which is wired independently of `CreateProfile`'s.
    #[tokio::test]
    async fn an_edit_is_held_to_the_same_rules_as_a_create() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        let original = sup
            .store
            .profile("starter-claude")
            .await
            .expect("read")
            .expect("the starter is there to edit");

        for (req_id, name, invocation, resume_template, expected) in [
            (
                1,
                String::new(),
                "claude".to_string(),
                None,
                "must not be empty",
            ),
            (
                2,
                "   ".to_string(),
                "claude".to_string(),
                None,
                "must not be empty",
            ),
            (
                3,
                "empty program".to_string(),
                "''".to_string(),
                None,
                "names no program",
            ),
            (
                4,
                "empty program in the template".to_string(),
                "claude".to_string(),
                Some(vec![String::new(), "--resume".to_string()]),
                "names no program",
            ),
            (
                5,
                "nul".to_string(),
                "claude\u{0}".to_string(),
                None,
                "NUL byte",
            ),
            // The placeholder in the program slot; see the create table's
            // row for what makes this shape slip past every other rule.
            (
                6,
                "placeholder as the program".to_string(),
                "claude".to_string(),
                Some(vec![
                    crate::agent_kind::CONVERSATION_PLACEHOLDER.to_string(),
                    "--resume".to_string(),
                ]),
                "the PROGRAM",
            ),
        ] {
            handle_control(
                &sup,
                ControlMsg::UpdateProfile {
                    req_id,
                    profile: farhelm_proto::Profile {
                        name,
                        invocation,
                        resume_template,
                        agent_kind: AgentKind::Generic,
                        ..original.clone()
                    },
                },
                ConnectionCtx {
                    tx: &tx,
                    priority: &tx,
                    input_routes: &mut input_routes,
                    upload_routes: &mut no_uploads(),
                    tasks: &mut tasks,
                },
            )
            .await;
            let frame = rx.try_recv().expect("a reply must have been sent");
            let decoded: ControlMsg = serde_json::from_slice(&frame.body).expect("decode");
            let ControlMsg::Error {
                req_id: replied,
                kind,
                message,
            } = decoded
            else {
                panic!("edit {req_id} must be refused, got {decoded:?}");
            };
            assert_eq!(replied, req_id);
            assert_eq!(kind, ErrorKind::InvalidRequest);
            assert!(
                message.contains(expected),
                "edit {req_id}'s refusal must say what was wrong: {message}"
            );
        }

        // `{cwd}` as the PROGRAM, held to the same full-wording bar as the
        // create-side test: `UpdateProfile` runs its own
        // `validate_profile_fields` call independently of `CreateProfile`'s
        // (`handle_update_profile`, not shared code beyond the validator
        // itself), so a regression in this wiring specifically would pass
        // every create-side test above while still breaking edits.
        for (req_id, invocation, resume_template) in [
            (
                50,
                format!("{} claude", crate::agent_kind::CWD_PLACEHOLDER),
                None,
            ),
            (
                51,
                "claude".to_string(),
                Some(vec![
                    crate::agent_kind::CWD_PLACEHOLDER.to_string(),
                    "claude".to_string(),
                    "--resume".to_string(),
                    crate::agent_kind::CONVERSATION_PLACEHOLDER.to_string(),
                ]),
            ),
        ] {
            handle_control(
                &sup,
                ControlMsg::UpdateProfile {
                    req_id,
                    profile: farhelm_proto::Profile {
                        invocation,
                        resume_template,
                        agent_kind: AgentKind::Claude,
                        ..original.clone()
                    },
                },
                ConnectionCtx {
                    tx: &tx,
                    priority: &tx,
                    input_routes: &mut input_routes,
                    upload_routes: &mut no_uploads(),
                    tasks: &mut tasks,
                },
            )
            .await;
            let frame = rx.try_recv().expect("a reply must have been sent");
            let decoded: ControlMsg = serde_json::from_slice(&frame.body).expect("decode");
            let ControlMsg::Error {
                req_id: replied,
                kind,
                message,
            } = decoded
            else {
                panic!("edit {req_id} must be refused, got {decoded:?}");
            };
            assert_eq!(replied, req_id);
            assert_eq!(kind, ErrorKind::InvalidRequest);
            assert!(
                message.contains(crate::agent_kind::CWD_PLACEHOLDER)
                    && message.contains("PROGRAM")
                    && message.contains("belongs in an argument slot"),
                "edit {req_id}'s refusal must carry the complete editor-facing message: {message}"
            );
        }

        assert_eq!(
            sup.store.profile("starter-claude").await.expect("read"),
            Some(original),
            "not one refused edit may have reached the catalog"
        );
    }

    /// Every profile verb against an unreadable catalog answers, and
    /// answers with a correlated `Internal`.
    ///
    /// Table-driven across all four because the failure being excluded is
    /// SILENCE, and silence is per-arm: a verb whose error path forgot to
    /// reply leaves its caller waiting on a `req_id` that never comes, and
    /// a hung request looks nothing like a failure from either end. The
    /// classification matters too — `Internal` rather than
    /// `InvalidRequest`, because nothing about these requests is wrong and
    /// a client told "bad request" would stop retrying something that will
    /// work again the moment the database does.
    #[tokio::test]
    async fn every_profile_verb_answers_when_the_catalog_cannot_be_read() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        let profile = sup
            .store
            .profile("starter-claude")
            .await
            .expect("read")
            .expect("present");
        sup.store.drop_profile_catalog_for_test().await;

        for (req_id, request) in [
            (1, ControlMsg::ListProfiles { req_id: 1 }),
            (
                2,
                ControlMsg::CreateProfile {
                    req_id: 2,
                    name: "new".to_string(),
                    invocation: "bash".to_string(),
                    agent_kind: AgentKind::Generic,
                    resume_template: None,
                },
            ),
            (
                3,
                ControlMsg::UpdateProfile {
                    req_id: 3,
                    profile: profile.clone(),
                },
            ),
            (
                4,
                ControlMsg::DeleteProfile {
                    req_id: 4,
                    profile_id: profile.id.clone(),
                },
            ),
        ] {
            handle_control(
                &sup,
                request,
                ConnectionCtx {
                    tx: &tx,
                    priority: &tx,
                    input_routes: &mut input_routes,
                    upload_routes: &mut no_uploads(),
                    tasks: &mut tasks,
                },
            )
            .await;
            let frame = rx
                .try_recv()
                .expect("a profile request must be answered, never dropped");
            let decoded: ControlMsg = serde_json::from_slice(&frame.body).expect("decode");
            let ControlMsg::Error {
                req_id: replied,
                kind,
                ..
            } = decoded
            else {
                panic!("request {req_id} cannot have succeeded against no catalog: {decoded:?}");
            };
            assert_eq!(replied, req_id, "the failure must correlate");
            assert_eq!(
                kind,
                ErrorKind::Internal,
                "request {req_id}: a database that cannot be read is not a bad request"
            );
        }
    }

    /// The unknown-profile precondition (PLAN_M6_75.md item 4), driven as
    /// the RACE it actually is rather than as a made-up id: the profile is
    /// read, then deleted, then the create that names it arrives — exactly
    /// what happens when a user deletes a profile in one client while
    /// another has the create dialog open.
    ///
    /// Three things must hold, and each is a distinct way this could go
    /// wrong. The create must FAIL rather than silently fall back to some
    /// other profile (a launch the user never asked for, and SPEC.md's
    /// creation-failure split makes a precondition failure a visible one).
    /// It must leave NO session — the failure is decided before anything is
    /// launched. And the refusal must be recorded against the intent key so
    /// a retry replays it: an unknown profile is a precondition failure
    /// like a vanished working directory, and `create_session`'s contract
    /// for those has no exception.
    #[tokio::test]
    async fn a_profile_deleted_between_the_picker_and_the_submit_fails_the_create() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();

        // What the client read from its picker, and what happened to it
        // before the client got around to submitting.
        let picked = sup.store.profiles().await.expect("catalog")[0].clone();
        assert!(
            sup.store
                .delete_profile(&picked.id)
                .await
                .expect("delete the profile out from under the create")
        );

        let create = |req_id| ControlMsg::CreateSession {
            req_id,
            parent: None,
            profile_name: None,
            cwd: state.path().to_string_lossy().to_string(),
            invocation: None,
            profile_id: Some(picked.id.clone()),
            title: None,
            cols: 80,
            rows: 24,
            intent_key: Some("intent-abc".to_string()),
            agent_kind: None,
            resume_template: None,
        };
        let mut refusal = async |msg, rx: &mut mpsc::Receiver<Frame>| -> (u64, ErrorKind, String) {
            handle_control(
                &sup,
                msg,
                ConnectionCtx {
                    tx: &tx,
                    priority: &tx,
                    input_routes: &mut input_routes,
                    upload_routes: &mut no_uploads(),
                    tasks: &mut tasks,
                },
            )
            .await;
            let frame = rx.try_recv().expect("a reply must have been sent");
            match serde_json::from_slice(&frame.body).expect("decode") {
                ControlMsg::Error {
                    req_id,
                    kind,
                    message,
                } => (req_id, kind, message),
                other => panic!("a create naming a deleted profile must fail: {other:?}"),
            }
        };

        let (req_id, kind, message) = refusal(create(7), &mut rx).await;
        assert_eq!(req_id, 7);
        assert_eq!(kind, ErrorKind::NotFound);
        assert!(
            message.contains(&picked.id) && message.contains("deleted"),
            "the refusal must name the profile and say what likely happened to it, so the user \
             re-picks instead of guessing their profile is broken: {message}"
        );
        assert!(
            sup.sessions.lock().await.is_empty(),
            "a precondition failure launches nothing"
        );

        // The retry replays the SAME answer rather than re-deriving it from
        // a catalog that may have changed again — and the way that is
        // pinned is by making the catalog change first, in the one
        // direction that would make a re-derivation SUCCEED.
        //
        // A profile with the same id existing again is not reachable
        // through the real API (creates mint their own ids), which is
        // exactly why the replay rule has to be tested rather than assumed:
        // the only thing standing between this key and a second, different
        // answer is that a settled refusal is replayed rather than
        // recomputed. A build that re-resolved here would create a session
        // for an intent whose client was already told, definitively, that
        // it had failed.
        sup.store
            .insert_profile_with_id(farhelm_proto::Profile {
                id: picked.id.clone(),
                name: "Recreated under the same id".to_string(),
                invocation: "bash".to_string(),
                agent_kind: AgentKind::Generic,
                resume_template: None,
            })
            .await
            .expect("reconstruct the id the catalog once held");

        let (_, replayed_kind, replayed_message) = refusal(create(8), &mut rx).await;
        assert_eq!(replayed_kind, kind);
        assert_eq!(replayed_message, message);
        assert!(
            sup.sessions.lock().await.is_empty(),
            "a settled refusal is replayed, not recomputed against a catalog that has changed \
             since"
        );
    }

    /// What a profile-backed create actually PRODUCES (PLAN_M6_75.md item
    /// 4): a session whose durable snapshot came from the profile, and a
    /// source-profile identity recorded beside it.
    ///
    /// The integration half is the point the plan is emphatic about — the
    /// profile feeds the EXISTING `IntegrationSnapshot` seam rather than a
    /// second path — so it is asserted through what the seam produces: the
    /// kind the profile named, and the resume template that kind derives
    /// from the profile's OWN argv0. A build that stored the profile's
    /// fields without going through the seam would still create a session,
    /// and it would silently be a session that can never resume its
    /// conversation.
    ///
    /// The replay half pins the idempotency contract for the new mode: a
    /// retried key returns the first attempt's session rather than
    /// launching a second agent, which is the whole reason the mode and the
    /// profile identity join the create fingerprint.
    ///
    /// Finally, the first create enters through an ordinary `ConnectionCtx`
    /// and its durable reservation must therefore carry `Permanent`. The
    /// assertion lives on this production dispatcher path so item 4 can add
    /// an authenticated variant without weakening the ordinary connection's
    /// M3 contract.
    #[tokio::test]
    async fn a_profile_backed_create_snapshots_the_profile_and_replays_its_key() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();

        // A profile whose invocation is a PATH, so the derived resume
        // template can only have come from this profile's own argv0 rather
        // than from a bare command name someone hardcoded.
        let profile = match sup
            .store
            .create_profile(
                "Local Claude".to_string(),
                "/opt/bin/claude --verbose".to_string(),
                AgentKind::Claude,
                None,
            )
            .await
            .expect("create the profile")
        {
            crate::store::ProfileCreation::Created(profile) => profile,
            other => panic!("a catalog of four starters is not full: {other:?}"),
        };
        let create = |req_id| ControlMsg::CreateSession {
            req_id,
            parent: None,
            profile_name: None,
            cwd: state.path().to_string_lossy().to_string(),
            invocation: None,
            profile_id: Some(profile.id.clone()),
            title: Some("from a profile".to_string()),
            cols: 80,
            rows: 24,
            intent_key: Some("intent-profile".to_string()),
            agent_kind: None,
            resume_template: None,
        };
        let mut created = async |msg, rx: &mut mpsc::Receiver<Frame>| -> SessionInfo {
            handle_control(
                &sup,
                msg,
                ConnectionCtx {
                    tx: &tx,
                    priority: &tx,
                    input_routes: &mut input_routes,
                    upload_routes: &mut no_uploads(),
                    tasks: &mut tasks,
                },
            )
            .await;
            let frame = rx.try_recv().expect("a reply must have been sent");
            match serde_json::from_slice(&frame.body).expect("decode") {
                ControlMsg::SessionCreated { session, .. } => session,
                other => panic!("a create naming a live profile must succeed: {other:?}"),
            }
        };

        let session = created(create(1), &mut rx).await;
        assert_eq!(
            sup.store
                .reservation("intent-profile")
                .await
                .expect("reservation lookup")
                .expect("a keyed create persists its reservation")
                .dedup_scope,
            crate::store::DedupScope::Permanent,
            "an ordinary connection derives M3's permanent scope; authenticated spawn changes \
             this seam without letting request fields choose the policy"
        );
        assert_eq!(
            session.invocation, "/opt/bin/claude --verbose",
            "the profile is what says WHAT to run"
        );
        assert_eq!(
            session.source_profile,
            Some(farhelm_proto::SourceProfile {
                id: profile.id.clone(),
                name: "Local Claude".to_string(),
                existence: farhelm_proto::ProfileExistence::Present,
            })
        );
        let snapshot = sup
            .session_snapshot(&session.id)
            .await
            .expect("reading the snapshot")
            .expect("the session exists");
        assert_eq!(
            snapshot.kind,
            AgentKind::Claude,
            "the profile's kind is an explicit choice, not a basename guess"
        );
        assert_eq!(
            snapshot.resume_template.as_deref().unwrap(),
            [
                "/opt/bin/claude",
                "--resume",
                crate::agent_kind::CONVERSATION_PLACEHOLDER
            ],
            "an absent template on an integrated profile means the KIND's default, derived from \
             this profile's own argv0"
        );

        // The catalog moves between the two attempts, which is what makes
        // this a derivation test as well as an idempotency test: a REPLAY
        // is still a reply, so its existence must describe the catalog now
        // rather than repeating what the original attempt reported.
        sup.store
            .update_profile(farhelm_proto::Profile {
                name: "Local Claude, renamed".to_string(),
                ..profile.clone()
            })
            .await
            .expect("rename")
            .expect("the profile is there to rename");

        let replayed = created(create(2), &mut rx).await;
        assert_eq!(
            replayed.id, session.id,
            "a retried intent key replays its session rather than launching a second agent"
        );
        assert_eq!(
            sup.sessions.lock().await.len(),
            1,
            "and there is still exactly one session"
        );
        let source = replayed
            .source_profile
            .expect("a replay describes the same session, profile and all");
        assert_eq!(
            source.name, "Local Claude",
            "the SNAPSHOT is immutable: an edit does not reach the sessions already created \
             from the profile (SPEC.md)"
        );
        assert_eq!(
            source.existence,
            farhelm_proto::ProfileExistence::Renamed,
            "while existence is derived for the reply being built, not replayed"
        );

        // And the same again for a DELETE, which is the state a client is
        // most likely to be looking at when it retries an old key.
        assert!(sup.store.delete_profile(&profile.id).await.expect("delete"));
        let after_delete = created(create(3), &mut rx).await;
        let source = after_delete
            .source_profile
            .expect("a deleted profile does not erase what the session came from");
        assert_eq!(source.name, "Local Claude");
        assert_eq!(source.existence, farhelm_proto::ProfileExistence::Deleted);
    }

    /// An EXPLICIT resume template on a profile must reach the session
    /// verbatim, never re-derived from the invocation's argv0.
    ///
    /// The neighboring test above proves the NULL-template half of the
    /// contract: an absent template on an integrated profile lets the kind
    /// derive one from argv0. This test proves the other half, and it is
    /// the half `STARTER_PROFILES` actually depends on for its two yolo
    /// rows — derivation from argv0 alone would rebuild
    /// `codex resume {conversation}` from `starter-codex-yolo`'s
    /// invocation and silently drop `--yolo`, turning a resumed unattended
    /// session back into a permission-gated one with no error anywhere.
    /// Every other test in this module creates from a NULL-template
    /// profile, so this shape would slip past all of them if the create
    /// path ever started re-deriving instead of honoring what the profile
    /// says.
    ///
    /// Driven against `starter-codex-yolo` itself rather than a
    /// hand-built fixture, which doubles as an end-to-end check that the
    /// seeded yolo starter is actually usable through the ordinary create
    /// path.
    #[tokio::test]
    async fn a_profile_backed_create_honors_an_explicit_resume_template_verbatim() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();

        handle_control(
            &sup,
            ControlMsg::CreateSession {
                req_id: 1,
                parent: None,
                profile_name: None,
                cwd: state.path().to_string_lossy().to_string(),
                invocation: None,
                profile_id: Some("starter-codex-yolo".to_string()),
                title: Some("from the yolo starter".to_string()),
                cols: 80,
                rows: 24,
                intent_key: Some("intent-yolo".to_string()),
                agent_kind: None,
                resume_template: None,
            },
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;
        let frame = rx.try_recv().expect("a reply must have been sent");
        let session = match serde_json::from_slice(&frame.body).expect("decode") {
            ControlMsg::SessionCreated { session, .. } => session,
            other => panic!("a create naming the seeded yolo starter must succeed: {other:?}"),
        };

        assert_eq!(
            session.invocation, "codex --yolo",
            "the profile is what says WHAT to run, flag included"
        );
        assert_eq!(
            session.source_profile,
            Some(farhelm_proto::SourceProfile {
                id: "starter-codex-yolo".to_string(),
                name: "codex-yolo".to_string(),
                existence: farhelm_proto::ProfileExistence::Present,
            })
        );

        let snapshot = sup
            .session_snapshot(&session.id)
            .await
            .expect("reading the snapshot")
            .expect("the session exists");
        assert_eq!(snapshot.kind, AgentKind::Codex);
        assert_eq!(
            snapshot.resume_template.as_deref().unwrap(),
            [
                "codex",
                "--yolo",
                "resume",
                crate::agent_kind::CONVERSATION_PLACEHOLDER
            ],
            "an explicit template is stored AS WRITTEN, not re-derived from argv0 — \
             re-deriving here would silently drop --yolo and resume this session gated again"
        );
    }

    /// A session-authenticated peer has a SHORT list of operations, and a
    /// message outside it is refused as an authority question rather than
    /// falling through to a catch-all; creating, which is on the list,
    /// still cannot forge a sibling or ancestor relationship.
    ///
    /// The list is no longer "create only" — `ReportConversation` joined
    /// it — so the two halves here are about different things: the first
    /// pins that the allowlist is still an ALLOWLIST (an off-list message
    /// gets `Unauthorized`, not a reply built from the session's own
    /// credential), and the second pins the one check that has to happen
    /// inside an allowed operation. Adding an operation must not quietly
    /// turn the first half into a tautology about whichever message this
    /// test happens to pick.
    #[tokio::test]
    async fn restricted_dispatch_refuses_an_off_list_message_and_a_forged_parent() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let auth = authenticated_parent(&sup, state.path(), "parent-session").await;

        handle_restricted_control(
            &sup,
            ControlMsg::ListSessions {
                req_id: 41,
                cursor: None,
                limit: None,
            },
            &tx,
            &auth,
        )
        .await;
        let unauthorized: ControlMsg =
            serde_json::from_slice(&rx.recv().await.expect("authority refusal").body).unwrap();
        assert!(matches!(
            unauthorized,
            ControlMsg::Error {
                req_id: 41,
                kind: ErrorKind::Unauthorized,
                ..
            }
        ));

        handle_restricted_control(
            &sup,
            ControlMsg::CreateSession {
                req_id: 42,
                parent: Some("forged-parent".to_string()),
                cwd: state.path().to_string_lossy().into_owned(),
                invocation: None,
                profile_id: None,
                profile_name: Some("Claude Code".to_string()),
                title: None,
                cols: 80,
                rows: 24,
                intent_key: Some("forged-key".to_string()),
                agent_kind: None,
                resume_template: None,
            },
            &tx,
            &auth,
        )
        .await;
        let forged: ControlMsg =
            serde_json::from_slice(&rx.recv().await.expect("parent refusal").body).unwrap();
        let ControlMsg::Error {
            req_id,
            kind,
            message,
        } = forged
        else {
            panic!("a forged parent must be refused");
        };
        assert_eq!(req_id, 42);
        assert_eq!(kind, ErrorKind::Unauthorized);
        assert!(message.contains("parent-session"));
        assert!(sup.sessions.lock().await.is_empty());
        assert_eq!(sup.store.reservation("forged-key").await.unwrap(), None);
    }

    /// Deleting a parent revokes every already-open restricted connection.
    ///
    /// Hello-time authentication is only admission to the connection; the
    /// lifecycle-serialized check here is what prevents a cached bearer from
    /// creating descendants after its authority row is gone.
    #[tokio::test]
    async fn restricted_create_revalidates_after_its_parent_is_deleted() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let auth = authenticated_parent(&sup, state.path(), "revoked-parent").await;
        sup.store
            .delete_session_settling_reservations(&auth.session_id)
            .await
            .expect("delete authenticated parent");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);

        handle_restricted_control(
            &sup,
            ControlMsg::CreateSession {
                req_id: 43,
                parent: None,
                cwd: state.path().to_string_lossy().into_owned(),
                invocation: None,
                profile_id: None,
                profile_name: None,
                title: None,
                cols: 80,
                rows: 24,
                intent_key: Some("revoked-key".to_string()),
                agent_kind: None,
                resume_template: None,
            },
            &tx,
            &auth,
        )
        .await;

        let reply: ControlMsg =
            serde_json::from_slice(&rx.recv().await.expect("revocation reply").body).unwrap();
        assert!(matches!(
            reply,
            ControlMsg::Error {
                req_id: 43,
                kind: ErrorKind::Unauthorized,
                ..
            }
        ));
        assert_eq!(sup.store.reservation("revoked-key").await.unwrap(), None);
    }

    /// Seed a session that is able to report its own conversation
    /// identity: the durable row a restricted connection's credential is
    /// validated against, AND the in-memory entry the report itself lands
    /// in.
    ///
    /// Both halves are needed, and for different reasons —
    /// `authenticates_session` reads the row, while
    /// `Supervisor::report_conversation` looks up the ENTRY for the
    /// generation its durable write is fenced on and for the capture cell
    /// the accepted report advances. `authenticated_parent` above seeds
    /// only the durable half because a create needs nothing more, so a
    /// report driven through it would answer `NotFound` for a reason that
    /// has nothing to do with what is under test.
    ///
    /// Claude-kind with a placeholder-carrying resume template, so an
    /// accepted report can actually turn into `RestartOffer::Resume`: a
    /// Generic session has no integration, and its offer would stay
    /// `FreshOnly` no matter what was reported — an assertion that passed
    /// for the wrong reason.
    async fn reporting_session(sup: &Arc<Supervisor>, id: &str) -> farhelm_proto::SessionAuth {
        let claimed = sup
            .store
            .insert_session(
                crate::store::StoredSession {
                    conversation_source: None,
                    id: id.to_string(),
                    parent: None,
                    archived: false,
                    title: id.to_string(),
                    created_at: crate::store::now_unix(),
                    last_activity_at: crate::store::now_unix(),
                    creation_seq: 0,
                    cwd: "/tmp".to_string(),
                    invocation: "claude".to_string(),
                    tmux_name: format!("fh-{id}"),
                    pane: String::new(),
                    outcome: LastOutcome::Running,
                    agent_kind: AgentKind::Claude,
                    resume_template: Some(vec![
                        "claude".to_string(),
                        "--resume".to_string(),
                        crate::agent_kind::CONVERSATION_PLACEHOLDER.to_string(),
                    ]),
                    canonical_cwd: Some("/tmp".to_string()),
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                    source_profile: None,
                },
                None,
            )
            .await
            .expect("seed a reporting session");
        let crate::store::Claimed::Ours { session_token, .. } = claimed else {
            panic!("an unkeyed insert cannot be taken");
        };
        let mut entry = entry_with(None, LastOutcome::Running);
        entry.info.id = id.to_string();
        entry.snapshot = IntegrationSnapshot {
            kind: AgentKind::Claude,
            resume_template: None,
        };
        sup.sessions
            .lock()
            .await
            .insert(id.to_string(), Arc::new(entry));
        farhelm_proto::SessionAuth {
            session_id: id.to_string(),
            token: session_token,
        }
    }

    /// Send one `ReportConversation` down a restricted connection and
    /// decode the single reply it produces, with the ordinary `startup`
    /// source every test that is not about the source field wants.
    async fn send_report(
        sup: &Arc<Supervisor>,
        auth: &farhelm_proto::SessionAuth,
        req_id: u64,
        conversation: &str,
    ) -> ControlMsg {
        send_report_with_source(sup, auth, req_id, conversation, "startup").await
    }

    /// [`send_report`] with the vendor's `source` string chosen by the
    /// caller, for the one test that has to drive a HOSTILE value through
    /// the real dispatch rather than through `sanitized_source` alone.
    async fn send_report_with_source(
        sup: &Arc<Supervisor>,
        auth: &farhelm_proto::SessionAuth,
        req_id: u64,
        conversation: &str,
        source: &str,
    ) -> ControlMsg {
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        handle_restricted_control(
            sup,
            ControlMsg::ReportConversation {
                req_id,
                conversation: conversation.to_string(),
                source: source.to_string(),
            },
            &tx,
            auth,
        )
        .await;
        serde_json::from_slice(&rx.recv().await.expect("a report is always answered").body)
            .expect("decode the report reply")
    }

    /// A helm — full authority over every session on the host — may not
    /// report a conversation identity, and is TOLD so rather than ignored.
    ///
    /// Reporting is not an authority question. It is a claim to BE a
    /// particular session's agent, which only that session's own
    /// credential can support; a helm that could make it could silently
    /// redirect any session's resume to any conversation. The full-
    /// authority dispatch's catch-all would log the message and send
    /// nothing, which is worse than a refusal in a specific way: the
    /// sender waits on a reply that never arrives, and a request/reply
    /// client with no timeout hangs forever. That is why this arm exists
    /// at all, and why the exact message is pinned — it is the only thing
    /// telling the sender which door to use instead.
    #[tokio::test]
    async fn a_helm_may_not_report_a_conversation_identity() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let (_tasks, mut rx) = dispatch_for_test(
            &sup,
            ControlMsg::ReportConversation {
                req_id: 61,
                conversation: "conv-helm".to_string(),
                source: "startup".to_string(),
            },
        )
        .await;
        let reply: ControlMsg = serde_json::from_slice(
            &rx.recv()
                .await
                .expect("the refusal must be SENT, not merely logged")
                .body,
        )
        .unwrap();
        let ControlMsg::Error {
            req_id,
            kind,
            message,
        } = reply
        else {
            panic!("a helm's report must be refused: {reply:?}");
        };
        assert_eq!(req_id, 61);
        assert_eq!(kind, ErrorKind::Unauthorized);
        assert_eq!(
            message,
            "only the session's own agent may report its conversation"
        );
    }

    /// A report carrying a credential that does not authenticate is
    /// refused, exactly as a create with the same credential would be.
    ///
    /// Hello-time authentication is admission to the connection, not
    /// standing for each request on it — the session behind a cached
    /// bearer can be deleted, or the token replaced, while the connection
    /// stays open. Reporting revalidates for the same reason creating
    /// does, and this is the check that stops a stale or forged credential
    /// from rewriting a live session's resume identity.
    #[tokio::test]
    async fn a_report_with_an_invalid_credential_is_unauthorized() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let auth = reporting_session(&sup, "reporting-session").await;
        let forged = farhelm_proto::SessionAuth {
            session_id: auth.session_id.clone(),
            token: "not-the-session-token".to_string(),
        };

        let reply = send_report(&sup, &forged, 62, "conv-forged").await;
        assert!(
            matches!(
                reply,
                ControlMsg::Error {
                    req_id: 62,
                    kind: ErrorKind::Unauthorized,
                    ..
                }
            ),
            "a report with a credential that does not authenticate must be refused: {reply:?}"
        );
        assert_eq!(
            sup.session_snapshot(&auth.session_id)
                .await
                .unwrap()
                .expect("the session still exists")
                .captured_conversation,
            None,
            "a refused report must not have written anything"
        );
    }

    /// An identity this build will not store is refused as an INVALID
    /// REQUEST, before any WRITE.
    ///
    /// Not before every lookup — the credential check runs first and reads
    /// the session row, deliberately, so that an unauthenticated peer
    /// cannot use the shape of the refusal to learn which half of its
    /// request was wrong. What the shape check precedes is the store write
    /// and the log line.
    ///
    /// Every row is a shape that reaches an agent's command line if it is
    /// admitted. A leading `-` is the sharp one: `--resume --last` is not
    /// a weird identifier, it is a different flag, and the argv slot
    /// substitution that keeps an id in one argument does nothing about
    /// it — slot substitution guarantees the id stays ONE argument, never
    /// that the argument is not a flag. Whitespace is a different and
    /// milder argument: it is not an injection vector at all once the id is
    /// one argv element, it is simply not a shape any plausible vendor id
    /// has. And the 129-byte row pins the handler's OWN bound —
    /// post-handshake frames are capped only at `MAX_FRAME_LEN`, so nothing
    /// upstream of this check bounds the field.
    ///
    /// `InvalidRequest` rather than `Unauthorized` because the peer is
    /// entitled to report; what it sent is not reportable. The distinction
    /// is what a hook's trace file records, and it is the difference
    /// between "your credential is wrong" and "your vendor changed its id
    /// format".
    ///
    /// The `req_id` is asserted per row rather than only the kind: a
    /// refusal is a REPLY, and a reply carrying the wrong correlation id
    /// leaves a request/reply client waiting on one that never comes —
    /// exactly the hang the helm-refusal arm exists to prevent, reached
    /// here by a different route.
    #[tokio::test]
    async fn a_report_carrying_an_implausible_identity_is_refused() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let auth = reporting_session(&sup, "reporting-session").await;

        let oversized = "a".repeat(MAX_CONVERSATION_BYTES + 1);
        for (req_id, conversation) in [
            (63u64, "-bad"),
            (64, "has space"),
            (65, ""),
            (66, oversized.as_str()),
        ] {
            let reply = send_report(&sup, &auth, req_id, conversation).await;
            let ControlMsg::Error {
                req_id: answered,
                kind,
                ..
            } = reply
            else {
                panic!("{conversation:?} is not an identity this build stores: {reply:?}");
            };
            assert_eq!(kind, ErrorKind::InvalidRequest);
            assert_eq!(
                answered, req_id,
                "{conversation:?}: the refusal must answer the request that caused it"
            );
        }
        assert_eq!(
            sup.session_snapshot(&auth.session_id)
                .await
                .unwrap()
                .expect("the session still exists")
                .captured_conversation,
            None,
            "no refused shape may reach the column"
        );
    }

    /// An accepted report becomes the session's resume identity, and a
    /// SECOND report replaces the first.
    ///
    /// The first half is the whole point of the mechanism: the offer the
    /// user sees, and the id a restart substitutes into the resume
    /// template, both come from the durable column this handler writes.
    /// Asserting through `session_snapshot` rather than the in-memory
    /// capture state is deliberate — the snapshot is what a restart
    /// actually reads, so it proves the write landed rather than that this
    /// process merely believes it did.
    ///
    /// The second half is the bug the design exists to fix. `/clear` and
    /// `/new` start a genuinely new conversation inside the SAME agent
    /// process; the hook fires again with a new id, and the previous one
    /// is then precisely what must never be resumed. A handler that
    /// treated the second report as a duplicate would leave farhelm
    /// offering to resume a conversation the user has already thrown away.
    #[tokio::test]
    async fn a_report_claims_the_resume_identity_and_a_later_one_replaces_it() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let auth = reporting_session(&sup, "reporting-session").await;

        let first = send_report(&sup, &auth, 67, "conv-first").await;
        assert!(
            matches!(first, ControlMsg::ConversationReported { req_id: 67 }),
            "a plausible report from the session's own agent must be accepted: {first:?}"
        );
        let snapshot = sup
            .session_snapshot(&auth.session_id)
            .await
            .unwrap()
            .expect("the session exists");
        assert_eq!(
            snapshot.captured_conversation.as_deref(),
            Some("conv-first")
        );
        assert_eq!(
            snapshot.restart_offer,
            RestartOffer::Resume,
            "an integrated session with a stored identity owes the user a resume offer"
        );

        let second = send_report(&sup, &auth, 68, "conv-second").await;
        assert!(
            matches!(second, ControlMsg::ConversationReported { req_id: 68 }),
            "a second report is a new conversation, not a duplicate request: {second:?}"
        );
        let snapshot = sup
            .session_snapshot(&auth.session_id)
            .await
            .unwrap()
            .expect("the session exists");
        assert_eq!(
            snapshot.captured_conversation.as_deref(),
            Some("conv-second"),
            "the identity the agent discarded must stop being offered"
        );
        let state = sup.sessions.lock().await[&auth.session_id]
            .capture
            .lock()
            .expect("capture mutex poisoned")
            .clone();
        assert!(
            matches!(
                &state,
                CaptureState::Reported { conversation } if conversation == "conv-second"
            ),
            "and the in-memory mirror must follow the durable write: {state:?}"
        );
    }

    /// A supervisor that may not record refuses the report and changes
    /// NOTHING — not the row, not the in-memory state.
    ///
    /// `may_record` is false for a supervisor that does not hold its state
    /// directory's claim or could not read this host's boot id: it can
    /// still answer honestly from what is stored, but it has no standing
    /// to draw new conclusions. A report accepted in that state would be
    /// exactly such a conclusion. Advancing memory without the write would
    /// be worse still — `Reported` advertises `Resume`, which promises a
    /// restart there is a stored id to fill in, and there would not be.
    ///
    /// Nothing retries it: neither vendor re-fires the hook, so the report
    /// is simply lost and the scan remains the fallback for that session,
    /// which it still is precisely because this refusal left the state
    /// unsettled.
    #[tokio::test]
    async fn a_report_is_refused_and_dropped_while_the_supervisor_is_not_recording() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let auth = reporting_session(&sup, "reporting-session").await;
        sup.may_record
            .store(false, std::sync::atomic::Ordering::SeqCst);

        let reply = send_report(&sup, &auth, 69, "conv-unrecorded").await;
        let ControlMsg::Error {
            req_id,
            kind,
            message,
        } = reply
        else {
            panic!("a supervisor that may not record must refuse the report: {reply:?}");
        };
        assert_eq!(req_id, 69);
        assert_eq!(kind, ErrorKind::Internal);
        assert_eq!(message, "the supervisor is not recording right now");
        assert_eq!(
            sup.session_snapshot(&auth.session_id)
                .await
                .unwrap()
                .expect("the session still exists")
                .captured_conversation,
            None,
            "nothing may be written by a supervisor with no standing to write"
        );
        let capture = sup.sessions.lock().await[&auth.session_id]
            .capture
            .lock()
            .expect("capture mutex poisoned")
            .clone();
        assert!(
            matches!(capture, CaptureState::Unclaimed),
            "and the session must stay under the scan's authority: {capture:?}"
        );
    }

    /// A report whose durable write FAILS is refused, and leaves both the
    /// row and the in-memory state exactly as they were.
    ///
    /// The invariant is the same one every capture write carries: the
    /// DURABLE write decides what is claimed, so an in-memory `Reported`
    /// installed after a failed write would advertise `RestartOffer::Resume`
    /// — a promise that a restart has a stored id to substitute — with
    /// nothing behind it. The session must instead stay under the scan's
    /// authority, which is precisely what not advancing achieves.
    ///
    /// Driven through the capture store-fault seam rather than a genuinely
    /// broken database, because what is under test is this handler's
    /// response to a failed write, not SQLite's behaviour. `Internal`
    /// rather than `Conflict` is the reply, because unlike a stale
    /// generation something really did malfunction.
    #[tokio::test]
    async fn a_report_whose_write_fails_changes_nothing_and_answers_internal() {
        let state = StateDir::new();
        let attempts: Arc<std::sync::Mutex<Vec<crate::service::CaptureWrite>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = Arc::clone(&attempts);
        let fault: crate::service::CaptureStoreFault = Arc::new(move |write, _id| {
            seen.lock().expect("fault log poisoned").push(write);
            Err(anyhow::anyhow!("the store is unavailable"))
        });
        let sup = Supervisor::new_with_seams(
            state.path(),
            dummy_exe(),
            crate::service::core::SupervisorTimeouts::default(),
            crate::service::core::SupervisorSeams {
                capture_store_fault: Some(fault),
                ..crate::service::core::SupervisorSeams::default()
            },
        )
        .await
        .expect("supervisor");
        let auth = reporting_session(&sup, "reporting-session").await;

        let reply = send_report(&sup, &auth, 70, "conv-unwritable").await;
        let ControlMsg::Error { req_id, kind, .. } = reply else {
            panic!("a report whose write failed must be refused: {reply:?}");
        };
        assert_eq!(req_id, 70);
        assert_eq!(kind, ErrorKind::Internal);
        assert_eq!(
            attempts.lock().expect("fault log poisoned").as_slice(),
            [crate::service::CaptureWrite::Report],
            "the report's own write is the one that was attempted"
        );
        assert_eq!(
            sup.session_snapshot(&auth.session_id)
                .await
                .unwrap()
                .expect("the session still exists")
                .captured_conversation,
            None,
            "a failed write must leave the column untouched"
        );
        let capture = sup.sessions.lock().await[&auth.session_id]
            .capture
            .lock()
            .expect("capture mutex poisoned")
            .clone();
        assert!(
            matches!(capture, CaptureState::Unclaimed),
            "and must not install a Reported state with nothing behind it: {capture:?}"
        );
    }

    /// A report is answered while the session's LIFECYCLE CLAIM is held by
    /// somebody else.
    ///
    /// This is the design's sharpest timing constraint, and it is invisible
    /// in the code — the report path is defined by a lock it does NOT take.
    /// Claude's hook fires at the replacement process's startup, which is
    /// inside `restart_session`'s claim; a report that waited on that claim
    /// would queue behind the tail of the very restart that caused it, blow
    /// the hook's 2 s budget, and surface to the user as a hook error the
    /// vendor never retries.
    ///
    /// Holding the claim from the test and requiring the reply BEFORE
    /// releasing it is the only way to state that as a test: a version that
    /// took the claim would deadlock here rather than fail an assertion,
    /// which is why the reply is awaited inside the guard's scope instead
    /// of after it.
    ///
    /// That deadlock is the reason for the timeout around the round trip.
    /// The regression this test exists to catch does not produce a wrong
    /// answer, it produces NO answer, and an un-timed await on it hangs the
    /// whole test binary until CI's job limit kills it with no indication
    /// of which test was at fault. The bound is deliberately generous — the
    /// path under test does no I/O beyond one SQLite write, so seconds are
    /// orders of magnitude of headroom, and nothing here is a latency
    /// assertion.
    #[tokio::test]
    async fn a_report_is_answered_while_the_session_lifecycle_claim_is_held() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let auth = reporting_session(&sup, "reporting-session").await;

        let held = sup.lifecycle_locks.claim(&auth.session_id).await;
        let reply = tokio::time::timeout(
            Duration::from_secs(30),
            send_report(&sup, &auth, 71, "conv-during-restart"),
        )
        .await
        .expect(
            "a report must not wait on the session lifecycle claim; this timing out means the \
             report path started taking it and would otherwise deadlock the suite",
        );
        assert!(
            matches!(reply, ControlMsg::ConversationReported { req_id: 71 }),
            "a report must not wait on the claim a restart holds: {reply:?}"
        );
        drop(held);
        assert_eq!(
            sup.session_snapshot(&auth.session_id)
                .await
                .unwrap()
                .expect("the session exists")
                .captured_conversation
                .as_deref(),
            Some("conv-during-restart"),
            "and it must have written, not merely replied"
        );
    }

    /// The hook's `source` is bounded and de-controlled before it reaches a
    /// log line, and doing so never costs the report.
    ///
    /// The field is unvalidated by design — a vendor may add an event name
    /// at any time, and refusing an unrecognized one would throw away a
    /// good report over a diagnostic string — so whatever a
    /// credential-holding process in the agent's tree sends ends up in the
    /// supervisor's log. Unbounded, that is an unbounded write target;
    /// with control characters intact, a newline forges log ENTRIES and an
    /// escape sequence repaints the operator's terminal.
    ///
    /// The multibyte case is not decoration: truncation has to land on a
    /// character boundary, because slicing a `String` mid-UTF-8 panics and
    /// this input is attacker-chosen. A cap enforced with `&s[..N]` would
    /// turn a log-hygiene measure into a remote panic.
    #[test]
    fn a_reported_source_is_bounded_and_stripped_of_control_characters() {
        assert_eq!(sanitized_source("startup"), "startup");
        assert_eq!(
            sanitized_source("start\nup\u{1b}[2J"),
            "start\u{fffd}up\u{fffd}[2J",
            "newlines and escapes must not survive into a line-oriented log"
        );
        assert!(
            sanitized_source(&"a".repeat(4096)).len() <= MAX_SOURCE_BYTES,
            "an unbounded source must not become an unbounded log line"
        );
        // Four-byte characters, so a byte-indexed cap would land inside one.
        let wide = sanitized_source(&"🙂".repeat(64));
        assert!(wide.len() <= MAX_SOURCE_BYTES);
        assert_eq!(
            wide.chars().count(),
            MAX_SOURCE_BYTES / 4,
            "truncation lands on a character boundary rather than splitting one"
        );
    }

    /// The other half of the same rule, and the half the pure-function test
    /// cannot state: a hostile `source` driven through the REAL dispatch
    /// costs the report nothing.
    ///
    /// Sanitizing rather than refusing is a deliberate choice, and a choice
    /// is only real if something holds the other side of it. The pure test
    /// above proves the string is cleaned; it says nothing about whether
    /// the handler that cleans it also decides to reject the frame. A
    /// future reviewer looking at a field full of newlines, escapes, and
    /// four kilobytes of padding would find refusing it the obvious move —
    /// and refusing would trade a correct resume for a tidy log, on a path
    /// the vendor never retries.
    ///
    /// So the assertion is on the identity, not on the log: the report is
    /// ACCEPTED, and the conversation lands durably where a restart reads
    /// it. Whether the log line itself is clean is the pure function's
    /// business, tested without a tracing subscriber a few lines up. One
    /// value carries all three hostilities at once — a newline that would
    /// forge a log entry, an escape that would repaint a terminal, and
    /// enough bytes to blow the cap — because the property is that NONE of
    /// them can reject a report, and separate cases would only make it
    /// easier to fix one and lose the others.
    #[tokio::test]
    async fn a_hostile_report_source_is_sanitized_without_costing_the_report() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let auth = reporting_session(&sup, "reporting-session").await;

        let hostile = format!("start\nup\u{1b}[2J{}", "🙂".repeat(1024));
        let reply = send_report_with_source(&sup, &auth, 79, "conv-hostile-source", &hostile).await;
        assert!(
            matches!(reply, ControlMsg::ConversationReported { req_id: 79 }),
            "a diagnostic field's shape must never decide a report's fate: {reply:?}"
        );
        assert_eq!(
            sup.session_snapshot(&auth.session_id)
                .await
                .unwrap()
                .expect("the session exists")
                .captured_conversation
                .as_deref(),
            Some("conv-hostile-source"),
            "and the identity must have been written, not merely acknowledged"
        );
        let capture = sup
            .sessions
            .lock()
            .await
            .get(&auth.session_id)
            .expect("the entry is still published")
            .capture
            .lock()
            .expect("capture mutex poisoned")
            .clone();
        assert!(
            matches!(
                &capture,
                CaptureState::Reported { conversation } if conversation == "conv-hostile-source"
            ),
            "the in-memory mirror must follow the accepted report too: {capture:?}"
        );
    }

    /// A report that arrives before the session's entry is published is
    /// ACCEPTED, writing the durable row from the row's own generation.
    ///
    /// The gap is real and ordinary rather than a corner: Claude's hook
    /// fires at process start, which lands inside the window a create leaves
    /// between committing the reserved row and publishing the in-memory
    /// entry, and inside the equivalent window a relaunch leaves. Answering
    /// `NotFound` there would discard the report the vendor considers most
    /// reliable, and nothing would re-send it.
    ///
    /// Nothing in memory is advanced, because there is nothing to advance.
    /// The entry published afterwards does NOT read these columns — the
    /// create path mints `Unclaimed` — so the row and the mirror diverge for
    /// a while; `Supervisor::report_conversation`'s docs set out the three
    /// things that make that divergence harmless and self-correcting. The
    /// assertion is therefore on the row, which is what every decision that
    /// acts on the identity reads, a restart included.
    ///
    /// The entry is removed after seeding rather than never created,
    /// because the durable ROW is what makes the fallback possible and
    /// `reporting_session` is the one helper that seeds a row a report can
    /// legitimately claim.
    #[tokio::test]
    async fn a_report_during_the_publication_gap_is_written_from_the_row() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let auth = reporting_session(&sup, "reporting-session").await;
        sup.sessions
            .lock()
            .await
            .remove(&auth.session_id)
            .expect("test premise: the entry existed before the gap was simulated");

        let reply = send_report(&sup, &auth, 73, "conv-early").await;
        assert!(
            matches!(reply, ControlMsg::ConversationReported { req_id: 73 }),
            "a report arriving before publication must be accepted, not lost: {reply:?}"
        );
        assert_eq!(
            sup.session_snapshot(&auth.session_id)
                .await
                .unwrap()
                .expect("the session exists")
                .captured_conversation
                .as_deref(),
            Some("conv-early"),
            "the durable row is what carries the report until an entry appears"
        );
        assert!(
            !sup.sessions.lock().await.contains_key(&auth.session_id),
            "and nothing may fabricate an entry to hold it in the meantime"
        );
    }

    /// A report from a launch the session has already moved past is
    /// refused as a CONFLICT, and changes nothing.
    ///
    /// The generation fence is what stands in for the lifecycle claim this
    /// path deliberately does not take (see
    /// `Supervisor::report_conversation`): a session's credential survives
    /// a relaunch, so a hook process that outlived the restart's kill sweep
    /// still authenticates. It simply cannot write, because the generation
    /// it belongs to is gone.
    ///
    /// `Conflict` rather than `Internal` because nothing malfunctioned —
    /// the surviving launch's own hook is the one entitled to speak for this
    /// session now. The entry is left at the OLD generation on purpose, which
    /// is exactly the state a stale hook's report is judged against.
    #[tokio::test]
    async fn a_report_for_a_superseded_launch_is_refused_as_a_conflict() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let auth = reporting_session(&sup, "reporting-session").await;
        // The row moves to generation 1 while the published entry — and so
        // the report that reads its generation — stays at 0.
        let claim = sup
            .store
            .begin_relaunch(
                &auth.session_id,
                crate::store::OfferBasis {
                    captured_conversation: None,
                    capture_ambiguous: false,
                },
                true,
                false,
            )
            .await
            .expect("open a new generation");
        assert!(
            matches!(claim, crate::store::RelaunchDecision::Claimed(_)),
            "test premise: the relaunch must actually have opened generation 1"
        );

        let reply = send_report(&sup, &auth, 74, "conv-stale").await;
        let ControlMsg::Error { req_id, kind, .. } = reply else {
            panic!("a report for a superseded launch must be refused: {reply:?}");
        };
        assert_eq!(req_id, 74);
        assert_eq!(kind, ErrorKind::Conflict);
        assert_eq!(
            sup.session_snapshot(&auth.session_id)
                .await
                .unwrap()
                .expect("the session exists")
                .captured_conversation,
            None,
            "a fenced-out report may not reach the column"
        );
    }

    /// An id of exactly the cap is ACCEPTED and stored.
    ///
    /// The refusal table above pins one byte past the bound; this pins the
    /// bound itself, which is the half an off-by-one silently breaks. The
    /// direction matters: a cap that refused at exactly 128 would not fail
    /// loudly anywhere — the session would simply stop being resumable the
    /// day a vendor's id format grew, and the scan fallback would cover for
    /// it convincingly enough that nobody would look here.
    #[tokio::test]
    async fn a_report_at_exactly_the_byte_cap_is_accepted() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let auth = reporting_session(&sup, "reporting-session").await;

        let at_cap = "a".repeat(MAX_CONVERSATION_BYTES);
        let reply = send_report(&sup, &auth, 72, &at_cap).await;
        assert!(
            matches!(reply, ControlMsg::ConversationReported { req_id: 72 }),
            "an id of exactly {MAX_CONVERSATION_BYTES} bytes is inside the bound: {reply:?}"
        );
        assert_eq!(
            sup.session_snapshot(&auth.session_id)
                .await
                .unwrap()
                .expect("the session exists")
                .captured_conversation
                .as_deref(),
            Some(at_cap.as_str())
        );
    }

    /// An admitted spawn receives bounded idempotency, preserves its direct
    /// parent, replays an identical key, conflicts if only that parent
    /// intent changes, and may reuse the key after its child is deleted.
    #[tokio::test]
    async fn restricted_create_is_parented_session_lifetime_and_parent_sensitive() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let auth = authenticated_parent(&sup, state.path(), "parent-session").await;
        let create = |req_id, parent: Option<&str>| ControlMsg::CreateSession {
            req_id,
            parent: parent.map(str::to_string),
            cwd: state.path().to_string_lossy().into_owned(),
            invocation: None,
            profile_id: None,
            profile_name: None,
            title: Some("spawned child".to_string()),
            cols: 80,
            rows: 24,
            intent_key: Some("spawn-key".to_string()),
            agent_kind: None,
            resume_template: None,
        };
        let mut send = async |msg| {
            handle_restricted_control(&sup, msg, &tx, &auth).await;
            serde_json::from_slice::<ControlMsg>(&rx.recv().await.expect("create reply").body)
                .expect("decode create reply")
        };

        let first = send(create(51, Some("parent-session"))).await;
        let ControlMsg::SessionCreated { session, .. } = first else {
            panic!("a valid restricted create must succeed: {first:?}");
        };
        assert_eq!(session.parent.as_deref(), Some("parent-session"));
        assert_eq!(
            sup.store
                .reservation("spawn-key")
                .await
                .unwrap()
                .expect("key is durable while the child exists")
                .dedup_scope,
            DedupScope::SessionLifetime
        );

        let replay = send(create(52, Some("parent-session"))).await;
        let ControlMsg::SessionCreated {
            session: replayed, ..
        } = replay
        else {
            panic!("an identical key must replay: {replay:?}");
        };
        assert_eq!(replayed.id, session.id);

        let conflict = send(create(53, None)).await;
        assert!(matches!(
            conflict,
            ControlMsg::Error {
                req_id: 53,
                kind: ErrorKind::Conflict,
                ..
            }
        ));

        // Model the completed-delete boundary: the durable transaction has
        // removed both bounded records, and the published map follows it.
        sup.store
            .delete_session_settling_reservations(&session.id)
            .await
            .expect("delete the first child and release its key");
        sup.sessions.lock().await.remove(&session.id);

        let replacement = send(create(54, Some("parent-session"))).await;
        let ControlMsg::SessionCreated {
            session: replacement,
            ..
        } = replacement
        else {
            panic!("a deleted child's key must create a fresh child: {replacement:?}");
        };
        assert_ne!(replacement.id, session.id);
        assert_eq!(
            sup.store
                .reservation("spawn-key")
                .await
                .unwrap()
                .expect("the replacement owns the reused key")
                .session_id,
            replacement.id
        );
    }

    /// A keyed selectorless spawn binds the profile chosen by its first
    /// attempt, even when a later create changes the host's last-used source.
    ///
    /// Ambient default changes are exactly why replay resolves from the
    /// reservation and stored child rather than validating the selectorless
    /// request again. Re-deriving here would either launch a second child or
    /// turn an identical retry into a conflict after the user used another
    /// profile elsewhere.
    #[tokio::test]
    async fn selectorless_spawn_replays_its_child_after_the_host_default_changes() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let auth = authenticated_parent(&sup, state.path(), "default-source-parent").await;
        let original_source = sup
            .store
            .latest_source_profile()
            .await
            .expect("read original default")
            .expect("the authenticated parent is profile-backed");
        let request = |req_id| ControlMsg::CreateSession {
            req_id,
            parent: None,
            cwd: state.path().to_string_lossy().into_owned(),
            invocation: None,
            profile_id: None,
            profile_name: None,
            title: Some("selectorless child".to_string()),
            cols: 80,
            rows: 24,
            intent_key: Some("selectorless-replay-key".to_string()),
            agent_kind: None,
            resume_template: None,
        };

        handle_restricted_control(&sup, request(71), &tx, &auth).await;
        let first: ControlMsg =
            serde_json::from_slice(&rx.recv().await.expect("first create reply").body).unwrap();
        let ControlMsg::SessionCreated { session: child, .. } = first else {
            panic!("the first selectorless create must succeed: {first:?}");
        };
        assert_eq!(
            child.source_profile.as_ref().map(|source| &source.id),
            Some(&original_source.id)
        );

        let newer_profile = match sup
            .store
            .create_profile(
                "New host default".to_string(),
                "agent".to_string(),
                AgentKind::Generic,
                None,
            )
            .await
            .expect("create a different profile")
        {
            ProfileCreation::Created(profile) => profile,
            other => panic!("the catalog has room for a test profile: {other:?}"),
        };
        sup.store
            .insert_session(
                crate::store::StoredSession {
                    conversation_source: None,
                    id: "new-default-source".to_string(),
                    parent: None,
                    archived: false,
                    title: "new default source".to_string(),
                    created_at: crate::store::now_unix(),
                    last_activity_at: crate::store::now_unix(),
                    creation_seq: 0,
                    cwd: state.path().to_string_lossy().into_owned(),
                    invocation: "agent".to_string(),
                    tmux_name: "fh-new-default-source".to_string(),
                    pane: String::new(),
                    outcome: LastOutcome::Launching,
                    agent_kind: AgentKind::Generic,
                    resume_template: None,
                    canonical_cwd: None,
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                    source_profile: Some(crate::store::ProfileSnapshot {
                        id: newer_profile.id.clone(),
                        name: newer_profile.name,
                    }),
                },
                None,
            )
            .await
            .expect("record a newer profile-backed create");
        assert_eq!(
            sup.store
                .latest_source_profile()
                .await
                .unwrap()
                .expect("new default")
                .id,
            newer_profile.id,
            "premise: the host default changed before the retry"
        );

        handle_restricted_control(&sup, request(72), &tx, &auth).await;
        let replay: ControlMsg =
            serde_json::from_slice(&rx.recv().await.expect("replay reply").body).unwrap();
        let ControlMsg::SessionCreated {
            session: replayed, ..
        } = replay
        else {
            panic!("the identical selectorless key must replay: {replay:?}");
        };
        assert_eq!(replayed.id, child.id);
        assert_eq!(
            replayed.source_profile.as_ref().map(|source| &source.id),
            Some(&original_source.id),
            "replay returns the profile resolution captured by the original child"
        );
    }

    /// All three source-profile existence states, derived through a REAL
    /// `ListSessions` reply (PLAN_M6_75.md item 5).
    ///
    /// Existence is the one part of `SourceProfile` that is not stored, so
    /// the only way to be wrong about it is to derive it wrongly — and the
    /// three cases fail differently: a missed DELETED renders a profile
    /// that is gone as if it were still there, a missed RENAMED implies the
    /// snapshotted name is current (SPEC.md's snapshot rule says it is
    /// not), and a wrongly-flagged PRESENT would mark every ordinary
    /// session as broken. All three ride one reply because that is also
    /// what pins the BATCH: one catalog read answering a whole page, with
    /// each row resolved against its own id.
    ///
    /// The raw-created session in the mix is not filler either — it is the
    /// case every pre-M6.75 session is, and it must stay `None` rather than
    /// acquiring some default.
    ///
    /// Session entries are built by hand rather than launched: what is
    /// under test is the reply-build derivation, and three real tmux
    /// launches would test tmux.
    #[tokio::test]
    async fn a_list_reply_derives_present_renamed_and_deleted_source_profiles() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");

        // Three profiles and the three fates they meet. The snapshots below
        // record each one's name AS IT WAS, which is what the derivation
        // compares against.
        let mut profiles = Vec::new();
        for name in ["Kept", "Renamed later", "Deleted later"] {
            profiles.push(
                match sup
                    .store
                    .create_profile(
                        name.to_string(),
                        "bash".to_string(),
                        AgentKind::Generic,
                        None,
                    )
                    .await
                    .expect("create")
                {
                    crate::store::ProfileCreation::Created(profile) => profile,
                    other => panic!("the catalog is not full: {other:?}"),
                },
            );
        }
        for (index, profile) in profiles.iter().enumerate() {
            let snapshotted = Some(farhelm_proto::SourceProfile {
                id: profile.id.clone(),
                name: profile.name.clone(),
                // Deliberately the WRONG answer for two of the three: this
                // is the placeholder an entry carries, and a reply that
                // echoed it instead of deriving would pass every other
                // assertion here.
                existence: farhelm_proto::ProfileExistence::Present,
            });
            let id = format!("s{index}");
            sup.sessions.lock().await.insert(
                id.clone(),
                Arc::new(SessionEntry {
                    info: SessionInfo {
                        parent: None,
                        archived: false,
                        id: id.clone(),
                        title: id.clone(),
                        // Descending creation order is what `list_page`
                        // walks, so a later index must sort later.
                        created_at: 1_700_000_000 - index as i64,
                        last_activity_at: 1_700_000_000 - index as i64,
                        creation_seq: None,
                        cwd: "/tmp".to_string(),
                        invocation: "bash".to_string(),
                        status: SessionStatus::default(),
                        annotation: None,
                        restart_offer: RestartOffer::default(),
                        tabs: Vec::new(),
                        source_profile: snapshotted,
                    },
                    // No terminal, so the reply needs nothing from tmux.
                    terminal: None,
                    outcome: Arc::new(std::sync::Mutex::new(LastOutcome::Exited {
                        exit_code: Some(0),
                        annotation: None,
                    })),
                    snapshot: IntegrationSnapshot {
                        kind: AgentKind::Generic,
                        resume_template: None,
                    },
                    canonical_cwd: None,
                    first_input: Arc::new(std::sync::Mutex::new(FirstInput {
                        at: None,
                        durable: true,
                    })),
                    capture: Arc::new(std::sync::Mutex::new(CaptureState::Unclaimed)),
                    hooked: crate::service::core::hook_flag(false),
                    hook_warned: crate::service::core::hook_flag(false),
                    activity: crate::service::ticker::ActivitySample::unsampled(),
                    last_activity_at: crate::service::core::activity_stamp(
                        1_700_000_000 - index as i64,
                    ),
                    generation: 0,
                    scope: None,
                }),
            );
        }
        // A raw-created session beside them.
        sup.sessions.lock().await.insert(
            "s3".to_string(),
            Arc::new(SessionEntry {
                info: SessionInfo {
                    parent: None,
                    archived: false,
                    id: "s3".to_string(),
                    title: "s3".to_string(),
                    created_at: 1_699_999_997,
                    last_activity_at: 1_699_999_997,
                    creation_seq: None,
                    cwd: "/tmp".to_string(),
                    invocation: "bash".to_string(),
                    status: SessionStatus::default(),
                    annotation: None,
                    restart_offer: RestartOffer::default(),
                    tabs: Vec::new(),
                    source_profile: None,
                },
                terminal: None,
                outcome: Arc::new(std::sync::Mutex::new(LastOutcome::Exited {
                    exit_code: Some(0),
                    annotation: None,
                })),
                snapshot: IntegrationSnapshot {
                    kind: AgentKind::Generic,
                    resume_template: None,
                },
                canonical_cwd: None,
                first_input: Arc::new(std::sync::Mutex::new(FirstInput {
                    at: None,
                    durable: true,
                })),
                capture: Arc::new(std::sync::Mutex::new(CaptureState::Unclaimed)),
                hooked: crate::service::core::hook_flag(false),
                hook_warned: crate::service::core::hook_flag(false),
                activity: crate::service::ticker::ActivitySample::unsampled(),
                last_activity_at: crate::service::core::activity_stamp(1_699_999_997),
                generation: 0,
                scope: None,
            }),
        );

        // The catalog moves out from under the snapshots.
        sup.store
            .update_profile(farhelm_proto::Profile {
                name: "Renamed now".to_string(),
                ..profiles[1].clone()
            })
            .await
            .expect("rename")
            .expect("the profile is there to rename");
        assert!(
            sup.store
                .delete_profile(&profiles[2].id)
                .await
                .expect("delete")
        );

        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        handle_control(
            &sup,
            ControlMsg::ListSessions {
                req_id: 9,
                cursor: None,
                limit: None,
            },
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;
        // Spawned, so the reply is awaited rather than polled — see the
        // `ListSessions` arm's own comment.
        let reply = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("spawned ListSessions handler never replied")
            .expect("reply channel closed before a reply arrived");
        let ControlMsg::SessionList { sessions, .. } =
            serde_json::from_slice(&reply.body).expect("decode")
        else {
            panic!("expected a session list");
        };
        let by_id: HashMap<&str, Option<&farhelm_proto::SourceProfile>> = sessions
            .iter()
            .map(|s| (s.id.as_str(), s.source_profile.as_ref()))
            .collect();

        assert_eq!(
            by_id["s0"].map(|p| p.existence),
            Some(farhelm_proto::ProfileExistence::Present)
        );
        assert_eq!(
            by_id["s1"].map(|p| p.existence),
            Some(farhelm_proto::ProfileExistence::Renamed)
        );
        assert_eq!(
            by_id["s1"].map(|p| p.name.as_str()),
            Some("Renamed later"),
            "a renamed profile's sessions keep the name they snapshotted (SPEC.md's snapshot \
             rule); the CURRENT name is deliberately not on this wire"
        );
        assert_eq!(
            by_id["s2"].map(|p| p.existence),
            Some(farhelm_proto::ProfileExistence::Deleted)
        );
        assert_eq!(
            by_id["s2"].map(|p| p.name.as_str()),
            Some("Deleted later"),
            "and a deleted profile's sessions still filter under theirs"
        );
        assert_eq!(
            by_id["s3"], None,
            "a raw-created session names no profile and must not acquire one"
        );
    }

    /// Existence is derived per profile ID, so two profiles sharing a NAME
    /// meet their own fates.
    ///
    /// Profile names are not unique and nothing anywhere makes them so —
    /// `create_profile` mints an id and never looks at the name, and SPEC.md
    /// gives no uniqueness rule — so "Claude Code" twice is an ordinary
    /// catalog, not a contrived one. Every other fixture for this derivation
    /// happens to use distinct names, which means a name-keyed lookup would
    /// pass all of them: delete one of two same-named profiles and every
    /// session created from EITHER reads `Deleted`, or none does, depending
    /// on which way the lookup fell.
    ///
    /// The failure that produces is quiet and permanent — a session is
    /// labelled as coming from a profile that still exists, or a live
    /// profile's sessions are marked orphaned — and it is exactly the kind
    /// of thing a user creates by duplicating a profile to tweak it.
    #[tokio::test]
    async fn two_profiles_sharing_a_name_derive_their_existence_separately() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");

        // The SAME name twice: the catalog allows it, so the derivation has
        // to tell them apart by something else.
        let mut twins = Vec::new();
        for _ in 0..2 {
            twins.push(
                match sup
                    .store
                    .create_profile(
                        "Claude Code".to_string(),
                        "bash".to_string(),
                        AgentKind::Generic,
                        None,
                    )
                    .await
                    .expect("create")
                {
                    crate::store::ProfileCreation::Created(profile) => profile,
                    other => panic!("the catalog is not full: {other:?}"),
                },
            );
        }
        assert_ne!(twins[0].id, twins[1].id, "premise: two distinct profiles");

        for (index, profile) in twins.iter().enumerate() {
            let id = format!("s{index}");
            sup.sessions.lock().await.insert(
                id.clone(),
                Arc::new(SessionEntry {
                    info: SessionInfo {
                        parent: None,
                        archived: false,
                        id: id.clone(),
                        title: id.clone(),
                        created_at: 1_700_000_000 - index as i64,
                        last_activity_at: 1_700_000_000 - index as i64,
                        creation_seq: None,
                        cwd: "/tmp".to_string(),
                        invocation: "bash".to_string(),
                        status: SessionStatus::default(),
                        annotation: None,
                        restart_offer: RestartOffer::default(),
                        tabs: Vec::new(),
                        source_profile: Some(farhelm_proto::SourceProfile {
                            id: profile.id.clone(),
                            name: profile.name.clone(),
                            existence: farhelm_proto::ProfileExistence::Present,
                        }),
                    },
                    terminal: None,
                    outcome: Arc::new(std::sync::Mutex::new(LastOutcome::Exited {
                        exit_code: Some(0),
                        annotation: None,
                    })),
                    snapshot: IntegrationSnapshot {
                        kind: AgentKind::Generic,
                        resume_template: None,
                    },
                    canonical_cwd: None,
                    first_input: Arc::new(std::sync::Mutex::new(FirstInput {
                        at: None,
                        durable: true,
                    })),
                    capture: Arc::new(std::sync::Mutex::new(CaptureState::Unclaimed)),
                    hooked: crate::service::core::hook_flag(false),
                    hook_warned: crate::service::core::hook_flag(false),
                    activity: crate::service::ticker::ActivitySample::unsampled(),
                    last_activity_at: crate::service::core::activity_stamp(
                        1_700_000_000 - index as i64,
                    ),
                    generation: 0,
                    scope: None,
                }),
            );
        }

        // Exactly one of the twins goes away.
        assert!(
            sup.store
                .delete_profile(&twins[0].id)
                .await
                .expect("delete")
        );

        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        handle_control(
            &sup,
            ControlMsg::ListSessions {
                req_id: 1,
                cursor: None,
                limit: None,
            },
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;
        let reply = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("spawned ListSessions handler never replied")
            .expect("reply channel closed before a reply arrived");
        let ControlMsg::SessionList { sessions, .. } =
            serde_json::from_slice(&reply.body).expect("decode")
        else {
            panic!("expected a session list");
        };
        let by_id: HashMap<&str, Option<&farhelm_proto::SourceProfile>> = sessions
            .iter()
            .map(|s| (s.id.as_str(), s.source_profile.as_ref()))
            .collect();

        assert_eq!(
            by_id["s0"].map(|p| p.existence),
            Some(farhelm_proto::ProfileExistence::Deleted),
            "the session from the DELETED twin must say so"
        );
        assert_eq!(
            by_id["s1"].map(|p| p.existence),
            Some(farhelm_proto::ProfileExistence::Present),
            "and the surviving twin's session must not be dragged down with it by a name-keyed \
             lookup"
        );
    }

    /// What a list reply COSTS in catalog reads: exactly one for a page
    /// that needs the catalog, and exactly zero for one that does not
    /// (PLAN_M6_75.md item 5).
    ///
    /// This is a performance contract with a correctness-shaped failure. A
    /// per-session lookup is invisible in every other test — the answers
    /// are identical — and turns one small query per reply into one per
    /// row, on the path a fleet's whole session list is served from. The
    /// zero case matters at least as much: every session predating this
    /// feature is raw-created, so the overwhelmingly common page must not
    /// pay for a catalog nothing on it references.
    ///
    /// The third case is the failure mode, and it is here rather than in
    /// its own test because it is the same seam: a catalog that cannot be
    /// read FAILS the reply instead of degrading to an empty map. An empty
    /// map is indistinguishable from "every profile was deleted", so
    /// degrading would render a transient database error as a page of
    /// sessions whose profiles are all gone — a specific, alarming lie
    /// about durable state, in place of an error the next list retries.
    #[tokio::test]
    async fn a_list_reads_the_catalog_once_never_per_session_and_never_optionally() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        let mut list = async |req_id, rx: &mut mpsc::Receiver<Frame>| -> ControlMsg {
            handle_control(
                &sup,
                ControlMsg::ListSessions {
                    req_id,
                    cursor: None,
                    limit: None,
                },
                ConnectionCtx {
                    tx: &tx,
                    priority: &tx,
                    input_routes: &mut input_routes,
                    upload_routes: &mut no_uploads(),
                    tasks: &mut tasks,
                },
            )
            .await;
            let reply = tokio::time::timeout(Duration::from_secs(5), rx.recv())
                .await
                .expect("spawned ListSessions handler never replied")
                .expect("reply channel closed before a reply arrived");
            serde_json::from_slice(&reply.body).expect("decode")
        };

        // Three sessions from ONE profile, so a per-row lookup and a
        // per-reply read differ by two.
        let profile = match sup
            .store
            .create_profile(
                "Shared".to_string(),
                "bash".to_string(),
                AgentKind::Generic,
                None,
            )
            .await
            .expect("create the profile")
        {
            crate::store::ProfileCreation::Created(profile) => profile,
            other => panic!("the catalog is not full: {other:?}"),
        };
        let entry = |id: &str, source: Option<&farhelm_proto::Profile>| {
            Arc::new(SessionEntry {
                info: SessionInfo {
                    parent: None,
                    archived: false,
                    id: id.to_string(),
                    title: id.to_string(),
                    created_at: 1_700_000_000,
                    last_activity_at: 1_700_000_000,
                    creation_seq: None,
                    cwd: "/tmp".to_string(),
                    invocation: "bash".to_string(),
                    status: SessionStatus::default(),
                    annotation: None,
                    restart_offer: RestartOffer::default(),
                    tabs: Vec::new(),
                    source_profile: source.map(|profile| farhelm_proto::SourceProfile {
                        id: profile.id.clone(),
                        name: profile.name.clone(),
                        existence: farhelm_proto::ProfileExistence::Present,
                    }),
                },
                terminal: None,
                outcome: Arc::new(std::sync::Mutex::new(LastOutcome::Exited {
                    exit_code: Some(0),
                    annotation: None,
                })),
                snapshot: IntegrationSnapshot {
                    kind: AgentKind::Generic,
                    resume_template: None,
                },
                canonical_cwd: None,
                first_input: Arc::new(std::sync::Mutex::new(FirstInput {
                    at: None,
                    durable: true,
                })),
                capture: Arc::new(std::sync::Mutex::new(CaptureState::Unclaimed)),
                hooked: crate::service::core::hook_flag(false),
                hook_warned: crate::service::core::hook_flag(false),
                activity: crate::service::ticker::ActivitySample::unsampled(),
                last_activity_at: crate::service::core::activity_stamp(1_700_000_000),
                generation: 0,
                scope: None,
            })
        };

        // A page of raw-created sessions only: no catalog read at all.
        for id in ["r1", "r2", "r3"] {
            sup.sessions
                .lock()
                .await
                .insert(id.to_string(), entry(id, None));
        }
        let before = sup.store.profile_name_reads();
        list(1, &mut rx).await;
        assert_eq!(
            sup.store.profile_name_reads(),
            before,
            "a page where no session names a profile must not read the catalog at all"
        );

        // Add the profile-created ones: one read for the whole page.
        for id in ["p1", "p2", "p3"] {
            sup.sessions
                .lock()
                .await
                .insert(id.to_string(), entry(id, Some(&profile)));
        }
        let before = sup.store.profile_name_reads();
        let reply = list(2, &mut rx).await;
        assert_eq!(
            sup.store.profile_name_reads(),
            before + 1,
            "one read per REPLY, not one per profile-created session"
        );
        let ControlMsg::SessionList { sessions, .. } = reply else {
            panic!("expected a session list");
        };
        assert_eq!(
            sessions.len(),
            6,
            "premise: all six sessions are on the page"
        );

        // And with the catalog unreadable, the whole request fails rather
        // than reporting six sessions whose profiles are all gone.
        sup.store.drop_profile_catalog_for_test().await;
        let ControlMsg::Error { req_id, kind, .. } = list(3, &mut rx).await else {
            panic!("an unreadable catalog must fail the list, not degrade it");
        };
        assert_eq!(req_id, 3);
        assert_eq!(kind, ErrorKind::Internal);
    }

    /// The same refusal on the single-session reply path (`RenameSession`),
    /// which reads the catalog for ONE id rather than in bulk.
    ///
    /// A separate path with the same rule, and the one where degrading
    /// would be most tempting: the rename itself has already succeeded by
    /// the time the reply is assembled, so answering with a slightly-wrong
    /// `SessionInfo` looks like the kind thing to do. It is not — the
    /// wrong field would say a profile the user still has was deleted —
    /// and the handler's own contract is that a mutation's reply is the
    /// authoritative answer rather than a best effort.
    #[tokio::test]
    async fn a_rename_reply_refuses_rather_than_guessing_when_the_catalog_is_unreadable() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        let cwd = state.path().to_string_lossy().to_string();
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();

        handle_control(
            &sup,
            ControlMsg::CreateSession {
                req_id: 1,
                parent: None,
                profile_name: None,
                cwd,
                invocation: None,
                profile_id: Some("starter-claude".to_string()),
                title: None,
                cols: 80,
                rows: 24,
                intent_key: None,
                agent_kind: None,
                resume_template: None,
            },
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;
        let frame = rx.try_recv().expect("a reply must have been sent");
        let ControlMsg::SessionCreated { session, .. } =
            serde_json::from_slice(&frame.body).expect("decode")
        else {
            panic!("the create must succeed before the rename can be tested");
        };

        sup.store.drop_profile_catalog_for_test().await;
        handle_control(
            &sup,
            ControlMsg::RenameSession {
                req_id: 2,
                session_id: session.id.clone(),
                title: "renamed".to_string(),
            },
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;
        let reply = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("the rename handler never replied")
            .expect("reply channel closed");
        let ControlMsg::Error { req_id, kind, .. } =
            serde_json::from_slice(&reply.body).expect("decode")
        else {
            panic!("an unreadable catalog must fail the reply, not fill the field in by guess");
        };
        assert_eq!(req_id, 2);
        assert_eq!(kind, ErrorKind::Internal);
        // The rename itself LANDED — the failure is about describing the
        // session, not about changing it, and a client that retries the
        // read gets the new title.
        assert_eq!(
            sup.store
                .session(&session.id)
                .await
                .expect("read")
                .expect("present")
                .title,
            "renamed"
        );
    }

    /// `RestartSession` carries a `req_id` a caller genuinely blocks on
    /// (unlike `PauseOutput`/`ResumeOutput`'s fire-and-forget precedent),
    /// so every request must produce a correlated reply — including the
    /// ones that fail. An unknown session is the cheapest such failure to
    /// drive end to end, and the one whose classification a client acts on
    /// differently (a 404 rather than a retryable error).
    ///
    /// The reply is awaited via the handler's own `JoinSet` rather than
    /// read immediately: this arm is spawned (a restart can run a
    /// multi-second kill sweep), so a `try_recv` straight after the call
    /// would race the task rather than test it.
    #[tokio::test]
    async fn restart_of_an_unknown_session_replies_not_found() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();

        handle_control(
            &sup,
            ControlMsg::RestartSession {
                req_id: 5,
                session_id: "no-such-session".to_string(),
                mode: farhelm_proto::RestartMode::Fresh,
                stop_if_running: false,
            },
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;
        while tasks.join_next().await.is_some() {}

        let reply = rx.try_recv().expect("a reply must have been sent");
        let decoded: ControlMsg = serde_json::from_slice(&reply.body).unwrap();
        let ControlMsg::Error {
            req_id,
            kind,
            message,
        } = decoded
        else {
            panic!("expected an Error reply, got {decoded:?}");
        };
        assert_eq!(req_id, 5);
        assert_eq!(kind, ErrorKind::NotFound);
        assert!(
            message.contains("no-such-session"),
            "the refusal names what was not found: {message}"
        );
    }

    /// The pre-side-effect half of the `CREATE_FIELD_CAP` guard: a request
    /// whose fields already exceed the cap must be rejected — with an
    /// `Error` reply correlated to its `req_id` and naming the cap — before
    /// `create_session` ever runs, so no session is left behind for a
    /// caller who was told the request failed. Drives the real
    /// `handle_control` dispatcher (not just the cap arithmetic in
    /// isolation) against a real `Supervisor`, since the invariant this
    /// protects is about what happens at the call site.
    #[tokio::test]
    async fn create_session_over_field_cap_is_rejected_before_any_side_effect() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        let req_id = 99;

        handle_control(
            &sup,
            ControlMsg::CreateSession {
                req_id,
                parent: None,
                profile_name: None,
                cwd: "x".repeat(CREATE_FIELD_CAP),
                invocation: Some("agent".to_string()),
                profile_id: None,
                title: None,
                cols: 80,
                rows: 24,
                intent_key: None,
                agent_kind: None,
                resume_template: None,
            },
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;

        let reply = rx.try_recv().expect("a reply must have been sent");
        let decoded: ControlMsg = serde_json::from_slice(&reply.body).unwrap();
        let ControlMsg::Error {
            req_id: got_req_id,
            message,
            kind,
        } = decoded
        else {
            panic!("expected ControlMsg::Error, got {decoded:?}");
        };
        assert_eq!(got_req_id, req_id);
        assert!(
            message.contains(&CREATE_FIELD_CAP.to_string()),
            "error message must name the limit that was exceeded: {message}"
        );
        assert_eq!(
            kind,
            ErrorKind::InvalidRequest,
            "an oversized request is the caller's mistake, not a server fault"
        );
        assert!(
            sup.sessions.lock().await.is_empty(),
            "a rejected request must create nothing"
        );
    }

    /// An intent key that could never do its job is refused at the edge,
    /// before it can reach the durable, deliberately un-pruned reservation
    /// table (`INTENT_KEY_CAP`).
    ///
    /// The empty key is the one worth spelling out: accepted, it would
    /// collapse every create from a client that forgot to fill the field
    /// into a single intent — the second such create would replay the
    /// first's session instead of making its own.
    #[tokio::test]
    async fn a_degenerate_intent_key_is_rejected_before_anything_is_stored() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();

        // Every refusal shape, including the boundary either side of the
        // cap and a multibyte key whose CHARACTER count is comfortably
        // under it — the cap is bytes, because bytes are what the row
        // costs, and a char-counted cap would let a four-byte-per-char key
        // store four times the intended maximum.
        let over_by_one_char = "\u{1f600}".repeat(INTENT_KEY_CAP / 4 + 1);
        assert!(
            over_by_one_char.chars().count() < INTENT_KEY_CAP,
            "test fixture: this key is over the cap only when counted in bytes"
        );
        for (req_id, key) in [
            (1u64, String::new()),
            (2, "k".repeat(INTENT_KEY_CAP + 1)),
            (3, over_by_one_char),
        ] {
            handle_control(
                &sup,
                ControlMsg::CreateSession {
                    req_id,
                    parent: None,
                    profile_name: None,
                    cwd: "/".to_string(),
                    invocation: Some("agent".to_string()),
                    profile_id: None,
                    title: None,
                    cols: 80,
                    rows: 24,
                    intent_key: Some(key),
                    agent_kind: None,
                    resume_template: None,
                },
                ConnectionCtx {
                    tx: &tx,
                    priority: &tx,
                    input_routes: &mut input_routes,
                    upload_routes: &mut no_uploads(),
                    tasks: &mut tasks,
                },
            )
            .await;
            let frame = rx.try_recv().expect("a reply must have been sent");
            let decoded: ControlMsg = serde_json::from_slice(&frame.body).expect("decode");
            let ControlMsg::Error { kind, message, .. } = decoded else {
                panic!("a degenerate intent key must be refused: {decoded:?}");
            };
            assert_eq!(kind, ErrorKind::InvalidRequest);
            assert!(
                message.contains("intent key"),
                "the refusal must name what was wrong: {message}"
            );
        }
        assert!(
            sup.store.load_all().await.expect("load").is_empty(),
            "a refused request must not have reached the store at all"
        );
        assert!(
            sup.store
                .pending_reservations()
                .await
                .expect("reservations")
                .is_empty(),
            "and must not have left a reservation either — a key refused at the edge is not \
             spent, so a corrected retry with the same key must still be able to use it"
        );
        // A key EXACTLY at the cap is accepted, which is what makes the
        // refusals above a boundary rather than a vague limit. It fails on
        // the working directory instead, well past the key check.
        handle_control(
            &sup,
            ControlMsg::CreateSession {
                req_id: 4,
                parent: None,
                profile_name: None,
                cwd: "/nonexistent/definitely/not/here".to_string(),
                invocation: Some("agent".to_string()),
                profile_id: None,
                title: None,
                cols: 80,
                rows: 24,
                intent_key: Some("k".repeat(INTENT_KEY_CAP)),
                agent_kind: None,
                resume_template: None,
            },
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;
        let frame = rx.try_recv().expect("a reply must have been sent");
        let decoded: ControlMsg = serde_json::from_slice(&frame.body).expect("decode");
        let ControlMsg::Error { message, .. } = decoded else {
            panic!("expected the cwd refusal: {decoded:?}");
        };
        assert!(
            message.contains("working directory"),
            "a key at exactly the cap must be accepted and the request judged on its merits: \
             {message}"
        );
    }

    /// The resume-template override is bounded on BOTH axes, before it can
    /// reach the never-pruned reservation row that stores a copy of it.
    ///
    /// Two independent limits because they fail independently: a template
    /// of a few enormous elements is caught by the shared byte cap (which
    /// it now counts against, alongside cwd/invocation/title), while a
    /// template of very many tiny ones costs almost no bytes and is caught
    /// by the element cap. Either shape unbounded is a permanent write
    /// sized by the request.
    #[tokio::test]
    async fn an_oversized_resume_template_is_refused_before_anything_is_stored() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();

        for (req_id, template, expected) in [
            (1u64, vec!["x".repeat(CREATE_FIELD_CAP)], "exceeding the"),
            (
                2,
                vec![String::new(); RESUME_TEMPLATE_ELEMENT_CAP + 1],
                "element limit",
            ),
        ] {
            handle_control(
                &sup,
                ControlMsg::CreateSession {
                    req_id,
                    parent: None,
                    profile_name: None,
                    cwd: "/".to_string(),
                    invocation: Some("agent".to_string()),
                    profile_id: None,
                    title: None,
                    cols: 80,
                    rows: 24,
                    intent_key: Some("key".to_string()),
                    agent_kind: None,
                    resume_template: Some(template),
                },
                ConnectionCtx {
                    tx: &tx,
                    priority: &tx,
                    input_routes: &mut input_routes,
                    upload_routes: &mut no_uploads(),
                    tasks: &mut tasks,
                },
            )
            .await;
            let frame = rx.try_recv().expect("a reply must have been sent");
            let decoded: ControlMsg = serde_json::from_slice(&frame.body).expect("decode");
            let ControlMsg::Error { kind, message, .. } = decoded else {
                panic!("an oversized resume template must be refused: {decoded:?}");
            };
            assert_eq!(kind, ErrorKind::InvalidRequest);
            assert!(
                message.contains(expected),
                "the refusal must name the limit that was exceeded: {message}"
            );
        }
        assert!(
            sup.store.load_all().await.expect("load").is_empty()
                && sup
                    .store
                    .pending_reservations()
                    .await
                    .expect("reservations")
                    .is_empty(),
            "neither refusal may have written anything"
        );
    }

    /// Call-site regression: drives `handle_control` itself, not
    /// `build_list_reply`/`reply_frame` in isolation. Before M2's list
    /// cap and byte budget existed, an oversized `ListSessions` reply
    /// could only be caught by `reply_frame`'s backstop degrading it to
    /// an `Error` — that scenario is still pinned directly against
    /// `reply_frame` by `reply_frame_substitutes_error_for_oversized_reply`
    /// in `connection`'s tests. Once `build_list_reply` sits in front of
    /// it at this call site, the SAME oversized fixture (a single session
    /// whose title alone exceeds `MAX_FRAME_LEN`) never reaches
    /// `reply_frame` in an oversized state at all: the byte budget catches
    /// it first, and — since Theme B of the M6.75 review-swarm batch — the
    /// zero-kept-with-a-session-remaining case `build_list_reply` finds is
    /// itself surfaced as `ErrorKind::Internal`, not the fake `total: 1`,
    /// empty-`sessions`, `next_cursor: None` "success" an earlier build
    /// answered (which the panel found indistinguishable from genuine
    /// exhaustion, and which made the rest of the walk unreachable).
    /// `build_list_reply`'s own unit tests in `listing` pin that outcome
    /// in isolation; this test pins it through the REAL call site, so a
    /// future change that quietly dropped `build_list_reply` from this
    /// call site (reverting to plain, uncapped `Frame::control`) would
    /// pass every `reply_frame`/`build_list_reply` unit test — they call
    /// those helpers directly — and only this test would catch it. It also
    /// proves the refusal is scoped to its own request: a second, ordinary
    /// request on the same connection (same `tx`) must still get an
    /// honest, untruncated reply.
    #[tokio::test]
    async fn list_sessions_call_site_refuses_a_record_too_large_to_fit_alone() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");

        // Populate the session map directly with fake data, sidestepping
        // tmux/launch entirely: only the size-driven reply behavior at
        // the ListSessions call site is under test here.
        sup.sessions.lock().await.insert(
            "s1".to_string(),
            Arc::new(SessionEntry {
                info: SessionInfo {
                    parent: None,
                    archived: false,
                    id: "s1".to_string(),
                    title: "x".repeat(farhelm_proto::MAX_FRAME_LEN as usize),
                    created_at: 1_700_000_000,
                    last_activity_at: 1_700_000_000,
                    creation_seq: None,
                    cwd: "/tmp".to_string(),
                    invocation: "agent".to_string(),
                    status: SessionStatus::default(),
                    annotation: None,
                    restart_offer: RestartOffer::default(),
                    tabs: Vec::new(),
                    source_profile: None,
                },
                terminal: Some(Terminal {
                    tmux_name: "fh-fake".to_string(),
                    pane: "%0".to_string(),
                }),
                outcome: Arc::new(std::sync::Mutex::new(LastOutcome::Running)),
                snapshot: IntegrationSnapshot {
                    kind: AgentKind::Generic,
                    resume_template: None,
                },
                canonical_cwd: None,
                first_input: Arc::new(std::sync::Mutex::new(FirstInput {
                    at: None,
                    durable: true,
                })),
                capture: Arc::new(std::sync::Mutex::new(CaptureState::Unclaimed)),
                hooked: crate::service::core::hook_flag(false),
                hook_warned: crate::service::core::hook_flag(false),
                activity: crate::service::ticker::ActivitySample::unsampled(),
                last_activity_at: crate::service::core::activity_stamp(1_700_000_000),
                generation: 0,
                scope: None,
            }),
        );

        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();

        handle_control(
            &sup,
            ControlMsg::ListSessions {
                req_id: 1,
                cursor: None,
                limit: None,
            },
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;
        // `ListSessions` is now spawned onto its own task (see that arm's
        // own comment on why), so `handle_control`'s `.await` above only
        // proves the request was ACCEPTED, not that the reply has been
        // sent yet — an immediate `try_recv` would be a race against the
        // spawned task's own tmux round trip. `recv().await`, bounded by
        // a timeout so a genuine regression fails fast instead of hanging
        // the test suite, is what actually waits for it.
        let reply = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("spawned ListSessions handler never replied")
            .expect("reply channel closed before a reply arrived");
        let decoded: ControlMsg = serde_json::from_slice(&reply.body).unwrap();
        let ControlMsg::Error {
            req_id,
            message,
            kind,
        } = decoded
        else {
            panic!("expected a refusal ControlMsg::Error, got {decoded:?}");
        };
        assert_eq!(req_id, 1);
        assert_eq!(kind, ErrorKind::Internal);
        assert!(
            message.contains("s1"),
            "the refusal must name the session that could not fit: {message}"
        );

        // Clear the oversized fixture and send a normal request through
        // the SAME tx: a healthy reply here is what proves the earlier
        // substitution was scoped to its one request. Clearing only
        // AFTER the first reply was actually received (not merely after
        // the request was accepted) keeps this ordering deliberate rather
        // than racing the first spawned task's still-in-flight tmux
        // query.
        sup.sessions.lock().await.clear();
        handle_control(
            &sup,
            ControlMsg::ListSessions {
                req_id: 2,
                cursor: None,
                limit: None,
            },
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;
        let reply2 = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("spawned ListSessions handler never replied a second time")
            .expect("reply channel closed before a second reply arrived");
        let decoded2: ControlMsg = serde_json::from_slice(&reply2.body).unwrap();
        let ControlMsg::SessionList {
            req_id,
            sessions,
            total,
            next_cursor,
        } = decoded2
        else {
            panic!("expected a normal ControlMsg::SessionList, got {decoded2:?}");
        };
        assert_eq!(req_id, 2);
        assert!(sessions.is_empty());
        assert_eq!(total, 0);
        assert_eq!(next_cursor, None);
    }

    /// Production call-site coverage for `LIST_SESSION_CAP` as the DEFAULT
    /// page size (`limit: None`) — the cheapest honest way to exercise the
    /// REAL wiring (`handle_list_sessions`'s own cursor/limit walk) rather
    /// than only `build_list_reply`'s pure-function tests, which never
    /// touch the handler at all. Creating `LIST_SESSION_CAP + 1` REAL tmux
    /// sessions to exercise this would be slow and environment-dependent
    /// for no added signal; `fake_entry` gives every entry a synthetic,
    /// terminal-less (`terminal: None`) fixture instead, which is enough to
    /// drive the cap/total wiring without needing a single real tmux round
    /// trip to succeed for any of them (`session_status` returns `Exited`
    /// for a terminal-less entry without ever consulting `pane_states`). An
    /// explicit `limit` above this same cap being HONORED rather than
    /// clamped back down to it is
    /// `list_sessions_limit_above_cap_is_honored_until_the_byte_budget_cuts`'s
    /// job, below (Theme C of the M6.75 review-swarm batch).
    #[tokio::test]
    async fn list_sessions_honors_the_session_cap_at_the_handler_level() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");

        {
            let mut sessions = sup.sessions.lock().await;
            for i in 0..LIST_SESSION_CAP + 1 {
                let id = format!("s{i}");
                sessions.insert(id.clone(), fake_entry(&id, 1_700_000_000));
            }
        }

        let (sessions, total, next_cursor) = list_sessions_page(&sup, 1, None, None).await;
        assert_eq!(
            sessions.len(),
            LIST_SESSION_CAP,
            "the cap must win over the full count at the real handler call site"
        );
        assert_eq!(
            total,
            (LIST_SESSION_CAP + 1) as u64,
            "total is the count BEFORE the cap"
        );
        assert!(
            next_cursor.is_some(),
            "one session remains beyond a cap-cut page, so a real resume cursor must be issued"
        );
    }

    /// `limit: Some(n)` above `LIST_SESSION_CAP` is HONORED, not clamped
    /// back down to it (Theme C of the M6.75 review-swarm batch:
    /// PLAN_M6.md keeps `LIST_SESSION_CAP` alive only as the DEFAULT page
    /// size — an earlier build's silent downward clamp to that same cap
    /// for an explicit `Some(n)` was never sanctioned by the plan).
    ///
    /// Proven by constructing more sessions than the cap, with titles fat
    /// enough that the BYTE BUDGET, not the count, decides where the first
    /// page ends: a still-clamped implementation would stop at exactly
    /// `LIST_SESSION_CAP` entries regardless of the budget; an honoring
    /// one keeps going past the cap until the budget itself cuts, landing
    /// somewhere between the cap and the full session count — so
    /// `page1.len() > LIST_SESSION_CAP` is the one assertion that tells
    /// the two implementations apart.
    ///
    /// The cursor the budget cut leaves is then FOLLOWED to a second page
    /// — a panel reviewer flagged that this test's predecessor asserted
    /// only page length and `total`, which proves nothing about whether
    /// the cursor `build_list_reply` issues can actually resume the walk,
    /// only that it left some cursor value.
    #[tokio::test]
    async fn list_sessions_limit_above_cap_is_honored_until_the_byte_budget_cuts() {
        const TOTAL_SESSIONS: usize = LIST_SESSION_CAP + 300;
        // Sized so the byte budget (`LIST_BYTE_BUDGET`, half of the 8 MiB
        // `MAX_FRAME_LEN`) admits meaningfully more than `LIST_SESSION_CAP`
        // entries but not all of `TOTAL_SESSIONS` — the gap between "the
        // cap" and "everything" is where this test's signal lives.
        const TITLE_LEN: usize = 6_500;

        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");

        {
            let mut sessions = sup.sessions.lock().await;
            for i in 0..TOTAL_SESSIONS {
                // Zero-padded and strictly descending so every entry gets
                // a distinct, stable `list_order_key` slot (matching the
                // convention `list_sessions_full_walk_equals_unpaginated_
                // truth` documents above).
                let id = format!("s{i:04}");
                let created_at = 1_700_000_000 - i as i64;
                sessions.insert(
                    id.clone(),
                    fake_entry_with_title(&id, created_at, "x".repeat(TITLE_LEN)),
                );
            }
        }

        let (page1, total, next_cursor) =
            list_sessions_page(&sup, 1, None, Some((LIST_SESSION_CAP * 2) as u32)).await;
        assert_eq!(total, TOTAL_SESSIONS as u64);
        assert!(
            page1.len() > LIST_SESSION_CAP,
            "an honored limit above the cap must be able to return more than \
             LIST_SESSION_CAP entries in one page: got {}",
            page1.len()
        );
        assert!(
            page1.len() < TOTAL_SESSIONS,
            "the byte budget, not the (now-unbounded) limit, must still be \
             what ends this page: got {}",
            page1.len()
        );
        let cursor = next_cursor.expect("sessions remain beyond the budget cut");

        let (page2, _, next_cursor2) =
            list_sessions_page(&sup, 2, Some(cursor), Some((LIST_SESSION_CAP * 2) as u32)).await;
        assert_eq!(
            page1.len() + page2.len(),
            TOTAL_SESSIONS,
            "the two pages together must account for every session exactly once, \
             proving the budget cut's cursor genuinely resumes the walk"
        );
        assert_eq!(
            next_cursor2, None,
            "the second page exhausts the remaining sessions"
        );
    }

    /// `limit: Some(0)` is refused outright rather than clamped up to 1 —
    /// this handler's own documented decision (see its doc comment for the
    /// reasoning: a caller sending 0 almost certainly has a bug, and
    /// silently substituting a value it never asked for would hide that).
    /// The refusal must land BEFORE any list work is admitted: `tasks` (the
    /// `JoinSet` `spawn_admitted` work lands on) is asserted empty, and the
    /// reply is drained with `try_recv` rather than an awaited `recv`, so a
    /// handler that spawned list work first and refused asynchronously
    /// afterward would fail this test even if the refusal eventually
    /// arrived.
    #[tokio::test]
    async fn list_sessions_limit_zero_is_refused_as_invalid_request() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");

        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        handle_control(
            &sup,
            ControlMsg::ListSessions {
                req_id: 1,
                cursor: None,
                limit: Some(0),
            },
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;
        assert_eq!(
            tasks.len(),
            0,
            "a limit-zero refusal must happen before any list work is admitted onto tasks"
        );
        let reply = rx.try_recv().expect(
            "the error reply must already be in the channel once handle_control's future \
             returns — a handler that scheduled list work before refusing could only reply \
             later, which try_recv (unlike an awaited recv) would not paper over",
        );
        let decoded: ControlMsg = serde_json::from_slice(&reply.body).unwrap();
        let ControlMsg::Error { req_id, kind, .. } = decoded else {
            panic!("expected ControlMsg::Error, got {decoded:?}");
        };
        assert_eq!(req_id, 1);
        assert_eq!(kind, ErrorKind::InvalidRequest);
    }

    /// An UNDECODABLE cursor — bad base64, truncated base64, or base64 of
    /// JSON with the wrong fields — is a clean `ErrorKind::InvalidRequest`,
    /// never a panic and never a silently wrong page (PLAN_M6.md's own
    /// pagination testing decision). Table-driven over several distinct
    /// malformed shapes because `decode_list_cursor`'s contract is that
    /// EVERY such shape collapses to the same refusal, not just the one a
    /// hand-picked example happens to hit.
    ///
    /// This is coverage for DECODING failure only, not for "tamper-
    /// proofing" — cursors carry no authority to tamper with in the first
    /// place (single-user supervisor, every caller may read every session).
    /// A value that decodes cleanly to a well-formed `ListCursor` — even
    /// one hand-edited to name a key this supervisor never issued — is
    /// ACCEPTED and simply resumes at that ordering position: see
    /// `list_sessions_cursor_from_a_deleted_session_still_resumes`, which
    /// pins exactly that as a feature, not a gap this test's fixtures
    /// happen not to cover.
    ///
    /// The refusal must land BEFORE the request is even spawned onto its
    /// own task, which this test pins directly rather than by inference:
    /// `tasks` (the `JoinSet` `spawn_admitted` list work lands on) is
    /// asserted empty, and the reply is drained with `try_recv` — not an
    /// awaited `recv`, which would equally pass for a handler that
    /// admitted list work first and refused only once that work finished.
    #[tokio::test]
    async fn list_sessions_malformed_cursor_is_refused_as_invalid_request() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");

        // The third shape is built rather than hand-typed: valid base64 of
        // valid JSON, but the WRONG shape (missing `ListCursor`'s required
        // fields) — the case a naive "does it base64-decode" check would
        // miss, and the one most worth generating rather than guessing at
        // by hand.
        use base64::Engine;
        let wrong_shape = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&serde_json::json!({"unrelated": true})).unwrap());
        for (i, malformed) in ["not-base64-at-all!!", "YQ", wrong_shape.as_str()]
            .into_iter()
            .enumerate()
        {
            let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
            let mut input_routes = HashMap::new();
            let mut tasks = tokio::task::JoinSet::new();
            let req_id = i as u64 + 1;
            handle_control(
                &sup,
                ControlMsg::ListSessions {
                    req_id,
                    cursor: Some(malformed.to_string()),
                    limit: None,
                },
                ConnectionCtx {
                    tx: &tx,
                    priority: &tx,
                    input_routes: &mut input_routes,
                    upload_routes: &mut no_uploads(),
                    tasks: &mut tasks,
                },
            )
            .await;
            assert_eq!(
                tasks.len(),
                0,
                "a cursor refusal must happen before any list work is admitted onto tasks \
                 (cursor {malformed:?})"
            );
            let reply = rx.try_recv().expect(
                "the error reply must already be in the channel once handle_control's future \
                 returns — a handler that scheduled list work before refusing could only reply \
                 later, which try_recv (unlike an awaited recv) would not paper over",
            );
            let decoded: ControlMsg = serde_json::from_slice(&reply.body).unwrap();
            let ControlMsg::Error { kind, .. } = decoded else {
                panic!("expected ControlMsg::Error for cursor {malformed:?}, got {decoded:?}");
            };
            assert_eq!(kind, ErrorKind::InvalidRequest);
        }
    }

    /// A terminal-less, synthetic session entry for the pagination tests
    /// below — real tmux/launch machinery is irrelevant to WALK ORDER, so
    /// every fixture here skips it the same way
    /// `list_sessions_honors_the_session_cap_at_the_handler_level`'s
    /// fixtures do (`terminal: None`, decided by `session_status` without
    /// any tmux round trip).
    fn fake_entry(id: &str, created_at: i64) -> Arc<SessionEntry> {
        fake_entry_with_title(id, created_at, "t".to_string())
    }

    /// [`fake_entry`], with an explicit title — the byte-budget test below
    /// needs fat ones to force a real cut.
    fn fake_entry_with_title(id: &str, created_at: i64, title: String) -> Arc<SessionEntry> {
        Arc::new(SessionEntry {
            info: SessionInfo {
                parent: None,
                archived: false,
                id: id.to_string(),
                title,
                created_at,
                last_activity_at: created_at,
                creation_seq: None,
                cwd: "/tmp".to_string(),
                invocation: "agent".to_string(),
                status: SessionStatus::default(),
                annotation: None,
                restart_offer: RestartOffer::default(),
                tabs: Vec::new(),
                source_profile: None,
            },
            terminal: None,
            outcome: Arc::new(std::sync::Mutex::new(LastOutcome::Running)),
            snapshot: IntegrationSnapshot {
                kind: AgentKind::Generic,
                resume_template: None,
            },
            canonical_cwd: None,
            first_input: Arc::new(std::sync::Mutex::new(FirstInput {
                at: None,
                durable: true,
            })),
            capture: Arc::new(std::sync::Mutex::new(CaptureState::Unclaimed)),
            hooked: crate::service::core::hook_flag(false),
            hook_warned: crate::service::core::hook_flag(false),
            activity: crate::service::ticker::ActivitySample::unsampled(),
            last_activity_at: crate::service::core::activity_stamp(created_at),
            generation: 0,
            scope: None,
        })
    }

    /// Send one `ListSessions` request through the real `handle_control`
    /// dispatch and wait for its (spawned) reply — every pagination test
    /// below walks several pages, so the request/spawn/timeout/decode
    /// boilerplate lives here once rather than once per page per test.
    async fn list_sessions_page(
        sup: &Arc<Supervisor>,
        req_id: u64,
        cursor: Option<String>,
        limit: Option<u32>,
    ) -> (Vec<SessionInfo>, u64, Option<String>) {
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        handle_control(
            sup,
            ControlMsg::ListSessions {
                req_id,
                cursor,
                limit,
            },
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;
        let reply = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("spawned ListSessions handler never replied")
            .expect("reply channel closed before a reply arrived");
        let decoded: ControlMsg = serde_json::from_slice(&reply.body).unwrap();
        let ControlMsg::SessionList {
            sessions,
            total,
            next_cursor,
            ..
        } = decoded
        else {
            panic!("expected ControlMsg::SessionList, got {decoded:?}");
        };
        (sessions, total, next_cursor)
    }

    /// The plan's headline pagination guarantee: walking a session set
    /// page by page (a limit smaller than the total count) with each
    /// page's `next_cursor` reproduces EXACTLY the unpaginated truth — in
    /// order, no duplicates, no gaps — across a set spanning several
    /// distinct `created_at` values (so the walk exercises both halves of
    /// the ordering key, not just the tiebreak). Ids are zero-padded
    /// (`s00`..`s11`) specifically so their STRING order matches their
    /// NUMERIC order, which is what lets this test's expected sequence be
    /// written out by hand without a second, independent sort to compare
    /// against — `list_order_key`'s own tiebreak direction (ascending id)
    /// is exercised, and separately pinned in isolation, by
    /// `list_sessions_same_created_at_tiebreaks_ascending_by_id` below.
    #[tokio::test]
    async fn list_sessions_full_walk_equals_unpaginated_truth() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");

        // Three sessions per `created_at` bucket, four buckets: descending
        // creation order puts the HIGHEST-numbered bucket (and, within it,
        // the lowest ids) first.
        {
            let mut sessions = sup.sessions.lock().await;
            for i in 0..12 {
                let id = format!("s{i:02}");
                let created_at = 1_000 + i / 3;
                sessions.insert(id.clone(), fake_entry(&id, created_at));
            }
        }
        let expected: Vec<&str> = vec![
            "s09", "s10", "s11", "s06", "s07", "s08", "s03", "s04", "s05", "s00", "s01", "s02",
        ];

        let mut walked: Vec<String> = Vec::new();
        let mut cursor = None;
        let mut req_id = 1;
        loop {
            let (page, total, next_cursor) =
                list_sessions_page(&sup, req_id, cursor, Some(5)).await;
            assert_eq!(total, 12, "total must stay the full count on every page");
            walked.extend(page.into_iter().map(|s| s.id));
            req_id += 1;
            match next_cursor {
                Some(c) => cursor = Some(c),
                None => break,
            }
            // A defensive bound so a bug that never terminates the walk
            // (an off-by-one that keeps re-serving the same page) fails
            // this test instead of hanging the suite.
            assert!(
                req_id < 20,
                "walk did not terminate within a sane page count"
            );
        }

        assert_eq!(
            walked, expected,
            "the paginated walk must reproduce the unpaginated order exactly, with no \
             duplicates and no gaps"
        );
    }

    /// `list_order_key`'s tiebreak direction, pinned in isolation: three
    /// sessions sharing one `created_at` must come back ascending by id.
    #[tokio::test]
    async fn list_sessions_same_created_at_tiebreaks_ascending_by_id() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        {
            let mut sessions = sup.sessions.lock().await;
            for id in ["sC", "sA", "sB"] {
                sessions.insert(id.to_string(), fake_entry(id, 1_700_000_000));
            }
        }
        let (page, total, next_cursor) = list_sessions_page(&sup, 1, None, None).await;
        assert_eq!(total, 3);
        assert_eq!(next_cursor, None, "one page covers the whole set");
        assert_eq!(
            page.into_iter().map(|s| s.id).collect::<Vec<_>>(),
            vec!["sA".to_string(), "sB".to_string(), "sC".to_string()],
            "sessions sharing one created_at must tiebreak ascending by id"
        );
    }

    /// Page stability across interleaved create/delete, PLAN_M6.md's
    /// pagination testing decision: a session created AFTER paging began
    /// must not tear the walk (it may simply be missed by this walk — the
    /// documented, accepted contract, since it sorts ahead of wherever the
    /// walk already is), and deleting a session already returned by an
    /// earlier page must not shift or duplicate what later pages return.
    #[tokio::test]
    async fn list_sessions_page_walk_survives_interleaved_create_and_delete() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        {
            let mut sessions = sup.sessions.lock().await;
            for i in 0..5 {
                let id = format!("s{i}");
                sessions.insert(id.clone(), fake_entry(&id, 1_000 + i));
            }
        }

        // Page 1: the two newest (s4, s3).
        let (page1, total1, cursor1) = list_sessions_page(&sup, 1, None, Some(2)).await;
        assert_eq!(
            page1.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["s4", "s3"]
        );
        assert_eq!(total1, 5);
        let cursor1 = cursor1.expect("three sessions remain beyond page 1");

        // Mid-walk mutation: delete an ALREADY-RETURNED session (s4, from
        // page 1 — "behind the cursor"), and create a NEW one whose
        // created_at is higher than everything else, so it sorts ahead of
        // the cursor's resume point — squarely inside the region this walk
        // already consumed.
        {
            let mut sessions = sup.sessions.lock().await;
            sessions.remove("s4");
            sessions.insert("s_new".to_string(), fake_entry("s_new", 1_000_000));
        }

        // Page 2 must resume exactly where page 1 left off — s2, s1 — never
        // reintroducing the deleted s4, and never surfacing s_new (missed
        // by this walk, per the documented contract).
        let (page2, total2, cursor2) = list_sessions_page(&sup, 2, Some(cursor1), Some(2)).await;
        assert_eq!(
            page2.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["s2", "s1"]
        );
        assert_eq!(
            total2, 5,
            "total reflects the CURRENT count: -1 (delete) +1 (create)"
        );
        let cursor2 = cursor2.expect("one session remains beyond page 2");

        // Page 3: the last remaining session, and the walk's real end.
        let (page3, _total3, cursor3) = list_sessions_page(&sup, 3, Some(cursor2), Some(2)).await;
        assert_eq!(
            page3.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["s0"]
        );
        assert_eq!(cursor3, None, "the walk must reach a real end");
    }

    /// A cursor naming a session that has SINCE been deleted must still
    /// resume cleanly — "strictly after this key", per `SessionList::
    /// next_cursor`'s own docs, never "starting from this row", which is
    /// exactly what makes this safe: nothing about resuming requires the
    /// named session to still exist.
    #[tokio::test]
    async fn list_sessions_cursor_from_a_deleted_session_still_resumes() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        {
            let mut sessions = sup.sessions.lock().await;
            for i in 0..5 {
                let id = format!("s{i}");
                sessions.insert(id.clone(), fake_entry(&id, 1_000 + i));
            }
        }

        // Take exactly one page (s4, s3, s2) and hold onto the cursor
        // resuming after s2 — the session about to be deleted.
        let (page1, _, cursor) = list_sessions_page(&sup, 1, None, Some(3)).await;
        assert_eq!(
            page1.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["s4", "s3", "s2"]
        );
        let cursor = cursor.expect("two sessions remain beyond page 1");

        sup.sessions.lock().await.remove("s2");

        let (page2, total2, next_cursor) = list_sessions_page(&sup, 2, Some(cursor), None).await;
        assert_eq!(
            page2.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["s1", "s0"],
            "a cursor from a since-deleted session must still resume after its key"
        );
        assert_eq!(total2, 4, "the deleted session must no longer be counted");
        assert_eq!(next_cursor, None);
    }

    /// PR 3 follow-up round, item 8: the sibling test above deletes ONE
    /// since-deleted session behind the cursor; this pins the far edge —
    /// EVERY session the walk has not yet seen is gone by the time the
    /// continuation runs. Resuming "strictly after this key"
    /// (`SessionList::next_cursor`'s own docs) with nothing left to be
    /// strictly after must land on a genuinely empty page, not an error
    /// and not a stale echo of the sessions that used to be there — the
    /// partition-point resume logic (`list_order_key`-sorted, sliced by
    /// `partition_point`) has no special-cased "nothing resumes" branch,
    /// so this is the case that would expose one if the slicing ever grew
    /// an off-by-one at the far end of the order.
    #[tokio::test]
    async fn list_sessions_page_walk_survives_every_unseen_session_being_deleted() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        {
            let mut sessions = sup.sessions.lock().await;
            for i in 0..5 {
                let id = format!("s{i}");
                sessions.insert(id.clone(), fake_entry(&id, 1_000 + i));
            }
        }

        // Page 1: the two newest (s4, s3) — s2, s1, s0 are still unseen.
        let (page1, total1, cursor1) = list_sessions_page(&sup, 1, None, Some(2)).await;
        assert_eq!(
            page1.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["s4", "s3"]
        );
        assert_eq!(total1, 5);
        let cursor1 = cursor1.expect("three sessions remain beyond page 1");

        // Every still-unseen session is deleted before the walk resumes —
        // nothing is left for the cursor to be "strictly after".
        {
            let mut sessions = sup.sessions.lock().await;
            sessions.remove("s2");
            sessions.remove("s1");
            sessions.remove("s0");
        }

        let (page2, total2, cursor2) = list_sessions_page(&sup, 2, Some(cursor1), None).await;
        assert!(
            page2.is_empty(),
            "a continuation with every unseen session deleted must return an empty page, \
             never a stale ghost of them: got {page2:?}"
        );
        assert_eq!(
            total2, 2,
            "total must reflect the CURRENT count (s4, s3 only), not the count as of page 1"
        );
        assert_eq!(
            cursor2, None,
            "an empty continuation must still be a SUCCESSFUL end of the walk, not an error \
             and not a cursor claiming more remains"
        );
    }

    /// The byte budget's real end-to-end path, through the handler rather
    /// than `build_list_reply` in isolation (`listing`'s own tests cover
    /// the pure function): fat titles force a genuine mid-page cut, and the
    /// resulting `next_cursor` must resume correctly into a second page —
    /// the concatenation of every page still reproducing the full,
    /// unpaginated set with no duplicates and no gaps.
    #[tokio::test]
    async fn list_sessions_byte_budget_cut_resumes_across_pages() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        // Four ~2 MiB titles comfortably exceed `LIST_BYTE_BUDGET` (half of
        // `MAX_FRAME_LEN`, itself 8 MiB) together, forcing a real cut well
        // before the count cap (`LIST_SESSION_CAP`) would ever bind.
        {
            let mut sessions = sup.sessions.lock().await;
            for i in 0..4 {
                let id = format!("s{i}");
                sessions.insert(
                    id.clone(),
                    fake_entry_with_title(&id, 1_000 + i, "x".repeat(2_000_000)),
                );
            }
        }

        let mut walked: Vec<String> = Vec::new();
        let mut cursor = None;
        let mut req_id = 1;
        let mut saw_a_cut = false;
        loop {
            let (page, total, next_cursor) = list_sessions_page(&sup, req_id, cursor, None).await;
            assert_eq!(total, 4);
            if next_cursor.is_some() {
                saw_a_cut = true;
            }
            walked.extend(page.into_iter().map(|s| s.id));
            req_id += 1;
            match next_cursor {
                Some(c) => cursor = Some(c),
                None => break,
            }
            assert!(
                req_id < 20,
                "walk did not terminate within a sane page count"
            );
        }

        assert!(
            saw_a_cut,
            "the fat-title fixture must actually force a byte-budget cut — otherwise this \
             test is not exercising the path it claims to"
        );
        assert_eq!(
            walked,
            vec!["s3", "s2", "s1", "s0"],
            "the byte-budget-cut walk must still reproduce the full unpaginated order"
        );
    }

    /// PR 3 follow-up round, item 7: the count/cursor cut
    /// (`ListSessions::limit`) and `build_list_reply`'s own byte-budget
    /// cut can both fire on the SAME page — the sibling test above only
    /// ever exercises the byte budget alone, with `limit: None` letting
    /// every candidate through the count cut untouched. Here `limit:
    /// Some(3)` selects the newest THREE (s4, s3, s2) before
    /// `build_list_reply` ever runs, and only the newest TWO of those
    /// three (s4, s3) fit under `LIST_BYTE_BUDGET` — s2 is fat enough on
    /// its own to overflow the ~400 KiB the first two fat titles leave
    /// behind, so the byte cut trims the count-cut's own candidate set
    /// further still.
    ///
    /// Two things must hold for the returned cursor to be trustworthy:
    /// it must encode s3 (the last entry the BYTE cut actually kept), not
    /// s2 (the count cut's boundary, which never made it into `kept` at
    /// all) — and following it into a second page must land on s2 FIRST,
    /// proving the byte-dropped entry was deferred to the next page
    /// rather than silently skipped past.
    #[tokio::test]
    async fn list_sessions_count_limit_and_byte_budget_cut_the_same_page() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        {
            let mut sessions = sup.sessions.lock().await;
            // s2, s3, s4: fat enough that any two together sit just under
            // `LIST_BYTE_BUDGET` (4 MiB) but three together blow past it —
            // see this test's own docs for the arithmetic. s0, s1: lean,
            // so page two's own byte accounting stays uneventful and the
            // test's only cut of interest is the one on page one.
            for i in 2..5 {
                let id = format!("s{i}");
                sessions.insert(
                    id.clone(),
                    fake_entry_with_title(&id, 1_000 + i, "x".repeat(1_900_000)),
                );
            }
            for i in 0..2 {
                let id = format!("s{i}");
                sessions.insert(id.clone(), fake_entry(&id, 1_000 + i));
            }
        }

        // Page 1: `limit: Some(3)` selects s4, s3, s2 before the byte
        // budget ever runs; only s4 and s3 survive it.
        let (page1, total1, cursor1) = list_sessions_page(&sup, 1, None, Some(3)).await;
        assert_eq!(
            page1.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["s4", "s3"],
            "the byte budget must trim the count cut's own 3-entry candidate set to 2"
        );
        assert_eq!(total1, 5);
        let cursor1 = cursor1.expect("both cuts left sessions unreturned beyond this page");
        assert_eq!(
            cursor1,
            encode_list_cursor(1_003, "s3"),
            "the cursor must encode s3 — the last entry the BYTE cut actually kept — not s2, \
             the count cut's boundary that never survived into `kept` at all"
        );

        // Page 2: resuming after s3 must land on s2 FIRST — the entry the
        // byte cut deferred, not skipped — before the lean s1/s0 tail.
        let (page2, total2, cursor2) = list_sessions_page(&sup, 2, Some(cursor1), None).await;
        assert_eq!(
            page2.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["s2", "s1", "s0"],
            "resuming after the byte cut must not skip s2, the entry it deferred"
        );
        assert_eq!(total2, 5);
        assert_eq!(cursor2, None, "the walk must reach a real end");
    }

    /// PLAN_M2.md's list-status contract: a `ListSessions` reply whose
    /// (capped) subset contains NO entry with a terminal at all —
    /// including the empty-list case, but exercised here with one
    /// terminal-less entry so the reply is checked for real content too —
    /// must succeed even if tmux itself is completely unreachable, because
    /// those statuses are decidable without asking tmux anything
    /// (`session_status` returns `Exited` for a terminal-less entry
    /// unconditionally). Proven by actually killing the supervisor's own
    /// private tmux server (bypassing the supervisor entirely) rather than
    /// just supplying terminal-less fixtures against a healthy one — if
    /// `ListSessions` asked tmux anything here, this test would see an
    /// `Error` reply instead of the expected `SessionList`.
    #[tokio::test]
    async fn list_sessions_skips_pane_states_when_nothing_has_a_terminal() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");

        let sock = state.path().join("tmux.sock");
        let killed = std::process::Command::new("tmux")
            .arg("-S")
            .arg(&sock)
            .arg("kill-server")
            .output()
            .expect("run tmux kill-server");
        assert!(
            killed.status.success(),
            "test setup: tmux kill-server must succeed, got: {}",
            String::from_utf8_lossy(&killed.stderr)
        );

        sup.sessions.lock().await.insert(
            "s1".to_string(),
            Arc::new(SessionEntry {
                info: SessionInfo {
                    parent: None,
                    archived: false,
                    id: "s1".to_string(),
                    title: "t".to_string(),
                    created_at: 1_700_000_000,
                    last_activity_at: 1_700_000_000,
                    creation_seq: None,
                    cwd: "/tmp".to_string(),
                    invocation: "agent".to_string(),
                    status: SessionStatus::default(),
                    annotation: None,
                    restart_offer: RestartOffer::default(),
                    tabs: Vec::new(),
                    source_profile: None,
                },
                terminal: None,
                outcome: Arc::new(std::sync::Mutex::new(LastOutcome::Running)),
                snapshot: IntegrationSnapshot {
                    kind: AgentKind::Generic,
                    resume_template: None,
                },
                canonical_cwd: None,
                first_input: Arc::new(std::sync::Mutex::new(FirstInput {
                    at: None,
                    durable: true,
                })),
                capture: Arc::new(std::sync::Mutex::new(CaptureState::Unclaimed)),
                hooked: crate::service::core::hook_flag(false),
                hook_warned: crate::service::core::hook_flag(false),
                activity: crate::service::ticker::ActivitySample::unsampled(),
                last_activity_at: crate::service::core::activity_stamp(1_700_000_000),
                generation: 0,
                scope: None,
            }),
        );

        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        handle_control(
            &sup,
            ControlMsg::ListSessions {
                req_id: 1,
                cursor: None,
                limit: None,
            },
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;
        let reply = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("spawned ListSessions handler never replied")
            .expect("reply channel closed before a reply arrived");
        let decoded: ControlMsg = serde_json::from_slice(&reply.body).unwrap();
        let ControlMsg::SessionList { sessions, .. } = decoded else {
            panic!(
                "expected ControlMsg::SessionList (tmux must not have been consulted at all), \
                 got {decoded:?}"
            );
        };
        assert_eq!(sessions.len(), 1);
        assert_eq!(
            sessions[0].status,
            SessionStatus::Exited { exit_code: None }
        );
    }
}
