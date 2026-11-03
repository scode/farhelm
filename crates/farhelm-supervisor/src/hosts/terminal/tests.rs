//! The runtime against stand-in hosts: listeners in this test process that
//! play exactly the records and acknowledgements each case needs, which a
//! real host gives only under circumstances a test cannot arrange on cue (a
//! record straddling the replay's end, a stream resumed after a dropped
//! connection, an acknowledgement that discards, a window that never opens,
//! a snapshot record that does not restore). Real hosts, started from the
//! built `farhelm`, are in `crates/farhelm/tests/terminal_host_runtime.rs`.
//!
//! The stand-in's `Info` names this test process as the host's child, so the
//! runtime pins a real, live process; nothing here signals it.
//!
//! Sections: the stand-in and waits; adoption and replay; the clock; the
//! child's liveness and the adapters; input; connections and shutdown.

use super::*;
use farhelm_terminal_emulator::Collected;
use std::future::Future;
use std::pin::Pin;
use std::task::Poll;
use terminal_host::protocol::{
    ClientMessage, HostMessage, INPUT_COST, MAX_FRAME, decode_client, encode_host, split_frame,
};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{UnixListener, UnixStream};

use super::super::client::INPUT_WINDOW;

/// How long any one wait on a stand-in or the runtime may take.
const WAIT: Duration = Duration::from_secs(10);

/// What the emulator answers to DA1 (`CSI c`).
const DA1_ANSWER: &[u8] = b"\x1b[?1;2c";

/// What it answers to a status report request (`CSI 5 n`).
const STATUS_ANSWER: &[u8] = b"\x1b[0n";

// ---- The stand-in and waits ------------------------------------------------------

/// `future`, failing the test with `what` if it takes longer than [`WAIT`].
async fn within<T>(what: &str, future: impl Future<Output = T>) -> T {
    tokio::time::timeout(WAIT, future)
        .await
        .unwrap_or_else(|_| panic!("{what} did not finish in time"))
}

/// Poll `future` once, saying whether it is still pending: proof that a
/// send waits for room, without a delay to wait it out. The test task's
/// waker is registered, so awaiting the future later goes on from there.
async fn pending_once<F: Future + Unpin>(future: &mut F) -> bool {
    std::future::poll_fn(|cx| Poll::Ready(Pin::new(&mut *future).poll(cx).is_pending())).await
}

/// A listener at a fresh socket path.
struct StandIn {
    dir: tempfile::TempDir,
    listener: UnixListener,
}

impl StandIn {
    fn new() -> StandIn {
        let dir = tempfile::tempdir().unwrap();
        let listener = UnixListener::bind(dir.path().join("h")).unwrap();
        StandIn { dir, listener }
    }

    fn socket(&self) -> PathBuf {
        self.dir.path().join("h")
    }

    /// Accept one connection and answer its `Hello`, and nothing more.
    async fn accept_hello(&self) -> Server {
        let (stream, _) = within("a connection", self.listener.accept())
            .await
            .unwrap();
        let mut server = Server {
            stream,
            buf: Vec::new(),
        };
        assert_eq!(
            server.next().await,
            ClientMessage::Hello {
                version: 1,
                take_over: true
            }
        );
        server.send(&HostMessage::Welcome { version: 1 }).await;
        server
    }

    /// Accept one connection and answer what a runtime asks on it before
    /// reading: `Hello`, then `Info` (as `info`) as often as it asks, until
    /// its `Read`, whose position is returned. This is all a reconnect asks;
    /// an adoption then makes its two fence `Info` round trips
    /// ([`adopt_with`]).
    async fn accept(&self, info: &Info) -> (Server, Position) {
        let mut server = self.accept_hello().await;
        loop {
            match server.next().await {
                ClientMessage::Info => server.send(&HostMessage::InfoReply(info.clone())).await,
                ClientMessage::Read(from) => return (server, from),
                other => panic!("expected Info or Read, got {other:?}"),
            }
        }
    }
}

/// The stand-in's side of one connection.
struct Server {
    stream: UnixStream,
    /// Bytes read past the last frame decoded.
    buf: Vec<u8>,
}

impl Server {
    /// The client's next frame. Cancel-safe: what was read stays in `buf`.
    async fn next(&mut self) -> ClientMessage {
        within("a frame from the client", async {
            loop {
                if let Some(frame) = split_frame(&self.buf, MAX_FRAME).expect("a well-formed frame")
                {
                    let consumed = frame.consumed;
                    let message = decode_client(frame, 1).expect("a well-formed message");
                    self.buf.drain(..consumed);
                    return message;
                }
                assert_ne!(
                    self.stream.read_buf(&mut self.buf).await.unwrap(),
                    0,
                    "the client closed the connection"
                );
            }
        })
        .await
    }

    async fn send(&mut self, message: &HostMessage) {
        within(
            "a write to the client",
            self.stream.write_all(&encode_host(message)),
        )
        .await
        .unwrap();
    }

    /// Send output at `offset`.
    async fn output(&mut self, offset: u64, data: &[u8]) {
        self.send(&HostMessage::Output {
            offset,
            data: data.to_vec(),
        })
        .await;
    }

    /// The data of the client's next frame, which must be an `Input`.
    async fn input(&mut self) -> Vec<u8> {
        match self.next().await {
            ClientMessage::Input(data) => data,
            other => panic!("expected Input, got {other:?}"),
        }
    }

    /// Acknowledge `written` bytes written in all, none discarded.
    async fn ack(&mut self, written: u64) {
        self.send(&HostMessage::InputAck {
            written,
            discarded: 0,
        })
        .await;
    }
}

/// `Info` for a host whose child is this test process, still running, at
/// 80x24, with `end` bytes of output journaled and nothing trimmed or sent.
fn info(end: u64) -> Info {
    let size = Size { cols: 80, rows: 24 };
    Info {
        host_pid: std::process::id(),
        child_pid: std::process::id(),
        child_exit: None,
        size,
        initial_size: size,
        snapshot: Position::default(),
        tail_start: 0,
        end,
        resizes: 0,
        gap: false,
        delivered_end: 0,
        tag: Vec::new(),
    }
}

/// Short connection deadlines, so that a reconnect to nothing is judged in
/// about a second.
fn options() -> HostTerminalOptions {
    HostTerminalOptions {
        deadlines: ClientDeadlines {
            connect: Duration::from_secs(1),
            refusal_window: Duration::from_secs(1),
            retry_interval: Duration::from_millis(50),
            reply: Duration::from_secs(5),
            shutdown: Duration::from_secs(5),
        },
        ..HostTerminalOptions::default()
    }
}

/// Adopt the stand-in's host, answering the adoption with `info`, and send
/// `records` as the stream's first records in the order a version-1 host
/// may: the reply to the first `Info` after `Read` ahead of them (a host
/// that acts on `Read` and `Info` in one loop iteration queues the reply
/// before it moves any record to its outgoing queue), and the reply to the
/// second, which the adoption sends only once the first is answered,
/// behind them.
async fn adopt_with(
    stand_in: &StandIn,
    adoption: Adoption,
    info: &Info,
    options: HostTerminalOptions,
    records: &[HostMessage],
) -> (Result<HostTerminal, AdoptError>, Server, Position) {
    let socket = stand_in.socket();
    let host = async {
        let (mut server, from) = stand_in.accept(info).await;
        assert_eq!(server.next().await, ClientMessage::Info, "the first fence");
        server.send(&HostMessage::InfoReply(info.clone())).await;
        for record in records {
            server.send(record).await;
        }
        assert_eq!(server.next().await, ClientMessage::Info, "the fence");
        server.send(&HostMessage::InfoReply(info.clone())).await;
        (server, from)
    };
    let (terminal, (server, from)) = within("the adoption", async {
        tokio::join!(HostTerminal::adopt(&socket, adoption, options), host)
    })
    .await;
    (terminal, server, from)
}

/// [`adopt_with`] with no first records, for an adoption that succeeds.
async fn adopt(
    stand_in: &StandIn,
    adoption: Adoption,
    info: &Info,
    options: HostTerminalOptions,
) -> (HostTerminal, Server, Position) {
    let (terminal, server, from) = adopt_with(stand_in, adoption, info, options, &[]).await;
    (terminal.expect("the adoption succeeds"), server, from)
}

/// Wait until `condition` holds, checking again on every change of the
/// terminal's status (which every change of its emulator makes).
async fn wait_until(
    terminal: &HostTerminal,
    what: &str,
    condition: impl Fn(&HostTerminal) -> bool,
) {
    let mut status = terminal.subscribe();
    let waited = tokio::time::timeout(WAIT, async {
        while !condition(terminal) {
            if status.changed().await.is_err() {
                // The runtime ended; nothing will change any more.
                return condition(terminal);
            }
        }
        true
    })
    .await;
    assert!(
        waited == Ok(true),
        "{what} did not happen; status {:?}, screen {:?}",
        terminal.status(),
        terminal.screen_tail(4096)
    );
}

fn screen_has(text: &'static str) -> impl Fn(&HostTerminal) -> bool {
    move |terminal| terminal.screen_tail(4096).contains(text)
}

/// Send `data` as input, failing the test if the runtime does not take it
/// in time.
async fn send(terminal: &HostTerminal, data: &[u8]) -> InputReceipt {
    within("queueing input", terminal.send_input(data))
        .await
        .expect("the runtime takes the input")
}

/// The fate of `receipt`, failing the test if it is not known in time.
async fn fate(receipt: InputReceipt) -> InputFate {
    within("an input's fate", receipt.fate()).await
}

/// A snapshot record at (0, 0) of an emulator that printed `text`.
fn seed_record(text: &[u8]) -> HostMessage {
    let mut seed = Emulator::new(
        farhelm_terminal_emulator::Size {
            columns: 80,
            lines: 24,
        },
        100,
    );
    let mut ignored = Collected::default();
    seed.feed(text, Instant::now(), FeedMode::Live, &mut ignored);
    HostMessage::SnapshotRecord {
        position: Position::default(),
        blob: seed.snapshot(1 << 20).expect("a clean boundary").blob,
    }
}

// ---- Adoption and replay ---------------------------------------------------------

/// A recovered adoption feeds what the host journaled before it without
/// answering it, and answers what follows, splitting a read that straddles
/// the two.
///
/// Why: an earlier supervisor received the journaled output and answered its
/// queries; answering them again would type the answers into the program
/// (a shell shows them as garbage at its prompt). Spec: with `Info` saying 4
/// bytes were journaled, one `Output` at 0 holding DA1, `x` and DA1 again
/// (7 bytes) draws `x` and draws one answer, to the second DA1 only; a
/// status request that follows is answered next, with nothing between; the
/// runtime reads from (0, 0) and is caught up once past the 4 bytes.
#[farhelm_testtrace::test]
async fn a_recovered_adoption_answers_only_what_follows_it() {
    let stand_in = StandIn::new();
    let (terminal, mut server, from) =
        adopt(&stand_in, Adoption::Recovered, &info(4), options()).await;
    assert_eq!(from, Position::default());
    assert!(!terminal.status().caught_up, "premise: 4 bytes to replay");

    server.output(0, b"\x1b[cx\x1b[c").await;
    assert_eq!(server.input().await, DA1_ANSWER);
    server.output(7, b"\x1b[5n").await;
    assert_eq!(server.input().await, STATUS_ANSWER);
    wait_until(&terminal, "the replay", screen_has("x")).await;
    assert!(terminal.status().caught_up);
}

/// A fresh adoption answers everything the host journaled, including what
/// the program wrote before the runtime connected.
///
/// Why: a program's first query (an agent's startup probe) usually comes
/// before the supervisor has connected to the host it just spawned; nobody
/// answered it, and the program may wait for it. Spec: with the same 4
/// journaled bytes and the same `Output` as the recovered case, both DA1s
/// are answered, together in one `Input`.
#[farhelm_testtrace::test]
async fn a_fresh_adoption_answers_what_was_journaled_before_it() {
    let stand_in = StandIn::new();
    let (_terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info(4), options()).await;
    server.output(0, b"\x1b[cx\x1b[c").await;
    assert_eq!(server.input().await, [DA1_ANSWER, DA1_ANSWER].concat());
}

/// A fresh adoption of a host that has already sent output to a client
/// treats that output as replay.
///
/// Why: whether an adoption is fresh is the caller's claim, and a retried
/// create can adopt a host an earlier supervisor already read; trusting the
/// claim would answer that supervisor's queries a second time. The host's
/// `delivered_end` says how far some client was sent output. Spec: with
/// `delivered_end` at 4, the same `Output` as the recovered case gets one
/// answer, to the second DA1 only.
#[farhelm_testtrace::test]
async fn a_fresh_adoption_replays_what_a_client_was_sent() {
    let stand_in = StandIn::new();
    let read_before = Info {
        delivered_end: 4,
        ..info(4)
    };
    let (_terminal, mut server, _) =
        adopt(&stand_in, Adoption::Fresh, &read_before, options()).await;
    server.output(0, b"\x1b[cx\x1b[c").await;
    assert_eq!(server.input().await, DA1_ANSWER);
    server.output(7, b"\x1b[5n").await;
    assert_eq!(server.input().await, STATUS_ANSWER);
}

/// A replay that ends inside an open synchronized update draws it with
/// replay's effects, so the live output that ends it answers nothing from
/// before the adoption.
///
/// Why: vte holds an update's bytes and interprets them when it ends, with
/// the effects of whichever feed ends it; an update opened in the replayed
/// output and ended by the first live feed would answer its replayed
/// queries live. Spec: with 11 bytes journaled (an update start and DA1),
/// then live output ending the update and asking for status, the first
/// answer the program gets is the status answer.
#[farhelm_testtrace::test]
async fn a_replay_ending_inside_an_update_answers_nothing_of_it() {
    let stand_in = StandIn::new();
    let (_terminal, mut server, _) =
        adopt(&stand_in, Adoption::Recovered, &info(11), options()).await;
    server.output(0, b"\x1b[?2026h\x1b[c").await;
    server.output(11, b"\x1b[?2026l\x1b[5n").await;
    assert_eq!(server.input().await, STATUS_ANSWER);
}

/// An initial snapshot record is restored before adoption returns, and a
/// stream resumed after a dropped connection does not restore it again.
///
/// Why: a host seeded at spawn (Restart's new host) sends its seed as a
/// snapshot record at (0, 0), the position of an empty start, so the first
/// record must be restored though it is not after the emulator's position;
/// and a host resuming a stream may send the record again, and restoring it
/// then would throw away what was fed since. Spec: the record holding
/// `seeded` is on the screen when adoption returns; ` live` then shows
/// `seeded live`; on the connection after a dropped one, which reads from
/// (5, 0), the record again and then `!` at 5 show `seeded live!` at (6, 0)
/// (restoring again would show `seeded!`).
#[farhelm_testtrace::test]
async fn a_snapshot_record_is_restored_once() {
    let record = seed_record(b"seeded");
    let stand_in = StandIn::new();
    let info = info(0);
    let (terminal, mut server, _) = adopt_with(
        &stand_in,
        Adoption::Fresh,
        &info,
        options(),
        std::slice::from_ref(&record),
    )
    .await;
    let terminal = terminal.expect("the adoption succeeds");
    assert!(terminal.screen_tail(4096).contains("seeded"));
    server.output(0, b" live").await;
    wait_until(
        &terminal,
        "the seed and the output",
        screen_has("seeded live"),
    )
    .await;

    drop(server);
    let (mut server, from) = stand_in.accept(&info).await;
    assert_eq!(
        from,
        Position {
            offset: 5,
            resizes: 0
        }
    );
    server.send(&record).await;
    server.output(5, b"!").await;
    wait_until(&terminal, "the resumed output", |terminal| {
        terminal.status().position.offset == 6
    })
    .await;
    assert!(
        terminal.screen_tail(4096).contains("seeded live!"),
        "{:?}",
        terminal.screen_tail(4096)
    );
    assert_eq!(
        terminal.status().position,
        Position {
            offset: 6,
            resizes: 0
        }
    );
}

/// A host whose only state is an initial snapshot is caught up only with the
/// snapshot restored.
///
/// Why: caught up is what a caller waits on before it reads the screen,
/// modes or a Restart seed; a host with nothing journaled after its seed
/// has an end of (0, 0), the same as an empty host, and must not count as
/// caught up while the seed is still unread. Spec: a host with the seed
/// record and no output is caught up when adoption returns, and its screen
/// then shows the seed.
#[farhelm_testtrace::test]
async fn a_seeded_host_is_caught_up_only_with_its_seed() {
    let stand_in = StandIn::new();
    let (terminal, _server, _) = adopt_with(
        &stand_in,
        Adoption::Fresh,
        &info(0),
        options(),
        &[seed_record(b"only the seed")],
    )
    .await;
    let terminal = terminal.expect("the adoption succeeds");
    assert!(terminal.status().caught_up);
    assert!(terminal.screen_tail(4096).contains("only the seed"));
}

/// A snapshot record that does not restore makes the host unavailable,
/// naming its process: at adoption as an error, and later as the
/// terminal's liveness.
///
/// Why: a blob of a format this build does not know (a newer supervisor's,
/// after a downgrade) must never be guessed at, and the host must not be
/// treated as gone: a newer supervisor may come back for it. Spec: a first
/// record whose blob is not a snapshot fails the adoption with
/// `Snapshot(NotASnapshot)` and the stand-in (this process) as the peer; on
/// an adopted terminal, a later record past its position that does not
/// restore gives `Unavailable` with the same reason and peer, a probe that
/// is an error, and unattributed evidence.
#[farhelm_testtrace::test]
async fn a_snapshot_that_does_not_restore_makes_the_host_unavailable() {
    let broken = |offset| HostMessage::SnapshotRecord {
        position: Position { offset, resizes: 0 },
        blob: b"not a snapshot".to_vec(),
    };
    let stand_in = StandIn::new();
    let (refused, _server, _) = adopt_with(
        &stand_in,
        Adoption::Fresh,
        &info(0),
        options(),
        &[broken(0)],
    )
    .await;
    match refused {
        Err(AdoptError::Unavailable {
            reason: Unavailability::Snapshot(SnapshotError::NotASnapshot),
            peer: Some(peer),
        }) => assert_eq!(peer.pid, std::process::id()),
        other => panic!("expected the adoption refused for its snapshot, got {other:?}"),
    }

    let stand_in = StandIn::new();
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info(0), options()).await;
    server.send(&broken(10)).await;
    wait_until(&terminal, "unavailability", |terminal| {
        matches!(terminal.liveness(), Liveness::Unavailable { .. })
    })
    .await;
    match terminal.liveness() {
        Liveness::Unavailable {
            reason: Unavailability::Snapshot(SnapshotError::NotASnapshot),
            peer: Some(peer),
        } => assert_eq!(peer.pid, std::process::id()),
        other => panic!("expected an unavailable snapshot naming the peer, got {other:?}"),
    }
    assert!(terminal.probe().is_err());
    assert_eq!(terminal.evidence(), HostEvidence::Unattributed);
}

/// A gap in the journal keeps the emulator and continues after it.
///
/// Why: a gap means the bytes in it were never interpreted, not that the
/// terminal is broken; resetting a healthy screen would lose everything on
/// it. Spec: `abc`, a gap from 3 to 10, and `def` at 10 show `abcdef` at
/// (13, 0).
#[farhelm_testtrace::test]
async fn a_gap_keeps_the_emulator() {
    let stand_in = StandIn::new();
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info(0), options()).await;
    server.output(0, b"abc").await;
    server
        .send(&HostMessage::Gap {
            from: 3,
            to: 10,
            resizes: 0,
            size: Size { cols: 80, rows: 24 },
        })
        .await;
    server.output(10, b"def").await;
    wait_until(&terminal, "the output after the gap", screen_has("abcdef")).await;
    assert_eq!(
        terminal.status().position,
        Position {
            offset: 13,
            resizes: 0
        }
    );
}

// ---- The clock -------------------------------------------------------------------

/// A synchronized update the program never ends is drawn once the runtime's
/// clock passes its deadline, with no more output.
///
/// Why: vte only records the 150 ms safety deadline; nothing wakes it, so a
/// program that opens an update and goes silent would leave its frame
/// undrawn for good. The clock is the runtime's so a test steps past the
/// deadline instead of waiting it out. Spec: with a manual clock, an update
/// start and `hidden` (14 bytes) are processed and not drawn; advancing the
/// clock by 150 ms draws `hidden`.
#[farhelm_testtrace::test]
async fn an_update_left_open_is_drawn_at_its_deadline() {
    let clock = ManualClock::new();
    let options = HostTerminalOptions {
        clock: Clock::Manual(clock.clone()),
        ..options()
    };
    let stand_in = StandIn::new();
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info(0), options).await;
    server.output(0, b"\x1b[?2026hhidden").await;
    wait_until(&terminal, "the update's bytes", |terminal| {
        terminal.status().position.offset == 14
    })
    .await;
    assert!(
        !terminal.screen_tail(4096).contains("hidden"),
        "premise: the open update is held"
    );
    clock.advance(Duration::from_millis(150));
    wait_until(&terminal, "the flush", screen_has("hidden")).await;
}

// ---- The child's liveness and the adapters ---------------------------------------

/// The child's exit ends its root, keeps its anchor, and is announced, and
/// nothing brings the root back.
///
/// Why: Stop and Delete kill from the child's pid only while the host
/// reports it alive, since a reaped pid may name anything by now; a hook
/// report written before the exit must still be attributable; and the tab
/// reap waits on exits rather than polling. Spec: adopted, the terminal is
/// running with this process as the pinned child; after an `Exited` record
/// with status 3 it is exited, its root is gone, the probe says dead with
/// no pid, the evidence says dead with code 3, the anchor still names the
/// child with its start identity, and the notifier received the exit. On a
/// later connection whose `Info` says the child runs (no host does), the
/// root stays gone and the terminal exited.
#[farhelm_testtrace::test]
async fn the_childs_exit_ends_its_root_and_is_announced() {
    let (notifier, mut changes) = LivenessNotifier::channel();
    let options = HostTerminalOptions {
        notifier: Some(notifier),
        ..options()
    };
    let stand_in = StandIn::new();
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info(0), options).await;
    let own = ProcessIdentity::read(std::process::id()).expect("this process");
    let root = ProcessRoot {
        pid: own.pid,
        start: own.start,
    };
    assert_eq!(terminal.liveness(), Liveness::Running { child: Some(root) });
    assert_eq!(terminal.child_root(), Some(root));

    server
        .send(&HostMessage::Exited {
            offset: 0,
            exit: ChildExit::Code(3),
        })
        .await;
    wait_until(&terminal, "the exit", |terminal| {
        terminal.child_root().is_none()
    })
    .await;
    assert_eq!(
        terminal.liveness(),
        Liveness::Exited {
            exit: ChildExit::Code(3)
        }
    );
    match terminal.probe().unwrap() {
        PaneProbe::Owned(process) => assert_eq!(
            process,
            PaneProcess {
                pid: None,
                dead: true
            }
        ),
        other => panic!("expected an owned dead pane, got {other:?}"),
    }
    assert_eq!(
        terminal.evidence(),
        HostEvidence::Owned {
            dead: true,
            exit_code: Some(3)
        }
    );
    assert_eq!(terminal.exit_code(), Some(3));
    assert_eq!(
        terminal.anchor(),
        Some(PaneAnchor {
            pid: own.pid,
            start: Some(own.start)
        })
    );
    let change = within("the exit's announcement", changes.recv())
        .await
        .expect("the notifier is open");
    assert_eq!(change.socket, stand_in.socket());
    assert_eq!(change.liveness, terminal.liveness());

    // A new connection whose host claims the child runs again (no host
    // does) does not bring the root back.
    drop(server);
    let (mut server, _) = stand_in.accept(&info(0)).await;
    server.output(0, b"after").await;
    wait_until(&terminal, "the resumed stream", screen_has("after")).await;
    assert_eq!(terminal.child_root(), None);
    assert_eq!(
        terminal.liveness(),
        Liveness::Exited {
            exit: ChildExit::Code(3)
        }
    );
}

/// An exit the host's `Info` reports before its record withdraws the root
/// at once, and is announced only with the record, after the output it
/// follows.
///
/// Why: the host reaps its child before it has drained the child's last
/// output and journaled the exit record after it; whoever acts on an exit
/// (the tab reap taking the last words, the never-started excerpt) reads
/// the screen, and must find it final. Spec: adopted with `Info` saying the
/// child exited with 3, the terminal is running without a root, its probe
/// says dead with no pid, its evidence says alive, and nothing is
/// announced; after `last words` and the exit record, it is exited with 3,
/// announced, and `last words` is on the screen.
#[farhelm_testtrace::test]
async fn an_exit_from_info_is_announced_only_with_its_record() {
    let (notifier, mut changes) = LivenessNotifier::channel();
    let options = HostTerminalOptions {
        notifier: Some(notifier),
        ..options()
    };
    let reaped = Info {
        child_exit: Some(ChildExit::Code(3)),
        ..info(0)
    };
    let stand_in = StandIn::new();
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &reaped, options).await;
    assert_eq!(terminal.liveness(), Liveness::Running { child: None });
    assert_eq!(terminal.child_root(), None);
    assert_eq!(
        terminal.probe().unwrap(),
        PaneProbe::Owned(PaneProcess {
            pid: None,
            dead: true
        })
    );
    assert_eq!(
        terminal.evidence(),
        HostEvidence::Owned {
            dead: false,
            exit_code: None
        }
    );
    assert!(changes.try_recv().is_err(), "an exit announced too early");

    server.output(0, b"last words").await;
    server
        .send(&HostMessage::Exited {
            offset: 10,
            exit: ChildExit::Code(3),
        })
        .await;
    let change = within("the exit's announcement", changes.recv())
        .await
        .expect("the notifier is open");
    assert_eq!(
        change.liveness,
        Liveness::Exited {
            exit: ChildExit::Code(3)
        }
    );
    assert!(terminal.screen_tail(4096).contains("last words"));
}

/// A reconnect whose `Info` says the child was reaped withdraws the root at
/// once and leaves the exit to its record.
///
/// Why: the same as for adoption: the host reaps before it has journaled the
/// child's last output and the exit record after it, and a reconnect often
/// lands in between; announcing the exit then would hand whoever acts on it
/// a screen without the program's last words. Spec: a running terminal
/// whose connection drops, with the new connection's `Info` saying the child
/// exited with 3, is running without a root and has announced nothing;
/// after `bye` and the exit record it is exited with 3, announced, with
/// `bye` on the screen.
#[farhelm_testtrace::test]
async fn a_reconnect_finding_the_child_reaped_withdraws_only_the_root() {
    let (notifier, mut changes) = LivenessNotifier::channel();
    let options = HostTerminalOptions {
        notifier: Some(notifier),
        ..options()
    };
    let stand_in = StandIn::new();
    let (terminal, server, _) = adopt(&stand_in, Adoption::Fresh, &info(0), options).await;
    assert!(terminal.child_root().is_some(), "premise: a pinned child");

    drop(server);
    let reaped = Info {
        child_exit: Some(ChildExit::Code(3)),
        ..info(0)
    };
    let (mut server, _) = stand_in.accept(&reaped).await;
    wait_until(&terminal, "the root's withdrawal", |terminal| {
        terminal.liveness() == Liveness::Running { child: None }
    })
    .await;
    assert!(changes.try_recv().is_err(), "an exit announced too early");

    server.output(0, b"bye").await;
    server
        .send(&HostMessage::Exited {
            offset: 3,
            exit: ChildExit::Code(3),
        })
        .await;
    let change = within("the exit's announcement", changes.recv())
        .await
        .expect("the notifier is open");
    assert_eq!(
        change.liveness,
        Liveness::Exited {
            exit: ChildExit::Code(3)
        }
    );
    assert!(terminal.screen_tail(4096).contains("bye"));
}

/// The adapters give, for each liveness, what a tmux pane in the matching
/// state gives, and a pinned child counts as running only while its
/// process still has its start identity.
///
/// Why: the verbs and the status code decide on those answers for tmux
/// panes, and reuse them for host terminals; a wrong mapping would let Stop
/// signal a reaped pid, or read an unavailable host as exited. Spec: the
/// table below, row by row: a running child pinned to this process is a
/// live root; one pinned to this pid with another start identity (a pid
/// reused since) has no root and probes dead; a child the host reaped
/// before its record has no root and probes dead while its evidence says
/// alive; a signal death has no exit code; an unavailable host has no
/// anchor and a probe that is an error.
#[test]
fn the_adapters_map_each_liveness_like_a_pane() {
    let own = ProcessIdentity::read(std::process::id()).expect("this process");
    let root = ProcessRoot {
        pid: own.pid,
        start: own.start,
    };
    let reused = ProcessRoot {
        start: own.start.wrapping_add(1),
        ..root
    };
    let status = |liveness| HostStatus {
        liveness,
        child_pid: own.pid,
        child_start: Some(own.start),
        position: Position::default(),
        revision: 0,
        caught_up: true,
    };
    let anchor = Some(PaneAnchor {
        pid: own.pid,
        start: Some(own.start),
    });
    let owned = |pid, dead| Some(PaneProbe::Owned(PaneProcess { pid, dead }));
    let alive = HostEvidence::Owned {
        dead: false,
        exit_code: None,
    };
    let unavailable = Liveness::Unavailable {
        reason: Unavailability::Unstable,
        peer: None,
    };
    let rows = [
        (
            Liveness::Running { child: Some(root) },
            Some(root),
            owned(Some(own.pid), false),
            alive,
            None,
            anchor,
        ),
        (
            Liveness::Running {
                child: Some(reused),
            },
            None,
            owned(None, true),
            alive,
            None,
            anchor,
        ),
        (
            Liveness::Running { child: None },
            None,
            owned(None, true),
            alive,
            None,
            anchor,
        ),
        (
            Liveness::Exited {
                exit: ChildExit::Code(0),
            },
            None,
            owned(None, true),
            HostEvidence::Owned {
                dead: true,
                exit_code: Some(0),
            },
            Some(0),
            anchor,
        ),
        (
            Liveness::Exited {
                exit: ChildExit::Signal(9),
            },
            None,
            owned(None, true),
            HostEvidence::Owned {
                dead: true,
                exit_code: None,
            },
            None,
            anchor,
        ),
        (
            Liveness::Gone,
            None,
            Some(PaneProbe::Gone),
            HostEvidence::Absent,
            None,
            anchor,
        ),
        (
            unavailable,
            None,
            None,
            HostEvidence::Unattributed,
            None,
            None,
        ),
    ];
    for (liveness, root, probe, evidence, exit_code, anchor) in rows {
        let status = status(liveness);
        assert_eq!(status.child_root(), root, "{status:?}");
        assert_eq!(status.probe().ok(), probe, "{status:?}");
        assert_eq!(status.evidence(), evidence, "{status:?}");
        assert_eq!(status.exit_code(), exit_code, "{status:?}");
        assert_eq!(status.anchor(), anchor, "{status:?}");
    }
}

// ---- Input -----------------------------------------------------------------------

/// Each input's fate follows the host's acknowledgements, and input whose
/// connection ended unacknowledged is unknown and never sent again.
///
/// Why: some of an unacknowledged input may have reached the program, so
/// sending it again could paste or submit twice; a caller must be told it
/// is unknown instead. Spec: `one` acknowledged as written is `Written`;
/// `two` acknowledged as discarded is `Discarded`; `three`, sent and then
/// cut off, is `Unknown`, and on the new connection the first input is
/// `four`, sent after it; with the host then gone (its socket removed), the
/// terminal is gone, and input and resizes after that are refused (or input
/// never sent).
#[farhelm_testtrace::test]
async fn input_fates_follow_acknowledgements_and_nothing_is_resent() {
    let stand_in = StandIn::new();
    let info = info(0);
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info, options()).await;

    let one = send(&terminal, b"one").await;
    assert_eq!(server.input().await, b"one");
    server.ack(3).await;
    assert_eq!(fate(one).await, InputFate::Written);

    let two = send(&terminal, b"two").await;
    assert_eq!(server.input().await, b"two");
    server
        .send(&HostMessage::InputAck {
            written: 3,
            discarded: 3,
        })
        .await;
    assert_eq!(fate(two).await, InputFate::Discarded);

    let three = send(&terminal, b"three").await;
    assert_eq!(server.input().await, b"three");
    drop(server);
    assert_eq!(fate(three).await, InputFate::Unknown);
    let (mut server, _) = stand_in.accept(&info).await;
    let _four = send(&terminal, b"four").await;
    assert_eq!(server.input().await, b"four", "a lost input was resent");

    // The host goes away without a word, and its socket file with it, so
    // the reconnect finds nothing.
    std::fs::remove_file(stand_in.socket()).unwrap();
    drop(server);
    wait_until(&terminal, "the host's end", |terminal| {
        terminal.liveness() == Liveness::Gone
    })
    .await;
    match within("input to an ended terminal", terminal.send_input(b"five")).await {
        Err(InputRefused::Ended) => {}
        // Queued just before the runtime closed its queue, and settled as it
        // closed it.
        Ok(receipt) => assert_eq!(fate(receipt).await, InputFate::NotSent),
        Err(other) => panic!("expected the input refused, got {other:?}"),
    }
    assert_eq!(
        terminal.send_input(b"").await.unwrap_err(),
        InputRefused::Invalid
    );
    wait_until(&terminal, "the runtime's end", |terminal| {
        terminal.resize(Size { cols: 90, rows: 20 }) == Err(ResizeRefused::Ended)
    })
    .await;
}

/// Fill the input window with three inputs of `MAX_INPUT` bytes the
/// stand-in reads and does not acknowledge, and queue a fourth, which waits
/// for the window. Returns the four receipts.
async fn fill_the_window(terminal: &HostTerminal, server: &mut Server) -> Vec<InputReceipt> {
    let mut receipts = Vec::new();
    for byte in *b"abc" {
        receipts.push(send(terminal, &[byte; MAX_INPUT]).await);
        assert_eq!(server.input().await, [byte; MAX_INPUT]);
    }
    const _: () = assert!(4 * (MAX_INPUT + INPUT_COST) > INPUT_WINDOW);
    receipts.push(send(terminal, &[b'd'; MAX_INPUT]).await);
    receipts
}

/// While the input window is full, callers' sends wait, a send cancelled
/// while waiting sends nothing, and input moves again as acknowledgements
/// make room.
///
/// Why: a program that stops reading its terminal must hold back its
/// callers rather than grow the supervisor, and a caller that gives up on a
/// waiting send must be able to trust that nothing was sent. Spec: with the
/// window full and a fourth input waiting for it, the runtime takes 16 more
/// small inputs into its queue and a 17th send is pending when polled, and
/// still pending when polled again after the test yields to the runtime
/// (the boundary: on the current-thread runtime the yield lets the runtime
/// task run, and a runtime that took input with the window full would
/// empty its queue and let the 17th through), and is then dropped; once the
/// first input is acknowledged, the stand-in receives the fourth input and
/// the 16 in order, and then `fence`, sent last, with nothing of the
/// dropped one.
#[farhelm_testtrace::test]
async fn callers_wait_while_the_window_is_full() {
    let stand_in = StandIn::new();
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info(0), options()).await;
    let _filled = fill_the_window(&terminal, &mut server).await;
    let mut queued = Vec::new();
    for n in 0..INPUT_QUEUE {
        queued.push(send(&terminal, format!("q{n}").as_bytes()).await);
    }
    let mut dropped = Box::pin(terminal.send_input(b"dropped"));
    assert!(
        pending_once(&mut dropped).await,
        "a send went through a full window"
    );
    for _ in 0..8 {
        tokio::task::yield_now().await;
    }
    assert!(
        pending_once(&mut dropped).await,
        "a send went through a full window once the runtime ran"
    );
    drop(dropped);

    server.ack(MAX_INPUT as u64).await;
    assert_eq!(server.input().await, [b'd'; MAX_INPUT]);
    for n in 0..INPUT_QUEUE {
        assert_eq!(server.input().await, format!("q{n}").as_bytes());
    }
    let _fence = send(&terminal, b"fence").await;
    assert_eq!(server.input().await, b"fence");
}

/// When caller input is lost with a connection, input callers asked to send
/// before that is not sent after the hole, wherever it waits, and input
/// asked for afterwards is.
///
/// Why: the program would otherwise receive a paste or a typed line with
/// its middle missing, such as a bracketed paste's end marker without its
/// start. A caller chunking a paste is likely to be blocked on the full
/// queue when it happens, and draining the queue makes room for exactly
/// that caller. Spec: with three inputs in flight, a fourth waiting for the
/// window, 16 more filling the runtime's queue and a 17th send waiting for
/// room (pending when polled), a dropped
/// connection makes the three `Unknown` and the fourth, the 16 and the 17th
/// `NotSent`; on the new connection the first input is `after`, sent
/// afterwards.
#[farhelm_testtrace::test]
async fn nothing_is_written_after_input_lost_with_a_connection() {
    let stand_in = StandIn::new();
    let info = info(0);
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info, options()).await;
    let mut receipts = fill_the_window(&terminal, &mut server).await;
    for n in 0..INPUT_QUEUE {
        receipts.push(send(&terminal, format!("q{n}").as_bytes()).await);
    }
    // Waiting for room, so that it lands in the queue only after the drop.
    let mut waiting = Box::pin(terminal.send_input(b"waiting"));
    assert!(pending_once(&mut waiting).await, "premise: the send waits");

    drop(server);
    let (mut server, _) = stand_in.accept(&info).await;
    let waiting = within("the waiting send", waiting)
        .await
        .expect("the runtime takes the input");
    receipts.push(waiting);
    let mut fates = Vec::new();
    for receipt in receipts {
        fates.push(fate(receipt).await);
    }
    use InputFate::{NotSent, Unknown};
    let mut expected = vec![Unknown, Unknown, Unknown];
    expected.resize(4 + INPUT_QUEUE + 1, NotSent);
    assert_eq!(fates, expected);
    let _after = send(&terminal, b"after").await;
    assert_eq!(server.input().await, b"after");
}

/// Input cut off while it is being written is unknown and not sent again;
/// the connection's loss moves the input epoch once, and input sent on
/// hearing of it is written.
///
/// Why: an `Input` the client queued counts as sent even when the write
/// then fails, since part of it may have reached the host; sending it again
/// on the next connection could deliver it twice. And one lost connection
/// is one hole: moving the epoch twice for it would refuse input a caller
/// sent on hearing `Unknown`, stamped between the two moves (on the
/// supervisor's multi-thread runtime that can happen). Spec (Linux, whose
/// unix sockets fail a write to a peer that shut down its reading side
/// with `EPIPE`, deterministically): with `one` sent and not acknowledged,
/// and the stand-in then refusing further reads on that connection, `two`
/// is cut off; both are `Unknown`, the epoch has moved once, and the first
/// input on the new connection is `three`, sent on hearing that.
#[cfg(target_os = "linux")]
#[farhelm_testtrace::test]
async fn input_cut_off_while_written_is_unknown() {
    use std::os::fd::AsRawFd as _;
    let stand_in = StandIn::new();
    let info = info(0);
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info, options()).await;
    let one = send(&terminal, b"one").await;
    assert_eq!(server.input().await, b"one");

    // SAFETY: shutting down the reading side of a socket this test owns.
    assert_eq!(
        unsafe { libc::shutdown(server.stream.as_raw_fd(), libc::SHUT_RD) },
        0
    );
    let two = send(&terminal, b"two").await;
    assert_eq!(fate(two).await, InputFate::Unknown);
    assert_eq!(fate(one).await, InputFate::Unknown);
    assert_eq!(terminal.inner.epoch.load(Ordering::SeqCst), 1);
    let _three = send(&terminal, b"three").await;
    let (mut new, _) = stand_in.accept(&info).await;
    assert_eq!(new.input().await, b"three");
    drop(server);
}

/// The parts of a paste sent through one input sequence are not written
/// after input lost with a connection, though the sender never waited for
/// a receipt; one-shot input sent then is written.
///
/// Why: a paste is sent part after part without waiting for each part's
/// receipt, which would give up the window's pipelining; a part asked for
/// after a loss would otherwise be written after the hole, and the program
/// would receive the paste with its middle missing. Spec: `part one` sent
/// through a sequence is in flight when the connection drops; `part two`,
/// sent through the same sequence once the runtime has reconnected, is
/// `NotSent`; `fresh`, sent with `send_input` then, is the first input on
/// the new connection; `part one` is `Unknown`. Once every terminal handle
/// is dropped, a further part is refused as ended: a sequence kept past its
/// paste must not keep the runtime and its connection alive.
#[farhelm_testtrace::test]
async fn a_sequence_sends_nothing_after_input_lost_with_a_connection() {
    let stand_in = StandIn::new();
    let info = info(0);
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info, options()).await;
    let paste = terminal.input_sequence();
    let one = within("queueing a part", paste.send(b"part one"))
        .await
        .unwrap();
    assert_eq!(server.input().await, b"part one");

    drop(server);
    let (mut server, _) = stand_in.accept(&info).await;
    let two = within("queueing a part", paste.send(b"part two"))
        .await
        .unwrap();
    let _fresh = send(&terminal, b"fresh").await;
    assert_eq!(server.input().await, b"fresh");
    assert_eq!(fate(two).await, InputFate::NotSent);
    assert_eq!(fate(one).await, InputFate::Unknown);

    // A sequence holds its terminal weakly: once every handle is gone, it
    // keeps nothing alive and its parts are refused.
    drop(terminal);
    assert_eq!(
        paste.send(b"part three").await.unwrap_err(),
        InputRefused::Ended
    );
}

/// A lost connection frees no room in the runtime's input queue: callers
/// still blocked on it stay blocked while the runtime reconnects, and the
/// requests already queued are refused one at a time in the runtime's turn,
/// as the queue's own bound allows.
///
/// Why: the queue's bound (`INPUT_QUEUE`) is what limits the caller input
/// the supervisor holds for a program that is not reading; a reconnect that
/// emptied the queue in one go would let busy callers refill each freed
/// slot as it was freed, with nothing bounding how many requests it took
/// in or how long it put off the reconnect. Spec: with the window full, a
/// fourth input waiting for it and three producers sending `MAX_INPUT`
/// inputs without pause, the queue fills with 16 and the producers block;
/// after the connection drops, while the runtime waits for the new
/// connection's answer to `Info`, the producers have still had exactly 16
/// admitted after the test yields to them repeatedly; once the reconnect
/// completes, the runtime goes on taking input (new inputs reach the
/// stand-in) and a resize asked for then reaches it too.
#[farhelm_testtrace::test]
async fn a_lost_connection_frees_no_room_in_the_input_queue() {
    use std::sync::atomic::AtomicUsize;
    let stand_in = StandIn::new();
    let info = info(0);
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info, options()).await;
    let _filled = fill_the_window(&terminal, &mut server).await;
    let admitted = Arc::new(AtomicUsize::new(0));
    let producers: Vec<_> = (0..3)
        .map(|_| {
            let terminal = terminal.clone();
            let admitted = admitted.clone();
            tokio::spawn(async move {
                while terminal.send_input(&[b'p'; MAX_INPUT]).await.is_ok() {
                    admitted.fetch_add(1, Ordering::SeqCst);
                }
            })
        })
        .collect();
    within("the queue to fill", async {
        while admitted.load(Ordering::SeqCst) < INPUT_QUEUE {
            tokio::task::yield_now().await;
        }
    })
    .await;
    for _ in 0..8 {
        tokio::task::yield_now().await;
    }
    assert_eq!(
        admitted.load(Ordering::SeqCst),
        INPUT_QUEUE,
        "premise: the producers block on a full queue"
    );

    drop(server);
    let mut server = stand_in.accept_hello().await;
    assert_eq!(server.next().await, ClientMessage::Info);
    // The runtime now waits for this answer; freed slots would be refilled
    // by the producers as soon as they run.
    for _ in 0..8 {
        tokio::task::yield_now().await;
    }
    assert_eq!(
        admitted.load(Ordering::SeqCst),
        INPUT_QUEUE,
        "the reconnect freed room in the queue"
    );

    server.send(&HostMessage::InfoReply(info.clone())).await;
    loop {
        match server.next().await {
            ClientMessage::Info => server.send(&HostMessage::InfoReply(info.clone())).await,
            ClientMessage::Read(_) => break,
            other => panic!("expected Info or Read, got {other:?}"),
        }
    }
    assert_eq!(server.input().await, [b'p'; MAX_INPUT]);
    let size = Size { cols: 90, rows: 20 };
    terminal.resize(size).unwrap();
    within("the resize among the input", async {
        loop {
            match server.next().await {
                ClientMessage::Resize(resized) if resized == size => break,
                ClientMessage::Input(_) => {}
                other => panic!("expected Input or Resize, got {other:?}"),
            }
        }
    })
    .await;
    for producer in producers {
        producer.abort();
    }
}

/// Answers to a flood of queries the program never reads are held only up
/// to the window and [`MAX_PENDING_ANSWERS`]; the rest are dropped.
///
/// Why: one program must not grow the shared supervisor without bound by
/// asking questions faster than it reads the answers. Spec: with nothing
/// acknowledged, 80,000 DA1 queries (560,000 bytes of answers) are fed; the
/// answers then received, once acknowledgements open the window again and
/// a status request ends the flood, are whole DA1 answers, more than the
/// held answers alone could be, and no more than the window plus the held
/// answers.
#[farhelm_testtrace::test]
async fn answers_past_the_bound_are_dropped() {
    let stand_in = StandIn::new();
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info(0), options()).await;
    let queries = b"\x1b[c".repeat(20_000);
    for n in 0..4u64 {
        server.output(n * queries.len() as u64, &queries).await;
    }
    let fed = 4 * queries.len() as u64;
    let mut received: Vec<u8> = Vec::new();
    // Read without acknowledging until every query has been fed, so that
    // all the answers are made while the window stays shut.
    let mut status = terminal.subscribe();
    within("feeding the flood", async {
        while status.borrow_and_update().position.offset < fed {
            tokio::select! {
                input = server.input() => received.extend(input),
                changed = status.changed() => changed.expect("the runtime publishes"),
            }
        }
    })
    .await;
    server.ack(received.len() as u64).await;
    server.output(fed, b"\x1b[5n").await;
    while !received.ends_with(STATUS_ANSWER) {
        let input = server.input().await;
        received.extend(&input);
        server.ack(received.len() as u64).await;
    }
    let answers = &received[..received.len() - STATUS_ANSWER.len()];
    assert_eq!(answers.len() % DA1_ANSWER.len(), 0);
    assert!(
        answers.chunks(DA1_ANSWER.len()).all(|a| a == DA1_ANSWER),
        "answers were cut"
    );
    assert!(answers.len() > MAX_PENDING_ANSWERS, "{}", answers.len());
    assert!(
        answers.len() <= INPUT_WINDOW + MAX_PENDING_ANSWERS,
        "{} bytes of answers held",
        answers.len()
    );
}

// ---- Connections and shutdown -----------------------------------------------------

/// A dropped connection is reconnected and read from where processing
/// stopped, and a resize asked for on the new connection reaches the
/// emulator through the host's resize record.
///
/// Why: a connection ends without the host ending (a reply timed out on a
/// loaded machine, say); a terminal that stopped there would freeze for
/// good, and one that read from the start again would draw everything
/// twice. Resizes take effect at their place in the output, so the
/// emulator follows the host's record, not the request. Spec: after `abc`
/// and a dropped connection, the runtime reads from (3, 0) on a new one;
/// `def` then shows `abcdef`; a resize to 90x20 sends `Resize`, and the
/// emulator is 90x20 once the stand-in sends its record; a size of 0 is
/// refused.
#[farhelm_testtrace::test]
async fn a_dropped_connection_resumes_where_processing_stopped() {
    let stand_in = StandIn::new();
    let info = info(0);
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info, options()).await;
    server.output(0, b"abc").await;
    wait_until(&terminal, "abc", screen_has("abc")).await;

    drop(server);
    let (mut server, from) = stand_in.accept(&info).await;
    assert_eq!(
        from,
        Position {
            offset: 3,
            resizes: 0
        }
    );
    server.output(3, b"def").await;
    wait_until(&terminal, "the resumed output", screen_has("abcdef")).await;

    let size = Size { cols: 90, rows: 20 };
    terminal.resize(size).unwrap();
    assert_eq!(server.next().await, ClientMessage::Resize(size));
    assert_ne!(
        terminal.size(),
        size,
        "premise: not resized before the record"
    );
    server
        .send(&HostMessage::Resized {
            offset: 6,
            index: 0,
            size,
        })
        .await;
    wait_until(&terminal, "the resize", |terminal| terminal.size() == size).await;
    assert_eq!(
        terminal.resize(Size { cols: 0, rows: 20 }),
        Err(ResizeRefused::Invalid)
    );
}

/// A resize written to a connection that ended before the host acted on it
/// is sent again on the next one.
///
/// Why: a host drops what a displaced client sent and it had not acted on,
/// so a resize that went out whole can still be lost; without a second
/// send the program and the screen stay at the old size until the viewer
/// happens to resize again. Spec: a resize to 90x20 is received, the
/// connection is dropped without a resize record, and the new connection,
/// whose `Info` still says 80x24, receives the resize again.
#[farhelm_testtrace::test]
async fn a_resize_lost_with_a_connection_is_sent_again() {
    let stand_in = StandIn::new();
    let info = info(0);
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info, options()).await;
    let size = Size { cols: 90, rows: 20 };
    terminal.resize(size).unwrap();
    assert_eq!(server.next().await, ClientMessage::Resize(size));
    drop(server);
    let (mut server, _) = stand_in.accept(&info).await;
    assert_eq!(server.next().await, ClientMessage::Resize(size));
}

/// A resize the host already applied is not sent again on the next
/// connection.
///
/// Why: a repeated resize is harmless but not free: it adds a resize record
/// to the journal and signals the program (`SIGWINCH`), which redraws.
/// Spec: a resize to 90x20 is received, the connection is dropped, and the
/// new connection, whose `Info` already says 90x20, receives `fence`, sent
/// afterwards, as its first frame.
#[farhelm_testtrace::test]
async fn a_resize_already_in_force_is_not_sent_again() {
    let stand_in = StandIn::new();
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info(0), options()).await;
    let size = Size { cols: 90, rows: 20 };
    terminal.resize(size).unwrap();
    assert_eq!(server.next().await, ClientMessage::Resize(size));
    drop(server);
    let resized = Info { size, ..info(0) };
    let (mut server, _) = stand_in.accept(&resized).await;
    let _fence = send(&terminal, b"fence").await;
    assert_eq!(server.next().await, ClientMessage::Input(b"fence".to_vec()));
}

/// A connection that keeps ending makes the terminal unavailable after
/// [`RECONNECT_LIMIT`] reconnects, rather than fighting for the host.
///
/// Why: a host that answers and is taken away again and again has another
/// owner, and every reconnect would cut that owner off in turn. Spec: with
/// the adoption's connection and three reconnects each dropped, the
/// terminal is unavailable as `Unstable`, naming the stand-in, and no
/// further connection is made.
#[farhelm_testtrace::test]
async fn a_connection_that_keeps_ending_makes_the_terminal_unavailable() {
    let stand_in = StandIn::new();
    let info = info(0);
    let (terminal, server, _) = adopt(&stand_in, Adoption::Fresh, &info, options()).await;
    drop(server);
    for _ in 0..RECONNECT_LIMIT {
        let (server, _) = stand_in.accept(&info).await;
        drop(server);
    }
    wait_until(&terminal, "unavailability", |terminal| {
        matches!(terminal.liveness(), Liveness::Unavailable { .. })
    })
    .await;
    match terminal.liveness() {
        Liveness::Unavailable {
            reason: Unavailability::Unstable,
            peer: Some(peer),
        } => assert_eq!(peer.pid, std::process::id()),
        other => panic!("expected an unstable connection, got {other:?}"),
    }
    // An observation window: no connection may arrive within it.
    let another =
        tokio::time::timeout(Duration::from_millis(300), stand_in.listener.accept()).await;
    assert!(another.is_err(), "the runtime connected again");
}

/// A shutdown on a connection that ended without the runtime noticing goes
/// out again on a new connection.
///
/// Why: the runtime learns that a connection ended only by using it; a
/// shutdown requested in between would otherwise report a healthy host
/// unconfirmed and leave Delete to signal it. Spec (Linux, see
/// `input_cut_off_while_written_is_unknown`): with the stand-in refusing
/// further reads on the adopted connection, a shutdown connects again; the
/// stand-in receives `Shutdown` on the new connection and answers
/// `Goodbye`, and the report has the goodbye (its process, this one, stays,
/// so the end is unconfirmed after a 300 ms shutdown deadline).
#[cfg(target_os = "linux")]
#[farhelm_testtrace::test]
async fn a_shutdown_on_a_connection_ended_unnoticed_goes_out_anew() {
    use std::os::fd::AsRawFd as _;
    let mut options = options();
    options.deadlines.shutdown = Duration::from_millis(300);
    let stand_in = StandIn::new();
    let (terminal, server, _) = adopt(&stand_in, Adoption::Fresh, &info(0), options).await;
    // SAFETY: shutting down the reading side of a socket this test owns.
    assert_eq!(
        unsafe { libc::shutdown(server.stream.as_raw_fd(), libc::SHUT_RD) },
        0
    );
    let report = async { within("the shutdown", terminal.shutdown()).await };
    let host = async {
        let mut new = stand_in.accept_hello().await;
        assert_eq!(new.next().await, ClientMessage::Shutdown);
        new.send(&HostMessage::Goodbye).await;
        new
    };
    let (report, _new) = tokio::join!(report, host);
    let report = report.expect("the runtime shuts down");
    assert!(report.goodbye, "{report:?}");
    assert!(!report.ended, "{report:?}");
    drop(server);
}

/// A reconnect that finds another process at the socket, one that does not
/// answer, makes the terminal unavailable as `Replaced`, naming the adopted
/// host rather than the process found.
///
/// Why: whatever a report names as the host's process is what Delete may
/// signal; a process the runtime never adopted must never be named, even
/// when the connect to it failed and only its peer credentials say who it
/// is. Spec: after the stand-in's socket file is replaced by one a Python
/// process listens on and never answers, and the stand-in's connection is
/// dropped, the terminal is unavailable as `Replaced` with this process
/// (the adopted stand-in) as its peer.
#[farhelm_testtrace::test]
async fn a_reconnect_to_another_process_is_replaced() {
    let stand_in = StandIn::new();
    let (terminal, server, _) = adopt(&stand_in, Adoption::Fresh, &info(0), options()).await;
    let socket = stand_in.socket();
    std::fs::remove_file(&socket).unwrap();
    let mut other = tokio::process::Command::new("python3")
        .args(["-I", "-c", OTHER_LISTENER])
        .arg(&socket)
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("starting python3");
    let mut ready = String::new();
    let mut stdout = tokio::io::BufReader::new(other.stdout.take().unwrap());
    within("the other listener", async {
        use tokio::io::AsyncBufReadExt as _;
        stdout.read_line(&mut ready).await.unwrap()
    })
    .await;
    assert_eq!(
        ready.trim(),
        "listening",
        "premise: the other process listens"
    );

    drop(server);
    wait_until(&terminal, "unavailability", |terminal| {
        matches!(terminal.liveness(), Liveness::Unavailable { .. })
    })
    .await;
    match terminal.liveness() {
        Liveness::Unavailable {
            reason: Unavailability::Replaced,
            peer: Some(peer),
        } => assert_eq!(peer.pid, std::process::id()),
        other => panic!("expected the host replaced, naming the adopted one, got {other:?}"),
    }
}

/// A listener for [`a_reconnect_to_another_process_is_replaced`] that holds
/// every connection and never answers, at the path given as its argument.
const OTHER_LISTENER: &str = "\
import socket, sys
s = socket.socket(socket.AF_UNIX)
s.bind(sys.argv[1])
s.listen(8)
print('listening', flush=True)
held = []
while True:
    held.append(s.accept()[0])
";

/// A shutdown whose host acknowledges it but whose process does not end is
/// reported unconfirmed, with the process, and leaves the terminal
/// unavailable.
///
/// Why: Delete falls back to the verified SIGTERM only when it knows the
/// host may still run; reading a goodbye as proof would leak the host.
/// Spec: the stand-in (this process, which goes on running) answers
/// `Shutdown` with `Goodbye`; with a 300 ms shutdown deadline, the report
/// has the goodbye, no confirmed end and this process as the peer, and the
/// terminal is unavailable as `ShutdownUnconfirmed`.
#[farhelm_testtrace::test]
async fn a_shutdown_whose_process_stays_is_unconfirmed() {
    let mut options = options();
    options.deadlines.shutdown = Duration::from_millis(300);
    let stand_in = StandIn::new();
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info(0), options).await;
    let report = async { within("the shutdown", terminal.shutdown()).await };
    let host = async {
        assert_eq!(server.next().await, ClientMessage::Shutdown);
        server.send(&HostMessage::Goodbye).await;
    };
    let (report, ()) = tokio::join!(report, host);
    let report = report.expect("the runtime shuts down");
    assert!(report.goodbye);
    assert!(!report.ended);
    assert_eq!(report.peer.map(|peer| peer.pid), Some(std::process::id()));
    assert!(matches!(
        terminal.liveness(),
        Liveness::Unavailable {
            reason: Unavailability::ShutdownUnconfirmed,
            ..
        }
    ));
}
