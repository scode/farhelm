//! The client against stand-in hosts: listeners in this test process that
//! answer exactly as each case needs, which a real host never would (busy
//! to a take-over, a version it cannot have, a broken frame, silence). The
//! client's own bookkeeping (the input window, replies matched after a
//! dropped request, records kept while waiting for a reply, the event
//! queue's bound, the write deadline) is checked the same way, frame by
//! frame. Real hosts, started from the built `farhelm`, are in
//! `crates/farhelm/tests/terminal_host_client.rs`.
//!
//! No stand-in forks: they are tasks and sockets in this process, so these
//! run as ordinary unit tests. A stand-in reads what the client sends while
//! the client sends it, never after: a macOS unix socket buffers about 8 KiB
//! each way, so a client writing a 64 KiB frame to a stand-in that is not
//! reading would wait out its deadline.

use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use terminal_host::protocol::{MAX_OUTPUT_CHUNK, decode_client, encode_host};
use tokio::net::UnixListener;

/// How long any one wait on a stand-in may take before the test fails.
const WAIT: Duration = Duration::from_secs(10);

/// Short deadlines, so every outcome arrives in about a second.
fn deadlines() -> ClientDeadlines {
    ClientDeadlines {
        connect: Duration::from_secs(1),
        refusal_window: Duration::from_secs(1),
        retry_interval: Duration::from_millis(50),
        reply: Duration::from_secs(5),
        shutdown: Duration::from_secs(5),
    }
}

/// A listener at a fresh socket path.
fn bind() -> (tempfile::TempDir, PathBuf, UnixListener) {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("h");
    let listener = UnixListener::bind(&socket).unwrap();
    (dir, socket, listener)
}

/// The next client frame on `stream`, with `buf` holding what was read
/// past it.
async fn read_frame(stream: &mut UnixStream, buf: &mut Vec<u8>) -> ClientMessage {
    tokio::time::timeout(WAIT, async {
        loop {
            if let Some(frame) = split_frame(buf, MAX_FRAME).expect("a well-formed frame") {
                let consumed = frame.consumed;
                let message = decode_client(frame, 1).expect("a well-formed message");
                buf.drain(..consumed);
                return message;
            }
            assert_ne!(
                stream.read_buf(buf).await.unwrap(),
                0,
                "the client closed the connection"
            );
        }
    })
    .await
    .expect("a frame from the client in time")
}

/// The next client frame on `stream`, reading exactly its bytes and none
/// past it, so that what follows stays unread in the socket.
async fn read_one_frame(stream: &mut UnixStream) -> ClientMessage {
    tokio::time::timeout(WAIT, async {
        let mut length = [0u8; 4];
        stream.read_exact(&mut length).await.unwrap();
        let mut frame = length.to_vec();
        frame.resize(4 + u32::from_be_bytes(length) as usize, 0);
        stream.read_exact(&mut frame[4..]).await.unwrap();
        let raw = split_frame(&frame, MAX_FRAME)
            .expect("a well-formed frame")
            .expect("a whole frame");
        decode_client(raw, 1).expect("a well-formed message")
    })
    .await
    .expect("a frame from the client in time")
}

async fn send(stream: &mut UnixStream, message: &HostMessage) {
    stream.write_all(&encode_host(message)).await.unwrap();
}

/// An `Info` reply whose `host_pid` tells it apart.
fn info_numbered(host_pid: u32) -> Info {
    let size = Size { cols: 80, rows: 24 };
    Info {
        host_pid,
        child_pid: 0,
        child_exit: None,
        size,
        initial_size: size,
        snapshot: Position::default(),
        tail_start: 0,
        end: 0,
        resizes: 0,
        gap: false,
        delivered_end: 0,
        tag: Vec::new(),
    }
}

/// A connected client and the stand-in host's side of its connection.
struct Pair {
    _dir: tempfile::TempDir,
    client: HostConnection,
    server: UnixStream,
    /// Bytes the stand-in read past the last frame it decoded.
    buf: Vec<u8>,
}

impl Pair {
    async fn next_frame(&mut self) -> ClientMessage {
        read_frame(&mut self.server, &mut self.buf).await
    }
}

/// The client's next event, failing the test if none comes in time.
async fn next(client: &mut HostConnection) -> Result<HostEvent, ClientError> {
    tokio::time::timeout(WAIT, client.next_event())
        .await
        .expect("an event or an error in time")
}

/// A client connected to a stand-in that checked its `Hello` and said
/// `Welcome`, with `deadlines`.
async fn connected_with(deadlines: ClientDeadlines) -> Pair {
    let (dir, socket, listener) = bind();
    let host = async {
        let (mut server, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        assert_eq!(
            read_frame(&mut server, &mut buf).await,
            ClientMessage::Hello {
                version: 1,
                take_over: true
            }
        );
        send(&mut server, &HostMessage::Welcome { version: 1 }).await;
        (server, buf)
    };
    let (client, (server, buf)) =
        tokio::join!(HostConnection::connect_with(&socket, deadlines), host);
    Pair {
        _dir: dir,
        client: client.expect("the stand-in welcomed the client"),
        server,
        buf,
    }
}

async fn connected() -> Pair {
    connected_with(deadlines()).await
}

/// A stand-in that accepts every connection, reads its `Hello`, writes
/// `answer` (if any) and holds the connection open, counting connections.
fn answering(
    listener: UnixListener,
    answer: Vec<u8>,
) -> (Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
    let attempts = Arc::new(AtomicUsize::new(0));
    let counted = attempts.clone();
    let task = tokio::spawn(async move {
        let mut held = Vec::new();
        loop {
            let (mut server, _) = listener.accept().await.unwrap();
            counted.fetch_add(1, Ordering::SeqCst);
            let mut buf = Vec::new();
            assert_eq!(
                read_frame(&mut server, &mut buf).await,
                ClientMessage::Hello {
                    version: 1,
                    take_over: true
                }
            );
            if !answer.is_empty() {
                server.write_all(&answer).await.unwrap();
            }
            held.push(server);
        }
    });
    (attempts, task)
}

// ---- Outcome classes at connect ---------------------------------------------------

/// A missing socket file is gone without waiting for retries.
///
/// Why: a host removes its socket as it ends, so a missing file is the one
/// answer that needs no retry; waiting on it would slow every startup scan
/// over ended hosts. Spec: with a 5 s connect deadline and refusal window,
/// connecting to a path with no file (and no process serving it) gives
/// `Gone(NoSocket)` in under 3 s; the margin is for the process-table
/// search that confirms nothing serves the path.
#[farhelm_testtrace::test]
async fn a_missing_socket_is_gone_without_retries() {
    let dir = tempfile::tempdir().unwrap();
    let mut long = deadlines();
    long.connect = Duration::from_secs(5);
    long.refusal_window = Duration::from_secs(5);
    let started = Instant::now();
    let result = HostConnection::connect_with(&dir.path().join("h"), long).await;
    assert_eq!(result.unwrap_err(), ConnectError::Gone(Gone::NoSocket));
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
}

/// A socket that only refuses is gone, but only once refusals have lasted
/// the refusal window, even when that runs past the connect deadline.
///
/// Why: on macOS a live host with a full listen backlog refuses as well, so
/// one refusal is not evidence; and a socket left by a killed host must
/// still be judged gone, never left as unavailable because the connect
/// deadline came first. Spec: with a stale socket file (its listener
/// closed), a 300 ms connect deadline and a 1 s refusal window, the result
/// is `Gone(Refused)`, no sooner than 1 s.
#[farhelm_testtrace::test]
async fn a_refusing_socket_is_gone_only_after_the_refusal_window() {
    let (_dir, socket, listener) = bind();
    drop(listener);
    assert!(socket.exists(), "premise: the stale socket file stays");
    let mut short = deadlines();
    short.connect = Duration::from_millis(300);
    let started = Instant::now();
    let result = HostConnection::connect_with(&socket, short).await;
    assert_eq!(result.unwrap_err(), ConnectError::Gone(Gone::Refused));
    assert!(
        started.elapsed() >= Duration::from_secs(1),
        "{:?}",
        started.elapsed()
    );
}

/// Anything other than a refusal starts the refusal window over.
///
/// Why: the window stands for "nothing has listened for a while"; a socket
/// that answered in between was alive then, and a window that kept its old
/// start would judge a host gone moments after it last answered. Spec: a
/// stale socket refuses until a stand-in binds it, accepts one connection
/// and closes it without a word, and then goes away again (its file
/// staying); the result is `Gone(Refused)`, and no sooner than the 1 s
/// window after that one connection was served.
#[farhelm_testtrace::test]
async fn a_non_refusal_starts_the_refusal_window_over() {
    let (_dir, socket, listener) = bind();
    drop(listener);
    let mut long = deadlines();
    long.connect = Duration::from_secs(5);
    let path = socket.clone();
    let stand_in = tokio::spawn(async move {
        // sleep-ok: let the client's first refusals start the window before the one answer.
        tokio::time::sleep(Duration::from_millis(300)).await;
        std::fs::remove_file(&path).unwrap();
        let listener = UnixListener::bind(&path).unwrap();
        let (server, _) = tokio::time::timeout(WAIT, listener.accept())
            .await
            .expect("the client connected while the stand-in listened")
            .unwrap();
        drop(server);
        drop(listener);
        Instant::now()
    });
    let result = HostConnection::connect_with(&socket, long).await;
    let ended = Instant::now();
    let served = stand_in.await.unwrap();
    assert_eq!(result.unwrap_err(), ConnectError::Gone(Gone::Refused));
    assert!(
        ended - served >= Duration::from_secs(1),
        "judged gone {:?} after the last answer",
        ended - served
    );
}

/// A host that answers `Busy` is retried until the deadline, and is then
/// reported busy, never gone.
///
/// Why: a busy host is alive, and reading it as gone would let a caller
/// remove its socket or record its terminal as ended while it runs. Spec:
/// with a stand-in that answers every `Hello` with `Busy`, the client says
/// `Hello` (version 1, take-over) on two or more connections and the result
/// is `Busy` naming this process as the peer.
#[farhelm_testtrace::test]
async fn a_busy_host_is_retried_and_never_gone() {
    let (_dir, socket, listener) = bind();
    let (attempts, host) = answering(listener, encode_host(&HostMessage::Busy));
    let result = HostConnection::connect_with(&socket, deadlines()).await;
    host.abort();
    match result {
        Err(ConnectError::Busy { peer: Some(peer) }) => {
            assert_eq!(peer.pid, std::process::id());
        }
        other => panic!("expected Busy, got {other:?}"),
    }
    assert!(attempts.load(Ordering::SeqCst) >= 2, "it was not retried");
}

/// Says whether an unavailable reason is the one a case expects.
type ReasonCheck = fn(&Unavailable) -> bool;

/// Answers that no retry can change make the host unavailable, at once.
///
/// Why: a host this supervisor cannot speak to may still be running a
/// terminal, so it must be neither used nor taken for gone; and retrying
/// it would only slow every operation that meets it. Spec: a `Welcome` for
/// version 2, an `Error` (unsupported version) before `Welcome`, an empty
/// frame, and an `InputAck` in place of `Welcome` each give `Unavailable`
/// with the matching reason (unsupported version 2, refused, malformed,
/// malformed), naming this process as the peer, after exactly one
/// connection.
#[farhelm_testtrace::test]
async fn incompatible_answers_are_unavailable() {
    let cases: Vec<(Vec<u8>, ReasonCheck)> = vec![
        (encode_host(&HostMessage::Welcome { version: 2 }), |r| {
            *r == Unavailable::UnsupportedVersion(2)
        }),
        (
            encode_host(&HostMessage::Error {
                code: ErrorCode::UnsupportedVersion,
                message: "no".into(),
            }),
            |r| {
                matches!(
                    r,
                    Unavailable::Refused {
                        code: ErrorCode::UnsupportedVersion,
                        ..
                    }
                )
            },
        ),
        (vec![0, 0, 0, 0], |r| matches!(r, Unavailable::Malformed(_))),
        (
            encode_host(&HostMessage::InputAck {
                written: 0,
                discarded: 0,
            }),
            |r| matches!(r, Unavailable::Malformed(_)),
        ),
    ];
    for (answer, expected) in cases {
        let (_dir, socket, listener) = bind();
        let (attempts, host) = answering(listener, answer.clone());
        let result = HostConnection::connect_with(&socket, deadlines()).await;
        host.abort();
        match result {
            Err(ConnectError::Unavailable {
                reason,
                peer: Some(peer),
            }) => {
                assert!(expected(&reason), "{answer:?} gave {reason:?}");
                assert_eq!(peer.pid, std::process::id());
            }
            other => panic!("{answer:?}: expected Unavailable, got {other:?}"),
        }
        assert_eq!(attempts.load(Ordering::SeqCst), 1, "{answer:?} was retried");
    }
}

/// A host that accepts but never answers is unavailable at the deadline,
/// naming the listening process.
///
/// Why: a stopped or wedged host looks exactly like this, and the peer is
/// what the caller then hands to the verified termination. Spec: with a
/// stand-in that accepts and stays silent, the result is
/// `Unavailable(Timeout)` with this process as the peer, within the
/// deadline plus a margin. (Whether that peer may be signalled is
/// `process::terminate_verified`'s to judge; its own tests show a process
/// whose command line does not name the socket is never signalled, without
/// aiming a real signal at this test process.)
#[farhelm_testtrace::test]
async fn a_silent_host_times_out_naming_its_peer() {
    let (_dir, socket, listener) = bind();
    let (_, host) = answering(listener, Vec::new());
    let started = Instant::now();
    let result = HostConnection::connect_with(&socket, deadlines()).await;
    host.abort();
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
    match result {
        Err(ConnectError::Unavailable {
            reason: Unavailable::Timeout,
            peer: Some(peer),
        }) => assert_eq!(peer.pid, std::process::id()),
        other => panic!("expected a timeout, got {other:?}"),
    }
}

/// A host that keeps closing connections before answering is retried, and
/// is then unavailable, not gone.
///
/// Why: a host that is ending closes the connections that wait for their
/// `Hello`, and one that refuses this user closes without a word; both are
/// still listening, so neither is gone. Spec: with a stand-in that closes
/// every connection at once, the result is `Unavailable(Closed)` after two
/// or more connections.
#[farhelm_testtrace::test]
async fn a_host_that_closes_before_answering_is_unavailable() {
    let (_dir, socket, listener) = bind();
    let attempts = Arc::new(AtomicUsize::new(0));
    let counted = attempts.clone();
    let host = tokio::spawn(async move {
        loop {
            let (server, _) = listener.accept().await.unwrap();
            counted.fetch_add(1, Ordering::SeqCst);
            drop(server);
        }
    });
    let result = HostConnection::connect_with(&socket, deadlines()).await;
    host.abort();
    assert!(
        matches!(
            result,
            Err(ConnectError::Unavailable {
                reason: Unavailable::Closed,
                ..
            })
        ),
        "{result:?}"
    );
    assert!(attempts.load(Ordering::SeqCst) >= 2, "it was not retried");
}

// ---- Requests --------------------------------------------------------------------

/// Shift+Enter's two bytes go out as one `Input`.
///
/// Why: the host offers an `Input` of up to `ATOMIC_INPUT` bytes to the pty
/// in one write, and programs tell Shift+Enter from Escape-then-Enter by the
/// bytes arriving together; split over two `Input`s, they could be read
/// apart. Spec: after `send_input(ESC CR)` the stand-in receives exactly one
/// frame, `Input([0x1b, 0x0d])`, and the ticket ends at 2.
#[farhelm_testtrace::test]
async fn shift_enter_is_one_input() {
    let mut pair = connected().await;
    let ticket = pair.client.send_input(b"\x1b\r").await.unwrap();
    assert_eq!(ticket, InputTicket { end: 2 });
    assert_eq!(
        pair.next_frame().await,
        ClientMessage::Input(b"\x1b\r".to_vec())
    );
    assert!(pair.buf.is_empty(), "more was sent: {:?}", pair.buf);
}

/// Unacknowledged input stays within the window, and acknowledgements make
/// room again.
///
/// Why: past the host's `MAX_PENDING_INPUT` the host stops reading the
/// client, so a program that never reads its terminal would hold back the
/// connection's resizes, snapshots and shutdown; the window keeps the
/// client far from that. Spec: against a stand-in that reads every frame
/// but acknowledges nothing, three `MAX_INPUT` inputs are sent and the
/// fourth is refused as `InputWindowFull` (nothing more reaches the
/// stand-in); after an acknowledgement of the first as written,
/// `next_event` yields `InputAcknowledged` with 65536 written and the rest
/// unknown, and the fourth input is accepted and arrives.
#[farhelm_testtrace::test]
async fn unacknowledged_input_stays_within_the_window() {
    let mut pair = connected().await;
    let chunk = vec![b'x'; MAX_INPUT];
    let Pair {
        client,
        server,
        buf,
        ..
    } = &mut pair;
    let (sent, ()) = tokio::join!(
        async {
            for _ in 0..3 {
                client.send_input(&chunk).await.unwrap();
            }
            client.send_input(&chunk).await
        },
        async {
            for _ in 0..3 {
                assert!(matches!(
                    read_frame(server, buf).await,
                    ClientMessage::Input(_)
                ));
            }
        }
    );
    assert!(
        matches!(sent, Err(ClientError::InputWindowFull { .. })),
        "{sent:?}"
    );
    assert!(buf.is_empty(), "a refused input was sent");
    assert_eq!(client.input_counts().sent, 3 * MAX_INPUT as u64);
    send(
        server,
        &HostMessage::InputAck {
            written: MAX_INPUT as u64,
            discarded: 0,
        },
    )
    .await;
    let counts = InputCounts {
        sent: 3 * MAX_INPUT as u64,
        written: MAX_INPUT as u64,
        discarded: 0,
    };
    assert_eq!(
        next(client).await.unwrap(),
        HostEvent::InputAcknowledged(counts)
    );
    assert_eq!(counts.unknown(), 2 * MAX_INPUT as u64);
    assert!(counts.resolved(InputTicket {
        end: MAX_INPUT as u64
    }));
    let (sent, received) = tokio::join!(client.send_input(&chunk), read_frame(server, buf));
    sent.unwrap();
    assert!(matches!(received, ClientMessage::Input(_)));
}

/// An acknowledgement that breaks the counts' rules ends the connection:
/// one that ends inside an `Input`, one that covers more than was sent, and
/// one that goes backwards.
///
/// Why: the window and the "never resend" report both rest on the counts
/// meaning what the protocol says; a host that breaks that cannot be
/// trusted for either. Spec: after two inputs of 5 bytes, an
/// acknowledgement of 7 written, one of 11 written, and (after a valid one
/// of 5 written) one of 0 written each give `Malformed` from `next_event`;
/// the next request returns the same error.
#[farhelm_testtrace::test]
async fn an_acknowledgement_that_breaks_the_counts_ends_the_connection() {
    for (valid_first, bad) in [(None, (7, 0)), (None, (11, 0)), (Some((5, 0)), (0, 0))] {
        let mut pair = connected().await;
        pair.client.send_input(b"hello").await.unwrap();
        pair.client.send_input(b"world").await.unwrap();
        if let Some((written, discarded)) = valid_first {
            send(
                &mut pair.server,
                &HostMessage::InputAck { written, discarded },
            )
            .await;
            assert!(matches!(
                next(&mut pair.client).await,
                Ok(HostEvent::InputAcknowledged(_))
            ));
        }
        let (written, discarded) = bad;
        send(
            &mut pair.server,
            &HostMessage::InputAck { written, discarded },
        )
        .await;
        let error = next(&mut pair.client).await.unwrap_err();
        assert!(
            matches!(error, ClientError::Malformed(_)),
            "{bad:?}: {error:?}"
        );
        assert_eq!(
            pair.client.resize(Size { cols: 1, rows: 1 }).await,
            Err(error)
        );
    }
}

/// A connection reads one stream: a second `Read` is refused before it is
/// sent, and a record before any `Read` breaks the protocol.
///
/// Why: the host answers a second `Read` with an `Error` that ends the
/// connection, and records only ever follow a `Read`. Spec: the stand-in
/// receives one `Read((5, 1))`; the second `start_read` gives
/// `ReadAlreadyStarted` and the connection still works (an `Info` round
/// trip). On a second connection, an `Output` before `Read` makes
/// `next_event` give `Malformed`.
#[farhelm_testtrace::test]
async fn one_read_per_connection() {
    let mut pair = connected().await;
    let from = Position {
        offset: 5,
        resizes: 1,
    };
    pair.client.start_read(from).await.unwrap();
    assert_eq!(
        pair.client.start_read(from).await,
        Err(ClientError::ReadAlreadyStarted)
    );
    assert_eq!(pair.next_frame().await, ClientMessage::Read(from));
    let Pair {
        client,
        server,
        buf,
        ..
    } = &mut pair;
    let (info, ()) = tokio::join!(client.info(), async {
        assert_eq!(read_frame(server, buf).await, ClientMessage::Info);
        send(server, &HostMessage::InfoReply(info_numbered(7))).await;
    });
    assert_eq!(info.unwrap().host_pid, 7);

    let mut early = connected().await;
    send(
        &mut early.server,
        &HostMessage::Output {
            offset: 0,
            data: b"x".to_vec(),
        },
    )
    .await;
    let error = next(&mut early.client).await;
    assert!(matches!(error, Err(ClientError::Malformed(_))), "{error:?}");
}

/// Records that arrive while a request waits for its reply are kept, in
/// order, for `next_event`.
///
/// Why: replies share the byte stream with the records, and a runtime that
/// asks for `Info` mid-stream must not lose the output read on the way to
/// the reply. Spec: with a stream started, the stand-in sends `Output`,
/// `Resized` and then the `Info` reply; `info` returns the reply, and
/// `next_event` then yields the `Output` and the `Resized`, in that order.
#[farhelm_testtrace::test]
async fn records_read_while_waiting_for_a_reply_are_kept() {
    let mut pair = connected().await;
    pair.client.start_read(Position::default()).await.unwrap();
    assert!(matches!(pair.next_frame().await, ClientMessage::Read(_)));
    let output = HostMessage::Output {
        offset: 0,
        data: b"before the reply".to_vec(),
    };
    let size = Size { cols: 90, rows: 30 };
    let Pair {
        client,
        server,
        buf,
        ..
    } = &mut pair;
    let (info, ()) = tokio::join!(client.info(), async {
        assert_eq!(read_frame(server, buf).await, ClientMessage::Info);
        send(server, &output).await;
        send(
            server,
            &HostMessage::Resized {
                offset: 16,
                index: 0,
                size,
            },
        )
        .await;
        send(server, &HostMessage::InfoReply(info_numbered(9))).await;
    });
    assert_eq!(info.unwrap().host_pid, 9);
    assert_eq!(
        pair.client.next_event().await.unwrap(),
        HostEvent::Output {
            offset: 0,
            data: b"before the reply".to_vec()
        }
    );
    assert_eq!(
        pair.client.next_event().await.unwrap(),
        HostEvent::Resized {
            offset: 16,
            index: 0,
            size
        }
    );
}

/// A request whose future was dropped before its reply does not hand that
/// reply to the next request.
///
/// Why: a runtime's `select!` or a timeout can drop a request after it was
/// sent; the host still answers it, and the next request of the same kind
/// would otherwise take the stale answer for its own. Spec: an `info` that
/// is dropped after its request went out, then a second `info`; the
/// stand-in answers the first with `host_pid` 1 and the second with 2, and
/// the second call returns 2.
#[farhelm_testtrace::test]
async fn a_dropped_requests_reply_is_skipped() {
    let mut pair = connected().await;
    let dropped = tokio::time::timeout(Duration::from_millis(100), pair.client.info()).await;
    assert!(
        dropped.is_err(),
        "premise: the first request was dropped unanswered"
    );
    assert_eq!(pair.next_frame().await, ClientMessage::Info);
    let Pair {
        client,
        server,
        buf,
        ..
    } = &mut pair;
    let (info, ()) = tokio::join!(client.info(), async {
        assert_eq!(read_frame(server, buf).await, ClientMessage::Info);
        send(server, &HostMessage::InfoReply(info_numbered(1))).await;
        send(server, &HostMessage::InfoReply(info_numbered(2))).await;
    });
    assert_eq!(info.unwrap().host_pid, 2);
}

/// A frame whose request was dropped half-written keeps its write deadline:
/// `next_event` gives up on a host that stopped reading.
///
/// Why: the rest of the frame stays queued and is written by whatever call
/// comes next; if that call is `next_event`, whose wait for events has no
/// bound, a host that no longer reads would hold it forever. Spec: with a
/// 1 s reply deadline and a stand-in that never reads, a 4 MiB `snapshot`
/// is dropped after 100 ms with part of it unwritten; `next_event` then
/// gives `Timeout` within the deadline plus a margin, and the connection is
/// ended.
#[farhelm_testtrace::test]
async fn a_dropped_half_written_frame_keeps_its_deadline() {
    let mut short = deadlines();
    short.reply = Duration::from_secs(1);
    let mut pair = connected_with(short).await;
    let started = Instant::now();
    let dropped = tokio::time::timeout(
        Duration::from_millis(100),
        pair.client
            .snapshot(Position::default(), vec![0; 4 * 1024 * 1024]),
    )
    .await;
    assert!(
        dropped.is_err(),
        "premise: the snapshot was dropped unwritten"
    );
    assert!(
        !pair.client.outbound.is_empty(),
        "premise: part of the frame waits to be written"
    );
    assert_eq!(next(&mut pair.client).await, Err(ClientError::Timeout));
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(pair.client.ended(), Some(&ClientError::Timeout));
}

/// Requests dropped again and again against a host that stopped reading
/// never queue more than one request's frame, and a request dropped while
/// an earlier frame is still being written leaves nothing behind.
///
/// Why: an owner's `select!` may drop a request every time it runs, and
/// each dropped request used to leave its whole frame queued behind the
/// last one; with snapshots of up to 16 MiB, a host that stopped reading
/// could make the supervisor allocate without bound before any deadline
/// passed. Writing out the leftover first fixes that, but only if a request
/// dropped meanwhile takes nothing: a request number taken with no frame
/// sent would wait for a reply that never comes, a window cost with no
/// input would shrink the window for good, and a read flag with no `Read`
/// would refuse the real one. Spec: against a stand-in that is not reading,
/// a 2 MiB `snapshot` is dropped after 20 ms with part of it unwritten, and
/// seven more are each dropped the same way; after every one the queue
/// holds no more than one such frame. An `info`, a `send_input`, a
/// `start_read` and a `resize` are each dropped the same way: one snapshot
/// request is numbered, no `Info` is, no input is sent and the window is
/// whole, and no stream is started. The connection has not ended. Then the
/// stand-in reads: the first frame on the wire is the first snapshot, a
/// fresh `snapshot` is the next frame and gets its own reply, and a fresh
/// `start_read` is accepted and is the frame after that.
#[farhelm_testtrace::test]
async fn dropped_requests_never_queue_more_than_one_frame() {
    let Pair {
        _dir,
        mut client,
        mut server,
        buf: _,
    } = connected().await;
    let blob = vec![0; 2 * 1024 * 1024];
    let frame = encode_client(&ClientMessage::Snapshot {
        position: Position::default(),
        blob: blob.clone(),
    })
    .len();
    let drop_soon = Duration::from_millis(20);
    for attempt in 0..8 {
        let dropped = tokio::time::timeout(
            drop_soon,
            client.snapshot(Position::default(), blob.clone()),
        )
        .await;
        assert!(dropped.is_err(), "premise: request {attempt} was dropped");
        assert!(
            client.write_pending(),
            "premise: the first frame is still partly unwritten"
        );
        assert!(
            client.outbound.len() <= frame,
            "after {} dropped requests, {} bytes are queued",
            attempt + 1,
            client.outbound.len()
        );
    }
    let dropped = tokio::time::timeout(drop_soon, client.info()).await;
    assert!(dropped.is_err(), "premise: the info was dropped");
    assert!(
        client.write_pending(),
        "premise: the leftover was still unwritten when the info was dropped"
    );
    let dropped = tokio::time::timeout(drop_soon, client.send_input(b"typed")).await;
    assert!(dropped.is_err(), "premise: the input was dropped");
    assert!(
        client.write_pending(),
        "premise: the leftover was still unwritten when the input was dropped"
    );
    let dropped = tokio::time::timeout(drop_soon, client.start_read(Position::default())).await;
    assert!(dropped.is_err(), "premise: the start_read was dropped");
    assert!(
        client.write_pending(),
        "premise: the leftover was still unwritten when the start_read was dropped"
    );
    let dropped = tokio::time::timeout(drop_soon, client.resize(Size { cols: 9, rows: 9 })).await;
    assert!(dropped.is_err(), "premise: the resize was dropped");
    assert!(
        client.write_pending(),
        "premise: the leftover was still unwritten when the resize was dropped"
    );
    assert_eq!(client.snapshots.sent, 1, "dropped snapshots were numbered");
    assert_eq!(client.info.sent, 0, "a dropped Info was numbered");
    assert_eq!(client.input_counts().sent, 0, "a dropped input was sent");
    assert_eq!(
        client.input_room(),
        INPUT_WINDOW,
        "a dropped input took room"
    );
    assert!(!client.reading, "a dropped start_read started the stream");
    assert_eq!(client.ended(), None, "premise: within the deadline");

    let second = Position {
        offset: 2,
        resizes: 0,
    };
    let host = async {
        assert!(matches!(
            read_one_frame(&mut server).await,
            ClientMessage::Snapshot { position, .. } if position == Position::default()
        ));
        send(
            &mut server,
            &HostMessage::SnapshotAccepted(Position::default()),
        )
        .await;
        assert!(matches!(
            read_one_frame(&mut server).await,
            ClientMessage::Snapshot { position, .. } if position == second
        ));
        send(&mut server, &HostMessage::SnapshotAccepted(second)).await;
    };
    let (reply, ()) = tokio::join!(client.snapshot(second, vec![1; 16]), host);
    assert_eq!(reply, Ok(SnapshotReply::Accepted(second)));
    let (read, frame) = tokio::join!(
        client.start_read(Position::default()),
        read_one_frame(&mut server)
    );
    read.unwrap();
    assert_eq!(frame, ClientMessage::Read(Position::default()));
}

/// A request whose `select!` arm loses before it is first polled sends
/// nothing, even with nothing waiting to be written, and the connection
/// goes on working.
///
/// Why: these requests are `async fn`s and do nothing until polled, so a
/// `select!` that picks another ready arm first drops them unsent; an owner
/// must not read an empty write queue as proof that a cancelled input or
/// resize went out. Spec: with nothing pending, a biased `select!` whose
/// first arm is ready and whose second is `send_input`, then the same with
/// `resize`, leaves the input counts at nothing sent, the window whole and
/// nothing queued; the stand-in receives nothing from them, and an `Info`
/// round trip afterwards is the next frame it reads.
#[farhelm_testtrace::test]
async fn a_request_that_loses_its_select_before_its_first_poll_sends_nothing() {
    let Pair {
        _dir,
        mut client,
        mut server,
        mut buf,
    } = connected().await;
    assert!(!client.write_pending(), "premise: nothing is pending");
    let mut lost = 0;
    tokio::select! {
        biased;
        () = std::future::ready(()) => lost += 1,
        _ = client.send_input(b"typed") => {}
    }
    tokio::select! {
        biased;
        () = std::future::ready(()) => lost += 1,
        _ = client.resize(Size { cols: 9, rows: 9 }) => {}
    }
    assert_eq!(lost, 2, "premise: the ready arm won both times");
    assert_eq!(client.input_counts(), InputCounts::default());
    assert_eq!(client.input_room(), INPUT_WINDOW);
    assert!(!client.write_pending(), "a dropped request queued a frame");
    let (info, ()) = tokio::join!(client.info(), async {
        assert_eq!(read_frame(&mut server, &mut buf).await, ClientMessage::Info);
        send(&mut server, &HostMessage::InfoReply(info_numbered(3))).await;
    });
    assert_eq!(info.unwrap().host_pid, 3);
}

/// `shutdown` keeps to its own deadline even when it must first write out
/// what a dropped request left queued.
///
/// Why: Delete and orderly stops wait on `shutdown` before falling back to
/// a signal, and its deadline is the bound they plan with; waiting out the
/// leftover frame's longer reply deadline first would stretch it to that.
/// Spec: with a 10 s reply deadline and a 1 s shutdown deadline, against a
/// stand-in that is not reading, a 4 MiB snapshot is dropped after 100 ms
/// with part of it unwritten; `shutdown` then gives `Timeout` in under 3 s.
#[farhelm_testtrace::test]
async fn shutdown_keeps_its_own_deadline_behind_a_leftover_frame() {
    let mut budgets = deadlines();
    budgets.reply = Duration::from_secs(10);
    budgets.shutdown = Duration::from_secs(1);
    let mut pair = connected_with(budgets).await;
    let dropped = tokio::time::timeout(
        Duration::from_millis(100),
        pair.client
            .snapshot(Position::default(), vec![0; 4 * 1024 * 1024]),
    )
    .await;
    assert!(dropped.is_err(), "premise: the snapshot was dropped");
    assert!(
        pair.client.write_pending(),
        "premise: part of the frame waits to be written"
    );
    let started = Instant::now();
    let outcome = tokio::time::timeout(WAIT * 2, pair.client.shutdown())
        .await
        .expect("shutdown returned at all");
    assert_eq!(outcome, Err(ClientError::Timeout));
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "shutdown took {:?}",
        started.elapsed()
    );
}

/// A frame written after a dropped, half-written one gets its own write
/// deadline, not the older frame's.
///
/// Why: a request made while an earlier dropped one is still being written
/// must not inherit the time that one already used up, or a large snapshot
/// queued late would time out after a fraction of its deadline. Spec: with
/// a 3 s reply deadline and a stand-in that reads nothing for 2 s, a 4 MiB
/// snapshot is dropped after 100 ms, and a second 4 MiB snapshot is made
/// at once. The stand-in reads the first at 2 s and the second at 3.2 s,
/// after the first frame's deadline but within 3 s of when the first was
/// written out, and accepts both; the second snapshot is accepted.
#[farhelm_testtrace::test]
async fn a_frame_after_a_dropped_one_gets_its_own_deadline() {
    let mut slow = deadlines();
    slow.reply = Duration::from_secs(3);
    let Pair {
        _dir,
        mut client,
        mut server,
        buf: _,
    } = connected_with(slow).await;
    let first = Position {
        offset: 1,
        resizes: 0,
    };
    let second = Position {
        offset: 2,
        resizes: 0,
    };
    let blob = vec![0; 4 * 1024 * 1024];
    let started = Instant::now();
    let dropped = tokio::time::timeout(
        Duration::from_millis(100),
        client.snapshot(first, blob.clone()),
    )
    .await;
    assert!(dropped.is_err(), "premise: the first snapshot was dropped");
    assert!(
        !client.outbound.is_empty(),
        "premise: part of the first frame waits to be written"
    );
    let host = async {
        // sleep-ok: the stand-in stops reading for most of the first frame's deadline.
        tokio::time::sleep_until(started + Duration::from_secs(2)).await;
        assert!(matches!(
            read_one_frame(&mut server).await,
            ClientMessage::Snapshot { position, .. } if position == first
        ));
        // sleep-ok: hold the second frame back past the first frame's deadline, well within its own.
        tokio::time::sleep_until(started + Duration::from_millis(3200)).await;
        assert!(matches!(
            read_one_frame(&mut server).await,
            ClientMessage::Snapshot { position, .. } if position == second
        ));
        let finished = Instant::now();
        send(&mut server, &HostMessage::SnapshotAccepted(first)).await;
        send(&mut server, &HostMessage::SnapshotAccepted(second)).await;
        finished
    };
    let (reply, finished) = tokio::join!(client.snapshot(second, blob), host);
    assert!(
        finished > started + Duration::from_secs(3),
        "premise: the second frame was written after the first frame's deadline ({:?})",
        finished - started
    );
    assert_eq!(reply, Ok(SnapshotReply::Accepted(second)));
}

/// Events decoded from the same read as the frame that ends the connection
/// are handed out before the error.
///
/// Why: a host that sends output and then an `Error` writes them back to
/// back, and whether they arrive in one read or two is the socket's
/// business; the output must reach the owner either way. Spec: the
/// stand-in writes an `Output` and an `Error` in one write; `next_event`
/// yields the `Output`, then the host's error, then the same error again.
#[farhelm_testtrace::test]
async fn events_before_an_ending_frame_come_first() {
    let mut pair = connected().await;
    pair.client.start_read(Position::default()).await.unwrap();
    assert!(matches!(pair.next_frame().await, ClientMessage::Read(_)));
    let mut both = encode_host(&HostMessage::Output {
        offset: 0,
        data: b"last words".to_vec(),
    });
    both.extend(encode_host(&HostMessage::Error {
        code: ErrorCode::NotAllowed,
        message: "no".into(),
    }));
    pair.server.write_all(&both).await.unwrap();
    assert!(matches!(
        next(&mut pair.client).await,
        Ok(HostEvent::Output { .. })
    ));
    let error = ClientError::Host {
        code: ErrorCode::NotAllowed,
        message: "no".into(),
    };
    assert_eq!(next(&mut pair.client).await, Err(error.clone()));
    assert_eq!(next(&mut pair.client).await, Err(error));
}

/// An owner that leaves events unread past the queue's limit has its
/// connection ended, rather than the supervisor grow without bound.
///
/// Why: requests read the socket while they wait and keep what arrives; a
/// program flooding its terminal while its owner only makes requests would
/// otherwise make the shared supervisor hold all of it. Spec: with a
/// stream started, the stand-in sends full-size `Output` chunks past
/// `EVENT_QUEUE_LIMIT` and never answers an `Info`; `info` gives
/// `EventQueueFull` and the connection is ended.
#[farhelm_testtrace::test]
async fn unread_events_past_the_limit_end_the_connection() {
    let mut pair = connected().await;
    pair.client.start_read(Position::default()).await.unwrap();
    assert!(matches!(pair.next_frame().await, ClientMessage::Read(_)));
    let chunk = encode_host(&HostMessage::Output {
        offset: 0,
        data: vec![b'y'; MAX_OUTPUT_CHUNK],
    });
    let Pair {
        client,
        server,
        buf,
        ..
    } = &mut pair;
    let flood = async {
        assert_eq!(read_frame(server, buf).await, ClientMessage::Info);
        // Once the client gives up it stops reading, and a write then
        // waits; that is where the flood ends.
        for _ in 0..EVENT_QUEUE_LIMIT / MAX_OUTPUT_CHUNK + 2 {
            match tokio::time::timeout(Duration::from_secs(1), server.write_all(&chunk)).await {
                Ok(Ok(())) => {}
                _ => break,
            }
        }
    };
    let (info, ()) = tokio::join!(client.info(), flood);
    assert_eq!(info, Err(ClientError::EventQueueFull));
    assert_eq!(client.ended(), Some(&ClientError::EventQueueFull));
}

/// When the host closes the connection, the events read before it are
/// handed out first, then `Closed`, and the input sent but not
/// acknowledged stays unknown.
///
/// Why: a takeover by another client is announced only by the end of the
/// stream, and the input in flight then has an unknown fate that the
/// caller must report rather than resend. Spec: after an input of 4 bytes,
/// the stand-in sends one `Output` and closes; `next_event` yields the
/// `Output`, then `Closed` (and `Closed` again on the next call); the counts
/// are 4 sent and 4 unknown.
#[farhelm_testtrace::test]
async fn a_closed_connection_hands_out_what_came_first_and_leaves_input_unknown() {
    let mut pair = connected().await;
    pair.client.start_read(Position::default()).await.unwrap();
    pair.client.send_input(b"ls\r\n").await.unwrap();
    assert!(matches!(pair.next_frame().await, ClientMessage::Read(_)));
    assert!(matches!(pair.next_frame().await, ClientMessage::Input(_)));
    send(
        &mut pair.server,
        &HostMessage::Output {
            offset: 0,
            data: b"last words".to_vec(),
        },
    )
    .await;
    drop(pair.server);
    assert!(matches!(
        next(&mut pair.client).await,
        Ok(HostEvent::Output { .. })
    ));
    assert_eq!(next(&mut pair.client).await, Err(ClientError::Closed));
    assert_eq!(next(&mut pair.client).await, Err(ClientError::Closed));
    let counts = pair.client.input_counts();
    assert_eq!((counts.sent, counts.unknown()), (4, 4));
}

/// `shutdown` reports `Goodbye` as acknowledged, a connection that ends
/// after the request without it as unacknowledged, and one that ended
/// before the request could be written as an error.
///
/// Why: Delete counts a host as ended only on evidence; a `Shutdown` the
/// host never read must not look like one it acted on. Spec: a stand-in
/// that reads `Shutdown` and says `Goodbye` gives `goodbye: true`; one that
/// reads it and closes gives `goodbye: false`; on a third connection, the
/// stand-in closes first, the client learns it from `next_event`, and
/// `shutdown` then gives `Closed`; on a fourth, the stand-in closes and
/// `shutdown` is called at once, so it is `shutdown` that meets the close
/// with its frame still unwritten, and it gives `Closed` too.
#[farhelm_testtrace::test]
async fn shutdown_reports_whether_the_host_acknowledged() {
    for goodbye in [true, false] {
        let Pair {
            _dir,
            client,
            mut server,
            mut buf,
        } = connected().await;
        let (outcome, ()) = tokio::join!(client.shutdown(), async {
            assert_eq!(
                read_frame(&mut server, &mut buf).await,
                ClientMessage::Shutdown
            );
            if goodbye {
                send(&mut server, &HostMessage::Goodbye).await;
            }
            server.shutdown().await.unwrap();
        });
        assert_eq!(outcome.unwrap().goodbye, goodbye);
    }
    let mut pair = connected().await;
    drop(pair.server);
    assert_eq!(next(&mut pair.client).await, Err(ClientError::Closed));
    assert_eq!(pair.client.shutdown().await, Err(ClientError::Closed));

    let pair = connected().await;
    drop(pair.server);
    assert_eq!(
        pair.client.ended(),
        None,
        "premise: the client has not seen the close"
    );
    assert_eq!(pair.client.shutdown().await, Err(ClientError::Closed));
}
