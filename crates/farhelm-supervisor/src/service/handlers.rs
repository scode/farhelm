//! Named per-message handlers for `handle_control`'s dispatch.
//!
//! One `async fn handle_<message>` per substantial `ControlMsg` variant
//! (M4.5's structural pass — functional no-op, see PLAN.md's milestone
//! ladder); `handle_control` itself is just the dispatch match. Each
//! handler owns exactly the connection-local state its message can
//! mutate and otherwise leans on `Supervisor` methods (`service::core`)
//! and the other `service` submodules for everything else.

use super::connection::{
    Forwarder, notify_detached, reply_frame, send_reply, set_attachment_paused, spawn_admitted,
};
use super::core::{
    CreateInputs, LIST_BYTE_BUDGET, LIST_SESSION_CAP, SessionEntry, Supervisor, build_list_reply,
    create_fingerprint, dead_pane_exit_code, decode_list_cursor, ensure_title_printable,
    entry_info, error_kind, list_order_key, observation, truncate_for_error,
};
use super::launch_artifacts::{
    cleanup_launch_artifacts, read_launch_sentinel, remove_fail_closed,
    remove_launch_artifacts_for_session, sentinel_could_still_apply, wrapper_failure_detail,
};
use super::snapshots::{
    capture_alt_screen_before_stop, publish_alt_screen_snapshot, snapshot_path,
};
use super::sweep::{StopFailure, SweepTarget, reap_process_tree, stop_live_agent};
use super::terminals::{
    ActiveAttach, AttachmentKey, DETACH_REASON_REPLACED, DETACH_REASON_TAKEOVER, InputRoute,
    MAX_LEASE_BYTES, Terminal, TerminalId, displaced_by_attach, resolve_terminal,
};
use super::uploads::{
    MAX_UPLOADS_PER_CONNECTION, UPLOAD_CHUNK_QUEUE, UPLOAD_SIGNAL_QUEUE, UploadCommand,
    UploadHandle, UploadOutcome, UploadRequest, UploadRoute, UploadSignal, abort_session_uploads,
    commit_without_upload, run_upload,
};
use crate::store::{IntentClaim, LastOutcome, Transition};
use crate::tmux::PaneState;
use anyhow::Context;
use farhelm_proto::{
    AgentKind, ControlMsg, ErrorKind, Frame, RestartMode, SessionInfo, TerminalSelector,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio::sync::{mpsc, oneshot, watch};
use tracing::{debug, warn};

/// Combined byte cap on `CreateSession`'s `cwd` + `invocation` + `title`,
/// enforced before `create_session` does anything.
///
/// Without this, a request whose fields nearly fill `MAX_FRAME_LEN` can
/// succeed at creating the session, and only then discover that its
/// `SessionCreated` reply — the same fields again, plus the generated id
/// and the frame wrapper — exceeds the cap and gets degraded to an
/// `Error` by `reply_frame`. That leaves the session alive while the
/// caller is told the request failed, with no way to learn the id needed
/// to attach to (or tear down) the very session it just created. 64 KiB
/// is orders of magnitude beyond any real cwd, invocation, or title —
/// each of which must also survive being embedded in a tmux command line
/// — so capping the inputs this far below the frame limit makes an
/// oversized `SessionCreated` reply structurally impossible, and does so
/// before `create_session` has touched tmux or the filesystem.
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

/// Cap on how many argv elements `CreateSession`'s `resume_template`
/// override may carry (PLAN_M3.md items 6 and 7).
///
/// Independent of the byte cap it is enforced alongside, because the two
/// bound different things: a template of ten thousand EMPTY elements costs
/// almost nothing in bytes while still being nothing a resume invocation
/// could legitimately be, and it lands in the same never-pruned
/// reservation row. 64 elements is far beyond every real resume
/// invocation (`claude --resume {conversation}` is three).
const RESUME_TEMPLATE_ELEMENT_CAP: usize = 64;

/// Validates the caller-supplied fields against the reply-size and
/// idempotency-store caps, then hands off to [`Supervisor::create_session`].
#[allow(clippy::too_many_arguments)]
async fn handle_create_session(
    sup: &Arc<Supervisor>,
    tx: &mpsc::Sender<Frame>,
    req_id: u64,
    cwd: String,
    invocation: String,
    title: Option<String>,
    cols: u16,
    rows: u16,
    intent_key: Option<String>,
    // Two consumers, and they must see the SAME values: item 6's
    // fingerprint (a retry differing only in an override is a
    // different request and is refused as a key reuse) and item
    // 7's snapshot resolution, which is what makes the overrides
    // shape the session itself.
    agent_kind: Option<AgentKind>,
    resume_template: Option<Vec<String>>,
) {
    // One accounting for every caller-supplied field that this
    // request can make the supervisor STORE — the reply-size
    // argument `CREATE_FIELD_CAP` was introduced for, plus item
    // 6's: the fingerprint holds a copy of all of them, in a
    // reservation row that is never pruned, so an unbounded
    // override is an unbounded permanent write.
    let template_bytes: usize = resume_template
        .iter()
        .flatten()
        .map(|element| element.len())
        .sum();
    let field_len =
        cwd.len() + invocation.len() + title.as_deref().map_or(0, str::len) + template_bytes;
    let refusal = if field_len > CREATE_FIELD_CAP {
        Some(format!(
            "cwd, invocation, title, and resume template together are {field_len} bytes, \
             exceeding the {CREATE_FIELD_CAP}-byte limit"
        ))
    } else if resume_template
        .as_ref()
        .is_some_and(|template| template.len() > RESUME_TEMPLATE_ELEMENT_CAP)
    {
        // Bounded separately from the byte total because the two
        // are independent: a template of ten thousand EMPTY
        // elements costs almost no bytes and is still nothing a
        // resume invocation could legitimately be.
        Some(format!(
            "resume template has {} elements, exceeding the \
             {RESUME_TEMPLATE_ELEMENT_CAP}-element limit",
            resume_template.as_ref().map_or(0, Vec::len)
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
    let idempotency = intent_key.map(|intent_key| IntentClaim {
        intent_key,
        fingerprint: create_fingerprint(
            &cwd,
            &invocation,
            title.as_deref(),
            agent_kind,
            resume_template.as_deref(),
        ),
    });
    match sup
        .create_session(
            CreateInputs {
                cwd: &cwd,
                invocation: &invocation,
                title,
                cols,
                rows,
                agent_kind,
                resume_template,
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

/// What one pre-reply observation of a session concluded — the
/// per-entry half of a `ListSessions` pass, hoisted out so a
/// single-session reply can reach the same conclusions (PLAN_M5.md
/// item 3's `SessionRenamed`, whose `SessionInfo` must be built the
/// way a list builds one).
struct EntryObservation {
    /// A launch-sentinel or wrapper-failure detail found for this
    /// entry NOW. Outranks whatever `session_status` would compute,
    /// whether or not the matching transition also commits — see
    /// [`entry_info`]'s `sentinel` parameter.
    sentinel: Option<String>,
    /// The transition this observation wants committed, or `None`
    /// when nothing changed or this supervisor may not record.
    transition: Option<Transition>,
    /// This entry is ALREADY durably `Error`: there is nothing left to
    /// witness, and its launch artifacts are due for the idempotent
    /// cleanup a crash between an earlier commit and its cleanup can
    /// leave behind.
    settled_error: bool,
}

/// Look at one entry the way a `ListSessions` pass looks at it:
/// classify what its pane and launch artifacts say, without
/// committing anything.
///
/// Extracted from that pass rather than reimplemented, and shared
/// with it, because the precedence here is subtle and duplicating it
/// would eventually mean two different answers to "what happened to
/// this session" depending on which request asked. The order is
/// itself the contract (PLAN_M3.md items 2, 3 and 4): an entry
/// already durably `Error` is settled; otherwise a launch sentinel —
/// or the wrapper-failure shape that stands in for one — outranks
/// every inference, because a failed exec leaves an ordinary dead
/// pane that no probe can tell from a command that ran and finished;
/// only then does the plain pane observation apply.
///
/// Deliberately does NOT commit: the list pass batches every entry's
/// transition into ONE transaction, and taking that apart per entry
/// would turn one poll into a write per session. Callers commit what
/// they collect (`SessionStore::transition_many`) and then mirror
/// what it reports.
///
/// An unreadable sentinel is an `Err`, never a fall-through: basing a
/// reply on an inference the unreadable file might contradict is
/// exactly the silent-wrong-answer this refuses to give. The error
/// already names the session, so callers report it verbatim.
async fn observe_entry(
    sup: &Arc<Supervisor>,
    entry: &Arc<SessionEntry>,
    pane_states: &HashMap<String, PaneState>,
) -> anyhow::Result<EntryObservation> {
    let recorded = entry
        .outcome
        .lock()
        .expect("outcome mutex poisoned")
        .clone();
    // Borrowed out of the caller's map rather than cloned: this runs once
    // per entry on the polling path, and the pane state is only ever read.
    let live: Option<&PaneState> = entry.terminal.as_ref().and_then(|terminal| {
        pane_states
            .get(&terminal.pane)
            .filter(|state| state.session_name == terminal.tmux_name)
    });
    // Two different questions, deliberately not one: "no live
    // process" (which a sentinel check needs) and "a pane that
    // EXISTS and is dead" (which the wrapper-failure classifier
    // needs — see its docs for why an absent pane must not qualify).
    let dead_or_absent = live.is_none_or(|state| state.dead);
    let pane_dead = live.is_some_and(|state| state.dead);

    if matches!(recorded, LastOutcome::Error { .. }) {
        return Ok(EntryObservation {
            sentinel: None,
            transition: None,
            settled_error: true,
        });
    }

    // A sentinel is READ regardless of whether this supervisor
    // `may_record()` (item 2 of the review-swarm fix batch): a
    // degraded supervisor still has standing to REPORT what it can
    // read, even though it must not WRITE a conclusion it has no
    // standing to store — which is why `sentinel` and `transition`
    // below are set independently.
    if sentinel_could_still_apply(&recorded) && dead_or_absent {
        let found = read_launch_sentinel(&sup.state_dir, &entry.info.id, entry.generation)
            .await
            .with_context(|| {
                format!("could not read session {}'s launch sentinel", entry.info.id)
            })?;
        let detail = match found {
            Some(detail) => Some(detail),
            // The wrapper-failure shape: no sentinel, a pane that is
            // present and dead, and a launch spec nothing consumed.
            None => {
                wrapper_failure_detail(
                    &sup.state_dir,
                    &entry.info.id,
                    entry.generation,
                    entry.scope.is_some(),
                    pane_dead,
                )
                .await
            }
        };
        if let Some(detail) = detail {
            // No pane to rediscover here (unlike `reload_sessions`'s
            // by-name search): callers only visit sessions this
            // process already tracks a `Terminal` for or explicitly
            // does not, so there is nothing new for this transition
            // to record beyond the outcome itself.
            let transition = sup.may_record().then(|| Transition::SentinelError {
                detail: detail.clone(),
                pane: None,
            });
            return Ok(EntryObservation {
                sentinel: Some(detail),
                transition,
                settled_error: false,
            });
        }
    }

    let transition = if sup.may_record() {
        observation(&recorded, live)
    } else {
        None
    };
    Ok(EntryObservation {
        sentinel: None,
        transition,
        settled_error: false,
    })
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
/// It is single-flight and cheap in the steady state (see
/// `Supervisor::capture_now`).
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
        // `total` is captured, and the page sliced out, BEFORE a single
        // entry is cloned further or status-annotated: cloning the WHOLE
        // map (an `Arc` bump per entry, cheap — session counts are small
        // enough that sorting a snapshot per request needs no index) is
        // fine, but the PER-ENTRY status computation just below is not
        // free, and doing it for entries outside this page wastes work
        // proportional to however many sessions exist beyond one page.
        //
        // The lock's own critical section is just the snapshot — clone
        // every `Arc<SessionEntry>` and read `total` — nothing more:
        // sorting and partitioning the clone happen AFTER the guard drops,
        // so a request sorting however many thousand entries never holds
        // `sup.sessions` (and therefore blocks every OTHER connection's
        // create/list/stop/delete) for the duration. Still no index over
        // the map — PLAN_M6.md's "Pagination shape" explicitly rejects
        // one, since session counts stay small enough that sorting a
        // snapshot per request is cheaper than the bookkeeping an index
        // would cost on every create and delete.
        let (ordered, total): (Vec<Arc<SessionEntry>>, u64) = {
            let sessions = sup.sessions.lock().await;
            (sessions.values().cloned().collect(), sessions.len() as u64)
        };
        let (entries, more_beyond_page): (Vec<Arc<SessionEntry>>, bool) = {
            let mut ordered = ordered;
            ordered.sort_by(|a, b| list_order_key(&a.info).cmp(&list_order_key(&b.info)));
            // `partition_point` is valid here because `ordered` is sorted
            // by the SAME key the predicate compares against: "at or
            // before the cursor" is true for a prefix and false
            // afterward, so its boundary is exactly the first entry
            // strictly after the cursor — resuming, never re-serving.
            let start = match &cursor_key {
                Some(key) => {
                    let key = (std::cmp::Reverse(key.created_at), key.id.as_str());
                    ordered.partition_point(|e| list_order_key(&e.info) <= key)
                }
                None => 0,
            };
            let remaining = ordered.len() - start;
            let take_n = effective_limit.min(remaining);
            let page: Vec<Arc<SessionEntry>> = ordered[start..start + take_n].to_vec();
            let more_beyond_page = start + take_n < ordered.len();
            (page, more_beyond_page)
        };
        // Before the reply is computed, so an identity claimed on
        // this very pass is reflected in the `restart_offer` it
        // carries rather than only in the next poll's. Cheap by
        // construction for the steady state (see `capture_pass`'s
        // cost envelope), single-flight, and over EVERY session
        // rather than this reply's capped subset — see
        // `Supervisor::capture_now` for why the cap must not bind
        // the ambiguity rule.
        //
        // Note for whoever lands PLAN_M6.md item 5's helm-side drain loop:
        // this sweep runs PER PAGE, not once per `ListSessions` walk — a
        // caller paging through N pages to see the whole session set pays
        // N whole-host capture sweeps, not one. Harmless today (nothing
        // walks multiple pages in a tight loop), but the drain loop is
        // exactly the caller that would, so its design needs to account
        // for that multiplication rather than assume one sweep per walk.
        sup.capture_now().await;
        // ONE query for every session's liveness, not one per
        // session (`TmuxDriver::pane_states`'s own docs on why
        // that multiplies subprocess spawns under a polling UI) —
        // and skipped altogether when it could not possibly
        // change the answer: a terminal-less entry is decided
        // entirely by its recorded outcome (`session_status` never
        // consults the map for one), so a capped subset that is
        // ALL terminal-less (including the empty list) is fully
        // decidable without asking tmux anything. This matters
        // beyond just saving a subprocess spawn: it is what keeps
        // an authoritative "every session is a restart gap" (or
        // simply empty) listing from being turned into a spurious
        // `Internal` error by a private tmux server that happens
        // to ALSO be down for an unrelated reason.
        let pane_states = if entries.iter().any(|entry| entry.terminal.is_some()) {
            match sup.tmux.pane_states().await {
                Ok(states) => states,
                // Reached only for a genuinely UNCLASSIFIED tmux
                // failure: `TmuxDriver::pane_states` itself now
                // tolerates a vanished private tmux server (the
                // whole reason a dead-tmux-server `ListSessions`
                // no longer lands here at all — see that method's
                // own docs for why an empty pane-states map is
                // honest, not fabricated, in that case).
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
            }
        } else {
            HashMap::new()
        };
        // A list request is one of the places this supervisor
        // WITNESSES an exit (PLAN_M3.md item 2): the dead pane it
        // just found may be gone entirely by the next reboot,
        // taking its exit code with it, so the code is recorded
        // now — while tmux still has it — rather than recomputed
        // forever from a fact that expires. Every observation this
        // pass produces commits in ONE transaction, and the store
        // decides what each one means (`Transition::apply`), so a
        // stop running concurrently cannot have its annotation
        // erased by this list and this list cannot be misled by a
        // stale reading of its own.
        // A launch sentinel is READ regardless of whether this
        // supervisor `may_record()` (item 2 of the review-swarm
        // fix batch): a degraded supervisor (a handoff candidate,
        // or one whose boot-id read failed) still has standing to
        // REPORT what it can read, even though it must not WRITE a
        // conclusion it has no standing to store — the two halves
        // below are deliberately independent (`reply_status`
        // always reflects a sentinel this pass found; only
        // `observations` is gated on `may_record()`).
        //
        // A plain loop, not `observation()`'s pure `filter_map`
        // closure, because the sentinel check below is real I/O
        // (`read_launch_sentinel`) and therefore has to run
        // between two lock scopes rather than inside one
        // synchronous closure body: PLAN_M3.md item 3 wants the
        // SAME observation offered here that `reload_sessions`
        // offers — a non-terminal outcome whose pane is dead or
        // gone entirely gets its sentinel checked before falling
        // back to `observation()`'s plain exit inference, because
        // the sentinel outranks that inference exactly as surely
        // here as at reload, including (addition 18) for an entry
        // ALREADY recorded as an inferred `Interrupted` or
        // unannotated `Exited` — both are themselves only
        // inferences a sentinel is defined to beat.
        let mut observations: Vec<(String, i64, Transition)> = Vec::new();
        // This pass's sentinel finds, id to detail — used both to
        // gate post-commit file cleanup on the transition actually
        // landing, and (`reply_status`, below) to surface the
        // Error for THIS reply even when it could not be
        // committed durably this pass (`may_record()` false, or
        // the commit itself fails) — PLAN_M3.md item 3's
        // write-inability note: retain the file, retry
        // persistence on a later poll, but never let the reply
        // itself regress to a stale `Exited` in the meantime.
        let mut sentinel_hits: HashMap<String, String> = HashMap::new();
        for entry in &entries {
            let observed = match observe_entry(&sup, entry, &pane_states).await {
                Ok(observed) => observed,
                // Loud propagation, not fall-through (item 1): the
                // WHOLE request fails rather than silently basing
                // this — or any other — entry's reply on an
                // inference the unreadable sentinel might
                // contradict. Nothing gathered so far this pass is
                // committed: this `return` happens before
                // `transition_many` is ever called.
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
            if observed.settled_error {
                cleanup_launch_artifacts(&sup.state_dir, &entry.info.id, entry.generation).await;
                continue;
            }
            if let Some(detail) = observed.sentinel {
                sentinel_hits.insert(entry.info.id.clone(), detail);
            }
            if let Some(transition) = observed.transition {
                observations.push((entry.info.id.clone(), entry.generation, transition));
            }
        }
        if !observations.is_empty() {
            match sup.store.transition_many(observations).await {
                Ok(committed) => {
                    for entry in &entries {
                        if let Some(outcome) = committed.get(&entry.info.id) {
                            *entry.outcome.lock().expect("outcome mutex poisoned") =
                                outcome.clone();
                        }
                    }
                    // Cleanup folded into this successful arm
                    // (item 7), not a separate loop afterward: see
                    // `reload_sessions`'s identical step for the
                    // full lifecycle rationale (both files are
                    // cosmetic once the durable outcome already
                    // says what happened; a failed write must
                    // leave them for the next pass to retry
                    // against, hence gating on `committed` here
                    // rather than on `sentinel_hits` alone).
                    for entry in &entries {
                        if sentinel_hits.contains_key(&entry.info.id)
                            && matches!(
                                committed.get(&entry.info.id),
                                Some(LastOutcome::Error { .. })
                            )
                        {
                            cleanup_launch_artifacts(
                                &sup.state_dir,
                                &entry.info.id,
                                entry.generation,
                            )
                            .await;
                        }
                    }
                }
                // Logged, not fatal: the reply below is computed
                // from what this pass OBSERVED plus what is
                // durably recorded, both of which are still honest
                // when the write fails — and the next list retries.
                Err(e) => warn!(
                    error = %format!("{e:#}"),
                    "could not record observed session outcomes; \
                     the next list will retry"
                ),
            }
        }
        let sessions: Vec<SessionInfo> = entries
            .iter()
            .map(|entry| {
                entry_info(
                    entry,
                    &pane_states,
                    sentinel_hits.get(&entry.info.id).map(String::as_str),
                )
            })
            .collect();
        // `Err` is `build_list_reply`'s degenerate-budget case (Theme B of
        // the M6.75 review-swarm batch): the byte budget kept ZERO
        // entries while at least one candidate remained, so there is no
        // honest page to send — reporting `next_cursor: None` here would
        // lie that the walk was exhausted. Named by session id so the
        // caller (a human, in practice — no client is expected to react
        // programmatically to a page-shaped answer becoming impossible)
        // can tell which record is the problem.
        let reply =
            match build_list_reply(req_id, sessions, total, LIST_BYTE_BUDGET, more_beyond_page) {
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
                Ok(pane) => pane,
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

/// Spawned for the same reason as `StopSession` — the same
/// process-tree sweep, plus tmux teardown and SQLite writes on
/// top — and, being the slowest of the six handlers spawned
/// here, the one spawning matters most for. Safe for the
/// same reason: everything this handler touches (`sessions`,
/// `attachments`, `tmux`, `store`) is already designed to
/// tolerate concurrent requests interleaving (see the
/// `Supervisor` struct's lock-discipline docs, and this handler's
/// own existing comments on why the sweep runs before any lock
/// is held at all). Tracked and admitted exactly like
/// `ListSessions` above — see
/// `HANDLER_ADMISSION_PERMITS`/`HANDLER_SHUTDOWN_TIMEOUT`.
async fn handle_delete_session(
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
        // Same claim the stop and restart paths take, and the
        // reason delete-vs-restart resolves to one winner: without
        // it, a delete can tear down the tmux session a restart is
        // mid-way through respawning into, leaving the loser to
        // report a half-finished teardown rather than an honest
        // "it was deleted". See `Supervisor::lifecycle_locks`.
        let _lifecycle = sup.lifecycle_locks.claim(&session_id).await;
        let entry = sup.sessions.lock().await.get(&session_id).cloned();
        let Some(entry) = entry else {
            send_reply(
                &tx,
                &ControlMsg::Error {
                    req_id,
                    message: format!(
                        "no such session: {}",
                        truncate_for_error(&session_id)
                    ),
                    kind: ErrorKind::NotFound,
                },
            )
            .await;
            return;
        };

        // In-flight uploads are cancelled FIRST — before the
        // process sweep, not after it. The sweep is where a delete
        // spends its seconds (a grace period plus several /proc
        // walks), and a transfer left running through it goes on
        // writing into the directory this delete is about to take
        // away, for as long as the sweep lasts. Cancelling first
        // costs nothing (the transfer is doomed either way) and
        // bounds those writes to the instant it takes each task to
        // notice.
        //
        // `abort_session_uploads` also WAITS for each task to
        // finish cleaning up, so from here on nothing can write
        // into (or publish into) that directory, and the lifecycle
        // claim this handler has held since its first line keeps a
        // new transfer from staging into it (see `stage_upload`).
        abort_session_uploads(&sup, &session_id, "the session was deleted").await;

        // The process-tree sweep runs BEFORE any lock is held: it can
        // take seconds (a grace period plus several /proc walks), and
        // holding `attachments` for that long would stall every OTHER
        // session's attach/input behind one slow delete — the map-
        // wide mutex's already-documented coarseness (see the
        // `Supervisor` struct's lock-discipline docs) made worse if a
        // multi-second sweep sat inside it. A concurrent Attach can
        // therefore install a fresh attachment WHILE this runs; the
        // lock-held phase below tears down WHATEVER attachment exists
        // by the time it runs, new or old, and gives it the deleted
        // notice — that is the one acceptable consequence of not
        // holding the lock here, not an oversight.
        //
        // Same dead/absent/terminal-less handling as `StopSession`:
        // the marker sweep still runs even with no live pane pid, for
        // the same leftover-reaping reason documented there.
        let root_pid = match entry.terminal.as_ref() {
            Some(terminal) => match sup
                .tmux
                .pane_process(&terminal.tmux_name, &terminal.pane)
                .await
            {
                Ok(Some(pane)) if !pane.dead => Some(pane.pid),
                Ok(_) => None,
                Err(e) => {
                    send_reply(
                        &tx,
                        &ControlMsg::Error {
                            req_id,
                            message: format!("querying pane process: {e:#}"),
                            kind: ErrorKind::Internal,
                        },
                    )
                    .await;
                    return;
                }
            },
            None => None,
        };
        // `WholeSession`: delete is the one lifecycle operation
        // that takes tabs down with the agent (SPEC.md — stop
        // leaves them running, delete and archive do not), so this
        // sweep deliberately does NOT subtract tab processes. It
        // needs no per-tab PPID root either: a tab's shell carries
        // the session marker like everything else the session
        // launched, and the marker scan finds it wherever it is.
        // (Rediscovery below only needs the tab's WINDOW, which
        // survives whether or not its shell already exited —
        // `remain-on-exit` keeps a dead pane's window listed — so
        // "the tmux teardown has not run yet" is what matters
        // here, not that any pane is still alive.)
        //
        // Every tab's SCOPE, however, does have to be named: a
        // cgroup kill can only reach what its own `systemd-run`
        // placed there, so the agent's unit alone would leave a
        // tab's environment-scrubbing double-fork behind — the one
        // shape the marker sweep provably cannot find.
        //
        // Named from TWO independent sources, and the second is
        // the load-bearing one. Rediscovering tabs from tmux
        // covers the ordinary case, but the case that matters here
        // is a tmux server that died BEFORE the delete: there are
        // no windows left to read tab ids from, while a scrubbed
        // tab daemon is still running inside a cgroup that
        // outlived its pane. So the manager is also asked directly
        // for every unit matching this session's tab glob, which
        // needs no tmux at all. A failure to ENUMERATE fails the
        // delete outright, row retained: publishing "deleted" over
        // an unenumerated cgroup is exactly the unreapable,
        // invisible agent lore/2026-07-27-m2-process-tree-stop.md
        // ends on.
        let mut units = entry.scope.clone().into_iter().collect::<Vec<_>>();
        if let Some(terminal) = entry.terminal.as_ref() {
            match sup.session_tabs(terminal).await {
                Ok(tabs) => units.extend(
                    tabs.iter()
                        .filter_map(|tab| {
                            crate::scope::tab_unit_name(&session_id, &tab.id)
                        }),
                ),
                // Strict: "we could not ask tmux" is not "there
                // are no tabs", and a delete that assumed the
                // latter would skip live tab scopes.
                Err(e) => {
                    send_reply(
                        &tx,
                        &ControlMsg::Error {
                            req_id,
                            message: format!(
                                "could not determine this session's terminal tabs, so                                          nothing was deleted: {e:#}"
                            ),
                            kind: ErrorKind::Internal,
                        },
                    )
                    .await;
                    return;
                }
            }
        }
        if let Some(glob) = crate::scope::tab_unit_glob(&session_id) {
            match sup.seams.scopes.units_matching(&glob).await {
                Ok(found) => units.extend(found),
                // Only a host with a usable manager can be asked at
                // all; where there is none this is not a failure,
                // it is the sweep-only world M2 already lived in.
                Err(e) if !sup.seams.scopes.available().await => debug!(
                    session = %session_id, error = %format!("{e:#}"),
                    "no systemd user manager to enumerate this session's tab scopes;                              the process-tree sweep is the whole mechanism"
                ),
                Err(e) => {
                    send_reply(
                        &tx,
                        &ControlMsg::Error {
                            req_id,
                            message: format!(
                                "this host has a systemd user manager but its terminal-\
                                 tab scopes could not be enumerated, so nothing was \
                                 deleted: {e:#}"
                            ),
                            kind: ErrorKind::Internal,
                        },
                    )
                    .await;
                    return;
                }
            }
        }
        units.sort();
        units.dedup();
        if let Err(e) = reap_process_tree(
            &sup.seams.scopes,
            &units,
            root_pid,
            &session_id,
            &SweepTarget::WholeSession,
        )
        .await
        {
            send_reply(
                &tx,
                &ControlMsg::Error {
                    req_id,
                    message: format!("killing process tree: {e:#}"),
                    kind: ErrorKind::Internal,
                },
            )
            .await;
            return;
        }

        // Everything from here on is fast (one tmux round trip, a
        // few fail-closed removals, one sqlite
        // write) and runs under `attachments`, mirroring the Attach
        // handler's takeover for the same reason: a concurrent Attach
        // must not be able to install itself mid-teardown. This is
        // also the one path that acquires BOTH locks at once — `map
        // removal` below briefly takes `sessions` too, while still
        // holding `attachments` — which is the ordering rule this
        // establishes and the only one that needs to exist as long as
        // nothing else ever needs both: `attachments` first,
        // `sessions` second.
        let mut attachments = sup.attachments.lock().await;
        // EVERY terminal of the session, not just the agent's: a
        // delete takes the whole session down, so every channel it
        // has attached is about to be streaming something that no
        // longer exists (PLAN_M4.md item 3's session-scoped
        // ownership, on the teardown side). Restart is the
        // deliberate contrast — see `detach_for_restart`.
        //
        // Abort the forwarders now, before they can race their own
        // natural "session terminal ended" Detached against whatever
        // truthful notice this handler sends once the real outcome
        // below is known — but do not send that notice yet.
        //
        // ALL of them are aborted before ANY is awaited, exactly as
        // the attach takeover does it: the awaits are sequential, so
        // aborting inside the same loop would leave the later
        // forwarders streaming (and able to emit their own detach)
        // while the earlier ones are already gone.
        let doomed: Vec<ActiveAttach> = attachments
            .extract_if(|key, _| key.session == session_id)
            .map(|(_, attachment)| attachment)
            .collect();
        for old in &doomed {
            old.forwarder.abort();
        }
        let mut notify_detach = Vec::with_capacity(doomed.len());
        // `..` drops each attachment's input client — killing its
        // control-mode process via `kill_on_drop`, like every other
        // teardown path — and its pause sender, which the aborted
        // forwarder can no longer observe anyway.
        for ActiveAttach {
            channel,
            notify,
            forwarder,
            ..
        } in doomed
        {
            let _ = forwarder.await;
            notify_detach.push((channel, notify));
        }

        // Fail-closed and sequenced deliberately: artifacts before the
        // DB row (a leftover launch spec may hold credentials, and
        // this is the last moment anything will ever come back to
        // remove it — see `remove_fail_closed`'s docs), and the row
        // only after the terminal and process tree are positively
        // gone (a crash here leaves a listed-but-dead session,
        // recoverable by the next delete or a manual cleanup, rather
        // than an unlisted-but-running agent, invisible and
        // unreapable — see lore/2026-07-27-m2-process-tree-stop.md's
        // final paragraph). One `Result`-returning block with `?`
        // rather than a hand-threaded `teardown_error` variable, now
        // that none of these steps need to happen outside the lock.
        // Where this session's attachments were parked, if it had
        // any: filled in by the block below and discarded only
        // once the row removal has committed (see the
        // quarantining step's own comment).
        let mut quarantined: Option<PathBuf> = None;
        let teardown: Result<(), String> = async {
            if let Some(terminal) = entry.terminal.as_ref() {
                sup.tmux
                    .kill_session(&terminal.tmux_name)
                    .await
                    .map_err(|e| format!("killing tmux session: {e:#}"))?;
            }
            // EVERY generation's launch files, not just the current
            // one: they are named per launch now
            // (`launch::spec_path_for_launch`), and a session that
            // was restarted has one pair per launch it ever had.
            // Delete is the last moment anything comes back for
            // them, and a spec holds the agent's full command line
            // — credentials included — so a missed generation is a
            // credential leak, not untidiness. Fail-closed for that
            // reason (`remove_fail_closed`), including the failure
            // to LIST them: an unreadable directory is not evidence
            // there was nothing in it.
            remove_launch_artifacts_for_session(&sup.state_dir, &session_id).await?;
            // Same fail-closed treatment as the launch artifacts
            // above and for the same reason: the snapshot can hold
            // secrets an agent echoed to an alt-screen app, and
            // delete is the last moment anything will ever come
            // back to remove it.
            remove_fail_closed(
                &snapshot_path(&sup.state_dir, &session_id),
                "alt-screen snapshot",
            )
            .await?;
            // SPEC.md: "attachment files are removed when their
            // session is deleted". DETACHED here (an atomic
            // rename into the reserved quarantine directory) and
            // actually removed after the row is gone, which is
            // what makes the crash window safe in the right
            // direction: this step still fails the delete closed
            // — with the row retained for a retry — while a crash
            // between it and the commit leaves debris the next
            // startup reconciles rather than a live session whose
            // attachments have silently vanished.
            //
            // Nothing recreates the directory afterwards: every
            // in-flight transfer was cancelled and joined above,
            // and a new one cannot start while this handler holds
            // the session's lifecycle claim.
            quarantined =
                crate::attachments::quarantine_session_dir(&sup.state_dir, &session_id)
                    .await?;
            // Settles this session's create reservations in the
            // same transaction as the row removal, which is what
            // turns them into TOMBSTONES rather than stale claims:
            // a replay of one of those intent keys must report the
            // gone-error, never a dead id and never a fresh
            // duplicate (PLAN_M3.md item 6; the store method's own
            // docs carry the argument).
            sup.store
                .delete_session_settling_reservations(&session_id)
                .await
                .map_err(|e| format!("{e:#}"))
        }
        .await;

        if let Err(err_msg) = teardown {
            for (channel, notify) in &notify_detach {
                notify_detached(
                    notify,
                    *channel,
                    format!("detached during a failed delete: {err_msg}"),
                );
            }
            drop(attachments);
            send_reply(
                &tx,
                &ControlMsg::Error {
                    req_id,
                    message: err_msg,
                    kind: ErrorKind::Internal,
                },
            )
            .await;
            return;
        }
        sup.sessions.lock().await.remove(&session_id);
        // The row is gone, so the quarantined attachments now
        // belong to nothing and can be removed for real. Failure
        // here is logged rather than reported: there is no row
        // left to retain for a retry, the caller's delete
        // genuinely succeeded, and the next startup reconciles
        // whatever is left.
        if let Some(parked) = quarantined {
            crate::attachments::discard_quarantined(&parked).await;
        }

        for (channel, notify) in &notify_detach {
            notify_detached(notify, *channel, "session deleted".to_string());
        }
        drop(attachments);
        send_reply(&tx, &ControlMsg::SessionDeleted { req_id }).await;
    })
    .await;
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
    let mut attachments = sup.attachments.lock().await;

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
    // generation). So a replacement is accepted when it still
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
    let displaced: Vec<ActiveAttach> = attachments
        .extract_if(|k, a| displaced_by_attach(k, &a.lease, &session_id, &lease))
        .map(|(_, attachment)| attachment)
        .collect();
    // Same terminal, same lease: an ordinary reconnect, and the
    // one-attachment-per-terminal enforcement point. It cannot
    // overlap the sweep above — a same-lease incumbent is never
    // swept — so removing it separately is not a second takeover,
    // just the cutover that has always happened here. Which is
    // exactly why it is told a DIFFERENT reason: see
    // `DETACH_REASON_REPLACED`.
    let incumbent = attachments.remove(&key);

    // Abort every doomed forwarder BEFORE awaiting any of them:
    // the awaits are sequential, so aborting inside the same loop
    // would leave the not-yet-aborted forwarders free to emit
    // output — and their own end-of-stream `Detached` — while the
    // earlier ones are already being torn down.
    for old in displaced.iter().chain(incumbent.iter()) {
        old.forwarder.abort();
    }
    let mut notices = Vec::with_capacity(displaced.len() + usize::from(incumbent.is_some()));
    for (old, reason) in displaced
        .into_iter()
        .map(|old| (old, DETACH_REASON_TAKEOVER))
        .chain(
            incumbent
                .into_iter()
                .map(|old| (old, DETACH_REASON_REPLACED)),
        )
    {
        // `..` drops this attachment's input client (killing its
        // control-mode process via `kill_on_drop`) and its pause
        // sender, which is what every teardown path does.
        let ActiveAttach {
            channel,
            notify,
            forwarder,
            ..
        } = old;
        // Awaiting the abort is what actually makes the old
        // control-mode client gone: dropping its OutputStream
        // (and so killing the process) happens when the task is
        // polled after cancellation, not when abort() returns.
        // The forwarder never takes this lock, so awaiting it
        // here cannot deadlock.
        let _ = forwarder.await;
        notices.push((channel, notify, reason));
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
    let (modes, prefill, stream) = match sup
        .tmux
        .open_replay_stream(&terminal.tmux_name, &terminal.pane)
        .await
    {
        Ok(parts) => parts,
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
            drop(attachments);
            stream.shutdown().await;
            permit.send(reply_frame(&ControlMsg::Error {
                req_id,
                message: format!("{e:#}"),
                kind: error_kind(&e),
            }));
            return;
        }
    };

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
    let forwarder = Forwarder {
        sup: Arc::clone(sup),
        session_id: session_id.clone(),
        terminal: key.terminal.clone(),
        channel,
        tx: tx.clone(),
        stream,
        pause_rx,
        stall_timeout: sup.timeouts.stall_detach,
    };
    let task = tokio::spawn(forwarder.run(modes, prefill));

    attachments.insert(
        key.clone(),
        ActiveAttach {
            channel,
            lease,
            notify: tx.clone(),
            forwarder: task,
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
        // Abort AND await, mirroring the takeover path: detach
        // followed by an immediate reattach (a browser reload is
        // exactly this) finds no incumbent to kick, so the only
        // thing keeping the old control-mode client from
        // overlapping the new one — the documented frozen-replay
        // hazard — is waiting for it here, before the lock is
        // released. Awaiting cannot deadlock: forwarders never
        // take this lock.
        a.forwarder.abort();
        let _ = a.forwarder.await;
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

/// Dispatch one control message from a connected client.
///
/// Failures belonging to one request—bad cwd, a tmux hiccup, an unknown
/// session—become `ControlMsg::Error` replies here. They must not escape
/// into the connection loop: one connection carries every session the
/// helm is driving, so request-local failure cannot be allowed to detach
/// unrelated terminals.
///
/// `tx` doubles as this connection's identity: `same_channel` against it
/// is how the handlers tell "the connection that owns this attachment"
/// from any other, which channel ids alone cannot do.
pub(crate) async fn handle_control(
    sup: &Arc<Supervisor>,
    msg: ControlMsg,
    tx: &mpsc::Sender<Frame>,
    // This connection's prioritized queue, for the upload family alone —
    // see `send_upload`.
    priority: &mpsc::Sender<Frame>,
    input_routes: &mut HashMap<u32, InputRoute>,
    upload_routes: &mut HashMap<u32, UploadRoute>,
    tasks: &mut tokio::task::JoinSet<()>,
) {
    match msg {
        ControlMsg::CreateSession {
            req_id,
            cwd,
            invocation,
            title,
            cols,
            rows,
            intent_key,
            agent_kind,
            resume_template,
        } => {
            handle_create_session(
                sup,
                tx,
                req_id,
                cwd,
                invocation,
                title,
                cols,
                rows,
                intent_key,
                agent_kind,
                resume_template,
            )
            .await
        }
        ControlMsg::ListSessions {
            req_id,
            cursor,
            limit,
        } => handle_list_sessions(sup, tx, tasks, req_id, cursor, limit).await,
        ControlMsg::StopSession { req_id, session_id } => {
            handle_stop_session(sup, tx, tasks, req_id, session_id).await
        }
        ControlMsg::DeleteSession { req_id, session_id } => {
            handle_delete_session(sup, tx, tasks, req_id, session_id).await
        }
        ControlMsg::Attach {
            req_id,
            session_id,
            channel,
            cols,
            rows,
            terminal: selector,
            lease,
        } => {
            handle_attach(
                sup,
                tx,
                input_routes,
                upload_routes,
                req_id,
                session_id,
                channel,
                cols,
                rows,
                selector,
                lease,
            )
            .await
        }
        ControlMsg::PauseOutput { channel } => {
            set_attachment_paused(sup, tx, channel, true).await;
        }
        ControlMsg::ResumeOutput { channel } => {
            set_attachment_paused(sup, tx, channel, false).await;
        }
        ControlMsg::Detach { channel } => handle_detach(sup, tx, input_routes, channel).await,
        ControlMsg::Resize {
            session_id,
            channel,
            cols,
            rows,
        } => handle_resize(sup, tx, input_routes, session_id, channel, cols, rows).await,
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
            handle_restart_session(sup, tx, tasks, req_id, session_id, mode, stop_if_running).await
        }
        ControlMsg::RenameSession {
            req_id,
            session_id,
            title,
        } => handle_rename_session(sup, tx, tasks, req_id, session_id, title).await,
        ControlMsg::OpenTab { req_id, session_id } => {
            handle_open_tab(sup, tx, tasks, req_id, session_id).await
        }
        ControlMsg::CloseTab {
            req_id,
            session_id,
            tab_id,
        } => handle_close_tab(sup, tx, tasks, req_id, session_id, tab_id).await,
        ControlMsg::BeginUpload {
            req_id,
            session_id,
            channel,
            filename,
            size,
        } => {
            handle_begin_upload(
                sup,
                tx,
                priority,
                input_routes,
                upload_routes,
                req_id,
                session_id,
                channel,
                filename,
                size,
            )
            .await
        }
        ControlMsg::CommitUpload { req_id, channel } => {
            handle_commit_upload(tx, upload_routes, req_id, channel).await
        }
        ControlMsg::AbortUpload { channel } => handle_abort_upload(upload_routes, channel),
        // Response/event messages arriving at the supervisor are peer
        // bugs; log and continue.
        other => warn!(?other, "unexpected control message at supervisor"),
    }
}

#[cfg(test)]
mod tests {
    use super::super::connection::CONNECTION_WRITER_QUEUE;
    use super::super::core::tests::{StateDir, dummy_exe, no_uploads};
    use super::super::core::{CaptureState, FirstInput, encode_list_cursor};
    use super::super::terminals::Terminal;
    use super::*;
    use crate::agent_kind::IntegrationSnapshot;
    use farhelm_proto::{RestartOffer, SessionStatus};
    use std::time::Duration;

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
            &tx,
            &tx,
            &mut input_routes,
            &mut no_uploads(),
            &mut tasks,
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
                cwd: "x".repeat(CREATE_FIELD_CAP),
                invocation: "agent".to_string(),
                title: None,
                cols: 80,
                rows: 24,
                intent_key: None,
                agent_kind: None,
                resume_template: None,
            },
            &tx,
            &tx,
            &mut input_routes,
            &mut no_uploads(),
            &mut tasks,
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
                    cwd: "/".to_string(),
                    invocation: "agent".to_string(),
                    title: None,
                    cols: 80,
                    rows: 24,
                    intent_key: Some(key),
                    agent_kind: None,
                    resume_template: None,
                },
                &tx,
                &tx,
                &mut input_routes,
                &mut no_uploads(),
                &mut tasks,
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
                cwd: "/nonexistent/definitely/not/here".to_string(),
                invocation: "agent".to_string(),
                title: None,
                cols: 80,
                rows: 24,
                intent_key: Some("k".repeat(INTENT_KEY_CAP)),
                agent_kind: None,
                resume_template: None,
            },
            &tx,
            &tx,
            &mut input_routes,
            &mut no_uploads(),
            &mut tasks,
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
                    cwd: "/".to_string(),
                    invocation: "agent".to_string(),
                    title: None,
                    cols: 80,
                    rows: 24,
                    intent_key: Some("key".to_string()),
                    agent_kind: None,
                    resume_template: Some(template),
                },
                &tx,
                &tx,
                &mut input_routes,
                &mut no_uploads(),
                &mut tasks,
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
    /// `build_list_reply`'s own unit tests in `core` pin that outcome in
    /// isolation; this test pins it through the REAL call site, so a
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
                    id: "s1".to_string(),
                    title: "x".repeat(farhelm_proto::MAX_FRAME_LEN as usize),
                    created_at: 1_700_000_000,
                    cwd: "/tmp".to_string(),
                    invocation: "agent".to_string(),
                    status: SessionStatus::default(),
                    annotation: None,
                    restart_offer: RestartOffer::default(),
                    tabs: Vec::new(),
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
            &tx,
            &tx,
            &mut input_routes,
            &mut no_uploads(),
            &mut tasks,
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
            &tx,
            &tx,
            &mut input_routes,
            &mut no_uploads(),
            &mut tasks,
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
            &tx,
            &tx,
            &mut input_routes,
            &mut no_uploads(),
            &mut tasks,
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
                &tx,
                &tx,
                &mut input_routes,
                &mut no_uploads(),
                &mut tasks,
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
                id: id.to_string(),
                title,
                created_at,
                cwd: "/tmp".to_string(),
                invocation: "agent".to_string(),
                status: SessionStatus::default(),
                annotation: None,
                restart_offer: RestartOffer::default(),
                tabs: Vec::new(),
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
            &tx,
            &tx,
            &mut input_routes,
            &mut no_uploads(),
            &mut tasks,
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
    /// than `build_list_reply` in isolation (`core`'s own tests cover the
    /// pure function): fat titles force a genuine mid-page cut, and the
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
                    id: "s1".to_string(),
                    title: "t".to_string(),
                    created_at: 1_700_000_000,
                    cwd: "/tmp".to_string(),
                    invocation: "agent".to_string(),
                    status: SessionStatus::default(),
                    annotation: None,
                    restart_offer: RestartOffer::default(),
                    tabs: Vec::new(),
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
            &tx,
            &tx,
            &mut input_routes,
            &mut no_uploads(),
            &mut tasks,
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
