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
//! their substance too: `ListSessions` to `listing::list_all` and
//! `DeleteSession` to `Supervisor::teardown_session`, leaving the handler
//! with validation on the way in and reply shaping on the way out.

use super::agent_relay::NO_HELM_ATTACHED;
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
use super::listing::list_all;
use super::status::{dead_pane_exit_code, entry_info, observe_entry};
use super::sweep::{SweepTarget, reap_process_tree, stop_live_agent};
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
use crate::store::{DedupScope, IntentClaim, LastOutcome, Transition};
use crate::tmux::PaneProbe;

/// Authority-derived create policy.
///
/// One value controls both selector defaulting and reservation lifetime, so
/// a caller cannot accidentally combine interactive derivation with bounded
/// keys or spawn derivation with permanent tombstones.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CreateAdmission {
    Interactive,
    Spawn { asking_session: String },
}

impl CreateAdmission {
    fn dedup_scope(&self) -> DedupScope {
        match self {
            CreateAdmission::Interactive => DedupScope::Permanent,
            CreateAdmission::Spawn { .. } => DedupScope::SessionLifetime,
        }
    }
}
use anyhow::Context;
use farhelm_proto::{
    AgentKind, AgentOutcome, AgentReply, AgentVerb, ControlMsg, ErrorKind, Frame,
    MAX_SESSION_ID_BYTES, ProfileSnapshot as WireProfileSnapshot, RestartMode, SessionInfo,
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
/// spawn-only profile NAME alongside any resolved bundle field is just as
/// ambiguous as naming both a profile and an invocation. There is no honest
/// precedence rule between "resolve this name" and "launch these values".
///
/// The `Err` is the user-facing message verbatim (SPEC.md's concrete,
/// actionable errors), so each one names both what was sent and what would
/// have been acceptable.
///
/// The overrides are MOVED into the raw variant rather than passed onward
/// beside the mode: past this function they are meaningful only for a raw
/// create, and [`CreateMode`] is where that stops being a convention.
enum CreateSelector {
    Bundle(CreateMode),
    ProfileName(String),
    Derived,
}

/// Validate the create selector's wire shape before any profile resolution
/// or reservation work begins.
///
/// `profile_name` and the selectorless form are retained only for the spawn
/// protocol. All ordinary helm creates arrive as an invocation bundle, with
/// profile provenance present only when the helm resolved one.
fn create_mode(
    invocation: Option<String>,
    profile_name: Option<String>,
    agent_kind: Option<AgentKind>,
    resume_template: Option<Vec<String>>,
    source_profile: Option<WireProfileSnapshot>,
) -> Result<CreateSelector, String> {
    match (invocation, profile_name) {
        (Some(invocation), None) => Ok(CreateSelector::Bundle(CreateMode::Raw {
            invocation,
            agent_kind,
            resume_template,
            source_profile: source_profile.map(|profile| crate::store::ProfileSnapshot {
                id: profile.id,
                name: profile.name,
            }),
        })),
        (None, Some(profile_name)) => {
            if agent_kind.is_some() || resume_template.is_some() || source_profile.is_some() {
                return Err(
                    "a spawn profile name cannot also carry invocation bundle fields".to_string(),
                );
            }
            Ok(CreateSelector::ProfileName(profile_name))
        }
        (None, None)
            if agent_kind.is_none() && resume_template.is_none() && source_profile.is_none() =>
        {
            Ok(CreateSelector::Derived)
        }
        (None, None) => Err(
            "a create without an invocation cannot carry agent_kind, resume_template, or \
             source_profile"
                .to_string(),
        ),
        (Some(_), Some(_)) => Err(
            "a create names exactly one of invocation or profile name; this request named both"
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
/// under the same key. That is deliberately NOT true of launch preconditions
/// past this point (such as a working directory that does not exist): those
/// are durable outcomes replayed under the key, which
/// is the contract `Supervisor::create_session` states in full.
async fn resolve_create_selector(
    sup: &Arc<Supervisor>,
    admission: &CreateAdmission,
    selector: CreateSelector,
) -> Result<CreateMode, (ErrorKind, String)> {
    match selector {
        CreateSelector::Bundle(mode) => Ok(mode),
        CreateSelector::ProfileName(name) => {
            let CreateAdmission::Spawn { asking_session } = admission else {
                return Err((
                    ErrorKind::InvalidRequest,
                    "profile_name is available only to a session-authenticated spawn".to_string(),
                ));
            };
            match sup
                .relay_agent_request(
                    asking_session.clone(),
                    AgentVerb::ResolveProfile { name },
                    None,
                )
                .await
            {
                AgentOutcome::Ok {
                    reply:
                        AgentReply::ResolvedProfile {
                            invocation,
                            agent_kind,
                            resume_template,
                            source_profile,
                        },
                } => Ok(CreateMode::Raw {
                    invocation,
                    agent_kind: Some(agent_kind),
                    resume_template,
                    source_profile: Some(crate::store::ProfileSnapshot {
                        id: source_profile.id,
                        name: source_profile.name,
                    }),
                }),
                AgentOutcome::Ok { reply } => Err((
                    ErrorKind::Internal,
                    format!("the attached helm returned an unexpected {reply:?} reply"),
                )),
                AgentOutcome::Err {
                    kind: ErrorKind::Unavailable,
                    message,
                } if message.starts_with(NO_HELM_ATTACHED) => Err((
                    ErrorKind::Unavailable,
                    "an attached helm is needed to resolve a profile name; omit --agent to \
                     reuse the asking session's agent"
                        .to_string(),
                )),
                AgentOutcome::Err { kind, message } => Err((kind, message)),
            }
        }
        CreateSelector::Derived => {
            let CreateAdmission::Spawn { asking_session } = admission else {
                return Err((
                    ErrorKind::InvalidRequest,
                    "a create must carry an invocation bundle".to_string(),
                ));
            };
            let parent = sup
                .store
                .session(asking_session)
                .await
                .map_err(|error| {
                    (
                        ErrorKind::Internal,
                        format!("could not read the asking session's agent bundle: {error:#}"),
                    )
                })?
                .ok_or_else(|| {
                    (
                        ErrorKind::Unauthorized,
                        "the asking session no longer exists".to_string(),
                    )
                })?;
            Ok(CreateMode::Raw {
                invocation: parent.invocation,
                agent_kind: Some(parent.agent_kind),
                resume_template: parent.resume_template,
                source_profile: parent.source_profile,
            })
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_create_session(
    sup: &Arc<Supervisor>,
    tx: &mpsc::Sender<Frame>,
    req_id: u64,
    parent: Option<String>,
    cwd: String,
    invocation: Option<String>,
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
    source_profile: Option<WireProfileSnapshot>,
) {
    let selector = match create_mode(
        invocation,
        profile_name,
        agent_kind,
        resume_template,
        source_profile,
    ) {
        Ok(selector) => selector,
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
    if let CreateSelector::ProfileName(name) = &selector {
        let field_len = parent.as_deref().map_or(0, str::len)
            + cwd.len()
            + name.len()
            + title.as_deref().map_or(0, str::len);
        if field_len > CREATE_FIELD_CAP {
            send_reply(
                tx,
                &ControlMsg::Error {
                    req_id,
                    message: format!(
                        "parent, cwd, profile name, and title together are {field_len} bytes, \
                         exceeding the {CREATE_FIELD_CAP}-byte limit"
                    ),
                    kind: ErrorKind::InvalidRequest,
                },
            )
            .await;
            return;
        }
    }
    if let Some(message) = match intent_key.as_deref() {
        Some("") => Some("intent key must not be empty".to_string()),
        Some(key) if key.len() > INTENT_KEY_CAP => Some(format!(
            "intent key is {} bytes, exceeding the {INTENT_KEY_CAP}-byte limit",
            key.len()
        )),
        _ => None,
    } {
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
    let mode = match resolve_create_selector(sup, &admission, selector).await {
        Ok(mode) => mode,
        Err((kind, message)) => {
            send_reply(
                tx,
                &ControlMsg::Error {
                    req_id,
                    message,
                    kind,
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
            source_profile,
            ..
        } => (
            invocation.len()
                + resume_template
                    .iter()
                    .flatten()
                    .map(|element| element.len())
                    .sum::<usize>()
                + source_profile
                    .as_ref()
                    .map_or(0, |profile| profile.id.len() + profile.name.len()),
            resume_template.as_ref().map_or(0, Vec::len),
        ),
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
    // The fingerprint binds the resolved bundle. A profile edit between
    // retries therefore changes the fingerprint instead of replaying a
    // launch shaped by stale catalog data.
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
    Ok(entry_info(
        entry,
        &pane_states,
        observed.sentinel.as_deref(),
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
/// ## The reply is the whole list
///
/// There is nothing to validate on the way in: `ListSessions` carries
/// only its `req_id` (`PROTOCOL_VERSION` 14), and `list_all` answers with
/// every session up to `LIST_SESSIONS_CAP`, flagging a cut. A reply too
/// large for one frame is not budgeted for here — `reply_frame`'s
/// oversize defusal turns it into an `Internal` error, and the helm keeps
/// its previous cache; see the cap's own docs in `farhelm-proto` for why
/// that is an accepted failure rather than a case worth a byte budget.
async fn handle_list_sessions(
    sup: &Arc<Supervisor>,
    tx: &mpsc::Sender<Frame>,
    tasks: &mut tokio::task::JoinSet<()>,
    req_id: u64,
) {
    let sup2 = Arc::clone(sup);
    let tx = tx.clone();
    spawn_admitted(&sup.admission, tasks, async move {
        let sup = sup2;
        let reply = match list_all(&sup).await {
            Ok(list) => ControlMsg::SessionList {
                req_id,
                sessions: list.sessions,
                truncated: list.truncated,
            },
            // Every failure the listing can hit — an unreadable launch
            // sentinel, an unclassified tmux failure — is an `Internal`
            // carrying the original error verbatim: see `list_all`'s own
            // docs for why each of them fails the whole request rather
            // than degrading one entry.
            Err(e) => ControlMsg::Error {
                req_id,
                message: format!("{e:#}"),
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
/// never takes the attachment-map lock at all — it leaves every
/// attachment entry intact — and never touches `input_routes`
/// either (see `ControlMsg::StopSession`'s
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
        // The one "alive" check that decides which lifecycle this
        // stop runs (kill a live tree, or classify a dead pane). The
        // stale pid a dead pane still reports is deliberately never
        // read; it may already be recycled.
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
            // Nothing is read off the pane before the kill. A stop
            // used to capture an alternate-screen app's last frame
            // here and replay it on the dead pane afterwards; SPEC.md
            // (Terminal experience) now says a full-screen program's
            // last frame is not retained after it exits and no
            // snapshot of it is taken or stored. What the dead pane
            // shows is whatever tmux itself still holds.
            stop_live_agent(&sup, &session_id, &entry, Some(pane.pid))
                .await
                .err()
                .map(|failure| failure.message())
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
            // Claimed BEFORE the lifecycle lock, and that order is load-
            // bearing (see `Supervisor::agent_request_locks`). An asking
            // session's own credential is validated once, at the top of
            // the `AgentRequest` handler, and this delete may be racing a
            // mutation that credential already authorized — a rename,
            // stop, or archive still in flight up to the helm and back.
            // Waiting here for that fence to clear means such a mutation
            // always finishes against a session this delete has not yet
            // torn down, rather than the delete invalidating the very
            // credential mid-flight.
            let _agent_fence = mutation_sup.agent_request_locks.claim(&mutation_id).await;
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

    // Everything from here on — the replay prefill and the live
    // pump — happens inside the forwarder task rather than here,
    // and that placement is load-bearing.
    // A full replay is megabytes of 32 KiB frames, and this
    // handler runs under the supervisor-wide `attachments` mutex;
    // sending them here would mean AWAITING a bounded queue (see
    // CONNECTION_WRITER_QUEUE) with that lock held, letting one
    // slow client stall every other session's attach and input.
    // Ordering is unaffected: the forwarder is this channel's
    // only writer, so its prefill necessarily precedes its own
    // live output.
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
            profile_name,
            title,
            cols,
            rows,
            intent_key,
            agent_kind,
            resume_template,
            source_profile,
        } => {
            handle_create_session(
                sup,
                ctx.tx,
                req_id,
                parent,
                cwd,
                invocation,
                profile_name,
                title,
                cols,
                rows,
                intent_key,
                CreateAdmission::Interactive,
                agent_kind,
                resume_template,
                source_profile,
            )
            .await
        }
        ControlMsg::ListSessions { req_id } => {
            handle_list_sessions(sup, ctx.tx, ctx.tasks, req_id).await
        }
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
            profile_name,
            title,
            cols,
            rows,
            intent_key,
            agent_kind,
            resume_template,
            source_profile,
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
            // A session may ask the helm to resolve a name, but it may not
            // assert that an arbitrary invocation came from a trusted
            // profile. Only the full-authority helm and this supervisor's
            // own named-spawn resolution can attach that provenance.
            if source_profile.is_some() {
                send_reply(
                    tx,
                    &ControlMsg::Error {
                        req_id,
                        message: "source_profile is not available to session-authenticated creates"
                            .to_string(),
                        kind: ErrorKind::InvalidRequest,
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
                profile_name,
                title,
                cols,
                rows,
                intent_key,
                CreateAdmission::Spawn {
                    asking_session: auth.session_id.clone(),
                },
                agent_kind,
                resume_template,
                source_profile,
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
            // NO claim on `lifecycle_locks` — that registry is keyed by
            // the TARGET of a mutation, and this request may not even name
            // a session on this host (a `Some(id)` target can live
            // anywhere in the fleet, and the two read-only verbs name no
            // session at all). Taking it here would also serialize an
            // unrelated restart or stop behind a peer that may be waiting
            // out the full upcall budget, for no correctness gain.
            //
            // What IS held, for the MUTATING verbs only, is a claim
            // on `agent_request_locks` keyed by THIS connection's OWN
            // asking session id — see that field's docs for the full
            // reasoning. In short: `authenticates_session` below is a
            // snapshot taken once, not a lease held for the round trip,
            // and the trip to the helm and back can run long enough for
            // `DeleteSession` to remove the very row that snapshot vouched
            // for. Holding the claim is what keeps a delete from
            // invalidating this credential out from under a mutation it
            // already authorized. Read-only verbs take no claim: nothing
            // they return is durable, so a delete racing a listing has
            // nothing to protect against.
            //
            // CLAIMED BEFORE THE CREDENTIAL IS CHECKED, not after, and that
            // order is the whole guarantee rather than a stylistic
            // preference. Claiming afterwards leaves a gap the fence cannot
            // see: a `DeleteSession` for this same id can take the key,
            // finish the entire teardown, and drop it again between
            // `authenticates_session` returning `true` and this handler
            // reaching for the key — after which the claim succeeds
            // immediately against a session that no longer exists, and the
            // mutation is relayed on an identity the delete already
            // revoked. `relay_agent_request` re-authorizes nothing, so
            // nothing downstream would catch it. Claiming first makes the
            // credential check itself happen under the fence: either the
            // delete got there first and the check fails honestly, or it is
            // parked behind this claim and cannot revoke anything until the
            // mutation it is racing has finished.
            //
            // The claim is keyed by this connection's OWN session id, so a
            // peer holding an invalid or already-revoked credential can
            // only ever fence itself — the cost of taking it before the
            // check rather than after is bounded to the asker's own row.
            //
            // Every refusal below is an `AgentResponse` rather than a bare
            // `Error`: this exchange has two hops and refusals from both,
            // and one reply shape means the asking CLI decodes exactly one
            // thing (see `ControlMsg::AgentRequest`'s docs).
            // The two CREATING verbs are fenced for exactly the reason the
            // lifecycle three are, and the stakes are higher: a create that
            // completes while this credential is being invalidated leaves a
            // real session running on some host with the asking process
            // told nothing about it. Which verbs count is not decided here
            // — `AgentVerb::is_mutating` is the single exhaustive answer
            // both sides of the relay read, so a verb added to the enum
            // cannot be fenced on one side and not the other.
            let fence = if request.is_mutating() {
                Some(sup.agent_request_locks.claim(&auth.session_id).await)
            } else {
                None
            };
            // The claim-before-check window, held open on demand so a test
            // can attempt the delete inside it — see `AgentAuthGate` for
            // why nothing observable distinguishes the two orderings from
            // outside. Production installs no gate.
            if let Some(gate) = &sup.seams.agent_auth_gate {
                gate().await;
            }
            let outcome = match sup
                .store
                .authenticates_session(&auth.session_id, &auth.token)
                .await
            {
                Ok(true) if session_id == auth.session_id => match validate_agent_verb(&request) {
                    // The fence moves into the relay, which releases it
                    // when the mutation is really over rather than when
                    // this call returns — see `HelmLink::upcall`.
                    Ok(()) => sup.relay_agent_request(session_id, request, fence).await,
                    Err(message) => farhelm_proto::AgentOutcome::Err {
                        kind: ErrorKind::InvalidRequest,
                        message,
                    },
                },
                // A credential for one session is not authority to speak AS
                // another. The check is here rather than at the far end
                // because the helm never sees the credential: by the time
                // the request reaches it, `session_id` is the only claim
                // about who is asking, and it has to already be true.
                //
                // The refusal names the distinction it is enforcing,
                // because the obvious reading of the old wording ("may ask
                // only as itself") was that a session could not touch any
                // other session at all — which is not the rule. This field
                // is the ASKER's identity; the lifecycle verbs carry their
                // own target and may name any session in the fleet.
                Ok(true) => farhelm_proto::AgentOutcome::Err {
                    kind: ErrorKind::Unauthorized,
                    message: format!(
                        "a session-authenticated peer may only send requests under its own \
                         identity ({}); to act on a different session, ask as yourself and name \
                         that session as the verb's own target",
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

/// Bound and sanitize an `AgentRequest`'s TARGET before it is logged or
/// relayed anywhere.
///
/// This is the first hop under this connection's own control, and the only
/// one able to enforce a bound cheaply: an explicit `session_id` target, a
/// `Rename` `title`, and a creating verb's host name, directory, selector
/// and title otherwise ride two more shared, byte-unbounded queues
/// — this connection's writer queue, then the helm's own — before
/// `route_session` at the far end ever gets a chance to reject an oversized
/// or unknown one. Rejecting here, before either queue ever sees the
/// bytes, is also what keeps a control character or an empty string out of
/// `resolve_target`'s `info!` audit line on the helm side: that function
/// logs `target` verbatim on the assumption that whatever reaches it has
/// already been validated, which before this function existed was not
/// true.
///
/// The bounds mirror ones enforced elsewhere for the SAME kind of value —
/// [`MAX_SESSION_ID_BYTES`] is the connection hello's own cap on a session
/// id, [`CREATE_FIELD_CAP`] is `RenameSession::title`'s existing 64 KiB
/// field cap, and [`INTENT_KEY_CAP`] is what `handle_create_session`
/// already holds an idempotency key to — rather than inventing new numbers:
/// a value that would be refused when read straight off the wire elsewhere
/// should be refused here too, before it ever leaves this process a second
/// time. Control characters are refused for the same reason and by the same
/// rule ([`char::is_control`], Unicode's `Cc` category) in the three fields
/// something on THIS side of the wire goes on to echo — the target, a
/// `Rename` title, and a creating verb's host name — which for the title is
/// exactly what the TARGET supervisor's own `ensure_title_printable` will
/// apply at the far end.
///
/// The CREATING verbs are bounded as a GROUP, the way
/// `handle_create_session` bounds a create's fields: cwd, selector and
/// title are summed against one cap rather than each given its own, because
/// what the cap protects against is the total a single request can push
/// through the two queues downstream, and three individually-legal fields
/// can add up to three times the intended ceiling. The intent key is
/// separate for the same reason it is separate over there — it bounds a
/// durable table keyed by whatever the caller sent, not a reply.
///
/// A host NAME is bounded by the same total. It is caller-supplied text
/// that the helm compares against its registry and quotes back in a
/// not-found refusal, so it is exactly the kind of field this function
/// exists to stop early.
///
/// WHAT IT IS NOT: this is a doorway bound, not the target's admission
/// check restated. It knows nothing about whether the named session exists,
/// which host owns it, whether the named host is registered, or whether the
/// owning host will accept the mutation or the create — `route_session`,
/// the helm's own host lookup and the owning supervisor decide all of that,
/// and a request that passes here can still be refused at any of them. The
/// parity claimed above is limited to the field-shape rules a byte-unbounded
/// queue makes it worth enforcing early.
///
/// The public read-only verbs carry no such field and pass. The internal
/// `ResolveProfile` verb is different: only the supervisor's trusted spawn
/// resolver may send it upward, so a session presenting it directly is
/// refused here before the helm can disclose a launch bundle.
fn validate_agent_verb(verb: &AgentVerb) -> Result<(), String> {
    fn validate_target(target: &Option<String>) -> Result<(), String> {
        let Some(target) = target else {
            return Ok(());
        };
        if target.is_empty() {
            return Err(
                "an explicit --session target must not be empty; omit --session to act on the \
                 asking session instead"
                    .to_string(),
            );
        }
        if target.len() > MAX_SESSION_ID_BYTES {
            return Err(format!(
                "an explicit --session target is {} bytes, exceeding the {MAX_SESSION_ID_BYTES}-\
                 byte limit every session id is already held to",
                target.len()
            ));
        }
        if target.chars().any(char::is_control) {
            return Err(
                "an explicit --session target must not contain control characters".to_string(),
            );
        }
        Ok(())
    }
    match verb {
        AgentVerb::Hosts {} | AgentVerb::Sessions {} => Ok(()),
        AgentVerb::Rename { session_id, title } => {
            validate_target(session_id)?;
            if title.len() > CREATE_FIELD_CAP {
                return Err(format!(
                    "title is {} bytes, exceeding the {CREATE_FIELD_CAP}-byte limit",
                    title.len()
                ));
            }
            // The same refusal the TARGET's `ensure_title_printable` would
            // produce, applied at this hop for the same reason the id's is:
            // a newline-laced title otherwise rides two byte-unbounded
            // queues and lands in the helm's `resolve_target` audit line
            // before anything rejects it. The empty title stays legal here,
            // exactly as it is at the far end.
            if title.chars().any(char::is_control) {
                return Err("title must not contain control characters".to_string());
            }
            Ok(())
        }
        AgentVerb::Stop { session_id } | AgentVerb::Archive { session_id } => {
            validate_target(session_id)
        }
        AgentVerb::Create {
            host,
            cwd,
            profile_name,
            invocation,
            title,
            intent_key,
        } => validate_create_fields(
            host.as_deref(),
            cwd,
            [profile_name.as_deref(), invocation.as_deref()],
            title.as_deref(),
            intent_key.as_deref(),
        ),
        AgentVerb::Clone {
            host,
            cwd,
            title,
            intent_key,
        } => validate_create_fields(
            host.as_deref(),
            cwd.as_deref().unwrap_or_default(),
            [None, None],
            title.as_deref(),
            intent_key.as_deref(),
        ),
        AgentVerb::ResolveProfile { .. } => {
            Err("profile resolution is not available to sessions".to_string())
        }
    }
}

/// The shared bound on everything a CREATING verb can push downstream.
///
/// Factored out because `Create` and `Clone` carry overlapping subsets of
/// the same fields and must be held to identical limits: a clone is a
/// create whose selector the helm derives, so a looser cap on one of them
/// would be a way to send through the other's doorway.
///
/// THREE bounds, not one, and the split follows what each field is:
///
/// - The CREATE PAYLOAD — cwd, selector, title — is summed against
///   [`CREATE_FIELD_CAP`], because those are exactly the fields
///   `handle_create_session` holds a create to over there and a create that
///   would be refused at the far end should be refused here first. Summed
///   rather than checked one by one for the reason [`validate_agent_verb`]
///   documents.
/// - The HOST NAME gets [`AGENT_HOST_NAME_CAP`], its own allowance. It is
///   ROUTING metadata: the helm matches it against the registry and never
///   forwards it to any supervisor, so charging it against the create's
///   payload budget would let a long registered host name make an
///   otherwise-legal create fail through the agent surface alone. It still
///   needs a bound of its own — it rides the same two byte-unbounded queues
///   as everything else here, and the helm quotes it back in a not-found
///   refusal.
/// - The INTENT KEY gets [`INTENT_KEY_CAP`], for the reason it is separate
///   over there: it keys a durable table, not a reply.
///
/// Control characters are refused in the host name alone — the one field of
/// these that this process's own downstream (the helm's not-found refusal)
/// echoes back as free text. `cwd`, `invocation` and `title` are the TARGET
/// supervisor's to judge, with rules this one has no business duplicating;
/// what happens to them here is a size bound and nothing else.
///
/// `cwd` arrives as `""` for a `Clone` that named none, and that is the
/// wire's `None` rather than an empty directory. It contributes nothing to
/// the sum, correctly: the directory such a clone will actually use is the
/// SOURCE's, which was bounded when the source was created and never
/// travels through this doorway at all.
fn validate_create_fields(
    host: Option<&str>,
    cwd: &str,
    selectors: [Option<&str>; 2],
    title: Option<&str>,
    intent_key: Option<&str>,
) -> Result<(), String> {
    if let Some(host) = host {
        if host.is_empty() {
            return Err(
                "an explicit --host must not be empty; omit --host to act on this session's own \
                 host instead"
                    .to_string(),
            );
        }
        if host.chars().any(char::is_control) {
            return Err("an explicit --host must not contain control characters".to_string());
        }
        if host.len() > AGENT_HOST_NAME_CAP {
            return Err(format!(
                "an explicit --host is {} bytes, exceeding the {AGENT_HOST_NAME_CAP}-byte limit",
                host.len()
            ));
        }
    }
    let field_len = cwd.len()
        + selectors.into_iter().flatten().map(str::len).sum::<usize>()
        + title.map_or(0, str::len);
    if field_len > CREATE_FIELD_CAP {
        return Err(format!(
            "cwd, profile or invocation, and title together are {field_len} bytes, exceeding the \
             {CREATE_FIELD_CAP}-byte limit"
        ));
    }
    match intent_key {
        Some("") => Err("intent key must not be empty".to_string()),
        Some(key) if key.len() > INTENT_KEY_CAP => Err(format!(
            "intent key is {} bytes, exceeding the {INTENT_KEY_CAP}-byte limit",
            key.len()
        )),
        _ => Ok(()),
    }
}

/// The longest a creating verb's `--host` value may be.
///
/// [`MAX_SESSION_ID_BYTES`]'s number, borrowed rather than newly invented,
/// and for the same shape of reason: both are identifiers a peer supplies
/// to name something the receiver already knows about, and neither is
/// content. A kilobyte is far past any real ssh destination while staying
/// small enough that a fleet-sized refusal listing many of them still fits
/// in one frame.
///
/// The registry itself imposes no length limit on a display name, so this
/// is a bound on what may be ASKED FOR, not a claim about what may be
/// stored. A registered host whose name exceeds it simply cannot be named
/// as a target — which the helm's own refusal explains (see
/// `agent_requests::unnameable_hosts` for the sibling case).
const AGENT_HOST_NAME_CAP: usize = MAX_SESSION_ID_BYTES;

#[cfg(test)]
mod tests {
    use super::super::capture::{CaptureState, FirstInput};
    use super::super::connection::CONNECTION_WRITER_QUEUE;
    use super::super::core::tests::{StateDir, dummy_exe, entry_with, no_uploads};
    use super::super::core::{ArchiveStage, SupervisorSeams};
    use super::super::terminals::Terminal;
    use super::*;
    use crate::agent_kind::IntegrationSnapshot;
    use farhelm_proto::{RestartOffer, SessionStatus};

    /// Seed the durable half of a parent, which is the authority source a
    /// restricted connection must revalidate before every create.
    async fn authenticated_parent(
        sup: &Supervisor,
        cwd: &std::path::Path,
        id: &str,
    ) -> farhelm_proto::SessionAuth {
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
                    invocation: "/fixture/parent-agent --flag".to_string(),
                    tmux_name: format!("fh-{id}"),
                    pane: String::new(),
                    outcome: LastOutcome::Launching,
                    agent_kind: AgentKind::Codex,
                    resume_template: Some(vec![
                        "/fixture/parent-agent".to_string(),
                        "resume".to_string(),
                        crate::agent_kind::CONVERSATION_PLACEHOLDER.to_string(),
                    ]),
                    canonical_cwd: None,
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                    source_profile: Some(crate::store::ProfileSnapshot {
                        id: "parent-profile".to_string(),
                        name: "Parent profile".to_string(),
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

    /// A helm-resolved bundle is recorded verbatim and its source existence
    /// remains unresolved on the supervisor wire.
    ///
    /// This pins the authority split: the supervisor validates and stores
    /// launch fields but never consults a profile catalog or invents an
    /// existence verdict.
    #[tokio::test]
    async fn a_resolved_create_records_the_bundle_without_catalog_resolution() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        handle_control(
            &sup,
            ControlMsg::CreateSession {
                req_id: 1,
                parent: None,
                cwd: state.path().to_string_lossy().into_owned(),
                invocation: Some("/opt/bin/claude --verbose".to_string()),
                profile_name: None,
                title: Some("resolved".to_string()),
                cols: 80,
                rows: 24,
                intent_key: Some("resolved-key".to_string()),
                agent_kind: Some(AgentKind::Claude),
                resume_template: None,
                source_profile: Some(WireProfileSnapshot {
                    id: "profile-1".to_string(),
                    name: "Claude".to_string(),
                }),
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
        let reply: ControlMsg =
            serde_json::from_slice(&rx.recv().await.expect("create reply").body).expect("decode");
        let ControlMsg::SessionCreated { session, .. } = reply else {
            panic!("resolved create must succeed: {reply:?}");
        };
        assert_eq!(session.invocation, "/opt/bin/claude --verbose");
        assert_eq!(
            session.source_profile,
            Some(farhelm_proto::SourceProfile {
                id: "profile-1".to_string(),
                name: "Claude".to_string(),
                existence: farhelm_proto::ProfileExistence::Unresolved,
            })
        );
        let stored = sup
            .store
            .session(&session.id)
            .await
            .expect("read stored session")
            .expect("session exists");
        assert_eq!(stored.agent_kind, AgentKind::Claude);
        assert_eq!(stored.source_profile.unwrap().id, "profile-1");
    }

    /// A selectorless spawn copies the authenticated parent's complete
    /// stored agent bundle without any helm attachment.
    ///
    /// This is the offline scripting contract: the parent, not ambient host
    /// history, determines invocation, integration, resume, and provenance.
    #[tokio::test]
    async fn selectorless_spawn_copies_the_asking_sessions_bundle_offline() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let auth = authenticated_parent(&sup, state.path(), "parent").await;
        handle_restricted_control(
            &sup,
            ControlMsg::CreateSession {
                req_id: 2,
                parent: Some("parent".to_string()),
                cwd: state.path().to_string_lossy().into_owned(),
                invocation: None,
                profile_name: None,
                title: Some("child".to_string()),
                cols: 80,
                rows: 24,
                intent_key: Some("spawn-copy".to_string()),
                agent_kind: None,
                resume_template: None,
                source_profile: None,
            },
            &tx,
            &auth,
        )
        .await;
        let reply: ControlMsg =
            serde_json::from_slice(&rx.recv().await.expect("spawn reply").body).expect("decode");
        let ControlMsg::SessionCreated { session, .. } = reply else {
            panic!("selectorless spawn must succeed: {reply:?}");
        };
        let stored = sup
            .store
            .session(&session.id)
            .await
            .expect("read child")
            .expect("child exists");
        assert_eq!(stored.parent.as_deref(), Some("parent"));
        assert_eq!(stored.invocation, "/fixture/parent-agent --flag");
        assert_eq!(stored.agent_kind, AgentKind::Codex);
        assert_eq!(
            stored.resume_template,
            Some(vec![
                "/fixture/parent-agent".to_string(),
                "resume".to_string(),
                crate::agent_kind::CONVERSATION_PLACEHOLDER.to_string(),
            ])
        );
        assert_eq!(stored.source_profile.unwrap().id, "parent-profile");
    }

    /// A named spawn with no attached helm refuses with both available
    /// remedies instead of falling back to the parent's agent silently.
    #[tokio::test]
    async fn named_spawn_without_a_helm_explains_how_to_reuse_the_parent() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let auth = authenticated_parent(&sup, state.path(), "parent").await;
        handle_restricted_control(
            &sup,
            ControlMsg::CreateSession {
                req_id: 3,
                parent: Some("parent".to_string()),
                cwd: state.path().to_string_lossy().into_owned(),
                invocation: None,
                profile_name: Some("Claude".to_string()),
                title: None,
                cols: 80,
                rows: 24,
                intent_key: None,
                agent_kind: None,
                resume_template: None,
                source_profile: None,
            },
            &tx,
            &auth,
        )
        .await;
        let reply: ControlMsg =
            serde_json::from_slice(&rx.recv().await.expect("refusal").body).expect("decode");
        let ControlMsg::Error { kind, message, .. } = reply else {
            panic!("named spawn without a helm must fail: {reply:?}");
        };
        assert_eq!(kind, ErrorKind::Unavailable);
        assert!(message.contains("attached helm"));
        assert!(message.contains("omit --agent"));
        assert!(message.contains("asking session's agent"));
    }
    use std::time::Duration;

    /// The pre-storage create refusals, driven through the
    /// real dispatcher against a real supervisor rather than through
    /// `create_mode` alone: what matters is not only that the resolver
    /// returns an error, but that the error reaches the CALLER as a
    /// correlated `InvalidRequest` and that NOTHING was created on the way.
    ///
    /// The table covers every full-authority shape refusal: naming no
    /// selector, naming multiple selectors, and pairing the unresolved
    /// profile-name selector with any resolved-bundle field. The override
    /// rows are the subtle ones — a client that "helpfully" forwards a
    /// default `agent_kind` alongside a profile selection has written a
    /// request whose meaning nobody can defend, and the refusal stops an
    /// invented precedence rule at launch time.
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
        for (
            req_id,
            invocation,
            profile_name,
            agent_kind,
            resume_template,
            source_profile,
            expected,
        ) in [
            (1u64, None, None, None, None, None, "invocation bundle"),
            (
                2,
                Some("agent".to_string()),
                Some("Claude Code".to_string()),
                None,
                None,
                None,
                "exactly one",
            ),
            (
                3,
                None,
                Some("Claude Code".to_string()),
                Some(AgentKind::Claude),
                None,
                None,
                "bundle fields",
            ),
            (
                4,
                None,
                Some("Claude Code".to_string()),
                None,
                Some(vec!["claude".to_string(), "{conversation}".to_string()]),
                None,
                "bundle fields",
            ),
            (
                5,
                None,
                Some("Claude Code".to_string()),
                None,
                None,
                None,
                "session-authenticated spawn",
            ),
            (
                6,
                None,
                Some("Claude Code".to_string()),
                None,
                None,
                Some(WireProfileSnapshot {
                    id: "profile-1".to_string(),
                    name: "Claude Code".to_string(),
                }),
                "bundle fields",
            ),
            (
                7,
                None,
                None,
                None,
                None,
                Some(WireProfileSnapshot {
                    id: "profile-1".to_string(),
                    name: "Claude Code".to_string(),
                }),
                "without an invocation",
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
                    source_profile,
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

    /// Spec: an in-flight `AgentRequest` mutation's fence on session `id` —
    /// `Supervisor::agent_request_locks`, claimed by `handle_restricted_control`
    /// for the whole of any `AgentVerb::is_mutating` upcall — makes a
    /// concurrent `DeleteSession` for the SAME id wait, rather than let the
    /// delete invalidate the credential that mutation was authorized under.
    ///
    /// This is the mechanism the `AgentRequest` arm's stale comment used to
    /// contradict ("the two verbs this version carries are read-only
    /// questions"): with mutating verbs on the wire, a session deleted
    /// between its credential's one-time validation and the mutation's
    /// completion would otherwise keep authority it had already lost. The
    /// fence is claimed directly here, the way the `AgentRequest` handler
    /// itself would, rather than by driving a real relay round trip —
    /// exercising the relay's own routing belongs to
    /// `tests/e2e/agent_relay.rs`, and this test's whole point is the LOCAL
    /// ordering guarantee this crate owns end to end.
    ///
    /// No session row exists for `s1` in this fixture, which is deliberate:
    /// the interesting fact is that the delete WAITS while the fence is
    /// held, not what it finds once it looks — so it resolves to `NotFound`
    /// the moment it is finally allowed to run, and a `SessionDeleted`
    /// here would mean the test built a real session by accident and
    /// stopped exercising the fence.
    #[tokio::test]
    async fn restricted_delete_waits_for_an_in_flight_agent_request_fence() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .unwrap();
        let fence = sup.agent_request_locks.claim("s1").await;

        let (mut delete_tasks, mut delete_rx) = dispatch_for_test(
            &sup,
            ControlMsg::DeleteSession {
                req_id: 1,
                session_id: "s1".to_string(),
            },
        )
        .await;
        // The delete's OWN claim of `s1` — the second on that key, after
        // the one held above — which is what makes the empty queue below an
        // ordering fact. Yielding a turn or two instead proves nothing: a
        // task that has not been polled at all leaves exactly the same
        // empty queue as one parked on the fence.
        //
        // Bounded: the observation is unbounded by nature, so against the
        // very regression it exists to catch — a delete that stopped
        // claiming the fence — an unbounded await parks the suite instead
        // of failing it.
        tokio::time::timeout(
            Duration::from_secs(5),
            sup.agent_request_locks.claims_reached_for_test("s1", 2),
        )
        .await
        .expect("the delete never reached the agent-request fence");
        assert!(
            matches!(
                delete_rx.try_recv(),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty)
            ),
            "delete must wait while an agent-request fence is held on the same session"
        );

        drop(fence);
        delete_tasks.join_next().await.unwrap().unwrap();
        let reply: ControlMsg =
            serde_json::from_slice(&delete_rx.try_recv().unwrap().body).unwrap();
        assert!(
            matches!(
                reply,
                ControlMsg::Error {
                    req_id: 1,
                    kind: ErrorKind::NotFound,
                    ..
                }
            ),
            "expected a not-found refusal once the fence cleared, got {reply:?}"
        );
    }

    /// Spec: dispatching a MUTATING `AgentRequest` claims the delete fence
    /// as a side effect — before the relay runs, and before the credential
    /// is even checked — while a read-only verb claims nothing.
    ///
    /// The sibling test above proves the DELETE side of the same fence by
    /// claiming the key by hand, which leaves the other half unproven: that
    /// dispatch takes it at all. Nothing in the handler's shape forces it —
    /// the claim is one hoisted `if` away from being lost in a refactor,
    /// and losing it breaks nothing that any other test observes, since the
    /// relay answers `Unavailable` either way with no helm attached.
    ///
    /// Both directions are asserted from ONE fixture on purpose. "The
    /// mutation parked" is only meaningful next to "the listing did not" —
    /// a handler that had simply stopped answering would satisfy the first
    /// clause alone, and a fence claimed for every verb would serialize
    /// every listing behind an unrelated delete for no correctness gain.
    ///
    /// The verb is `Rename` rather than `Stop`/`Archive` because the point
    /// is reached before any of the three diverge, and rename is the one
    /// whose name does not invite a reader to wonder what it tore down.
    #[tokio::test]
    async fn a_mutating_agent_request_claims_the_delete_fence_and_a_listing_does_not() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .unwrap();
        let auth = authenticated_parent(&sup, state.path(), "asker").await;
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let fence = sup.agent_request_locks.claim("asker").await;

        // The control. No helm is attached, so the relay refuses at once —
        // which is exactly what makes the refusal ARRIVING the evidence:
        // had a listing taken the fence, this call could not have returned.
        handle_restricted_control(
            &sup,
            ControlMsg::AgentRequest {
                req_id: 1,
                session_id: "asker".to_string(),
                request: AgentVerb::Hosts {},
            },
            &tx,
            &auth,
        )
        .await;
        let listing = rx.try_recv().expect("a listing must not wait on the fence");
        assert!(matches!(
            serde_json::from_slice::<ControlMsg>(&listing.body).unwrap(),
            ControlMsg::AgentResponse { req_id: 1, .. }
        ));

        let mutation = tokio::spawn({
            let sup = Arc::clone(&sup);
            let tx = tx.clone();
            let auth = auth.clone();
            async move {
                handle_restricted_control(
                    &sup,
                    ControlMsg::AgentRequest {
                        req_id: 2,
                        session_id: "asker".to_string(),
                        request: AgentVerb::Rename {
                            session_id: None,
                            title: "new title".to_string(),
                        },
                    },
                    &tx,
                    &auth,
                )
                .await;
            }
        });
        // The dispatch's own claim of `asker` — the second on that key,
        // after the one held above — which is the observation that makes
        // the empty queue below mean "parked on the fence". A count of
        // scheduler turns cannot: a task that has not been polled leaves
        // the same empty queue as one that is genuinely blocked, so a
        // version that had lost the claim entirely would still pass.
        // Bounded so that a dispatch which stopped claiming fails here
        // rather than parking the suite on an observation that can no
        // longer arrive.
        tokio::time::timeout(
            Duration::from_secs(5),
            sup.agent_request_locks.claims_reached_for_test("asker", 2),
        )
        .await
        .expect("the mutating dispatch never reached the fence, so it never claimed it");
        assert!(
            matches!(
                rx.try_recv(),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty)
            ),
            "a mutating verb must park on the fence rather than relaying under it"
        );

        drop(fence);
        mutation.await.expect("the parked dispatch finishes");
        let answered = rx.try_recv().expect("the mutation answers once released");
        assert!(matches!(
            serde_json::from_slice::<ControlMsg>(&answered.body).unwrap(),
            ControlMsg::AgentResponse { req_id: 2, .. }
        ));
    }

    /// Spec: the delete fence is claimed BEFORE the asker's credential is
    /// checked — a `DeleteSession` for the same id cannot get in while that
    /// check is in flight.
    ///
    /// This is the ordering the `AgentRequest` arm's comment calls "the
    /// whole guarantee", and it is exactly the thing its two sibling tests
    /// cannot see. Both of them observe the fence from OUTSIDE the check:
    /// one pre-claims the key and watches a mutation park, the other
    /// pre-claims it and watches a delete park, and an implementation that
    /// authenticated first and claimed afterwards would satisfy both. What
    /// the order actually buys is the interval covered here: with the check
    /// unfenced, a delete for this same session can take the key, finish the
    /// entire teardown, and drop it again while `authenticates_session` is
    /// still reading — after which the claim succeeds instantly against a
    /// session that no longer exists and the mutation is relayed on a
    /// credential the delete already revoked. Nothing downstream
    /// re-authorizes it.
    ///
    /// The interval is a database read wide in production, which is to say
    /// unobservable by timing; `SupervisorSeams::agent_auth_gate` holds it
    /// open instead. That seam is only meaningful where it sits — below the
    /// claim, above the check — so a refactor that moves the claim must move
    /// the gate with it or this test stops pinning anything.
    #[tokio::test]
    async fn the_delete_fence_is_claimed_before_the_credential_is_checked() {
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
                agent_auth_gate: Some(Arc::new(move || {
                    let entered = Arc::clone(&gate_entered);
                    let release = Arc::clone(&gate_release);
                    Box::pin(async move {
                        entered.notify_one();
                        release.notified().await;
                    })
                })),
                ..SupervisorSeams::default()
            },
        )
        .await
        .unwrap();
        let auth = authenticated_parent(&sup, state.path(), "asker").await;
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);

        let mutation = tokio::spawn({
            let sup = Arc::clone(&sup);
            let tx = tx.clone();
            let auth = auth.clone();
            async move {
                handle_restricted_control(
                    &sup,
                    ControlMsg::AgentRequest {
                        req_id: 1,
                        session_id: "asker".to_string(),
                        request: AgentVerb::Stop { session_id: None },
                    },
                    &tx,
                    &auth,
                )
                .await;
            }
        });
        // Parked inside the window: the claim is behind us, the credential
        // check is not.
        entered.notified().await;

        let (mut delete_tasks, mut delete_rx) = dispatch_for_test(
            &sup,
            ControlMsg::DeleteSession {
                req_id: 2,
                session_id: "asker".to_string(),
            },
        )
        .await;
        // The delete's own claim of `asker` — the second on that key, after
        // the mutation's, which is parked in the gate above still holding
        // it. Waiting for the ARRIVAL rather than for a number of scheduler
        // turns is what makes the empty queue below evidence: an unpolled
        // task and a fenced one are indistinguishable by turn count, so the
        // turn-based form passed whether or not the delete ever reached the
        // lock. Bounded, so a claim that moved above the credential check
        // (which is exactly what this test forbids) fails here instead of
        // hanging.
        tokio::time::timeout(
            Duration::from_secs(5),
            sup.agent_request_locks.claims_reached_for_test("asker", 2),
        )
        .await
        .expect("the delete never reached the fence the gated mutation is holding");
        assert!(
            matches!(
                delete_rx.try_recv(),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty)
            ),
            "a delete must not run while the mutation's credential is being checked under the \
             fence"
        );

        release.notify_one();
        mutation.await.expect("the gated dispatch finishes");
        let answered = rx.try_recv().expect("the mutation answers once released");
        assert!(matches!(
            serde_json::from_slice::<ControlMsg>(&answered.body).unwrap(),
            ControlMsg::AgentResponse { req_id: 1, .. }
        ));

        // And the delete runs afterwards, which is what makes the emptiness
        // above a wait rather than a delete that was never going to answer.
        // It resolves to `NotFound` — the asking row is seeded in the store
        // but not in the live session map, exactly as in
        // `restricted_delete_waits_for_an_in_flight_agent_request_fence` —
        // and what matters here is WHEN it answers, not what it found.
        delete_tasks.join_next().await.unwrap().unwrap();
        let deleted: ControlMsg =
            serde_json::from_slice(&delete_rx.try_recv().unwrap().body).unwrap();
        assert!(
            matches!(
                deleted,
                ControlMsg::Error {
                    req_id: 2,
                    kind: ErrorKind::NotFound,
                    ..
                }
            ),
            "the delete must run once the fence clears, got {deleted:?}"
        );
    }

    /// Spec: a verb `validate_agent_verb` refuses is refused BY DISPATCH,
    /// as an `AgentResponse` carrying `InvalidRequest` — not as a bare
    /// `Error`, and not by being forwarded and refused somewhere else.
    ///
    /// The unit test below covers the predicate; this covers the wiring,
    /// which is a separate thing to get wrong in two ways. The validator
    /// could stop being called at all (the relay would then carry the
    /// hostile target onto two byte-unbounded queues, which is the whole
    /// reason it exists), and its refusal could be sent as a
    /// `ControlMsg::Error` instead — a shape the asking CLI decodes on a
    /// different path from every other relay refusal, so half the failures
    /// would render differently from the other half.
    #[tokio::test]
    async fn dispatch_refuses_an_invalid_agent_verb_as_an_agent_response() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .unwrap();
        let auth = authenticated_parent(&sup, state.path(), "asker").await;
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);

        handle_restricted_control(
            &sup,
            ControlMsg::AgentRequest {
                req_id: 9,
                session_id: "asker".to_string(),
                request: AgentVerb::Rename {
                    session_id: Some("evil\nid".to_string()),
                    title: "fine".to_string(),
                },
            },
            &tx,
            &auth,
        )
        .await;

        let frame = rx.try_recv().expect("dispatch answers immediately");
        let ControlMsg::AgentResponse {
            req_id: 9,
            outcome: farhelm_proto::AgentOutcome::Err { kind, message },
        } = serde_json::from_slice::<ControlMsg>(&frame.body).unwrap()
        else {
            panic!("a validator refusal must travel as an AgentResponse");
        };
        assert_eq!(kind, ErrorKind::InvalidRequest);
        assert!(
            message.contains("control character"),
            "the refusal must name what was wrong: {message}"
        );
    }

    /// Spec: the `agent_request_locks` fence and a session's own
    /// `lifecycle_locks` claim are independent locks even for the identical
    /// id — the property that keeps a self-targeting lifecycle verb (no
    /// `--session`: the asking session acting on itself) from deadlocking
    /// against its own fence.
    ///
    /// See `Supervisor::agent_request_locks`'s docs for the two-thread
    /// argument this pins: if the fence reused `lifecycle_locks` for the
    /// same key, a self-stop's own target-side execution — which claims
    /// `lifecycle_locks` for that same id — would wait forever on a fence
    /// held by the very upcall waiting for it to finish. Wrapped in an
    /// explicit timeout rather than left to hang, so a regression here
    /// fails this test instead of wedging the run.
    #[tokio::test]
    async fn agent_request_fence_does_not_block_the_same_ids_lifecycle_claim() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .unwrap();
        let _fence = sup.agent_request_locks.claim("s1").await;

        let claimed =
            tokio::time::timeout(Duration::from_secs(2), sup.lifecycle_locks.claim("s1")).await;
        assert!(
            claimed.is_ok(),
            "a lifecycle claim on the same id must not be blocked by an agent-request fence"
        );
    }

    /// Spec: `validate_agent_verb` bounds and sanitizes exactly the fields
    /// the merged review flagged as unbounded at this hop — an explicit
    /// `--session` target and a `Rename` title, admits the two public
    /// read-only verbs, and rejects the helm-internal profile resolver at
    /// session-authenticated ingress. Every otherwise-legal target and title
    /// remains untouched.
    ///
    /// The BOUNDARY cases are the ones worth spelling out. A bound is a
    /// place where an off-by-one is invisible from either side: a cap that
    /// refused a value of exactly its own size would reject ids the
    /// connection hello had already admitted, and one that admitted a value
    /// one over would leave the queue it protects unbounded by exactly the
    /// margin somebody would eventually find. The multibyte case pins the
    /// other half of the same rule — the limit counts BYTES, because bytes
    /// are what ride the queue, and `str::len` agreeing with character
    /// count in every ASCII fixture is what would hide a switch to
    /// `chars().count()`.
    ///
    /// The empty TITLE is admitted on purpose and asserted here so it stays
    /// that way: the target supervisor accepts one (see
    /// `RenameSession::title`), and a doorway check that refused it would
    /// be inventing a rule the far end does not have.
    #[test]
    fn validate_agent_verb_bounds_targets_and_titles() {
        assert!(validate_agent_verb(&AgentVerb::Hosts {}).is_ok());
        assert!(validate_agent_verb(&AgentVerb::Sessions {}).is_ok());
        assert!(
            validate_agent_verb(&AgentVerb::Stop { session_id: None }).is_ok(),
            "omitting --session (asking-session substitution) is always valid"
        );
        assert!(
            validate_agent_verb(&AgentVerb::Stop {
                session_id: Some("other".to_string())
            })
            .is_ok()
        );

        let empty = validate_agent_verb(&AgentVerb::Stop {
            session_id: Some(String::new()),
        });
        assert!(empty.unwrap_err().contains("empty"));

        let oversized = validate_agent_verb(&AgentVerb::Archive {
            session_id: Some("x".repeat(MAX_SESSION_ID_BYTES + 1)),
        });
        assert!(
            oversized
                .unwrap_err()
                .contains(&MAX_SESSION_ID_BYTES.to_string())
        );

        let control_char = validate_agent_verb(&AgentVerb::Rename {
            session_id: Some("evil\nid".to_string()),
            title: "fine".to_string(),
        });
        assert!(control_char.unwrap_err().contains("control character"));

        let oversized_title = validate_agent_verb(&AgentVerb::Rename {
            session_id: None,
            title: "x".repeat(CREATE_FIELD_CAP + 1),
        });
        assert!(
            oversized_title
                .unwrap_err()
                .contains(&CREATE_FIELD_CAP.to_string())
        );

        let control_title = validate_agent_verb(&AgentVerb::Rename {
            session_id: None,
            title: "one\ntwo".to_string(),
        });
        assert!(
            control_title.unwrap_err().contains("control characters"),
            "a title is held to the same rule the target and the far end are"
        );

        // Exactly at each cap: admitted, both of them.
        assert!(
            validate_agent_verb(&AgentVerb::Archive {
                session_id: Some("x".repeat(MAX_SESSION_ID_BYTES)),
            })
            .is_ok(),
            "a target of exactly the cap is inside it"
        );
        assert!(
            validate_agent_verb(&AgentVerb::Rename {
                session_id: None,
                title: "x".repeat(CREATE_FIELD_CAP),
            })
            .is_ok(),
            "a title of exactly the cap is inside it"
        );
        assert!(
            validate_agent_verb(&AgentVerb::Rename {
                session_id: None,
                title: String::new(),
            })
            .is_ok(),
            "the empty title is legal here because it is legal at the target"
        );

        // Multibyte: half as many characters as the cap allows, twice as
        // many bytes — refused, because the bound is on bytes.
        let multibyte = "é".repeat(MAX_SESSION_ID_BYTES);
        assert_eq!(multibyte.chars().count(), MAX_SESSION_ID_BYTES);
        assert!(
            validate_agent_verb(&AgentVerb::Stop {
                session_id: Some(multibyte),
            })
            .is_err(),
            "the cap counts bytes, not characters"
        );
    }

    /// Spec: the two CREATING verbs pass with every optional absent, refuse
    /// an empty or control-laced `--host`, refuse a field TOTAL past
    /// [`CREATE_FIELD_CAP`] even when no single field exceeds it, and hold
    /// an intent key to [`INTENT_KEY_CAP`] — with `Clone` bounded
    /// identically to `Create`.
    ///
    /// The SUM clause is the one worth a test of its own. Each of cwd,
    /// selector and title is individually free to be large, and a
    /// per-field check would let a request three times the intended
    /// ceiling through the two byte-unbounded queues this validation
    /// exists to protect (this connection's writer queue, then the helm's)
    /// — the same reasoning `handle_create_session` applies to a create
    /// arriving on the wire directly.
    ///
    /// The identical-bounds clause matters because `Clone` is a create
    /// whose selector the HELM derives: a looser cap on it would be a way
    /// to push bytes through the other verb's doorway, and the two field
    /// lists overlapping only partially is exactly how such a gap gets
    /// written by accident.
    ///
    /// EVERY payload field is checked at the cap and one byte past it, not
    /// just a representative pair. The aggregate is the kind of rule an
    /// implementation can partially forget — dropping the title or the
    /// profile name from the sum leaves every remaining case passing — so
    /// coverage of two fields would have said nothing about the other two.
    /// The host name is pinned separately, on its own allowance, for the
    /// reason [`validate_create_fields`] gives.
    #[test]
    fn validate_agent_verb_bounds_the_creating_verbs() {
        // A builder rather than one base value spread with `..`: struct
        // ENUM variants have no functional-update syntax, so varying one
        // field at a time needs a closure.
        let full = |host: Option<&str>,
                    cwd: &str,
                    profile_name: Option<&str>,
                    invocation: Option<&str>,
                    title: Option<&str>,
                    intent_key: Option<&str>| AgentVerb::Create {
            host: host.map(str::to_string),
            cwd: cwd.to_string(),
            profile_name: profile_name.map(str::to_string),
            invocation: invocation.map(str::to_string),
            title: title.map(str::to_string),
            intent_key: intent_key.map(str::to_string),
        };
        let create =
            |host: Option<&str>, cwd: &str, invocation: Option<&str>, intent_key: Option<&str>| {
                full(host, cwd, None, invocation, None, intent_key)
            };
        assert!(
            validate_agent_verb(&create(None, "/w", None, None)).is_ok(),
            "a create naming only a directory is the ordinary shape"
        );
        assert!(
            validate_agent_verb(&AgentVerb::Clone {
                host: None,
                cwd: None,
                title: None,
                intent_key: None,
            })
            .is_ok(),
            "a clone naming nothing at all means \"another one of these, here\""
        );

        let empty_host = validate_agent_verb(&create(Some(""), "/w", None, None));
        assert!(empty_host.unwrap_err().contains("empty"));

        let control_host = validate_agent_verb(&AgentVerb::Clone {
            host: Some("evil\nhost".to_string()),
            cwd: None,
            title: None,
            intent_key: None,
        });
        assert!(control_host.unwrap_err().contains("control character"));

        // A host name is bounded on its OWN allowance rather than against
        // the create payload, because it is routing metadata the helm
        // consumes and no supervisor ever sees. Pinned here because the
        // previous shape charged it to the payload, which let a long
        // registered host name make an otherwise-legal create fail only
        // through the agent surface.
        let long_host = validate_agent_verb(&create(
            Some(&"h".repeat(AGENT_HOST_NAME_CAP + 1)),
            "/w",
            None,
            None,
        ));
        assert!(
            long_host
                .unwrap_err()
                .contains(&AGENT_HOST_NAME_CAP.to_string()),
            "the host name has its own limit, not the create payload's"
        );
        assert!(
            validate_agent_verb(&full(
                Some(&"h".repeat(AGENT_HOST_NAME_CAP)),
                &"x".repeat(CREATE_FIELD_CAP),
                None,
                None,
                None,
                None,
            ))
            .is_ok(),
            "a host name at its cap does not eat into the create payload's cap"
        );

        // EVERY payload field contributes to the ONE total, and each is
        // checked at the cap and one byte past it. Exercising only two of
        // them (which is what this test used to do) would let an
        // implementation that stopped charging the profile name or the
        // title pass unchanged.
        for (label, at_cap, over_cap) in [
            (
                "cwd alone",
                full(None, &"x".repeat(CREATE_FIELD_CAP), None, None, None, None),
                full(
                    None,
                    &"x".repeat(CREATE_FIELD_CAP + 1),
                    None,
                    None,
                    None,
                    None,
                ),
            ),
            (
                "profile name",
                full(
                    None,
                    "/w",
                    Some(&"p".repeat(CREATE_FIELD_CAP - 2)),
                    None,
                    None,
                    None,
                ),
                full(
                    None,
                    "/w",
                    Some(&"p".repeat(CREATE_FIELD_CAP - 1)),
                    None,
                    None,
                    None,
                ),
            ),
            (
                "invocation",
                full(
                    None,
                    "/w",
                    None,
                    Some(&"i".repeat(CREATE_FIELD_CAP - 2)),
                    None,
                    None,
                ),
                full(
                    None,
                    "/w",
                    None,
                    Some(&"i".repeat(CREATE_FIELD_CAP - 1)),
                    None,
                    None,
                ),
            ),
            (
                "title",
                full(
                    None,
                    "/w",
                    None,
                    None,
                    Some(&"t".repeat(CREATE_FIELD_CAP - 2)),
                    None,
                ),
                full(
                    None,
                    "/w",
                    None,
                    None,
                    Some(&"t".repeat(CREATE_FIELD_CAP - 1)),
                    None,
                ),
            ),
        ] {
            assert!(
                validate_agent_verb(&at_cap).is_ok(),
                "{label} exactly at the cap is legal"
            );
            assert!(
                validate_agent_verb(&over_cap)
                    .unwrap_err()
                    .contains(&CREATE_FIELD_CAP.to_string()),
                "{label} one byte past the cap is refused"
            );
        }

        // Neither third exceeds the cap alone; together they do. A
        // per-field check would accept this.
        let third = "x".repeat(CREATE_FIELD_CAP / 3 + 1);
        let summed =
            validate_agent_verb(&full(None, &third, Some(&third), None, Some(&third), None));
        assert!(
            summed.unwrap_err().contains(&CREATE_FIELD_CAP.to_string()),
            "cwd, selector and title share one total"
        );

        let half = "x".repeat(CREATE_FIELD_CAP / 2 + 1);
        let summed_clone = validate_agent_verb(&AgentVerb::Clone {
            host: None,
            cwd: Some(half.clone()),
            title: Some(half),
            intent_key: None,
        });
        assert!(
            summed_clone
                .unwrap_err()
                .contains(&CREATE_FIELD_CAP.to_string()),
            "a clone is bounded by the same total a create is"
        );

        let empty_key = validate_agent_verb(&create(None, "/w", None, Some("")));
        assert!(empty_key.unwrap_err().contains("empty"));

        let oversized_key = validate_agent_verb(&AgentVerb::Clone {
            host: None,
            cwd: None,
            title: None,
            intent_key: Some("k".repeat(INTENT_KEY_CAP + 1)),
        });
        assert!(
            oversized_key
                .unwrap_err()
                .contains(&INTENT_KEY_CAP.to_string())
        );
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

        handle_restricted_control(&sup, ControlMsg::ListSessions { req_id: 41 }, &tx, &auth).await;
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
                source_profile: None,
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
                source_profile: None,
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
            source_profile: None,
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
                source_profile: None,
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
                    source_profile: None,
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
                source_profile: None,
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
                    source_profile: None,
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
    /// `reply_frame` in isolation. There is no byte budget in
    /// `listing::list_all` any more — the wire protocol dropped pagination
    /// entirely (`LIST_SESSIONS_CAP` is a ROW cap, not a size one) — so an
    /// oversized reply is caught only where it always could be: at
    /// `reply_frame`'s own encode-and-check backstop, which
    /// `reply_frame_substitutes_error_for_oversized_reply` in
    /// `connection`'s tests pins directly. This test pins the same
    /// defusal through the REAL call site (`handle_list_sessions` ->
    /// `send_reply` -> `reply_frame`) with a fixture that is not
    /// contrived to hit a cap that no longer exists — a single session
    /// whose title alone exceeds `MAX_FRAME_LEN` is already enough. It
    /// also proves the refusal is scoped to its own request: a second,
    /// ordinary request on the same connection (same `tx`) must still get
    /// an honest reply.
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
            ControlMsg::ListSessions { req_id: 1 },
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
        // The backstop speaks in frame terms, not session terms: it sees
        // an encoded reply, not the row that made it too big. What matters
        // is that the caller gets an explicit refusal rather than a
        // silently dropped or truncated list.
        assert!(
            message.contains("frame limit"),
            "the refusal must say the reply exceeded the frame limit: {message}"
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
            ControlMsg::ListSessions { req_id: 2 },
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
            truncated,
        } = decoded2
        else {
            panic!("expected a normal ControlMsg::SessionList, got {decoded2:?}");
        };
        assert_eq!(req_id, 2);
        assert!(sessions.is_empty());
        assert!(!truncated);
    }

    /// Production call-site coverage for `LIST_SESSIONS_CAP`: seeding one
    /// session past it must come back as EXACTLY `LIST_SESSIONS_CAP` rows,
    /// `truncated: true`, and — because the ordering contract
    /// (`listing::list_order_key`) keeps the newest and drops the oldest —
    /// missing precisely the session created first. `fake_entry` gives
    /// every entry a synthetic, terminal-less (`terminal: None`) fixture,
    /// which is enough to drive the cap wiring without a single real tmux
    /// round trip (`session_status` returns `Exited` for a terminal-less
    /// entry without ever consulting `pane_states`) — `LIST_SESSIONS_CAP +
    /// 1` REAL tmux sessions would be slow and environment-dependent for
    /// no added signal.
    #[tokio::test]
    async fn list_sessions_honors_the_session_cap_at_the_handler_level() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");

        {
            let mut sessions = sup.sessions.lock().await;
            // created_at ascending with the loop index: s0 is the OLDEST
            // and therefore the one row the cap must drop.
            for i in 0..farhelm_proto::LIST_SESSIONS_CAP + 1 {
                let id = format!("s{i}");
                sessions.insert(id.clone(), fake_entry(&id, 1_700_000_000 + i as i64));
            }
        }

        let (sessions, truncated) = list_sessions(&sup, 1).await;
        assert_eq!(
            sessions.len(),
            farhelm_proto::LIST_SESSIONS_CAP,
            "the cap must win over the full count at the real handler call site"
        );
        assert!(
            truncated,
            "one session past the cap must be reported as a cut"
        );
        assert!(
            !sessions.iter().any(|s| s.id == "s0"),
            "the OLDEST session must be the one the cap drops"
        );
    }

    /// A terminal-less, synthetic session entry for the ordering tests
    /// below — real tmux/launch machinery is irrelevant to REPLY ORDER, so
    /// every fixture here skips it the same way
    /// `list_sessions_honors_the_session_cap_at_the_handler_level`'s
    /// fixtures do (`terminal: None`, decided by `session_status` without
    /// any tmux round trip).
    fn fake_entry(id: &str, created_at: i64) -> Arc<SessionEntry> {
        Arc::new(SessionEntry {
            info: SessionInfo {
                parent: None,
                archived: false,
                id: id.to_string(),
                title: "t".to_string(),
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
    /// dispatch and wait for its (spawned) reply — every ordering test
    /// below sends at least one such request, so the
    /// request/spawn/timeout/decode boilerplate lives here once rather
    /// than once per test. There is no page walk any more: a `ListSessions`
    /// reply is the WHOLE capped list in one shot, so this returns the
    /// sessions it carried and whether the cap cut any of them.
    async fn list_sessions(sup: &Arc<Supervisor>, req_id: u64) -> (Vec<SessionInfo>, bool) {
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        handle_control(
            sup,
            ControlMsg::ListSessions { req_id },
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
            truncated,
            ..
        } = decoded
        else {
            panic!("expected ControlMsg::SessionList, got {decoded:?}");
        };
        (sessions, truncated)
    }

    /// The protocol's headline ordering guarantee, through a REAL
    /// `ListSessions` reply rather than `listing::order_and_cut` in
    /// isolation: newest-first by `created_at`, id ascending on ties,
    /// across a set spanning several distinct `created_at` values (so both
    /// halves of the ordering key are exercised, not just the tiebreak).
    /// Ids are zero-padded (`s00`..`s11`) specifically so their STRING
    /// order matches their NUMERIC order, which is what lets this test's
    /// expected sequence be written out by hand without a second,
    /// independent sort to compare against — the tiebreak direction itself
    /// is exercised, and separately pinned, by
    /// `list_sessions_same_created_at_tiebreaks_ascending_by_id` below.
    #[tokio::test]
    async fn list_sessions_replies_in_creation_order_newest_first() {
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

        let (sessions, truncated) = list_sessions(&sup, 1).await;
        assert!(!truncated, "twelve sessions never approach the cap");
        assert_eq!(
            sessions.into_iter().map(|s| s.id).collect::<Vec<_>>(),
            expected,
            "the reply must be ordered newest-first exactly, with no duplicates and no gaps"
        );
    }

    /// The order's tiebreak direction, pinned in isolation: three sessions
    /// sharing one `created_at` must come back ascending by id.
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
        let (sessions, truncated) = list_sessions(&sup, 1).await;
        assert!(!truncated);
        assert_eq!(
            sessions.into_iter().map(|s| s.id).collect::<Vec<_>>(),
            vec!["sA".to_string(), "sB".to_string(), "sC".to_string()],
            "sessions sharing one created_at must tiebreak ascending by id"
        );
    }

    // Interleaved create/delete mid-walk, a resume cursor from a deleted
    // session, an all-unseen-deleted continuation, and the byte-budget
    // cut/resume tests that used to live here all tested PAGE-WALK
    // behavior — cursors, `limit`, a byte budget mid-list — that no longer
    // exists: a `ListSessions` reply is now the whole capped list in one
    // shot, so there is no walk to interleave a mutation into and no
    // budget to force a mid-page cut. Deleted rather than adapted.

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
            ControlMsg::ListSessions { req_id: 1 },
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
