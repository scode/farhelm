//! Tests of a grid attachment's sender and input side, against a stand-in
//! host in this test process (`hosts::terminal::test_host`): the attach
//! sequence while output runs, and a long catch-up that loses no output or
//! exit and leaves a resize drag to settle; history kept whole through a
//! paused flood and a full writer queue, eviction at the scrollback cap, a
//! resize, an erasure and a restore resetting history (a resize drag only
//! once), no change lost between a read and its bookkeeping, the stall
//! bound, an input failure while paused, clipboard writes, the exit and an
//! unreadable host; and input: keys,
//! text and pastes, a paste written whole or not at all (query answers and
//! other input never inside it, nothing of one cut short or past the
//! backlog, closed first after a lost connection), the pump ending with its
//! attachment, and input never waiting on the emulator.
//!
//! Each stream test runs a [`GridStream`] into a channel standing in for
//! the connection's writer queue and applies what arrives to a test client
//! (`grid_output::test_client`), which refuses any stream a viewer would
//! refuse. "Converged" means the client shows exactly what the emulator
//! holds: screen, cursor and every history line.

use super::*;
use crate::grid_output::test_client::{TestClient, held_history, row_text};
use crate::hosts::terminal::test_host::{
    TestHost, TestHostSpec, adopt_test_host_with, test_host_options,
};
use crate::hosts::{Adoption, HostTerminalOptions};
use farhelm_proto::FrameKind;
use farhelm_proto::grid::ExitState;

/// How long any wait in these tests may take before it fails.
const WAIT: Duration = Duration::from_secs(10);

/// The stall timeout of streams whose test is not about stalling: far
/// longer than any test, so a pause never ends one.
const NO_STALL: Duration = Duration::from_secs(3600);

/// What the emulator answers a primary device attributes query (DA1).
const DA1_ANSWER: &[u8] = b"\x1b[?1;2c";

/// `count` numbered lines from `from`, each ended with CR LF.
fn numbered(from: usize, count: usize) -> Vec<u8> {
    (from..from + count)
        .flat_map(|n| format!("line {n:04}\r\n").into_bytes())
        .collect()
}

/// `line from` to `line to - 1`, as the client's history texts read.
fn lines(from: usize, to: usize) -> Vec<String> {
    (from..to).map(|n| format!("line {n:04}")).collect()
}

/// A stand-in host keeping `scrollback` lines of history.
async fn host_with(scrollback: usize) -> (HostTerminal, TestHost) {
    adopt_test_host_with(
        TestHostSpec {
            options: Some(HostTerminalOptions {
                scrollback_lines: scrollback,
                ..test_host_options(None)
            }),
            ..TestHostSpec::default()
        },
        Adoption::Fresh,
    )
    .await
}

/// How a test's [`Viewer`] is set up.
struct ViewerOptions {
    stall_timeout: Duration,
    /// Subscribe to the terminal's notices.
    notices: bool,
    /// The depth of the channel standing in for the writer queue.
    queue: usize,
    /// Pause the stream after each read until the test says (the
    /// `after_read` seam).
    hold_after_reads: bool,
    /// The attachment's input failure, as its input pump would set it.
    input_failure: Option<watch::Receiver<Option<String>>>,
}

impl Default for ViewerOptions {
    fn default() -> Self {
        ViewerOptions {
            stall_timeout: NO_STALL,
            notices: false,
            queue: super::super::connection::CONNECTION_WRITER_QUEUE,
            hold_after_reads: false,
            input_failure: None,
        }
    }
}

/// One grid attachment under test: its stream running into a channel, and
/// a test client applying what arrives.
struct Viewer {
    host: HostTerminal,
    rx: mpsc::Receiver<Frame>,
    pause: watch::Sender<Option<tokio::time::Instant>>,
    /// How many times the stream parked for a pause.
    parked: watch::Receiver<u64>,
    /// The stream's reads, each waiting for the test to let it go on, when
    /// `hold_after_reads`.
    reads: Option<mpsc::UnboundedReceiver<oneshot::Sender<()>>>,
    client: TestClient,
    /// The data messages applied before `ReplayComplete`, once it arrived.
    replay_complete_after: Option<usize>,
    stream: tokio::task::JoinHandle<ForwarderEnd>,
}

impl Drop for Viewer {
    fn drop(&mut self) {
        self.stream.abort();
    }
}

impl Viewer {
    /// Start streaming `host` with the default options.
    fn start(host: &HostTerminal) -> Viewer {
        Viewer::start_with(host, ViewerOptions::default())
    }

    fn start_with(host: &HostTerminal, options: ViewerOptions) -> Viewer {
        let (tx, rx) = mpsc::channel(options.queue);
        let (pause, pause_rx) = watch::channel(None);
        let (parked_tx, parked) = watch::channel(0);
        let (reads_tx, reads) = mpsc::unbounded_channel();
        let stream = GridStream::new(GridStreamParts {
            channel: 1,
            tx,
            host: host.clone(),
            notices: options.notices.then(|| host.subscribe_notices()),
            pause_rx,
            stall_timeout: options.stall_timeout,
            attach_size: None,
            input_failure: options.input_failure,
            seams: StreamSeams {
                after_read: options.hold_after_reads.then_some(reads_tx),
                parked: Some(parked_tx),
            },
        });
        Viewer {
            host: host.clone(),
            rx,
            pause,
            parked,
            reads: options.hold_after_reads.then_some(reads),
            client: TestClient::new(),
            replay_complete_after: None,
            stream: tokio::spawn(stream.run()),
        }
    }

    /// Apply one frame from the stream.
    fn take(&mut self, frame: Frame) {
        match frame.kind {
            FrameKind::Data => {
                assert_eq!(frame.channel, 1, "data on another channel");
                self.client.apply(&frame.body);
            }
            FrameKind::Control => {
                let message: ControlMsg =
                    serde_json::from_slice(&frame.body).expect("a control message");
                assert!(
                    matches!(message, ControlMsg::ReplayComplete { channel: 1 }),
                    "unexpected control message {message:?}"
                );
                assert_eq!(self.replay_complete_after, None, "a second ReplayComplete");
                self.replay_complete_after = Some(self.client.kinds.len());
            }
        }
    }

    /// Apply frames until `done` holds, re-checking on every frame and every
    /// change of the terminal; fails after [`WAIT`]. A stream holding at
    /// its `after_read` seam is let go on at once.
    async fn until(&mut self, what: &str, done: impl Fn(&Viewer) -> bool) {
        let mut status = self.host.subscribe();
        let reached = tokio::time::timeout(WAIT, async {
            while !done(self) {
                let read = async {
                    match &mut self.reads {
                        Some(reads) => reads.recv().await,
                        None => std::future::pending().await,
                    }
                };
                tokio::select! {
                    frame = self.rx.recv() => match frame {
                        Some(frame) => self.take(frame),
                        None => return false,
                    },
                    go = read => drop(go),
                    changed = status.changed() => {
                        if changed.is_err() {
                            return done(self);
                        }
                    }
                }
            }
            true
        })
        .await;
        assert_eq!(
            reached,
            Ok(true),
            "{what}: the client has {:?}, {} history lines, after {:?}",
            (self.client.cols, self.client.rows),
            self.client.history.len(),
            self.client.kinds
        );
    }

    /// Whether the attach's catch-up is over and the client shows exactly
    /// what the emulator holds, with every output byte `end` the test sent
    /// processed.
    fn converged(&self, end: u64) -> bool {
        self.replay_complete_after.is_some()
            && self.host.status().position.offset >= end
            && self
                .host
                .try_read(|emulator| self.client.shows(emulator))
                .unwrap_or(false)
    }

    fn pause(&self) {
        self.pause.send_replace(Some(tokio::time::Instant::now()));
    }

    fn resume(&self) {
        self.pause.send_replace(None);
    }

    /// Wait until the stream has parked for the pause more than `before`
    /// times: it had work and found the viewer paused.
    async fn parked_after(&mut self, before: u64) {
        let parked = tokio::time::timeout(WAIT, self.parked.wait_for(|count| *count > before))
            .await
            .is_ok_and(|parked| parked.is_ok());
        assert!(parked, "the stream did not reach the pause");
    }

    /// The texts of the client's history lines, oldest first.
    fn history_texts(&self) -> Vec<String> {
        self.client
            .history
            .iter()
            .map(|(_, _, row)| row_text(row))
            .collect()
    }
}

/// Wait until the runtime has processed `end` bytes of output.
async fn processed(host: &HostTerminal, end: u64) {
    let mut status = host.subscribe();
    let reached = tokio::time::timeout(WAIT, async {
        while host.status().position.offset < end {
            if status.changed().await.is_err() {
                break;
            }
        }
    })
    .await;
    assert!(
        reached.is_ok() && host.status().position.offset >= end,
        "the runtime processed {:?} of {end} bytes",
        host.status().position
    );
}

/// Wait until the runtime's status says the terminal is `size`.
async fn sized(host: &HostTerminal, size: Size) {
    let mut status = host.subscribe();
    let reached = tokio::time::timeout(WAIT, status.wait_for(|status| status.size == size)).await;
    assert!(
        reached.is_ok_and(|reached| reached.is_ok()),
        "the terminal did not become {size:?}"
    );
}

/// The snapshot blob of an 80x24 emulator that printed `text`, for a test
/// host to have the runtime restore.
fn snapshot_of(text: &[u8]) -> Vec<u8> {
    let mut emulator = farhelm_terminal_emulator::Emulator::new(
        farhelm_terminal_emulator::Size {
            columns: 80,
            lines: 24,
        },
        1000,
    );
    emulator.feed(
        text,
        std::time::Instant::now(),
        farhelm_terminal_emulator::FeedMode::Live,
        &mut farhelm_terminal_emulator::Collected::default(),
    );
    emulator.snapshot(1 << 20).expect("a clean boundary").blob
}

// ---- The attach sequence ---------------------------------------------------------

/// An attach while the program prints sends `hello`, `state`, the screen,
/// then the history, then `ReplayComplete`, and the viewer ends up with
/// every line.
///
/// Why: the screen first is what makes a terminal usable at once while a
/// long history follows, and output arriving during the catch-up must
/// neither be lost nor break the history's numbering. Spec: with 100 lines
/// printed before the attach and 300 more sent right after it, the first
/// messages are `hello` and `state`, the first frame comes before the first
/// history message, `ReplayComplete` comes after it, and once all output is
/// processed the client shows exactly the emulator's screen and history,
/// whose lines run from `line 0000` to `line 0376` without a gap.
#[farhelm_testtrace::test]
async fn an_attach_during_output_sends_the_screen_then_the_history() {
    let (host, test_host) = host_with(1000).await;
    let before = numbered(0, 100);
    test_host.output(&before);
    processed(&host, before.len() as u64).await;
    let mut viewer = Viewer::start(&host);
    let after = numbered(100, 300);
    test_host.output(&after);
    let end = (before.len() + after.len()) as u64;
    viewer
        .until("converged", |viewer| viewer.converged(end))
        .await;

    let kinds = &viewer.client.kinds;
    assert_eq!(&kinds[..2], ["hello", "state"]);
    let first_frame = kinds.iter().position(|kind| *kind == "frame").unwrap();
    let first_history = kinds.iter().position(|kind| *kind == "history").unwrap();
    assert!(first_frame < first_history, "{kinds:?}");
    assert!(viewer.replay_complete_after.unwrap() > first_history);
    assert_eq!(viewer.history_texts(), lines(0, 377));
}

/// An attach to 2,200 lines with a writer queue of one frame: the stream
/// is left holding its first messages, in the middle of a catch-up of
/// several history reads, once `hello` has arrived (its first read done).
async fn stalled_long_catch_up(host: &HostTerminal, test_host: &TestHost) -> Viewer {
    let before = numbered(0, 2200);
    test_host.output(&before);
    processed(host, before.len() as u64).await;
    let mut viewer = Viewer::start_with(
        host,
        ViewerOptions {
            queue: 1,
            ..ViewerOptions::default()
        },
    );
    viewer
        .until("hello", |viewer| !viewer.client.kinds.is_empty())
        .await;
    assert!(
        viewer.replay_complete_after.is_none(),
        "premise: the catch-up is still going"
    );
    viewer
}

/// Output that comes during a long catch-up reaches the viewer even when
/// the program then goes quiet.
///
/// Why: the catch-up's reads after the first take history only, and mark
/// the terminal's status seen; a change they consumed would never reach
/// the viewer's screen, nor lines past the catch-up's end its history,
/// until something else happened. Spec: with 2,200 lines printed before
/// the attach and the stream held after `hello`, 2,000 more lines are
/// printed and processed and nothing after them; the client then converges
/// with every line from `line 0000` to `line 4176` in its history, once.
#[farhelm_testtrace::test]
async fn output_during_a_long_catch_up_is_not_lost() {
    let (host, test_host) = host_with(5000).await;
    let mut viewer = stalled_long_catch_up(&host, &test_host).await;
    test_host.output(&numbered(2200, 2000));
    let end = (numbered(0, 4200)).len() as u64;
    processed(&host, end).await;
    viewer
        .until("converged", |viewer| viewer.converged(end))
        .await;
    assert_eq!(viewer.history_texts(), lines(0, 4177));
}

/// An exit that comes during a long catch-up reaches the viewer.
///
/// Why: after an exit the runtime has little left to publish, so an exit
/// a catch-up read consumed could be missing from the viewer for good.
/// Spec: with 2,200 lines printed before the attach and the stream held
/// after `hello`, `bye` is printed and the child exits with 3, both
/// processed; the client then converges and its state shows exit code 3.
#[farhelm_testtrace::test]
async fn an_exit_during_a_long_catch_up_is_not_lost() {
    let (host, test_host) = host_with(5000).await;
    let mut viewer = stalled_long_catch_up(&host, &test_host).await;
    test_host.output(b"bye\r\n");
    test_host.exit(3);
    let mut status = host.subscribe();
    let exited = tokio::time::timeout(
        WAIT,
        status.wait_for(|status| matches!(status.liveness, Liveness::Exited { .. })),
    )
    .await;
    assert!(exited.is_ok_and(|exited| exited.is_ok()), "the exit");
    let end = (numbered(0, 2200).len() + 5) as u64;
    viewer
        .until("converged with the exit", |viewer| {
            viewer.converged(end)
                && viewer.client.state.and_then(|state| state.exit) == Some(ExitState::Code(3))
        })
        .await;
}

/// A resize drag during a long catch-up ends the catch-up, and the
/// viewer's history is reset once the drag settles, not at every step.
///
/// Why: at the scrollback cap a resend costs megabytes, and a drag resizes
/// the terminal many times a second; the catch-up must leave the resend to
/// the live path's settle rule rather than start over at every step.
/// Spec: with 2,200 lines printed before the attach and the stream held
/// after `hello`, the terminal is resized to 60 to 67 columns, each taken
/// by the terminal and shown to the client (a frame of that width, so the
/// stream saw every step's layout) before the next; the client then
/// converges at 67 columns with its history at that width, after fewer
/// resets than half the resizes (one, unless the machine stretches a step
/// past the settle time), every one of them after `ReplayComplete`.
#[farhelm_testtrace::test]
async fn a_resize_drag_during_a_long_catch_up_resets_once_it_settles() {
    let (host, test_host) = host_with(5000).await;
    let mut viewer = stalled_long_catch_up(&host, &test_host).await;
    let steps: u16 = 8;
    for step in 0..steps {
        let size = Size {
            cols: 60 + step,
            rows: 24,
        };
        host.resize(size).expect("a valid size");
        sized(&host, size).await;
        viewer
            .until("a frame at the step's width", |viewer| {
                viewer.client.cols == size.cols
            })
            .await;
    }
    let last = 60 + steps - 1;
    let end = numbered(0, 2200).len() as u64;
    viewer
        .until("converged at the last size", |viewer| {
            viewer.client.cols == last
                && viewer.converged(end)
                && viewer
                    .client
                    .history
                    .iter()
                    .all(|(_, cols, _)| *cols == last)
        })
        .await;
    assert!(
        viewer.client.resets >= 1 && viewer.client.resets < usize::from(steps) / 2,
        "{} resets for {steps} resizes",
        viewer.client.resets
    );
    let replay_complete = viewer.replay_complete_after.expect("converged");
    let first_reset = viewer
        .client
        .kinds
        .iter()
        .position(|kind| *kind == "history_reset")
        .expect("a reset");
    assert!(first_reset >= replay_complete, "{:?}", viewer.client.kinds);
}

// ---- Flow control ----------------------------------------------------------------

/// A flood longer than a screen while the viewer is paused is withheld
/// while it is paused, and reaches it once it resumes as appended history:
/// no line is lost, none is repeated, and history is not reset.
///
/// Why: coalescing applies to the visible screen only; lines that scrolled
/// into history while the viewer was behind must still reach it (SPEC.md:
/// flow control never drops terminal output), and a paused viewer must get
/// nothing meanwhile. Spec: after the attach, the viewer pauses, 200 lines
/// are printed on a 24-row screen and processed, and the stream reaches the
/// pause with that work to do; nothing has arrived since the pause; then
/// the viewer resumes, and its history holds every line from `line 0000`
/// to `line 0176` once, in order, with no `history_reset`.
#[farhelm_testtrace::test]
async fn a_paused_flood_is_withheld_and_loses_no_history_line() {
    let (host, test_host) = host_with(1000).await;
    let mut viewer = Viewer::start(&host);
    viewer
        .until("the attach caught up", |viewer| viewer.converged(0))
        .await;
    let parked = *viewer.parked.borrow();
    viewer.pause();
    let flood = numbered(0, 200);
    test_host.output(&flood);
    processed(&host, flood.len() as u64).await;
    viewer.parked_after(parked).await;
    assert!(
        viewer.rx.try_recv().is_err(),
        "the stream sent something while its viewer was paused"
    );
    viewer.resume();
    viewer
        .until("converged", |viewer| viewer.converged(flood.len() as u64))
        .await;
    assert_eq!(viewer.history_texts(), lines(0, 177));
    assert_eq!(viewer.client.resets, 0);
}

/// A writer queue that stays full while a flood crosses several history
/// batches loses no line either.
///
/// Why: the other way a sender falls behind is a slow link: it waits for
/// room in the connection's queue, and the history it owes grows past what
/// one cycle sends ([`LIVE_HISTORY_LINES`]); the owed lines must still go
/// out in order, as appends. Spec: with a writer queue of 4 frames that the
/// test does not read, 4,000 lines are printed and processed, and the queue
/// fills; then the test reads, and the client's history holds `line 0000`
/// to `line 3976` once, in order, with no `history_reset`.
#[farhelm_testtrace::test]
async fn a_full_writer_queue_loses_no_history_line() {
    let (host, test_host) = host_with(10_000).await;
    let mut viewer = Viewer::start_with(
        &host,
        ViewerOptions {
            queue: 4,
            ..ViewerOptions::default()
        },
    );
    viewer
        .until("the attach caught up", |viewer| viewer.converged(0))
        .await;
    let flood = numbered(0, 4000);
    test_host.output(&flood);
    processed(&host, flood.len() as u64).await;
    let filled = tokio::time::timeout(WAIT, async {
        while viewer.rx.len() < 4 {
            tokio::time::sleep(Duration::from_millis(10)).await; // sleep-ok: polling the channel's length, which offers no wait for its filling
        }
    })
    .await;
    assert!(filled.is_ok(), "premise: the writer queue filled");
    viewer
        .until("converged", |viewer| viewer.converged(flood.len() as u64))
        .await;
    assert_eq!(viewer.history_texts(), lines(0, 3977));
    assert_eq!(viewer.client.resets, 0);
}

/// Lines that scrolled past the scrollback cap before a paused viewer got
/// them make the sender reset the viewer's history and send what is held.
///
/// Why: an append cannot skip lines, and the client refuses a gap; the
/// lines it missed are gone from the supervisor too, so a reset leaves the
/// viewer with exactly the supervisor's history. Spec: with 30 lines of
/// scrollback and 10 lines already sent, a paused flood of 167 lines
/// (withheld while paused) ends, after resuming, in at least one
/// `history_reset` and a history of the 30 lines the emulator holds,
/// `line 0147` to `line 0176`.
#[farhelm_testtrace::test]
async fn eviction_at_the_cap_resets_the_viewers_history() {
    let (host, test_host) = host_with(30).await;
    let mut viewer = Viewer::start(&host);
    let first = numbered(0, 33);
    test_host.output(&first);
    viewer
        .until("ten lines in history", |viewer| {
            viewer.converged(first.len() as u64)
        })
        .await;
    assert_eq!(viewer.client.history.len(), 10, "premise: ten lines sent");
    let parked = *viewer.parked.borrow();
    viewer.pause();
    let flood = numbered(33, 167);
    test_host.output(&flood);
    let end = (first.len() + flood.len()) as u64;
    processed(&host, end).await;
    viewer.parked_after(parked).await;
    assert!(viewer.rx.try_recv().is_err(), "sent while paused");
    viewer.resume();
    viewer
        .until("converged", |viewer| viewer.converged(end))
        .await;
    assert!(viewer.client.resets >= 1);
    assert_eq!(viewer.history_texts(), lines(147, 177));
}

/// A resize resets the viewer's history and sends it again at the new
/// width, with a frame of the new size.
///
/// Why: a resize reflows history and moves lines between it and the
/// screen, so line numbers sent before no longer name the same lines; an
/// append cannot express that. Spec: after 60 lines on 80 columns, a
/// resize to 50 columns ends with one `history_reset`, the client at 50
/// columns, and its screen and history the emulator's.
#[farhelm_testtrace::test]
async fn a_resize_resets_the_viewers_history() {
    let (host, test_host) = host_with(1000).await;
    let mut viewer = Viewer::start(&host);
    let output = numbered(0, 60);
    test_host.output(&output);
    viewer
        .until("converged", |viewer| viewer.converged(output.len() as u64))
        .await;
    assert_eq!(
        viewer.client.resets, 0,
        "premise: no reset before the resize"
    );
    host.resize(Size { cols: 50, rows: 24 })
        .expect("a valid size");
    // The lines are short enough that their text does not reflow: only
    // their width says they were sent again.
    viewer
        .until("converged at the new size", |viewer| {
            viewer.client.cols == 50
                && viewer.converged(output.len() as u64)
                && viewer.client.history.iter().all(|(_, cols, _)| *cols == 50)
        })
        .await;
    assert_eq!(viewer.client.resets, 1);
}

/// A resize drag resets the viewer's history once it settles, not at every
/// step.
///
/// Why: each resize reflows the history, and resending a full scrollback
/// costs megabytes; a viewer whose window is dragged resizes the terminal
/// many times a second. Spec: after 60 lines, eight resizes in a row, each
/// taken by the terminal before the next and all well within
/// [`RELAYOUT_SETTLE`] on a quiet machine, end with the client converged at
/// the last size after fewer resets than half the resizes (one, unless the
/// machine is slow enough to stretch a step past the settle time).
#[farhelm_testtrace::test]
async fn a_resize_drag_resets_history_once_it_settles() {
    let (host, test_host) = host_with(1000).await;
    let mut viewer = Viewer::start(&host);
    let output = numbered(0, 60);
    test_host.output(&output);
    viewer
        .until("converged", |viewer| viewer.converged(output.len() as u64))
        .await;
    let steps: u16 = 8;
    for step in 0..steps {
        let size = Size {
            cols: 60 + step,
            rows: 24,
        };
        host.resize(size).expect("a valid size");
        sized(&host, size).await;
    }
    let last = 60 + steps - 1;
    viewer
        .until("converged at the last size", |viewer| {
            viewer.client.cols == last
                && viewer.converged(output.len() as u64)
                && viewer
                    .client
                    .history
                    .iter()
                    .all(|(_, cols, _)| *cols == last)
        })
        .await;
    assert!(
        viewer.client.resets >= 1 && viewer.client.resets < usize::from(steps) / 2,
        "{} resets for {steps} resizes",
        viewer.client.resets
    );
}

/// Erasing history (`CSI 3 J`) and a restored emulator each reset the
/// viewer's history through the stream.
///
/// Why: after either, the history the viewer holds is not the emulator's:
/// erased, or replaced by the restored emulator's. Spec: after 60 lines,
/// `CSI 3 J` ends with a reset and an empty client history; then a restore
/// of a snapshot of 40 other lines (`line 0500` on) ends with another reset
/// and the client's history the restored emulator's, `line 0500` to `line
/// 0516`.
#[farhelm_testtrace::test]
async fn an_erasure_and_a_restore_reset_the_viewers_history() {
    let (host, test_host) = host_with(1000).await;
    let mut viewer = Viewer::start(&host);
    let output = numbered(0, 60);
    test_host.output(&output);
    viewer
        .until("converged", |viewer| viewer.converged(output.len() as u64))
        .await;
    test_host.output(b"\x1b[3J");
    let erased = output.len() as u64 + 4;
    viewer
        .until("the erasure", |viewer| {
            viewer.client.resets == 1 && viewer.converged(erased)
        })
        .await;
    assert!(viewer.client.history.is_empty());

    test_host.restore(snapshot_of(&numbered(500, 40)));
    viewer
        .until("the restore", |viewer| {
            viewer.client.resets == 2 && viewer.converged(erased + 1)
        })
        .await;
    assert_eq!(viewer.history_texts(), lines(500, 517));
}

/// Output published between a read and the stream's bookkeeping of it, and
/// then silence, still reaches the viewer, and so does an exit published
/// there.
///
/// Why: a sender that marks the terminal's status seen after its read can
/// mark a change it did not read, and then sleep through the silence that
/// follows, leaving the viewer without the program's last output, or its
/// exit, until something else happens. Spec: holding the stream right after
/// a live read, `last line` is printed and processed, and the stream is let
/// go on with nothing printed after it; the client then shows `last line`.
/// Holding it again, the child exits with 3; the client then shows the
/// exit.
#[farhelm_testtrace::test]
async fn a_change_published_after_a_read_is_not_lost() {
    let (host, test_host) = host_with(1000).await;
    let mut viewer = Viewer::start_with(
        &host,
        ViewerOptions {
            hold_after_reads: true,
            ..ViewerOptions::default()
        },
    );
    viewer
        .until("the attach caught up", |viewer| viewer.converged(0))
        .await;
    test_host.output(b"first\r\n");
    // The next read is the live cycle for `first`; hold it there.
    let held = tokio::time::timeout(WAIT, viewer.reads.as_mut().unwrap().recv())
        .await
        .expect("the stream reads")
        .expect("a held read");
    test_host.output(b"last line\r\n");
    processed(&host, 18).await;
    drop(held);
    viewer
        .until("the last output", |viewer| {
            viewer
                .client
                .screen
                .iter()
                .any(|row| row_text(row) == "last line")
        })
        .await;

    test_host.output(b"x");
    let held = tokio::time::timeout(WAIT, viewer.reads.as_mut().unwrap().recv())
        .await
        .expect("the stream reads")
        .expect("a held read");
    test_host.exit(3);
    let mut status = host.subscribe();
    let exited = tokio::time::timeout(
        WAIT,
        status.wait_for(|status| matches!(status.liveness, Liveness::Exited { .. })),
    )
    .await;
    assert!(exited.is_ok_and(|exited| exited.is_ok()), "the exit");
    drop(held);
    viewer
        .until("the exit", |viewer| {
            viewer.client.state.and_then(|state| state.exit) == Some(ExitState::Code(3))
        })
        .await;
}

/// A pause that outlives the stall timeout ends the stream as stalled.
///
/// Why: a viewer that stops reading without closing its socket must not
/// hold its attachment forever; the stall detach is what ends it, for grid
/// streams as for byte streams. Spec: with a 200 ms stall timeout, a
/// viewer that pauses after the attach and stays paused while output
/// arrives has its stream end with `Stalled` well within the test's wait.
#[farhelm_testtrace::test]
async fn a_pause_past_the_stall_timeout_ends_the_stream() {
    let (host, test_host) = host_with(1000).await;
    let mut viewer = Viewer::start_with(
        &host,
        ViewerOptions {
            stall_timeout: Duration::from_millis(200),
            ..ViewerOptions::default()
        },
    );
    viewer
        .until("the attach caught up", |viewer| viewer.converged(0))
        .await;
    viewer.pause();
    test_host.output(b"more output\r\n");
    let ended = tokio::time::timeout(WAIT, &mut viewer.stream)
        .await
        .expect("the stream ends")
        .expect("the stream task does not panic");
    assert!(matches!(ended, ForwarderEnd::Stalled));
}

/// A paused stream ends at once when its attachment's input fails, as an
/// input failure.
///
/// Why: a viewer that paused while its input broke would otherwise keep a
/// broken attachment until it resumed or the stall timeout ran out, and be
/// told then that it stalled. Spec: with no stall timeout to speak of, the
/// viewer pauses, output arrives and the stream parks with it; the input
/// failure `broken` is then set, and the stream ends with it as an input
/// failure without the viewer resuming.
#[farhelm_testtrace::test]
async fn a_paused_stream_ends_when_its_input_fails() {
    let (host, test_host) = host_with(1000).await;
    let (failure_tx, failure) = watch::channel(None);
    let mut viewer = Viewer::start_with(
        &host,
        ViewerOptions {
            input_failure: Some(failure),
            ..ViewerOptions::default()
        },
    );
    viewer
        .until("the attach caught up", |viewer| viewer.converged(0))
        .await;
    let parked = *viewer.parked.borrow();
    viewer.pause();
    test_host.output(b"more output\r\n");
    viewer.parked_after(parked).await;
    failure_tx.send_replace(Some("broken".to_owned()));
    let ended = tokio::time::timeout(WAIT, &mut viewer.stream)
        .await
        .expect("the stream ends")
        .expect("the stream task does not panic");
    let ForwarderEnd::InputFailed(reason) = ended else {
        panic!("the stream did not end as an input failure");
    };
    assert_eq!(reason, "broken");
}

// ---- Notices, exit and end ---------------------------------------------------------

/// Clipboard writes reach the viewer whole, and while it is behind only
/// the latest unsent write is kept; neither a burst of bells nor writes to
/// the primary selection push a write out, and the latter are not sent.
///
/// Why: a program's OSC 52 write is a copy the user expects; a burst of
/// them while the viewer is paused must not queue without bound, the format
/// keeps the newest, and bells and primary-selection writes, which the
/// client does nothing with, must not cost it a write. Spec: a write of
/// `copied` followed by 40 bells in one read arrives as one clipboard part;
/// while paused, writes of `first` and `second` to the clipboard, each
/// followed by ten writes to the primary selection, arrive, after resuming,
/// as `second` alone.
#[farhelm_testtrace::test]
async fn clipboard_writes_arrive_whole_and_latest_per_target() {
    let (host, test_host) = host_with(1000).await;
    let mut viewer = Viewer::start_with(
        &host,
        ViewerOptions {
            notices: true,
            ..ViewerOptions::default()
        },
    );
    viewer
        .until("the attach caught up", |viewer| viewer.converged(0))
        .await;
    let mut write = b"\x1b]52;c;Y29waWVk\x07".to_vec();
    write.extend([0x07; 40]);
    test_host.output(&write);
    viewer
        .until("the first write", |viewer| {
            !viewer.client.clipboard.is_empty()
        })
        .await;
    assert_eq!(
        viewer.client.clipboard,
        vec![(ClipboardTarget::Clipboard, "copied".to_owned())]
    );

    let parked = *viewer.parked.borrow();
    viewer.pause();
    // `first` and `second` to the clipboard, each followed by ten writes
    // of `selected` to the primary selection: more than a subscription's
    // backlog, had the runtime sent them.
    let primary = b"\x1b]52;p;c2VsZWN0ZWQ=\x07".repeat(10);
    let writes = [
        &b"\x1b]52;c;Zmlyc3Q=\x07"[..],
        &primary,
        b"\x1b]52;c;c2Vjb25k\x07",
        &primary,
    ]
    .concat();
    test_host.output(&writes);
    let end = (write.len() + writes.len()) as u64;
    processed(&host, end).await;
    viewer.parked_after(parked).await;
    viewer.resume();
    viewer
        .until("the writes after the resume", |viewer| {
            viewer.client.clipboard.len() >= 2 && viewer.converged(end)
        })
        .await;
    assert_eq!(
        viewer.client.clipboard[1..],
        [(ClipboardTarget::Clipboard, "second".to_owned())]
    );
}

/// The program's exit shows in `state`, and the stream goes on showing the
/// final screen.
///
/// Why: a host terminal has no "Pane is dead" banner; the client shows the
/// exit itself, from `state`, over the screen the program left. Spec: after
/// `bye` and an exit with status 5, the client's state says code 5, its
/// screen still shows `bye`, and the stream has not ended.
#[farhelm_testtrace::test]
async fn the_exit_shows_in_state_and_the_stream_goes_on() {
    let (host, test_host) = host_with(1000).await;
    let mut viewer = Viewer::start(&host);
    test_host.output(b"bye\r\n");
    test_host.exit(5);
    viewer
        .until("the exit over the final screen", |viewer| {
            viewer.client.state.and_then(|state| state.exit) == Some(ExitState::Code(5))
                && row_text(&viewer.client.screen[0]) == "bye"
        })
        .await;
    assert!(!viewer.stream.is_finished());
}

/// A host that becomes unreadable ends the stream as failed.
///
/// Why: an unavailable host may still run, but its runtime can no longer
/// read it, so the viewer would watch a screen that silently stopped; it is
/// told instead. Spec: once the stand-in host turns unreadable, the stream
/// ends with a failure that says the host is unavailable.
#[farhelm_testtrace::test]
async fn an_unreadable_host_ends_the_stream() {
    let (host, test_host) = host_with(1000).await;
    let mut viewer = Viewer::start(&host);
    viewer
        .until("the attach caught up", |viewer| viewer.converged(0))
        .await;
    test_host.become_unreadable();
    let ended = tokio::time::timeout(WAIT, async {
        loop {
            let frame = tokio::select! {
                ended = &mut viewer.stream => break ended.expect("the stream task does not panic"),
                frame = viewer.rx.recv() => frame,
            };
            if let Some(frame) = frame {
                viewer.take(frame);
            }
        }
    })
    .await
    .expect("the stream ends");
    let ForwarderEnd::StreamFailed(reason) = ended else {
        panic!("the stream failed");
    };
    assert!(reason.contains("unavailable"), "{reason}");
}

// ---- Input -----------------------------------------------------------------------

/// Wait until the host has received exactly `expected`.
async fn received(test_host: &TestHost, expected: &[u8]) {
    assert!(
        test_host.received_becomes(expected, WAIT).await,
        "the host received {:?}, not {:?}",
        String::from_utf8_lossy(&test_host.received()),
        String::from_utf8_lossy(expected)
    );
}

/// A host with bracketed paste on, as a program sets it.
async fn bracketed_host() -> (HostTerminal, TestHost) {
    let (host, test_host) = host_with(1000).await;
    let mode = b"\x1b[?2004h";
    test_host.output(mode);
    processed(&host, mode.len() as u64).await;
    assert!(host.status().modes.bracketed_paste, "premise: mode 2004");
    (host, test_host)
}

/// A `paste` message.
fn paste(data: &str, last: bool) -> Vec<u8> {
    GridInput::Paste {
        data: data.to_owned(),
        last,
    }
    .encode()
    .expect("a paste message")
}

const KEY_A: &[u8] = br#"{"t":"key","key":"a","code":"KeyA","kc":65,"mods":0}"#;

/// Wait until `input`'s pump has stopped.
async fn pump_finished(input: &HostAttach, within: Duration) -> bool {
    tokio::time::timeout(within, async {
        while !input.pump_finished() {
            tokio::time::sleep(Duration::from_millis(10)).await; // sleep-ok: polling a task handle, which offers no wait that leaves it with its owner
        }
    })
    .await
    .is_ok()
}

/// Keys and text are encoded and written to the program, and a submitted
/// line is reported as one.
///
/// Why: a viewer's keystrokes are the program's input; the hook warning's
/// first-input mark needs to know which input submitted a line. Spec: the
/// key `a` writes `a` and submits nothing; the text `hi` and Enter writes
/// `hi\r` and submits a line.
#[farhelm_testtrace::test]
async fn keys_and_text_reach_the_program() {
    let (host, test_host) = host_with(1000).await;
    let (mut input, _failure) = HostAttach::new(host.clone());
    assert!(!input.send_input(KEY_A).expect("a key is taken"));
    let text = br#"{"t":"text","data":"hi"}"#;
    assert!(!input.send_input(text).expect("text is taken"));
    let enter = br#"{"t":"key","key":"Enter","code":"Enter","kc":13,"mods":0}"#;
    assert!(input.send_input(enter).expect("Enter is taken"));
    received(&test_host, b"ahi\r").await;
}

/// A paste to a program with bracketed paste on is wrapped in the markers
/// once around all its parts, with its line endings converted.
///
/// Why: paste is critical; the markers tell the program the text was
/// pasted rather than typed, and a paste split across messages must look
/// to the program like one paste. Spec: with mode 2004 set, the parts
/// `one\n` and `two` write `ESC [200~ one CR two ESC [201~`, and neither
/// part counts as submitting a line.
#[farhelm_testtrace::test]
async fn a_bracketed_paste_is_wrapped_once() {
    let (host, test_host) = bracketed_host().await;
    let (mut input, _failure) = HostAttach::new(host.clone());
    assert!(!input.send_input(&paste("one\n", false)).expect("a part"));
    assert!(
        !input
            .send_input(&paste("two", true))
            .expect("the last part")
    );
    received(&test_host, b"\x1b[200~one\rtwo\x1b[201~").await;
}

/// A paste reaches the program only once its last part has arrived, and a
/// paste whose last part never arrives writes nothing.
///
/// Why: written part by part, a paste could have other input land between
/// its parts, and one cut short would leave the program reading everything
/// after it as pasted text; collecting it whole avoids both (module docs,
/// "Input"). Spec: with mode 2004 set, the parts `one` and `two`, neither
/// last, are taken; the attachment then ends, and `after`, sent by another
/// caller, is all the program reads.
#[farhelm_testtrace::test]
async fn a_paste_whose_last_part_never_arrives_writes_nothing() {
    let (host, test_host) = bracketed_host().await;
    let (mut input, _failure) = HostAttach::new(host.clone());
    input.send_input(&paste("one", false)).expect("a part");
    input.send_input(&paste("two", false)).expect("a part");
    drop(input);
    host.send_input(b"after").await.expect("taken");
    received(&test_host, b"after").await;
}

/// Empty paste parts add nothing, and a paste of nothing but empty parts
/// to a program without bracketed paste writes nothing.
///
/// Why: the parts' bytes are joined as they arrive, and only the encoder
/// decides the markers; an empty part must add no bytes of its own, and a
/// paste with no text and no markers must not reach the runtime, which
/// refuses empty input. Spec: with mode 2004 off, the parts `` and ``
/// (last) write nothing, so `after` from another caller is the first thing
/// the program reads; with mode 2004 then on, the parts ``, `abc`, `` and
/// `` (last) write `ESC [200~ abc ESC [201~` after it.
#[farhelm_testtrace::test]
async fn empty_paste_parts_add_nothing() {
    let (host, test_host) = host_with(1000).await;
    let (mut input, _failure) = HostAttach::new(host.clone());
    input.send_input(&paste("", false)).expect("a part");
    input.send_input(&paste("", true)).expect("the last part");
    host.send_input(b"after").await.expect("taken");
    received(&test_host, b"after").await;

    let mode = b"\x1b[?2004h";
    test_host.output(mode);
    processed(&host, mode.len() as u64).await;
    for (data, last) in [("", false), ("abc", false), ("", false), ("", true)] {
        input.send_input(&paste(data, last)).expect("a part");
    }
    received(&test_host, b"after\x1b[200~abc\x1b[201~").await;
}

/// The terminal's answers to the program's queries, and other callers'
/// input, never land inside a paste: a query asked while the paste's parts
/// arrive is answered before the paste, and one asked while the paste is
/// being written is answered after its closing marker, with input another
/// caller sent meanwhile after that.
///
/// Why: text written inside a bracketed paste is read as pasted text: the
/// program would never get its answer, and a prompt with markers of its own
/// would end the paste early or be pasted. Spec: with mode 2004 set, after
/// the part `one` (not last) the program asks DA1 and reads its answer
/// alone; the host then holds its input, the rest of the paste arrives as
/// ten parts of 32 KiB of `x`, the last flagged, which the runtime writes
/// as six `Input`s; once the host holds three of them (the input window
/// full, the other three still waiting in the runtime, where an answer could
/// otherwise go between them), the program asks DA1 again and another
/// caller sends `PROMPT`; once the host releases its input, the program
/// has read the first answer, the whole paste, the second answer and
/// `PROMPT`, in that order.
#[farhelm_testtrace::test]
async fn answers_and_other_input_never_land_inside_a_paste() {
    let (host, test_host) = bracketed_host().await;
    let (mut input, _failure) = HostAttach::new(host.clone());
    input.send_input(&paste("one", false)).expect("a part");
    let query = b"\x1b[c";
    test_host.output(query);
    processed(&host, 8 + query.len() as u64).await;
    received(&test_host, DA1_ANSWER).await;

    test_host.hold_input().await;
    let part = "x".repeat(32 * 1024);
    for at in 0..10 {
        input.send_input(&paste(&part, at == 9)).expect("a part");
    }
    // Three `MAX_INPUT` pieces fill the input window; a fourth does not
    // fit (`hosts::client::INPUT_WINDOW`).
    assert!(
        test_host.holds(3, WAIT).await,
        "premise: the window is full of the paste, the rest of it waiting"
    );
    test_host.output(query);
    processed(&host, 8 + 2 * query.len() as u64).await;
    host.send_input(b"PROMPT").await.expect("taken");
    test_host.release_input();
    let expected = [
        DA1_ANSWER,
        b"\x1b[200~one",
        part.repeat(10).as_bytes(),
        b"\x1b[201~",
        DA1_ANSWER,
        b"PROMPT",
    ]
    .concat();
    received(&test_host, &expected).await;
}

/// A paste that a lost connection cuts short is closed first thing on the
/// new connection, ahead of the answers kept from before the loss, and the
/// attachment's input fails.
///
/// Why: the program read the paste's opening marker; without a closing one
/// it reads everything after as pasted text, the kept answers first. Spec:
/// with mode 2004 set and the host writing one `Input` and then holding the
/// rest, a paste of ten parts of 32 KiB of `p` is sent; once the program
/// has read the first `Input` of it and the host holds the next, the
/// program asks DA1 (answered behind the rest of the paste); the
/// connection drops and the host releases its input: the program has read
/// the paste's first `Input`, `ESC [201~` and the DA1 answer, in that
/// order, and the attachment's input has failed.
#[farhelm_testtrace::test]
async fn a_paste_cut_short_by_a_lost_connection_is_closed_first() {
    let (host, test_host) = bracketed_host().await;
    test_host.hold_input_after(1).await;
    let (mut input, mut failure) = HostAttach::new(host.clone());
    let part = "p".repeat(32 * 1024);
    for at in 0..10 {
        input.send_input(&paste(&part, at == 9)).expect("a part");
    }
    let encoded = [&b"\x1b[200~"[..], part.repeat(10).as_bytes(), b"\x1b[201~"].concat();
    let first = &encoded[..terminal_host::protocol::MAX_INPUT];
    received(&test_host, first).await;
    assert!(
        test_host.holds(1, WAIT).await,
        "premise: the host holds the paste's next piece"
    );
    let query = b"\x1b[c";
    test_host.output(query);
    processed(&host, 8 + query.len() as u64).await;
    test_host.drop_connection();
    test_host.release_input();
    let expected = [first, b"\x1b[201~", DA1_ANSWER].concat();
    received(&test_host, &expected).await;
    let failed = tokio::time::timeout(WAIT, failure.wait_for(Option::is_some)).await;
    assert!(
        failed.is_ok_and(|failed| failed.is_ok()),
        "the input failed"
    );
}

/// A takeover while the displaced attachment has a paste half collected
/// writes nothing of that paste, and attachments that end leave no pump
/// behind, even on a host that does not read.
///
/// Why: the takeover's exclusive-input boundary: the new viewer's keys must
/// not land inside, or after half of, the old viewer's paste, and repeated
/// takeovers must not pile up pumps waiting on a program. Spec: with mode
/// 2004 set, an attachment sends the part `old` (not last) and ends as a new
/// attachment types `a`; the program reads `a` alone, and the old pump has
/// finished. With the host holding its input, five attachments that each
/// hand it a key all have their pumps finished once they end.
#[farhelm_testtrace::test]
async fn a_takeover_writes_nothing_of_a_half_collected_paste() {
    let (host, test_host) = bracketed_host().await;
    let (mut old, _old_failure) = HostAttach::new(host.clone());
    old.send_input(&paste("old", false)).expect("a part");
    let (mut new, _new_failure) = HostAttach::new(host.clone());
    let old_pump = {
        drop(old.cancel.take());
        old
    };
    new.send_input(KEY_A).expect("a key");
    received(&test_host, b"a").await;
    assert!(pump_finished(&old_pump, WAIT).await, "the old pump stopped");

    test_host.hold_input().await;
    let mut ended = Vec::new();
    for _ in 0..5 {
        let (mut input, _failure) = HostAttach::new(host.clone());
        input.send_input(KEY_A).expect("a key");
        ended.push(input);
    }
    assert!(
        test_host.holds(5, WAIT).await,
        "premise: every key reached the holding host"
    );
    for input in &mut ended {
        drop(input.cancel.take());
    }
    for input in &ended {
        assert!(
            pump_finished(input, WAIT).await,
            "a pump outlived its attachment"
        );
    }
    drop(new);
}

/// A paste larger than the backlog fails the attachment's input, and
/// nothing of it is written.
///
/// Why: the attachment holds a paste until its last part, so the backlog
/// bounds how large a paste can be; past the bound it must give the paste
/// up whole, never write the part it had. Spec: parts of 32 KiB are taken
/// until the backlog refuses one, which happens within 300 parts; the
/// attachment ends, and `after` from another caller is all the program
/// reads.
#[farhelm_testtrace::test]
async fn a_paste_past_the_backlog_writes_nothing() {
    let (host, test_host) = bracketed_host().await;
    let (mut input, _failure) = HostAttach::new(host.clone());
    let part = "p".repeat(32 * 1024);
    let mut taken = 0;
    while input.send_input(&paste(&part, false)).is_ok() {
        taken += 1;
        assert!(taken < 300, "the backlog never refused a part");
    }
    drop(input);
    host.send_input(b"after").await.expect("taken");
    received(&test_host, b"after").await;
}

/// A malformed message, and input inside an unfinished paste, are refused.
///
/// Why: the format has the supervisor end an attachment on a message that
/// breaks it, and the caller ends it on this refusal; typing into the
/// middle of a paste would land inside the pasted text. Spec: `not json` is
/// refused; after a first paste part, a key is refused as interrupting the
/// paste; the attachment ends, and `after` from another caller is all the
/// program reads.
#[farhelm_testtrace::test]
async fn broken_input_is_refused() {
    let (host, test_host) = host_with(1000).await;
    let (mut input, _failure) = HostAttach::new(host.clone());
    assert!(input.send_input(b"not json").is_err());
    input.send_input(&paste("abc", false)).expect("a part");
    let refused = input.send_input(KEY_A).expect_err("a key inside a paste");
    assert!(
        format!("{refused:#}").contains("middle of a paste"),
        "{refused:#}"
    );
    drop(input);
    host.send_input(b"after").await.expect("taken");
    received(&test_host, b"after").await;
}

/// Input is taken while the emulator is held.
///
/// Why: the connection hands viewer input over under the supervisor-wide
/// attachments lock; input that waited on the emulator's lock (held for a
/// viewer's screen and history reads, or while a snapshot is taken) would
/// stall every session's input and attach. Spec: while another task holds
/// the emulator, a key is taken within a second and, once the emulator is
/// let go, written.
#[farhelm_testtrace::test]
async fn input_does_not_wait_on_the_emulator() {
    let (host, test_host) = host_with(1000).await;
    let (holding_tx, holding) = std::sync::mpsc::channel();
    let (release, released) = std::sync::mpsc::channel::<()>();
    let holder = {
        let host = host.clone();
        let runtime = tokio::runtime::Handle::current();
        tokio::task::spawn_blocking(move || {
            runtime.block_on(host.read(|_| {
                holding_tx.send(()).expect("the test waits");
                let _ = released.recv_timeout(WAIT);
            }))
        })
    };
    holding
        .recv_timeout(WAIT)
        .expect("premise: the emulator is held");
    let (mut input, _failure) = HostAttach::new(host.clone());
    // Timed on its own thread, which lets the emulator go once the input is
    // taken: a send that waited for the emulator would wait out the
    // holder's whole bound first. (This runtime's own thread may block on
    // the emulator meanwhile, in the terminal's task, so nothing on it can
    // time this.)
    let (input, taken, took) = tokio::task::spawn_blocking(move || {
        let started = std::time::Instant::now();
        let taken = input.send_input(KEY_A).is_ok();
        let took = started.elapsed();
        let _ = release.send(());
        (input, taken, took)
    })
    .await
    .expect("the input task");
    assert!(taken);
    assert!(
        took < Duration::from_secs(1),
        "the input waited {took:?} on the emulator"
    );
    holder
        .await
        .expect("the holder")
        .expect("the emulator was home");
    received(&test_host, b"a").await;
    drop(input);
}

/// The emulator's history and the test client's agree on what a held line
/// is, so that the comparisons above mean something.
///
/// Why: every convergence check compares the client's rows with rows read
/// from the emulator by the same code that encodes them; this pins the
/// read itself to the text that was printed. Spec: after 30 numbered lines
/// on a 24-row screen, the held history is `line 0000` to `line 0006`.
#[farhelm_testtrace::test]
async fn held_history_is_the_printed_text() {
    let (host, test_host) = host_with(1000).await;
    let output = numbered(0, 30);
    test_host.output(&output);
    processed(&host, output.len() as u64).await;
    let texts: Vec<String> = host
        .read(|emulator| held_history(emulator).iter().map(row_text).collect())
        .await
        .expect("the emulator");
    assert_eq!(texts, lines(0, 7));
}

// ---- The attach and resize cost at the scrollback cap -------------------------------

/// Output that fills the default scrollback (12,000 lines) and a 50-row
/// screen at 200 columns, of one kind: `typical` agent or build output (a
/// line number, a level, a path and a message in a few styles, about 140
/// of the 200 columns used), or every cell its own color, from a 256-color
/// `palette` or `truecolor`, the worst case for runs.
fn cap_output(kind: &str) -> Vec<u8> {
    let words: Vec<&str> =
        "the quick brown fox jumps over a lazy dog while compiling crates and running tests"
            .split(' ')
            .collect();
    let mut seed: u32 = 1;
    let mut random = move || {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345) & 0x7fff_ffff;
        seed
    };
    let mut output = Vec::new();
    for n in 0..12_050u32 {
        match kind {
            "typical" => {
                let text: String = (0..14)
                    .map(|_| words[random() as usize % words.len()])
                    .collect::<Vec<_>>()
                    .join(" ");
                let text = &text[..text.len().min(100)];
                let path = format!("src/module_{}.rs:{}", n % 97, n % 400);
                output.extend(
                    format!(
                        "\x1b[2;90m{n:08}\x1b[m \x1b[1;32mINFO\x1b[m  \x1b[3;36m{path:<20}\x1b[m {text}\r\n"
                    )
                    .into_bytes(),
                );
            }
            "palette" => {
                for _ in 0..200 {
                    let (fg, bg) = (16 + random() % 240, 16 + random() % 240);
                    output.extend(format!("\x1b[38;5;{fg};48;5;{bg}m▀").into_bytes());
                }
                output.extend(b"\x1b[m\r\n");
            }
            _ => {
                for _ in 0..200 {
                    let (fg, bg) = (random() & 0xff_ffff, random() & 0xff_ffff);
                    output.extend(
                        format!(
                            "\x1b[38;2;{};{};{};48;2;{};{};{}m▀",
                            fg >> 16,
                            (fg >> 8) & 0xff,
                            fg & 0xff,
                            bg >> 16,
                            (bg >> 8) & 0xff,
                            bg & 0xff
                        )
                        .into_bytes(),
                    );
                }
                output.extend(b"\x1b[m\r\n");
            }
        }
    }
    output
}

/// A link of 10 Mbit/s with 25 ms each way, simulated in this process: each
/// frame is taken from the writer queue only once the frames before it
/// have been serialized (real waits, so the real queue backpressures the
/// sender), and arrives one way's latency after its last bit left.
struct Link {
    started: tokio::time::Instant,
    free: tokio::time::Instant,
    bytes: usize,
}

impl Link {
    const BITS_PER_SECOND: f64 = 10_000_000.0;
    const ONE_WAY: Duration = Duration::from_millis(25);

    fn new() -> Link {
        let now = tokio::time::Instant::now();
        Link {
            started: now,
            free: now,
            bytes: 0,
        }
    }

    /// Carry `frame`; returns when it arrives, since the link started.
    async fn carry(&mut self, frame: &Frame) -> Duration {
        let size = 4 + frame.encoded_len();
        self.bytes += size;
        let sent = self.free.max(tokio::time::Instant::now())
            + Duration::from_secs_f64(size as f64 * 8.0 / Self::BITS_PER_SECOND);
        tokio::time::sleep_until(sent).await; // sleep-ok: the simulated link's serialization time, which is what backpressures the sender
        self.free = sent;
        sent + Self::ONE_WAY - self.started
    }
}

/// A stand-in host at 200x50 holding the full scrollback of `kind`, and the
/// output that made it.
async fn host_at_the_cap(kind: &str) -> (HostTerminal, TestHost, Vec<u8>) {
    let (host, test_host) = host_with(crate::hosts::terminal::DEFAULT_SCROLLBACK_LINES).await;
    let size = Size {
        cols: 200,
        rows: 50,
    };
    host.resize(size).expect("a valid size");
    sized(&host, size).await;
    let output = cap_output(kind);
    // Fed in steps, each processed before the next: the stand-in has none
    // of a real host's bounds on what it sends a slow client, and the
    // runtime stops reading while it encodes a checkpoint (seconds for a
    // true-color scrollback), so an unpaced flood would overrun the
    // connection's event queue.
    for step in output.chunks(256 * 1024) {
        let end = host.status().position.offset + step.len() as u64;
        test_host.output(step);
        let mut status = host.subscribe();
        tokio::time::timeout(Duration::from_secs(600), async {
            while host.status().position.offset < end {
                status.changed().await.expect("the runtime runs");
            }
        })
        .await
        .expect("the output is processed");
    }
    (host, test_host, output)
}

/// Take every frame the stream has queued, through `link` when there is
/// one; then, if `done` does not hold, wait for the next frame and repeat.
/// `done` is checked only between bursts, since it reads the whole history.
/// Returns when the last frame arrived over the link.
async fn drain_until(
    viewer: &mut Viewer,
    mut link: Option<&mut Link>,
    done: impl Fn(&Viewer) -> bool,
) -> Duration {
    let mut arrived = Duration::ZERO;
    loop {
        while let Ok(frame) = viewer.rx.try_recv() {
            if let Some(link) = link.as_deref_mut() {
                arrived = link.carry(&frame).await;
            }
            viewer.take(frame);
        }
        if done(viewer) {
            return arrived;
        }
        let frame = viewer.rx.recv().await.expect("the stream runs");
        if let Some(link) = link.as_deref_mut() {
            arrived = link.carry(&frame).await;
        }
        viewer.take(frame);
    }
}

/// Measure what an attach to a terminal at the full scrollback of `kind`
/// costs: the bytes of its grid stream up to `ReplayComplete`, and when the
/// screen and the end of the history arrive over a simulated 10 Mbit/s link
/// with 25 ms each way ([`Link`]). Prints one JSON line; with
/// `FARHELM_GRID_MEASURE_OUT` naming a directory, also writes the data
/// messages there, one per line (`<kind>.jsonl`), and the output that made
/// them (`<kind>.raw`), for measuring the grid client's memory and today's
/// tmux replay of the same output.
async fn measure_attach(kind: &str) {
    let out_dir = std::env::var_os("FARHELM_GRID_MEASURE_OUT").map(std::path::PathBuf::from);
    let (host, _test_host, output) = host_at_the_cap(kind).await;
    let held = host
        .read(|emulator| held_history(emulator).len())
        .await
        .expect("the emulator");
    let (tx, mut rx) = mpsc::channel(super::super::connection::CONNECTION_WRITER_QUEUE);
    let (_pause, pause_rx) = watch::channel(None);
    let stream = GridStream::new(GridStreamParts {
        channel: 1,
        tx,
        host: host.clone(),
        notices: None,
        pause_rx,
        stall_timeout: NO_STALL,
        attach_size: None,
        input_failure: None,
        seams: StreamSeams::default(),
    });
    let task = tokio::spawn(stream.run());
    let mut link = Link::new();
    let mut messages = 0usize;
    let mut screen_at = None;
    let mut lines = Vec::new();
    let replay_complete_at = loop {
        let frame = rx.recv().await.expect("the stream runs");
        let arrived = link.carry(&frame).await;
        match frame.kind {
            FrameKind::Data => {
                messages += 1;
                if screen_at.is_none() && frame.body.starts_with(br#"{"t":"frame""#) {
                    screen_at = Some(arrived);
                }
                if out_dir.is_some() {
                    lines.extend_from_slice(&frame.body);
                    lines.push(b'\n');
                }
            }
            FrameKind::Control => break arrived,
        }
    };
    task.abort();
    if let Some(dir) = &out_dir {
        std::fs::write(dir.join(format!("{kind}.jsonl")), &lines).expect("write messages");
        std::fs::write(dir.join(format!("{kind}.raw")), &output).expect("write output");
    }
    println!(
        "{}",
        serde_json::json!({
            "measure": "attach",
            "kind": kind,
            "history_lines": held,
            "program_output_bytes": output.len(),
            "stream_bytes": link.bytes,
            "messages": messages,
            "screen_arrived_ms": screen_at.map(|at| at.as_millis()),
            "replay_complete_arrived_ms": replay_complete_at.as_millis(),
        })
    );
}

/// Measure what a resize drag costs a viewer of a terminal at the full
/// scrollback of `kind`: after the attach has caught up, ten resizes in a
/// row (each taken by the terminal before the next), and then everything
/// the stream sends until the viewer shows the emulator again at the last
/// size: its bytes, how many history resets it took, and when the last of
/// it arrives over the simulated link ([`Link`]). Prints one JSON line.
async fn measure_resize(kind: &str) {
    let (host, _test_host, output) = host_at_the_cap(kind).await;
    let end = output.len() as u64;
    let mut viewer = Viewer::start(&host);
    drain_until(&mut viewer, None, |viewer| viewer.converged(end)).await;
    let resets = viewer.client.resets;
    let mut link = Link::new();
    for step in 0..10u16 {
        let size = Size {
            cols: 190 - step,
            rows: 50,
        };
        host.resize(size).expect("a valid size");
        sized(&host, size).await;
        while let Ok(frame) = viewer.rx.try_recv() {
            link.carry(&frame).await;
            viewer.take(frame);
        }
    }
    let arrived = drain_until(&mut viewer, Some(&mut link), |viewer| {
        viewer.client.cols == 181
            && viewer
                .client
                .history
                .iter()
                .all(|(_, cols, _)| *cols == 181)
            && viewer.converged(end)
    })
    .await;
    println!(
        "{}",
        serde_json::json!({
            "measure": "resize drag",
            "kind": kind,
            "resizes": 10,
            "history_resets": viewer.client.resets - resets,
            "stream_bytes": link.bytes,
            "last_arrived_ms": arrived.as_millis(),
        })
    );
}

/// The measurements, one test per kind of output so that each stays within
/// nextest's time limit.
///
/// Why: the goal's scrollback-delivery proposal sends the whole history
/// once at attach and keeps paging on demand for if that proves too
/// costly, and a resize resends it; these are the measurements that decide
/// it, kept so they can be repeated when the encoding, the cap or the
/// debounce changes. Ignored: each takes a minute or more, in release, and
/// asserts nothing; their numbers are read against the threshold the
/// deciding change stated.
#[farhelm_testtrace::test]
#[ignore = "a measurement, run by hand in release with --no-capture"]
async fn measure_the_attach_cost_at_the_scrollback_cap_typical() {
    measure_attach("typical").await;
}

/// See [`measure_the_attach_cost_at_the_scrollback_cap_typical`].
#[farhelm_testtrace::test]
#[ignore = "a measurement, run by hand in release with --no-capture"]
async fn measure_the_attach_cost_at_the_scrollback_cap_palette() {
    measure_attach("palette").await;
}

/// See [`measure_the_attach_cost_at_the_scrollback_cap_typical`].
#[farhelm_testtrace::test]
#[ignore = "a measurement, run by hand in release with --no-capture"]
async fn measure_the_attach_cost_at_the_scrollback_cap_truecolor() {
    measure_attach("truecolor").await;
}

/// See [`measure_the_attach_cost_at_the_scrollback_cap_typical`].
#[farhelm_testtrace::test]
#[ignore = "a measurement, run by hand in release with --no-capture"]
async fn measure_the_resize_cost_at_the_scrollback_cap_typical() {
    measure_resize("typical").await;
}

/// See [`measure_the_attach_cost_at_the_scrollback_cap_typical`].
#[farhelm_testtrace::test]
#[ignore = "a measurement, run by hand in release with --no-capture"]
async fn measure_the_resize_cost_at_the_scrollback_cap_palette() {
    measure_resize("palette").await;
}

/// See [`measure_the_attach_cost_at_the_scrollback_cap_typical`].
#[farhelm_testtrace::test]
#[ignore = "a measurement, run by hand in release with --no-capture"]
async fn measure_the_resize_cost_at_the_scrollback_cap_truecolor() {
    measure_resize("truecolor").await;
}
