//! Conversation-identity capture, from first input through a durable claim.
//!
//! A capture pass correlates every eligible session against the agent record trees under
//! the configured home directory. The scan stays host-wide because two sessions in one
//! working directory can make each other's evidence ambiguous; evaluating either session
//! alone could turn that refusal into a silently wrong conversation.
//!
//! The state ladder delays publication until the capture horizon has closed on one complete,
//! unambiguous scan. Pass scheduling keeps reply-producing callers fresh while allowing the
//! periodic ticker to skip redundant work. Keeping the seams, ladder, scheduling, and
//! scan-and-commit sequence together makes those two contracts reviewable as one unit.
//!
//! ## The scan is the fallback, not the authority
//!
//! The scan is no longer the only source of identity. Where an agent kind supports it, farhelm
//! appends a per-launch command-line hook at spawn time and the agent reports its own
//! conversation id from inside its own process, every time that id comes into being — including
//! the ids that `/clear` and `/new` mint mid-run, which no amount of outside observation can
//! attribute correctly. That report arrives as [`CaptureState::Reported`] and dominates every
//! state the scan can produce.
//!
//! This does not make farhelm an agent-configuring integration: the hook rides one launch's argv
//! and touches no user configuration or record directory, and a launch that carries no hook (an
//! unsupported kind, an argv shape that forbids injection, hooks disabled) is served by the scan
//! exactly as before. So the ordering to hold in mind is: identity is REPORTED where it can be,
//! and inferred where it cannot — the scan is the fallback, never the override.

use super::core::{SessionEntry, Supervisor};
use crate::agent_kind::{CaptureVerdict, CaptureWindow, CaptureWindowBounds, RecordStamp};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// Which durable write of the conversation-capture machinery a
/// [`CaptureStoreFault`] is being asked about.
///
/// Named rather than a bare "fail the next write" switch because the three
/// have different consequences and the tests need to provoke them
/// independently: losing the first-input anchor costs capture across a
/// restart, losing a claim costs the `Resume` offer until a retry lands,
/// and losing an ambiguity verdict would — without its retry — let a
/// restart re-decide on thinner evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureWrite {
    /// `store::SessionStore::record_first_input`.
    FirstInput,
    /// `store::SessionStore::record_captured_conversation`.
    Conversation,
    /// `store::SessionStore::record_capture_ambiguous`.
    Ambiguity,
    /// `store::SessionStore::record_reported_conversation`, reached from
    /// `Supervisor::report_conversation` rather than from a capture pass.
    ///
    /// Sharing the capture seam rather than getting one of its own because
    /// the property under test is the same one the others exist for: a
    /// failed durable write must never leave an in-memory state
    /// advertising `Resume`. That the writer is the report handler instead
    /// of a pass changes who fails, not what must hold.
    Report,
}

/// A failure injected in place of one of the capture machinery's durable
/// writes (PLAN_M3.md item 8).
///
/// A seam rather than a fault-injecting store wrapper because what needs
/// exercising is the SUPERVISOR's retry states — a failed write must never
/// yield an in-memory claim that advertises `Resume`, and the retry has to
/// ride the polling cadence rather than the input path. Production installs
/// none, so every call site is one `Option` check.
pub type CaptureStoreFault = Arc<dyn Fn(CaptureWrite, &str) -> anyhow::Result<()> + Send + Sync>;

/// A hook awaited at the START of every conversation-capture pass, after
/// the pass has taken the coordination lock and committed to running.
///
/// The barrier the scheduling tests need, and there is no substitute for
/// it: `capture_pass_for`'s whole contract is about what happens to a
/// SECOND caller while a first pass is in flight, and a real pass over a
/// small fixture tree finishes far too quickly to make that window
/// observable. Holding the lock from a test would prove the mutual
/// exclusion but not the freshness bookkeeping, which only a real pass
/// records. Production installs none, so the cost is one `Option` clone
/// per pass.
///
/// Returns a boxed future rather than being an `async` trait method
/// because this is a plain `Fn` seam like every other one in
/// [`super::core::SupervisorSeams`]; the box is paid once per pass, next to a
/// filesystem scan.
pub type CaptureGate =
    Arc<dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync>;

/// The first-input correlator and its durability.
///
/// Two fields rather than one `Option<i64>` because the write can FAIL and
/// the failure must be retried somewhere other than the input path: a
/// keystroke may not wait on a journal sync (see `note_first_input`), so
/// the retry rides the polling cadence instead. Until `durable` is true,
/// the anchor exists only for this process — capture still works, but a
/// restart would lose it, so the retry is not cosmetic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FirstInput {
    pub(crate) at: Option<i64>,
    pub(crate) durable: bool,
}

/// One session's conversation-capture progress (PLAN_M3.md item 8).
///
/// ## The claim discipline
///
/// The states form a strict dominance order, enforced by
/// [`CaptureState::advance`], and the whole point of the ladder is that a
/// claim is not made durable while it could still turn out to be wrong:
///
/// 1. `Unclaimed` — nothing yet.
/// 2. `Provisional` — exactly one record matches, but the session's
///    capture window is still OPEN, so a rival record (or a rival
///    session's first input) can still arrive and make this ambiguous.
///    Nothing is written and `farhelm_proto::RestartOffer::Resume` is NOT
///    advertised.
/// 3. `PendingCommit` — the horizon has closed on a COMPLETE scan with
///    exactly one match, so the claim is settled; only the durable write
///    is outstanding. Still not advertised, because the offer means "there
///    is a stored identity a restart can fill in".
/// 4. `UncapturedFinal` — the horizon closed on a complete scan with no
///    match. Terminal, so this session leaves the eligible set instead of
///    rescanning its directory forever.
/// 5. `Captured` — committed and read back. Advertises Resume.
/// 6. `Ambiguous` — dominant over every scan-derived state. Durable, so a
///    restart cannot re-decide on evidence that has since gotten thinner.
/// 7. `Reported` — the agent's own answer, delivered through the launch
///    hook. Dominant over everything, including `Ambiguous`, because it is
///    not evidence ABOUT which record is ours; it is the identity itself.
///    Also advertises Resume.
///
/// Ranks 1-6 are all conclusions the SCAN drew from files on disk, and the
/// ladder among them is a confidence ordering. Rank 7 is a different kind
/// of thing altogether: it does not sit above `Ambiguous` because it is
/// better evidence, but because it is not evidence at all.
///
/// ## What is and is not persisted
///
/// The RESULT is persisted (`captured_conversation`, `capture_ambiguous`);
/// the intermediate progress is not, and deliberately so: after a restart
/// the same evidence is re-examined from scratch, which is what
/// SPEC_impl.md's "re-verifies identity after each restart rather than
/// assuming either behavior" asks for.
#[derive(Debug, Clone)]
pub(crate) enum CaptureState {
    /// Nothing claimed yet, and still eligible for the rescan. Sessions
    /// sit here from create until their agent writes a record — an
    /// unbounded wait by construction, since the record appears at first
    /// prompt submission and nothing bounds when a human types one.
    Unclaimed,
    /// One record matches, but the window is still open, so this is a
    /// working hypothesis rather than a claim. Re-derived from scratch on
    /// every pass, which is exactly what lets a late rival flip it.
    Provisional { conversation: String },
    /// Settled but not yet stored: the durable write failed (or has not
    /// been attempted). Retried on the polling cadence.
    PendingCommit {
        conversation: String,
        record: PathBuf,
        stamp: RecordStamp,
    },
    /// The horizon closed on a complete scan that found nothing. Terminal
    /// — a session whose agent never wrote a correlatable record will not
    /// start doing so, and rescanning its directory on every poll for the
    /// rest of its life would be pure cost. The honest fresh-launch
    /// fallback is the answer from here on.
    UncapturedFinal,
    /// An identity is committed and was read back out of the row. Carries
    /// the record it came from and that record's stamp at the moment it
    /// was last verified: the stamp is what makes re-verification cheap —
    /// an unchanged file needs no read, and a changed one is the
    /// resume-append signal.
    Captured {
        conversation: String,
        record: PathBuf,
        stamp: RecordStamp,
    },
    /// The correlation was ambiguous, so nothing will ever be claimed for
    /// this session again — not "not yet", but "not from this launch".
    ///
    /// Dominant and sticky, which is the mechanical form of SPEC.md's
    /// no-silently-wrong-conversation rule: the collision that produced it
    /// does not become less ambiguous with time, and a later pass that
    /// happened to see only one of the two candidates would claim an
    /// identity on strictly worse evidence than the pass that bailed.
    /// `durable` is false only between the decision and the write that
    /// records it; the write is retried on the polling cadence, and until
    /// it lands the refusal still holds for this process.
    Ambiguous { durable: bool },
    /// The identity the agent itself reported through the launch hook.
    ///
    /// Dominates every scan-derived state, including `Ambiguous`, because
    /// it is not evidence about which record is ours — it IS the agent's
    /// own answer, produced inside the agent's process. The guarantee
    /// `Ambiguous` weakens here existed because scan evidence could not be
    /// trusted; a report is not scan evidence.
    ///
    /// Replaceable only by another `Reported`, and that replacement is the
    /// whole reason this variant exists: `/clear` (Claude) and `/new`
    /// (Codex) start a NEW conversation inside the same process, and the
    /// old id is then precisely the one that must not be resumed any more.
    ///
    /// ## The contract with the durable write
    ///
    /// This state is only ever entered AFTER the store write recording the
    /// same id has succeeded — never before, never speculatively. Modelled
    /// on `commit_capture`: the durable write is what decides what is
    /// claimed, and `committed_conversation` (hence
    /// `farhelm_proto::RestartOffer::Resume`) promises a restart that there
    /// is something stored for it to fill in. The reporting path
    /// (`Supervisor::report_conversation`) owes this ordering; a write that
    /// fails is logged and leaves memory alone, with no retry list. The
    /// vendor does not re-attempt that DELIVERY — the hook call has already
    /// returned, and its result is not revisited — which is not the same as
    /// the hook never firing again: a later lifecycle event in the same
    /// process (another `/clear`, a resume, a compaction) fires a fresh
    /// hook and produces a fresh report. What is lost is this one report,
    /// and the scan is still running for that session precisely because it
    /// never reached `Reported`.
    Reported { conversation: String },
}

impl CaptureState {
    /// The DURABLY claimed identity, if any.
    ///
    /// Deliberately `None` for `Provisional` and `PendingCommit`: this is
    /// what `farhelm_proto::RestartOffer::Resume` is computed from, and the
    /// offer is a promise that a stored identity exists for restart to fill
    /// in. A provisional match is not that promise, and a pending one is not
    /// yet.
    ///
    /// `Reported` qualifies for the same reason `Captured` does and not by
    /// exception: it is only ever entered once its own durable write has
    /// landed (see the variant's docs), so by the time it can be read here
    /// the row already holds the id.
    pub(crate) fn committed_conversation(&self) -> Option<&str> {
        match self {
            CaptureState::Captured { conversation, .. }
            | CaptureState::Reported { conversation } => Some(conversation.as_str()),
            _ => None,
        }
    }

    /// Position in the dominance order; see the type's own docs.
    fn rank(&self) -> u8 {
        match self {
            CaptureState::Unclaimed => 0,
            CaptureState::Provisional { .. } => 1,
            CaptureState::PendingCommit { .. } => 2,
            CaptureState::UncapturedFinal => 3,
            CaptureState::Captured { .. } => 4,
            CaptureState::Ambiguous { .. } => 5,
            CaptureState::Reported { .. } => 6,
        }
    }

    /// Whether this session still has anything to scan for.
    ///
    /// `Reported` is settled in the strongest sense available: nothing on
    /// disk can improve on the agent's own answer, so the pass neither
    /// scans for such a session nor re-verifies it.
    fn is_settled(&self) -> bool {
        matches!(
            self,
            CaptureState::UncapturedFinal
                | CaptureState::Captured { .. }
                | CaptureState::Ambiguous { .. }
                | CaptureState::Reported { .. }
        )
    }

    /// Move to `next` if the order allows it, returning whether it landed.
    ///
    /// This is the compare-and-set that makes a stale pass harmless. Two
    /// things it protects, both of which the mutual exclusion around
    /// `capture_pass` (`CaptureCoordination`) makes rare rather than
    /// impossible — a delete can interleave at any await, and the entry a
    /// pass is holding can be replaced under it:
    ///
    /// - **Nothing may regress.** A pass that computed `Provisional` from
    ///   evidence gathered before a newer pass found `Ambiguous` must not
    ///   overwrite it. Rank alone gives that.
    /// - **Same-rank refresh is allowed only where re-deriving is the
    ///   point.** `Provisional` and `PendingCommit` are recomputed or
    ///   retried every pass and must be replaceable in place;
    ///   `Captured` must NOT be, because replacing a committed identity
    ///   with a different one is precisely the wrong-conversation move
    ///   this whole design excludes. An `Ambiguous` refresh is allowed so
    ///   the `durable` flag can be updated once its write lands.
    ///
    ///   `Reported` MAY be replaced by another `Reported`, and the reason
    ///   is that it is not the same kind of claim as `Captured`. A
    ///   `Captured` replacement would mean the scan changed its mind about
    ///   which record was ours — a guess overwriting a guess. A `Reported`
    ///   replacement means the agent told us its conversation id changed,
    ///   which is what `/clear` and `/new` literally do inside a running
    ///   process; refusing it would leave the session advertising a resume
    ///   of a conversation the user has already discarded. That is the
    ///   exact bug this state exists to fix, so the replacement is the
    ///   feature rather than a hole in the ladder.
    ///
    /// `pub(crate)` for one caller outside this module:
    /// `Supervisor::report_conversation`, which installs the `Reported`
    /// state a hook delivered. Everything else that advances a capture
    /// state is a capture pass, and lives here.
    pub(crate) fn advance(&mut self, next: CaptureState) -> bool {
        let allowed = match (self.rank(), next.rank()) {
            (current, incoming) if incoming > current => true,
            (current, incoming) if incoming == current => matches!(
                next,
                CaptureState::Provisional { .. }
                    | CaptureState::PendingCommit { .. }
                    | CaptureState::Ambiguous { .. }
                    | CaptureState::Reported { .. }
            ),
            _ => false,
        };
        if allowed {
            *self = next;
        }
        allowed
    }
}

/// Who is asking for a capture pass, and therefore what "recent enough"
/// means for them.
///
/// The two callers want opposite things from a pass that is already in
/// flight, and collapsing them into one policy is what made the original
/// single-flight design wrong in both directions at once (PLAN_M6_75.md
/// item 1's review): a reply-producing caller that SKIPPED could answer
/// from evidence gathered before its own request, and a ticker that
/// WAITED would serialize behind every poll and then sweep again anyway.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaptureReason {
    /// A request whose REPLY describes what the pass concludes — the list
    /// path's `restart_offer`, the restart handler's mode validation — or
    /// a startup pass that must simply happen.
    ///
    /// These join an in-flight pass and then run their own if the pass
    /// they joined began before they asked. Never skipping on that basis
    /// is what the helm's post-write wake depends on: proto v10 puts no
    /// push on the supervisor edge, so a create followed by a drain is the
    /// ONLY way a client learns anything, and a drain replying off a
    /// pre-request sweep would report the state of the world before the
    /// write it is racing.
    Reply,
    /// The supervisor's own periodic tick, which owes freshness to nobody
    /// in particular and only has to guarantee that capture keeps moving.
    ///
    /// Skips outright when a pass is in flight (that pass does the work)
    /// and suppresses itself when one COMPLETED within the tick interval,
    /// so an actively polled supervisor pays for the polls it is already
    /// answering rather than for both cadences.
    Tick {
        /// How recent a completed REPLY pass has to be for this tick to
        /// have nothing to add. The ticker passes its own interval.
        suppress_within: Duration,
    },
}

/// The scheduling state behind [`Supervisor::capture_pass_for`].
///
/// Two locks with different jobs, deliberately. `lock` is the mutual
/// exclusion that keeps two passes from interleaving between gathering
/// evidence and acting on it — the window in which one pass's
/// `Provisional` could overwrite another's `Ambiguous`
/// (`CaptureState::advance`'s compare-and-set is the belt to this
/// suspenders). `history` is a leaf mutex over the bookkeeping, held for a
/// couple of field reads and never across an await, so a caller can decide
/// whether it still needs a pass at all without holding the big one.
pub(super) struct CaptureCoordination {
    pub(super) lock: tokio::sync::Mutex<()>,
    pub(super) history: std::sync::Mutex<CaptureHistory>,
}

/// What the last capture pass did, in this process's monotonic time.
///
/// Both instants are needed and neither substitutes for the other:
/// [`CaptureReason::Reply`] asks "did a pass BEGIN after my request", so
/// that a pass which started before it cannot answer for it, while
/// [`CaptureReason::Tick`] asks "did a pass COMPLETE recently", because a
/// completed pass is what makes another one redundant.
#[derive(Debug, Default)]
pub(super) struct CaptureHistory {
    /// When the most recently COMPLETED pass started — whichever reason
    /// ran it, because a tick's pass observes the world just as well as a
    /// reply's and can therefore answer for a request that arrived before
    /// it began.
    started: Option<Instant>,
    /// When the most recently completed REPLY pass finished.
    ///
    /// Deliberately not "any pass", and the distinction is what keeps the
    /// ticker's own guarantee intact. Counting a tick's own completions
    /// here would have the ticker suppress ITSELF: with nothing else
    /// sweeping, the previous tick's pass is always about one interval old
    /// when the next tick fires, so roughly every other tick would decide
    /// it had nothing to add and the unattended capture cadence would
    /// silently halve — breaking the very "one ticker interval" bound the
    /// ticker exists to provide. Suppression is supposed to mean "a POLL
    /// already did this work", so only a poll records it.
    reply_completed: Option<Instant>,
    /// How many passes have run to completion — the observable counter the
    /// coalescing tests assert against, since nothing about the resulting
    /// capture state can distinguish "one pass" from "two identical
    /// passes".
    completed_count: u64,
}

impl Supervisor {
    /// Run one conversation-capture pass over every session, on behalf of
    /// a caller whose reply depends on what it concludes.
    ///
    /// Joins a pass already in flight and then runs its own unless that
    /// pass BEGAN after this call did — see [`CaptureReason::Reply`] for
    /// why a reply may never be built on evidence older than the request
    /// it answers, and [`Supervisor::capture_pass_for`] for the mechanism.
    ///
    /// Takes the whole session map rather than a caller-supplied subset,
    /// deliberately: the ambiguity rule is a statement about all sessions
    /// sharing a working directory, and evaluating it over an arbitrary
    /// subset (`ListSessions`'s reply cap, say) could let a session outside
    /// that subset fail to poison a window it genuinely occupies — turning
    /// a bail into a wrong capture, the one outcome this design exists to
    /// exclude.
    pub(crate) async fn capture_now(&self) {
        self.capture_pass_for(CaptureReason::Reply).await;
    }

    /// Run a capture pass if `reason`'s freshness rule still wants one.
    ///
    /// The whole scheduling policy, in one place because the two reasons
    /// are only correct RELATIVE to each other. The ordering below is
    /// load-bearing:
    ///
    /// 1. A `Tick` gives up immediately if it cannot take the lock. The
    ///    pass it would have queued behind is about to do its work, and a
    ///    ticker that waited would serialize behind every poll and then
    ///    sweep redundantly the moment it got in.
    /// 2. A `Reply` WAITS for the lock, because giving up would mean
    ///    answering from whatever the in-flight pass happens to have
    ///    concluded so far.
    /// 3. Once the lock is held, no pass is running, so the recorded
    ///    history describes a COMPLETED one. A `Reply` skips only if that
    ///    pass started at or after its own request — it observed
    ///    everything this caller could have asked about. A `Tick` skips if
    ///    a REPLY pass finished within `suppress_within` — its own earlier
    ///    passes deliberately do not count, or the ticker would suppress
    ///    itself into half its cadence (see
    ///    `CaptureHistory::reply_completed`).
    ///
    /// The honest cost envelope, since the previous design's claim that
    /// running both cadences "costs nothing" was false: single-flight
    /// coalescing only ever collapsed passes that OVERLAP, and a 2-second
    /// ticker beside a 3-second drain mostly does not overlap, so the old
    /// shape paid for both. What suppression buys is that an actively
    /// drained supervisor pays roughly the drain's cadence rather than the
    /// sum, and an unattended one pays the ticker's. The worst case is
    /// still one pass per ticker interval; what is excluded is paying for
    /// a tick that a poll has just made redundant.
    pub(crate) async fn capture_pass_for(&self, reason: CaptureReason) {
        if self.agent_home.is_none() {
            // No home means no scan, so there is no pass to run — but the
            // liveness tripwire is not part of the scan. It is a statement
            // about a hook that did not report, which is a fact about the
            // session rather than about any file on disk, and this is
            // precisely the configuration in which nothing else would ever
            // mention it. Bailing before running it, as an earlier shape of
            // this function did, made the tripwire inside `capture_pass`
            // unreachable in production for a homeless supervisor.
            //
            // Run outside the pass lock and the coalescing checks below,
            // because it needs neither: it is pure, it touches only
            // in-memory flags, and each entry's own capture mutex is what
            // makes its check-and-latch atomic against a concurrent caller.
            let entries: Vec<Arc<SessionEntry>> =
                self.sessions.lock().await.values().cloned().collect();
            report_liveness_tripwire(&entries, self.capture_window, crate::agent_kind::now_unix());
            return;
        }
        let requested_at = Instant::now();
        let _pass = match reason {
            CaptureReason::Tick { .. } => match self.capture.lock.try_lock() {
                Ok(guard) => guard,
                Err(_) => return,
            },
            CaptureReason::Reply => self.capture.lock.lock().await,
        };
        {
            let history = self
                .capture
                .history
                .lock()
                .expect("capture history poisoned");
            let redundant = match reason {
                CaptureReason::Reply => history
                    .started
                    .is_some_and(|started| started >= requested_at),
                CaptureReason::Tick { suppress_within } => {
                    history.reply_completed.is_some_and(|completed| {
                        requested_at.duration_since(completed) < suppress_within
                    })
                }
            };
            if redundant {
                return;
            }
        }
        let started = Instant::now();
        if let Some(gate) = self.seams.capture_gate.clone() {
            gate().await;
        }
        let entries: Vec<Arc<SessionEntry>> =
            self.sessions.lock().await.values().cloned().collect();
        // Boxed, not awaited inline: `capture_pass` is by far the largest
        // future in this crate (per-record buffers, nested per-root scans),
        // and every async fn that awaits it inlines that size into its OWN
        // future. Two callers already compose it with a lot of other work
        // — `ListSessions` and the restart handler — and one of them ran a
        // test thread out of stack merely by CONSTRUCTING the combined
        // future. One heap allocation per pass is nothing next to the
        // filesystem scan it wraps.
        Box::pin(capture_pass(self, &entries, self.may_record())).await;
        let mut history = self
            .capture
            .history
            .lock()
            .expect("capture history poisoned");
        history.started = Some(started);
        history.completed_count += 1;
        if matches!(reason, CaptureReason::Reply) {
            history.reply_completed = Some(Instant::now());
        }
    }

    /// How many capture passes have run to completion in this process.
    ///
    /// The only observable difference between correct coalescing and none
    /// at all: two passes over the same evidence leave the same capture
    /// state behind, so nothing about a session can tell them apart. Tests
    /// assert against this; production never reads it.
    #[cfg(test)]
    pub(crate) fn capture_passes_completed(&self) -> u64 {
        self.capture
            .history
            .lock()
            .expect("capture history poisoned")
            .completed_count
    }
}

/// Record that input has been DELIVERED to this session's pane, if this
/// is the first time (PLAN_M3.md item 8's correlator).
///
/// ## Why the first delivered input, and why here
///
/// The agents write their conversation record at first PROMPT submission,
/// not at launch, and the gap between the two is unbounded — a session can
/// sit at a prompt for hours. So the correlator cannot be the launch time,
/// and it cannot be a timeout from launch either; it has to be the last
/// moment before which the record cannot yet exist. That is the instant
/// tmux CONFIRMED it executed a `send-keys` carrying real bytes, which is
/// what `InputClient::delivered_any_bytes` reports: an empty data frame
/// never counts (nothing reached the pane, so nothing could have provoked
/// a record and an earlier anchor would only widen the window for
/// nothing), and a partial send that failed halfway still counts, because
/// the bytes that did land could have been the prompt's newline.
///
/// The honest residual is documented where the window constants are
/// (`agent_kind::CAPTURE_WINDOW_AFTER`): not every delivered byte is a
/// human keystroke, since a terminal emulator answers device-status
/// queries on its own. That can start this clock early, whose failure
/// direction is a MISSED capture, never a wrong one.
///
/// ## The persistence decision
///
/// The in-memory mirror is set SYNCHRONOUSLY and the durable write is
/// spawned. Keystrokes are the most latency-sensitive path in the process
/// and a SQLite commit can block on a real disk flush; making a user wait
/// for a journal sync to see their character echo would be a bad trade for
/// a fact only a background rescan consults. A durable value is still
/// required (rather than memory alone) because the gap this correlator
/// spans is exactly where a supervisor restart is most likely to land: a
/// session whose user typed before the restart and whose agent wrote its
/// record after would otherwise be uncapturable forever. A write that
/// FAILS therefore leaves `FirstInput::durable` false, and `capture_pass`
/// retries it on the polling cadence — off this path, which is the whole
/// point.
///
/// The compare-and-set is what makes exactly ONE of many input frames
/// spawn a write, and the store's own write-once predicate is what makes
/// even that redundant work harmless.
pub(crate) fn note_first_input(sup: &Arc<Supervisor>, entry: &Arc<SessionEntry>) {
    let now = crate::agent_kind::now_unix();
    {
        let mut first = entry
            .first_input
            .lock()
            .expect("first-input mutex poisoned");
        if first.at.is_some() {
            return;
        }
        first.at = Some(now);
        first.durable = false;
    }
    if !sup.may_record() {
        // A supervisor without standing to write (no state-directory
        // claim, or a blind boot-id detector) still tracks this in memory
        // so its OWN capture pass can correlate; it simply does not store
        // a fact about sessions that are not its to own.
        return;
    }
    let sup = Arc::clone(sup);
    let entry = Arc::clone(entry);
    tokio::spawn(async move {
        persist_first_input(&sup, &entry, now).await;
    });
}

/// Store one session's first-input time and mark the mirror durable, or
/// log why it could not be.
///
/// Shared by the input path's spawned write and `capture_pass`'s retry, so
/// the two cannot disagree about what "durable" means.
async fn persist_first_input(sup: &Supervisor, entry: &SessionEntry, at: i64) {
    if let Some(fault) = &sup.seams.capture_store_fault
        && let Err(e) = fault(CaptureWrite::FirstInput, &entry.info.id)
    {
        warn!(
            session = %entry.info.id, error = %format!("{e:#}"),
            "injected failure while persisting this session's first-input time"
        );
        return;
    }
    match sup
        .store
        .record_first_input(&entry.info.id, entry.generation, at)
        .await
    {
        Ok(()) => {
            entry
                .first_input
                .lock()
                .expect("first-input mutex poisoned")
                .durable = true;
        }
        Err(e) => warn!(
            session = %entry.info.id, error = %format!("{e:#}"),
            "could not persist this session's first-input time; correlation still works for \
             this supervisor's lifetime and the next capture pass retries the write"
        ),
    }
}

/// Every conversation id currently held as `Reported`, grouped the way
/// correlation groups sessions: by agent kind and canonical working
/// directory.
///
/// This is the one thing a report buys the RIVALS of the session that made
/// it. A record whose id another session in the group has been TOLD is its
/// own cannot also be this one's, so it drops out of the rival's candidate
/// list before any verdict is computed — strictly more evidence, never
/// less.
///
/// Deliberately independent of first input: Claude reports at process
/// startup, so a session can hold `Reported` before this supervisor has
/// confirmed a single keystroke for it. Such a session occupies no capture
/// window at all, but its id is spoken for regardless and must not be
/// claimable by anyone else.
///
/// A function rather than an inline walk because a pass reads it MANY
/// times: once immediately before each pending claim's commit decision, and
/// once at the top of each verdict-loop iteration. Both loops await store
/// writes, so one snapshot per loop would answer about the moment the loop
/// STARTED rather than the moment each decision is made. That is affordable
/// only because this is a pure in-memory walk over `entries` — no lock is
/// held across it and no I/O happens in it — and keeping the collection in
/// one place is what keeps every one of those answers built the same way.
///
/// Keyed on the kind's stable column spelling rather than the wire enum
/// only because `AgentKind` is not `Hash`; the mapping is injective, so the
/// grouping is exactly the one `occupied` uses.
fn reported_ids<'a>(
    entries: &'a [Arc<SessionEntry>],
) -> HashMap<(&'static str, &'a str), HashSet<String>> {
    let mut ids: HashMap<(&'static str, &'a str), HashSet<String>> = HashMap::new();
    for entry in entries {
        let (Some(_), Some(cwd)) = (entry.snapshot.integration(), entry.canonical_cwd.as_deref())
        else {
            continue;
        };
        if let CaptureState::Reported { conversation } =
            &*entry.capture.lock().expect("capture mutex poisoned")
        {
            ids.entry((crate::store::agent_kind_column(entry.snapshot.kind), cwd))
                .or_default()
                .insert(conversation.clone());
        }
    }
    ids
}

/// Whether `conversation` is held as `Reported` by some session in
/// `entry`'s correlation group.
///
/// Note what this does NOT exclude: a session that reported an id is itself
/// in the set that named it. Every caller is asking about a SCAN verdict,
/// and a `Reported` session never produces one — it is settled, so it never
/// reaches the scanning set and never holds a pending claim. An entry with
/// no integration or no canonical cwd belongs to no group and is therefore
/// never spoken for.
fn is_spoken_for(
    reported: &HashMap<(&'static str, &str), HashSet<String>>,
    entry: &SessionEntry,
    conversation: &str,
) -> bool {
    let Some(cwd) = entry.canonical_cwd.as_deref() else {
        return false;
    };
    reported
        .get(&(crate::store::agent_kind_column(entry.snapshot.kind), cwd))
        .is_some_and(|ids| ids.contains(conversation))
}

/// One conversation-capture rescan across every session this supervisor
/// holds (PLAN_M3.md item 8).
///
/// ## Why polling, and what it costs
///
/// SPEC_impl.md says "watch"; it does not mandate inotify, and this
/// implementation deliberately polls — from the supervisor's own periodic
/// ticker (PLAN_M6_75.md item 1, which is what makes progress independent
/// of any caller) and from the passes it already performs anyway, every
/// `ListSessions` and every reload. That buys correctness properties an
/// event watcher would have to re-earn: one task to supervise rather than
/// a watch registration per directory; no missed-event class (an inotify
/// watch registered a moment too late, or one that hits the per-user watch
/// limit, silently never fires); no per-directory registration for working
/// directories that come and go; and identical behavior on a restart,
/// since the rescan is the same code that runs at steady state.
///
/// The cost of being late is worth stating honestly, because "one poll
/// interval" undersells it. For a capture that only the TICKER will ever
/// drive, the bound is the ticker interval, PLUS however long a busy
/// sampling limiter makes the tick wait for its permit, PLUS the pass's
/// own duration — and a tick that finds a pass in flight defers to the
/// next interval rather than queueing. None of that is on an interactive
/// path, which is why an unbounded-in-principle delay is acceptable here
/// and would not be anywhere else; a caller that needs an answer as fresh
/// as its own request asks for one (`CaptureReason::Reply`) instead of
/// waiting for a tick.
///
/// The cost envelope, per pass:
///
/// - Sessions with a non-integrated kind, no first-input time yet, or a
///   SETTLED verdict (`CaptureState::is_settled`) cost ZERO filesystem
///   work. That is the steady state for essentially every session:
///   eligibility begins at first input and ends at the horizon, so the
///   eligible set drains rather than accumulating — which is exactly what
///   `UncapturedFinal` exists to guarantee for the sessions that never
///   produce a record at all.
/// - An eligible session costs a share of ONE scan per record ROOT (see
///   `agent_kind::scan_records` for that scan's own three budgets); roots
///   are shared, which matters most for Codex, where every session on the
///   host has the same one.
/// - An already-captured session costs one `stat` on its own record, and
///   re-reads it only when that stamp moved — which is exactly the
///   resume-append signal SPEC_impl.md describes.
///
/// ## The claim discipline
///
/// A match is not durable while it could still turn out to be wrong. Until
/// a session's HORIZON
/// (`crate::agent_kind::CaptureWindowBounds::horizon`) has passed, a lone
/// match is only `Provisional`: a rival record, or a rival session's first
/// input, can still arrive inside the window and make it ambiguous, and
/// this pass re-derives the verdict from scratch every time so that flip
/// actually happens. Only at or after the horizon, and only from a
/// COMPLETE scan, is a claim written — because an incomplete scan's unseen
/// record is precisely the one that would have forced a bail.
///
/// ## The ambiguity rule, mechanically
///
/// Windows are compared across ALL sessions of the same kind in the same
/// canonical working directory that have ever taken input — captured,
/// ambiguous, or still waiting — because a record landing in a shared span
/// could honestly belong to any of them. Any overlap poisons the session
/// being evaluated, permanently for this launch. Only after that does the
/// scan run, and a session with more than one in-window candidate bails
/// too. Both bails are logged: SPEC.md owes the user an explanation for
/// the fallback they are about to be offered.
///
/// ## Writes
///
/// `may_write` gates the DURABLE half only. A supervisor that may not
/// record (no state-directory claim, or a failed boot-id read — see
/// `Supervisor::may_record`) still correlates, still refuses ambiguously,
/// and still reports what it found; it simply never leaves a session in a
/// state that claims durability it does not have. Every durable write here
/// has a retry state, so a failed one costs a poll interval rather than
/// the capture.
pub(crate) async fn capture_pass(sup: &Supervisor, entries: &[Arc<SessionEntry>], may_write: bool) {
    let bounds = sup.capture_window;
    let now = crate::agent_kind::now_unix();
    // Deliberately AHEAD of the agent-home bail below. The tripwire is a
    // statement about a hook that did not report, which is a fact about
    // the session rather than about any file on disk, and a supervisor
    // with no home to scan is precisely the configuration where nothing
    // else would ever mention it. Production never reaches this function
    // without a home — `capture_pass_for` bails first — so that
    // configuration's tripwire is run by that bail instead; this call is
    // what serves everything else, including the tests that drive
    // `capture_pass` directly.
    report_liveness_tripwire(entries, bounds, now);
    let Some(home) = sup.agent_home.as_deref() else {
        return;
    };

    // Retry the durable writes that failed earlier, off the input path.
    //
    // A pending claim is re-checked against the reported set before it is
    // committed, because the pass that produced it could not have seen a
    // report that landed afterwards.
    for entry in entries {
        let pending_first_input = {
            let first = entry
                .first_input
                .lock()
                .expect("first-input mutex poisoned");
            (!first.durable).then_some(first.at).flatten()
        };
        if may_write && let Some(at) = pending_first_input {
            persist_first_input(sup, entry, at).await;
        }
        let pending = entry
            .capture
            .lock()
            .expect("capture mutex poisoned")
            .clone();
        match pending {
            CaptureState::PendingCommit {
                conversation,
                record,
                stamp,
            } => {
                // A rival in this session's group has since been TOLD that
                // this conversation is its own, so this pending claim is
                // now known to be about someone else's record. Committing
                // it would be exactly the wrong-conversation claim the
                // design exists to exclude.
                //
                // `UncapturedFinal` is not a consolation prize; it is the
                // verdict the pass below would have reached had the report
                // been visible when the claim was computed. A pending claim
                // only exists past the horizon and off a COMPLETE scan, so
                // "the one in-window candidate belongs to someone else"
                // means this session's evidence is in and there is none of
                // it — which is that state's definition. Filtering the
                // candidate out in the verdict loop yields `NotYet` and
                // lands on the same state by the same reasoning.
                //
                // Checked outside `may_write`, unlike the commit it
                // replaces: refusing to claim someone else's identity is a
                // decision, not a durable write, and a supervisor without a
                // recording claim owes the same refusal.
                //
                // Collected HERE, per entry, rather than once before the
                // loop: this loop awaits a store write for every session it
                // commits, so a set gathered before the first iteration is
                // already stale by the time the tenth session's decision is
                // made — and the report that landed in between is exactly
                // the one that would have stopped this commit.
                // `reported_ids` is an in-memory walk over entries, so
                // paying for it per pending claim is cheaper than the write
                // it may avoid. The window it leaves is the pure compute
                // between this read and this session's own commit below,
                // and nothing wider.
                let spoken_for_at_retry = reported_ids(entries);
                if is_spoken_for(&spoken_for_at_retry, entry, &conversation) {
                    let dropped = entry
                        .capture
                        .lock()
                        .expect("capture mutex poisoned")
                        .advance(CaptureState::UncapturedFinal);
                    if dropped {
                        info!(
                            session = %entry.info.id, conversation = %conversation,
                            "another session in this directory reported this conversation as \
                             its own before the pending claim could be written; nothing is \
                             claimed for this session"
                        );
                    }
                } else if may_write {
                    commit_capture(sup, entry, conversation, record, stamp).await;
                }
            }
            CaptureState::Ambiguous { durable: false } if may_write => {
                persist_ambiguity(sup, entry).await;
            }
            _ => {}
        }
    }

    // Every (kind, canonical cwd) group's windows, including sessions that
    // have already captured or already bailed: they still occupy their
    // span, and a newcomer overlapping one of them is just as ambiguous as
    // two newcomers overlapping each other. Grouping by KIND as well as by
    // directory is not incidental — a Claude record can only ever be a
    // Claude session's, so a Codex session in the same directory cannot
    // poison it.
    //
    // Keyed on the kind's stable column spelling rather than the wire enum
    // only because `AgentKind` is not `Hash` (it is protocol vocabulary,
    // and this PR does not touch the protocol crate); the mapping is
    // injective, so the grouping is exactly the same.
    //
    // A session holding `Reported` is emphatically still in here even
    // though nothing below will scan for it. Dropping it would be an
    // outright regression: a rival with an overlapping window in the same
    // directory would stop bailing, and if the rival's own record has not
    // appeared yet, the reported session's record is the lone candidate
    // and the rival commits somebody else's conversation — the
    // wrong-conversation claim the whole design exists to exclude.
    let mut occupied: HashMap<(&'static str, &str), Vec<(&str, CaptureWindow)>> = HashMap::new();
    for entry in entries {
        let (Some(_), Some(cwd)) = (entry.snapshot.integration(), entry.canonical_cwd.as_deref())
        else {
            continue;
        };
        let key = (crate::store::agent_kind_column(entry.snapshot.kind), cwd);
        let Some(at) = entry
            .first_input
            .lock()
            .expect("first-input mutex poisoned")
            .at
        else {
            continue;
        };
        occupied
            .entry(key)
            .or_default()
            .push((entry.info.id.as_str(), CaptureWindow::around(at, bounds)));
    }

    // Who will actually consult a scan this pass. Decided BEFORE any
    // scanning so each root's mtime floor derives only from the sessions
    // that will call `choose` — a settled session's window must not drag a
    // floor backwards and make every scan read history nothing will look
    // at.
    struct Scanning<'a> {
        entry: &'a Arc<SessionEntry>,
        integration: &'static dyn crate::agent_kind::AgentIntegration,
        root: PathBuf,
        cwd: &'a str,
        window: CaptureWindow,
        /// Carried rather than re-derived from `window`: the horizon is a
        /// function of the first-input time, and reconstructing that from
        /// the window's end would silently break the moment either bound
        /// changed shape.
        first_input_at: i64,
    }
    let mut scanning: Vec<Scanning<'_>> = Vec::new();
    for entry in entries {
        let (Some(integration), Some(cwd)) =
            (entry.snapshot.integration(), entry.canonical_cwd.as_deref())
        else {
            continue;
        };
        {
            let state = entry.capture.lock().expect("capture mutex poisoned");
            if state.is_settled() || matches!(*state, CaptureState::PendingCommit { .. }) {
                continue;
            }
        }
        let Some(at) = entry
            .first_input
            .lock()
            .expect("first-input mutex poisoned")
            .at
        else {
            // No delivered input yet, so no record can exist for this
            // session — and, crucially, no deadline is running either. The
            // launch-to-first-input gap is unbounded by design.
            continue;
        };
        let window = CaptureWindow::around(at, bounds);
        let key = (crate::store::agent_kind_column(entry.snapshot.kind), cwd);
        if let Some(rival) = occupied.get(&key).and_then(|group| {
            group
                .iter()
                .find(|(id, other)| *id != entry.info.id && other.overlaps(&window))
        }) {
            declare_ambiguous(sup, entry, may_write, || {
                let reason = overlapping_windows_reason(&entry.info.id, rival.0, cwd);
                warn!(session = %entry.info.id, rival = %rival.0, cwd = %cwd, "{reason}");
            })
            .await;
            continue;
        }
        scanning.push(Scanning {
            entry,
            integration,
            root: integration.record_root(home, cwd),
            cwd,
            window,
            first_input_at: at,
        });
    }

    // One scan per ROOT, shared by every session that consults it. Claude's
    // root is per munged directory (so two cwds that munge alike share
    // one), and Codex's is the whole host's rollout tree — keying on the
    // path itself is what makes both cases fall out without a special case.
    let mut floors: HashMap<&Path, i64> = HashMap::new();
    for scan in &scanning {
        let floor = floors
            .entry(scan.root.as_path())
            .or_insert(scan.window.start);
        *floor = (*floor).min(scan.window.start);
    }
    let mut scanned: HashMap<PathBuf, crate::agent_kind::ScanOutcome> = HashMap::new();
    for (root, floor) in floors {
        let integration = scanning
            .iter()
            .find(|scan| scan.root == root)
            .expect("every floor came from a scanning session's root")
            .integration;
        scanned.insert(
            root.to_path_buf(),
            crate::agent_kind::scan_records(integration, root, floor).await,
        );
    }

    for scan in scanning {
        // Read at the top of EVERY iteration, after the scan awaits have
        // completed, and not back in the `occupied` walk where an earlier
        // version gathered it. Two staleness sources motivate the
        // placement. The scans are the long await in this function, so a
        // set snapshotted before them misses every report that landed
        // during them. And this loop itself awaits a store write for each
        // session it commits, so even a set read once after the scans ages
        // across the sessions that follow. Either way the verdict would
        // hand a rival a conversation the supervisor had already been told
        // belongs to somebody else. `reported_ids` is an in-memory walk, so
        // re-collecting per session costs nothing worth saving.
        //
        // The residual race, stated honestly rather than papered over: this
        // is a read, not a lock, so it is not atomic with this iteration's
        // own commit. A report landing between this line and that write is
        // caught by nothing on the rival's side — the store's
        // `conversation_source` fences protect only the REPORTED session's
        // own row, not a rival's. What remains open is exactly the pure
        // compute between this read and this session's write, and nothing
        // beyond it. It is also the same window that has always existed for
        // records: a record file appearing after the scan read its
        // directory is equally invisible to the verdict built from it.
        // Reports did not introduce this class of race; closing it would
        // mean serializing the report handler against capture passes, which
        // the plan's timing note deliberately does not do.
        let spoken_for_at_verdict = reported_ids(entries);
        let outcome = scanned
            .get(&scan.root)
            .expect("every scanning session's root was scanned");
        // The recorded cwd FIELD, not the directory the record was found
        // in: the munging is non-injective, and Codex does not partition
        // by directory at all.
        //
        // Then drop every record whose conversation id another session in
        // this (kind, cwd) group has been TOLD is its own. Placed here,
        // ahead of `choose`, because `choose` is where the lone-candidate
        // decision is made: filtering afterwards could only reject a
        // verdict, whereas filtering the input can turn a two-candidate
        // bail into an honest single match on the one record that is still
        // unspoken for. This session's own reported id can never appear in
        // the list — a `Reported` session is settled and so never reaches
        // the scanning set at all — so no self-exclusion is needed.
        let spoken_for = spoken_for_at_verdict.get(&(
            crate::store::agent_kind_column(scan.entry.snapshot.kind),
            scan.cwd,
        ));
        let mine: Vec<&crate::agent_kind::Candidate> = outcome
            .candidates
            .iter()
            .filter(|candidate| candidate.correlators.cwd == scan.cwd)
            .filter(|candidate| {
                !spoken_for.is_some_and(|ids| ids.contains(&candidate.correlators.conversation))
            })
            .collect();
        // Past the horizon, this session's evidence is in: its window has
        // closed and anything written inside it has had the publication
        // grace to become readable. Only then may a claim be committed.
        let settled = now >= bounds.horizon(scan.first_input_at);
        match crate::agent_kind::choose(&mine, scan.window) {
            CaptureVerdict::Ambiguous(why) => {
                declare_ambiguous(sup, scan.entry, may_write, || {
                    warn!(session = %scan.entry.info.id, cwd = %scan.cwd, "{why}");
                })
                .await;
            }
            CaptureVerdict::Captured(conversation) => {
                if settled && outcome.complete {
                    let claimed = mine
                        .iter()
                        .find(|c| c.correlators.conversation == conversation)
                        .expect("the chosen conversation came from this candidate list");
                    let (record, stamp) = (claimed.path.clone(), claimed.stamp);
                    let advanced = {
                        let mut state = scan.entry.capture.lock().expect("capture mutex poisoned");
                        state.advance(CaptureState::PendingCommit {
                            conversation: conversation.clone(),
                            record: record.clone(),
                            stamp,
                        })
                    };
                    if advanced && may_write {
                        commit_capture(sup, scan.entry, conversation, record, stamp).await;
                    }
                } else {
                    // A working hypothesis only. Nothing is written, and
                    // `Resume` is not advertised, because the window is
                    // still open (or the scan could not see everything).
                    // The id is carried so a later pass can tell a stable
                    // hypothesis from one that changed under it, which is
                    // worth a log line: a provisional match that moves is
                    // the shape a second agent in the directory produces
                    // just before the ambiguity bail catches it.
                    let mut state = scan.entry.capture.lock().expect("capture mutex poisoned");
                    if let CaptureState::Provisional { conversation: was } = &*state
                        && was != &conversation
                    {
                        warn!(
                            session = %scan.entry.info.id, was = %was, now = %conversation,
                            "the provisional conversation match for this session changed \
                             before its capture window closed; nothing is claimed until the \
                             window settles"
                        );
                    }
                    state.advance(CaptureState::Provisional { conversation });
                }
            }
            CaptureVerdict::NotYet => {
                if settled && outcome.complete {
                    // The evidence is in and there is none. Leaving the
                    // eligible set here is what keeps a session that never
                    // wrote a record from rescanning its directory on
                    // every poll for the rest of its life.
                    scan.entry
                        .capture
                        .lock()
                        .expect("capture mutex poisoned")
                        .advance(CaptureState::UncapturedFinal);
                }
            }
        }
    }

    // Re-verification last, and only for sessions holding a SCAN-derived
    // committed identity: an append is a confirmation signal, not a claim,
    // so it neither needs nor may use the scan above. `Reported` is
    // excluded by construction — the match below names `Captured` only —
    // because there is nothing for a file to confirm about an answer the
    // agent gave directly, and re-reading a record could at best agree.
    for entry in entries {
        let Some(integration) = entry.snapshot.integration() else {
            continue;
        };
        let state = entry
            .capture
            .lock()
            .expect("capture mutex poisoned")
            .clone();
        if let CaptureState::Captured {
            conversation,
            record,
            stamp,
        } = state
        {
            reverify_capture(entry, integration, &conversation, &record, stamp).await;
        }
    }
}

/// Warn once per launch about a session that was launched WITH a
/// conversation-reporting hook and whose report was never successfully
/// ACCEPTED.
///
/// That is deliberately broader than "the hook never ran", and the message
/// is worded to match. The state this reads cannot distinguish a hook that
/// was never invoked from one that ran and was refused — a credential the
/// store could not validate, an implausible id, a supervisor that was not
/// recording — because all three leave the session un-`Reported` in exactly
/// the same way. What separates them is the hook's own trace file, which is
/// where a diagnosis continues after this line points at a session.
///
/// This is the only way that failure is ever visible at all. A hook that
/// does not run — a vendor that renamed its flag, a settings file the
/// injection declined to fight over, a wrapper script that dropped the
/// appended tail — costs nothing observable: the scan fallback keeps
/// working, the session looks entirely ordinary, and the resume offer it
/// produces is the same slightly-less-certain one farhelm made before hooks
/// existed. Without a tripwire the mechanism could silently stop working
/// across a vendor release and nobody would learn of it from anything but a
/// wrong-conversation resume months later.
///
/// The horizon is `first input + after + grace` — the scan's own settling
/// point, reused rather than given a constant of its own. It is late
/// enough for both vendors from opposite directions: Claude reports at
/// process startup, so a Claude session is normally `Reported` before it
/// has an anchor at all, while Codex reports at the first prompt, which is
/// the very event that sets the anchor. A session with no first input yet
/// is therefore never warned about — the launch-to-first-prompt gap is
/// unbounded by design, and a user who has not typed anything has not yet
/// given a Codex hook anything to fire on.
///
/// Pure and write-free: it takes no store, honours no `may_write`, and
/// touches nothing but the two diagnostic flags. A supervisor that may not
/// record may still say what it sees — and, for the same reason, so may one
/// with no agent home to scan, which is why `capture_pass_for` runs this on
/// the path where it bails before ever entering a pass.
///
/// ## Why the decision is made under the capture lock
///
/// The check and the latch have to be one step. A report can land at any
/// moment — the handler is not serialized with capture passes — so a
/// sequence of "read the state, drop the lock, latch and warn" can warn
/// about a session that became `Reported` in between, and then set the
/// once-only latch so the truthful non-warning is never reconsidered
/// either. Reading the state, testing the latch, and setting it all inside
/// one hold of `entry.capture` makes the decision consistent with the state
/// it was made from. It also serializes this function against another
/// concurrent copy of itself, which matters on the homeless path above
/// where no pass lock is held. The `warn!` itself stays outside: emitting a
/// log line while holding a lock every capture pass wants is a needless
/// coupling, and by then the decision is already made.
///
/// That lock covers the read-then-latch step, not the report handler's
/// two-part landing: a report that has already written durably but has not
/// yet advanced the in-memory state can still be warned about in that
/// instant. Accepted rather than coordinated, because the latch bounds the
/// cost at a single spurious line per launch, and the alternative is
/// serializing the report path against every capture pass to buy nothing
/// but a tidier log.
///
/// Returns how many warnings this call actually emitted. Production ignores
/// it; it exists so "once per launch" is an ordinary assertion rather than a
/// tracing-subscriber fixture. The latch alone cannot carry that test — an
/// already-latched entry and one that just re-warned look identical from
/// outside.
fn report_liveness_tripwire(
    entries: &[Arc<SessionEntry>],
    bounds: CaptureWindowBounds,
    now: i64,
) -> usize {
    let ordering = std::sync::atomic::Ordering::Relaxed;
    let mut warned = 0;
    for entry in entries {
        // `hooked` is read outside the lock deliberately: it only ever goes
        // false-to-true, at publication, so the worst a stale read costs is
        // deferring the warning to the next pass.
        if !entry.hooked.load(ordering) {
            continue;
        }
        let Some(at) = entry
            .first_input
            .lock()
            .expect("first-input mutex poisoned")
            .at
        else {
            continue;
        };
        if now < bounds.horizon(at) {
            continue;
        }
        let warn_now = {
            let state = entry.capture.lock().expect("capture mutex poisoned");
            let silent = !matches!(*state, CaptureState::Reported { .. });
            let first = silent && !entry.hook_warned.load(ordering);
            if first {
                entry.hook_warned.store(true, ordering);
            }
            first
        };
        if warn_now {
            warned += 1;
            warn!(
                session = %entry.info.id,
                "this session was launched with a conversation hook but none has reported"
            );
        }
    }
    warned
}

/// Commit one settled claim and, if the store confirms it, advertise it.
///
/// The DURABLE write decides what is claimed, not this process's
/// intention: the column is write-once and loses to a recorded ambiguity,
/// so a value already there (another pass, or a supervisor that ran before
/// this one) wins and the mirror follows it. Same rule `Supervisor::record`
/// applies to outcomes.
///
/// A failure leaves the session in `PendingCommit`, which the next pass
/// retries — and, crucially, does NOT advertise `Resume`, since there is
/// nothing stored for a restart to fill in.
async fn commit_capture(
    sup: &Supervisor,
    entry: &Arc<SessionEntry>,
    conversation: String,
    record: PathBuf,
    stamp: RecordStamp,
) {
    if let Some(fault) = &sup.seams.capture_store_fault
        && let Err(e) = fault(CaptureWrite::Conversation, &entry.info.id)
    {
        warn!(
            session = %entry.info.id, error = %format!("{e:#}"),
            "injected failure while committing a captured conversation identity"
        );
        return;
    }
    let committed = match sup
        .store
        .record_captured_conversation(&entry.info.id, entry.generation, &conversation, &record)
        .await
    {
        Ok(Some(committed)) => committed,
        // Either the row is gone (a concurrent delete) or the claim lost
        // to a recorded ambiguity. Neither may advertise Resume, and
        // neither is repairable here.
        Ok(None) => return,
        Err(e) => {
            warn!(
                session = %entry.info.id, error = %format!("{e:#}"),
                "could not record this session's captured conversation identity; \
                 the next capture pass retries the write"
            );
            return;
        }
    };
    // The record path is only trustworthy when the committed identity is
    // the one this pass chose. If another writer got there first with a
    // different answer, the file this pass found is not that identity's
    // record — an empty path is the "identity held, record not located"
    // state, which re-verification repairs on its next pass.
    let (record, stamp) = if committed == conversation {
        (record, stamp)
    } else {
        (
            PathBuf::new(),
            RecordStamp {
                len: 0,
                mtime_unix: None,
            },
        )
    };
    info!(
        session = %entry.info.id, conversation = %committed,
        "captured this session's agent conversation identity"
    );
    entry
        .capture
        .lock()
        .expect("capture mutex poisoned")
        .advance(CaptureState::Captured {
            conversation: committed,
            record,
            stamp,
        });
}

/// Refuse to claim anything for this session, for the rest of this launch,
/// and record that refusal durably.
///
/// The in-memory state moves FIRST: ambiguity dominates every other
/// scan-derived state, and a supervisor that cannot write (or whose write
/// fails) must still refuse rather than keep hunting for a claim it has
/// already decided it may not make. `durable` then tracks whether the
/// refusal survived the process, and `capture_pass` retries until it has.
///
/// The one state that REFUSES the advance is `Reported`, and when it does,
/// nothing is persisted either. There are two independent guards against a
/// pass erasing a report — this skip, and `record_capture_ambiguous`'s
/// `conversation_source IS NULL` fence in the store — and they are not
/// redundant, because they protect different things. They have to be
/// separate: the report handler is not serialized with capture passes at
/// all. Passes serialize among themselves through `CaptureCoordination`;
/// the handler takes NEITHER that lock nor the session's lifecycle claim —
/// see `Supervisor::report_conversation` for why a hook with a 2 s budget
/// cannot be made to wait behind a restart. What a report does hold is the
/// entry's own capture-state lock, for the moment it swaps the state, and
/// the store's generation fence, which decides whether its row write lands
/// at all. Neither of those orders it against a pass's DECISION, so a pass
/// can compute an ambiguity, have a report land underneath it, and then try
/// to write. The SQL fence is what makes the durable row right in that
/// race. This
/// skip is what keeps the log honest: without it the supervisor would emit
/// a "recorded an ambiguous correlation" line for a session it did not
/// mark ambiguous and is still, correctly, advertising a resume for.
///
/// `explain` is the caller's own account of WHY the correlation is
/// ambiguous — the log line SPEC.md owes the user for the fallback they are
/// about to be offered. It is a callback rather than a message argument for
/// one reason: it must run only once the advance has been ACCEPTED. The
/// refusal above is exactly the case where the explanation would be a lie,
/// and a caller that logged before calling here would emit "these windows
/// overlap, so nothing is claimed" about a session that is at that moment
/// happily advertising the resume its agent reported.
async fn declare_ambiguous(
    sup: &Supervisor,
    entry: &Arc<SessionEntry>,
    may_write: bool,
    explain: impl FnOnce(),
) {
    let advanced = entry
        .capture
        .lock()
        .expect("capture mutex poisoned")
        .advance(CaptureState::Ambiguous { durable: false });
    if !advanced {
        return;
    }
    explain();
    if may_write {
        persist_ambiguity(sup, entry).await;
    }
}

/// Store this session's ambiguity verdict and mark the mirror durable.
/// Shared by the decision itself and by `capture_pass`'s retry.
async fn persist_ambiguity(sup: &Supervisor, entry: &Arc<SessionEntry>) {
    if let Some(fault) = &sup.seams.capture_store_fault
        && let Err(e) = fault(CaptureWrite::Ambiguity, &entry.info.id)
    {
        warn!(
            session = %entry.info.id, error = %format!("{e:#}"),
            "injected failure while recording an ambiguous correlation"
        );
        return;
    }
    match sup
        .store
        .record_capture_ambiguous(&entry.info.id, entry.generation)
        .await
    {
        Ok(()) => {
            entry
                .capture
                .lock()
                .expect("capture mutex poisoned")
                .advance(CaptureState::Ambiguous { durable: true });
        }
        Err(e) => warn!(
            session = %entry.info.id, error = %format!("{e:#}"),
            "could not record this session's ambiguous conversation correlation; \
             it is refused for this supervisor's lifetime and the next pass retries the write"
        ),
    }
}

/// Why two sessions in one working directory poisoned each other, in
/// words.
///
/// A named function rather than an inline format string because SPEC.md
/// owes the user an explanation whenever it offers the fallback instead of
/// a resume, which makes this message part of the contract rather than
/// debug output. Its sibling, the two-records-in-one-window case, is
/// `agent_kind::choose`'s own `Ambiguous` payload.
pub(super) fn overlapping_windows_reason(session: &str, rival: &str, cwd: &str) -> String {
    format!(
        "sessions {session} and {rival} took their first input close enough together in {cwd} \
         that a conversation record could honestly belong to either; neither will have its \
         conversation identity captured for this launch"
    )
}

/// Re-verify one already-claimed identity against the record it came from
/// — SPEC_impl.md's "the watcher treats appends as the resume signal and
/// cheaply re-verifies identity after each restart".
///
/// Two facts shape every branch here. First, a plain resume APPENDS under
/// the same id, so a changed stamp is a confirmation opportunity, not a
/// new conversation. Second, a new id appears only on an explicit fork,
/// which writes a DIFFERENT file — so a fork is never seen through this
/// path at all, and the original identity is retained by construction
/// rather than by a rule that has to be remembered. (The one place a fork
/// COULD be seen is the relocation branch below, which is why that branch
/// matches on the claimed id rather than taking whatever it finds.)
///
/// Nothing here can ever un-claim an identity. A vanished, unreadable, or
/// disagreeing record is logged and the claim stands: trading a resumable
/// session for an unresumable one on the strength of a missing file would
/// lose real user value for no gain in honesty, since the durable column
/// already records what was observed when it was observed.
async fn reverify_capture(
    entry: &SessionEntry,
    integration: &dyn crate::agent_kind::AgentIntegration,
    conversation: &str,
    record: &Path,
    stamp: RecordStamp,
) {
    // An empty path is a claim whose record this process has never
    // located: the durable locator hint was missing (a row written before
    // the hint existed) or pointed nowhere. There is nothing to verify
    // against, and re-scanning the directory to find it would make startup
    // cost multiplicative in captured sessions for no gain — the identity
    // is already durable and already correct.
    if record.as_os_str().is_empty() {
        return;
    }
    let current = match crate::agent_kind::stamp_of(record).await {
        Ok(Some(current)) => current,
        Ok(None) => {
            warn!(
                session = %entry.info.id, claimed = %conversation,
                record = %record.display(),
                "this session's conversation record is gone; keeping the identity captured \
                 earlier, since the record's absence says nothing about which conversation ran"
            );
            return;
        }
        Err(e) => {
            warn!(
                session = %entry.info.id, claimed = %conversation,
                error = %format!("{e:#}"),
                "could not stat this session's conversation record; keeping the identity \
                 captured earlier"
            );
            return;
        }
    };
    if !current.differs(&stamp) {
        return;
    }
    match crate::agent_kind::read_record(record, integration).await {
        Ok(Some((correlators, stamp))) if correlators.conversation == conversation => {
            entry
                .capture
                .lock()
                .expect("capture mutex poisoned")
                .advance(CaptureState::Captured {
                    conversation: conversation.to_string(),
                    record: record.to_path_buf(),
                    stamp,
                });
        }
        Ok(Some((correlators, _))) => warn!(
            session = %entry.info.id, claimed = %conversation,
            found = %correlators.conversation,
            "this session's conversation record now names a different conversation; \
             keeping the identity captured earlier rather than adopting the new one"
        ),
        Ok(None) => warn!(
            session = %entry.info.id, claimed = %conversation,
            record = %record.display(),
            "this session's conversation record vanished or stopped being recognizable; \
             keeping the identity captured earlier"
        ),
        Err(e) => warn!(
            session = %entry.info.id, claimed = %conversation,
            error = %format!("{e:#}"),
            "could not re-read this session's conversation record; keeping the identity \
             captured earlier"
        ),
    }
}

/// The claim ladder and the pass's two report-aware behaviours: the
/// candidate exclusion a report buys the session's rivals, and the refusal
/// to record an ambiguity a report has already settled.
///
/// The ladder half is pure and needs no fixture. The pass half plants real
/// record files under a temporary agent home and calls `capture_pass`
/// directly with hand-built entries: the subject is what one pass concludes
/// from a given arrangement of state and files, and going through
/// `create_session` would add a launch, a tmux pane, and a scheduling
/// decision to a test that is about none of those.
#[cfg(test)]
mod tests {
    use super::super::core::tests::{StateDir, dummy_exe, entry_with};
    use super::super::core::{SupervisorSeams, SupervisorTimeouts};
    use super::*;
    use crate::agent_kind::{CaptureWindowBounds, IntegrationSnapshot};
    use farhelm_proto::AgentKind;
    use std::sync::Mutex as StdMutex;

    /// A window short enough that a test never waits out a production
    /// interval, with the horizon it implies (`after` + `grace`) sitting
    /// only a couple of seconds past first input.
    fn fast_bounds() -> CaptureWindowBounds {
        CaptureWindowBounds::new(
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(1),
        )
    }

    /// A Claude-kind entry carrying the correlators a capture pass reads,
    /// and nothing else it needs.
    ///
    /// Built by hand rather than through `create_session` because
    /// `capture_pass` takes entries directly: it never consults the
    /// supervisor's session map, so a test can present exactly the
    /// arrangement of states and windows it wants to reason about without
    /// launching anything.
    fn claude_entry(
        id: &str,
        cwd: &str,
        first_input_at: Option<i64>,
        capture: CaptureState,
    ) -> Arc<SessionEntry> {
        entry_of_kind(AgentKind::Claude, id, cwd, first_input_at, capture)
    }

    /// [`claude_entry`] for any integrated kind, which the group-isolation
    /// tests need: correlation groups on `(kind, canonical cwd)`, and the
    /// only way to show the kind half is load-bearing is to put a session of
    /// a DIFFERENT kind in the same directory.
    fn entry_of_kind(
        kind: AgentKind,
        id: &str,
        cwd: &str,
        first_input_at: Option<i64>,
        capture: CaptureState,
    ) -> Arc<SessionEntry> {
        let mut entry = entry_with(None, crate::store::LastOutcome::Running);
        entry.info.id = id.to_string();
        entry.snapshot = IntegrationSnapshot {
            kind,
            resume_template: None,
        };
        entry.canonical_cwd = Some(cwd.to_string());
        entry.first_input = Arc::new(std::sync::Mutex::new(FirstInput {
            at: first_input_at,
            durable: true,
        }));
        entry.capture = Arc::new(std::sync::Mutex::new(capture));
        Arc::new(entry)
    }

    /// Plant the JSONL record Claude would have written for `conversation`
    /// in `cwd`, timestamped `at`.
    ///
    /// One `user` line is all the correlator parse needs; the point of
    /// planting rather than running an agent is that a capture pass reads
    /// files, and the files are the whole input under test.
    fn plant_claude_record(home: &Path, cwd: &str, conversation: &str, at: i64) {
        let project = home
            .join(".claude")
            .join("projects")
            .join(crate::agent_kind::munge_cwd(cwd));
        std::fs::create_dir_all(&project).expect("record directory");
        let line = serde_json::json!({
            "type": "user",
            "sessionId": conversation,
            "cwd": cwd,
            "timestamp": crate::agent_kind::format_rfc3339(at),
        });
        std::fs::write(
            project.join(format!("{conversation}.jsonl")),
            format!("{line}\n"),
        )
        .expect("plant the record");
    }

    /// A supervisor pointed at `home` for agent records, with the fast
    /// window and whatever store fault the caller wants.
    async fn supervisor_over(
        state: &StateDir,
        home: &Path,
        fault: Option<CaptureStoreFault>,
    ) -> Arc<Supervisor> {
        Supervisor::new_with_seams(
            state.path(),
            dummy_exe(),
            SupervisorTimeouts::default(),
            SupervisorSeams {
                agent_home: Some(home.to_path_buf()),
                capture_window: fast_bounds(),
                capture_store_fault: fault,
                ..SupervisorSeams::default()
            },
        )
        .await
        .expect("supervisor")
    }

    fn reported(conversation: &str) -> CaptureState {
        CaptureState::Reported {
            conversation: conversation.to_string(),
        }
    }

    /// Every state the SCAN can reach, for the ladder tests that have to
    /// assert over all of them rather than over a representative one.
    fn scan_derived_states() -> Vec<CaptureState> {
        vec![
            CaptureState::Unclaimed,
            CaptureState::Provisional {
                conversation: "conv-prov".to_string(),
            },
            CaptureState::PendingCommit {
                conversation: "conv-pending".to_string(),
                record: PathBuf::from("/tmp/pending.jsonl"),
                stamp: RecordStamp {
                    len: 0,
                    mtime_unix: None,
                },
            },
            CaptureState::UncapturedFinal,
            CaptureState::Captured {
                conversation: "conv-scan".to_string(),
                record: PathBuf::from("/tmp/scan.jsonl"),
                stamp: RecordStamp {
                    len: 0,
                    mtime_unix: None,
                },
            },
            CaptureState::Ambiguous { durable: false },
            CaptureState::Ambiguous { durable: true },
        ]
    }

    /// The one weakening this design makes to the existing ladder, pinned
    /// across every state below it: a report displaces every scan-derived
    /// verdict, including the `Ambiguous` that used to be dominant over
    /// everything.
    ///
    /// That dominance existed because scan evidence could not be trusted —
    /// a pass that saw only one of two candidates would claim on strictly
    /// worse evidence than the pass that bailed. A report is not scan
    /// evidence at all; it is the agent naming its own conversation.
    /// Refusing it would leave a session permanently unresumable because
    /// two agents once shared a directory, which is a worse answer than the
    /// exact one now in hand.
    ///
    /// `durable: true` is in the list deliberately: that is the state a
    /// RELOADED ambiguity comes back as, so a comparison that happened to
    /// work only for the in-flight flag would leave every restarted session
    /// unreportable.
    #[test]
    fn a_report_displaces_every_scan_derived_verdict() {
        for scanned in scan_derived_states() {
            let mut state = scanned.clone();
            assert!(
                state.advance(reported("conv-hook")),
                "a report must land over {scanned:?}"
            );
            assert!(matches!(
                &state,
                CaptureState::Reported { conversation } if conversation == "conv-hook"
            ));
        }
    }

    /// `Reported` replaces `Reported`, and nothing else does.
    ///
    /// The same-rank replacement is the feature rather than a hole in the
    /// ladder: `/clear` and `/new` mint a new conversation id inside a
    /// running agent, and the previous id is then exactly the one that must
    /// not be resumed. Refusing the second report would leave farhelm
    /// offering to resume a conversation the user has already discarded —
    /// the bug this state exists to fix.
    ///
    /// The other direction matters just as much, and is why every variant
    /// is tried rather than a representative one: once the agent has
    /// spoken, no amount of later scanning may talk the supervisor out of
    /// it. A pass that found the old record, or two records, or none, must
    /// leave the reported identity exactly where it is.
    #[test]
    fn only_another_report_may_replace_a_report() {
        let mut state = reported("conv-first");
        assert!(
            state.advance(reported("conv-second")),
            "a second report is a replacement, not a regression"
        );
        assert!(matches!(
            &state,
            CaptureState::Reported { conversation } if conversation == "conv-second"
        ));

        for scanned in scan_derived_states() {
            let mut state = reported("conv-second");
            assert!(
                !state.advance(scanned.clone()),
                "{scanned:?} must not displace a reported identity"
            );
            assert!(
                matches!(
                    &state,
                    CaptureState::Reported { conversation } if conversation == "conv-second"
                ),
                "and the refusal must leave the reported identity untouched"
            );
        }
    }

    /// A reported identity is advertised to the user and takes the session
    /// out of the eligible set.
    ///
    /// Both predicates are contracts other code reads rather than
    /// conveniences. `committed_conversation` is what
    /// `farhelm_proto::RestartOffer::Resume` is computed from, so a
    /// `Reported` that answered `None` would have collected the exact id
    /// the user wants and then offered them a fresh launch instead.
    /// `is_settled` is what keeps the pass from scanning for a session
    /// whose identity is already known — nothing on disk can improve on the
    /// agent's own answer, and re-verification is excluded for the same
    /// reason.
    #[test]
    fn a_reported_identity_is_advertised_and_settled() {
        let state = reported("conv-hook");
        assert_eq!(state.committed_conversation(), Some("conv-hook"));
        assert!(state.is_settled());
    }

    /// A pass that computed an ambiguity for a session which has since been
    /// reported writes NOTHING.
    ///
    /// The race is real rather than theoretical: capture passes serialize
    /// among themselves through `CaptureCoordination`, but the report
    /// handler does not join that queue, so a report can land between a
    /// pass deciding "ambiguous" and the pass persisting it. The store's
    /// `conversation_source IS NULL` fence is what makes the ROW right in
    /// that race; this skip is what keeps the supervisor from announcing it
    /// recorded a refusal for a session it is still, correctly, advertising
    /// a resume for.
    ///
    /// Proven through the fault seam rather than by reading the row back,
    /// because "no write was attempted" is the property — a test that
    /// checked only the final row could not tell a skipped write from one
    /// the SQL fence rejected, and that other half of the belt and braces
    /// is pinned in `store.rs` instead.
    ///
    /// The "nor logged" half is checked by standing in for the log: the
    /// explanation callback records that it ran, which is as close as a
    /// unit test can get to asserting on a `tracing` line without a
    /// subscriber. That the explanation is a callback at all exists for
    /// exactly this case — a caller that logged before delegating here
    /// would describe a refusal that never happened.
    #[tokio::test]
    async fn an_ambiguity_is_neither_recorded_nor_logged_for_a_reported_session() {
        let state = StateDir::new();
        let home = tempfile::tempdir().expect("agent home");
        let attempts: Arc<StdMutex<Vec<CaptureWrite>>> = Arc::new(StdMutex::new(Vec::new()));
        let seen = Arc::clone(&attempts);
        // Fails whatever it is asked about, so an attempted write is
        // recorded here and never reaches the store either way.
        let fault: CaptureStoreFault = Arc::new(move |write, _id| {
            seen.lock().expect("fault log poisoned").push(write);
            Err(anyhow::anyhow!("no write should have been attempted"))
        });
        let sup = supervisor_over(&state, home.path(), Some(fault)).await;

        let entry = claude_entry("reported-session", "/tmp/work", None, reported("conv-hook"));
        let explained = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let ran = Arc::clone(&explained);
        declare_ambiguous(&sup, &entry, true, || {
            ran.store(true, std::sync::atomic::Ordering::Relaxed);
        })
        .await;

        assert!(
            attempts.lock().expect("fault log poisoned").is_empty(),
            "a refused advance must not reach the durable ambiguity write"
        );
        assert!(
            !explained.load(std::sync::atomic::Ordering::Relaxed),
            "a refused advance must not emit the explanation either"
        );
        assert!(
            matches!(
                &*entry.capture.lock().expect("capture mutex poisoned"),
                CaptureState::Reported { conversation } if conversation == "conv-hook"
            ),
            "and the reported identity must survive the attempt"
        );
    }

    /// The same gating, reached the way production reaches it: through a
    /// whole `capture_pass` whose verdict loop computes an ambiguity for a
    /// session that became `Reported` after the pass had already committed
    /// to scanning for it.
    ///
    /// Worth having ALONGSIDE the direct-call test above, not instead of
    /// it. The direct test pins `declare_ambiguous`'s own contract; this one
    /// pins that a real pass can still arrive at that call with a
    /// `Reported` entry in hand. That is not obvious from the code — the
    /// scanning set is built by skipping every settled state, so a reader
    /// could reasonably conclude a reported session never reaches a verdict
    /// at all and delete the guard as dead weight. It reaches one because
    /// the set is decided BEFORE the scans, and the report handler is not
    /// serialized against passes: the session was un-`Reported` when the
    /// pass chose it and `Reported` by the time the verdict landed.
    ///
    /// The interleaving is produced through the store-fault seam rather
    /// than by racing threads, so the test is deterministic. The seam fires
    /// synchronously inside the FIRST session's ambiguity write, which is
    /// precisely the mid-pass instant a real report can occupy, and the
    /// callback flips the second session's state there.
    ///
    /// Two sessions are needed because the flip has to happen after the
    /// scanning set is built and before the second verdict is reached, and
    /// the first session's durable write is the only hook this pass offers
    /// in that gap. Their windows deliberately do NOT overlap, so the
    /// window-overlap bail is out of the picture and each ambiguity comes
    /// from its own pair of in-window records.
    #[tokio::test]
    async fn a_pass_skips_the_ambiguity_write_for_a_session_reported_mid_pass() {
        let state = StateDir::new();
        let home = tempfile::tempdir().expect("agent home");
        let work = tempfile::tempdir().expect("workdir");
        let cwd = std::fs::canonicalize(work.path())
            .expect("canonicalize")
            .to_string_lossy()
            .to_string();

        let now = crate::agent_kind::now_unix();
        // Ten seconds apart against a one-second window half-width, so the
        // two windows cannot touch; both sit past their horizons.
        let a_at = now - 20;
        let b_at = now - 10;
        let b = claude_entry("session-b", &cwd, Some(b_at), CaptureState::Unclaimed);

        let attempts: Arc<StdMutex<Vec<(CaptureWrite, String)>>> =
            Arc::new(StdMutex::new(Vec::new()));
        let seen = Arc::clone(&attempts);
        let flip = Arc::clone(&b);
        // Stands in for a report landing between the pass choosing to scan
        // for B and the pass reaching B's verdict. Written straight into
        // the cell rather than through `advance` because the point under
        // test is what a pass does when it FINDS `Reported`, not how a
        // state gets there. Every write is recorded and then failed, so an
        // attempted store write shows up here and never reaches SQLite.
        let fault: CaptureStoreFault = Arc::new(move |write, id| {
            seen.lock()
                .expect("fault log poisoned")
                .push((write, id.to_string()));
            if id == "session-a" {
                *flip.capture.lock().expect("capture mutex poisoned") = reported("conv-hook-b");
            }
            Err(anyhow::anyhow!("no write reaches the store in this test"))
        });
        let sup = supervisor_over(&state, home.path(), Some(fault)).await;

        let a = claude_entry("session-a", &cwd, Some(a_at), CaptureState::Unclaimed);
        // Two records inside each session's window: the pair is what makes
        // each verdict `Ambiguous` on its own evidence.
        plant_claude_record(home.path(), &cwd, "conv-a1", a_at);
        plant_claude_record(home.path(), &cwd, "conv-a2", a_at);
        plant_claude_record(home.path(), &cwd, "conv-b1", b_at);
        plant_claude_record(home.path(), &cwd, "conv-b2", b_at);

        capture_pass(&sup, &[Arc::clone(&a), Arc::clone(&b)], true).await;

        let attempts = attempts.lock().expect("fault log poisoned").clone();
        assert_eq!(
            attempts,
            vec![(CaptureWrite::Ambiguity, "session-a".to_string())],
            "only the session that was still unclaimed may reach a durable ambiguity write; \
             the reported one must be skipped before the store is touched: {attempts:?}"
        );
        assert!(
            matches!(
                &*b.capture.lock().expect("capture mutex poisoned"),
                CaptureState::Reported { conversation } if conversation == "conv-hook-b"
            ),
            "and the pass must leave the reported identity exactly as the report left it"
        );
    }

    /// A record another session has been TOLD is its own is not a candidate
    /// for anybody else in the same (kind, cwd) group.
    ///
    /// The scenario is the `/clear` case seen from the outside: session A
    /// has been running in this directory long enough that its capture
    /// window closed hours ago, then clears its conversation and reports
    /// the fresh id — whose record file is written NOW, inside the window
    /// of session B, which has just started in the same directory. The
    /// window-overlap bail cannot help B here, because the windows do not
    /// overlap; without this exclusion A's brand-new record is B's lone
    /// candidate and B durably commits A's conversation. That is the
    /// wrong-conversation claim the whole design exists to prevent, and it
    /// would survive every later pass, since a committed identity is never
    /// replaced.
    ///
    /// The second half is what keeps the exclusion from being merely
    /// conservative: with B's own record present too, the filter turns what
    /// would otherwise be a two-candidate bail into the correct single
    /// match. A report is strictly more evidence for the rivals, never
    /// less.
    #[tokio::test]
    async fn a_rival_never_claims_a_conversation_another_session_reported() {
        let state = StateDir::new();
        let home = tempfile::tempdir().expect("agent home");
        let work = tempfile::tempdir().expect("workdir");
        let cwd = std::fs::canonicalize(work.path())
            .expect("canonicalize")
            .to_string_lossy()
            .to_string();
        let sup = supervisor_over(&state, home.path(), None).await;

        // B sits past its horizon (`after` + `grace` = 2s under
        // `fast_bounds`) so the pass is allowed to conclude for it, while
        // A's window closed an hour ago — which is what takes the
        // window-overlap bail out of the picture and leaves the candidate
        // filter as the only thing standing between B and A's record.
        let now = crate::agent_kind::now_unix();
        let b_at = now - 10;
        let a = claude_entry("session-a", &cwd, Some(now - 3600), reported("conv-a"));
        let b = claude_entry("session-b", &cwd, Some(b_at), CaptureState::Unclaimed);
        plant_claude_record(home.path(), &cwd, "conv-a", b_at);

        // `may_write` is false throughout: these entries were never
        // inserted, so a durable write could only fail, and the verdict is
        // fully observable in the in-memory state one step earlier.
        capture_pass(&sup, &[Arc::clone(&a), Arc::clone(&b)], false).await;
        let verdict = b.capture.lock().expect("capture mutex poisoned").clone();
        assert!(
            matches!(verdict, CaptureState::UncapturedFinal),
            "B must end its window with no identity rather than claiming the record A \
             reported as its own: {verdict:?}"
        );

        // Now B's own record appears. The same filter that produced the
        // bail above must now produce a match rather than an ambiguity, so
        // B gets a fresh entry — the one above is terminally
        // `UncapturedFinal` by design.
        let b2 = claude_entry("session-b", &cwd, Some(b_at), CaptureState::Unclaimed);
        plant_claude_record(home.path(), &cwd, "conv-b", b_at);
        capture_pass(&sup, &[Arc::clone(&a), Arc::clone(&b2)], false).await;
        let verdict = b2.capture.lock().expect("capture mutex poisoned").clone();
        assert!(
            matches!(
                &verdict,
                CaptureState::PendingCommit { conversation, .. } if conversation == "conv-b"
            ),
            "with the reported record excluded, B's own is the single honest match rather \
             than one of two colliding candidates: {verdict:?}"
        );
        assert!(
            matches!(
                &*a.capture.lock().expect("capture mutex poisoned"),
                CaptureState::Reported { conversation } if conversation == "conv-a"
            ),
            "and nothing in the pass may disturb A's reported identity"
        );
    }

    /// The liveness tripwire fires exactly once for a hooked launch that
    /// never reported, and never for one that did.
    ///
    /// This is the only signal that the whole hook mechanism has stopped
    /// working. A hook that never runs — a vendor that renamed its flag, a
    /// wrapper that swallowed the appended tail — degrades silently to the
    /// scan fallback, which keeps producing plausible resume offers, so
    /// there is no failure to notice and no test in production to notice
    /// it. If this warning does not appear, nothing else will say anything
    /// at all.
    ///
    /// Three properties in one pass, because they are one behaviour:
    /// - a hooked, unreported session past its horizon warns;
    /// - a hooked session that DID report never warns, since the mechanism
    ///   worked and a warning would train the reader to ignore the line;
    /// - the flag latches, so the ticker's next pass is silent — otherwise
    ///   a single broken launch produces a line per poll for the life of
    ///   the session and buries every other log.
    ///
    /// Ages the anchor rather than sleeping: the horizon is `after` +
    /// `grace` past first input, and moving first input into the past is
    /// the same arithmetic seen from the other end.
    #[tokio::test]
    async fn the_tripwire_warns_once_for_a_hooked_launch_that_never_reported() {
        let state = StateDir::new();
        let home = tempfile::tempdir().expect("agent home");
        let sup = supervisor_over(&state, home.path(), None).await;
        let ordering = std::sync::atomic::Ordering::Relaxed;

        // Well past the horizon `fast_bounds` implies (1s + 1s), so the
        // pass is entitled to conclude that no report is coming.
        let stale = crate::agent_kind::now_unix() - 3600;
        let silent = claude_entry(
            "hooked-silent",
            "/tmp/one",
            Some(stale),
            CaptureState::Unclaimed,
        );
        let spoke = claude_entry(
            "hooked-reported",
            "/tmp/two",
            Some(stale),
            reported("conv-hook"),
        );
        for entry in [&silent, &spoke] {
            entry.hooked.store(true, ordering);
        }

        capture_pass(&sup, &[Arc::clone(&silent), Arc::clone(&spoke)], false).await;
        assert!(
            silent.hook_warned.load(ordering),
            "a hooked launch still unreported past its horizon must trip the wire"
        );
        assert!(
            !spoke.hook_warned.load(ordering),
            "a session whose hook did report has nothing to warn about"
        );

        // Second pass: the latch is what makes this one line per launch
        // rather than one per tick.
        silent.hook_warned.store(false, ordering);
        capture_pass(&sup, &[Arc::clone(&silent)], false).await;
        assert!(
            silent.hook_warned.load(ordering),
            "the wire is armed by `hooked` alone, so clearing the latch re-arms it — \
             which is what makes the latch the only thing suppressing repeats"
        );
    }

    /// An UNHOOKED launch never trips the wire, however long it goes
    /// without reporting.
    ///
    /// Most launches are unhooked — a Generic session, a kind excluded
    /// from injection, an argv the injection declined to touch — and every
    /// one of them stays unreported forever by design. Warning about them
    /// would make the line meaningless, which is the same as removing it.
    ///
    /// The session with no first input yet is the second half of the same
    /// point: the launch-to-first-prompt gap is unbounded by construction,
    /// so a session nobody has typed into has not yet given a Codex hook
    /// anything to fire on and cannot be late.
    #[tokio::test]
    async fn the_tripwire_stays_silent_without_a_hook_or_a_first_input() {
        let state = StateDir::new();
        let home = tempfile::tempdir().expect("agent home");
        let sup = supervisor_over(&state, home.path(), None).await;
        let ordering = std::sync::atomic::Ordering::Relaxed;

        let stale = crate::agent_kind::now_unix() - 3600;
        let unhooked = claude_entry("unhooked", "/tmp/one", Some(stale), CaptureState::Unclaimed);
        let untyped = claude_entry("no-input", "/tmp/two", None, CaptureState::Unclaimed);
        untyped.hooked.store(true, ordering);

        capture_pass(&sup, &[Arc::clone(&unhooked), Arc::clone(&untyped)], false).await;
        assert!(
            !unhooked.hook_warned.load(ordering),
            "a launch that carried no hook cannot be late with a report"
        );
        assert!(
            !untyped.hook_warned.load(ordering),
            "a session with no delivered input yet has no deadline running"
        );
    }

    /// A reported id is spoken for even when its own session has never
    /// taken input.
    ///
    /// This is the ordinary Claude shape, not a corner: Claude's hook fires
    /// at process startup, so a session normally holds `Reported` before
    /// this supervisor has confirmed a single keystroke for it. Such a
    /// session occupies NO capture window — the window is anchored on first
    /// input — so the overlap bail cannot protect anybody from it, and the
    /// id filter is the only thing standing between a rival and a record
    /// that is already accounted for. A collection that gathered reported
    /// ids only from sessions with an anchor would leave exactly the common
    /// case unprotected.
    #[tokio::test]
    async fn a_report_from_a_session_with_no_first_input_still_excludes_its_record() {
        let state = StateDir::new();
        let home = tempfile::tempdir().expect("agent home");
        let work = tempfile::tempdir().expect("workdir");
        let cwd = canonical(work.path());
        let sup = supervisor_over(&state, home.path(), None).await;

        let now = crate::agent_kind::now_unix();
        let b_at = now - 10;
        let a = claude_entry("session-a", &cwd, None, reported("conv-a"));
        let b = claude_entry("session-b", &cwd, Some(b_at), CaptureState::Unclaimed);
        plant_claude_record(home.path(), &cwd, "conv-a", b_at);

        capture_pass(&sup, &[Arc::clone(&a), Arc::clone(&b)], false).await;
        let verdict = b.capture.lock().expect("capture mutex poisoned").clone();
        assert!(
            matches!(verdict, CaptureState::UncapturedFinal),
            "an anchorless reported session's id must still be excluded: {verdict:?}"
        );
    }

    /// A settled `Reported` session still OCCUPIES its capture window, so a
    /// rival overlapping it bails ambiguous.
    ///
    /// The exclusion tested above and this occupancy pull in opposite
    /// directions, and both are needed. Dropping a reported session from the
    /// window grouping would look like a harmless optimization — nothing
    /// scans for it any more — and would be a regression: a rival whose own
    /// record has not appeared yet would stop bailing, find the reported
    /// session's record as its lone candidate, and commit somebody else's
    /// conversation. The id filter alone does not save it, because the
    /// filter can only remove candidates the rival can see; ambiguity is
    /// about the ones it cannot.
    #[tokio::test]
    async fn a_reported_session_still_occupies_its_window_against_a_rival() {
        let state = StateDir::new();
        let home = tempfile::tempdir().expect("agent home");
        let work = tempfile::tempdir().expect("workdir");
        let cwd = canonical(work.path());
        let sup = supervisor_over(&state, home.path(), None).await;

        // Both anchored at the same instant, so their windows overlap by
        // construction rather than by arithmetic that a bounds change could
        // quietly undo.
        let at = crate::agent_kind::now_unix() - 10;
        let a = claude_entry("session-a", &cwd, Some(at), reported("conv-a"));
        let b = claude_entry("session-b", &cwd, Some(at), CaptureState::Unclaimed);
        plant_claude_record(home.path(), &cwd, "conv-b", at);

        capture_pass(&sup, &[Arc::clone(&a), Arc::clone(&b)], false).await;
        let verdict = b.capture.lock().expect("capture mutex poisoned").clone();
        assert!(
            matches!(verdict, CaptureState::Ambiguous { .. }),
            "a rival overlapping a reported session's window must refuse rather than claim \
             the only record it can see: {verdict:?}"
        );
    }

    /// Every reported id in a group is excluded, not merely the first one
    /// found.
    ///
    /// Two agents reporting in one directory is the ordinary busy case, and
    /// a collection that overwrote rather than accumulated per group — a
    /// `HashMap<_, String>` where this uses a set — would pass every
    /// single-report test above while silently leaving one of the two
    /// records claimable.
    #[tokio::test]
    async fn every_reported_id_in_a_group_is_excluded() {
        let state = StateDir::new();
        let home = tempfile::tempdir().expect("agent home");
        let work = tempfile::tempdir().expect("workdir");
        let cwd = canonical(work.path());
        let sup = supervisor_over(&state, home.path(), None).await;

        // Neither reporter has an anchor, so neither occupies a window and
        // the overlap bail cannot stand in for the filter under test.
        let now = crate::agent_kind::now_unix();
        let c_at = now - 10;
        let a = claude_entry("session-a", &cwd, None, reported("conv-a"));
        let b = claude_entry("session-b", &cwd, None, reported("conv-b"));
        let c = claude_entry("session-c", &cwd, Some(c_at), CaptureState::Unclaimed);
        plant_claude_record(home.path(), &cwd, "conv-a", c_at);
        plant_claude_record(home.path(), &cwd, "conv-b", c_at);

        capture_pass(
            &sup,
            &[Arc::clone(&a), Arc::clone(&b), Arc::clone(&c)],
            false,
        )
        .await;
        let verdict = c.capture.lock().expect("capture mutex poisoned").clone();
        assert!(
            matches!(verdict, CaptureState::UncapturedFinal),
            "with both in-window records spoken for, C has no candidate left rather than a \
             choice between two: {verdict:?}"
        );
    }

    /// The exclusion is scoped to the correlation group, on BOTH halves of
    /// the key: a report in another directory, or from another agent kind,
    /// hides nothing.
    ///
    /// The negative matters as much as the positive. An exclusion keyed too
    /// broadly — on the id alone, say — would silently suppress honest
    /// captures across the whole host, and the symptom would be sessions
    /// that mysteriously never offer a resume rather than anything that
    /// looks like a bug in this filter.
    #[tokio::test]
    async fn a_report_outside_the_group_hides_nothing() {
        let state = StateDir::new();
        let home = tempfile::tempdir().expect("agent home");
        let elsewhere = tempfile::tempdir().expect("other workdir");
        let work = tempfile::tempdir().expect("workdir");
        let other_cwd = canonical(elsewhere.path());
        let cwd = canonical(work.path());
        let sup = supervisor_over(&state, home.path(), None).await;

        let now = crate::agent_kind::now_unix();
        let b_at = now - 10;
        // Same conversation id reported by a session in a DIFFERENT
        // directory, and by one of a different KIND in the same directory.
        // Neither shares B's group, so neither may take B's record away.
        let far = claude_entry("session-far", &other_cwd, None, reported("conv-b"));
        let codex = entry_of_kind(
            AgentKind::Codex,
            "session-codex",
            &cwd,
            None,
            reported("conv-b"),
        );
        let b = claude_entry("session-b", &cwd, Some(b_at), CaptureState::Unclaimed);
        plant_claude_record(home.path(), &cwd, "conv-b", b_at);

        capture_pass(
            &sup,
            &[Arc::clone(&far), Arc::clone(&codex), Arc::clone(&b)],
            false,
        )
        .await;
        let verdict = b.capture.lock().expect("capture mutex poisoned").clone();
        assert!(
            matches!(
                &verdict,
                CaptureState::PendingCommit { conversation, .. } if conversation == "conv-b"
            ),
            "reports from other groups must not suppress an honest capture: {verdict:?}"
        );
    }

    /// A pending claim is re-checked against the reported set before it is
    /// retried, and dropped when a rival has since claimed it.
    ///
    /// The gap this closes: a pending claim is produced by one pass and
    /// written by a later one, and the report that invalidates it can land
    /// in between — the report handler does not join the pass queue. The
    /// verdict-time filter cannot help, because a session holding
    /// `PendingCommit` is skipped by the scanning loop entirely; without
    /// this check the stale claim would be committed by the retry as though
    /// nothing had happened, and a committed identity is never replaced.
    ///
    /// `UncapturedFinal` is the state it lands in because that is what the
    /// verdict loop would have produced from the same evidence: a pending
    /// claim only exists past the horizon off a complete scan, so "the one
    /// candidate belongs to someone else" means there is nothing to claim.
    ///
    /// The second half pins the negative — an unrelated report leaves the
    /// pending claim alone — because a check that dropped every pending
    /// claim whenever ANY report existed in the group would pass the first
    /// half and quietly disable capture for every busy directory.
    #[tokio::test]
    async fn a_pending_claim_a_rival_has_reported_is_dropped_rather_than_committed() {
        let state = StateDir::new();
        let home = tempfile::tempdir().expect("agent home");
        let work = tempfile::tempdir().expect("workdir");
        let cwd = canonical(work.path());
        let sup = supervisor_over(&state, home.path(), None).await;

        let pending = |conversation: &str| CaptureState::PendingCommit {
            conversation: conversation.to_string(),
            record: PathBuf::from("/tmp/pending.jsonl"),
            stamp: RecordStamp {
                len: 0,
                mtime_unix: None,
            },
        };
        let at = crate::agent_kind::now_unix() - 10;
        let a = claude_entry("session-a", &cwd, None, reported("conv-a"));
        let b = claude_entry("session-b", &cwd, Some(at), pending("conv-a"));
        let c = claude_entry("session-c", &cwd, Some(at), pending("conv-c"));

        capture_pass(
            &sup,
            &[Arc::clone(&a), Arc::clone(&b), Arc::clone(&c)],
            false,
        )
        .await;
        let dropped = b.capture.lock().expect("capture mutex poisoned").clone();
        assert!(
            matches!(dropped, CaptureState::UncapturedFinal),
            "a pending claim the rival reported must be abandoned, not written: {dropped:?}"
        );
        let kept = c.capture.lock().expect("capture mutex poisoned").clone();
        assert!(
            matches!(&kept, CaptureState::PendingCommit { conversation, .. }
                if conversation == "conv-c"),
            "an unrelated report must leave a pending claim exactly where it was: {kept:?}"
        );
    }

    /// The tripwire warns once per launch across ORDINARY consecutive
    /// evaluations, with nothing reset in between.
    ///
    /// The existing pass-level test clears the latch by hand to show the
    /// wire re-arms; that deliberately does not answer the question a
    /// reader actually has, which is whether an untouched entry stays
    /// silent on the second pass. It cannot be answered from the latch —
    /// "already true" and "set again" are indistinguishable — so this
    /// counts the decisions instead, which is what the return value exists
    /// for. A tripwire that warned per pass would produce a line every
    /// couple of seconds for the life of the session and bury the log it
    /// was meant to be visible in.
    #[test]
    fn the_tripwire_warns_once_across_consecutive_evaluations() {
        let ordering = std::sync::atomic::Ordering::Relaxed;
        let bounds = fast_bounds();
        let at = 1_700_000_000;
        let entry = claude_entry(
            "hooked-silent",
            "/tmp/one",
            Some(at),
            CaptureState::Unclaimed,
        );
        entry.hooked.store(true, ordering);
        let well_past = bounds.horizon(at) + 3600;

        let entries = [Arc::clone(&entry)];
        assert_eq!(
            report_liveness_tripwire(&entries, bounds, well_past),
            1,
            "the first evaluation past the horizon is the one that speaks"
        );
        assert_eq!(
            report_liveness_tripwire(&entries, bounds, well_past),
            0,
            "and every one after it is silent without anything being reset"
        );
        assert!(entry.hook_warned.load(ordering));
    }

    /// The horizon is a real boundary: silent one second before it, warning
    /// at it.
    ///
    /// Driven by passing `now` directly rather than by aging an anchor
    /// against the wall clock, because that is the only way to test a
    /// boundary rather than a neighbourhood of one — a test built on
    /// `now_unix()` would be deciding the answer with a race against the
    /// second it happens to run in.
    ///
    /// The boundary itself is not arbitrary: the tripwire reuses the scan's
    /// own settling point (`after` + `grace` past first input) rather than
    /// inventing a deadline, so a session cannot be called late while the
    /// scan still considers its evidence open.
    #[test]
    fn the_tripwire_is_silent_until_the_horizon_and_speaks_at_it() {
        let ordering = std::sync::atomic::Ordering::Relaxed;
        let bounds = fast_bounds();
        let at = 1_700_000_000;
        let horizon = bounds.horizon(at);

        let early = claude_entry("early", "/tmp/one", Some(at), CaptureState::Unclaimed);
        early.hooked.store(true, ordering);
        assert_eq!(
            report_liveness_tripwire(&[Arc::clone(&early)], bounds, horizon - 1),
            0,
            "one second short of the horizon the report may still be on its way"
        );
        assert!(!early.hook_warned.load(ordering));
        assert_eq!(
            report_liveness_tripwire(&[Arc::clone(&early)], bounds, horizon),
            1,
            "at the horizon the scan itself has settled, so a missing report is real"
        );
    }

    /// The tripwire runs on a supervisor with no agent home, which is the
    /// configuration it matters most in.
    ///
    /// A homeless supervisor never enters `capture_pass` at all —
    /// `capture_pass_for` bails before it — so a tripwire placed only inside
    /// the pass was unreachable in production for exactly the deployment
    /// where nothing else would ever mention a hook that stopped working.
    /// Driven through `capture_pass_for` rather than the pass itself
    /// because that early return IS the thing under test.
    ///
    /// "No home" is expressed as an empty path rather than by unsetting
    /// `HOME`: the seam falls back to the environment when it is `None`,
    /// and this repo's tests never mutate the test process's environment.
    /// An empty override is filtered out at construction and yields exactly
    /// the homeless supervisor production gets when `HOME` is unset.
    #[tokio::test]
    async fn the_tripwire_still_runs_where_there_is_no_agent_home() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_seams(
            state.path(),
            dummy_exe(),
            SupervisorTimeouts::default(),
            SupervisorSeams {
                agent_home: Some(PathBuf::new()),
                capture_window: fast_bounds(),
                ..SupervisorSeams::default()
            },
        )
        .await
        .expect("supervisor");
        assert!(
            sup.agent_home.is_none(),
            "test premise: an empty override must resolve to no agent home at all"
        );

        let ordering = std::sync::atomic::Ordering::Relaxed;
        let stale = crate::agent_kind::now_unix() - 3600;
        let silent = claude_entry(
            "hooked-silent",
            "/tmp/one",
            Some(stale),
            CaptureState::Unclaimed,
        );
        silent.hooked.store(true, ordering);
        // `capture_pass_for` reads the supervisor's own session map rather
        // than taking entries, so the entry has to be published into it.
        sup.sessions
            .lock()
            .await
            .insert(silent.info.id.clone(), Arc::clone(&silent));

        sup.capture_pass_for(CaptureReason::Reply).await;
        assert!(
            silent.hook_warned.load(ordering),
            "a supervisor with nothing to scan must still say that a hook never reported"
        );
    }

    /// The canonical form of a temporary directory, which is what a capture
    /// group is keyed on.
    ///
    /// Not cosmetic: on macOS (and anywhere `/tmp` is a symlink) the path a
    /// `tempdir` hands back differs from the one the kernel reports to the
    /// agent, and a test comparing the two would fail for a reason that has
    /// nothing to do with capture.
    fn canonical(path: &Path) -> String {
        std::fs::canonicalize(path)
            .expect("canonicalize")
            .to_string_lossy()
            .to_string()
    }
}
