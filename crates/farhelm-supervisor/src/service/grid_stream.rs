//! A host terminal's attachments: the grid sender that streams the terminal
//! to one viewer, and the path that viewer's input takes back to the
//! program.
//!
//! A terminal in a terminal host has no byte stream a viewer could render:
//! its screen lives in the supervisor's emulator (`crate::hosts::terminal`),
//! and reaches a viewer as grid messages (`farhelm_proto::grid`). An attach
//! that asks for a grid gets one [`GridSender`] task, which fills the
//! attachment's `ActiveAttach::forwarder` slot (shut down and joined like an
//! old terminal's `Forwarder`), and one [`HostAttach`], the attachment's
//! `AttachBackend`, which takes its input. Everything around them (takeover,
//! detach, the pause watch, the stall detach, the detach verdicts) is the
//! attachment machinery every terminal's attachment shares.
//!
//! # The stream
//!
//! [`GridStream`] is the sender's body, without the supervisor, so tests can
//! run it against a channel. It reads the emulator with
//! `HostTerminal::try_read`, which never waits (the emulator can be away for
//! seconds while a snapshot is encoded), marking the terminal's status seen
//! just before each attempt, so that a change published after a read is
//! still unseen and wakes the sender again; `crate::grid_output` says what
//! to send.
//!
//! - **Attach** ([`GridStream::catch_up`]): `hello`, `state`, the whole
//!   visible screen, then the history once, oldest first, then the existing
//!   `ReplayComplete` control message, then live updates (the format's
//!   attach sequence). The viewer's size is asked for before the sender
//!   starts, and the sender waits briefly ([`ATTACH_SIZE_WAIT`]) for it to
//!   take effect, so that the history is not sent at the old width only to
//!   be reflowed and sent again. The catch-up ends once every history line
//!   that was held when it started has been sent or has scrolled off the
//!   far end, or as soon as history's layout changes under it: the reset
//!   and the history again are then the live path's to send, after the
//!   change settles, like any other (see "Flow control"). Either way the
//!   first live cycle follows at once, reading the screen, `state` and
//!   history again, since the catch-up's later reads take history only and
//!   output or an exit may have come meanwhile.
//! - **Live** ([`GridStream::live`]): whenever the terminal changes, one
//!   cycle sends what changed: `state` if it did, the history lines that
//!   entered history since the last cycle (or a `history_reset` and the
//!   history again, see "Flow control"), the screen rows that differ, and
//!   any clipboard write. Cycles are at least [`MIN_CYCLE_INTERVAL`] apart,
//!   which bounds the reading and diffing a flood of output costs per
//!   viewer; the client draws at most once per animation frame anyway.
//! - **Exit and end**: the program's exit is the `state` message's `exit`;
//!   the attachment stays, showing the final screen. A host that is gone
//!   ends the attachment as any terminal that ended does ("session terminal
//!   ended"), after one last cycle; one that became unavailable ends it as a
//!   failed stream. An attach to a host that is unavailable already (a
//!   startup placeholder included, which has no runtime) is refused before
//!   any of this starts (`super::handlers`, `host_attach_refusal`, which
//!   says when such an attach can work again).
//!
//! # Flow control
//!
//! The sender coalesces whenever it cannot proceed: while the connection's
//! bounded writer queue is full it waits to enqueue the cycle's messages,
//! and while the viewer has paused (`PauseOutput`) it sends nothing at all
//! (a stricter reading of the format's "pause means stop sending screen
//! frames": history waits too, since nothing about it is urgent and
//! holding it costs nothing). Either way the next cycle starts from the
//! emulator as it is then, so any number of screen revisions in between
//! cost one update. A slow link costs frames, never memory: the host keeps
//! reading the program's output, and the program never waits for a viewer.
//!
//! History is never coalesced away. The sender knows the number of the next
//! line the viewer is owed, and lines that entered history while it was
//! behind go out as appended ranges. When the owed line is no longer held
//! (more lines arrived than the scrollback keeps), or history changed shape
//! (a resize, an erasure, a restored emulator, a redefined palette entry),
//! it sends `history_reset` and the history as it is: lines the viewer
//! never received are gone from the supervisor too, so its history ends up
//! the supervisor's either way (SPEC.md: flow control never drops terminal
//! output). A change of shape is acted on only once it has held for
//! [`RELAYOUT_SETTLE`]: a viewer resized by dragging a window would
//! otherwise be sent its whole history again at every step of the drag.
//! Changes that never stop (a long drag, a program that keeps redefining
//! its palette) are acted on once, [`RELAYOUT_MAX_DELAY`] after the first;
//! after such a forced reset, the next run waits until it settles, so that
//! continuous change resends the history once and then not again until it
//! stops (a resend can take longer than the cap, and each would cut the
//! last one short). Screen frames go on meanwhile; history waits.
//!
//! Each live cycle sends at most [`LIVE_HISTORY_BYTES`] of history before
//! its screen frame, so a history resend (megabytes for a colored
//! scrollback) goes out over many cycles with the screen kept current in
//! each, rather than hold typing's echo back for the whole transfer.
//!
//! A resend costs as much as an attach: for a full 12,000-line scrollback
//! 200 columns wide, about 2.5 MB of typical output, 60 MB colored cell by
//! cell from the 256-color palette, and 86 MB in true color (the
//! `measure_the_*_cost_at_the_scrollback_cap_*` tests). A redefined
//! palette entry costs the same, whether or not any history row uses it:
//! telling which rows do would mean scanning the history on every change.
//! A theme script that sets the palette once costs one resend.
//!
//! The stall timeout and stall detach are the shared attachment machinery's
//! (`SupervisorTimeouts::stall_detach`, by default
//! `super::core::STALL_DETACH_TIMEOUT`, the same for every terminal): a pause
//! that outlives the stall timeout ends the attachment as stalled, from any
//! point the sender can wait at (`super::connection::park_while_paused`,
//! `super::connection::stalled_past_deadline`).
//!
//! # Notices
//!
//! A program's clipboard writes (OSC 52) reach the viewer as `clipboard`
//! messages, live output only, from the terminal's notice subscription
//! (`HostTerminal::subscribe_notices`), taken when the attachment starts.
//! While the sender is behind it keeps only the latest unsent write, and it
//! sends a write's parts within one cycle, with nothing else between them
//! (a pause can hold back the rest of a write it has started, and a stall
//! detach can end the attachment there). Bells and title changes have no message in the
//! format (today's terminal does nothing on a bell, and a title reaches
//! the viewer with the session's details), and the client honors only
//! writes to the clipboard, not to the primary selection; so the runtime
//! sends a subscription writes to the clipboard only, and nothing else can
//! push one out of its bounded backlog.
//!
//! # Input
//!
//! The connection's data-frame arm hands each viewer input frame to
//! [`HostAttach::send_input`], under the supervisor-wide attachments lock,
//! which must never wait on the program or on the emulator. So the frame is
//! decoded and encoded there, at once, with the program's modes and the
//! terminal's size as the runtime last published them in its status (as
//! today's terminal encodes a key with the modes it has seen), and the
//! bytes go to the attachment's input pump ([`HostAttach`]), a task that
//! writes them to the host in order, waiting as long as the program takes
//! to read them. What an attachment holds is bounded ([`INPUT_BACKLOG`]).
//!
//! - **One sequence.** Everything the viewer sends goes through one input
//!   sequence made when the attachment starts, so that once any input is
//!   lost with the host's connection, nothing the viewer sent after it is
//!   written.
//! - **A paste is written whole, or not at all.** The attachment collects a
//!   paste's parts, encoded, and hands it on only with its last part, as
//!   one `InputSequence::send_paste`, which the runtime writes with nothing
//!   between its pieces: not another producer's input (a prompt, another
//!   viewer's keys), not the terminal's answers to the program's queries,
//!   which queue behind it. A paste whose last part never arrives (the
//!   attachment ended, its input broke, the collected paste outgrew the
//!   backlog) is dropped whole, with nothing of it written; so is one the
//!   runtime never took. One the runtime took and a lost connection cut
//!   short is the runtime's to close (`crate::hosts::terminal`, "A paste
//!   goes in whole"), and the attachment's input fails.
//! - **Failures end the attachment.** A malformed message, a broken format
//!   rule (input in the middle of a paste), input past the backlog, or input
//!   whose fate is unknown or that was not sent ends the attachment as an input
//!   failure: accepting later input after losing some can turn a command into a
//!   different one. Input the program's pty discarded (a program that exited)
//!   is not a failure: the program it was for is gone, so no later input
//!   depends on it. The sender notices a failure at once, wherever it waits, a
//!   pause included.
//! - **The pump ends with the attachment.** When the attachment ends, its
//!   pump drops whatever input it has not yet handed to the host's runtime
//!   (the viewer was told its attachment ended) and stops at once. Input it
//!   handed over is the runtime's, and is written in order before anything
//!   a replacement attachment sends.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use anyhow::Context as _;
use farhelm_proto::grid::{
    ClipboardTarget, ExitState, GRID_MAX_HISTORY_CAP, GridInput, GridOutput, TerminalState,
};
use farhelm_proto::{ControlMsg, Frame};
use farhelm_terminal_emulator::Notice;
use terminal_host::protocol::{ChildExit, Size};
use tokio::sync::{mpsc, oneshot, watch};
use tracing::warn;

use super::capture::submits_a_line;
use super::connection::{
    ForwarderEnd, park_while_paused, report_forwarder_end, stalled_past_deadline,
};
use super::core::Supervisor;
use super::terminals::{AttachmentKey, OutputReapOutcome};
use crate::grid_input::{InputEncoder, input_frames};
use crate::grid_output::{
    self, HistoryBatch, HistoryCursor, Layout, SentScreen, clipboard_messages, clipboard_target,
    history_messages,
};
use crate::hosts::{
    EmulatorRef, HostStatus, HostTerminal, InputFate, InputSequence, Liveness, NoticeSubscription,
    Unavailability,
};

/// How long an attach waits for the viewer's size to take effect before it
/// reads the screen it starts with. A resize takes a round trip to the host
/// (the emulator takes a size from the host's resize record); without the
/// wait, an attach at a new width would send the screen and the whole
/// history at the old width, and then all of it again once the resize
/// reflowed it. Past the wait it goes ahead at whatever size is in force.
pub(crate) const ATTACH_SIZE_WAIT: Duration = Duration::from_secs(1);

/// History lines read per emulator read while an attach catches up, and so
/// per round of messages: enough that 12,000 lines take a dozen rounds, few
/// enough that reading them (about a quarter of a million cells at 200
/// columns) holds the emulator, which the runtime needs to feed output,
/// for no more than a few milliseconds.
pub(crate) const CATCH_UP_LINES: usize = 1024;

/// History lines a live cycle sends at most. A viewer owed more (after a
/// flood, a pause, or a reset) gets them over several cycles, with the
/// screen updated in each, so a large history transfer never holds back
/// the visible screen.
pub(crate) const LIVE_HISTORY_LINES: usize = 1024;

/// How many bytes of history a live cycle sends at most, as
/// `HistoryCursor::read_within` estimates them (erring high, links and
/// escaping counted), plus at most one row more. Lines of a colored
/// scrollback encode at several kilobytes each, so
/// [`LIVE_HISTORY_LINES`] alone would let one cycle carry megabytes ahead
/// of its screen frame, which then reaches the viewer only after them.
pub(crate) const LIVE_HISTORY_BYTES: usize = 256 * 1024;

/// The least time between two live cycles. Bounds how often a flood of
/// output makes a sender read and diff the screen, which it would otherwise
/// do for every output record the runtime processes, as fast as the writer
/// queue drains. At this interval a cycle can still carry more history than
/// any program produces (`LIVE_HISTORY_LINES` per cycle), and the client
/// draws at most once per animation frame.
pub(crate) const MIN_CYCLE_INTERVAL: Duration = Duration::from_millis(8);

/// How long a change of history's layout must hold before the sender resets
/// the viewer's history and sends it again (module docs, "Flow control").
/// A window dragged to a new size resizes the terminal many times a second;
/// each resize reflows the history, and resending a full scrollback costs
/// megabytes (the `measure_the_resize_cost_at_the_scrollback_cap_*`
/// tests). A quarter second after the last step is soon enough that a
/// viewer that stopped dragging sees its history again at once.
pub(crate) const RELAYOUT_SETTLE: Duration = Duration::from_millis(250);

/// The longest a change of history's layout waits to be acted on, counted
/// from the first change in a run of them: a run that never settles (a
/// long drag, a program redefining its palette over and over) would
/// otherwise hold the viewer's history back for as long as it lasts. It
/// applies once: after a reset it forced, the next run waits to settle
/// (module docs, "Flow control").
pub(crate) const RELAYOUT_MAX_DELAY: Duration = Duration::from_secs(2);

/// The most bytes of viewer input an attachment holds and the host's
/// runtime has not yet written: a paste being collected, input waiting for
/// the pump, and a paste the runtime holds until it is written. Past this
/// the attachment's input fails (module docs, "Input"). The longest paste
/// the runtime takes ([`crate::hosts::MAX_PASTE`]), so that a paste this
/// large is refused here, before any of it is handed on.
pub(crate) const INPUT_BACKLOG: usize = crate::hosts::MAX_PASTE;

// ---- The stream ----------------------------------------------------------------

/// What a [`GridStream`] is made from.
pub(crate) struct GridStreamParts {
    /// The attachment's data channel.
    pub(crate) channel: u32,
    /// The connection's writer queue.
    pub(crate) tx: mpsc::Sender<Frame>,
    pub(crate) host: HostTerminal,
    /// The terminal's live notices (`HostTerminal::subscribe_notices`).
    pub(crate) notices: Option<NoticeSubscription>,
    /// The attachment's pause (`ActiveAttach::pause`).
    pub(crate) pause_rx: watch::Receiver<Option<tokio::time::Instant>>,
    pub(crate) stall_timeout: Duration,
    /// The size the viewer asked for when it attached, which the attach
    /// waits a moment for ([`ATTACH_SIZE_WAIT`]); `None` to start at once.
    pub(crate) attach_size: Option<Size>,
    /// Set by the attachment's input pump when its input failed
    /// ([`HostAttach::new`]).
    pub(crate) input_failure: Option<watch::Receiver<Option<String>>>,
    /// What tests observe the stream through; nothing in production.
    pub(crate) seams: StreamSeams,
}

/// A grid stream's test seams. Empty outside tests.
#[derive(Default)]
pub(crate) struct StreamSeams {
    /// Handed a sender after every emulator read the stream makes, before
    /// it acts on what it read; the stream goes on once that sender is
    /// used or dropped. Lets a test publish a change in exactly the window
    /// between a read and its bookkeeping.
    #[cfg(test)]
    pub(crate) after_read: Option<mpsc::UnboundedSender<oneshot::Sender<()>>>,
    /// Counts the times the stream found its viewer paused and parked: a
    /// test's proof that the stream reached the pause with work to do.
    #[cfg(test)]
    pub(crate) parked: Option<watch::Sender<u64>>,
}

/// One grid attachment's output: everything between a host terminal's
/// emulator and one channel's data frames (module docs, "The stream").
pub(crate) struct GridStream {
    channel: u32,
    tx: mpsc::Sender<Frame>,
    host: HostTerminal,
    notices: Option<NoticeSubscription>,
    pause_rx: watch::Receiver<Option<tokio::time::Instant>>,
    stall_timeout: Duration,
    attach_size: Option<Size>,
    input_failure: Option<watch::Receiver<Option<String>>>,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "test seams, empty outside tests")
    )]
    seams: StreamSeams,
    /// The runtime's status, to wake on every change of the emulator or the
    /// program's liveness. Marked seen just before each emulator read.
    status: watch::Receiver<HostStatus>,
    /// Set once the runtime's status channel closed: its task ended, and
    /// the status it left is final.
    runtime_ended: bool,
    /// What the client was last sent of the screen; `None` before `hello`.
    screen: Option<SentScreen>,
    /// Which history lines the client has.
    history: HistoryCursor,
    /// Whether the last cycle left history lines owed.
    history_owed: bool,
    /// A change of history's layout the stream has seen and not yet acted
    /// on (module docs, "Flow control").
    relayout: Option<Relayout>,
    /// Whether the last reset for a change of layout was forced by
    /// [`RELAYOUT_MAX_DELAY`] before the change settled: until a run
    /// settles, the next one waits for that, so that changes that never
    /// stop do not resend the history every two seconds.
    relayout_capped: bool,
    /// The history's end (`HistoryBatch::end`) when the last live frame was
    /// made, if that frame showed the primary screen and its history read
    /// reset nothing: how far history had got, so that the next frame knows
    /// how many lines the screen scrolled since. `None` before the first
    /// live frame, after a frame of the alternate screen, and after any
    /// reset or deferred history read, so it is never a base from another
    /// numbering.
    frame_history_end: Option<u64>,
    /// The last `state` sent.
    sent_state: Option<TerminalState>,
    /// The latest clipboard write not yet sent.
    clipboard: Option<String>,
    /// When the last live cycle started ([`MIN_CYCLE_INTERVAL`]).
    last_cycle: Option<tokio::time::Instant>,
}

/// A run of changes to history's layout, waiting to settle before the
/// viewer's history is reset ([`RELAYOUT_SETTLE`], [`RELAYOUT_MAX_DELAY`]).
#[derive(Debug, Clone, Copy)]
struct Relayout {
    /// The layout the history has now.
    layout: Layout,
    /// When it moved to `layout`.
    since: tokio::time::Instant,
    /// When the run's first change was seen.
    first: tokio::time::Instant,
}

impl Relayout {
    /// When the run is acted on if nothing more changes; only once it
    /// settles when the last reset was forced (`capped`).
    fn due(&self, capped: bool) -> tokio::time::Instant {
        let settled = self.since + RELAYOUT_SETTLE;
        if capped {
            settled
        } else {
            settled.min(self.first + RELAYOUT_MAX_DELAY)
        }
    }
}

/// What one emulator read takes for a cycle.
struct Capture {
    screen: grid_output::Screen,
    state: TerminalState,
    /// The history read, or `None` when a layout change is still settling.
    batch: Option<HistoryBatch>,
    /// The layout the history moved to, when it moved.
    moved_to: Option<Layout>,
    /// Whether `batch` resets for a change that had not settled, forced by
    /// [`RELAYOUT_MAX_DELAY`].
    forced: bool,
}

impl GridStream {
    pub(crate) fn new(parts: GridStreamParts) -> GridStream {
        let status = parts.host.subscribe();
        GridStream {
            channel: parts.channel,
            tx: parts.tx,
            host: parts.host,
            notices: parts.notices,
            pause_rx: parts.pause_rx,
            stall_timeout: parts.stall_timeout,
            attach_size: parts.attach_size,
            input_failure: parts.input_failure,
            seams: parts.seams,
            status,
            runtime_ended: false,
            screen: None,
            history: HistoryCursor::new(),
            history_owed: false,
            relayout: None,
            relayout_capped: false,
            frame_history_end: None,
            sent_state: None,
            clipboard: None,
            last_cycle: None,
        }
    }

    /// Stream the terminal until something ends the attachment, and say
    /// what did. The caller reports it ([`GridSender::run`]).
    pub(crate) async fn run(mut self) -> ForwarderEnd {
        if let Err(end) = self.catch_up().await {
            return end;
        }
        self.live().await
    }

    /// The attach sequence: `hello`, `state`, the screen, the history once,
    /// and `ReplayComplete` (module docs, "Attach").
    async fn catch_up(&mut self) -> Result<(), ForwarderEnd> {
        self.await_attach_size().await;
        // Out of `self` while the read borrows it; put back below.
        let mut history = std::mem::take(&mut self.history);
        let read = self
            .read_home(|emulator| {
                (
                    grid_output::screen(emulator),
                    state_of(emulator),
                    history.read(emulator, relayouts(emulator), CATCH_UP_LINES),
                )
            })
            .await;
        self.history = history;
        let ((screen, state, first), status) = read?;
        // The client keeps as much history as the supervisor holds; a
        // scrollback past the format's bound would leave the client with
        // less, which no terminal is configured for.
        let history_cap = u32::try_from(self.host.scrollback_lines())
            .map_or(GRID_MAX_HISTORY_CAP, |lines| {
                lines.min(GRID_MAX_HISTORY_CAP)
            });
        self.send(GridOutput::Hello {
            cols: screen.cols,
            rows: screen.rows,
            history_cap,
        })
        .await?;
        let state = with_exit(state, &status.liveness);
        self.send(GridOutput::State(state)).await?;
        self.sent_state = Some(state);
        let mut sent = SentScreen::after_hello(screen.cols, screen.rows);
        let update = sent
            .update(&screen, 0)
            .expect("the first update after hello commits a frame");
        self.screen = Some(sent);
        for message in update {
            self.send(message).await?;
        }

        // Every line held when the attach read the history: lines evicted
        // meanwhile only move the start forward, so this ends. A change of
        // layout ends it early, leaving the reset to the live path, which
        // waits for the change to settle (module docs, "Attach").
        let target = first.end;
        let mut batch = first;
        loop {
            self.history_owed = batch.owed() > 0;
            self.send_history(batch).await?;
            if self.history.next() >= target {
                break;
            }
            let mut history = std::mem::take(&mut self.history);
            let read = self
                .read_home(|emulator| {
                    let relayouts = relayouts(emulator);
                    let (moved, _) = history.layout_moved(emulator, relayouts);
                    (!moved).then(|| history.read(emulator, relayouts, CATCH_UP_LINES))
                })
                .await;
            self.history = history;
            match read?.0 {
                Some(next) => batch = next,
                None => {
                    self.history_owed = true;
                    break;
                }
            }
        }
        // The reads after the first took history only, and each marked the
        // status seen: a change they consumed (output, the exit) has not
        // reached the viewer's screen or `state`. The first live cycle reads
        // them again, whether or not anything changes after this.
        self.status.mark_changed();
        self.send_replay_complete().await
    }

    /// Wait, at most [`ATTACH_SIZE_WAIT`], until the terminal has the size
    /// the viewer attached with, as its status says (never the emulator).
    async fn await_attach_size(&mut self) {
        let Some(size) = self.attach_size else {
            return;
        };
        let status = &mut self.status;
        let _ = tokio::time::timeout(ATTACH_SIZE_WAIT, async {
            while status.borrow().size != size {
                if status.changed().await.is_err() {
                    return;
                }
            }
        })
        .await;
    }

    /// Read the emulator with `read` once it is home, with the status as
    /// marked seen just before the read that succeeded: a change published
    /// after it stays unseen, so the sender is woken for it, and the
    /// status's liveness is never ahead of what the read saw. While the
    /// emulator is away (a snapshot encode), the wait for its return (which
    /// changes the status) still ends the attachment when a pause outlives
    /// the stall timeout or its input fails.
    async fn read_home<R>(
        &mut self,
        mut read: impl FnMut(&EmulatorRef<'_>) -> R,
    ) -> Result<(R, HostStatus), ForwarderEnd> {
        loop {
            let status = self.status.borrow_and_update().clone();
            if let Some(value) = self.host.try_read(&mut read) {
                self.after_read().await;
                return Ok((value, status));
            }
            if matches!(
                status.liveness,
                Liveness::Unavailable {
                    reason: Unavailability::EmulatorLost,
                    ..
                }
            ) {
                return Err(ForwarderEnd::StreamFailed(
                    "the terminal's emulator was lost".to_string(),
                ));
            }
            tokio::select! {
                changed = self.status.changed() => {
                    if changed.is_err() {
                        return Err(ForwarderEnd::StreamFailed(
                            "the terminal's runtime ended while its emulator was away".to_string(),
                        ));
                    }
                }
                reason = input_failed(&mut self.input_failure) => {
                    return Err(ForwarderEnd::InputFailed(reason));
                }
                () = stalled_past_deadline(self.pause_rx.clone(), self.stall_timeout) => {
                    return Err(ForwarderEnd::Stalled);
                }
            }
        }
    }

    /// The test seam after a read (`StreamSeams::after_read`).
    async fn after_read(&mut self) {
        #[cfg(test)]
        if let Some(seam) = &self.seams.after_read {
            let (go, wait) = oneshot::channel();
            if seam.send(go).is_ok() {
                let _ = wait.await;
            }
        }
    }

    /// Tell the client its attach's catch-up is over, with the same control
    /// message every attachment sends (`Forwarder::send_replay_complete` for
    /// an old terminal): not held back by a pause, which is about terminal
    /// output, but raced against the stall deadline.
    async fn send_replay_complete(&mut self) -> Result<(), ForwarderEnd> {
        let frame = Frame::control(&ControlMsg::ReplayComplete {
            channel: self.channel,
        });
        tokio::select! {
            sent = self.tx.send(frame) => sent.map_err(|_| ForwarderEnd::ClientGone),
            reason = input_failed(&mut self.input_failure) => Err(ForwarderEnd::InputFailed(reason)),
            () = stalled_past_deadline(self.pause_rx.clone(), self.stall_timeout) => {
                Err(ForwarderEnd::Stalled)
            }
        }
    }

    /// Send one history batch: a `history_reset` first when it says so,
    /// then its lines.
    async fn send_history(&mut self, batch: HistoryBatch) -> Result<(), ForwarderEnd> {
        if batch.reset.is_some() {
            self.send(GridOutput::HistoryReset).await?;
        }
        for message in history_messages(batch.first, batch.cols, &batch.lines) {
            self.send(message).await?;
        }
        Ok(())
    }

    /// Live updates until the attachment ends (module docs, "Live").
    async fn live(&mut self) -> ForwarderEnd {
        loop {
            if let Err(end) = self.wait_for_work().await {
                return end;
            }
            let liveness = match self.cycle().await {
                Ok(liveness) => liveness,
                Err(end) => return end,
            };
            match liveness {
                Liveness::Running { .. } | Liveness::Exited { .. } => {}
                Liveness::Gone => return ForwarderEnd::TerminalEnded,
                Liveness::Unavailable { reason, .. } => {
                    return ForwarderEnd::StreamFailed(format!(
                        "the terminal host is unavailable: {reason}"
                    ));
                }
            }
            if self.runtime_ended {
                return ForwarderEnd::StreamFailed("the terminal's runtime ended".to_string());
            }
        }
    }

    /// Wait until a cycle has something to send: a change of the terminal,
    /// history still owed, a settled layout change, a clipboard write, or
    /// the runtime's end. Returns the attachment's end instead when its
    /// input failed or its pause outlived the stall timeout.
    async fn wait_for_work(&mut self) -> Result<(), ForwarderEnd> {
        loop {
            self.drain_notices();
            if let Some(reason) = self.input_failure_now() {
                return Err(ForwarderEnd::InputFailed(reason));
            }
            if self.history_owed || self.clipboard.is_some() {
                return Ok(());
            }
            match self.status.has_changed() {
                Ok(true) => return Ok(()),
                Ok(false) => {}
                Err(_) => {
                    self.runtime_ended = true;
                    return Ok(());
                }
            }
            let capped = self.relayout_capped;
            let settled = self.relayout.as_ref().map(|run| run.due(capped));
            let settle = async {
                match settled {
                    Some(at) => tokio::time::sleep_until(at).await,
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                changed = self.status.changed() => {
                    if changed.is_err() {
                        self.runtime_ended = true;
                    }
                    return Ok(());
                }
                notice = next_notice(&mut self.notices) => match notice {
                    Some(notice) => self.stash(notice),
                    None => self.notices = None,
                },
                reason = input_failed(&mut self.input_failure) => {
                    return Err(ForwarderEnd::InputFailed(reason));
                }
                () = settle => return Ok(()),
                () = stalled_past_deadline(self.pause_rx.clone(), self.stall_timeout) => {
                    return Err(ForwarderEnd::Stalled);
                }
            }
        }
    }

    /// One live cycle (module docs, "Live" and "Flow control"). Returns the
    /// liveness the cycle's read saw.
    async fn cycle(&mut self) -> Result<Liveness, ForwarderEnd> {
        if let Some(last) = self.last_cycle {
            tokio::time::sleep_until(last + MIN_CYCLE_INTERVAL).await;
        }
        self.park().await?;
        let now = tokio::time::Instant::now();
        self.last_cycle = Some(now);
        self.drain_notices();
        let pending = self.relayout;
        let capped = self.relayout_capped;
        let mut history = std::mem::take(&mut self.history);
        let read = self
            .read_home(|emulator| {
                let relayouts = relayouts(emulator);
                let (moved, layout) = history.layout_moved(emulator, relayouts);
                // A layout change is acted on once it has held for the
                // settle time, or once its run has gone on for the longest
                // delay, unless the last reset was forced that way (module
                // docs, "Flow control").
                let settled = pending
                    .is_some_and(|run| run.layout == layout && now >= run.since + RELAYOUT_SETTLE);
                let forced = !settled
                    && !capped
                    && pending.is_some_and(|run| now >= run.first + RELAYOUT_MAX_DELAY);
                let settling = moved && !settled && !forced;
                Capture {
                    screen: grid_output::screen(emulator),
                    state: state_of(emulator),
                    batch: (!settling).then(|| {
                        history.read_within(
                            emulator,
                            relayouts,
                            LIVE_HISTORY_LINES,
                            LIVE_HISTORY_BYTES,
                        )
                    }),
                    moved_to: moved.then_some(layout),
                    forced: moved && forced,
                }
            })
            .await;
        self.history = history;
        let (capture, status) = read?;
        let Capture {
            screen,
            state,
            batch,
            moved_to,
            forced,
        } = capture;
        if moved_to.is_some() && batch.is_some() {
            self.relayout_capped = forced;
        }
        self.relayout = match (&batch, moved_to) {
            (None, Some(layout)) => Some(match self.relayout {
                Some(run) if run.layout == layout => run,
                Some(run) => Relayout {
                    layout,
                    since: now,
                    first: run.first,
                },
                None => Relayout {
                    layout,
                    since: now,
                    first: now,
                },
            }),
            _ => None,
        };
        let mut messages = Vec::new();
        let state = with_exit(state, &status.liveness);
        if self.sent_state != Some(state) {
            messages.push(GridOutput::State(state));
            self.sent_state = Some(state);
        }
        // Lines the screen pushed into history since the last frame, which
        // the frame may shift by only when they all go out before it (the
        // format's ordering rule): a history read that reset nothing and
        // owes nothing, and both frames on the primary screen, whose rows
        // are what scrolls into history.
        let (scrolled, history_end) = match &batch {
            Some(batch) if batch.reset.is_none() && batch.owed() == 0 && !screen.alternate => {
                let scrolled = self
                    .frame_history_end
                    .map_or(0, |previous| batch.end.saturating_sub(previous));
                (scrolled, Some(batch.end))
            }
            _ => (0, None),
        };
        self.history_owed = batch.as_ref().is_some_and(|batch| batch.owed() > 0);
        if let Some(batch) = batch {
            if batch.reset.is_some() {
                messages.push(GridOutput::HistoryReset);
            }
            messages.extend(history_messages(batch.first, batch.cols, &batch.lines));
        }
        let sent = self
            .screen
            .as_mut()
            .expect("the catch-up sent the first screen");
        if let Some(update) = sent.update(&screen, scrolled) {
            messages.extend(update);
            self.frame_history_end = history_end;
        } else if history_end.is_none() {
            self.frame_history_end = None;
        }
        if let Some(text) = self.clipboard.take() {
            messages.extend(clipboard_messages(ClipboardTarget::Clipboard, &text));
        }
        for message in messages {
            self.send(message).await?;
        }
        Ok(status.liveness)
    }

    /// Park while the viewer has paused, as every send does, counting the
    /// park for tests (`StreamSeams::parked`). The attachment's input
    /// failing ends the park, and the attachment, at once: a paused viewer
    /// would otherwise keep a broken attachment until it resumed or the
    /// stall timeout ran out, and then be told the wrong reason.
    async fn park(&mut self) -> Result<(), ForwarderEnd> {
        #[cfg(test)]
        if self.pause_rx.borrow().is_some()
            && let Some(parked) = &self.seams.parked
        {
            parked.send_modify(|count| *count += 1);
        }
        tokio::select! {
            parked = park_while_paused(&mut self.pause_rx, self.stall_timeout) => parked,
            reason = input_failed(&mut self.input_failure) => Err(ForwarderEnd::InputFailed(reason)),
        }
    }

    /// Enqueue one message on the attachment's channel, after any pause,
    /// waiting for room in the writer queue, both raced against the stall
    /// deadline and the attachment's input failing. A message that does not
    /// encode is a sender bug, and ends the attachment rather than reach a
    /// client that would refuse it.
    async fn send(&mut self, message: GridOutput) -> Result<(), ForwarderEnd> {
        let bytes = message.encode().map_err(|error| {
            ForwarderEnd::StreamFailed(format!("a grid message could not be encoded: {error}"))
        })?;
        self.park().await?;
        tokio::select! {
            sent = self.tx.send(Frame::data(self.channel, bytes)) => {
                sent.map_err(|_| ForwarderEnd::ClientGone)
            }
            reason = input_failed(&mut self.input_failure) => Err(ForwarderEnd::InputFailed(reason)),
            () = stalled_past_deadline(self.pause_rx.clone(), self.stall_timeout) => {
                Err(ForwarderEnd::Stalled)
            }
        }
    }

    /// Take every notice already waiting.
    fn drain_notices(&mut self) {
        while let Some(notice) = self.notices.as_mut().and_then(NoticeSubscription::try_recv) {
            self.stash(notice);
        }
    }

    /// Keep a notice for the next cycle: a write to the clipboard replaces
    /// an unsent one. Anything else sends nothing (the runtime sends a
    /// subscription nothing else; module docs, "Notices").
    fn stash(&mut self, notice: Notice) {
        if let Notice::Clipboard { target, text } = notice
            && clipboard_target(target) == ClipboardTarget::Clipboard
        {
            self.clipboard = Some(text);
        }
    }

    /// Why the attachment's input failed, once it has.
    fn input_failure_now(&self) -> Option<String> {
        self.input_failure
            .as_ref()
            .and_then(|failure| failure.borrow().clone())
    }
}

/// The runtime's count of the emulator's rebuilds and resizes: what moves
/// history's layout besides erasures and the palette (`grid_output::Layout`).
fn relayouts(emulator: &EmulatorRef<'_>) -> u64 {
    emulator.rebuilds() + emulator.resizes()
}

/// The terminal's modes for `state`, with whether history is short as the
/// runtime knows it (a gap counts, which the emulator alone does not know).
fn state_of(emulator: &EmulatorRef<'_>) -> TerminalState {
    TerminalState {
        history_trimmed: emulator.history_trimmed(),
        ..grid_output::terminal_state(emulator)
    }
}

/// Resolve with the reason once the attachment's input has failed; never
/// for an attachment without an input side, or once its pump is gone
/// without saying.
async fn input_failed(failure: &mut Option<watch::Receiver<Option<String>>>) -> String {
    let Some(failure) = failure else {
        return std::future::pending().await;
    };
    loop {
        if let Some(reason) = failure.borrow_and_update().clone() {
            return reason;
        }
        if failure.changed().await.is_err() {
            return std::future::pending().await;
        }
    }
}

/// The next notice, or `None` once the runtime has ended; never for a
/// stream without a subscription.
async fn next_notice(notices: &mut Option<NoticeSubscription>) -> Option<Notice> {
    match notices {
        Some(subscription) => subscription.recv().await,
        None => std::future::pending().await,
    }
}

/// `state` with the program's exit as the host reports it.
fn with_exit(state: TerminalState, liveness: &Liveness) -> TerminalState {
    let exit = match liveness {
        Liveness::Exited {
            exit: ChildExit::Code(code),
        } => Some(ExitState::Code(*code)),
        Liveness::Exited {
            exit: ChildExit::Signal(signal),
        } => Some(ExitState::Signal(*signal)),
        Liveness::Running { .. } | Liveness::Gone | Liveness::Unavailable { .. } => None,
    };
    TerminalState { exit, ..state }
}

// ---- The task ------------------------------------------------------------------

/// One grid attachment's output task: a [`GridStream`] run until it ends or
/// the attachment is torn down, then the verdict reported through the
/// shared attachment machinery (`super::connection::report_forwarder_end`),
/// as every attachment's output task reports its own.
///
/// Like the forwarder, it never takes `Supervisor::attachments` (the
/// verdicts that need it run on tasks of their own), so teardown can
/// signal it and await it with that lock held. It holds no external
/// process, so its cleanup is confirmed as soon as it stops.
pub(crate) struct GridSender {
    pub(crate) sup: Arc<Supervisor>,
    pub(crate) key: AttachmentKey,
    pub(crate) channel: u32,
    pub(crate) tx: mpsc::Sender<Frame>,
    pub(crate) stream: GridStream,
    pub(crate) stall_timeout: Duration,
    /// Published once the task has stopped streaming
    /// (`ActiveAttach::forwarder_cleanup`).
    pub(crate) cleanup: watch::Sender<OutputReapOutcome>,
}

impl GridSender {
    /// The whole task: stream, confirm the cleanup, report the end.
    pub(crate) async fn run(self, mut shutdown: watch::Receiver<bool>) {
        let GridSender {
            sup,
            key,
            channel,
            tx,
            stream,
            stall_timeout,
            cleanup,
        } = self;
        let end = {
            let streaming = stream.run();
            tokio::pin!(streaming);
            tokio::select! {
                biased;
                _ = shutdown.changed() => ForwarderEnd::Requested,
                end = &mut streaming => end,
            }
        };
        cleanup.send_replace(Some(Ok(())));
        // As in `Forwarder::run`: a teardown that won after the stream
        // ended on its own owns the detach reason.
        let end = if *shutdown.borrow_and_update() {
            ForwarderEnd::Requested
        } else {
            end
        };
        report_forwarder_end(&sup, key, channel, tx, end, stall_timeout);
    }
}

// ---- Input ---------------------------------------------------------------------

/// A grid attachment's input side, and its `AttachBackend` (module docs,
/// "Input"): it decodes and encodes each message where the connection reads
/// it, without waiting, collects a paste until its last part, and owns the
/// pump that writes the bytes to the host. Dropping it tells the pump to
/// stop at once; a paste still being collected is dropped with it, never
/// written.
pub(crate) struct HostAttach {
    /// The terminal's status, for the modes and size input is encoded
    /// with (never the emulator, which the attachments lock must not wait
    /// on).
    status: watch::Receiver<HostStatus>,
    encoder: InputEncoder,
    /// The paste whose parts are arriving, encoded so far: nothing of it is
    /// handed on until its last part has arrived.
    paste: Option<CollectedPaste>,
    queue: mpsc::UnboundedSender<QueuedInput>,
    /// Bytes this attachment holds and the host's runtime has not yet
    /// written: a paste being collected, input queued for the pump, and a
    /// paste whose fate is not known yet ([`INPUT_BACKLOG`]).
    backlog: Arc<AtomicUsize>,
    /// Why the pump stopped on a failure, once it has: later input is
    /// refused.
    failure: watch::Receiver<Option<String>>,
    /// Tells the pump the attachment ended.
    cancel: Option<oneshot::Sender<()>>,
    /// The pump, owned by the attachment; it ends as soon as it sees
    /// `cancel`, so dropping the handle leaves nothing running. Read by
    /// tests.
    #[cfg_attr(not(test), expect(dead_code, reason = "tests read it"))]
    pump: tokio::task::JoinHandle<()>,
}

/// A paste being collected (`HostAttach::paste`). Its bytes count against
/// the backlog from the moment they are encoded.
struct CollectedPaste {
    bytes: Vec<u8>,
    /// The program had bracketed paste on when the paste began, so `bytes`
    /// starts with the opening marker and will end with the closing one.
    bracketed: bool,
}

/// Encoded input waiting for the pump.
enum QueuedInput {
    /// A key, text, a mouse or focus report: one-off input.
    OneOff(Vec<u8>),
    /// A whole paste, written through `InputSequence::send_paste`. Its
    /// bytes stay counted in the backlog until its fate is known.
    Paste { bytes: Vec<u8>, bracketed: bool },
}

impl HostAttach {
    /// A grid attachment's input side for `host`, and the watch its pump
    /// says why it failed on, for the attachment's [`GridStream`]. Starts
    /// the pump, with the one input sequence the attachment's input goes
    /// through.
    pub(crate) fn new(host: HostTerminal) -> (HostAttach, watch::Receiver<Option<String>>) {
        let (queue, queued) = mpsc::unbounded_channel();
        let (failure_tx, failure) = watch::channel(None);
        let (cancel, cancelled) = oneshot::channel();
        let backlog = Arc::new(AtomicUsize::new(0));
        let status = host.subscribe();
        let pump = Pump {
            input: host.input_sequence(),
            socket: host.socket().to_path_buf(),
            queue: queued,
            backlog: Arc::clone(&backlog),
            failure: failure_tx,
            fates: VecDeque::new(),
        };
        let pump = tokio::spawn(pump.run(cancelled));
        (
            HostAttach {
                status,
                encoder: InputEncoder::new(),
                paste: None,
                queue,
                backlog,
                failure: failure.clone(),
                cancel: Some(cancel),
                pump,
            },
            failure,
        )
    }

    /// Take one data frame of viewer input: decode it, encode it from the
    /// program's modes as last published, and queue the bytes for the pump,
    /// or, for a paste part, add them to the paste, which is queued whole
    /// with its last part. Returns whether the bytes submit a line, for the
    /// hook warning's first-input mark (`super::capture::submits_a_line`):
    /// input queued here is as good as delivered as far as that mark goes,
    /// and a bracketed paste never submits one.
    ///
    /// Never waits, on the program or on the emulator. An `Err` means the
    /// attachment's input is broken (module docs, "Input"), and the caller
    /// tears the attachment down; a paste being collected is then never
    /// written.
    pub(crate) fn send_input(&mut self, frame: &[u8]) -> anyhow::Result<bool> {
        if let Some(reason) = self.failure.borrow().clone() {
            anyhow::bail!(reason);
        }
        let input = GridInput::decode(frame).context("a malformed grid input message")?;
        let (modes, size) = {
            let status = self.status.borrow();
            (status.modes, status.size)
        };
        let size = farhelm_terminal_emulator::Size {
            columns: size.cols,
            lines: size.rows,
        };
        let bytes = self.encoder.encode(&input, &modes, size)?;
        match input {
            GridInput::Paste { last, .. } => self.collect_paste(bytes, last, modes.bracketed_paste),
            GridInput::Key { .. }
            | GridInput::Text { .. }
            | GridInput::Mouse { .. }
            | GridInput::Focus { .. } => {
                if bytes.is_empty() {
                    return Ok(false);
                }
                let submits = submits_a_line(&bytes);
                self.reserve(bytes.len())?;
                self.queue_for_pump(QueuedInput::OneOff(bytes))?;
                Ok(submits)
            }
        }
    }

    /// Add one encoded paste part to the paste, and queue the paste once its
    /// last part is in. `bracketed` is the program's mode now, which decides
    /// a paste's markers when its first part arrives (as the encoder
    /// decided).
    fn collect_paste(
        &mut self,
        bytes: Vec<u8>,
        last: bool,
        bracketed: bool,
    ) -> anyhow::Result<bool> {
        if self.paste.is_none() && last && bytes.is_empty() {
            // An empty paste of one part, which the encoder drops whole, as
            // today's paste path does.
            return Ok(false);
        }
        if let Err(refused) = self.reserve(bytes.len()) {
            // Nothing of the paste was handed on, and nothing will be.
            self.drop_paste();
            return Err(refused);
        }
        let paste = self.paste.get_or_insert_with(|| CollectedPaste {
            bytes: Vec::new(),
            bracketed,
        });
        paste.bytes.extend_from_slice(&bytes);
        if !last {
            return Ok(false);
        }
        let CollectedPaste { bytes, bracketed } = self.paste.take().expect("collected above");
        if bytes.is_empty() {
            // Unbracketed parts that were all empty: nothing to write.
            return Ok(false);
        }
        let submits = !bracketed && submits_a_line(&bytes);
        self.queue_for_pump(QueuedInput::Paste { bytes, bracketed })?;
        Ok(submits)
    }

    /// Count `len` more bytes against [`INPUT_BACKLOG`], or refuse them,
    /// counting nothing. Never waits: the attachments lock is held.
    fn reserve(&self, len: usize) -> anyhow::Result<()> {
        let held = self.backlog.fetch_add(len, Ordering::SeqCst) + len;
        if held > INPUT_BACKLOG {
            self.backlog.fetch_sub(len, Ordering::SeqCst);
            anyhow::bail!(
                "more than {} MiB of input is waiting: a paste that large, or a program that \
                 is not reading its input",
                INPUT_BACKLOG / (1024 * 1024)
            );
        }
        Ok(())
    }

    /// Forget the paste being collected, and what it counted.
    fn drop_paste(&mut self) {
        if let Some(paste) = self.paste.take() {
            self.backlog.fetch_sub(paste.bytes.len(), Ordering::SeqCst);
        }
    }

    /// Hand `input`, already counted in the backlog, to the pump.
    fn queue_for_pump(&self, input: QueuedInput) -> anyhow::Result<()> {
        self.queue
            .send(input)
            .map_err(|_| anyhow::anyhow!("the terminal's input has ended"))
    }

    /// Whether the pump has stopped. For tests.
    #[cfg(test)]
    pub(crate) fn pump_finished(&self) -> bool {
        self.pump.is_finished()
    }
}

impl Drop for HostAttach {
    /// Tell the pump the attachment ended; never waits for it.
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
    }
}

/// An input's fate, awaited in the order the inputs were sent.
type PendingFate = Pin<Box<dyn Future<Output = InputFate> + Send>>;

/// The input pump: writes an attachment's queued input to the host in
/// order, through the attachment's one input sequence, and watches the
/// fates (module docs, "Input").
struct Pump {
    /// The attachment's one input sequence (module docs, "Input").
    input: InputSequence,
    /// The host's socket, for the log.
    socket: std::path::PathBuf,
    queue: mpsc::UnboundedReceiver<QueuedInput>,
    backlog: Arc<AtomicUsize>,
    failure: watch::Sender<Option<String>>,
    /// Fates of the input sent, oldest first.
    fates: VecDeque<PendingFate>,
}

impl Pump {
    /// Handle queued input until the attachment ends or its input breaks,
    /// then stop. What it has handed to the host's runtime is the runtime's
    /// to write (a paste whole, or closed if a loss cuts it short); what it
    /// has not is dropped.
    async fn run(mut self, mut cancelled: oneshot::Receiver<()>) {
        let failed = loop {
            let oldest = async {
                match self.fates.front_mut() {
                    Some(fate) => fate.as_mut().await,
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                biased;
                _ = &mut cancelled => break None,
                fate = oldest => {
                    self.fates.pop_front();
                    if let Some(reason) = lost(fate) {
                        break Some(reason);
                    }
                }
                queued = self.queue.recv() => {
                    let Some(input) = queued else {
                        break None;
                    };
                    // Cancelling the hand-over is safe: a send that has not
                    // returned queued nothing (`InputSequence::send`).
                    let handled = tokio::select! {
                        biased;
                        _ = &mut cancelled => break None,
                        handled = self.handle(input) => handled,
                    };
                    if let Err(reason) = handled {
                        break Some(reason);
                    }
                }
            }
        };
        if let Some(reason) = failed {
            warn!(socket = %self.socket.display(), %reason, "a grid viewer's input failed");
            self.failure.send_replace(Some(reason));
        }
        // What was not handed to the host goes no further: its attachment
        // has ended, or its input broke.
        self.queue.close();
    }

    /// Hand one queued input to the host's runtime and keep its fate.
    async fn handle(&mut self, input: QueuedInput) -> Result<(), String> {
        match input {
            QueuedInput::OneOff(bytes) => {
                self.backlog.fetch_sub(bytes.len(), Ordering::SeqCst);
                for piece in input_frames(&bytes) {
                    let receipt = self
                        .input
                        .send(piece)
                        .await
                        .map_err(|refused| refused.to_string())?;
                    self.fates.push_back(Box::pin(receipt.fate()));
                }
            }
            QueuedInput::Paste { bytes, bracketed } => {
                let len = bytes.len();
                let sent = self.input.send_paste(bytes, bracketed).await;
                let receipt = match sent {
                    Ok(receipt) => receipt,
                    Err(refused) => {
                        self.backlog.fetch_sub(len, Ordering::SeqCst);
                        return Err(refused.to_string());
                    }
                };
                // The runtime holds the paste until it is written, so the
                // backlog counts it until then.
                let backlog = Arc::clone(&self.backlog);
                self.fates.push_back(Box::pin(async move {
                    let fate = receipt.fate().await;
                    backlog.fetch_sub(len, Ordering::SeqCst);
                    fate
                }));
            }
        }
        Ok(())
    }
}

/// Why an input's fate breaks the attachment's input, if it does: input
/// that may have been lost, or that was not sent after other input was.
/// Input the pty discarded does not.
fn lost(fate: InputFate) -> Option<String> {
    match fate {
        InputFate::Written | InputFate::Discarded => None,
        InputFate::Unknown => Some(
            "the connection to the terminal host ended before it confirmed the input, so some \
             of it may be lost"
                .to_string(),
        ),
        InputFate::NotSent => Some("input was not sent after earlier input was lost".to_string()),
    }
}

#[cfg(test)]
mod tests;
