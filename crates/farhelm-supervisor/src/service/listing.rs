//! `ListSessions`'s page walk: the total order, the cursor that names a
//! position in it, the two independent page cuts, and the reply they
//! produce.
//!
//! One module because it is one contract. A page is defined by three
//! things that only mean anything together — the order sessions are
//! walked in ([`list_order_key`]), the opaque token that names a position
//! in that order ([`ListCursor`] and its codec), and the two cuts that
//! decide where a page ends (the count/cursor cut [`list_page`] applies,
//! and [`build_list_reply`]'s byte budget). All three have to agree about
//! what "strictly after the last entry actually returned" means. Split
//! across modules, one of them could drift from the others without
//! anything failing loudly: the symptom of a disagreement is a caller
//! that pages forever making no progress, or one that silently skips
//! sessions — never a compile error.
//!
//! [`list_page`] is the seam `handle_list_sessions` sits behind. It takes
//! an ALREADY-VALIDATED query (raw wire values — a zero limit, an
//! undecodable cursor — are the handler's to refuse before any list work
//! is admitted) and answers with the page, leaving the handler only the
//! mapping of that answer onto reply frames. Nothing in this module knows
//! about the connection: no reply channel, no transport, no `req_id`
//! except the one `build_list_reply` must echo. That is what makes the
//! walk reasoned about — and tested — without a socket anywhere in sight.

use super::core::{SessionEntry, Supervisor};
use super::launch_artifacts::cleanup_launch_artifacts;
use super::status::{entry_info, observe_entry};
use crate::store::{LastOutcome, ProfileNames, Transition};
use anyhow::Context;
use farhelm_proto::{ControlMsg, Frame, SessionInfo};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::warn;

/// `ListSessions`'s count cap (PLAN_M2.md's "Proto growth"). ~500 keeps a
/// single reply's session count bounded before the byte budget below ever
/// has to do the harder job of bounding fat, variable-length records.
pub const LIST_SESSION_CAP: usize = 500;

/// `ListSessions`'s encoded-size budget, independent of the count cap: a
/// count alone cannot bound encoded bytes when each session's title, cwd,
/// and invocation are caller-controlled strings of unbounded length — 500
/// sessions with fat titles can still blow past `MAX_FRAME_LEN` on their
/// own. Deliberately well under `MAX_FRAME_LEN` (half of it) rather than
/// flush against it: `Frame::encoded_len` (what this budget is compared
/// against, in `build_list_reply`) already accounts for the frame's own
/// envelope — the header and the `SessionList` object's fixed fields —
/// which is a few dozen bytes, negligible next to a multi-megabyte cap.
/// The margin is headroom for a future additive `SessionList`/
/// `SessionInfo` field instead: a number tuned flush against today's
/// fields would need re-tuning the moment PLAN_M2.md adds another one.
/// `reply_frame`'s oversize defusal stays as the last-resort backstop
/// regardless — this budget is meant to make that backstop unreachable in
/// practice, not to replace it.
pub(crate) const LIST_BYTE_BUDGET: usize = (farhelm_proto::MAX_FRAME_LEN / 2) as usize;

/// Total order `ListSessions` pages walk (PLAN_M6.md's "Pagination
/// shape"): creation time descending, with session id ASCENDING as the
/// tiebreak. Both halves are stable for a session's whole life — `id`
/// never changes, and `created_at` is written once at insert
/// (`StoredSession::created_at`'s own docs) — which is what lets a cursor
/// resume a walk unaffected by concurrent creates or deletes: the key a
/// cursor encodes still names the same place in the same order no matter
/// what else in the session set changed since it was issued.
///
/// The tiebreak DIRECTION is pinned by the protocol contract, not a free
/// choice of this build: `ControlMsg::SessionList`'s own docs
/// (`farhelm-proto`) specify "session id ascending as the tiebreak", so
/// this function's ordering must match that wording exactly, not merely
/// produce SOME fixed relative order for same-second creations.
///
/// `std::cmp::Reverse` on `created_at` is what turns "descending time,
/// ascending id" into a single ordinary ascending comparison other code
/// can sort and binary-search by directly, rather than writing a
/// hand-rolled `Ordering::then_with` at every call site.
fn list_order_key(info: &SessionInfo) -> (std::cmp::Reverse<i64>, &str) {
    (std::cmp::Reverse(info.created_at), info.id.as_str())
}

/// A `ListSessions` page cursor's decoded contents: the ordering key
/// (`list_order_key`'s own shape, owned rather than borrowed since a
/// decoded cursor outlives the request that carried it) of the last
/// session a page actually returned. Resuming means "strictly after this
/// key" (see `SessionList::next_cursor`'s own docs) — not "starting from
/// this row" — which is what lets a cursor naming a since-deleted
/// session's key still resume cleanly: nothing about decoding or
/// resuming ever needs the named session to still exist.
// Only `Serialize` (`encode_list_cursor`) and `Deserialize`
// (`decode_list_cursor`) are exercised anywhere in this crate: every
// caller reads `created_at`/`id` off a decoded value directly rather than
// comparing, cloning, or printing the struct itself, so `Debug`, `Clone`,
// `PartialEq`, and `Eq` would be dead derives.
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct ListCursor {
    created_at: i64,
    id: String,
}

/// Generous upper bound on one encoded `next_cursor`'s length, reserved
/// unconditionally in `build_list_reply`'s byte-budget accounting — see
/// that function's own docs for why a flat reserve, rather than an
/// encode-then-recheck loop, is this build's chosen strategy. Every
/// session id `ListSessions` can ever report is supervisor-minted via
/// `uuid::Uuid::new_v4().to_string()` (every id-minting call site in this
/// crate follows that same convention), a fixed 36 ASCII characters, so
/// the raw `{"created_at":<i64>,"id":"<uuid>"}` JSON `encode_list_cursor`
/// produces never exceeds roughly 80 bytes even at `i64::MIN`; base64
/// inflates that by 4/3. This constant is set well past that real
/// ceiling, so the reserve stays correct even if a future id-minting site
/// used a slightly longer format.
const LIST_CURSOR_RESERVE: usize = 200;

/// Encode a page's last-returned entry into the opaque string
/// `SessionList::next_cursor` carries — base64 of a compact JSON
/// serialization, chosen (over, say, a raw `created_at:id` join) because
/// it is self-describing at decode time: `decode_list_cursor` can reject
/// a malformed value by construction (JSON parse failure) rather than by
/// hand-rolled delimiter scanning that would have to guess whether a
/// stray `:` came from a corrupted id or a value built by hand. URL-safe,
/// unpadded base64 keeps the result a plain opaque token with no
/// characters a caller might feel tempted to interpret — opaque as a
/// USAGE convention (store and replay verbatim), not as an authority
/// boundary: this cursor is an ordering key, nothing more, and carries no
/// claim about who may present it (`decode_list_cursor`'s own docs cover
/// what that means for a hand-built one).
pub(crate) fn encode_list_cursor(created_at: i64, id: &str) -> String {
    use base64::Engine;
    let cursor = ListCursor {
        created_at,
        id: id.to_string(),
    };
    // Unwrap is safe: `ListCursor` has no map keys or non-UTF-8 bytes for
    // JSON serialization to ever fail on.
    let json = serde_json::to_vec(&cursor).expect("ListCursor is always serializable");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json)
}

/// Decode a caller-supplied cursor into its ordering key, or `None` for
/// anything that fails to decode cleanly — invalid base64, JSON that will
/// not parse, or JSON of the wrong shape. Every failure mode collapses to
/// this SAME `None`, deliberately not distinguishing which way decoding
/// failed: `handle_list_sessions` turns any `None` into one
/// `ErrorKind::InvalidRequest`, and a bit-flipped byte, a truncated
/// value, and a string from nowhere are indistinguishable to an honest
/// server — there is no differently-actionable response for a caller to
/// receive for any of them, so there is nothing this function should try
/// to tell them apart for. Never panics on caller input: `?` short-
/// circuits through `Option`, not `unwrap`, at every fallible step.
///
/// What this function is NOT is an authority check: a `ListCursor` that
/// decodes cleanly is accepted regardless of whether THIS supervisor ever
/// encoded it — a hand-built or mutated-but-well-formed key is a valid
/// resume position like any other, per `SessionList::next_cursor`'s own
/// docs. That is deliberate, not an oversight this function should start
/// closing: every `ListSessions` caller may already read every session (a
/// single-user supervisor), so a forged key resumes at a position honest
/// paging would reach anyway, and strictly-after resumption is what lets
/// a cursor naming a since-deleted session still resume cleanly
/// (`list_sessions_cursor_from_a_deleted_session_still_resumes` pins that
/// as the feature it is). "Refuse the undecodable, trust every decodable
/// key" is the whole contract — there is no third case to add.
pub(crate) fn decode_list_cursor(cursor: &str) -> Option<ListCursor> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(cursor)
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Byte-budget half of `ListSessions`'s two independent page cuts. The
/// count/cursor cut is the CALLER's job, applied before this is ever
/// reached — [`list_page`] walks [`list_order_key`]'s order and slices out
/// at most one page's worth of entries before cloning or status-
/// annotating a single one of them (see that function's own comment on the
/// snapshot for why paying that cost for entries this function would only
/// drop anyway is wasteful to avoid in the first place). Because of that,
/// this function cannot reconstruct the true pre-cut session count from
/// `sessions.len()` — `total` is supplied by the caller instead, and is
/// reported as-is.
///
/// `sessions` arrives ALREADY in page order (`list_order_key`'s walk), so
/// truncation here drops only from the tail of that order — never
/// reorders, never drops from the middle — which is what makes "the last
/// entry actually kept" a meaningful resume point at all.
///
/// `page_continues_beyond_caller_cut` is whether the caller's own count/
/// cursor cut already left sessions unreturned beyond what it handed to
/// this function (i.e. `sessions.len()` is a page, not the full remaining
/// walk). This function OR's that against whatever ITS OWN byte-budget cut
/// does: `next_cursor` is `Some` (encoding the last entry actually kept)
/// whenever either cut fired with more sessions left beyond the return,
/// `None` only when the walk that produced `sessions` — after BOTH cuts —
/// genuinely reached the end of the order. See `SessionList::next_cursor`'s
/// own docs for the one case this function refuses outright rather than
/// answering `None`: a single session too large to fit under `byte_budget`
/// even alone leaves `kept` empty with no last entry to build a cursor
/// from. Nothing bounds a record's size below the budget (a title is
/// caller-controlled and unbounded), so this IS reachable, not merely
/// theoretical — and `next_cursor: None` here would silently claim the
/// walk was exhausted while sessions still remain, unreachable behind a
/// cut this function can never resume past. So this returns `Err`
/// instead, naming the session that could not be represented — but only
/// once the EXACT reply that single candidate would produce has also been
/// checked and still does not fit (see the next paragraph): the reserve-
/// padded first pass can refuse a candidate the real wire size would have
/// accepted, and that false refusal is not the "no honest reply exists"
/// case this paragraph is about. Turning a genuine refusal into a wire
/// reply is left to the caller (`handle_list_sessions` maps it to
/// `ErrorKind::Internal`, matching `reply_frame`'s own precedent for "the
/// honest answer does not fit on the wire").
///
/// Single-pass, exact size accounting, and the final reply is constructed
/// exactly ONCE per candidate page — a previous version re-encoded a
/// shrinking candidate on every dropped entry, which is quadratic in the
/// number of entries eventually dropped. Instead: `envelope_len` is the
/// encoded size of this SAME reply shape with an empty `sessions` array
/// and no cursor, measured once via the real `Frame`/`ControlMsg` path
/// (never hand-computed, so it can't drift from what `Frame::control`
/// actually produces), PLUS [`LIST_CURSOR_RESERVE`] — a flat worst-case
/// reserve for whatever `next_cursor` this call ends up emitting. That
/// reserve is this function's chosen answer to the accounting problem a
/// `Some` cursor creates: unlike the pre-8 `next_cursor: None` constant, a
/// REAL cursor's encoded size varies with its ordering key, so the scan
/// below needs to budget for it before it even knows whether one will be
/// emitted at all. The alternative — encode the real reply on every
/// iteration, and if it comes out oversized, drop the last-kept entry and
/// retry — was considered and rejected for the ORDINARY (non-empty `kept`)
/// case: it would still need a bound on the retry count and adds real
/// branching for a saving ([`LIST_CURSOR_RESERVE`]'s 200 bytes) that is
/// noise next to `LIST_BYTE_BUDGET`'s multi-megabyte scale. Each candidate
/// entry is serialized exactly once (`serde_json::to_vec`) and its EXACT
/// marginal contribution to the `sessions` JSON array — its own bytes,
/// plus one comma separator once it is not the first surviving entry — is
/// added to a running total seeded from `envelope_len`. An entry that
/// would push the running total over `byte_budget` stops the scan;
/// everything kept up to that point is the candidate answer, and
/// `next_cursor` is then decided from it.
///
/// The one place this function DOES pay for a second, exact encoding is
/// deliberately narrow: when the reserve-padded check rejects the FIRST
/// candidate examined (`kept` still empty), rather than accept the
/// reserve's pessimism as final, this recheck builds the exact reply that
/// one candidate would produce — `next_cursor: None` if nothing remains
/// beyond it (`page_continues_beyond_caller_cut` is false and the scan's
/// iterator has nothing left), or its real encoded cursor if something
/// does — and measures ITS true size instead of the worst-case reserve.
/// Two shapes this closes: a final page's last entry that fits the raw
/// budget cursorless but not budget-minus-reserve, and a continuing
/// single entry whose real cursor (almost always well under
/// [`LIST_CURSOR_RESERVE`]'s 200-byte ceiling) fits where the reserve
/// would not. Bounded to ONE extra encode, exactly when `kept` is empty —
/// never on a later, ordinary tail cut, which is what keeps the "single-
/// pass in the common case" property above intact rather than reopening
/// the quadratic-retry shape this function's docs already rejected once.
///
/// A `debug_assert!` re-encodes the actual returned reply as a sanity
/// check that the accounting above never drifted from reality — reachable
/// only via the ordinary (non-empty `kept`) return path, since the empty-
/// page recheck above already measures its own candidate's true size
/// directly, by construction, before ever returning it. Deliberately not
/// a release-mode check: `reply_frame`'s `MAX_FRAME_LEN` defusal remains
/// the real last-resort backstop in production; this assert exists only
/// to catch an accounting bug in tests/debug builds before it could ever
/// reach that backstop. `byte_budget.max(envelope_len)` tolerates the
/// degenerate case of a budget smaller than the envelope itself (only
/// reachable with a pathologically tiny `byte_budget`, never
/// `LIST_BYTE_BUDGET` in production) — this function must still return
/// SOMETHING even then, and the assert should not fire over a caller
/// having chosen an unreasonable budget.
///
/// Returns `Err` instead of a reply exactly when the scan kept nothing,
/// `sessions` was non-empty, AND the empty-page recheck above also
/// rejected the exact reply its sole candidate would produce: the first
/// (and, since the scan stops at the first entry it cannot afford, only)
/// candidate examined does not fit even alone, at its true encoded size.
/// There is no honest `SessionList` to build in that case — see the
/// paragraph above — so the caller gets the rejected session's id back
/// instead, to turn into whatever error reply its own transport
/// conventions use.
pub(crate) fn build_list_reply(
    req_id: u64,
    sessions: Vec<SessionInfo>,
    total: u64,
    byte_budget: usize,
    page_continues_beyond_caller_cut: bool,
) -> Result<ControlMsg, String> {
    let candidate_len = sessions.len();
    let envelope_len = Frame::control(&ControlMsg::SessionList {
        req_id,
        sessions: Vec::new(),
        total,
        next_cursor: None,
    })
    .encoded_len()
        + LIST_CURSOR_RESERVE;

    let mut kept: Vec<SessionInfo> = Vec::with_capacity(sessions.len());
    let mut used = envelope_len;
    // Peekable so the empty-page recheck below can tell "this candidate
    // is the walk's last entry" (nothing left to peek, and no caller-side
    // cut beyond it either) from "something else is still queued behind
    // it" without consuming an extra entry to find out.
    let mut sessions = sessions.into_iter().peekable();
    while let Some(session) = sessions.next() {
        let separator = if kept.is_empty() { 0 } else { 1 };
        let entry_len = serde_json::to_vec(&session)
            .expect("SessionInfo is always serializable")
            .len()
            + separator;
        if used + entry_len > byte_budget {
            if kept.is_empty() {
                // The empty-page recheck (this function's own docs cover
                // why it is scoped to exactly this case): the reserve-
                // padded first pass rejected this candidate, but that
                // reserve is a worst-case guess, not this reply's true
                // size. Build the EXACT reply this one candidate would
                // produce and measure it for real before concluding the
                // session is genuinely unfittable.
                let something_remains =
                    page_continues_beyond_caller_cut || sessions.peek().is_some();
                let exact_cursor =
                    something_remains.then(|| encode_list_cursor(session.created_at, &session.id));
                let exact_reply = ControlMsg::SessionList {
                    req_id,
                    sessions: vec![session.clone()],
                    total,
                    next_cursor: exact_cursor,
                };
                if Frame::control(&exact_reply).encoded_len() <= byte_budget {
                    return Ok(exact_reply);
                }
                return Err(session.id);
            }
            break;
        }
        used += entry_len;
        kept.push(session);
    }

    let more_beyond_this_reply = page_continues_beyond_caller_cut || kept.len() < candidate_len;
    let next_cursor = if more_beyond_this_reply {
        kept.last()
            .map(|last| encode_list_cursor(last.created_at, &last.id))
    } else {
        None
    };

    let reply = ControlMsg::SessionList {
        req_id,
        sessions: kept,
        total,
        next_cursor,
    };
    debug_assert!(
        Frame::control(&reply).encoded_len() <= byte_budget.max(envelope_len),
        "build_list_reply's single-pass size accounting drifted from the real encoded size"
    );
    Ok(reply)
}

/// The already-validated inputs one page walk needs, bundled so the walk
/// itself has a single named entry point rather than a positional
/// parameter list that grows every time paging gains a knob.
///
/// "Already validated" is the contract: raw wire values that could never
/// produce a page — a limit of zero, a cursor that will not decode — are
/// refused by `handle_list_sessions` BEFORE any list work is admitted, so
/// nothing here has to carry a second opinion about them.
pub(crate) struct ListQuery {
    /// Where to resume. `None` starts at the front of [`list_order_key`]'s
    /// order; `Some` resumes STRICTLY AFTER the key it names, whether or
    /// not a session bearing that exact key still exists.
    pub(crate) cursor: Option<ListCursor>,
    /// The most entries this page may carry. Honored as given — the count
    /// cut is this walk's only count cut, and [`LIST_BYTE_BUDGET`] is what
    /// bounds the reply regardless of how large a limit asked for.
    pub(crate) limit: usize,
}

/// One page of the walk, in the three pieces [`build_list_reply`] needs:
/// the entries themselves, the PRE-CUT session count (which the reply
/// reports as-is and cannot reconstruct from a page), and whether the
/// count/cursor cut left anything behind it.
pub(crate) struct ListPage {
    pub(crate) sessions: Vec<SessionInfo>,
    pub(crate) total: u64,
    pub(crate) more_beyond_page: bool,
}

/// Walk one `ListSessions` page: snapshot the session map, order it, cut
/// out the page `query` asks for, and describe every entry on it.
///
/// Two of this walk's reads are BATCHED — taken once for the whole page
/// rather than once per entry — and both are cut that way on purpose: one
/// tmux probe for liveness, and (only when some session on the page names a
/// profile) one read of the profile catalog, from which every
/// source-profile existence on the page is derived. That is a statement
/// about those two reads and not a count of everything this function does:
/// the observation loop below still reads a launch sentinel per entry that
/// has one, and commits its transitions in one batched write.
///
/// The whole of what a list request DOES, with nothing of how it answers:
/// no reply channel, no frames, no `req_id`. `handle_list_sessions` keeps
/// the request-shaped half — validating raw wire values on the way in and
/// mapping this result onto a reply or an error on the way out — which is
/// what lets the walk be exercised (and, per PLAN_M6.md, eventually
/// filtered) without a connection anywhere in the picture.
///
/// Every failure is an `Err` carrying the original error verbatim, and
/// every one of them fails the WHOLE request: an unreadable launch
/// sentinel or an unclassified tmux failure would otherwise leave this
/// page reporting an inference the unread file might contradict. Nothing
/// observed is committed before such a failure — see the observation loop
/// below.
pub(crate) async fn list_page(sup: &Supervisor, query: ListQuery) -> anyhow::Result<ListPage> {
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
        let start = match &query.cursor {
            Some(key) => {
                let key = (std::cmp::Reverse(key.created_at), key.id.as_str());
                ordered.partition_point(|e| list_order_key(&e.info) <= key)
            }
            None => 0,
        };
        let remaining = ordered.len() - start;
        let take_n = query.limit.min(remaining);
        let page: Vec<Arc<SessionEntry>> = ordered[start..start + take_n].to_vec();
        let more_beyond_page = start + take_n < ordered.len();
        (page, more_beyond_page)
    };
    // Before the reply is computed, so an identity claimed on
    // this very pass is reflected in the `restart_offer` it
    // carries rather than only in the next poll's. Cheap by
    // construction for the steady state (see `capture_pass`'s
    // cost envelope) and over EVERY session rather than this
    // reply's capped subset — see `Supervisor::capture_now` for
    // why the cap must not bind the ambiguity rule.
    //
    // KEPT after PLAN_M6_75.md item 1 gave the supervisor its own
    // ticker, deliberately: the ticker guarantees that capture
    // PROGRESSES with nobody connected, while this call is what
    // makes a REPLY fresh as of the request it answers. Proto v10
    // gives the supervisor edge no push, so a drain's reply is the
    // only way a client ever learns anything; the helm's
    // post-write wake exists precisely so a create is followed
    // immediately by a drain, and a drain answering from a sweep
    // that predates its own request would describe the world
    // before the write it is racing.
    //
    // That is why this is a `CaptureReason::Reply` pass and not
    // the old single-flight skip. The skip was wrong here in both
    // directions: it could reply from a PRE-COMMIT `restart_offer`
    // (somebody else's pass was in flight, so this one gave up),
    // and it did not actually make the two cadences free —
    // skipping only ever collapses passes that OVERLAP, and a
    // 2-second ticker beside a 3-second drain mostly does not.
    // Suppression on the TICKER side is what buys that back; see
    // `Supervisor::capture_pass_for` for the whole rule and its
    // real cost envelope.
    //
    // The multiplication to keep in view: this sweep runs PER PAGE,
    // not once per `ListSessions` walk — a caller paging through N
    // pages to see the whole session set pays N whole-host capture
    // sweeps, not one, and the `Reply` rule means each of those N
    // genuinely runs rather than coalescing, because each page is a
    // fresh request that no earlier pass can answer for. That is the
    // correct behavior (a page must be as fresh as its own request)
    // and it is the dominant term in this path's cost, so a future
    // drain that pages aggressively should be designed knowing it.
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
        // An error here is reached only for a genuinely
        // UNCLASSIFIED tmux failure: `TmuxDriver::pane_states`
        // itself now tolerates a vanished private tmux server
        // (the whole reason a dead-tmux-server `ListSessions`
        // no longer lands here at all — see that method's
        // own docs for why an empty pane-states map is
        // honest, not fabricated, in that case).
        sup.tmux.pane_states().await?
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
        // Loud propagation, not fall-through (item 1): the
        // WHOLE request fails rather than silently basing
        // this — or any other — entry's reply on an
        // inference the unreadable sentinel might
        // contradict. Nothing gathered so far this pass is
        // committed: this `?` short-circuits before
        // `transition_many` is ever called.
        let observed = observe_entry(sup, entry, &pane_states).await?;
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
                        *entry.outcome.lock().expect("outcome mutex poisoned") = outcome.clone();
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
                        cleanup_launch_artifacts(&sup.state_dir, &entry.info.id, entry.generation)
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
    // ONE catalog read for the whole page (`farhelm_proto::SourceProfile`'s
    // note on per-snapshot lookup cost), and only when a session on this
    // page actually names a profile — the overwhelmingly common page, where
    // every session is raw-created, pays nothing at all.
    //
    // A failed read FAILS the request rather than degrading to an empty
    // catalog, and the asymmetry is the point: an empty catalog is
    // indistinguishable from "every profile was deleted", so degrading
    // would make a transient database error render as a page of sessions
    // whose profiles are all gone. A failed list is retried in a second;
    // that lie is not correctable by anything.
    let profiles = if entries
        .iter()
        .any(|entry| entry.info.source_profile.is_some())
    {
        sup.store
            .profile_names()
            .await
            .context("reading the profile catalog to describe these sessions' source profiles")?
    } else {
        ProfileNames::new()
    };
    let sessions: Vec<SessionInfo> = entries
        .iter()
        .map(|entry| {
            entry_info(
                entry,
                &pane_states,
                sentinel_hits.get(&entry.info.id).map(String::as_str),
                &profiles,
            )
        })
        .collect();
    Ok(ListPage {
        sessions,
        total,
        more_beyond_page,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use farhelm_proto::{RestartOffer, SessionStatus};

    /// A minimal, distinct `SessionInfo` for `build_list_reply`'s own
    /// tests — distinct ids so a truncation bug that drops the wrong
    /// entries (rather than merely the wrong COUNT) would still be
    /// caught.
    fn fake_session(id: &str, title_len: usize) -> SessionInfo {
        SessionInfo {
            creation_seq: None,
            parent: None,
            archived: false,
            id: id.to_string(),
            title: "x".repeat(title_len),
            created_at: 1_700_000_000,
            cwd: "/tmp".to_string(),
            invocation: "agent".to_string(),
            status: SessionStatus::Running,
            annotation: None,
            restart_offer: RestartOffer::default(),
            tabs: Vec::new(),
            source_profile: None,
        }
    }

    /// The common case: everything fits under the byte budget AND the
    /// caller already handed over the whole remaining walk
    /// (`page_continues_beyond_caller_cut: false`), so nothing is dropped
    /// and there is nothing left to resume — `next_cursor` is `None`. The
    /// OTHER tests below are what actually exercise a cut producing a real
    /// cursor. `total` is passed explicitly here (as the real
    /// `ListSessions` call site does — see that arm's own comment) rather
    /// than derived from `sessions.len()`, since `build_list_reply` does
    /// not own count/cursor cutting itself (the caller applies that before
    /// this is ever reached; the handler-level walk is pinned by
    /// `handlers`' own `ListSessions` tests).
    #[test]
    fn build_list_reply_keeps_everything_under_the_byte_budget() {
        let sessions: Vec<SessionInfo> = (0..10).map(|i| fake_session(&i.to_string(), 4)).collect();
        let reply = build_list_reply(1, sessions, 10, LIST_BYTE_BUDGET, false)
            .expect("every session fits comfortably under the byte budget");
        let ControlMsg::SessionList {
            req_id,
            sessions,
            total,
            next_cursor,
        } = reply
        else {
            panic!("expected ControlMsg::SessionList, got {reply:?}");
        };
        assert_eq!(req_id, 1);
        assert_eq!(sessions.len(), 10);
        assert_eq!(total, 10);
        assert_eq!(next_cursor, None);
    }

    /// The byte-budget's whole job: a count well under any cap can still
    /// overflow a small budget if the records themselves are fat, and the
    /// reply must keep dropping from the tail until it fits — and, since
    /// the drop leaves sessions behind that this call's own candidate DID
    /// include, `next_cursor` must now carry a real resume point even
    /// though the caller passed `page_continues_beyond_caller_cut: false`
    /// (the cut here is entirely `build_list_reply`'s own byte-budget cut,
    /// not something the caller already knew about).
    #[test]
    fn build_list_reply_enforces_the_byte_budget_independent_of_count() {
        // Large enough to survive `LIST_CURSOR_RESERVE`'s flat overhead
        // (reserved unconditionally, whether or not this cut ends up
        // needing a cursor — see `build_list_reply`'s own docs) and still
        // leave room for at least one ~200-byte-titled entry, but well
        // under what all 5 would need — the whole point being a REAL,
        // non-degenerate cut: some entries kept, some dropped.
        const BUDGET: usize = 1000;
        let sessions: Vec<SessionInfo> =
            (0..5).map(|i| fake_session(&i.to_string(), 200)).collect();
        let reply = build_list_reply(1, sessions, 5, BUDGET, false)
            .expect("BUDGET keeps at least the first entry, so this is an ordinary tail cut");
        let ControlMsg::SessionList {
            sessions,
            total,
            next_cursor,
            ..
        } = reply
        else {
            panic!("expected ControlMsg::SessionList, got {reply:?}");
        };
        assert_eq!(total, 5);
        assert!(
            sessions.len() < 5,
            "fat records must be dropped even though the count never reached any cap"
        );
        let last_kept = sessions.last().expect("at least one entry must survive");
        assert_eq!(
            next_cursor,
            Some(encode_list_cursor(last_kept.created_at, &last_kept.id)),
            "a byte-budget cut must carry a real cursor resuming after the last kept entry"
        );
        assert!(
            Frame::control(&ControlMsg::SessionList {
                req_id: 1,
                sessions,
                total,
                next_cursor,
            })
            .encoded_len()
                <= BUDGET,
            "the kept reply, cursor included, must actually respect the byte budget"
        );
    }

    /// Exact-prefix pin for the single-pass accounting itself: a budget
    /// derived from a REAL encoded reply — including the cursor this cut
    /// now actually carries, via `Frame::control`, not by repeating
    /// `build_list_reply`'s own per-entry/envelope arithmetic — for
    /// EXACTLY `K` entries must keep exactly those `K` and drop the rest.
    ///
    /// The budget is derived from a reply already shaped WITH its cursor
    /// (`next_cursor: Some(encode_list_cursor(...))` for entry `K-1`)
    /// rather than `None`, because that is what `build_list_reply` itself
    /// will actually produce for a cut page — `LIST_CURSOR_RESERVE`'s
    /// worst-case reserve during the scan is deliberately conservative
    /// (see that constant's own docs), so a budget derived from the
    /// EXACT real cursor size, rather than the reserve, is what proves
    /// the scan still keeps exactly `K` once the reserve is accounted
    /// for rather than the exact bytes.
    #[test]
    fn build_list_reply_keeps_exactly_the_entries_a_derived_budget_fits() {
        let sessions: Vec<SessionInfo> = (0..5).map(|i| fake_session(&i.to_string(), 20)).collect();
        let total = sessions.len() as u64;
        const K: usize = 3;

        let k_cursor = encode_list_cursor(sessions[K - 1].created_at, &sessions[K - 1].id);
        let k_reply = ControlMsg::SessionList {
            req_id: 1,
            sessions: sessions[..K].to_vec(),
            total,
            next_cursor: Some(k_cursor),
        };
        // The scan budgets `LIST_CURSOR_RESERVE` worst-case bytes for a
        // cursor before it knows the real one, so the derived budget must
        // give the scan that same headroom, or a real K-entry reply
        // (whose actual cursor is smaller than the reserve) would come up
        // short purely from the reserve's own conservatism, not from a
        // bug in the per-entry accounting this test exists to pin.
        let budget = Frame::control(&k_reply).encoded_len() + LIST_CURSOR_RESERVE;

        let reply = build_list_reply(1, sessions.clone(), total, budget, false)
            .expect("the derived budget keeps K entries, so this is an ordinary tail cut");
        let ControlMsg::SessionList { sessions: kept, .. } = reply else {
            panic!("expected ControlMsg::SessionList, got {reply:?}");
        };
        assert_eq!(
            kept,
            sessions[..K],
            "a budget derived from a real K-entry reply must keep exactly those K"
        );
    }

    /// (E5 of the M6.75 review-swarm batch: this test's name used to call
    /// this boundary "exact-fit", which overstated it — `budget` below is
    /// the cursor-less reply's own size PLUS the full `LIST_CURSOR_RESERVE`
    /// padding, not the reply's true encoded size, so there is slack left
    /// over even at the "boundary". What this actually pins: a budget
    /// generous enough that `LIST_CURSOR_RESERVE`'s conservatism alone
    /// (never the real per-entry accounting) decides whether all 5
    /// sessions survive, and the walk truly reaching the end (so
    /// `page_continues_beyond_caller_cut: false`) still yields no cursor —
    /// not silently dropping the last entry. The boundary most likely to
    /// regress if `build_list_reply`'s single-pass accounting ever drifts
    /// from `Frame::control`'s real output again.
    #[test]
    fn build_list_reply_keeps_everything_at_a_reserve_padded_boundary() {
        let sessions: Vec<SessionInfo> = (0..5).map(|i| fake_session(&i.to_string(), 20)).collect();
        let total = sessions.len() as u64;

        let full_reply = ControlMsg::SessionList {
            req_id: 1,
            sessions: sessions.clone(),
            total,
            next_cursor: None,
        };
        // Padded by `LIST_CURSOR_RESERVE`: the scan reserves that much
        // headroom for a cursor unconditionally (see `build_list_reply`'s
        // own docs on why), so a budget derived from the cursor-less
        // reply's exact size alone would be `LIST_CURSOR_RESERVE` bytes
        // short of what the scan needs to accept the last entry, even
        // though the FINAL reply never ends up needing a cursor at all.
        let budget = Frame::control(&full_reply).encoded_len() + LIST_CURSOR_RESERVE;

        let reply = build_list_reply(1, sessions.clone(), total, budget, false)
            .expect("a reserve-padded budget covering all 5 sessions keeps all 5");
        let ControlMsg::SessionList {
            sessions: kept,
            next_cursor,
            ..
        } = reply
        else {
            panic!("expected ControlMsg::SessionList, got {reply:?}");
        };
        assert_eq!(
            kept, sessions,
            "a reserve-padded budget covering everything must not drop the last entry"
        );
        assert_eq!(
            next_cursor, None,
            "a page that genuinely reaches the end of the walk must not carry a cursor"
        );
    }

    /// The degenerate case for the single-pass entry scan: an empty
    /// `sessions` vec simply never enters the `for` loop at all, so this
    /// pins that the empty case still produces a well-formed reply —
    /// `total: 0`, `next_cursor: None` — through the ordinary path, not a
    /// special case that could drift from it.
    #[test]
    fn build_list_reply_handles_zero_sessions() {
        let reply = build_list_reply(1, Vec::new(), 0, LIST_BYTE_BUDGET, false)
            .expect("an empty candidate list is never the degenerate too-large-to-fit case");
        let ControlMsg::SessionList {
            sessions,
            total,
            next_cursor,
            ..
        } = reply
        else {
            panic!("expected ControlMsg::SessionList, got {reply:?}");
        };
        assert!(sessions.is_empty());
        assert_eq!(total, 0);
        assert_eq!(next_cursor, None);
    }

    /// The degenerate byte-budget case named in `build_list_reply`'s own
    /// docs: a budget too small to fit even ONE entry (alongside the
    /// envelope and the cursor reserve) leaves `kept` empty. Before Theme B
    /// of the M6.75 review-swarm batch this answered `next_cursor: None`
    /// with an empty `sessions` list — a lie: `total: 1` alongside an empty
    /// page and no cursor claims the walk is exhausted, when really one
    /// session exists and can never be represented on any page at this
    /// budget. The fix makes that unrepresentable state an explicit `Err`
    /// instead, named by the session's id, rather than a fake empty
    /// success. Not a scenario production ever reaches with the real
    /// budget (`LIST_BYTE_BUDGET` has multi-megabyte headroom) — this pins
    /// the degenerate-budget path itself; `build_list_reply_refuses_a_fat_single_record`
    /// below pins the realistic trigger (an oversized field, not a starved
    /// budget).
    #[test]
    fn build_list_reply_with_a_budget_too_small_for_one_entry_is_refused() {
        let sessions = vec![fake_session("only", 4)];
        let envelope_only_budget = Frame::control(&ControlMsg::SessionList {
            req_id: 1,
            sessions: Vec::new(),
            total: 1,
            next_cursor: None,
        })
        .encoded_len();
        let unfit_id = build_list_reply(1, sessions, 1, envelope_only_budget, false)
            .expect_err("no room for even one entry must be refused, not answered as empty");
        assert_eq!(
            unfit_id, "only",
            "the refusal must name the session that could not fit"
        );
    }

    /// The realistic trigger for the same refusal (Theme B): nothing bounds
    /// a session record's size below `LIST_BYTE_BUDGET` on its own — tabs
    /// accumulate per `OpenTab` without a cap, and a title is caller-
    /// supplied — so a single fat record exceeding the budget by itself is
    /// reachable in production, unlike the previous test's starved-budget
    /// fixture. This is the scenario six review-swarm panelists converged
    /// on: a fat first record must not silently look like an exhausted,
    /// empty walk.
    #[test]
    fn build_list_reply_refuses_a_fat_single_record() {
        let sessions = vec![fake_session(
            "too-fat",
            farhelm_proto::MAX_FRAME_LEN as usize,
        )];
        let unfit_id = build_list_reply(1, sessions, 1, LIST_BYTE_BUDGET, false)
            .expect_err("a record fatter than the whole byte budget must be refused");
        assert_eq!(
            unfit_id, "too-fat",
            "the refusal must name the session that could not fit"
        );
    }

    /// PR 3 follow-up round, item 1: the reserve-padded first pass can
    /// refuse the FIRST candidate examined even though the reply it would
    /// actually belong to fits — the reserve is a flat, worst-case guess
    /// for a cursor this page may not even end up emitting. This pins the
    /// FINAL-page shape of the recheck `build_list_reply` now runs before
    /// giving up on an empty `kept`: the one candidate is also the whole
    /// remaining walk (`page_continues_beyond_caller_cut: false`, nothing
    /// else in `sessions`), so its exact reply carries `next_cursor: None`
    /// — cheaper than the reserve-padded pass assumed, which is exactly
    /// why that pass alone would have wrongly refused it.
    ///
    /// The budget is set to the reply's own EXACT encoded size — the
    /// tightest budget at which this candidate is still honestly
    /// representable — so the test also pins the OTHER edge: one byte
    /// under that true minimum, even the recheck must refuse it, since at
    /// that point the reserve's pessimism is no longer the only thing
    /// standing between the candidate and a real oversize.
    #[test]
    fn build_list_reply_zero_kept_recheck_admits_a_cursorless_final_page_at_its_exact_size() {
        let solo = fake_session("solo-final", 40);
        let exact_reply = ControlMsg::SessionList {
            req_id: 1,
            sessions: vec![solo.clone()],
            total: 1,
            next_cursor: None,
        };
        let exact_len = Frame::control(&exact_reply).encoded_len();

        let reply = build_list_reply(1, vec![solo.clone()], 1, exact_len, false).expect(
            "the exact cursorless reply fits at its own exact size, even though the \
             reserve-padded pass alone (LIST_CURSOR_RESERVE bytes more demanding) would refuse it",
        );
        assert_eq!(
            reply, exact_reply,
            "the recheck must return the SAME reply a direct encode of the sole candidate produces"
        );

        let unfit_id = build_list_reply(1, vec![solo], 1, exact_len - 1, false)
            .expect_err("one byte under the true minimum must still be refused");
        assert_eq!(unfit_id, "solo-final");
    }

    /// The sibling boundary to the test above: a CONTINUING single-entry
    /// page, whose reply carries `next_cursor: Some(..)` because
    /// something remains beyond it (here, `page_continues_beyond_caller_
    /// cut: true`, but an unconsumed second candidate in `sessions` would
    /// trigger the same `sessions.peek().is_some()` branch of the
    /// recheck). The REAL encoded cursor for a short id like this test's
    /// is far under `LIST_CURSOR_RESERVE`'s 200-byte worst-case allowance
    /// — the whole reason the reserve-padded pass refuses a candidate the
    /// recheck's exact accounting then accepts.
    ///
    /// Same "one byte under the true minimum" companion assertion as the
    /// cursorless sibling, so both recheck outcomes (cursorless and
    /// `Some`-cursor) are pinned at both their admitting and their
    /// refusing edge.
    #[test]
    fn build_list_reply_zero_kept_recheck_admits_a_continuing_single_entry_page_at_its_exact_size()
    {
        let solo = fake_session("solo-continuing", 40);
        let cursor = encode_list_cursor(solo.created_at, &solo.id);
        let exact_reply = ControlMsg::SessionList {
            req_id: 1,
            sessions: vec![solo.clone()],
            total: 5,
            next_cursor: Some(cursor),
        };
        let exact_len = Frame::control(&exact_reply).encoded_len();

        let reply = build_list_reply(1, vec![solo.clone()], 5, exact_len, true).expect(
            "the exact Some(cursor) reply fits at its own exact size, even though the \
             reserve-padded pass alone would refuse it",
        );
        assert_eq!(
            reply, exact_reply,
            "the recheck must return the SAME reply a direct encode of the sole candidate produces"
        );

        let unfit_id = build_list_reply(1, vec![solo], 5, exact_len - 1, true)
            .expect_err("one byte under the true minimum must still be refused");
        assert_eq!(unfit_id, "solo-continuing");
    }

    /// `LIST_CURSOR_RESERVE`'s worst-case headroom must actually cover the
    /// REAL worst case — not merely whatever short, hand-picked ids the
    /// other `build_list_reply` tests above happen to use, which could all
    /// pass while a genuine maximum-length cursor still overruns the
    /// reserve in production. The maximum key `list_order_key` can ever
    /// produce is `created_at` at `i64::MIN` (its widest decimal form) and
    /// a 36-character UUID id (`LIST_CURSOR_RESERVE`'s own docs assume
    /// this shape — every id this crate mints is `Uuid::new_v4().to_
    /// string()`).
    ///
    /// Compares the REAL marginal cost `build_list_reply` pays for going
    /// from `next_cursor: None` to this worst-case `Some` — the full JSON
    /// delta (key, quoting, and value), via `Frame::control`, not the
    /// cursor string's raw length alone, which would ignore everything the
    /// reserve also has to cover besides the string itself.
    #[test]
    fn list_cursor_reserve_covers_the_maximum_encoded_cursor_delta() {
        let max_id = uuid::Uuid::nil().to_string();
        assert_eq!(
            max_id.len(),
            36,
            "every id this crate mints (Uuid::new_v4().to_string()) is 36 characters; a nil \
             UUID's string form is the same length and cheaper to construct as a fixture"
        );
        let max_cursor = encode_list_cursor(i64::MIN, &max_id);

        let without_cursor = Frame::control(&ControlMsg::SessionList {
            req_id: 1,
            sessions: Vec::new(),
            total: 0,
            next_cursor: None,
        })
        .encoded_len();
        let with_max_cursor = Frame::control(&ControlMsg::SessionList {
            req_id: 1,
            sessions: Vec::new(),
            total: 0,
            next_cursor: Some(max_cursor),
        })
        .encoded_len();
        let delta = with_max_cursor - without_cursor;
        assert!(
            delta <= LIST_CURSOR_RESERVE,
            "LIST_CURSOR_RESERVE ({LIST_CURSOR_RESERVE}) must cover the real worst-case cursor \
             delta ({delta} bytes) for the maximum-length key this crate can ever emit, or \
             build_list_reply's budget accounting could silently undercount"
        );
    }

    /// `encode_list_cursor`/`decode_list_cursor` round trip exactly —
    /// the ordinary path every valid `next_cursor`/`ListSessions::cursor`
    /// pair takes.
    #[test]
    fn list_cursor_round_trips() {
        let encoded = encode_list_cursor(1_700_000_042, "session-abc");
        let decoded = decode_list_cursor(&encoded).expect("a freshly encoded cursor must decode");
        assert_eq!(decoded.created_at, 1_700_000_042);
        assert_eq!(decoded.id, "session-abc");
    }

    /// `handle_list_sessions`'s whole malformed-cursor contract rests on
    /// `decode_list_cursor` never panicking and always collapsing every
    /// UNDECODABLE shape to the same `None` — pinned directly here so a
    /// regression is caught at the smallest possible unit rather than only
    /// via the handler-level `InvalidRequest` tests. This is decoding
    /// coverage, not forge-proofing: every fixture below is malformed at
    /// the ENCODING level (bytes that cannot become a `ListCursor` at all),
    /// never a well-formed key naming a session this supervisor never
    /// issued one for — that shape decodes FINE and is accepted by design
    /// (`list_sessions_cursor_from_a_deleted_session_still_resumes` pins
    /// why: cursors carry no authority in a single-user supervisor). Three
    /// distinct malformed shapes: a bit flip inside otherwise-valid base64,
    /// base64 truncated mid-value, and a value that decodes to valid
    /// base64/JSON but the WRONG shape (an unrelated JSON object) — the
    /// last of which a naive "does it base64-decode" check would miss
    /// entirely.
    #[test]
    fn list_cursor_decode_rejects_malformed_input_without_panicking() {
        let valid = encode_list_cursor(1_700_000_000, "s1");

        let mut flipped = valid.clone().into_bytes();
        // Flip one bit in the middle of the encoded value — still the same
        // length, still plausible base64 alphabet-wise for most flips, but
        // no longer the bytes that were actually encoded.
        let mid = flipped.len() / 2;
        flipped[mid] ^= 0x01;
        let flipped = String::from_utf8(flipped).unwrap_or_default();

        let truncated = &valid[..valid.len() / 2];

        for malformed in [flipped.as_str(), truncated, "not-base64-at-all!!", ""] {
            assert!(
                decode_list_cursor(malformed).is_none(),
                "malformed cursor {malformed:?} must fail to decode, not panic or succeed"
            );
        }

        // Valid base64 of valid JSON, but the wrong SHAPE — proves decoding
        // checks the structure, not merely "is this base64 of some JSON".
        use base64::Engine;
        let wrong_shape = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&serde_json::json!({"unrelated": true})).unwrap());
        assert!(decode_list_cursor(&wrong_shape).is_none());
    }
}
