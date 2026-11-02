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
    ClientMessage, HostMessage, INPUT_COST, MAX_FRAME, SnapshotRejection, decode_client,
    encode_host, split_frame,
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

/// A recovered adoption answers the queries no supervisor saw and only
/// those: not those in output an earlier supervisor was sent, and those in
/// live output as ever, splitting a read across the three.
///
/// Why: this is the replay policy (D-7). An earlier supervisor answered the
/// queries in output it was sent; answering them again would type the
/// answers into the program (a shell shows them as garbage at its prompt).
/// A query written while no supervisor was attached was never answered,
/// and the program may still be waiting for it. Spec: with `Info` saying 8
/// bytes were journaled and 4 of them sent to a client (`delivered_end`),
/// one `Output` at 0 holds DA1 and `x` (seen), a status request (unseen,
/// to the journal's end) and DA1 again (live); the program gets the status
/// answer and then the DA1 answer, in one `Input`, and nothing for the
/// first DA1; `x` is drawn, and the runtime, reading from (0, 0), is caught
/// up once past the 8 bytes.
#[farhelm_testtrace::test]
async fn a_recovered_adoption_answers_only_what_no_supervisor_saw() {
    let stand_in = StandIn::new();
    let journaled = Info {
        delivered_end: 4,
        ..info(8)
    };
    let (terminal, mut server, from) =
        adopt(&stand_in, Adoption::Recovered, &journaled, options()).await;
    assert_eq!(from, Position::default());
    assert!(!terminal.status().caught_up, "premise: 8 bytes to replay");

    server.output(0, b"\x1b[cx\x1b[5n\x1b[c").await;
    assert_eq!(server.input().await, [STATUS_ANSWER, DA1_ANSWER].concat());
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
/// the effects of the replay's last part, so the live output that ends it
/// answers nothing from before the adoption a second time.
///
/// Why: vte holds an update's bytes and interprets them when it ends, with
/// the effects of whichever feed ends it; an update opened in the replayed
/// output and ended by the first live feed would answer its replayed
/// queries live. Spec: with 11 bytes journaled (an update start and DA1),
/// all sent to an earlier supervisor, then live output ending the update
/// and asking for status, the first answer the program gets is the status
/// answer.
#[farhelm_testtrace::test]
async fn a_replay_ending_inside_an_update_answers_nothing_of_it() {
    let stand_in = StandIn::new();
    let seen = Info {
        delivered_end: 11,
        ..info(11)
    };
    let (_terminal, mut server, _) = adopt(&stand_in, Adoption::Recovered, &seen, options()).await;
    server.output(0, b"\x1b[?2026h\x1b[c").await;
    server.output(11, b"\x1b[?2026l\x1b[5n").await;
    assert_eq!(server.input().await, STATUS_ANSWER);
}

/// A replay that ends inside an open synchronized update no supervisor saw
/// draws it when the replay ends, answering its query once, and shows its
/// frame without waiting for its deadline.
///
/// Why: a query a program asked while no supervisor was attached is
/// answered once, whether or not it sits in an update the program left
/// open; and a frame left open when the replay ends is drawn then, since
/// the replay is all the output there is (a pending update flushed during
/// recovery). Spec: with 17 bytes journaled and none sent (an update start,
/// DA1 and `hidden`), the program gets the DA1 answer, `hidden` is on the
/// screen once caught up with the runtime's manual clock never advanced,
/// and live output at 17 ending the update and asking for status gets the
/// status answer next, and no second DA1 answer, with the end marker
/// interpreted (no `2026l` on the screen).
#[farhelm_testtrace::test]
async fn a_replay_ending_inside_an_unseen_update_answers_it_once() {
    let clock = ManualClock::new();
    let options = HostTerminalOptions {
        clock: Clock::Manual(clock),
        ..options()
    };
    let stand_in = StandIn::new();
    let journaled = b"\x1b[?2026h\x1b[chidden";
    let end = journaled.len() as u64;
    assert_eq!(end, 17);
    let (terminal, mut server, _) =
        adopt(&stand_in, Adoption::Recovered, &info(end), options).await;
    server.output(0, journaled).await;
    assert_eq!(server.input().await, DA1_ANSWER);
    wait_until(&terminal, "the replay", |terminal| {
        terminal.status().caught_up
    })
    .await;
    assert!(terminal.screen_tail(4096).contains("hidden"));
    server.output(end, b"\x1b[?2026l\x1b[5n").await;
    assert_eq!(server.input().await, STATUS_ANSWER);
    let screen = terminal.screen_tail(4096);
    assert!(
        screen.contains("hidden") && !screen.contains("2026"),
        "{screen:?}"
    );
}

/// A resize at `delivered_end` that draws an update an earlier supervisor
/// was sent answers nothing from it.
///
/// Why: a resize draws an open update with the effects the runtime gives
/// it; the update's bytes are seen output, whose queries the earlier
/// supervisor answered, so a resize processed where the unseen part
/// begins must still draw them as seen (effects follow the bytes). Spec:
/// with 11 bytes journaled and sent (an update start and DA1) and a resize
/// at 11 journaled after them, a live status request at 11 gets the status
/// answer as the first input, and no DA1 answer.
#[farhelm_testtrace::test]
async fn a_resize_at_delivered_end_answers_nothing_seen() {
    let journaled = Info {
        delivered_end: 11,
        resizes: 1,
        ..info(11)
    };
    let stand_in = StandIn::new();
    let (_terminal, mut server, _) =
        adopt(&stand_in, Adoption::Recovered, &journaled, options()).await;
    server.output(0, b"\x1b[?2026h\x1b[c").await;
    server
        .send(&HostMessage::Resized {
            offset: 11,
            index: 0,
            size: Size { cols: 90, rows: 20 },
        })
        .await;
    server.output(11, b"\x1b[5n").await;
    assert_eq!(server.input().await, STATUS_ANSWER);
}

/// An update that straddles `delivered_end` answers what it held of the
/// seen output never, and what it held of the unseen output once.
///
/// Why: the update's seen part was sent to an earlier supervisor, which
/// answered it; drawing the whole update with the effects of its unseen end
/// would answer those queries a second time. Spec: with 23 bytes
/// journaled, 11 of them sent (an update start and DA1) and the rest not
/// (a status request and the update's end), the program gets the status
/// answer, and then, for a live status request at 23, the status answer
/// again, with no DA1 answer anywhere.
#[farhelm_testtrace::test]
async fn an_update_straddling_delivered_end_answers_its_seen_part_never() {
    let journaled = Info {
        delivered_end: 11,
        ..info(23)
    };
    let stand_in = StandIn::new();
    let (_terminal, mut server, _) =
        adopt(&stand_in, Adoption::Recovered, &journaled, options()).await;
    server
        .output(0, b"\x1b[?2026h\x1b[c\x1b[5n\x1b[?2026l")
        .await;
    assert_eq!(server.input().await, STATUS_ANSWER);
    server.output(23, b"\x1b[5n").await;
    assert_eq!(server.input().await, STATUS_ANSWER);
}

/// A recovery answers and reports only what no supervisor saw: a query, a
/// clipboard write (OSC 52) and a bell in the snapshot, in output an
/// earlier supervisor was sent after it, and in output written while none
/// was attached, then live ones.
///
/// Why: this is the goal's replay test (D-7). An answer delivered twice
/// reaches the program as typed garbage, and a bell or clipboard write
/// delivered again, maybe minutes later, surprises the user; a query no
/// supervisor saw must still be answered. Spec: with a notice sink, a
/// snapshot record of a terminal that was sent a query, a clipboard write
/// and a bell, then those three again as seen output (delivered up to its
/// end), then a status request, a clipboard write and a bell as unseen
/// output, then a clipboard write of `live` and a bell as live output: the
/// program gets the status answer only, and the sink receives the live
/// clipboard write only (a bell is never sent to a subscription, live or
/// not: nothing that subscribes has a use for it).
#[farhelm_testtrace::test]
async fn a_recovery_answers_and_reports_only_what_no_supervisor_saw() {
    const SEEN: &[u8] = b"\x1b[c\x1b]52;c;c2Vlbg==\x07\x07";
    const UNSEEN: &[u8] = b"\x1b[5n\x1b]52;c;dW5zZWVu\x07\x07";
    const LIVE: &[u8] = b"\x1b]52;c;bGl2ZQ==\x07\x07";
    let snapshot = match seed_record(b"\x1b[c\x1b]52;c;c25hcA==\x07\x07") {
        HostMessage::SnapshotRecord { blob, .. } => HostMessage::SnapshotRecord {
            position: Position {
                offset: 100,
                resizes: 0,
            },
            blob,
        },
        other => unreachable!("{other:?}"),
    };
    let seen_end = 100 + SEEN.len() as u64;
    let end = seen_end + UNSEEN.len() as u64;
    let journaled = Info {
        snapshot: Position {
            offset: 100,
            resizes: 0,
        },
        tail_start: 100,
        delivered_end: seen_end,
        ..info(end)
    };
    let stand_in = StandIn::new();
    let (terminal, mut server, _) = adopt_with(
        &stand_in,
        Adoption::Recovered,
        &journaled,
        options(),
        &[snapshot],
    )
    .await;
    let terminal = terminal.expect("the adoption succeeds");
    let mut notices = terminal.subscribe_notices();
    server.output(100, SEEN).await;
    server.output(seen_end, UNSEEN).await;
    server.output(end, LIVE).await;
    assert_eq!(server.input().await, STATUS_ANSWER);
    let first = within("the first notice", notices.recv()).await;
    assert_eq!(
        first,
        Some(Notice::Clipboard {
            target: farhelm_terminal_emulator::ClipboardTarget::Clipboard,
            text: "live".into()
        })
    );
    server.output(end + LIVE.len() as u64, b"\x1b[5n").await;
    assert_eq!(server.input().await, STATUS_ANSWER);
    assert_eq!(notices.try_recv(), None, "a notice from replayed output");
}

/// Every notice subscription gets every live notice from when it was
/// taken, and one that falls behind loses the oldest rather than holding
/// the terminal up.
///
/// Why: each viewer attachment shows clipboard writes to its own user, so
/// one viewer taking a notice must not take it from another; and a viewer
/// that stalls must not stall the terminal or grow without bound. Spec:
/// with two subscriptions taken before it, one read holding ten clipboard
/// writes (`n0` to `n9`) gives each subscription `n2` to `n9` (the last
/// [`NOTICE_BACKLOG`]), in order, and then nothing; a subscription taken
/// after the read gets nothing of it.
#[farhelm_testtrace::test]
async fn notices_fan_out_and_a_lagging_subscription_loses_the_oldest() {
    const TEXTS: [&str; 10] = [
        "bjA=", "bjE=", "bjI=", "bjM=", "bjQ=", "bjU=", "bjY=", "bjc=", "bjg=", "bjk=",
    ];
    assert_eq!(NOTICE_BACKLOG, 8, "the spec counts on a backlog of 8");
    let stand_in = StandIn::new();
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info(0), options()).await;
    let mut first = terminal.subscribe_notices();
    let mut second = terminal.subscribe_notices();
    let mut output = Vec::new();
    for text in TEXTS {
        output.extend_from_slice(format!("\x1b]52;c;{text}\x07").as_bytes());
    }
    let end = output.len() as u64;
    server.output(0, &output).await;
    wait_until(&terminal, "the clipboard writes", |terminal| {
        terminal.status().position.offset == end
    })
    .await;
    let mut late = terminal.subscribe_notices();
    for subscription in [&mut first, &mut second] {
        let texts: Vec<String> = std::iter::from_fn(|| subscription.try_recv())
            .map(|notice| match notice {
                Notice::Clipboard { text, .. } => text,
                other => panic!("expected a clipboard write, got {other:?}"),
            })
            .collect();
        let expected: Vec<String> = (2..10).map(|n| format!("n{n}")).collect();
        assert_eq!(texts, expected);
    }
    assert_eq!(late.try_recv(), None);
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
/// (restoring again would show `seeded!`). The emulator counts one rebuild
/// throughout, the restore at adoption, which is what tells a viewer's
/// grid stream to send its history again.
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
    assert_eq!(terminal.try_read(|emulator| emulator.rebuilds()), Some(1));
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
    assert_eq!(
        terminal.try_read(|emulator| emulator.rebuilds()),
        Some(1),
        "a record not restored again is no rebuild"
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

// ---- Checkpoints -----------------------------------------------------------------

/// Options with checkpoints due after exactly `after_bytes` of output,
/// whatever the snapshots' sizes, on `clock`.
fn checkpointing(after_bytes: u64, clock: &ManualClock) -> HostTerminalOptions {
    HostTerminalOptions {
        clock: Clock::Manual(clock.clone()),
        checkpoints: CheckpointPolicy {
            min_bytes: after_bytes,
            max_bytes: after_bytes,
        },
        ..options()
    }
}

/// The next `Snapshot` the client sends, answered as `accept` says.
async fn snapshot_request(server: &mut Server, accept: bool) -> (Position, Vec<u8>) {
    let (position, blob) = loop {
        match server.next().await {
            ClientMessage::Snapshot { position, blob } => break (position, blob),
            ClientMessage::Input(_) => {}
            other => panic!("expected Snapshot, got {other:?}"),
        }
    };
    let reply = if accept {
        HostMessage::SnapshotAccepted(position)
    } else {
        HostMessage::SnapshotRejected {
            position,
            reason: SnapshotRejection::OutsideRetained,
        }
    };
    server.send(&reply).await;
    (position, blob)
}

/// Whether the terminal's emulator is away being encoded (module docs,
/// "Checkpoints"): the premise of the tests that check what answers
/// meanwhile, which would pass vacuously if the encode had not started or
/// had already finished.
fn emulator_away(terminal: &HostTerminal) -> bool {
    matches!(
        terminal
            .inner
            .emulator
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .slot,
        Slot::Encoding(_)
    )
}

/// The screen text of a snapshot blob.
fn restored_screen(blob: &[u8]) -> String {
    Emulator::restore(blob, 100)
        .expect("a blob the runtime made restores")
        .emulator
        .screen_text()
}

/// Once enough output has passed, the runtime snapshots the emulator at
/// the end of a clean read and hands the snapshot to the host; a rejected
/// one leaves the previous, and the next is due only after as much output
/// again.
///
/// Why: checkpoints are what let the host drop raw output, keep its tail
/// short, and leave its cap for a supervisor's absence; a snapshot taken
/// anywhere but a clean boundary could not be continued from, and one
/// retried at every read after a rejection would cost a whole snapshot per
/// read. Spec: with checkpoints due after 10 bytes, `hello world!` (12
/// bytes, clean at its end) brings a `Snapshot` at (12, 0) whose blob
/// shows `hello world!`; rejected, the status names no checkpoint, and
/// `x` and a status request (5 bytes, so 5 since the attempt) bring none
/// (the stand-in's next frame is the status answer); `0123456789` and
/// another status request then bring one at (27, 0), at the split
/// candidate before the request, and accepted, the status names it.
#[farhelm_testtrace::test]
async fn checkpoints_are_taken_at_clean_reads_once_due() {
    let clock = ManualClock::new();
    let stand_in = StandIn::new();
    let (terminal, mut server, _) = adopt(
        &stand_in,
        Adoption::Fresh,
        &info(0),
        checkpointing(10, &clock),
    )
    .await;
    server.output(0, b"hello world!").await;
    let (position, blob) = snapshot_request(&mut server, false).await;
    assert_eq!(
        position,
        Position {
            offset: 12,
            resizes: 0
        }
    );
    assert!(restored_screen(&blob).contains("hello world!"));
    assert_eq!(terminal.status().checkpoint, None);

    server.output(12, b"x\x1b[5n").await;
    assert_eq!(
        server.next().await,
        ClientMessage::Input(STATUS_ANSWER.to_vec())
    );
    server.output(17, b"0123456789\x1b[5n").await;
    let (position, _) = snapshot_request(&mut server, true).await;
    assert_eq!(
        position,
        Position {
            offset: 27,
            resizes: 0
        }
    );
    wait_until(&terminal, "the checkpoint", |terminal| {
        terminal.status().checkpoint == Some(position)
    })
    .await;
}

/// A read whose split candidate comes before enough output is not
/// snapshotted there, but at its end, once the threshold is passed.
///
/// Why: checkpoints are due only after enough output (the plan's D-8,
/// revised); checking the threshold at the read's end alone would encode
/// up to a whole read early. Spec: with checkpoints due after 10 bytes,
/// `abc`, a status request and `0123456789` (17 bytes, split candidate at
/// 3, before the request) bring a `Snapshot` at (17, 0), not at (3, 0).
#[farhelm_testtrace::test]
async fn a_split_before_the_threshold_waits_for_the_read_end() {
    let clock = ManualClock::new();
    let stand_in = StandIn::new();
    let (_terminal, mut server, _) = adopt(
        &stand_in,
        Adoption::Fresh,
        &info(0),
        checkpointing(10, &clock),
    )
    .await;
    server.output(0, b"abc\x1b[5n0123456789").await;
    let (position, _) = snapshot_request(&mut server, true).await;
    assert_eq!(
        position,
        Position {
            offset: 17,
            resizes: 0
        }
    );
}

/// A read that ends inside an open synchronized update is snapshotted at
/// its split candidate, where the emulator is clean, not at its end.
///
/// Why: agents draw in synchronized updates, so their reads seldom end
/// clean; the split candidate (just before the update's start) is where a
/// checkpoint can still be taken, and a snapshot of an open update could
/// not be continued from. Spec: with checkpoints due after 1 byte, `text`
/// followed by an update start and `partial` brings a `Snapshot` at
/// (4, 0) whose blob shows `text` and not `partial`.
#[farhelm_testtrace::test]
async fn a_checkpoint_is_taken_at_the_split_candidate() {
    let clock = ManualClock::new();
    let stand_in = StandIn::new();
    let (_terminal, mut server, _) = adopt(
        &stand_in,
        Adoption::Fresh,
        &info(0),
        checkpointing(1, &clock),
    )
    .await;
    server.output(0, b"text\x1b[?2026hpartial").await;
    let (position, blob) = snapshot_request(&mut server, true).await;
    assert_eq!(
        position,
        Position {
            offset: 4,
            resizes: 0
        }
    );
    let screen = restored_screen(&blob);
    assert!(
        screen.contains("text") && !screen.contains("partial"),
        "{screen:?}"
    );
}

/// No checkpoint is taken inside a sequence, however long, and one is
/// taken at the first clean boundary after it ends.
///
/// Why: a snapshot inside a sequence could not be continued from (a fresh
/// parser would misread the rest), so a sequence longer than the host's
/// cap leaves the host's tail growing until it ends; the first clean point
/// after it is where the tail must be cut back, or a later recovery would
/// find a gap for no reason. Spec: with checkpoints due after 1 byte, a
/// read holding a title's start and 5,000 bytes of it, then a read holding
/// the title's end, `done` and a status request, bring one snapshot, the
/// first: at the position after `done`, before the request (5,009 bytes
/// in), showing `done`.
#[farhelm_testtrace::test]
async fn a_checkpoint_follows_a_long_sequence_at_once() {
    let clock = ManualClock::new();
    let stand_in = StandIn::new();
    let (_terminal, mut server, _) = adopt(
        &stand_in,
        Adoption::Fresh,
        &info(0),
        checkpointing(1, &clock),
    )
    .await;
    let mut title = b"\x1b]2;".to_vec();
    title.extend(vec![b'x'; 5000]);
    server.output(0, &title).await;
    server.output(5004, b"\x07done\x1b[5n").await;
    let (position, blob) = snapshot_request(&mut server, true).await;
    assert_eq!(
        position,
        Position {
            offset: 5009,
            resizes: 0
        }
    );
    assert!(restored_screen(&blob).contains("done"));
}

/// A checkpoint the replayed output made due is taken where the replay
/// finishes, whether output or a resize ends it.
///
/// Why: a recovery may replay most of the host's cap; with no read after
/// it (a program that went quiet while no supervisor was attached), the
/// checkpoint is due nowhere else, and the host's tail would stay near its
/// cap until the program writes again. Spec: with checkpoints due after
/// 10 bytes, a recovered adoption whose 15 journaled bytes end the replay
/// sends a `Snapshot` at (15, 0), not at the split candidate of the live
/// read that follows (`x` and a status request, which would put it at 16);
/// one whose replay ends with a resize after its 15 bytes sends one at
/// (15, 1).
#[farhelm_testtrace::test]
async fn a_replay_ending_with_a_due_checkpoint_takes_it() {
    let clock = ManualClock::new();
    let stand_in = StandIn::new();
    let (_terminal, mut server, _) = adopt(
        &stand_in,
        Adoption::Recovered,
        &info(15),
        checkpointing(10, &clock),
    )
    .await;
    server.output(0, b"fifteen bytes..").await;
    server.output(15, b"x\x1b[5n").await;
    let (position, _) = snapshot_request(&mut server, true).await;
    assert_eq!(
        position,
        Position {
            offset: 15,
            resizes: 0
        }
    );

    let resized = Info {
        resizes: 1,
        ..info(15)
    };
    let stand_in = StandIn::new();
    let (_terminal, mut server, _) = adopt(
        &stand_in,
        Adoption::Recovered,
        &resized,
        checkpointing(10, &clock),
    )
    .await;
    server.output(0, b"fifteen bytes..").await;
    server
        .send(&HostMessage::Resized {
            offset: 15,
            index: 0,
            size: Size { cols: 90, rows: 20 },
        })
        .await;
    let (position, _) = snapshot_request(&mut server, true).await;
    assert_eq!(
        position,
        Position {
            offset: 15,
            resizes: 1
        }
    );
}

/// Time alone never makes a checkpoint due: a terminal that writes a
/// little and goes quiet keeps its tail.
///
/// Why: a time trigger re-encodes the whole scrollback of every terminal
/// that changed at all (a spinner, a clock) once per period, which made it
/// the largest steady cost of checkpoints, to spare a recovery a tail that
/// replays in milliseconds; checkpoints are due by output alone (the
/// plan's D-8, revised). Spec: with checkpoints due after 1 KiB, `quiet`
/// and a status request bring the answer and no snapshot; with the
/// runtime's clock then advanced by a day, two more status requests each
/// bring their answer as the next frame, never a snapshot.
#[farhelm_testtrace::test]
async fn time_alone_never_makes_a_checkpoint_due() {
    let clock = ManualClock::new();
    let stand_in = StandIn::new();
    let (_terminal, mut server, _) = adopt(
        &stand_in,
        Adoption::Fresh,
        &info(0),
        checkpointing(1024, &clock),
    )
    .await;
    server.output(0, b"quiet\x1b[5n").await;
    assert_eq!(
        server.next().await,
        ClientMessage::Input(STATUS_ANSWER.to_vec())
    );
    clock.advance(Duration::from_secs(24 * 3600));
    // Two requests: a timer the clock woke could lose the race with the
    // first, never with the second.
    for offset in [9, 13] {
        server.output(offset, b"\x1b[5n").await;
        assert_eq!(
            server.next().await,
            ClientMessage::Input(STATUS_ANSWER.to_vec())
        );
    }
}

/// A checkpoint for an orderly stop is taken where the terminal is when it
/// is clean there, and otherwise leaves the host its previous snapshot.
///
/// Why: an orderly stop (an update, the app replacing its supervisor) must
/// leave the shortest replay it can, but must never wait for a clean
/// boundary, which a program stopped mid-sequence may never reach. Spec:
/// after `ready`, `checkpoint_now` sends a `Snapshot` at (5, 0) and,
/// accepted, reports it; asked again with nothing new, it reports the
/// checkpoint current without sending anything; after an unfinished
/// sequence (`ESC [`), it reports `NotClean` without sending a snapshot
/// (the next frame is the answer to a status request); and with a deadline
/// the stand-in does not answer by, it reports `TimedOut`.
#[farhelm_testtrace::test]
async fn checkpoint_now_takes_a_clean_snapshot_or_keeps_the_previous() {
    let clock = ManualClock::new();
    let stand_in = StandIn::new();
    let (terminal, mut server, _) = adopt(
        &stand_in,
        Adoption::Fresh,
        &info(0),
        checkpointing(1 << 30, &clock),
    )
    .await;
    server.output(0, b"ready").await;
    wait_until(&terminal, "the output", screen_has("ready")).await;
    let deadline = || tokio::time::Instant::now() + WAIT;
    let (outcome, (position, _)) = tokio::join!(
        terminal.checkpoint_now(deadline()),
        snapshot_request(&mut server, true)
    );
    assert_eq!(
        position,
        Position {
            offset: 5,
            resizes: 0
        }
    );
    assert_eq!(outcome, Ok(CheckpointOutcome::Accepted(position)));
    assert_eq!(
        terminal.checkpoint_now(deadline()).await,
        Ok(CheckpointOutcome::Current(position))
    );

    server.output(5, b"\x1b[").await;
    wait_until(&terminal, "the open sequence", |terminal| {
        terminal.status().position.offset == 7
    })
    .await;
    assert_eq!(
        terminal.checkpoint_now(deadline()).await,
        Ok(CheckpointOutcome::Kept(NotTaken::NotClean))
    );
    server.output(7, b"m\x1b[5n").await;
    assert_eq!(
        server.next().await,
        ClientMessage::Input(STATUS_ANSWER.to_vec())
    );

    let soon = tokio::time::Instant::now() + Duration::from_millis(200);
    let outcome = terminal.checkpoint_now(soon).await;
    assert_eq!(outcome, Ok(CheckpointOutcome::Kept(NotTaken::TimedOut)));
    assert!(matches!(
        server.next().await,
        ClientMessage::Snapshot { .. }
    ));
}

/// A checkpoint already due by output is taken as soon as a synchronized
/// update's deadline draws the update, between reads.
///
/// Why: output that came in reads with no clean point (an update the
/// program left open) still fills the host's journal; once the update is
/// drawn, the checkpoint that output made due must follow, or the host's
/// tail grows until the program writes again, which a stalled program may
/// never do. Spec: with checkpoints due after 10 bytes, an update start and 20
/// letters (28 bytes, never clean) bring no snapshot; advancing the
/// runtime's clock by 150 ms draws the update, and a `Snapshot` at (28, 0)
/// follows at once.
#[farhelm_testtrace::test]
async fn a_checkpoint_due_by_output_follows_as_soon_as_the_terminal_is_clean() {
    let clock = ManualClock::new();
    let stand_in = StandIn::new();
    let (terminal, mut server, _) = adopt(
        &stand_in,
        Adoption::Fresh,
        &info(0),
        checkpointing(10, &clock),
    )
    .await;
    server.output(0, b"\x1b[?2026habcdefghijklmnopqrst").await;
    wait_until(&terminal, "the update's bytes", |terminal| {
        terminal.status().position.offset == 28
    })
    .await;
    clock.advance(Duration::from_millis(150));
    let (position, _) = snapshot_request(&mut server, true).await;
    assert_eq!(
        position,
        Position {
            offset: 28,
            resizes: 0
        }
    );
}

/// An orderly checkpoint keeps its deadline while the snapshot's encode
/// runs long, even on a current-thread runtime, and the views do not wait
/// for the encode; the encode, once done, goes to the host as an ordinary
/// checkpoint.
///
/// Why: encoding a full terminal takes seconds; run on an async worker it
/// would stall the stop that asked for it within a budget, and every view
/// read on the way (status sampling, titles) would wait for it too; a
/// `read` of the whole emulator cannot be answered until it is back, and
/// must neither block the runtime nor miss its return. Spec: on this
/// test's current-thread runtime, with every encode held (holding the
/// emulator) by a closed gate, `checkpoint_now` with a 300 ms deadline
/// reports `TimedOut` within 2 s; the emulator is then away; the screen
/// tail and the title come back at once (within 1 s) with `ready` and the
/// title; a `read` is pending; once the gate opens, the
/// `read` answers with `ready` and a `Snapshot` at (11, 0), the end of the
/// title and `ready`, reaches the stand-in.
#[farhelm_testtrace::test]
async fn an_orderly_checkpoint_keeps_its_deadline_while_the_encode_runs() {
    let clock = ManualClock::new();
    let gate = EncodeGate::closed();
    let options = HostTerminalOptions {
        encode_gate: Some(gate.clone()),
        ..checkpointing(1 << 30, &clock)
    };
    let stand_in = StandIn::new();
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info(0), options).await;
    server.output(0, b"\x1b]2;t\x07ready").await;
    wait_until(&terminal, "the output", screen_has("ready")).await;

    let asked = std::time::Instant::now();
    let outcome = terminal
        .checkpoint_now(tokio::time::Instant::now() + Duration::from_millis(300))
        .await;
    assert_eq!(outcome, Ok(CheckpointOutcome::Kept(NotTaken::TimedOut)));
    assert!(
        asked.elapsed() < Duration::from_secs(2),
        "{:?}",
        asked.elapsed()
    );

    assert!(emulator_away(&terminal), "the encode holds the emulator");
    let read = std::time::Instant::now();
    assert!(terminal.screen_tail(4096).contains("ready"));
    assert_eq!(terminal.title(), "t");
    assert!(
        read.elapsed() < Duration::from_secs(1),
        "{:?}",
        read.elapsed()
    );

    let reading = terminal.read(|emulator| emulator.screen_text());
    tokio::pin!(reading);
    assert!(
        pending_once(&mut reading).await,
        "a read answered while the emulator was away"
    );

    gate.open();
    let (screen, (position, _)) = tokio::join!(
        within("the read once the emulator is back", reading),
        snapshot_request(&mut server, true)
    );
    let screen = screen.expect("the emulator came back");
    assert!(screen.contains("ready"), "{screen:?}");
    assert_eq!(
        position,
        Position {
            offset: 11,
            resizes: 0
        }
    );
}

/// An automatic checkpoint's encode, started from a read, leaves the status
/// and the views answering, and an orderly checkpoint asked meanwhile keeps
/// its deadline, on a current-thread runtime.
///
/// Why: publishing a read's status after the encode started once waited
/// for the encode's hold on the emulator, which on a current-thread runtime
/// stalled everything for the whole encode, an orderly stop's deadline
/// included; that path is taken by every automatic checkpoint, not by the
/// orderly one. Spec: with checkpoints due after 10 bytes and every encode
/// held by a closed gate, `hello world!` (12 bytes) sends the emulator away
/// for an encode; the status then reports position (12, 0) and the screen
/// tail shows `hello world!`, at once (within 1 s), and `try_read` gives
/// nothing; `checkpoint_now` with a 300 ms deadline reports `TimedOut`
/// within 2 s; once the gate opens, a status change shows `try_read`
/// reading again, and a `Snapshot` at (12, 0) reaches the stand-in.
#[farhelm_testtrace::test]
async fn an_automatic_checkpoint_leaves_the_views_and_deadlines_answering() {
    let clock = ManualClock::new();
    let gate = EncodeGate::closed();
    let options = HostTerminalOptions {
        encode_gate: Some(gate.clone()),
        ..checkpointing(10, &clock)
    };
    let stand_in = StandIn::new();
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info(0), options).await;
    server.output(0, b"hello world!").await;
    wait_until(&terminal, "the encode to take the emulator", emulator_away).await;

    let read = std::time::Instant::now();
    assert_eq!(
        terminal.status().position,
        Position {
            offset: 12,
            resizes: 0
        }
    );
    assert!(terminal.screen_tail(4096).contains("hello world!"));
    assert!(
        read.elapsed() < Duration::from_secs(1),
        "{:?}",
        read.elapsed()
    );
    assert_eq!(terminal.try_read(|_| ()), None, "try_read while away");

    let asked = std::time::Instant::now();
    let outcome = terminal
        .checkpoint_now(tokio::time::Instant::now() + Duration::from_millis(300))
        .await;
    assert_eq!(outcome, Ok(CheckpointOutcome::Kept(NotTaken::TimedOut)));
    assert!(
        asked.elapsed() < Duration::from_secs(2),
        "{:?}",
        asked.elapsed()
    );

    gate.open();
    wait_until(&terminal, "the emulator to come back", |terminal| {
        terminal.try_read(|_| ()).is_some()
    })
    .await;
    let (position, _) = snapshot_request(&mut server, true).await;
    assert_eq!(
        position,
        Position {
            offset: 12,
            resizes: 0
        }
    );
}

/// An orderly checkpoint is refused while the runtime still replays, and
/// reports the host's rejection.
///
/// Why: a snapshot during the replay would capture a state the host's
/// journal is still ahead of, and a caller deciding whether a stop may
/// proceed must learn that the host did not take a snapshot. Spec: a
/// recovered adoption with 5 bytes journaled and none fed reports
/// `Replaying`; once fed, with the stand-in rejecting the snapshot as
/// outside its retained output, it reports `Rejected(OutsideRetained)`.
#[farhelm_testtrace::test]
async fn an_orderly_checkpoint_reports_replay_and_rejection() {
    let stand_in = StandIn::new();
    let (terminal, mut server, _) =
        adopt(&stand_in, Adoption::Recovered, &info(5), options()).await;
    let deadline = || tokio::time::Instant::now() + WAIT;
    assert_eq!(
        terminal.checkpoint_now(deadline()).await,
        Ok(CheckpointOutcome::Kept(NotTaken::Replaying))
    );
    server.output(0, b"hello").await;
    wait_until(&terminal, "the replay", |terminal| {
        terminal.status().caught_up
    })
    .await;
    let (outcome, _) = tokio::join!(
        terminal.checkpoint_now(deadline()),
        snapshot_request(&mut server, false)
    );
    assert_eq!(
        outcome,
        Ok(CheckpointOutcome::Kept(NotTaken::Rejected(
            SnapshotRejection::OutsideRetained
        )))
    );
}

// ---- Gaps ------------------------------------------------------------------------

/// A gap keeps the emulator, marks the history trimmed, and asks the
/// program to redraw with a resize one row smaller and one back.
///
/// Why: the bytes in a gap were never interpreted, so the screen and modes
/// may be stale until the program draws again; a resize is the one thing
/// that makes every program redraw, and a resize to the size in force
/// sends no `SIGWINCH` (checked on Linux 6.8: none for the same size, one
/// for a different size and one for the change back; macOS was not
/// checked, though the BSD tty code it derives from compares sizes the
/// same way), so it takes two. Spec: `abc`, a gap from 3 to 10
/// at 80x24, and `def` at 10 show `abcdef` at (13, 0) with the history
/// marked trimmed, in the status and as read with the emulator, which was
/// not rebuilt (a viewer reading the emulator learns of the gap too, and
/// keeps its history); the stand-in receives a resize to 80x23 and then one to
/// 80x24.
#[farhelm_testtrace::test]
async fn a_gap_keeps_the_emulator_and_asks_for_a_redraw() {
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
    assert!(terminal.status().history_trimmed);
    assert_eq!(
        terminal.try_read(|emulator| (emulator.history_trimmed(), emulator.rebuilds())),
        Some((true, 0)),
        "the gap's mark is read with the emulator, which was not rebuilt"
    );
    assert_eq!(
        server.next().await,
        ClientMessage::Resize(Size { cols: 80, rows: 23 })
    );
    assert_eq!(
        server.next().await,
        ClientMessage::Resize(Size { cols: 80, rows: 24 })
    );
}

/// A recovery across a gap rebuilds from the snapshot and the retained
/// tail, and asks for the redraw only once the replay is over.
///
/// Why: this is the recovery after output overflowed the host's journal
/// while no supervisor was attached: what the snapshot held and what the
/// host kept after the gap are all there is, and a redraw requested in the
/// middle of the replay would be answered before the program's later
/// output was even fed. Spec: a host holding a snapshot of `seeded ` at
/// (7, 0), a gap from 7 to 50, `tail` at 50 and a resize to 100x30 at 54
/// (54 bytes and one resize journaled) shows `seeded tail` with the history
/// marked trimmed, caught up, and receives the two redraw resizes at the
/// size the replay ended at, 100x29 and 100x30, which a redraw asked for
/// before the replay's end would not have (80x23 and 80x24).
#[farhelm_testtrace::test]
async fn a_recovery_across_a_gap_rebuilds_and_then_asks_for_a_redraw() {
    let snapshot = match seed_record(b"seeded ") {
        HostMessage::SnapshotRecord { blob, .. } => HostMessage::SnapshotRecord {
            position: Position {
                offset: 7,
                resizes: 0,
            },
            blob,
        },
        other => unreachable!("{other:?}"),
    };
    let journaled = Info {
        snapshot: Position {
            offset: 7,
            resizes: 0,
        },
        tail_start: 50,
        gap: true,
        resizes: 1,
        ..info(54)
    };
    let stand_in = StandIn::new();
    let (terminal, mut server, _) = adopt_with(
        &stand_in,
        Adoption::Recovered,
        &journaled,
        options(),
        &[snapshot],
    )
    .await;
    let terminal = terminal.expect("the adoption succeeds");
    assert_eq!(
        terminal.status().checkpoint,
        Some(Position {
            offset: 7,
            resizes: 0
        })
    );
    server
        .send(&HostMessage::Gap {
            from: 7,
            to: 50,
            resizes: 0,
            size: Size { cols: 80, rows: 24 },
        })
        .await;
    server.output(50, b"tail").await;
    server
        .send(&HostMessage::Resized {
            offset: 54,
            index: 0,
            size: Size {
                cols: 100,
                rows: 30,
            },
        })
        .await;
    wait_until(&terminal, "the recovery", |terminal| {
        terminal.status().caught_up
    })
    .await;
    assert!(terminal.screen_tail(4096).contains("seeded tail"));
    assert!(terminal.status().history_trimmed);
    assert_eq!(
        server.next().await,
        ClientMessage::Resize(Size {
            cols: 100,
            rows: 29
        })
    );
    assert_eq!(
        server.next().await,
        ClientMessage::Resize(Size {
            cols: 100,
            rows: 30
        })
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
        checkpoint: None,
        history_trimmed: false,
        modes: Emulator::new(
            farhelm_terminal_emulator::Size {
                columns: 80,
                lines: 24,
            },
            0,
        )
        .modes(),
        size: Size { cols: 80, rows: 24 },
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

/// A paste goes out as consecutive `Input`s whose receipt says written only
/// once every one of them was written; one that a lost connection cuts
/// short is closed first on the new connection.
///
/// Why: prompt delivery submits only on a written paste, so the receipt
/// must not resolve on the first piece's acknowledgement; and a program
/// that read a bracketed paste's opening marker reads everything after as
/// pasted text until a closing one arrives. Spec: an unbracketed paste of
/// `2 * MAX_INPUT + 10` bytes arrives as three `Input`s that join to it,
/// and its receipt is still pending after the first is acknowledged and
/// `Written` after all are; a bracketed paste of three `Input`s whose first
/// was acknowledged and second was in flight when the connection dropped is
/// `Unknown`, and the new connection's first input is `ESC [201~`. An empty
/// paste and one over `MAX_PASTE` are refused.
#[farhelm_testtrace::test]
async fn a_paste_goes_out_whole_and_is_closed_after_a_loss() {
    let stand_in = StandIn::new();
    let info = info(0);
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info, options()).await;
    let sequence = terminal.input_sequence();

    let data: Vec<u8> = (0..2 * MAX_INPUT + 10).map(|at| (at % 251) as u8).collect();
    let receipt = within("queueing a paste", sequence.send_paste(data.clone(), false))
        .await
        .unwrap();
    let mut pieces = Vec::new();
    for _ in 0..3 {
        pieces.push(server.input().await);
    }
    assert_eq!(pieces.concat(), data);
    server.ack(MAX_INPUT as u64).await;
    let mut written = Box::pin(receipt.fate());
    assert!(pending_once(&mut written).await, "written after one piece");
    server.ack(data.len() as u64).await;
    assert_eq!(
        within("the paste's fate", written).await,
        InputFate::Written
    );

    let mut paste = crate::grid_input::PASTE_START.to_vec();
    paste.resize(2 * MAX_INPUT + 10, b'p');
    paste.extend_from_slice(PASTE_END);
    let receipt = within("queueing a paste", sequence.send_paste(paste.clone(), true))
        .await
        .unwrap();
    let first = server.input().await;
    assert_eq!(first, paste[..MAX_INPUT]);
    server.ack((data.len() + MAX_INPUT) as u64).await;
    let _second = server.input().await;
    drop(server);
    let (mut server, _) = stand_in.accept(&info).await;
    assert_eq!(server.input().await, PASTE_END);
    assert_eq!(fate(receipt).await, InputFate::Unknown);

    assert_eq!(
        sequence.send_paste(Vec::new(), false).await.unwrap_err(),
        InputRefused::Invalid
    );
    assert_eq!(
        sequence
            .send_paste(vec![b'p'; MAX_PASTE + 1], false)
            .await
            .unwrap_err(),
        InputRefused::Invalid
    );
}

/// A closing marker is owed for a paste a loss may have cut short even
/// when an earlier paste in the window was acknowledged, and stays owed
/// until the host acknowledges the marker itself.
///
/// Why: the window can hold several pastes, and acknowledging one says
/// nothing of the next; and a marker lost with a second connection leaves
/// the program inside the paste as surely as the first loss did. Either
/// way the next answer, or the next key, would be read as pasted text.
/// Spec: bracketed pastes A and B both go out; A is acknowledged and
/// written, and the connection drops with B unacknowledged (`Unknown`). On
/// the second connection the program asks DA1: the runtime sends `ESC
/// [201~` first and the answer after it. That connection drops with
/// neither acknowledged; on the third, DA1 asked again gets `ESC [201~`
/// first and the answer after it. Once both are acknowledged, a loss owes
/// nothing: on the fourth connection the answer to DA1 is the first input.
#[farhelm_testtrace::test]
async fn a_closing_marker_stays_owed_until_acknowledged() {
    let stand_in = StandIn::new();
    let info = info(0);
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info, options()).await;
    let sequence = terminal.input_sequence();
    let a = b"\x1b[200~A\x1b[201~".to_vec();
    let b = b"\x1b[200~B\x1b[201~".to_vec();
    let a_receipt = within("queueing A", sequence.send_paste(a.clone(), true))
        .await
        .unwrap();
    let b_receipt = within("queueing B", sequence.send_paste(b.clone(), true))
        .await
        .unwrap();
    assert_eq!(server.input().await, a);
    assert_eq!(server.input().await, b);
    server.ack(a.len() as u64).await;
    assert_eq!(fate(a_receipt).await, InputFate::Written);

    drop(server);
    let (mut server, _) = stand_in.accept(&info).await;
    assert_eq!(fate(b_receipt).await, InputFate::Unknown);
    server.output(0, b"\x1b[c").await;
    assert_eq!(server.input().await, PASTE_END);
    assert_eq!(server.input().await, DA1_ANSWER);

    drop(server);
    let (mut server, _) = stand_in.accept(&info).await;
    server.output(3, b"\x1b[c").await;
    assert_eq!(server.input().await, PASTE_END);
    assert_eq!(server.input().await, DA1_ANSWER);
    server
        .ack((PASTE_END.len() + DA1_ANSWER.len()) as u64)
        .await;
    // Output after the acknowledgement, processed: so is the
    // acknowledgement, which arrived before it.
    server.output(6, b"x").await;
    wait_until(&terminal, "x", screen_has("x")).await;

    drop(server);
    let (mut server, _) = stand_in.accept(&info).await;
    server.output(7, b"\x1b[c").await;
    assert_eq!(server.input().await, DA1_ANSWER);
}

/// Pastes waiting for the runtime share one budget per terminal: a paste
/// that does not fit in what is left waits until an earlier one has gone
/// out.
///
/// Why: the runtime's queue is bounded by requests, not bytes, and a
/// request can be a whole paste; pastes that attachments handed over and
/// then ended would otherwise pile up, up to sixteen of them per terminal.
/// Spec: with a paste of `MAX_PASTE` bytes taken and none of it
/// acknowledged, a second paste of one byte waits to be taken; once the
/// host has read and acknowledged every piece of the first, the second is
/// taken and goes out next, and the first is `Written`.
#[farhelm_testtrace::test]
async fn pastes_wait_for_the_terminal_paste_budget() {
    let stand_in = StandIn::new();
    let info = info(0);
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info, options()).await;
    let sequence = terminal.input_sequence();
    let first = within(
        "queueing the first paste",
        sequence.send_paste(vec![b'p'; MAX_PASTE], false),
    )
    .await
    .unwrap();
    let mut second = Box::pin(sequence.send_paste(b"q".to_vec(), false));
    assert!(
        pending_once(&mut second).await,
        "the second paste waits for the budget"
    );
    let mut written = 0;
    while written < MAX_PASTE {
        written += server.input().await.len();
        server.ack(written as u64).await;
    }
    let second = within("the second paste", second).await.unwrap();
    assert_eq!(server.input().await, b"q");
    server.ack((MAX_PASTE + 1) as u64).await;
    assert_eq!(fate(first).await, InputFate::Written);
    assert_eq!(fate(second).await, InputFate::Written);
}

/// An aborted runtime gives back the paste budget its queue held, and a
/// paste still waiting for the budget is refused as ended.
///
/// Why: a supervisor's orderly stop and Delete abort runtimes
/// (`HostTerminal::abort`); a paste that went on waiting for budget that
/// nothing would ever give back would hold its caller (a viewer's input
/// pump, a prompt delivery) until its own timeout. Spec: with a paste of
/// `MAX_PASTE` bytes taken and none of it acknowledged, and a second paste
/// waiting for the budget, aborting the runtime makes the second refused
/// as `Ended` and the first's fate `Unknown`.
#[farhelm_testtrace::test]
async fn an_aborted_runtime_refuses_a_paste_waiting_for_the_budget() {
    let stand_in = StandIn::new();
    let info = info(0);
    let (terminal, _server, _) = adopt(&stand_in, Adoption::Fresh, &info, options()).await;
    let sequence = terminal.input_sequence();
    let first = within(
        "queueing the first paste",
        sequence.send_paste(vec![b'p'; MAX_PASTE], false),
    )
    .await
    .unwrap();
    let mut second = Box::pin(sequence.send_paste(b"q".to_vec(), false));
    assert!(
        pending_once(&mut second).await,
        "premise: the second paste waits for the budget"
    );
    terminal.abort();
    assert_eq!(
        within("the waiting paste", second).await.unwrap_err(),
        InputRefused::Ended
    );
    assert_eq!(fate(first).await, InputFate::Unknown);
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

/// A resize to the size the host already has is not sent.
///
/// Why: every viewer asks for its size when it attaches, usually the size
/// in force; a resize record of an unchanged size would still signal the
/// program, which redraws, and would reset a grid viewer's history for
/// nothing (the grid stream format's "Resize"). Spec: on a host at 80x24,
/// asking for 80x24 and then sending `fence` puts `fence` on the wire with
/// no `Resize` before it, and `fence2`, sent once `fence` arrived, is next;
/// asking for 90x20 then sends that resize.
#[farhelm_testtrace::test]
async fn a_resize_to_the_size_in_force_is_not_sent() {
    let stand_in = StandIn::new();
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info(0), options()).await;
    terminal
        .resize(Size { cols: 80, rows: 24 })
        .expect("a valid size");
    let _fence = send(&terminal, b"fence").await;
    assert_eq!(server.next().await, ClientMessage::Input(b"fence".to_vec()));
    let _fence = send(&terminal, b"fence2").await;
    assert_eq!(
        server.next().await,
        ClientMessage::Input(b"fence2".to_vec())
    );
    let size = Size { cols: 90, rows: 20 };
    terminal.resize(size).expect("a valid size");
    assert_eq!(server.next().await, ClientMessage::Resize(size));
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

/// An encoder that panics hands the emulator back: an orderly stop hears
/// that the encode failed, and the terminal carries on.
///
/// Why: an encode runs the emulator through code a malformed state could
/// make panic; losing a healthy terminal to it would be worse than missing
/// one checkpoint, and an orderly stop told "not clean" would be told
/// something false about the terminal. Spec: with every encoder panicking,
/// after `ready`, `checkpoint_now` reports `EncodeFailed`; the emulator is
/// home (`try_read` shows `ready`), and a status request after it is still
/// answered.
#[farhelm_testtrace::test]
async fn an_encoder_panic_keeps_the_emulator_and_is_reported() {
    let clock = ManualClock::new();
    let gate = EncodeGate::closed();
    gate.fail_with(EncodeFault::EncoderPanics);
    gate.open();
    let options = HostTerminalOptions {
        encode_gate: Some(gate),
        ..checkpointing(1 << 30, &clock)
    };
    let stand_in = StandIn::new();
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info(0), options).await;
    server.output(0, b"ready").await;
    wait_until(&terminal, "the output", screen_has("ready")).await;
    assert_eq!(
        terminal
            .checkpoint_now(tokio::time::Instant::now() + WAIT)
            .await,
        Ok(CheckpointOutcome::Kept(NotTaken::EncodeFailed))
    );
    assert!(
        terminal
            .try_read(|emulator| emulator.screen_text())
            .is_some_and(|screen| screen.contains("ready"))
    );
    server.output(5, b"\x1b[5n").await;
    assert_eq!(
        server.next().await,
        ClientMessage::Input(STATUS_ANSWER.to_vec())
    );
}

/// An encode whose task is lost takes the emulator with it: the runtime
/// ends, unavailable, and a `read` waiting for the emulator is released
/// with an error.
///
/// Why: with the emulator gone, every later feed would have nothing to
/// feed; carrying on would crash the task or guess at the terminal's state,
/// and a reader waiting for the emulator's return would wait forever.
/// Spec: with checkpoints due after 10 bytes and the encode's task failing
/// outside the encoder once the gate opens, `hello world!` sends the
/// emulator away; a `read` is pending; once the gate opens, the read fails
/// with `EmulatorLost`, the terminal becomes unavailable as
/// `EmulatorLost`, the screen tail still shows `hello world!` and
/// `try_read` reads nothing.
#[farhelm_testtrace::test]
async fn a_lost_encode_ends_the_runtime_and_releases_readers() {
    let clock = ManualClock::new();
    let gate = EncodeGate::closed();
    gate.fail_with(EncodeFault::TaskLost);
    let options = HostTerminalOptions {
        encode_gate: Some(gate.clone()),
        ..checkpointing(10, &clock)
    };
    let stand_in = StandIn::new();
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info(0), options).await;
    server.output(0, b"hello world!").await;
    wait_until(&terminal, "the encode to take the emulator", emulator_away).await;
    let reading = terminal.read(|emulator| emulator.screen_text());
    tokio::pin!(reading);
    assert!(
        pending_once(&mut reading).await,
        "a read answered while away"
    );

    gate.open();
    assert_eq!(
        within("the read to be released", reading).await,
        Err(EmulatorLost)
    );
    wait_until(&terminal, "the runtime to end", |terminal| {
        matches!(
            terminal.liveness(),
            Liveness::Unavailable {
                reason: Unavailability::EmulatorLost,
                ..
            }
        )
    })
    .await;
    assert!(terminal.screen_tail(4096).contains("hello world!"));
    assert_eq!(terminal.try_read(|_| ()), None);
    drop(server);
}

/// A shutdown asked for while an encode holds the emulator and a `read`
/// waits for it brings the emulator home first: the read is answered and
/// the shutdown goes ahead.
///
/// Why: a runtime that ended with its emulator away would leave every
/// waiting reader (a Restart taking its seed, a viewer) waiting for a
/// return that never comes. Spec: with checkpoints due after 10 bytes and
/// every encode held by a closed gate, `hello world!` sends the emulator
/// away; a `read` is pending; a shutdown is asked for; once the gate opens,
/// the stand-in gets the checkpoint (accepted) and then `Shutdown`
/// (answered with `Goodbye`), the read answers with `hello world!`, and the
/// shutdown reports the goodbye.
#[farhelm_testtrace::test]
async fn a_shutdown_during_an_encode_answers_the_waiting_read() {
    let clock = ManualClock::new();
    let gate = EncodeGate::closed();
    let mut options = HostTerminalOptions {
        encode_gate: Some(gate.clone()),
        ..checkpointing(10, &clock)
    };
    options.deadlines.shutdown = Duration::from_millis(300);
    let stand_in = StandIn::new();
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info(0), options).await;
    server.output(0, b"hello world!").await;
    wait_until(&terminal, "the encode to take the emulator", emulator_away).await;
    let reading = terminal.read(|emulator| emulator.screen_text());
    tokio::pin!(reading);
    assert!(
        pending_once(&mut reading).await,
        "a read answered while away"
    );

    let shutdown = async { within("the shutdown", terminal.shutdown()).await };
    let host = async {
        gate.open();
        snapshot_request(&mut server, true).await;
        assert_eq!(server.next().await, ClientMessage::Shutdown);
        server.send(&HostMessage::Goodbye).await;
    };
    let (report, screen, ()) = tokio::join!(
        shutdown,
        within("the read once the emulator is back", reading),
        host
    );
    assert!(
        screen
            .expect("the emulator came back")
            .contains("hello world!")
    );
    assert!(report.expect("the runtime shuts down").goodbye);
}

/// Notices are queued only for subscriptions open when they were sent, and
/// a subscription ends with the runtime, even one taken after it ended.
///
/// Why: a clipboard write can carry megabytes; kept for nobody, the last
/// few would stay in the supervisor for the terminal's whole life. And a
/// viewer must learn that no more notices will come rather than wait for
/// them. Spec: with no subscription, a clipboard write leaves nothing
/// queued; a subscription taken before the runtime's shutdown, and one
/// taken after, both read `None`.
#[farhelm_testtrace::test]
async fn notices_are_kept_for_nobody_and_end_with_the_runtime() {
    let mut options = options();
    options.deadlines.shutdown = Duration::from_millis(300);
    let stand_in = StandIn::new();
    let (terminal, mut server, _) = adopt(&stand_in, Adoption::Fresh, &info(0), options).await;
    let write = b"\x1b]52;c;bjA=\x07";
    server.output(0, write).await;
    wait_until(&terminal, "the clipboard write", |terminal| {
        terminal.status().position.offset == write.len() as u64
    })
    .await;
    let queued = terminal
        .inner
        .notices
        .upgrade()
        .expect("the runtime runs")
        .len();
    assert_eq!(queued, 0, "a notice kept for nobody");

    let mut before = terminal.subscribe_notices();
    let host = async {
        assert_eq!(server.next().await, ClientMessage::Shutdown);
        server.send(&HostMessage::Goodbye).await;
    };
    let (report, ()) = tokio::join!(within("the shutdown", terminal.shutdown()), host);
    assert!(report.expect("the runtime shuts down").goodbye);
    assert_eq!(within("the end of the notices", before.recv()).await, None);
    let mut after = terminal.subscribe_notices();
    assert_eq!(within("an ended subscription", after.recv()).await, None);
}
