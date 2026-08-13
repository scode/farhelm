//! The supervisor's own heartbeat: one periodic task, started by `serve`,
//! that advances the work nobody should have to ask for — activity
//! sampling, conversation capture, and the dead-tab reap (SPEC.md's
//! "a tab whose process exits is reaped automatically", which has no
//! event to ride: tmux pushes no pane-death notification, and listing
//! paths are reads that must not carry the side effect).
//!
//! Until PLAN_M6_75.md item 1 this supervisor had no internal cadence at
//! all. Everything periodic rode a request — conversation capture advanced
//! because `ListSessions` happened to run a pass on its way to building a
//! reply. That worked, and it is still what makes a poll's answer fresh,
//! but it made the supervisor a hostage: the cadence belonged to whoever
//! dialled in, no contract promised one, and a helm that stopped polling
//! (or a supervisor nobody had connected to yet) simply stopped capturing.
//! A supervisor responsible for its own state has to be able to make
//! progress with nobody watching, which is what this task is.
//!
//! # Two cadences for capture, and what they actually cost
//!
//! The conversation-capture sweep runs from BOTH here and the list path,
//! and keeping the list-path call is a decision rather than an oversight.
//! They answer different questions:
//!
//! - The TICKER guarantees PROGRESS. Capture advances on a schedule this
//!   process owns, whether or not anything ever calls `ListSessions`.
//! - The LIST PATH guarantees FRESHNESS-ON-REPLY. Proto v10 puts no push
//!   on the supervisor edge, so a drain's reply is the only way a client
//!   ever learns anything; a reply whose `restart_offer` came from a sweep
//!   that predates the request would describe the world before the write
//!   the caller is racing, which is exactly what the helm's post-write
//!   wake exists to avoid.
//!
//! An earlier version of this doc claimed running both "costs nothing"
//! because the pass single-flights. That was false in two ways worth
//! recording, because both shaped the design that replaced it. Skipping on
//! a busy lock only ever collapses passes that OVERLAP — a 2-second ticker
//! beside a 3-second drain mostly does not overlap, so the steady cost was
//! additive, and multiplied again by pagination, since the list path
//! sweeps per PAGE. And the skip was actively wrong for the list path: a
//! caller that gave up because somebody else's pass was in flight could
//! reply from a pre-commit `restart_offer`.
//!
//! [`Supervisor::capture_pass_for`] replaces both behaviors with one
//! scheduling rule per caller ([`super::core::CaptureReason`]): a
//! reply-producing caller WAITS and then runs unless the pass it joined
//! began after its own request; a tick SKIPS a pass in flight and
//! SUPPRESSES itself when a REPLY-driven pass completed within the tick
//! interval. Its own previous passes pointedly do not suppress it — that
//! would halve the unattended cadence this task exists to guarantee. The
//! resulting envelope is roughly "one sweep per interval, whoever pays for
//! it" rather than one per cadence.
//!
//! # The sampling rule
//!
//! SPEC.md is unambiguous that wrong status is cosmetic and that status
//! detection "must never gate or delay interaction with the terminal".
//! Two things make that structural here rather than a promise:
//!
//! - [`start_ticker`] is called from exactly one place (`serve`), the
//!   sample pass from exactly one place (this task), and nothing on the
//!   attach, input, or resize path awaits either.
//! - The tick takes its permit from [`Supervisor::sampling_admission`],
//!   NOT from the request-handling `admission` semaphore. Sharing the
//!   latter looked reasonable — same tmux subprocesses — and was a SPEC
//!   violation: periodic work holding a request permit can park
//!   `handle_control`, and with it the connection read loop that
//!   dispatches keystrokes. That field's docs carry the full argument.
//!
//! Sampling itself writes nothing durable — it reads panes and updates
//! in-memory [`ActivitySample`] cells — so it is not a `may_record`
//! question at all. The capture half is, but only PARTLY: a supervisor
//! that may not record still scans the agents' record trees and still
//! advances its in-memory capture state, because reading and concluding
//! are not the thing it lacks standing for. What `may_record` gates is the
//! durable write, inside the pass, where the conclusion would become a
//! claim.
//!
//! # What the samples are for
//!
//! This module measures; it does not classify. `service::status`'s
//! `live_status` reads exactly what is recorded here — how many of a
//! session's own consecutive samples showed nothing new, and the tail it
//! last showed — and turns it into running/waiting/idle, with the per-kind
//! sharpeners matching a pending question or approval against that tail.
//!
//! Keeping the thresholds there rather than here is what lets the whole
//! classification be unit-tested against hand-built entries. Keeping them
//! expressed in SAMPLES rather than seconds is what keeps this task free
//! to be late — under a budgeted round robin its cadence is a function of
//! how many sessions are live, and a classification that read a clock
//! would silently turn that population into a status. See
//! [`ActivitySample`]'s own docs, which carry the argument.
//!
//! Being free to be late is not the same as being free to run flat out.
//! The loop's own deadline is ANCHORED so a pass that takes part of its
//! interval does not push every later tick later, and it is floored so a
//! pass that OVERRUNS its interval is not followed instantly by the next
//! one — which is what a supervisor too loaded to keep up would otherwise
//! do, spawning tmux subprocesses as fast as it could retire them.
//! [`next_deadline`] is both rules, as a pure function, for the same
//! reason the thresholds live in `status`: it can then be asserted rather
//! than timed.
//!
//! # Shutdown contract
//!
//! [`start_ticker`] returns a [`TickerHandle`] that OWNS the task. In
//! production `serve` holds it for the process's life; if `serve` is ever
//! cancelled or unwound the handle drops and the task stops. The task
//! additionally holds only a `Weak<Supervisor>`, so a supervisor dropped
//! out from under it ends the loop cleanly rather than keeping a whole
//! state directory, tmux driver, and SQLite connection alive for as long
//! as the runtime lives.
//!
//! Dropping the handle does NOT abort the task. An abort could land
//! anywhere, including inside a capture pass between a durable write and
//! the in-memory state that mirrors it, so the stop is COOPERATIVE and the
//! places it is honoured are chosen:
//!
//! - At the top of the loop, before sleeping.
//! - Between individual pane samples — each is a separate bounded tmux
//!   subprocess, so the worst a stop waits on is one capture, itself
//!   bounded by that command's own deadline.
//! - NOT during a capture pass, which is deliberately shielded: once a
//!   pass has begun it runs to completion so its durable writes and their
//!   in-memory mirrors cannot be separated by a shutdown.
//!
//! So the honest bound on how long a stop takes is: one pane capture
//! (bounded), plus a capture pass if one had already started. It is NOT
//! "the next await point", which the first version of this doc claimed
//! while the code only checked at the top of the loop.
//!
//! `TickerHandle::shutdown` waits for the task to actually be gone and
//! fails if it panicked, so "no leaked task" and "the ticker did not die
//! screaming" are both assertions rather than hopes. In production
//! `TickerHandle::watch` is what turns the same information into a loud
//! log line — see `serve`.

use super::core::{CaptureReason, SampleRead, SessionEntry, Supervisor};
use super::terminals::{Terminal, tabs_from_pane_states};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use tracing::{debug, error, info, warn};

/// How often the supervisor's periodic task fires in production.
///
/// Chosen against the helm's own 3-second `ListSessions` drain rather than
/// independently: a ticker SLOWER than the drain would leave drains
/// answering from samples nothing had refreshed since the previous one,
/// while a much faster one would spend subprocesses producing samples no
/// reader ever gets to see.
///
/// ## What this interval is, and is not, a bound on
///
/// It is the period of the TASK, not of any one session's sampling. A
/// live session is sampled once every `ceil(live / SAMPLE_TAIL_BUDGET)`
/// ticks, so its own refresh period is
/// `ceil(live / budget) × interval`, plus however long each round's work
/// takes. An earlier version of this doc promised a drain would find a
/// sample "no older than one interval"; that was only ever true below the
/// budget, and it is not what anything depends on.
///
/// Nothing depends on it because the classification does not read a clock
/// at all: `status::live_status` counts a session's OWN consecutive
/// unchanged samples ([`ActivitySample`]), so a longer effective period
/// makes a transition arrive later without ever making it wrong. Tail
/// freshness — what a sharpener matches against — has the same
/// population-dependent bound, with the same consequence: a prompt that
/// appeared is noticed at the session's next sample, whenever that is.
///
/// Overridable per supervisor through [`crate::service::SupervisorSeams`],
/// which is how tests get a cadence measured in milliseconds.
pub(crate) const TICKER_INTERVAL: Duration = Duration::from_secs(2);

/// How much of a sampled pane's screen is kept.
///
/// Enough for the bottom of a normal terminal — an 80x24 screen of dense
/// text is under 2 KiB — with headroom for a wide one, because the
/// sharpeners have to see a whole approval prompt to recognize it. The cap
/// exists because this is held per session for as long as the session
/// lives, and a pane rendering a 500-column wall of text should not be
/// able to grow the supervisor's resident memory through it.
const SAMPLE_TAIL_BYTES: usize = 4096;

/// How many pane tails one tick may capture.
///
/// Each tail is a `capture-pane` subprocess, so an unbounded pass would
/// make the supervisor's per-second subprocess load scale with the number
/// of live sessions — the same multiplication `TmuxDriver::pane_states`
/// exists to avoid for liveness. Sessions past the budget are not skipped,
/// they are DEFERRED: the pass resumes where the previous one stopped, so
/// a live session is sampled once every `ceil(live / budget)` ticks rather
/// than every tick. That is the real guarantee, and it is weaker than
/// "every live session every tick" — with more than this many live
/// sessions, status transitions are simply noticed later. Degrading the
/// resolution of a cosmetic status is the right thing to give up here.
const SAMPLE_TAIL_BUDGET: usize = 16;

/// Permits in [`Supervisor::sampling_admission`] — see that field's docs
/// for why the ticker may not draw on the request semaphore at all.
///
/// One, because there is exactly one ticker and its captures are
/// sequential. It is a real limiter rather than a formality only for a
/// future second sampler; what it buys today is a chokepoint a test can
/// hold in order to prove that a wedged sampler cannot delay a request.
pub(crate) const SAMPLING_ADMISSION_PERMITS: usize = 1;

/// What the sampler last observed on one session's agent pane.
///
/// Stored per entry (see [`SessionEntry::activity`] for why there and not
/// in a map on `Supervisor`) and never persisted: it describes what THIS
/// process has watched happen, and a value restored from disk would claim
/// knowledge of a stretch of time nobody was looking.
///
/// What the classifier (`status::live_status`) gets is deliberately raw —
/// counts and a screen, not a verdict — so that the running/waiting/idle
/// thresholds and the per-kind sharpening live in one place, beside the
/// precedence rules they extend, rather than being half-decided here.
///
/// # Why nothing here is a timestamp
///
/// The obvious shape for "is this pane still producing output" is an
/// instant of last change, compared by the classifier against a wall-clock
/// window. That shape is WRONG here, and the reason is [`SAMPLE_TAIL_BUDGET`]:
/// a session is sampled once every `ceil(live / budget)` ticks, so on a
/// host with more than `budget` live sessions the gap between one
/// session's OWN samples grows past any fixed window — and a pane that
/// changed between every one of its samples would still read "quiet for
/// longer than the window" and classify idle. The status would then be a
/// function of how many OTHER sessions the host is running, which is
/// nonsense a user would experience as the column going idle as their
/// fleet grew.
///
/// Counting the session's own consecutive unchanged samples instead makes
/// the sampling cadence cancel out of the classification entirely: the
/// question becomes "how many times have I looked at THIS pane and seen
/// nothing new", which means the same thing at any budget, any population,
/// and any interval.
#[derive(Debug, Default)]
pub(crate) struct ActivitySample {
    /// How many times this session has been sampled at all.
    ///
    /// Load-bearing rather than a statistic: the FIRST sample of a pane
    /// establishes the baseline that later ones are compared against, and
    /// must not itself count as output. Without this, every session would
    /// look busy the moment it was first looked at — including one that
    /// has sat at a shell prompt untouched since a reboot. The classifier
    /// reads it as "has this pane been watched twice yet", which is the
    /// question [`ActivitySample::unchanged_streak`] cannot answer on its
    /// own (a streak of zero means both "never compared" and "just
    /// changed").
    pub(crate) samples: u64,
    /// How many consecutive COMPARISONS have found the screen unchanged,
    /// reset to zero by any observed change.
    ///
    /// The decay signal `status::live_status` turns into idle. Counted in
    /// this session's own samples rather than in seconds — see the type's
    /// own docs for why a wall-clock window is the wrong shape under a
    /// budgeted round robin.
    ///
    /// Only ever incremented by a comparison, so the first sample of a
    /// pane leaves it at zero: there was nothing to compare against, and
    /// counting that as quiet would start every session's decay one step
    /// in.
    pub(crate) unchanged_streak: u64,
    /// The pane's screen as of the last sample, bounded to
    /// [`SAMPLE_TAIL_BYTES`] and trimmed of the blank rows a pane is
    /// padded out with.
    ///
    /// Serves both consumers: change detection compares it against the
    /// next capture, and the per-kind sharpeners match a prompt or
    /// approval shape in it.
    pub(crate) tail: Option<String>,
}

impl ActivitySample {
    /// A cell for a session nothing has looked at yet — what every
    /// freshly built [`SessionEntry`] starts with.
    ///
    /// Handed out pre-wrapped because the wrapper is part of the contract:
    /// a rename SHARES this `Arc` while a relaunch mints a new one, and a
    /// constructor returning a bare value would let a call site quietly
    /// pick the wrong side of that rule.
    pub(crate) fn unsampled() -> Arc<std::sync::Mutex<ActivitySample>> {
        Arc::new(std::sync::Mutex::new(ActivitySample::default()))
    }

    /// Fold one freshly captured screen into this sample.
    ///
    /// Change is detected by comparing screens rather than by asking tmux
    /// for an activity timestamp, because tmux's notion of window activity
    /// counts any write to the pty — a redraw of an unchanged screen, a
    /// cursor blink from a TUI's own timer — which for status purposes is
    /// exactly the noise that would report an idle agent as working. What
    /// this can miss in exchange is output that lands and is overwritten
    /// entirely between two samples; that costs one sample of decay on a
    /// status SPEC.md already calls cosmetic.
    ///
    /// Takes no clock, deliberately: everything the classifier needs is a
    /// count of this session's own observations (see the type's docs), and
    /// a timestamp here would only invite a wall-clock comparison back in.
    pub(crate) fn observe(&mut self, tail: String) {
        if self.samples > 0 {
            if self.tail.as_deref() == Some(tail.as_str()) {
                self.unchanged_streak += 1;
            } else {
                self.unchanged_streak = 0;
            }
        }
        self.tail = Some(tail);
        self.samples += 1;
    }

    /// Drop the retained screen after a capture this session was SELECTED
    /// for failed, without recording an observation.
    ///
    /// The bug this closes is specific and durable, which is why forgetting
    /// is worth doing at all. A tail is not only change-detection input: it
    /// is the evidence the per-kind sharpeners read, and they read the LAST
    /// one indefinitely. So a session whose pane showed an approval prompt,
    /// whose user then answered it, and whose captures then began failing
    /// — a pane that is still alive, so still classified from its
    /// baseline — would go on reporting `Waiting` forever on the strength
    /// of a screen that stopped being true at the first failure. Nothing
    /// recovers from that except a successful capture, and the whole
    /// premise of the case is that none is coming.
    ///
    /// What it deliberately does NOT do is count as a sample or as a quiet
    /// look: an unreachable pane is not evidence of stillness, and letting
    /// a run of failures decay a session to `Idle` would be the same wrong
    /// inference `sample_pass` refuses to make when tmux answers nothing at
    /// all. The baseline simply stops moving, which is honest, and
    /// sharpening stops claiming anything, which is the point.
    ///
    /// Change detection is unaffected in the way that matters: the next
    /// SUCCESSFUL capture has nothing to compare against, so it records no
    /// change (`observe` only compares when a previous sample exists...
    /// which it does — `samples` is untouched here — so a first capture
    /// after a forget compares against `None` and resets the streak).
    /// Resetting the streak is the conservative direction: it means
    /// `Running`, which is what "we do not know" has meant on this path
    /// since the beginning.
    pub(crate) fn forget_tail(&mut self) {
        self.tail = None;
    }
}

/// The owner of a running ticker task.
///
/// Holding this is what keeps the ticker alive, which is why `serve` binds
/// it to a name for the whole of its accept loop rather than discarding
/// it. See the module docs for the shutdown contract, including where the
/// cooperative stop is actually honoured and what it therefore bounds.
pub(crate) struct TickerHandle {
    /// Dropped (or fired) to ask the loop to stop. Never actually sent
    /// through in production — a plain drop resolves the receiver, which
    /// is the same signal — but a `oneshot` rather than a bare `Notify` so
    /// the stop is a latched fact the sampling loop can re-check between
    /// captures rather than an edge it can miss.
    _stop: oneshot::Sender<()>,
    /// The task, taken once by whoever observes its end.
    ///
    /// `Option` because a `JoinHandle` must never be polled after it
    /// completes, and [`TickerHandle::watch`] is polled from inside
    /// `serve`'s `select!` on every loop iteration; taking it on the first
    /// completion is what lets the second and later iterations park
    /// forever instead of panicking.
    task: Option<tokio::task::JoinHandle<()>>,
}

impl TickerHandle {
    /// Await the ticker's END and report it, loudly if it panicked; then
    /// never resolve again.
    ///
    /// `serve` selects on this beside `accept`, which is the only
    /// supervision a task like this can usefully get: restarting it would
    /// re-run whatever input panicked, and failing the process would take
    /// a whole host's sessions offline over best-effort bookkeeping. What
    /// is NOT acceptable is silence — a dead ticker means capture quietly
    /// stops advancing for any session nobody polls, and the first symptom
    /// would be a restart offering a fresh launch where a resume was
    /// expected.
    ///
    /// Cancellation-safe: `serve`'s other arm routinely wins the race and
    /// drops this future, which only drops a borrow of the handle.
    pub(crate) async fn watch(&mut self) {
        let Some(task) = self.task.as_mut() else {
            // Already reported. Parking is what keeps this arm quiet for
            // the rest of the accept loop's life.
            std::future::pending::<()>().await;
            return;
        };
        let outcome = task.await;
        self.task = None;
        match outcome {
            Ok(()) => warn!(
                "the supervisor's periodic ticker stopped while this process is still \
                 serving; conversation capture and status sampling will no longer advance \
                 on their own, only when a request drives them"
            ),
            Err(e) => error!(
                error = %e,
                "the supervisor's periodic ticker PANICKED; conversation capture and status \
                 sampling will no longer advance on their own, only when a request drives them"
            ),
        }
    }
}

#[cfg(test)]
impl TickerHandle {
    /// Stop the ticker and WAIT for its task to be gone, failing if it
    /// panicked.
    ///
    /// The determinism a test needs and production does not: after this
    /// returns, no further tick can touch the supervisor, so a test may
    /// assert that samples stopped accumulating without racing a pass that
    /// was already in flight. Production stops the ticker by dropping the
    /// handle, which asks for the same thing without waiting.
    ///
    /// The `expect` is load-bearing rather than tidiness: swallowing the
    /// `JoinError` would let a ticker that panicked on its very first tick
    /// pass every shutdown test in this file, since a panicked task is
    /// also a stopped one.
    pub(crate) async fn shutdown(mut self) {
        drop(self._stop);
        if let Some(task) = self.task.take() {
            task.await.expect("the ticker task must not panic");
        }
    }

    /// Whether the task has ended on its own.
    ///
    /// Only meaningful for the one thing that can end it without a stop
    /// signal: the last `Arc<Supervisor>` going away, which the loop
    /// notices at its next upgrade. Callers still `shutdown()` afterwards,
    /// so a task that ended by PANICKING rather than by noticing the drop
    /// fails the test instead of satisfying it.
    pub(crate) fn is_finished(&self) -> bool {
        self.task
            .as_ref()
            .is_some_and(tokio::task::JoinHandle::is_finished)
    }
}

/// The failure a test has installed in place of one of a sampling pass's
/// tmux reads, if any — see [`SampleFault`](super::core::SampleFault) for
/// why the two failure paths need a seam at all.
///
/// `None` in production, always, so each call site is one `Option` check
/// next to a subprocess round trip.
fn injected_sample_fault(sup: &Supervisor, read: SampleRead<'_>) -> Option<anyhow::Error> {
    sup.seams
        .sample_fault
        .as_ref()
        .and_then(|fault| fault(read))
        .map(anyhow::Error::msg)
}

/// Where the sampler's round robin resumes: the id of the last session it
/// sampled, or `None` to start at the head of the order.
///
/// An ID rather than an index, and [`sample_pass`]'s rotation section
/// carries the argument: an index into a re-sorted population is a position
/// that CHURN MOVES, so deletes ahead of the cursor can step over the
/// sessions behind it indefinitely. The id names a place in the order
/// instead, which nothing else appearing or disappearing can shift.
///
/// The named session need not still exist. Resuming means "the first live
/// session ordered after this one", which is well defined whether or not
/// the session it names is still there — so a deleted session's id needs no
/// cleanup and cannot strand the rotation.
type SampleCursor = Option<String>;

/// The smallest gap the ticker will ever leave between the END of one pass
/// and the START of the next, as a fraction of the configured interval.
///
/// A fraction rather than a constant duration so the rule scales with
/// whatever cadence a caller (or a test) chose: a 2-second production
/// interval and a 50-millisecond test interval want the same SHAPE of
/// recovery pause, not the same number of milliseconds. Half is
/// deliberately generous — under sustained overrun the supervisor spends at
/// least a third of its time not sampling, which is the whole point — and
/// the exact value matters little, because reaching this path at all means
/// the host is already too loaded to sample at the cadence anyone asked
/// for.
const MIN_TICK_GAP_DIVISOR: u32 = 2;

/// Where the next tick lands, given the deadline that just fired, the
/// instant the pass finished, and the configured interval.
///
/// Two rules, and they answer different failures:
///
/// - **Anchored**, not "now plus interval": the deadline advances by whole
///   intervals from its predecessor, so a pass that took 300ms of a
///   2-second cadence still fires the next tick 2 seconds after the LAST
///   one rather than 2.3 seconds later. A sleep-after-work loop makes the
///   real cadence `interval + work`, so every slow pass pushes every later
///   tick permanently later — a drift that compounds for the life of the
///   process and that nobody can see, since the only symptom is samples
///   arriving less often than the interval anyone configured.
/// - **Never sooner than [`MIN_TICK_GAP_DIVISOR`] allows**, which is what
///   the anchoring alone gets wrong. When a pass overruns its period every
///   anchored deadline is already in the past, so an anchor-only rule
///   schedules the next pass for *immediately*, forever — back-to-back
///   subprocess churn with no recovery pause, on precisely the host that is
///   already struggling. Pushing the deadline past `now` is what bounds it.
///
/// A pure function of three values, so the schedule can be tested without a
/// clock, a runtime, or a tmux (`next_deadline_*` below). That is the whole
/// reason it is not inlined into the loop: a scheduling rule verified only
/// by timing a real loop is verified by a stopwatch on a shared CI runner.
fn next_deadline(
    fired: tokio::time::Instant,
    now: tokio::time::Instant,
    interval: Duration,
) -> tokio::time::Instant {
    let mut next = fired + interval;
    if next > now {
        // On cadence: the anchor is in the future and is returned exactly,
        // whatever fraction of the interval the pass consumed. The minimum
        // gap deliberately does NOT apply here — applying it would stretch
        // the cadence of a perfectly healthy ticker whose passes merely
        // take more than half an interval, which is the drift this
        // anchoring exists to prevent.
        return next;
    }
    // Overran: skip whole intervals, so the phase survives a pass that ran
    // through several periods rather than re-anchoring on the overrun. The
    // loop is bounded by however many periods were missed.
    while next <= now {
        next += interval;
    }
    next.max(now + interval / MIN_TICK_GAP_DIVISOR)
}

/// Start this supervisor's periodic task and hand back its owner.
///
/// Called by `serve` once initialization is complete — the session map is
/// final, the socket is bound, the startup reconciliation has run — so
/// that no tick can observe a half-built supervisor. (It is NOT ordered
/// after the startup sweeps to avoid a file race: the capture pass reads
/// the agents' record roots under `agent_home` while those sweeps unlink
/// launch, snapshot, and tmux-config files under the state dir, which are
/// disjoint sets. `serve`'s own comment carries the correction.)
///
/// The task holds a `Weak`, so this does NOT extend the supervisor's
/// lifetime; a failed upgrade is a normal, silent end to the loop.
pub(crate) fn start_ticker(sup: &Arc<Supervisor>) -> TickerHandle {
    let interval = sup.seams.ticker_interval;
    let weak = Arc::downgrade(sup);
    let (stop_tx, mut stop_rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        // Round-robin position in the sampling budget, task-local because
        // there is exactly one ticker: keeping it here rather than on the
        // supervisor means no shared state and no lock for a value only
        // this loop ever reads.
        let mut cursor: SampleCursor = None;
        // ANCHORED rather than a sleep at the end of each pass. A
        // `sleep(interval)` after the work makes the real cadence
        // `interval + work`, so every slow capture pass permanently pushes
        // every later tick later — a drift that compounds for the life of
        // the process and that no reader can see, since the only symptom
        // is samples arriving less often than the interval anyone
        // configured.
        //
        // The first tick is anchored one interval out because `serve` has
        // just run a reload and a capture pass of its own; a tick at t=0
        // would repeat that work before anything could have changed.
        //
        // The deadline is computed rather than taken from a tokio
        // `Interval`, and the reason is the overrun case none of tokio's
        // three `MissedTickBehavior`s covers. All three of them return
        // IMMEDIATELY from a `tick()` whose deadline has already passed —
        // `Skip` only decides where the NEXT deadline lands — so under
        // sustained overrun (a pass that takes longer than the interval,
        // which a loaded host with many live sessions can genuinely do)
        // every tick is already overdue by the time it is awaited and the
        // passes run back to back forever. That is a supervisor spawning
        // tmux subprocesses as fast as it can retire them, with no pause in
        // which anything could recover; see `next_deadline` for the rule
        // that replaces it, and for why anchoring is still what the
        // non-overrun case gets.
        let mut deadline = tokio::time::Instant::now() + interval;
        loop {
            tokio::select! {
                _ = &mut stop_rx => break,
                _ = tokio::time::sleep_until(deadline) => {}
            }
            let Some(sup) = weak.upgrade() else {
                break;
            };
            tick(&sup, &mut cursor, SAMPLE_TAIL_BUDGET, &mut stop_rx).await;
            deadline = next_deadline(deadline, tokio::time::Instant::now(), interval);
            // A stop observed BETWEEN captures has already been taken out
            // of the channel by `stop_requested`, and a `oneshot::Receiver`
            // that has yielded its result panics when polled again — which
            // the `select!` above would do on the next iteration. Breaking
            // here is what keeps the cooperative stop from ending the task
            // by panicking instead of by returning; `try_recv` itself is
            // safe to repeat, so asking again costs nothing.
            if stop_requested(&mut stop_rx) {
                break;
            }
        }
    });
    TickerHandle {
        _stop: stop_tx,
        task: Some(task),
    }
}

/// Whether the ticker has been asked to stop.
///
/// A closed channel is the signal, because production stops the ticker by
/// DROPPING the handle rather than by sending; `try_recv` reports both
/// that and an explicit send without awaiting, which is what makes it
/// usable as a between-captures check inside an otherwise linear pass.
fn stop_requested(stop: &mut oneshot::Receiver<()>) -> bool {
    !matches!(stop.try_recv(), Err(oneshot::error::TryRecvError::Empty))
}

/// One period's work: reap dead tabs, sample a slice of the live panes,
/// then advance conversation capture.
///
/// All three phases are periodic and sequential, so each pushes the next
/// by up to its own duration; sampling precedes capture because its
/// result is what a drain arriving mid-tick will read, while a capture
/// arriving a beat later costs nothing anybody can observe. The REAP
/// phase (inside `sample_pass`, riding its pane snapshot) is the one that
/// weakens the old "neither half can delay a request" claim, and only
/// narrowly: a close takes the target session's LIFECYCLE claim, so an
/// interactive operation on that same session can wait behind one reap —
/// bounded by [`REAP_BUDGET_PER_TICK`] and by the stop check between
/// closes. Requests on other sessions are unaffected, and the request
/// semaphore stays separate (see [`Supervisor::sampling_admission`]'s
/// docs for why sharing it was a SPEC violation rather than a tuning
/// question).
///
/// The permit comes from [`Supervisor::sampling_admission`], never from
/// the request semaphore: see that field's docs for why sharing it was a
/// SPEC violation rather than a tuning question.
///
/// `budget` and `stop` are parameters rather than constants read here so
/// that tests can drive an OVER-budget population without standing up
/// seventeen tmux sessions, and can observe the cooperative stop without
/// racing the loop that owns it.
async fn tick(
    sup: &Arc<Supervisor>,
    cursor: &mut SampleCursor,
    budget: usize,
    stop: &mut oneshot::Receiver<()>,
) {
    let _permit = Arc::clone(&sup.sampling_admission)
        .acquire_owned()
        .await
        .expect("sampling semaphore is never closed");
    sample_pass(sup, cursor, budget, stop).await;
    if stop_requested(stop) {
        // A capture pass already in flight is shielded (it runs to
        // completion once begun), but there is no reason to START one for
        // a ticker that has been asked to stop.
        return;
    }
    sup.capture_pass_for(CaptureReason::Tick {
        suppress_within: sup.seams.ticker_interval,
    })
    .await;
}

/// Snapshot a BUDGETED slice of the live agent panes into their entries'
/// [`ActivitySample`] cells.
///
/// Liveness comes from ONE batched `pane_states` call for the whole
/// server, the same read the list path uses and for the same reason: a
/// per-session probe would multiply subprocess spawns by the session
/// count on every tick. Only a pane found under BOTH its remembered pane
/// id and its remembered tmux session name is sampled — the identity
/// cross-check `session_status` documents, since a recycled pane id would
/// otherwise let one session's screen be recorded as another's.
///
/// Dead panes and terminal-less entries are skipped rather than sampled
/// as empty: their status is decided entirely by the durable record, so a
/// sample would be work whose result nothing consults.
///
/// # Rotation, and why it is keyed by session ID
///
/// `cursor` names the session this pass should resume AFTER, by id, and
/// the next pass starts at the first live session ordered strictly after
/// it. An earlier version carried a numeric INDEX into the sorted live
/// list, which is a different thing that only looks the same while the
/// population holds still: sessions are sorted by id, so deleting or
/// stopping a session ORDERED BEFORE the cursor shifts every later session
/// one place toward the front, and the next pass resumes one position too
/// far in. Under steady churn ahead of it — which is what a busy host does
/// all day — a session near the end of the order can be stepped over
/// indefinitely and never sampled at all, so its status simply stops
/// updating with nothing anywhere reporting a problem. An id is a position
/// in the ORDER rather than in the list, so churn moves the population
/// around it without moving it.
///
/// `None` starts at the head. Both early returns that mean "there is
/// nothing to rotate through" reset to it, including the empty-map case:
/// carrying a resume point across a population REPLACEMENT would be
/// resuming after a session that no longer exists, and while the id order
/// makes that harmless it is still not what the first pass over a fresh
/// population should do.
///
/// A FAILED liveness probe is the one early return that does not reset,
/// and the difference is the point: "there are no live sessions" is an
/// answer about the population, while "tmux could not be asked" is no
/// answer at all, and rewinding the rotation on it would resample the head
/// of the list every time a flaky tmux dropped a query.
///
/// # Cancellation
///
/// `stop` is re-checked between individual captures — and, in the reap
/// phase this pass now opens with, between individual closes — so a
/// shutdown waits out at most one in-flight operation: one `capture-pane`
/// (bounded by that command's own deadline) or one tab close (bounded by
/// [`REAP_BUDGET_PER_TICK`] from ever being a batch). The cursor is still
/// advanced past what was actually sampled, so a resumed ticker would not
/// redo work — though in practice nothing resumes a stopped ticker.
///
/// SAMPLING failures are logged at DEBUG and the pass moves on. A ticker
/// that WARNED here would turn a tmux server that is down (or a pane that
/// vanished between the two calls, which is ordinary) into a log entry
/// every interval forever; the paths where a user is actually waiting on
/// the answer — the list path — still warn, which is where the signal
/// belongs. The REAP phase is the deliberate exception: a real close
/// failure warns (see `reap_dead_tabs`), because it can mean leftover
/// processes with no user-visible handle left to retry through.
async fn sample_pass(
    sup: &Arc<Supervisor>,
    cursor: &mut SampleCursor,
    budget: usize,
    stop: &mut oneshot::Receiver<()>,
) {
    // Cloned out of the map and the lock released before any tmux call:
    // the session mutex is never held across an await (see `Supervisor`'s
    // lock discipline), and every line below awaits a subprocess.
    let entries: Vec<Arc<SessionEntry>> = sup.sessions.lock().await.values().cloned().collect();
    if entries.is_empty() {
        *cursor = None;
        return;
    }
    let states = match injected_sample_fault(sup, SampleRead::PaneStates) {
        Some(fault) => Err(fault),
        None => sup.tmux.pane_states().await,
    };
    let states = match states {
        Ok(states) => states,
        Err(e) => {
            debug!(
                error = %format!("{e:#}"),
                "could not probe pane liveness for this sampling pass; forgetting every \
                 retained screen so nothing is sharpened from stale evidence"
            );
            // Nothing was looked at, so nothing may be CLASSIFIED from what
            // was last seen either. The per-pane failure below invalidates
            // one session's evidence for exactly this reason, and a probe
            // that failed for the whole server is the same fact about every
            // session at once — a prompt that has since been answered would
            // otherwise hold its session at `Waiting` for as long as these
            // probes keep failing, which is indefinitely, and which the
            // LIST path cannot correct because its own probe succeeding is
            // what makes those sessions live enough to be sharpened at all.
            //
            // Sample counts are deliberately untouched, for the same reason
            // they are on the per-pane path: an unreachable tmux is not
            // evidence of stillness, and decaying a fleet to `Idle` on it
            // would be a wrong answer rather than an absent one.
            for entry in &entries {
                entry
                    .activity
                    .lock()
                    .expect("activity mutex poisoned")
                    .forget_tail();
            }
            return;
        }
    };
    // Dead-tab reaping rides this pass's pane-state fetch, BEFORE the
    // agent-liveness filtering below: a session whose agent exited can
    // still hold tabs, and the early return on an all-dead fleet would
    // otherwise leave their corpses unreaped forever.
    reap_dead_tabs(sup, &states, &entries, stop).await;
    let mut live: Vec<(Arc<SessionEntry>, Terminal)> = entries
        .into_iter()
        .filter_map(|entry| {
            let terminal = entry.terminal.clone()?;
            let state = states.get(&terminal.pane)?;
            (state.session_name == terminal.tmux_name && !state.dead).then_some((entry, terminal))
        })
        .collect();
    if live.is_empty() {
        *cursor = None;
        return;
    }
    // A stable order is what makes the round-robin cursor mean anything:
    // over a `HashMap`'s iteration order the budget would resample an
    // arbitrary subset every tick and starve the rest indefinitely.
    live.sort_by(|(a, _), (b, _)| a.info.id.cmp(&b.info.id));
    // Resume at the first session ordered strictly AFTER the one this
    // cursor names — a position in the id order, not an index into the
    // list, so sessions appearing or disappearing ahead of it cannot step
    // over the sessions behind it. `partition_point` finds it in one binary
    // search over the sort just applied, and an id past the end of the
    // population wraps to the head, which is what a full rotation is.
    let start = match cursor.as_deref() {
        Some(last) => {
            live.partition_point(|(entry, _)| entry.info.id.as_str() <= last) % live.len()
        }
        None => 0,
    };
    let take = budget.min(live.len());
    for offset in 0..take {
        if stop_requested(stop) {
            break;
        }
        let (entry, terminal) = &live[(start + offset) % live.len()];
        // Advanced BEFORE the capture rather than after it, so a pass that
        // stops (or whose capture fails) still leaves the rotation past
        // this session: the alternative would resample whatever the ticker
        // was cut off at, forever, every time a shutdown or a flaky pane
        // landed on the same entry.
        *cursor = Some(entry.info.id.clone());
        let captured = match injected_sample_fault(
            sup,
            SampleRead::Tail {
                session: &entry.info.id,
            },
        ) {
            Some(fault) => Err(fault),
            None => {
                sup.tmux
                    .capture_pane_tail(&terminal.tmux_name, &terminal.pane, SAMPLE_TAIL_BYTES)
                    .await
            }
        };
        let tail = match captured {
            Ok(tail) => tail,
            Err(e) => {
                debug!(
                    session = %entry.info.id, error = %format!("{e:#}"),
                    "could not sample this session's pane; forgetting the screen it last showed \
                     so nothing is sharpened from stale evidence"
                );
                // The SELECTED-but-failed case: this session's pane was
                // reachable enough to be chosen and its screen could not be
                // read, so whatever it last showed is of unknown age from
                // here on. See `ActivitySample::forget_tail` for why that
                // must not count as a quiet look.
                entry
                    .activity
                    .lock()
                    .expect("activity mutex poisoned")
                    .forget_tail();
                continue;
            }
        };
        entry
            .activity
            .lock()
            .expect("activity mutex poisoned")
            .observe(tail);
    }
}

/// Upper bound on tab reaps attempted in one tick.
///
/// The bound is a liveness guard for everything queued behind this
/// function on the ticker's task — activity sampling, conversation
/// capture, and cooperative shutdown — because each close can wait on a
/// session's lifecycle claim and several teardown subprocesses. Whatever
/// the budget leaves behind stays hidden from listings and is retried
/// next tick, so the cap trades only reap LATENCY for a mass exit, never
/// correctness. Sized to comfortably cover the ordinary case (one shell
/// exiting at a time) with room for a burst.
const REAP_BUDGET_PER_TICK: usize = 4;

/// Close every tab whose window's panes are ALL dead — the enforcement
/// half of SPEC.md's "a tab whose process exits is reaped automatically"
/// (the listing paths hide dead tabs; this is what actually removes them).
///
/// Lives on the ticker rather than the listing paths deliberately:
/// listings are reads and stay side-effect-free, and tmux pushes no
/// pane-death event to react to, so the poll is the only trigger.
/// DISCOVERY rides this pass's already-fetched `pane_states` snapshot,
/// grouped by session in one O(panes) pass; each CLOSE then pays its own
/// resolution probe inside [`Supervisor::close_tab`], which is the
/// ordinary close — the same lifecycle claim, process sweep, window kill,
/// and attached-client notice a user-initiated close performs — so a
/// reaped tab is indistinguishable from a closed one. (The client treats
/// that notice's tab-closed reason as silent removal, per SPEC.)
///
/// Reaps are awaited serially on the ticker's own task, bounded by
/// [`REAP_BUDGET_PER_TICK`] and re-checked against `stop` between closes,
/// so neither a mass exit nor a slow teardown can wedge sampling, capture,
/// or shutdown behind it; the next tick retries whatever remains. The
/// expected failure is a race with a manual close of the same tab
/// (`NotFound`, logged at DEBUG — the dead-at-open refusal path can also
/// destroy a marked window the same instant this sees it); any OTHER
/// failure warns, because it may mean a close that killed the window and
/// then failed its process sweep — a tab that no longer exists to retry
/// through, with only its scope/marker sweeps left to catch stragglers.
///
/// One residual this inherits rather than creates: pane-state marker
/// collection is skipped entirely when NO session on the private tmux
/// server has two or more windows, so a server reduced by hand to
/// single-window sessions (an agent window killed directly against the
/// private socket, leaving one tab window) has undiscoverable — and
/// therefore unreapable — tabs. That state is unreachable through the
/// product (the agent window is never killed while the session lives;
/// `remain-on-exit` keeps even a dead agent pane's window), and it is the
/// same residual `detach_closed_tab` documents for by-hand window kills.
async fn reap_dead_tabs(
    sup: &Arc<Supervisor>,
    states: &std::collections::HashMap<String, crate::tmux::PaneState>,
    entries: &[Arc<SessionEntry>],
    stop: &mut oneshot::Receiver<()>,
) {
    // One pass over the server-wide snapshot, partitioned by session —
    // handing the full map to every session's rediscovery would rescan
    // all panes per session, a quadratic cost paid every two seconds on
    // hosts with many sessions, dead tabs or none. Borrowed, not cloned:
    // this runs every tick whether anything is dead or not.
    let mut by_session: std::collections::HashMap<&str, Vec<&crate::tmux::PaneState>> =
        std::collections::HashMap::new();
    for state in states.values() {
        by_session
            .entry(state.session_name.as_str())
            .or_default()
            .push(state);
    }
    let mut budget = REAP_BUDGET_PER_TICK;
    for entry in entries {
        let Some(terminal) = entry.terminal.as_ref() else {
            continue;
        };
        let Some(session_states) = by_session.get(terminal.tmux_name.as_str()) else {
            continue;
        };
        for tab in tabs_from_pane_states(session_states.iter().copied(), &terminal.tmux_name) {
            if !tab.dead {
                continue;
            }
            if stop_requested(stop) {
                return;
            }
            if budget == 0 {
                debug!(
                    "the tick's tab-reap budget is spent; remaining dead tabs stay hidden \
                     and the next tick continues"
                );
                return;
            }
            budget -= 1;
            match sup.close_tab(&entry.info.id, &tab.id).await {
                Ok(()) => {
                    info!(
                        session = %entry.info.id, tab = %tab.id,
                        "reaped a terminal tab whose shell exited"
                    );
                }
                Err(e) if e.kind == farhelm_proto::ErrorKind::NotFound => {
                    debug!(
                        session = %entry.info.id, tab = %tab.id, error = %e.message,
                        "an exited tab vanished before this tick's reap — a manual close or \
                         the open path's own cleanup won the race"
                    );
                }
                Err(e) => {
                    warn!(
                        session = %entry.info.id, tab = %tab.id, error = %e.message,
                        "reaping an exited tab failed; if its window survives the next tick \
                         retries, otherwise only the close's scope and marker sweeps remain \
                         to catch leftover processes"
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::connection::{CONNECTION_WRITER_QUEUE, ConnectionCtx};
    use super::super::core::tests::{StateDir, dummy_exe, entry_with, no_uploads};
    use super::super::core::{
        CreateInputs, CreateMode, SupervisorSeams, SupervisorTimeouts, note_first_input,
    };
    use super::super::handlers::handle_control;
    use super::super::status::session_status;
    use super::*;
    use crate::agent_kind::{CaptureWindowBounds, IntegrationSnapshot};
    use crate::store::{LastOutcome, StoredSession, now_unix};
    use farhelm_proto::{AgentKind, ControlMsg, SessionStatus};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::sync::mpsc;

    /// A cadence short enough that a test spends milliseconds rather than
    /// production intervals waiting for a tick, and long enough that the
    /// ticker is not spawning tmux subprocesses faster than a loaded CI
    /// runner can retire them.
    const TEST_INTERVAL: Duration = Duration::from_millis(50);

    /// How long the polling assertions below are willing to wait. Sized
    /// for a loaded runner where a `capture-pane` subprocess can take a
    /// surprising fraction of a second, not for the tick cadence.
    const TEST_DEADLINE: Duration = Duration::from_secs(30);

    /// A supervisor whose ticker cadence is [`TEST_INTERVAL`], with the
    /// caller's other seams folded in.
    ///
    /// Goes through `new_with_seams` rather than the shorter `new_with_exe`
    /// for exactly one reason — the interval — which is also why the
    /// interval is a seam at all: no test can wait out the production one.
    async fn supervisor_with(state: &StateDir, seams: SupervisorSeams) -> Arc<Supervisor> {
        Supervisor::new_with_seams(
            state.path(),
            dummy_exe(),
            SupervisorTimeouts::default(),
            SupervisorSeams {
                ticker_interval: TEST_INTERVAL,
                ..seams
            },
        )
        .await
        .expect("supervisor")
    }

    /// Start a real tmux session named `name` running `command`, and
    /// return its pane id.
    ///
    /// Deliberately NOT `create_session`: a create launches through the
    /// farhelm shim, and these tests point at [`dummy_exe`], so the pane
    /// would be dead before anything could sample it. Driving tmux
    /// directly is what lets a test choose what the pane DOES, which for a
    /// sampler is the entire subject.
    async fn spawn_pane(sup: &Arc<Supervisor>, name: &str, command: &str) -> String {
        let argv = ["sh".to_string(), "-c".to_string(), command.to_string()];
        sup.tmux
            .create_session(name, "/tmp", 80, 24, &[], &argv)
            .await
            .expect("create a tmux session directly");
        let states = sup.tmux.pane_states().await.expect("pane states");
        states
            .iter()
            .find(|(_, state)| state.session_name == name)
            .map(|(pane, _)| pane.clone())
            .expect("the session just created has a pane")
    }

    /// Put an entry addressing `terminal` into the session map under `id`.
    ///
    /// Reuses `core`'s own entry fixture rather than building a
    /// `SessionEntry` by hand so that a future field addition lands in one
    /// place — and so these tests keep asserting against the same shape
    /// every other classification test uses.
    async fn install_entry(sup: &Arc<Supervisor>, id: &str, terminal: Terminal) {
        install_entry_of_kind(sup, id, terminal, AgentKind::Generic).await;
    }

    /// [`install_entry`] for a session that claims an integrated agent
    /// kind, which is what makes the per-kind sharpeners apply to it. The
    /// snapshot is the only thing classification reads to decide that, so
    /// no launch has to be faked.
    async fn install_entry_of_kind(
        sup: &Arc<Supervisor>,
        id: &str,
        terminal: Terminal,
        kind: AgentKind,
    ) {
        let mut entry = entry_with(Some(terminal), LastOutcome::Running);
        entry.info.id = id.to_string();
        entry.snapshot = IntegrationSnapshot {
            kind,
            resume_template: None,
        };
        sup.sessions
            .lock()
            .await
            .insert(id.to_string(), Arc::new(entry));
    }

    /// A live session: a real pane running `command`, and a map entry that
    /// truthfully addresses it.
    async fn install_live_session(sup: &Arc<Supervisor>, id: &str, command: &str) {
        install_live_session_of_kind(sup, id, command, AgentKind::Generic).await;
    }

    /// [`install_live_session`] for an integrated kind.
    async fn install_live_session_of_kind(
        sup: &Arc<Supervisor>,
        id: &str,
        command: &str,
        kind: AgentKind,
    ) {
        let tmux_name = format!("fh-{id}");
        let pane = spawn_pane(sup, &tmux_name, command).await;
        install_entry_of_kind(sup, id, Terminal { tmux_name, pane }, kind).await;
    }

    /// What a `ListSessions` reply would say about this session right now,
    /// computed exactly the way one is: probe tmux once, then classify the
    /// entry against the result.
    ///
    /// Going through the real [`session_status`] rather than reading the
    /// sample cell is the entire point of the tests below — the unit tests
    /// in `status` already pin the classifier against hand-built cells,
    /// and what is left to prove is that a REAL pane, sampled by the REAL
    /// ticker, reaches it.
    async fn classify(sup: &Arc<Supervisor>, id: &str) -> SessionStatus {
        let states = sup.tmux.pane_states().await.expect("pane states");
        let entry = sup
            .sessions
            .lock()
            .await
            .get(id)
            .cloned()
            .expect("the session is in the map");
        session_status(&entry, &states).0
    }

    /// This session's sample cell, cloned out so an assertion never holds
    /// the session map's lock while it waits.
    async fn sample_of(sup: &Arc<Supervisor>, id: &str) -> Arc<std::sync::Mutex<ActivitySample>> {
        Arc::clone(
            &sup.sessions
                .lock()
                .await
                .get(id)
                .expect("the session is in the map")
                .activity,
        )
    }

    /// Poll `condition` until it holds or [`TEST_DEADLINE`] passes.
    ///
    /// Hand-rolled rather than borrowed: this crate has no shared waiter,
    /// and the deadline loops already scattered through `service::core`'s
    /// tests are the local convention.
    async fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
        let deadline = tokio::time::Instant::now() + TEST_DEADLINE;
        loop {
            if condition() {
                return;
            }
            assert!(tokio::time::Instant::now() < deadline, "timed out: {what}");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// A shared fixture for the reap tests: a tmux window on `tmux_name`,
    /// marked as a tab, whose command has already exited into a dead pane.
    ///
    /// Driven through tmux directly (window + marker) rather than
    /// `open_tab` so the shell's immediate exit is the test's choice and
    /// no launch machinery is in the frame. Returns the pane id — the
    /// handle the assertions watch for disappearing.
    async fn dead_tab_in(sup: &Arc<Supervisor>, tmux_name: &str) -> String {
        dead_tab_running(sup, tmux_name, "true").await.0
    }

    /// [`dead_tab_in`] with the tab's command chosen by the test, and the
    /// spawn-authority env vars a REAL `open_tab` would set — which is
    /// what lets a command daemonize a child the close's marker sweep can
    /// find. Returns `(pane, tab_id)`.
    async fn dead_tab_running(
        sup: &Arc<Supervisor>,
        tmux_name: &str,
        command: &str,
    ) -> (String, String) {
        let tab_id = uuid::Uuid::new_v4().to_string();
        let session_id = tmux_name.strip_prefix("fh-").expect("fh- prefixed name");
        let env = vec![
            (
                crate::launch::SESSION_ID_ENV_VAR.to_string(),
                session_id.to_string(),
            ),
            (crate::launch::TAB_ID_ENV_VAR.to_string(), tab_id.clone()),
        ];
        let (_window, pane) = sup
            .tmux
            .new_window(
                tmux_name,
                "/tmp",
                &env,
                &["sh".to_string(), "-c".to_string(), command.to_string()],
            )
            .await
            .expect("create the tab window");
        sup.tmux
            .mark_window(tmux_name, &pane, crate::tmux::TAB_WINDOW_OPTION, &tab_id)
            .await
            .expect("mark the tab window");
        let deadline = tokio::time::Instant::now() + TEST_DEADLINE;
        loop {
            let states = sup.tmux.pane_states().await.expect("pane states");
            if states.get(&pane).is_some_and(|state| state.dead) {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "the tab's shell never exited into a dead pane"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        (pane, tab_id)
    }

    /// Tabs whose shells exited are REAPED by the tick — every dead tab in
    /// the pass, not just the first match — while a live sibling tab
    /// survives untouched. This is the enforcement half of SPEC.md's "a
    /// tab whose process exits is reaped automatically" (BUGS_BURNDOWN.md
    /// issue 3; the listing paths only HIDE dead tabs).
    ///
    /// Pinned at the ticker because nothing else may do it: listings are
    /// side-effect-free reads, and tmux's control protocol pushes no
    /// pane-death event to react to, so the poll is the only trigger. Two
    /// dead tabs in one session because a first-match-only reap would pass
    /// a single-corpse test while leaving simultaneous exits behind.
    #[tokio::test]
    async fn dead_tab_panes_are_reaped_by_the_tick_and_live_tabs_survive() {
        let state = StateDir::new();
        let sup = supervisor_with(&state, SupervisorSeams::default()).await;
        install_live_session(&sup, "reap-tab", "sleep 600").await;
        let tmux_name = "fh-reap-tab";

        // dead_a daemonizes a child before exiting — the reap goes through
        // the ordinary close, whose marker sweep must take the child with
        // the window; a bare kill-window would leave it running.
        let daemon_pid_file = state.path().join("reap-daemon.pid");
        // The daemon writes its OWN pid (`$$` after setsid+exec), the same
        // identity-safe shape `write_daemon_script` uses in the e2e suite:
        // recording `$!` of a backgrounded `setsid` is wrong whenever
        // setsid forks (it does when it starts as a group leader), and a
        // dead recorded pid makes every later liveness assertion vacuous.
        // The trailing wait-for-the-pid-file is the fixture's other
        // survival condition: a pane whose shell exits before the child
        // has called setsid takes the child down with its process group,
        // so the pane holds until the daemon has provably detached.
        let (dead_a, _tab_a) = dead_tab_running(
            &sup,
            tmux_name,
            &format!(
                "( setsid /bin/sh -c 'echo $$ > {pid}; exec sleep 300' \
                 </dev/null >/dev/null 2>&1 & ); \
                 while [ ! -s {pid} ]; do sleep 0.05; done",
                pid = daemon_pid_file.display()
            ),
        )
        .await;
        let dead_b = dead_tab_in(&sup, tmux_name).await;
        let live_id = uuid::Uuid::new_v4().to_string();
        let (_window, live_pane) = sup
            .tmux
            .new_window(
                tmux_name,
                "/tmp",
                &[],
                &["sleep".to_string(), "600".to_string()],
            )
            .await
            .expect("create the live tab window");
        sup.tmux
            .mark_window(
                tmux_name,
                &live_pane,
                crate::tmux::TAB_WINDOW_OPTION,
                &live_id,
            )
            .await
            .expect("mark the live tab window");

        // Before the reap: the two reply-facing listings split exactly on
        // deadness. `session_tabs` (single-session replies) hides the
        // corpses; `session_tabs_including_dead` (teardown's scope
        // enumeration) still names them — a hidden corpse still owns a
        // cgroup scope that archive and delete must stop.
        let terminal = sup
            .sessions
            .lock()
            .await
            .get("reap-tab")
            .expect("the session is installed")
            .terminal
            .clone()
            .expect("the session has a terminal");
        let listed = sup.session_tabs(&terminal).await.expect("session_tabs");
        assert_eq!(
            listed.iter().map(|tab| tab.id.as_str()).collect::<Vec<_>>(),
            vec![live_id.as_str()],
            "reply-facing tab listing must hide dead tabs even before the reap"
        );
        let with_dead = sup
            .session_tabs_including_dead(&terminal)
            .await
            .expect("session_tabs_including_dead");
        assert_eq!(
            with_dead.len(),
            3,
            "the teardown-facing listing must still name every dead tab's scope-bearing id"
        );

        // The fixture's premise, proven BEFORE the reap: the daemon is a
        // real, running process. Without this, a fixture whose spawn
        // silently failed would let the swept-daemon assertion below pass
        // vacuously. The pid file is written by the daemon itself, so it
        // can trail the pane's death by a scheduler beat — hence the poll.
        let daemon_pid: u32 = {
            let deadline = tokio::time::Instant::now() + TEST_DEADLINE;
            loop {
                if let Ok(pid) = std::fs::read_to_string(&daemon_pid_file)
                    .map_err(|_| ())
                    .and_then(|contents| contents.trim().parse().map_err(|_| ()))
                {
                    break pid;
                }
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "the dead tab's command never wrote its daemon's pid"
                );
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        };
        assert!(
            std::path::Path::new(&format!("/proc/{daemon_pid}")).exists(),
            "the daemonized child must be alive before the reap for its death to mean anything"
        );

        let mut cursor = None;
        let (_stop_tx, mut stop) = oneshot::channel();
        sample_pass(&sup, &mut cursor, SAMPLE_TAIL_BUDGET, &mut stop).await;

        let states = sup.tmux.pane_states().await.expect("pane states");
        assert!(
            !states.contains_key(&dead_a) && !states.contains_key(&dead_b),
            "every dead tab's window must be gone after one tick, not just the first"
        );
        let survivors = tabs_from_pane_states(states.values(), tmux_name);
        assert_eq!(
            survivors
                .iter()
                .map(|tab| tab.id.as_str())
                .collect::<Vec<_>>(),
            vec![live_id.as_str()],
            "the live sibling tab must survive the reap untouched"
        );

        // The daemonized child went with its tab: the reap ran the real
        // close (marker sweep included), not a bare window kill.
        let deadline = tokio::time::Instant::now() + TEST_DEADLINE;
        while std::path::Path::new(&format!("/proc/{daemon_pid}")).exists() {
            assert!(
                tokio::time::Instant::now() < deadline,
                "the reaped tab's daemonized child (pid {daemon_pid}) must be swept"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// The per-tick reap budget defers, never drops: a burst of dead tabs
    /// larger than [`REAP_BUDGET_PER_TICK`] is finished by the NEXT pass,
    /// with the budget boundary observable in between.
    ///
    /// The boundary matters because the budget is a liveness guard for
    /// everything queued behind the reap on the ticker's task — a budget
    /// that silently dropped the remainder would strand corpses forever,
    /// and one that ignored its own cap would reintroduce the monopoly it
    /// exists to prevent.
    #[tokio::test]
    async fn the_reap_budget_defers_the_overflow_to_the_next_tick() {
        let state = StateDir::new();
        let sup = supervisor_with(&state, SupervisorSeams::default()).await;
        install_live_session(&sup, "reap-burst", "sleep 600").await;
        let tmux_name = "fh-reap-burst";
        let mut corpses = Vec::new();
        for _ in 0..(REAP_BUDGET_PER_TICK + 1) {
            corpses.push(dead_tab_in(&sup, tmux_name).await);
        }

        let mut cursor = None;
        let (_stop_tx, mut stop) = oneshot::channel();
        sample_pass(&sup, &mut cursor, SAMPLE_TAIL_BUDGET, &mut stop).await;
        let states = sup.tmux.pane_states().await.expect("pane states");
        let remaining = corpses
            .iter()
            .filter(|pane| states.contains_key(*pane))
            .count();
        assert_eq!(
            remaining, 1,
            "one pass reaps exactly its budget and defers the overflow"
        );

        sample_pass(&sup, &mut cursor, SAMPLE_TAIL_BUDGET, &mut stop).await;
        let states = sup.tmux.pane_states().await.expect("pane states");
        assert!(
            corpses.iter().all(|pane| !states.contains_key(pane)),
            "the next pass finishes what the budget deferred"
        );
    }

    /// The reap still runs when NO agent pane on the whole server is live
    /// — the sampler's `live.is_empty()` early return must not be reached
    /// before it (an easy regression: reaping placed after that return
    /// would strand tabs on exactly the sessions whose agents exited).
    ///
    /// A dedicated supervisor whose ONLY session has a dead agent pane, so
    /// the emptiness is real rather than incidental: an earlier version of
    /// this scenario shared a supervisor with a live-agent test and proved
    /// nothing. Deliberately a dead agent PANE, not a killed window — a
    /// by-hand window kill is the residual state the product cannot reach,
    /// and it also drops the session toward the single-window regime where
    /// the marker query is skipped server-wide.
    #[tokio::test]
    async fn a_dead_tab_is_reaped_when_no_agent_pane_is_live() {
        let state = StateDir::new();
        let sup = supervisor_with(&state, SupervisorSeams::default()).await;
        install_live_session(&sup, "reap-dead-agent", "true").await;
        let tmux_name = "fh-reap-dead-agent";
        let tab_pane = dead_tab_in(&sup, tmux_name).await;

        // The premise the test exists for: no live agent pane anywhere.
        let deadline = tokio::time::Instant::now() + TEST_DEADLINE;
        loop {
            let states = sup.tmux.pane_states().await.expect("pane states");
            let entries: Vec<Arc<SessionEntry>> =
                sup.sessions.lock().await.values().cloned().collect();
            let any_live_agent = entries.iter().any(|entry| {
                entry.terminal.as_ref().is_some_and(|terminal| {
                    states.get(&terminal.pane).is_some_and(|state| {
                        state.session_name == terminal.tmux_name && !state.dead
                    })
                })
            });
            if !any_live_agent {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "the agent pane never went dead"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        let mut cursor = None;
        let (_stop_tx, mut stop) = oneshot::channel();
        sample_pass(&sup, &mut cursor, SAMPLE_TAIL_BUDGET, &mut stop).await;

        let states = sup.tmux.pane_states().await.expect("pane states");
        assert!(
            !states.contains_key(&tab_pane),
            "a dead tab must be reaped even with no live agent pane anywhere"
        );
        assert!(
            tabs_from_pane_states(states.values(), tmux_name).is_empty(),
            "no tab may remain discoverable on the dead-agent session after the reap"
        );
    }

    /// The whole point of PLAN_M6_75.md item 1: conversation capture
    /// advances because the SUPERVISOR decided to, not because somebody
    /// polled it.
    ///
    /// This test never calls `ListSessions`, `list_page`, or `capture_now`
    /// — the three carriers capture used to ride — so a passing run is
    /// positive evidence that the ticker is what claimed the identity. It
    /// runs against a real planted record rather than a stubbed pass
    /// because the regression worth catching is a ticker that fires on
    /// schedule and does nothing.
    #[tokio::test]
    async fn capture_advances_on_the_ticker_with_nobody_polling() {
        let state = StateDir::new();
        let home = tempfile::tempdir().expect("agent home");
        let work = tempfile::tempdir().expect("workdir");
        let cwd = work.path().to_string_lossy().to_string();
        let sup = supervisor_with(
            &state,
            SupervisorSeams {
                agent_home: Some(home.path().to_path_buf()),
                // A horizon a couple of seconds out: no claim is durable
                // until the capture window has closed, so this is the
                // floor on how long this test can possibly take.
                capture_window: CaptureWindowBounds::new(
                    Duration::from_secs(1),
                    Duration::from_secs(1),
                    Duration::from_secs(1),
                ),
                ..SupervisorSeams::default()
            },
        )
        .await;

        // A Claude-kind session — derived from the invocation's basename —
        // whose launch is expected to fail: capture correlates a RECORD
        // tree against a first-input anchor, and neither needs a living
        // agent.
        let created = sup
            .create_session(
                CreateInputs {
                    cwd: &cwd,
                    parent: None,
                    mode: CreateMode::Raw {
                        invocation: "/opt/bin/claude".to_string(),
                        agent_kind: None,
                        resume_template: None,
                    },
                    title: Some("ticker".to_string()),
                    cols: 80,
                    rows: 24,
                },
                None,
            )
            .await
            .expect("the create reaches a launch");
        let entry = sup
            .sessions
            .lock()
            .await
            .get(&created.id)
            .cloned()
            .expect("the created session is in the map");
        note_first_input(&sup, &entry);
        let at = wait_for_first_input(&sup, &created.id).await;

        // The record the agent would have written, planted directly: the
        // subject here is the ticker, not the agent.
        let canonical = std::fs::canonicalize(work.path()).expect("canonicalize");
        let canonical = canonical.to_string_lossy().to_string();
        let project = home
            .path()
            .join(".claude")
            .join("projects")
            .join(crate::agent_kind::munge_cwd(&canonical));
        std::fs::create_dir_all(&project).expect("record directory");
        let line = serde_json::json!({
            "type": "user",
            "sessionId": "ticker-captured-conversation",
            "cwd": canonical,
            "timestamp": crate::agent_kind::format_rfc3339(at),
        });
        std::fs::write(project.join("ticker.jsonl"), format!("{line}\n"))
            .expect("plant the record");
        assert_eq!(
            sup.session_snapshot(&created.id)
                .await
                .expect("snapshot")
                .expect("present")
                .captured_conversation,
            None,
            "premise: nothing has captured this yet, so the ticker below is the only candidate"
        );

        let ticker = start_ticker(&sup);
        let mut captured = None;
        let deadline = tokio::time::Instant::now() + TEST_DEADLINE;
        while tokio::time::Instant::now() < deadline {
            captured = sup
                .session_snapshot(&created.id)
                .await
                .expect("snapshot")
                .expect("present")
                .captured_conversation;
            if captured.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        ticker.shutdown().await;
        assert_eq!(
            captured.as_deref(),
            Some("ticker-captured-conversation"),
            "no list, no poll, no manual pass — the ticker is what captured this"
        );
    }

    /// Poll until the first-input anchor has reached the database, which
    /// is what fixes the capture window the planted record has to fall
    /// inside. The write is spawned off the input path by design, so
    /// reading the anchor synchronously would race it.
    async fn wait_for_first_input(sup: &Arc<Supervisor>, id: &str) -> i64 {
        let deadline = tokio::time::Instant::now() + TEST_DEADLINE;
        loop {
            if let Some(at) = sup
                .session_snapshot(id)
                .await
                .expect("snapshot")
                .expect("present")
                .first_input_at
            {
                return at;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "the first-input anchor never landed"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// The sampling half, against real panes: one that keeps printing
    /// accumulates observed CHANGE, one that prints nothing does not, and
    /// both are sampled.
    ///
    /// Both halves earn their place. Without the busy case the sampler
    /// could be doing nothing at all; without the quiet case it could be
    /// calling every pane changed — a capture that carried styling noise,
    /// or a baseline that counted the first sample as output — which would
    /// have item 2's classifier report every session running forever. The
    /// still pane is the negative evidence.
    #[tokio::test]
    async fn samples_accumulate_for_a_busy_pane_and_stay_quiet_for_a_still_one() {
        let state = StateDir::new();
        let sup = supervisor_with(&state, SupervisorSeams::default()).await;
        // Distinct text on every line: a loop echoing a CONSTANT would
        // fill the screen and then scroll identical rows past it,
        // producing a grid that never changes — a busy pane this test
        // would then call quiet.
        install_live_session(
            &sup,
            "busy",
            "i=0; while true; do i=$((i+1)); echo \"tick $i\"; sleep 0.05; done",
        )
        .await;
        install_live_session(&sup, "still", "sleep 300").await;

        let busy = sample_of(&sup, "busy").await;
        let still = sample_of(&sup, "still").await;
        let ticker = start_ticker(&sup);
        wait_until(
            "the still pane's quiet streak passed the classifier's threshold",
            || still.lock().expect("activity mutex").unchanged_streak >= 3,
        )
        .await;
        ticker.shutdown().await;

        let busy = busy.lock().expect("activity mutex");
        assert!(
            busy.samples > 1,
            "change can only be established by comparing two samples"
        );
        assert_eq!(
            busy.unchanged_streak, 0,
            "a pane printing a new line every 50ms must have changed at its most recent \
             comparison; a streak here is change detection that never fires"
        );
        assert!(
            busy.tail
                .as_deref()
                .is_some_and(|tail| tail.contains("tick")),
            "the tail is what the sharpeners read, so it has to carry the pane's real \
             text; got {:?}",
            busy.tail
        );
        let still = still.lock().expect("activity mutex");
        assert_eq!(
            still.unchanged_streak,
            still.samples - 1,
            "every comparison on a pane that printed nothing is an unchanged one — the streak \
             must count them all rather than resetting on capture noise"
        );
    }

    /// Shutdown is deterministic: once `shutdown` returns, no tick can
    /// still be in flight.
    ///
    /// The property every other ticker test leans on. They all assert
    /// against state a stray pass could still be mutating, and a
    /// `shutdown` that returned while a tick was mid-pass would make those
    /// assertions race a writer — intermittently, and only under load,
    /// which is the worst way for a test suite to be wrong.
    ///
    /// Not a leak test, whatever the sleep below might suggest: each
    /// `#[tokio::test]` gets its own runtime, which is torn down with the
    /// test, so a task this file forgot to stop could not outlive it
    /// anyway. What the sleep checks is the real property — that samples
    /// stop accumulating and STAY stopped, rather than one more pass
    /// landing after the handle said it was done.
    #[tokio::test]
    async fn shutdown_stops_the_ticker_and_waits_for_it() {
        let state = StateDir::new();
        let sup = supervisor_with(&state, SupervisorSeams::default()).await;
        install_live_session(&sup, "one", "sleep 300").await;
        let sample = sample_of(&sup, "one").await;

        let ticker = start_ticker(&sup);
        wait_until("the ticker sampled at least once", || {
            sample.lock().expect("activity mutex").samples > 0
        })
        .await;
        ticker.shutdown().await;

        let settled = sample.lock().expect("activity mutex").samples;
        tokio::time::sleep(TEST_INTERVAL * 6).await;
        assert_eq!(
            sample.lock().expect("activity mutex").samples,
            settled,
            "a stopped ticker must not still be sampling several intervals later"
        );
    }

    /// A ticker outliving its supervisor ends by itself.
    ///
    /// The `Weak` is not a stylistic choice: a task holding an `Arc` would
    /// keep the state directory's claim, the tmux driver, and the SQLite
    /// connection alive for as long as the runtime lived, so an embedder
    /// dropping a supervisor would silently keep one running against it.
    /// Proven WITHOUT sending the stop signal, because the stop signal
    /// would prove nothing about the upgrade.
    ///
    /// The drop deliberately waits for one tick to have COMPLETED first.
    /// Dropping immediately would let the loop end while still parked on
    /// its very first interval — a task that never reached the upgrade at
    /// all — and the assertion below would pass without the `Weak` being
    /// load-bearing in the slightest.
    #[tokio::test]
    async fn the_task_ends_when_its_supervisor_is_dropped() {
        let state = StateDir::new();
        let sup = supervisor_with(&state, SupervisorSeams::default()).await;
        install_live_session(&sup, "one", "sleep 300").await;
        let sample = sample_of(&sup, "one").await;
        let ticker = start_ticker(&sup);
        wait_until(
            "the ticker completed a pass against this supervisor",
            || sample.lock().expect("activity mutex").samples > 0,
        )
        .await;
        drop(sup);
        wait_until("the ticker noticed its supervisor was gone", || {
            ticker.is_finished()
        })
        .await;
        // `is_finished` alone cannot tell "ended because the upgrade
        // failed" from "ended by PANICKING", and a panic on the very first
        // tick would satisfy the wait above perfectly. `shutdown` consumes
        // the `JoinError`, so only a clean end passes.
        ticker.shutdown().await;
    }

    /// The whole of what one cell records, in sequence: the first look is
    /// not evidence, repeats accumulate, and any change resets.
    ///
    /// A unit test rather than a tmux one because each mistake it guards
    /// against is one character wide with a large blast radius. Counting
    /// sample one as quiet would start every session's decay a step in —
    /// including every session on a supervisor that just restarted.
    /// Failing to RESET on a change is worse in the other direction: a
    /// session that went quiet once would be idle forever, however busy it
    /// became afterwards, which is the "always wrong in the same
    /// direction" failure that makes a status column worthless.
    #[test]
    fn the_first_sample_is_not_evidence_and_any_change_resets_the_streak() {
        let mut sample = ActivitySample::default();

        sample.observe("hello".to_string());
        assert_eq!(sample.samples, 1);
        assert_eq!(
            sample.unchanged_streak, 0,
            "one observation cannot establish that anything stood still"
        );

        sample.observe("hello".to_string());
        assert_eq!(
            sample.unchanged_streak, 1,
            "the first COMPARISON is the first evidence of quiet"
        );
        sample.observe("hello".to_string());
        assert_eq!(sample.unchanged_streak, 2, "and they accumulate");

        sample.observe("hello world".to_string());
        assert_eq!(
            sample.unchanged_streak, 0,
            "a change resets the decay outright; the classifier reads this as 'how many times \
             running have I seen nothing new'"
        );
        assert_eq!(sample.tail.as_deref(), Some("hello world"));

        sample.observe("hello there".to_string());
        assert_eq!(sample.unchanged_streak, 0);
        assert_eq!(sample.samples, 5);
    }

    /// The scheduling rule, exhaustively, with no clock and no runtime:
    /// [`next_deadline`] is a pure function of three values, so the two
    /// properties it exists for can be asserted rather than timed.
    ///
    /// Timing them is what this replaces, and the replacement is the point.
    /// The previous version of this test counted how many passes a real
    /// ticker completed inside a two-second wall-clock window and compared
    /// that against a threshold — which measures the CI runner as much as
    /// the code. Roughly 600ms of descheduling (ordinary on a shared
    /// runner) was enough to fail it, and no amount of widening the
    /// threshold fixes that without also admitting the bug it was written
    /// to catch.
    ///
    /// The two properties, and the failure each one exists for:
    ///
    /// - ANCHORED. A pass that takes part of its interval must not push the
    ///   next tick out by however long it took. A `sleep(interval)` at the
    ///   end of each pass makes the real period `interval + work`, so a
    ///   supervisor whose passes take a moment quietly samples less often
    ///   than anyone configured, and the error compounds with the work
    ///   rather than staying bounded.
    /// - NEVER IMMEDIATE. A pass that overruns its interval must not be
    ///   followed instantly by the next one. Every deadline is already in
    ///   the past once work exceeds the period, so an anchor-only rule
    ///   schedules back-to-back passes forever — a supervisor spawning tmux
    ///   subprocesses as fast as it can retire them, on precisely the host
    ///   already too loaded to keep up.
    #[test]
    fn next_deadline_anchors_when_on_cadence_and_backs_off_when_overrunning() {
        const INTERVAL: Duration = Duration::from_millis(100);
        // A fixed origin, so every case below is arithmetic rather than an
        // observation: `Instant::now()` is read once and only DIFFERENCES
        // from it are ever asserted.
        let fired = tokio::time::Instant::now();
        let at = |millis: u64| fired + Duration::from_millis(millis);

        // On cadence, whatever fraction of the interval the pass used —
        // including a pass slow enough to cross the minimum-gap threshold,
        // which must NOT stretch a ticker that is keeping up.
        for work in [0, 1, 50, 90, 99] {
            assert_eq!(
                next_deadline(fired, at(work), INTERVAL),
                at(100),
                "a pass finishing {work}ms into a 100ms interval must keep the anchor"
            );
        }

        // Exactly at the deadline is already late: the anchor has arrived,
        // so the next one is a whole interval further on rather than now.
        assert_eq!(next_deadline(fired, at(100), INTERVAL), at(200));

        // Overrun, with the anchored candidate comfortably ahead: the phase
        // is kept and the gap needs no help.
        assert_eq!(next_deadline(fired, at(101), INTERVAL), at(200));
        // Overrun by several periods: the phase still survives — the next
        // tick lands on a multiple of the original anchor, not on
        // "now plus an interval".
        assert_eq!(next_deadline(fired, at(250), INTERVAL), at(300));

        // The case the minimum gap exists for: an overrunning pass that
        // finishes just before the next anchor would leave no pause at all.
        assert_eq!(
            next_deadline(fired, at(199), INTERVAL),
            at(199) + INTERVAL / MIN_TICK_GAP_DIVISOR,
            "a pass ending 1ms before its next anchor must still get a recovery pause"
        );
        // Sustained overrun, stated as the property rather than as a
        // number: however far behind the loop falls, the next pass is never
        // scheduled back to back with the one that just ended.
        for overrun in [100, 150, 199, 200, 201, 999, 1_000] {
            let now = at(overrun);
            let next = next_deadline(fired, now, INTERVAL);
            assert!(
                next >= now + INTERVAL / MIN_TICK_GAP_DIVISOR,
                "a pass ending {overrun}ms after its deadline was followed too closely"
            );
        }
    }

    /// The loop waits out the INJECTED interval, not the production one.
    ///
    /// A cadence seam that is read once and then ignored is a specific,
    /// plausible bug — the constant is right there, and every other test in
    /// this file passes with the seam wired or not, since they only ever
    /// wait for work to happen rather than for it not to. This is the
    /// negative: an interval well above the production
    /// [`TICKER_INTERVAL`], with an observation window comfortably past
    /// that constant, so a loop sleeping the production cadence samples
    /// here and fails.
    ///
    /// Real time rather than a paused clock, deliberately. Pausing makes
    /// tokio auto-advance to the next timer whenever the runtime is idle,
    /// which for a "nothing should have happened yet" assertion is exactly
    /// backwards: the clock would jump straight to the tick this test
    /// exists to prove has not arrived.
    #[tokio::test]
    async fn the_loop_waits_out_the_injected_interval_rather_than_the_production_one() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_seams(
            state.path(),
            dummy_exe(),
            SupervisorTimeouts::default(),
            SupervisorSeams {
                // Far above `TICKER_INTERVAL`, so the window below can be
                // longer than the production cadence and shorter than this.
                ticker_interval: Duration::from_secs(30),
                ..SupervisorSeams::default()
            },
        )
        .await
        .expect("supervisor");
        install_live_session(&sup, "one", "sleep 300").await;
        let sample = sample_of(&sup, "one").await;

        let ticker = start_ticker(&sup);
        // Longer than the production interval a hardcoded loop would use,
        // by enough that a loaded runner cannot explain the difference.
        tokio::time::sleep(TICKER_INTERVAL * 2).await;
        let samples = sample.lock().expect("activity mutex").samples;
        ticker.shutdown().await;
        assert_eq!(
            samples,
            0,
            "a ticker configured with a 30-second interval sampled within {:?}",
            TICKER_INTERVAL * 2
        );
    }

    /// A pass that finds no live panes leaves every previous sample
    /// exactly as it was, and the capture half of the tick runs anyway.
    ///
    /// The tempting bug is treating "tmux told us nothing" as "these
    /// sessions are quiet". Blanking or re-observing the cells here would
    /// make a supervisor whose tmux server vanished report its entire
    /// fleet as freshly sampled and unchanged — which decays to `Idle`
    /// within a few ticks, for every session, on the strength of no
    /// observation at all. Leaving the cells untouched means the
    /// classification simply stops moving, which is the honest answer for
    /// a pass that saw nothing.
    ///
    /// The capture half is asserted separately because the two halves of a
    /// tick are independent: conversation capture reads the filesystem and
    /// has no business stopping because tmux is unreachable.
    ///
    /// Scope note: killing the server exercises `pane_states`'
    /// DEFINITIVELY-EMPTY path (tmux answers "no server running", which the
    /// driver reports as an empty map), not its ERROR path. The two are
    /// deliberately different: an empty answer is an answer — those panes
    /// are gone, so nothing will be classified from them — while a failed
    /// query is no answer at all and additionally invalidates every
    /// retained screen (`a_failed_sample_stops_the_session_being_sharpened_
    /// from_a_stale_screen`, which reaches that path through
    /// `SupervisorSeams::sample_fault`). The empty case also resets the
    /// rotation cursor, where a failed query leaves it alone.
    #[tokio::test]
    async fn a_pass_that_finds_no_live_panes_preserves_every_sample_and_still_captures() {
        let state = StateDir::new();
        let sup = supervisor_with(&state, SupervisorSeams::default()).await;
        install_live_session(&sup, "one", "sleep 300").await;
        let sample = sample_of(&sup, "one").await;

        let (_stop, mut stop) = never_stopped();
        let mut cursor: SampleCursor = None;
        sample_pass(&sup, &mut cursor, SAMPLE_TAIL_BUDGET, &mut stop).await;
        let (samples, tail) = {
            let sample = sample.lock().expect("activity mutex");
            (sample.samples, sample.tail.clone())
        };
        assert_eq!(samples, 1, "premise: the pass before the failure worked");
        assert_eq!(cursor.as_deref(), Some("one"));

        // The whole private tmux server, gone out from under the
        // supervisor — the shape a crashed or reaped server leaves behind.
        sup.tmux.kill_session("fh-one").await.expect(
            "the premise of this test is that the server really is gone; a kill that \
                     silently failed would leave the pass below succeeding for the wrong reason",
        );
        // Past the tick's own suppression window, so the capture half
        // below is skipped only if the probe failure stopped it — the
        // thing this asserts — rather than because the supervisor's
        // construction-time pass was still recent (`CaptureReason::Tick`).
        tokio::time::sleep(TEST_INTERVAL * 3).await;
        let baseline = sup.capture_passes_completed();
        tick(&sup, &mut cursor, SAMPLE_TAIL_BUDGET, &mut stop).await;

        {
            let after = sample.lock().expect("activity mutex");
            assert_eq!(
                after.samples, samples,
                "a pass that found no live pane must not record an observation"
            );
            assert_eq!(
                after.tail, tail,
                "and must leave the previous screen in place rather than blanking it"
            );
            assert_eq!(
                after.unchanged_streak, 0,
                "least of all may it count as a quiet look; that is how a vanished tmux \
                 would decay a whole fleet to idle"
            );
        }
        assert!(
            sup.capture_passes_completed() > baseline,
            "the capture half of a tick does not depend on tmux at all"
        );
    }

    /// A pane the entry cannot prove belongs to it is not sampled — the
    /// identity cross-check `session_status` documents, enforced here so a
    /// recycled pane id cannot write one session's screen into another
    /// session's sample.
    ///
    /// Driven through the real pass with a deliberately mismatched entry,
    /// because the filter is one `&&` away from silently passing
    /// everything and no end state would show it.
    #[tokio::test]
    async fn a_pane_under_another_sessions_name_is_not_sampled() {
        let state = StateDir::new();
        let sup = supervisor_with(&state, SupervisorSeams::default()).await;
        let pane = spawn_pane(&sup, "fh-real", "sleep 300").await;
        // The pane genuinely exists; the entry simply remembers it under
        // a session name it does not have — the shape a pane-id reset
        // after a tmux server restart produces.
        install_entry(
            &sup,
            "one",
            Terminal {
                tmux_name: "fh-somebody-else".to_string(),
                pane,
            },
        )
        .await;
        let sample = sample_of(&sup, "one").await;

        let (_stop, mut stop) = never_stopped();
        // A cursor naming a session that is not in this population at all:
        // resuming means "the first one ordered after it", which is well
        // defined whether or not that session still exists.
        let mut cursor: SampleCursor = Some("zzz-gone".to_string());
        sample_pass(&sup, &mut cursor, SAMPLE_TAIL_BUDGET, &mut stop).await;
        assert_eq!(
            sample.lock().expect("activity mutex").samples,
            0,
            "a pane whose session name does not match must not be sampled at all"
        );
        assert_eq!(
            cursor, None,
            "a pass with nothing eligible must RESET the rotation rather than leave a resume \
             point that a later, different population would start partway into"
        );
    }

    /// Rotation covers an OVER-budget population across successive
    /// passes, and starts a replacement population from its head.
    ///
    /// The budget is a parameter precisely so this is reachable: pinning
    /// it through the production constant would mean standing up
    /// seventeen real tmux sessions, a cost out of all proportion to the
    /// property. Driving three sessions at a budget of one is the same
    /// arithmetic with the same failure modes — a cursor that never
    /// advances (every pass resampling the head, starving the tail
    /// forever) or one that advances without wrapping. The churn half is
    /// the population-replacement bug: a cursor carried across a session
    /// set that no longer exists would make the first pass over its
    /// replacement start partway in.
    #[tokio::test]
    async fn an_over_budget_population_is_covered_by_rotation_across_passes() {
        let state = StateDir::new();
        let sup = supervisor_with(&state, SupervisorSeams::default()).await;
        for id in ["a", "b", "c"] {
            install_live_session(&sup, id, "sleep 300").await;
        }
        let samples = |id: &str| {
            let sup = Arc::clone(&sup);
            let id = id.to_string();
            async move { sample_of(&sup, &id).await }
        };
        let (a, b, c) = (samples("a").await, samples("b").await, samples("c").await);

        // A budget of one against three sessions: the interesting case,
        // and one that would otherwise need seventeen real tmux sessions
        // to reach through the production constant.
        let (_stop, mut stop) = never_stopped();
        let mut cursor: SampleCursor = None;
        for expected in ["a", "b", "c"] {
            sample_pass(&sup, &mut cursor, 1, &mut stop).await;
            // The cursor names the session just sampled, so a rotation that
            // stalled would repeat a name here rather than advancing
            // through the id order. The per-session counts below are what
            // prove the wrap; this is what localizes a failure to the
            // rotation rather than to the sampling.
            assert_eq!(
                cursor.as_deref(),
                Some(expected),
                "the cursor must name the session this pass sampled"
            );
        }
        for (id, cell) in [("a", &a), ("b", &b), ("c", &c)] {
            assert_eq!(
                cell.lock().expect("activity mutex").samples,
                1,
                "three passes at a budget of one must have covered {id} exactly once — a \
                 rotation that stalled would resample the head and starve the tail"
            );
        }

        // PARTIAL churn, which is the case an index-based cursor gets
        // wrong and the reason this cursor is an id. Deleting a session
        // ordered BEFORE the resume point shifts every later session one
        // place toward the front of the sorted list, so an index would
        // resume one position too far in — and under steady churn ahead of
        // it, the tail of the order is stepped over indefinitely and simply
        // stops being sampled, with nothing anywhere reporting a problem.
        //
        // The population deliberately never empties here: the reset on an
        // empty map would paper over exactly the bug being tested.
        sup.sessions.lock().await.remove("a");
        sample_pass(&sup, &mut cursor, 1, &mut stop).await;
        assert_eq!(
            cursor.as_deref(),
            Some("b"),
            "after c, the rotation wraps to the head of what REMAINS — not to whatever session \
             now happens to sit at the old index"
        );
        assert_eq!(
            b.lock().expect("activity mutex").samples,
            2,
            "b is the head of the surviving order and must be the one sampled"
        );
        assert_eq!(
            c.lock().expect("activity mutex").samples,
            1,
            "and c must not be resampled ahead of it"
        );

        // Full REPLACEMENT still resets: an empty map has nothing to
        // rotate through, and a resume point carried into a fresh
        // population is not what its first pass should use.
        sup.sessions.lock().await.clear();
        sample_pass(&sup, &mut cursor, 1, &mut stop).await;
        assert_eq!(
            cursor, None,
            "an empty map resets the rotation; carrying the old resume point would make the \
             first pass over a REPLACEMENT population start somewhere arbitrary"
        );
        install_live_session(&sup, "z", "sleep 300").await;
        let z = sample_of(&sup, "z").await;
        sample_pass(&sup, &mut cursor, 1, &mut stop).await;
        assert_eq!(
            z.lock().expect("activity mutex").samples,
            1,
            "the first session of a fresh population must be sampled by the first pass over it"
        );
    }

    /// The cursor names a POSITION IN THE ID ORDER, not a session that has
    /// to still be there: a resume point whose session disappeared resumes
    /// at the next greater id, never at the head.
    ///
    /// This is the branch the `partition_point` search exists for, and it is
    /// invisible in the ordinary rotation test because there the cursor
    /// always names a live session. The wrong implementation it excludes is
    /// the tempting one — look the id up, and start over from the front when
    /// it is gone — which under steady churn restarts the rotation every
    /// time a sampled session is deleted, and never reaches the tail of the
    /// order at all.
    ///
    /// The cursor is deliberately parked in the MIDDLE of the population
    /// before the deletion. Parking it at the head would make "resume after
    /// the missing id" and "start over from the head" the same answer, and
    /// the test would pass either way.
    #[tokio::test]
    async fn a_cursor_whose_session_vanished_resumes_at_the_next_greater_id() {
        let state = StateDir::new();
        let sup = supervisor_with(&state, SupervisorSeams::default()).await;
        for id in ["a", "b", "c"] {
            install_live_session(&sup, id, "sleep 300").await;
        }
        let (a, b, c) = (
            sample_of(&sup, "a").await,
            sample_of(&sup, "b").await,
            sample_of(&sup, "c").await,
        );

        let (_stop, mut stop) = never_stopped();
        let mut cursor: SampleCursor = None;
        for expected in ["a", "b"] {
            sample_pass(&sup, &mut cursor, 1, &mut stop).await;
            assert_eq!(
                cursor.as_deref(),
                Some(expected),
                "premise: the rotation walks the id order one session per pass"
            );
        }

        // The session the cursor names goes away while everything ordered
        // after it stays.
        sup.sessions.lock().await.remove("b");
        sample_pass(&sup, &mut cursor, 1, &mut stop).await;

        assert_eq!(
            cursor.as_deref(),
            Some("c"),
            "the rotation resumes after the position b HELD, not at the head of what remains"
        );
        assert_eq!(
            c.lock().expect("activity mutex").samples,
            1,
            "c is the next session in the order and must be the one sampled"
        );
        assert_eq!(
            a.lock().expect("activity mutex").samples,
            1,
            "a must not be resampled: a missing cursor is not a reason to start over"
        );
        drop(b);
    }

    /// A capture that FAILS still advances the rotation, so the next pass
    /// moves on rather than retrying the same session forever.
    ///
    /// The cursor is written before the capture is attempted, and that
    /// ordering is the whole point: a pane that has become unreadable —
    /// wedged tmux, a pane vanishing between the liveness probe and the
    /// capture — would otherwise pin the rotation to itself. At a budget of
    /// one that starves every other session outright; at the production
    /// budget it costs one slot per pass indefinitely. Either way nothing
    /// reports a problem, because a failed capture is logged at DEBUG by
    /// design.
    ///
    /// The fault is aimed at ONE session and then lifted, so the pass after
    /// it is an ordinary successful one: an implementation that advanced
    /// only on success would resample the failed session there, which is
    /// exactly what the assertions below refuse.
    #[tokio::test]
    async fn a_failed_capture_still_advances_the_rotation() {
        let state = StateDir::new();
        // Aimed at "b" alone: the pass-wide `pane_states` probe must keep
        // working, or nothing would be selected at all and the branch under
        // test would never be reached.
        let failing: Arc<AtomicBool> = Arc::default();
        let sup = {
            let failing = Arc::clone(&failing);
            supervisor_with(
                &state,
                SupervisorSeams {
                    sample_fault: Some(Arc::new(move |asked| {
                        let asked_about_b =
                            matches!(asked, SampleRead::Tail { session } if session == "b");
                        (asked_about_b && failing.load(Ordering::SeqCst))
                            .then(|| "injected capture failure".to_string())
                    })),
                    ..SupervisorSeams::default()
                },
            )
            .await
        };
        for id in ["a", "b", "c"] {
            install_live_session(&sup, id, "sleep 300").await;
        }
        let (b, c) = (sample_of(&sup, "b").await, sample_of(&sup, "c").await);

        let (_stop, mut stop) = never_stopped();
        let mut cursor: SampleCursor = None;
        sample_pass(&sup, &mut cursor, 1, &mut stop).await;
        assert_eq!(cursor.as_deref(), Some("a"), "premise: the head is first");

        failing.store(true, Ordering::SeqCst);
        sample_pass(&sup, &mut cursor, 1, &mut stop).await;
        assert_eq!(
            cursor.as_deref(),
            Some("b"),
            "a pass whose capture failed must still leave the rotation past the session it \
             selected"
        );
        assert_eq!(
            b.lock().expect("activity mutex").samples,
            0,
            "premise: the injected fault means b really was not observed"
        );

        failing.store(false, Ordering::SeqCst);
        sample_pass(&sup, &mut cursor, 1, &mut stop).await;
        assert_eq!(
            c.lock().expect("activity mutex").samples,
            1,
            "the pass after the failure moves on to c rather than retrying b"
        );
        assert_eq!(
            b.lock().expect("activity mutex").samples,
            0,
            "and b is not resampled ahead of it, however readable it has become"
        );
    }

    /// A prompt that has since been ANSWERED must not hold its session at
    /// `Waiting` through a run of failing captures — driven through
    /// `sample_pass` with an injected failure, not by calling
    /// `forget_tail` by hand.
    ///
    /// The distinction is the whole point of the seam. A direct call
    /// asserts that `forget_tail` does what it says; this asserts that the
    /// SAMPLER calls it, which is the half that regresses. A change that
    /// dropped the invalidation from the failure path would leave a
    /// unit-level test perfectly green while every session whose captures
    /// started failing sat at `Waiting` forever.
    ///
    /// Both failure paths, because they are separately reachable and
    /// separately wrong: one session's own capture failing while the pass
    /// otherwise succeeds, and the server-wide liveness probe failing so
    /// the pass learns nothing about anything. The second is the one a
    /// user actually notices, since the LIST path's own probe can keep
    /// succeeding — so the session stays live, stays sharpened, and keeps
    /// reporting a question that was answered long ago.
    ///
    /// ## And the RECOVERY, which is the half a forget can silently break
    ///
    /// Invalidating the tail is only correct if a later successful capture
    /// puts the session back on its feet, and the way that works is subtle
    /// enough to be worth pinning: `forget_tail` clears the screen but
    /// deliberately leaves `samples` alone, so the next `observe` still
    /// takes the comparison branch — comparing against `None`, finding a
    /// difference, and resetting the streak to zero. Two plausible
    /// implementations get this wrong in opposite directions. One treats a
    /// missing tail as "unchanged" and increments the streak, so a session
    /// that lost its screen keeps decaying toward `Idle` on no evidence.
    /// The other leaves the pre-failure streak standing, so a session that
    /// was nine quiet samples deep stays `Idle` through a screen that has
    /// visibly changed. The continuation below fails on both.
    #[tokio::test]
    async fn a_failed_sample_stops_the_session_being_sharpened_from_a_stale_screen() {
        for read in ["tail", "pane_states"] {
            let state = StateDir::new();
            let failing: Arc<AtomicBool> = Arc::default();
            let sup = {
                let failing = Arc::clone(&failing);
                supervisor_with(
                    &state,
                    SupervisorSeams {
                        sample_fault: Some(Arc::new(move |asked| {
                            let asked_about = matches!(
                                (read, asked),
                                ("tail", SampleRead::Tail { .. })
                                    | ("pane_states", SampleRead::PaneStates)
                            );
                            (asked_about && failing.load(Ordering::SeqCst))
                                .then(|| "injected sampling failure".to_string())
                        })),
                        ..SupervisorSeams::default()
                    },
                )
                .await
            };
            // A live claude session showing an approval dialog. The pane
            // itself only has to be alive; what is classified is the
            // TAIL, which the sample below writes.
            install_live_session(&sup, "one", "sleep 300").await;
            let entry = {
                let sessions = sup.sessions.lock().await;
                let entry = sessions.get("one").cloned().expect("installed");
                drop(sessions);
                Arc::new(SessionEntry {
                    snapshot: IntegrationSnapshot {
                        kind: AgentKind::Claude,
                        resume_template: None,
                    },
                    info: entry.info.clone(),
                    terminal: entry.terminal.clone(),
                    outcome: Arc::clone(&entry.outcome),
                    canonical_cwd: entry.canonical_cwd.clone(),
                    first_input: Arc::clone(&entry.first_input),
                    capture: Arc::clone(&entry.capture),
                    activity: Arc::clone(&entry.activity),
                    generation: entry.generation,
                    scope: entry.scope.clone(),
                })
            };
            sup.sessions
                .lock()
                .await
                .insert("one".to_string(), Arc::clone(&entry));
            {
                let mut activity = entry.activity.lock().expect("activity mutex");
                activity.samples = 9;
                activity.unchanged_streak = 9;
                activity.tail = Some(CLAUDE_APPROVAL_DIALOG.to_string());
            }
            let states = sup.tmux.pane_states().await.expect("probe");
            assert_eq!(
                session_status(&entry, &states).0,
                SessionStatus::Waiting,
                "premise ({read}): the screen on file is an unanswered prompt"
            );

            failing.store(true, Ordering::SeqCst);
            let (_stop, mut stop) = never_stopped();
            let mut cursor: SampleCursor = None;
            sample_pass(&sup, &mut cursor, SAMPLE_TAIL_BUDGET, &mut stop).await;

            assert_ne!(
                session_status(&entry, &states).0,
                SessionStatus::Waiting,
                "a {read} failure must invalidate the screen it can no longer confirm"
            );
            {
                let activity = entry.activity.lock().expect("activity mutex");
                assert_eq!(
                    (activity.samples, activity.unchanged_streak),
                    (9, 9),
                    "and must not count as an observation, least of all a quiet one ({read})"
                );
                assert_eq!(
                    activity.tail, None,
                    "premise ({read}): the screen really is gone, not merely unsharpened"
                );
            }

            // Recovery: the reads start working again and one ordinary pass
            // has to put the session back on its feet.
            failing.store(false, Ordering::SeqCst);
            sample_pass(&sup, &mut cursor, SAMPLE_TAIL_BUDGET, &mut stop).await;

            let activity = entry.activity.lock().expect("activity mutex");
            assert_eq!(
                activity.samples, 10,
                "the recovering pass is one observation, not a re-baseline ({read})"
            );
            assert!(
                activity.tail.is_some(),
                "and it restores the screen the failure dropped ({read})"
            );
            assert_eq!(
                activity.unchanged_streak, 0,
                "a capture that had nothing to compare against is a CHANGE, not a quiet look: \
                 carrying the pre-failure streak forward would hold a moved screen at Idle, and \
                 treating the missing tail as unchanged would decay it on no evidence ({read})"
            );
            drop(activity);
            assert_eq!(
                session_status(&entry, &states).0,
                SessionStatus::Running,
                "so the session classifies from what is on the pane NOW, not from the dialog it \
                 was showing before the failures ({read})"
            );
        }
    }

    /// A Claude approval dialog as it renders at the bottom of a pane —
    /// enough of the shape for the recognizer, whose own fixtures live in
    /// `agent_kind`.
    const CLAUDE_APPROVAL_DIALOG: &str = "\
╭───────────────────────────────────────────────╮
│ Do you want to run this command?              │
│                                               │
│ ❯ 1. Yes                                      │
│   2. No, and tell Claude what to do instead   │
╰───────────────────────────────────────────────╯";

    /// A stop the ticker never receives, for tests that drive
    /// [`sample_pass`] directly.
    ///
    /// The SENDER is returned rather than dropped, and callers must bind
    /// it: dropping it closes the channel, which IS the stop signal, and a
    /// pass that believed it had been cancelled would sample nothing and
    /// fail every assertion below for the wrong reason.
    fn never_stopped() -> (oneshot::Sender<()>, oneshot::Receiver<()>) {
        oneshot::channel()
    }

    /// A durable session row claiming an existing tmux session, for the
    /// tests that must go through a REAL reload rather than planting an
    /// entry in the map.
    ///
    /// `pane` is left empty on purpose: reload's by-name rediscovery is
    /// what then attaches the live pane, which is the same path a
    /// supervisor takes over a session it did not launch itself.
    async fn insert_row(sup: &Arc<Supervisor>, id: &str, tmux_name: &str) {
        sup.store
            .insert_session(
                StoredSession {
                    id: id.to_string(),
                    parent: None,
                    archived: false,
                    title: id.to_string(),
                    created_at: now_unix(),
                    creation_seq: 0,
                    cwd: "/tmp".to_string(),
                    invocation: "agent".to_string(),
                    tmux_name: tmux_name.to_string(),
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
                    source_profile: None,
                },
                None,
            )
            .await
            .expect("insert the row that claims that tmux session");
    }

    /// A [`super::super::core::CaptureGate`] that parks ONE capture pass —
    /// the first after it is armed — until the test releases it.
    ///
    /// The barrier both scheduling tests need, since the window they are
    /// about (what a second caller does while a first pass is in flight)
    /// is far too short to observe in a real pass over a fixture tree.
    ///
    /// # Why arming is a separate step
    ///
    /// `Supervisor::new_with_seams` runs a capture pass of its OWN, inside
    /// the constructor, before it ever returns the supervisor. A barrier
    /// that armed itself on creation would therefore trap CONSTRUCTION:
    /// `supervisor_with` would never return, the test would hang rather
    /// than fail, and nothing in the assertion text would point at why.
    /// (It did, which is how this note came to be written.) Arming after
    /// the supervisor exists is what aims the barrier at the pass the test
    /// actually means.
    ///
    /// One-shot on purpose: later passes fall straight through, because
    /// every test here releases the barrier and then asserts on what
    /// happens NEXT.
    struct PassBarrier {
        gate: super::super::core::CaptureGate,
        entered: Arc<AtomicBool>,
        /// Where an armed barrier parks the pass. Empty until `arm`.
        slot: Arc<std::sync::Mutex<Option<oneshot::Receiver<()>>>>,
        receiver: Option<oneshot::Receiver<()>>,
        release: Option<oneshot::Sender<()>>,
    }

    impl PassBarrier {
        fn new() -> PassBarrier {
            let (release, receiver) = oneshot::channel::<()>();
            let entered = Arc::new(AtomicBool::new(false));
            let slot: Arc<std::sync::Mutex<Option<oneshot::Receiver<()>>>> =
                Arc::new(std::sync::Mutex::new(None));
            let gate_entered = Arc::clone(&entered);
            let gate_slot = Arc::clone(&slot);
            let gate: super::super::core::CaptureGate = Arc::new(move || {
                // Taken, not peeked: the seam is an `Fn` that may run many
                // times while the receiver can only be awaited once.
                let waiting = gate_slot.lock().expect("barrier slot poisoned").take();
                let entered = Arc::clone(&gate_entered);
                Box::pin(async move {
                    if let Some(waiting) = waiting {
                        entered.store(true, Ordering::SeqCst);
                        let _ = waiting.await;
                    }
                })
            });
            PassBarrier {
                gate,
                entered,
                slot,
                receiver: Some(receiver),
                release: Some(release),
            }
        }

        /// The seam to hand to `SupervisorSeams`. Inert until [`Self::arm`].
        fn gate(&self) -> super::super::core::CaptureGate {
            Arc::clone(&self.gate)
        }

        /// Aim the barrier at the next capture pass to begin.
        fn arm(&mut self) {
            *self.slot.lock().expect("barrier slot poisoned") = self.receiver.take();
        }

        /// Whether a pass is parked in the barrier right now. Polled by
        /// the tests rather than slept on, so they are not timing guesses.
        fn entered(&self) -> bool {
            self.entered.load(Ordering::SeqCst)
        }

        fn release(&mut self) {
            if let Some(release) = self.release.take() {
                let _ = release.send(());
            }
        }
    }

    /// SPEC.md's status rule, at the one place it can actually be
    /// violated: a saturated SAMPLER must not delay a request.
    ///
    /// The regression this pins is subtle and was shipped once. The
    /// sampler originally drew its permit from the same
    /// `HANDLER_ADMISSION_PERMITS` semaphore the slow handlers use, which
    /// reads as prudent — it is the same tmux subprocesses being bounded —
    /// but a permit held by periodic work is a permit a REQUEST cannot
    /// have, and a request that parks inside `handle_control` parks
    /// `handle_connection`'s read loop, which is the loop that dispatches
    /// keystrokes. Status detection would then be delaying terminal input,
    /// which SPEC.md forbids outright.
    ///
    /// Exhausting the sampler's limiter and then driving a real request
    /// through `handle_control` is the smallest thing that tells the two
    /// designs apart: with a shared semaphore this deadlocks, with
    /// separate ones it replies.
    #[tokio::test]
    async fn a_saturated_sampler_cannot_delay_a_request() {
        let state = StateDir::new();
        let sup = supervisor_with(&state, SupervisorSeams::default()).await;
        // Every sampling permit, held for the whole test: the sampler is
        // as wedged as it can be.
        let held = Arc::clone(&sup.sampling_admission)
            .acquire_many_owned(SAMPLING_ADMISSION_PERMITS as u32)
            .await
            .expect("sampling semaphore is never closed");

        // A tick cannot even start now, which is the premise.
        let (_stop, mut stop) = never_stopped();
        let mut cursor: SampleCursor = None;
        assert!(
            tokio::time::timeout(
                Duration::from_millis(200),
                tick(&sup, &mut cursor, SAMPLE_TAIL_BUDGET, &mut stop)
            )
            .await
            .is_err(),
            "test premise: with the sampling permits held, a tick must be unable to proceed"
        );

        // And a request served through the real control path is
        // completely unaffected.
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut uploads = no_uploads();
        let mut tasks = tokio::task::JoinSet::new();
        tokio::time::timeout(
            Duration::from_secs(10),
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
                    upload_routes: &mut uploads,
                    tasks: &mut tasks,
                },
            ),
        )
        .await
        .expect("a request must not wait on the sampler's limiter");
        // `ListSessions` is admitted then SPAWNED, so the reply arrives on
        // the writer channel rather than from `handle_control` itself.
        let frame = tokio::time::timeout(Duration::from_secs(10), rx.recv())
            .await
            .expect("the reply must not wait on the sampler's limiter")
            .expect("a reply frame");
        let decoded: ControlMsg = serde_json::from_slice(&frame.body).expect("decode");
        assert!(
            matches!(decoded, ControlMsg::SessionList { .. }),
            "the request must be answered normally, got {decoded:?}"
        );
        drop(held);
    }

    /// The capture coordinator's whole point: a tick that lands on top of
    /// a reply-producing pass yields ONE sweep, not two.
    ///
    /// Driven through the capture gate rather than by racing two real
    /// passes, because the window under test is exactly the one a real
    /// pass over a small fixture tree closes too fast to observe. The
    /// counter is the only witness available — two passes over identical
    /// evidence leave identical capture state behind, so nothing about a
    /// session could tell the difference.
    ///
    /// A regression that removed the coalescing (a tick that waited for
    /// the lock instead of skipping) fails this on the timeout rather than
    /// on the count, which is why the tick is driven under one.
    #[tokio::test]
    async fn a_tick_landing_on_an_in_flight_pass_adds_no_second_sweep() {
        let state = StateDir::new();
        let home = tempfile::tempdir().expect("agent home");
        let mut barrier = PassBarrier::new();
        let sup = supervisor_with(
            &state,
            SupervisorSeams {
                agent_home: Some(home.path().to_path_buf()),
                capture_gate: Some(barrier.gate()),
                ..SupervisorSeams::default()
            },
        )
        .await;
        // Armed only now: the constructor has already run a pass of its
        // own, and an earlier arming would have trapped that one.
        barrier.arm();
        let baseline = sup.capture_passes_completed();

        let blocked = {
            let sup = Arc::clone(&sup);
            tokio::spawn(async move { sup.capture_now().await })
        };
        // Wait until a pass is genuinely parked rather than sleeping on an
        // assumption about scheduling.
        wait_until("a capture pass is parked in the gate", || barrier.entered()).await;

        let (_stop, mut stop) = never_stopped();
        let mut cursor: SampleCursor = None;
        tokio::time::timeout(
            Duration::from_secs(10),
            tick(&sup, &mut cursor, SAMPLE_TAIL_BUDGET, &mut stop),
        )
        .await
        .expect("a tick must SKIP a pass in flight, never queue behind it");
        assert_eq!(
            sup.capture_passes_completed(),
            baseline,
            "the tick must not have completed a sweep of its own"
        );

        barrier.release();
        blocked.await.expect("the blocked pass must finish");
        assert_eq!(
            sup.capture_passes_completed(),
            baseline + 1,
            "exactly one sweep total: the one the reply-producing caller ran"
        );
    }

    /// The two halves of the scheduling rule, stated as counts.
    ///
    /// A reply-producing caller ALWAYS gets a pass that began after it
    /// asked — that is what the helm's post-write wake rests on, since
    /// proto v10 gives the supervisor edge no push and a drain replying
    /// off an older sweep would describe the world before the write it is
    /// racing. A tick, by contrast, has nothing to add when a pass
    /// finished within its interval, and must say so by not sweeping.
    #[tokio::test]
    async fn replies_always_sweep_while_a_tick_suppresses_itself_after_a_recent_pass() {
        let state = StateDir::new();
        let home = tempfile::tempdir().expect("agent home");
        let sup = supervisor_with(
            &state,
            SupervisorSeams {
                agent_home: Some(home.path().to_path_buf()),
                ..SupervisorSeams::default()
            },
        )
        .await;

        let baseline = sup.capture_passes_completed();
        sup.capture_now().await;
        sup.capture_now().await;
        assert_eq!(
            sup.capture_passes_completed(),
            baseline + 2,
            "back-to-back reply-producing callers each need their OWN pass; reusing the \
             previous one would answer a request from evidence older than itself"
        );

        let after_replies = sup.capture_passes_completed();
        sup.capture_pass_for(CaptureReason::Tick {
            suppress_within: Duration::from_secs(3600),
        })
        .await;
        assert_eq!(
            sup.capture_passes_completed(),
            after_replies,
            "a tick has nothing to add right after a pass completed"
        );

        sup.capture_pass_for(CaptureReason::Tick {
            suppress_within: Duration::ZERO,
        })
        .await;
        assert_eq!(
            sup.capture_passes_completed(),
            after_replies + 1,
            "and it sweeps once that window has elapsed"
        );

        // A tick must NOT be suppressed by its OWN previous pass. This is
        // the bug the `reply_completed`/`started` split exists to prevent:
        // with nothing else sweeping, the previous tick's pass is always
        // about one interval old when the next tick fires, so counting it
        // would have the ticker skip every other tick and quietly halve
        // the unattended capture cadence the whole task exists to
        // guarantee.
        //
        // Discriminating between the two designs needs a real time gap:
        // the reply-driven passes above must age OUT of the window while
        // the tick under test stays well inside it. Then a tick that
        // counted its own predecessor would suppress and one that does not
        // will sweep.
        let window = Duration::from_millis(200);
        tokio::time::sleep(window * 2).await;
        let aged = sup.capture_passes_completed();
        sup.capture_pass_for(CaptureReason::Tick {
            suppress_within: window,
        })
        .await;
        assert_eq!(
            sup.capture_passes_completed(),
            aged + 1,
            "test premise: with no recent reply-driven pass, a tick sweeps"
        );
        sup.capture_pass_for(CaptureReason::Tick {
            suppress_within: window,
        })
        .await;
        assert_eq!(
            sup.capture_passes_completed(),
            aged + 2,
            "a tick immediately after ANOTHER TICK must still sweep — only a reply-driven \
             pass makes a tick redundant, or the unattended cadence silently halves"
        );
    }

    /// Shutdown SHIELDS a capture pass that has already begun.
    ///
    /// The rule the module doc states and the reason the stop is
    /// cooperative rather than an abort: a pass writes durable state and
    /// then mirrors it in memory, and a shutdown that tore it in half
    /// would leave the two disagreeing until something re-derived them.
    /// So `shutdown` must PEND while a pass is in flight and complete
    /// after it, with the pass counted as having finished.
    #[tokio::test]
    async fn shutdown_waits_out_a_capture_pass_already_in_flight() {
        let state = StateDir::new();
        let home = tempfile::tempdir().expect("agent home");
        let mut barrier = PassBarrier::new();
        let sup = supervisor_with(
            &state,
            SupervisorSeams {
                agent_home: Some(home.path().to_path_buf()),
                capture_gate: Some(barrier.gate()),
                ..SupervisorSeams::default()
            },
        )
        .await;
        // After construction, whose own capture pass would otherwise be
        // the one that got trapped — see `PassBarrier`.
        barrier.arm();
        let baseline = sup.capture_passes_completed();

        let ticker = start_ticker(&sup);
        // The ticker's first tick parks in the gate mid-capture. Polled,
        // not slept on: a sleep that guessed short would assert against a
        // ticker that had not started its pass yet and "prove" the
        // shielding for the wrong reason.
        wait_until("the ticker's capture pass reached the gate", || {
            barrier.entered()
        })
        .await;

        let mut shutting_down = Box::pin(ticker.shutdown());
        assert!(
            tokio::time::timeout(TEST_INTERVAL * 4, &mut shutting_down)
                .await
                .is_err(),
            "shutdown must not return while a capture pass is still in flight"
        );
        barrier.release();
        tokio::time::timeout(Duration::from_secs(30), shutting_down)
            .await
            .expect("shutdown must complete once the pass it was shielding finishes");
        assert_eq!(
            sup.capture_passes_completed(),
            baseline + 1,
            "the shielded pass must have run to COMPLETION rather than being torn in half by \
             the shutdown that was waiting on it"
        );
    }

    /// `serve` really does start the ticker.
    ///
    /// Every other test in this file calls `start_ticker` directly, so all
    /// of them would keep passing if the one line wiring it into `serve`
    /// were deleted — and the feature would be entirely absent from
    /// production. This drives the real entry point and makes no requests
    /// at all, so the only thing that can move the sample is the ticker
    /// `serve` owns.
    #[tokio::test]
    async fn serve_starts_the_ticker_without_any_request_arriving() {
        let state = StateDir::new();
        let sup = supervisor_with(&state, SupervisorSeams::default()).await;
        // Through the STORE, not the in-memory map: `serve` reloads the
        // map wholesale before it starts serving, so a hand-inserted entry
        // would be replaced by the row set. The reload's own pane
        // rediscovery is what gives the entry a live terminal.
        let pane_owner = spawn_pane(&sup, "fh-served", "sleep 300").await;
        assert!(!pane_owner.is_empty(), "the fixture pane must exist");
        insert_row(&sup, "served", "fh-served").await;

        let serving = Arc::clone(&sup);
        // `serve` never returns, so the task is abandoned deliberately;
        // `StateDir`'s own drop is what reaps the tmux server afterwards.
        let served = tokio::spawn(async move { serving.serve().await });

        wait_until("serve's own ticker sampled the session", || {
            served.is_finished()
                || sup
                    .sessions
                    .try_lock()
                    .ok()
                    .and_then(|sessions| {
                        sessions
                            .get("served")
                            .map(|entry| entry.activity.lock().expect("activity mutex").samples > 0)
                    })
                    .unwrap_or(false)
        })
        .await;
        assert!(
            !served.is_finished(),
            "serve must still be running; it failed with {:?}",
            served.await
        );
        served.abort();
    }

    /// The whole chain, against real panes: a pane that keeps printing
    /// classifies `Running`, and one that prints nothing decays from the
    /// unwatched default to `Idle` — both through the real sampler and the
    /// real [`session_status`], with nothing hand-fed.
    ///
    /// The DECAY is what earns this test its cost. `status`'s own unit
    /// tests pin the unchanged-streak arithmetic exactly, but they build the
    /// sample cell themselves; only here does the transition depend on the ticker
    /// having genuinely watched a pane twice and concluded nothing moved.
    /// The pre-ticker assertion is what makes it a transition rather than
    /// a coincidence: before the first pass the same session classifies
    /// `Running`, so `Idle` below can only have come from being watched.
    #[tokio::test]
    async fn a_busy_pane_classifies_running_and_a_quiet_one_decays_to_idle() {
        let state = StateDir::new();
        let sup = supervisor_with(&state, SupervisorSeams::default()).await;
        // Distinct text per line, for the reason the sampling test above
        // spells out: a constant would scroll an unchanging grid past and
        // look quiet.
        install_live_session(
            &sup,
            "busy",
            "i=0; while true; do i=$((i+1)); echo \"tick $i\"; sleep 0.05; done",
        )
        .await;
        install_live_session(&sup, "still", "sleep 300").await;

        assert_eq!(
            classify(&sup, "still").await,
            SessionStatus::Running,
            "premise: a live session nothing has sampled yet is running, not idle"
        );

        let still = sample_of(&sup, "still").await;
        let ticker = start_ticker(&sup);
        wait_until(
            "the still pane's quiet streak reached the threshold",
            || still.lock().expect("activity mutex").unchanged_streak >= 3,
        )
        .await;
        assert_eq!(
            classify(&sup, "busy").await,
            SessionStatus::Running,
            "a pane printing continuously must still be running after the same number of \
             passes that made its quiet neighbour idle"
        );
        assert_eq!(
            classify(&sup, "still").await,
            SessionStatus::Idle,
            "a pane watched repeatedly with nothing printed is at rest"
        );
        ticker.shutdown().await;
    }

    /// The same decay, with more sessions than the budget can sample in
    /// one pass: the busy one stays `Running` even though several ticks
    /// pass between its own samples.
    ///
    /// The end-to-end half of the bug `QUIET_SAMPLES_BEFORE_IDLE` exists
    /// to prevent. Under a wall-clock window this is the shape that breaks:
    /// with the rotation stretching a session's effective sampling period,
    /// a continuously-changing pane goes quiet for longer than the window
    /// BETWEEN ITS OWN SAMPLES and flips to idle — a status decided by how
    /// many other sessions the host is running.
    ///
    /// Driven at a budget of one so three sessions are enough to be
    /// over-budget; the production constant would need seventeen real tmux
    /// sessions to reach the same arithmetic. Passes are driven directly
    /// rather than by the ticker so the test controls how many samples each
    /// session gets, with a gap between them big enough that the busy pane
    /// has genuinely printed since its previous look.
    #[tokio::test]
    async fn a_busy_pane_stays_running_when_the_rotation_samples_it_rarely() {
        let state = StateDir::new();
        let sup = supervisor_with(&state, SupervisorSeams::default()).await;
        install_live_session(
            &sup,
            "busy",
            "i=0; while true; do i=$((i+1)); echo \"tick $i\"; sleep 0.05; done",
        )
        .await;
        install_live_session(&sup, "quiet-a", "sleep 300").await;
        install_live_session(&sup, "quiet-b", "sleep 300").await;

        let (_stop, mut stop) = never_stopped();
        let mut cursor: SampleCursor = None;
        // Twelve passes at a budget of one: four samples each, which is one
        // more than the quiet sessions need to cross the threshold.
        for _ in 0..12 {
            sample_pass(&sup, &mut cursor, 1, &mut stop).await;
            // Longer than the busy pane's own 50ms print interval, so each
            // of its samples genuinely differs from the previous one.
            tokio::time::sleep(Duration::from_millis(80)).await;
        }

        assert_eq!(
            classify(&sup, "busy").await,
            SessionStatus::Running,
            "a pane that changed at every one of its own samples is working, however many \
             ticks its neighbours consumed in between"
        );
        for quiet in ["quiet-a", "quiet-b"] {
            assert_eq!(
                classify(&sup, quiet).await,
                SessionStatus::Idle,
                "{quiet} was watched four times and never changed"
            );
        }
    }

    /// A pending question on a real pane reaches the per-kind sharpener
    /// and classifies `Waiting` — the end-to-end path PLAN_M6_75.md item
    /// 2's user-visible promise rides on.
    ///
    /// Deliberately not the fake agent: these tests drive tmux directly
    /// (see [`spawn_pane`]), so the shortest honest fixture is a shell that
    /// prints the dialog and then waits, exactly as an agent blocked on an
    /// approval does. What this adds over `agent_kind`'s recognition tests
    /// is every step between them — capture-pane's rendering, the tail
    /// bound, the sample cell, the snapshot lookup that decides which
    /// sharpener applies.
    ///
    /// The pane is SILENT after printing, so its unchanged-sample streak has
    /// carried the baseline to idle by the time it is classified: `Waiting`
    /// here can only be a promotion by the sharpener, never the generic
    /// classifier's answer wearing a different name.
    ///
    /// The fixture draws a selection pointer at its first option, because
    /// the recognizer requires one — that is the signal separating a widget
    /// from an agent's numbered prose (see
    /// `agent_kind::looks_like_a_choice_prompt`), and a fixture without it
    /// would be asserting that a paragraph reads as a dialog.
    #[tokio::test]
    async fn a_prompt_on_a_real_pane_classifies_waiting_through_the_sampler() {
        let state = StateDir::new();
        let sup = supervisor_with(&state, SupervisorSeams::default()).await;
        const DIALOG: &str = "printf 'Do you want to proceed?\\n ❯ 1. Yes\\n   2. No, and tell \
                              Claude what to do differently\\n'; sleep 300";
        install_live_session_of_kind(&sup, "asking", DIALOG, AgentKind::Claude).await;
        // The same screen on a session with no integration: the negative
        // half, in the one place where "the tail really did reach the
        // classifier" is not in question.
        install_live_session(&sup, "unintegrated", DIALOG).await;

        let asking = sample_of(&sup, "asking").await;
        let unintegrated = sample_of(&sup, "unintegrated").await;
        let ticker = start_ticker(&sup);
        for (what, sample) in [("asking", &asking), ("unintegrated", &unintegrated)] {
            wait_until(&format!("the {what} pane decayed to quiet"), || {
                sample.lock().expect("activity mutex").unchanged_streak >= 3
            })
            .await;
        }
        ticker.shutdown().await;

        assert_eq!(
            classify(&sup, "asking").await,
            SessionStatus::Waiting,
            "a claude-kind session showing an approval prompt is waiting; tail was {:?}",
            asking.lock().expect("activity mutex").tail
        );
        assert_eq!(
            classify(&sup, "unintegrated").await,
            SessionStatus::Idle,
            "the same screen without an integration keeps the generic baseline"
        );
    }
}
