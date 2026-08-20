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

use super::core::{SessionEntry, Supervisor};
use crate::agent_kind::{CaptureVerdict, CaptureWindow, RecordStamp};
use std::collections::HashMap;
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
/// 5. `Captured` — committed and read back. This is the only state that
///    advertises Resume.
/// 6. `Ambiguous` — dominant over everything. Durable, so a restart cannot
///    re-decide on evidence that has since gotten thinner.
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
}

impl CaptureState {
    /// The DURABLY claimed identity, if any.
    ///
    /// Deliberately `None` for `Provisional` and `PendingCommit`: this is
    /// what `farhelm_proto::RestartOffer::Resume` is computed from, and the
    /// offer is a promise that a stored identity exists for restart to fill
    /// in. A provisional match is not that promise, and a pending one is not
    /// yet.
    pub(crate) fn committed_conversation(&self) -> Option<&str> {
        match self {
            CaptureState::Captured { conversation, .. } => Some(conversation.as_str()),
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
        }
    }

    /// Whether this session still has anything to scan for.
    fn is_settled(&self) -> bool {
        matches!(
            self,
            CaptureState::UncapturedFinal
                | CaptureState::Captured { .. }
                | CaptureState::Ambiguous { .. }
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
    fn advance(&mut self, next: CaptureState) -> bool {
        let allowed = match (self.rank(), next.rank()) {
            (current, incoming) if incoming > current => true,
            (current, incoming) if incoming == current => matches!(
                next,
                CaptureState::Provisional { .. }
                    | CaptureState::PendingCommit { .. }
                    | CaptureState::Ambiguous { .. }
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
    let Some(home) = sup.agent_home.as_deref() else {
        return;
    };
    let bounds = sup.capture_window;
    let now = crate::agent_kind::now_unix();

    // Retry the durable writes that failed earlier, off the input path.
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
            } if may_write => {
                commit_capture(sup, entry, conversation, record, stamp).await;
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
    let mut occupied: HashMap<(&'static str, &str), Vec<(&str, CaptureWindow)>> = HashMap::new();
    for entry in entries {
        let (Some(_), Some(cwd)) = (entry.snapshot.integration(), entry.canonical_cwd.as_deref())
        else {
            continue;
        };
        let Some(at) = entry
            .first_input
            .lock()
            .expect("first-input mutex poisoned")
            .at
        else {
            continue;
        };
        occupied
            .entry((crate::store::agent_kind_column(entry.snapshot.kind), cwd))
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
            let reason = overlapping_windows_reason(&entry.info.id, rival.0, cwd);
            warn!(session = %entry.info.id, rival = %rival.0, cwd = %cwd, "{reason}");
            declare_ambiguous(sup, entry, may_write).await;
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
        let outcome = scanned
            .get(&scan.root)
            .expect("every scanning session's root was scanned");
        // The recorded cwd FIELD, not the directory the record was found
        // in: the munging is non-injective, and Codex does not partition
        // by directory at all.
        let mine: Vec<&crate::agent_kind::Candidate> = outcome
            .candidates
            .iter()
            .filter(|candidate| candidate.correlators.cwd == scan.cwd)
            .collect();
        // Past the horizon, this session's evidence is in: its window has
        // closed and anything written inside it has had the publication
        // grace to become readable. Only then may a claim be committed.
        let settled = now >= bounds.horizon(scan.first_input_at);
        match crate::agent_kind::choose(&mine, scan.window) {
            CaptureVerdict::Ambiguous(why) => {
                warn!(session = %scan.entry.info.id, cwd = %scan.cwd, "{why}");
                declare_ambiguous(sup, scan.entry, may_write).await;
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

    // Re-verification last, and only for sessions holding a committed
    // identity: an append is a confirmation signal, not a claim, so it
    // neither needs nor may use the scan above.
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
/// The in-memory state moves FIRST and unconditionally: ambiguity
/// dominates, and a supervisor that cannot write (or whose write fails)
/// must still refuse rather than keep hunting for a claim it has already
/// decided it may not make. `durable` then tracks whether the refusal
/// survived the process, and `capture_pass` retries until it has.
async fn declare_ambiguous(sup: &Supervisor, entry: &Arc<SessionEntry>, may_write: bool) {
    entry
        .capture
        .lock()
        .expect("capture mutex poisoned")
        .advance(CaptureState::Ambiguous { durable: false });
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
