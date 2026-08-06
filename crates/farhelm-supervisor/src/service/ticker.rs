//! The supervisor's own heartbeat: one periodic task, started by `serve`,
//! that advances the work nobody should have to ask for.
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
//! question. The capture half of a tick is, and it is gated where it
//! always was, inside the pass.
//!
//! # What the samples are for
//!
//! This module measures; it does not classify. `service::status`'s
//! `live_status` reads exactly what is recorded here — how long a pane has
//! been quiet, and the tail it last showed — and turns it into
//! running/waiting/idle, with the per-kind sharpeners matching a pending
//! question or approval against that tail. Keeping the thresholds there
//! rather than here is what lets the whole classification be unit-tested
//! against hand-built entries, and what keeps this task free to be late
//! without being wrong.
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

use super::core::{CaptureReason, SessionEntry, Supervisor};
use super::terminals::Terminal;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use tracing::{debug, error, warn};

/// How often the supervisor's periodic task fires in production.
///
/// Chosen against the helm's own 3-second `ListSessions` drain rather than
/// independently. PLAN_M6_75.md item 3 fixes the supervisor edge's
/// staleness bound at one ticker interval plus one drain, so a ticker
/// SLOWER than the drain would widen that bound for no saving, while a
/// much faster one would spend subprocesses producing samples no reader
/// ever gets to see. Sitting below the drain means every drain finds a
/// sample no older than one interval without the two cadences locking into
/// phase.
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
/// an instant and a screen, not a verdict — so that the
/// running/waiting/idle thresholds and the per-kind sharpening live in one
/// place, beside the precedence rules they extend, rather than being
/// half-decided here.
#[derive(Debug, Default)]
pub(crate) struct ActivitySample {
    /// How many times this session has been sampled at all.
    ///
    /// Load-bearing rather than a statistic: the FIRST sample of a pane
    /// establishes the baseline that later ones are compared against, and
    /// must not itself count as output. Without this, every session would
    /// look busy the moment it was first looked at — including one that
    /// has sat at a shell prompt untouched since a reboot.
    pub(crate) samples: u64,
    /// When the pane's screen was last observed to have CHANGED, on this
    /// process's monotonic clock. `None` means it has never been seen to
    /// change: either it has not been sampled twice yet, or it has
    /// genuinely shown the same thing throughout.
    ///
    /// Read by `status::live_status`, which turns "changed recently" into
    /// running and "quiet" into idle against its own recency window. The
    /// `None` is why [`ActivitySample::samples`] has to exist beside it:
    /// the classifier must be able to tell "never seen to change" from
    /// "not yet watched twice", and this field alone cannot.
    pub(crate) last_change: Option<Instant>,
    /// The pane's screen as of the last sample, bounded to
    /// [`SAMPLE_TAIL_BYTES`] and trimmed of the blank rows a pane is
    /// padded out with.
    ///
    /// Serves both consumers: change detection compares it against the
    /// next capture, and item 2's per-kind sharpeners match a prompt or
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
    /// entirely between two ticks; that costs one interval of recency on a
    /// status SPEC.md already calls cosmetic.
    ///
    /// `now` is a parameter rather than read here so the unit tests can
    /// drive recency without sleeping.
    pub(crate) fn observe(&mut self, tail: String, now: Instant) {
        if self.samples > 0 && self.tail.as_deref() != Some(tail.as_str()) {
            self.last_change = Some(now);
        }
        self.tail = Some(tail);
        self.samples += 1;
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
        let mut cursor: usize = 0;
        loop {
            // Sleep FIRST. `serve` has just run a reload and a capture
            // pass of its own, so a tick at t=0 would repeat that work
            // before anything could have changed.
            tokio::select! {
                _ = &mut stop_rx => break,
                _ = tokio::time::sleep(interval) => {}
            }
            let Some(sup) = weak.upgrade() else {
                break;
            };
            tick(&sup, &mut cursor, SAMPLE_TAIL_BUDGET, &mut stop_rx).await;
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

/// One period's work: sample a slice of the live panes, then advance
/// conversation capture.
///
/// Both halves are periodic and neither is on an interactive path, so the
/// only thing the order decides is which of the two is delayed by the
/// other WITHIN a tick — they are sequential, so each pushes the next by
/// up to its own duration. Sampling goes first because its result is what
/// a drain arriving mid-tick will read, while a capture arriving a beat
/// later costs nothing anybody can observe. (The claim to be careful with
/// is not "this delays nothing" — it plainly delays the other half — but
/// that neither half can delay a REQUEST, which is what the separate
/// limiter below is for.)
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
    cursor: &mut usize,
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
/// # Rotation, and why the cursor is reset rather than carried
///
/// `cursor` names where the NEXT pass should resume, and it is meaningful
/// only relative to the population it was computed against. Every early
/// return that means "there is nothing to rotate through" resets it to
/// zero, including the empty-map case: carrying an offset across a
/// population REPLACEMENT — a supervisor whose sessions were all deleted
/// and new ones created — would make the first pass over the new
/// population start partway in, permanently skipping the sessions before
/// that offset on the pass that mattered most.
///
/// # Cancellation
///
/// `stop` is re-checked between individual captures, so a shutdown waits
/// out at most one `capture-pane` (itself bounded by that command's own
/// deadline) rather than a whole pass. The cursor is still advanced past
/// what was actually sampled, so a resumed ticker would not redo work —
/// though in practice nothing resumes a stopped ticker.
///
/// Failures are logged at DEBUG and the pass moves on. A ticker that
/// WARNED here would turn a tmux server that is down (or a pane that
/// vanished between the two calls, which is ordinary) into a log entry
/// every interval forever; the paths where a user is actually waiting on
/// the answer — the list path — still warn, which is where the signal
/// belongs.
async fn sample_pass(
    sup: &Supervisor,
    cursor: &mut usize,
    budget: usize,
    stop: &mut oneshot::Receiver<()>,
) {
    // Cloned out of the map and the lock released before any tmux call:
    // the session mutex is never held across an await (see `Supervisor`'s
    // lock discipline), and every line below awaits a subprocess.
    let entries: Vec<Arc<SessionEntry>> = sup.sessions.lock().await.values().cloned().collect();
    if entries.is_empty() {
        *cursor = 0;
        return;
    }
    let states = match sup.tmux.pane_states().await {
        Ok(states) => states,
        Err(e) => {
            debug!(
                error = %format!("{e:#}"),
                "could not probe pane liveness for this sampling pass; skipping it"
            );
            return;
        }
    };
    let mut live: Vec<(Arc<SessionEntry>, Terminal)> = entries
        .into_iter()
        .filter_map(|entry| {
            let terminal = entry.terminal.clone()?;
            let state = states.get(&terminal.pane)?;
            (state.session_name == terminal.tmux_name && !state.dead).then_some((entry, terminal))
        })
        .collect();
    if live.is_empty() {
        *cursor = 0;
        return;
    }
    // A stable order is what makes the round-robin cursor mean anything:
    // over a `HashMap`'s iteration order the budget would resample an
    // arbitrary subset every tick and starve the rest indefinitely.
    live.sort_by(|(a, _), (b, _)| a.info.id.cmp(&b.info.id));
    let start = *cursor % live.len();
    let take = budget.min(live.len());
    let mut taken = 0;
    for offset in 0..take {
        if stop_requested(stop) {
            break;
        }
        taken += 1;
        let (entry, terminal) = &live[(start + offset) % live.len()];
        let tail = match sup
            .tmux
            .capture_pane_tail(&terminal.tmux_name, &terminal.pane, SAMPLE_TAIL_BYTES)
            .await
        {
            Ok(tail) => tail,
            Err(e) => {
                debug!(
                    session = %entry.info.id, error = %format!("{e:#}"),
                    "could not sample this session's pane; leaving its previous sample in place"
                );
                continue;
            }
        };
        entry
            .activity
            .lock()
            .expect("activity mutex poisoned")
            .observe(tail, Instant::now());
    }
    *cursor = start + taken;
}

#[cfg(test)]
mod tests {
    use super::super::connection::{CONNECTION_WRITER_QUEUE, ConnectionCtx};
    use super::super::core::tests::{StateDir, dummy_exe, entry_with, no_uploads};
    use super::super::core::{CreateInputs, SupervisorSeams, SupervisorTimeouts, note_first_input};
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
                    invocation: "/opt/bin/claude",
                    title: Some("ticker".to_string()),
                    cols: 80,
                    rows: 24,
                    agent_kind: None,
                    resume_template: None,
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
        wait_until("the busy pane was seen to change", || {
            busy.lock().expect("activity mutex").last_change.is_some()
        })
        .await;
        wait_until("the still pane was sampled more than once", || {
            still.lock().expect("activity mutex").samples > 1
        })
        .await;
        ticker.shutdown().await;

        let busy = busy.lock().expect("activity mutex");
        assert!(
            busy.samples > 1,
            "change can only be established by comparing two samples"
        );
        assert!(
            busy.tail
                .as_deref()
                .is_some_and(|tail| tail.contains("tick")),
            "the tail is what item 2's sharpeners read, so it has to carry the pane's real \
             text; got {:?}",
            busy.tail
        );
        let still = still.lock().expect("activity mutex");
        assert_eq!(
            still.last_change, None,
            "a pane that printed nothing must not look like one that did"
        );
    }

    /// Shutdown is deterministic: once `shutdown` returns, no tick can
    /// still be in flight.
    ///
    /// The property every other ticker test leans on — they all assert
    /// against state a stray pass could still be mutating — and the one
    /// that keeps this suite from leaking a task per test into a runtime
    /// shared with the rest of the binary.
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
    #[tokio::test]
    async fn the_task_ends_when_its_supervisor_is_dropped() {
        let state = StateDir::new();
        let sup = supervisor_with(&state, SupervisorSeams::default()).await;
        let ticker = start_ticker(&sup);
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

    /// The baseline rule, in isolation: the first look at a pane is not
    /// output.
    ///
    /// A unit test rather than a tmux one because the mistake it guards
    /// against is one character wide with a large blast radius — counting
    /// sample one as a change would classify every session running the
    /// moment the supervisor started, which is exactly the "always wrong
    /// in the same direction" failure that makes a status column
    /// worthless. It also pins that recency ADVANCES rather than latching
    /// on the first change, which is what lets a session decay to idle.
    #[test]
    fn the_first_sample_establishes_a_baseline_rather_than_reporting_change() {
        let start = Instant::now();
        let mut sample = ActivitySample::default();

        sample.observe("hello".to_string(), start);
        assert_eq!(sample.samples, 1);
        assert_eq!(
            sample.last_change, None,
            "one observation cannot establish that anything moved"
        );

        sample.observe("hello".to_string(), start + Duration::from_secs(1));
        assert_eq!(
            sample.last_change, None,
            "an unchanged screen is the quiet case, not a late baseline"
        );

        let moved = start + Duration::from_secs(2);
        sample.observe("hello world".to_string(), moved);
        assert_eq!(sample.last_change, Some(moved));
        assert_eq!(sample.tail.as_deref(), Some("hello world"));

        let moved_again = start + Duration::from_secs(3);
        sample.observe("hello there".to_string(), moved_again);
        assert_eq!(
            sample.last_change,
            Some(moved_again),
            "the classifier reads this as 'how long has it been quiet', so it must move"
        );
        assert_eq!(sample.samples, 4);
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
        let mut cursor = 7;
        sample_pass(&sup, &mut cursor, SAMPLE_TAIL_BUDGET, &mut stop).await;
        assert_eq!(
            sample.lock().expect("activity mutex").samples,
            0,
            "a pane whose session name does not match must not be sampled at all"
        );
        assert_eq!(
            cursor, 0,
            "a pass with nothing eligible must RESET the rotation rather than leave an offset \
             that a later, different population would start partway into"
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
        let mut cursor = 0;
        for expected_cursor in 1..=3 {
            sample_pass(&sup, &mut cursor, 1, &mut stop).await;
            // The stored cursor counts samples TAKEN; it is normalized
            // against the population only when next consumed (`start =
            // cursor % live.len()`), so it legitimately runs past the
            // population size rather than wrapping in place. What must
            // hold here is that it advances by exactly the budget — that
            // the wrap then happens correctly is what the per-session
            // counts below prove, and they are the assertion that would
            // catch a rotation which stalled.
            assert_eq!(
                cursor, expected_cursor,
                "the cursor must advance by exactly the budget each pass"
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

        // Population CHURN: the rotation must not carry an offset from a
        // population that no longer exists into one that replaced it.
        sup.sessions.lock().await.clear();
        sample_pass(&sup, &mut cursor, 1, &mut stop).await;
        assert_eq!(
            cursor, 0,
            "an empty map resets the rotation; carrying the old offset would make the first \
             pass over a REPLACEMENT population skip its head"
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
                    title: id.to_string(),
                    created_at: now_unix(),
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
        let mut cursor = 0;
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
        let mut cursor = 0;
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
    /// tests pin the recency arithmetic exactly, but they build the sample
    /// cell themselves; only here does the transition depend on the ticker
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

        let busy = sample_of(&sup, "busy").await;
        let still = sample_of(&sup, "still").await;
        let ticker = start_ticker(&sup);
        wait_until("the busy pane was seen to change", || {
            busy.lock().expect("activity mutex").last_change.is_some()
        })
        .await;
        wait_until("the still pane was sampled more than once", || {
            still.lock().expect("activity mutex").samples > 1
        })
        .await;

        // Re-classified in a loop rather than once: the busy pane is only
        // `Running` while its last observed change is inside the recency
        // window, and a runner that stalls between the wait above and the
        // probe here would otherwise fail on the machine's scheduling
        // rather than on the code.
        let deadline = tokio::time::Instant::now() + TEST_DEADLINE;
        loop {
            if classify(&sup, "busy").await == SessionStatus::Running {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "a pane printing continuously never classified running"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(
            classify(&sup, "still").await,
            SessionStatus::Idle,
            "a pane watched twice with nothing printed is at rest"
        );
        ticker.shutdown().await;
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
    /// The pane is SILENT after printing, so its baseline has decayed to
    /// idle by the time it is classified: `Waiting` here can only be a
    /// promotion by the sharpener, never recency wearing a different name.
    #[tokio::test]
    async fn a_prompt_on_a_real_pane_classifies_waiting_through_the_sampler() {
        let state = StateDir::new();
        let sup = supervisor_with(&state, SupervisorSeams::default()).await;
        install_live_session_of_kind(
            &sup,
            "asking",
            "printf 'Do you want to proceed?\\n 1. Yes\\n 2. No, and tell Claude what to do \
             differently\\n'; sleep 300",
            AgentKind::Claude,
        )
        .await;
        // The same screen on a session with no integration: the negative
        // half, in the one place where "the tail really did reach the
        // classifier" is not in question.
        install_live_session(
            &sup,
            "unintegrated",
            "printf 'Do you want to proceed?\\n 1. Yes\\n 2. No, and tell Claude what to do \
             differently\\n'; sleep 300",
        )
        .await;

        let asking = sample_of(&sup, "asking").await;
        let unintegrated = sample_of(&sup, "unintegrated").await;
        let ticker = start_ticker(&sup);
        for (what, sample) in [("asking", &asking), ("unintegrated", &unintegrated)] {
            wait_until(&format!("the {what} pane was sampled twice"), || {
                sample.lock().expect("activity mutex").samples > 1
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
