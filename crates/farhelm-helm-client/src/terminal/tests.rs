//! A terminal stream against a scripted WebSocket server standing in for the helm's terminal route.
//!
//! A real helm cannot serve a terminal here (that needs a supervisor and a running session), so this server checks
//! the parts the helm checks (path, query and credential subprotocol) and plays the frames a helm would send. The
//! tests run on real time but never wait on it: every step waits for the next socket or event, and the whole test is
//! bounded so a regression fails instead of hanging.

use super::*;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::handshake::server::{Request as Upgrade, Response};

/// One accepted socket and the path-and-query it was opened with.
struct Accepted {
    uri: String,
    socket: WebSocketStream<TcpStream>,
}

/// Accept sockets as a helm would, refusing any upgrade without the device credential.
async fn server() -> (SocketAddr, mpsc::UnboundedReceiver<Accepted>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Ok((tcp, _)) = listener.accept().await {
            let (uri_tx, uri_rx) = std::sync::mpsc::channel();
            #[expect(
                clippy::result_large_err,
                reason = "tungstenite's handshake callback fixes the error type"
            )]
            let callback = move |request: &Upgrade, mut response: Response| {
                let offered = request
                    .headers()
                    .get("Sec-WebSocket-Protocol")
                    .and_then(|v| v.to_str().ok());
                assert_eq!(offered, Some("farhelm, farhelm-device-test-secret"));
                response
                    .headers_mut()
                    .insert("Sec-WebSocket-Protocol", "farhelm".parse().unwrap());
                uri_tx.send(request.uri().to_string()).unwrap();
                Ok(response)
            };
            let socket = tokio_tungstenite::accept_hdr_async(tcp, callback)
                .await
                .unwrap();
            let uri = uri_rx.recv().unwrap();
            if tx.send(Accepted { uri, socket }).is_err() {
                break;
            }
        }
    });
    (addr, rx)
}

async fn next_event(events: &mut TerminalEvents) -> TerminalEvent {
    events.recv().await.expect("the stream task is alive")
}

async fn replay(socket: &mut WebSocketStream<TcpStream>) {
    let output = GridOutput::HistoryReset.encode().unwrap();
    socket.send(Message::binary(output)).await.unwrap();
    socket
        .send(Message::text(r#"{"type":"replay_complete"}"#))
        .await
        .unwrap();
}

/// The reconnect rules decide whether a client can steal a terminal without the user asking, so they are pinned on
/// the wire: the first attach takes control, a reconnect after a drop uses the non-displacing route with the same
/// lease and the latest size, a takeover notice stops the stream instead of fighting back, and only an explicit
/// take-control displaces again.
#[tokio::test]
async fn reconnects_never_displace_and_a_takeover_stops_the_stream() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let (addr, mut accepted) = server().await;
        let attach = TerminalAttach {
            session: "host:1/s 1".into(),
            tab: Some("t1".into()),
            lease: AttachmentLease::new(),
            cols: 80,
            rows: 24,
        };
        let lease = attach.lease.0.clone();
        let (handle, mut events) = open_terminal(
            HelmEndpoint::new(addr, "test-secret"),
            attach,
            ChannelConfig::default(),
        );
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Attaching)
        );

        // First attach: takes control, and the session id stays one segment.
        let mut first = accepted.recv().await.unwrap();
        assert_eq!(
            first.uri,
            format!("/api/sessions/host%3A1%2Fs%201/term?cols=80&rows=24&lease={lease}&tab=t1")
        );
        replay(&mut first.socket).await;
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Attached)
        );
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::Output(GridOutput::HistoryReset)
        );
        assert_eq!(next_event(&mut events).await, TerminalEvent::ReplayComplete);

        // Input and resizes reach the helm once attached.
        handle.input(GridInput::Text {
            data: "ls\r".into(),
        });
        handle.resize(100, 30);
        let Some(Ok(Message::Binary(bytes))) = first.socket.next().await else {
            panic!("no input frame")
        };
        assert_eq!(
            GridInput::decode(&bytes).unwrap(),
            GridInput::Text {
                data: "ls\r".into()
            }
        );
        let Some(Ok(Message::Text(resize))) = first.socket.next().await else {
            panic!("no resize frame")
        };
        let resize: serde_json::Value = serde_json::from_str(&resize).unwrap();
        assert_eq!(
            resize,
            serde_json::json!({"type": "resize", "cols": 100, "rows": 30})
        );

        // A drop reconnects through the non-displacing route, at the new size (after the backoff: this attachment never
        // answered a ping).
        drop(first);
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Reconnecting)
        );
        let mut second = accepted.recv().await.unwrap();
        assert_eq!(
            second.uri,
            format!(
                "/api/sessions/host%3A1%2Fs%201/term/unowned?cols=100&rows=30&lease={lease}&tab=t1"
            )
        );
        replay(&mut second.socket).await;
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Attached)
        );
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::Output(GridOutput::HistoryReset)
        );
        assert_eq!(next_event(&mut events).await, TerminalEvent::ReplayComplete);

        // Another client took the terminal: the stream stops and says why.
        let notice =
            r#"{"type":"detached","reason":"another client took over","code":"taken_over"}"#;
        second.socket.send(Message::text(notice)).await.unwrap();
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Detached {
                code: Some(DetachCode::TakenOver),
                reason: "another client took over".into(),
            })
        );

        // Only the user's take-control attaches again, through the displacing route.
        handle.take_control();
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Attaching)
        );
        let third = accepted.recv().await.unwrap();
        assert!(
            third
                .uri
                .starts_with("/api/sessions/host%3A1%2Fs%201/term?"),
            "{}",
            third.uri
        );

        handle.close();
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Closed)
        );
    })
    .await
    .expect("the stream test finished in time");
}

/// A dot segment from a host-supplied id must not become a request to another route; the stream reports it as the
/// refusal the helm would give (404) without opening a socket.
#[tokio::test]
async fn a_dot_session_id_never_becomes_a_path() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let (addr, mut accepted) = server().await;
        let (_handle, mut events) = open_terminal(
            HelmEndpoint::new(addr, "test-secret"),
            attach(".."),
            ChannelConfig::default(),
        );
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Attaching)
        );
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Refused { status: 404 })
        );
        assert!(accepted.try_recv().is_err(), "a socket was opened");
    })
    .await
    .expect("the stream test finished in time");
}

/// An agent-terminal attach for `session` at 80x24.
fn attach(session: &str) -> TerminalAttach {
    TerminalAttach {
        session: session.into(),
        tab: None,
        lease: AttachmentLease::new(),
        cols: 80,
        rows: 24,
    }
}

/// Open a stream against the scripted server and complete its first attachment (attach, replay, live).
async fn attached(
    config: ChannelConfig,
) -> (
    TerminalHandle,
    TerminalEvents,
    mpsc::UnboundedReceiver<Accepted>,
    Accepted,
) {
    let (addr, mut accepted) = server().await;
    let (handle, mut events) =
        open_terminal(HelmEndpoint::new(addr, "test-secret"), attach("s1"), config);
    assert_eq!(
        next_event(&mut events).await,
        TerminalEvent::State(StreamState::Attaching)
    );
    let mut socket = accepted.recv().await.unwrap();
    replay(&mut socket.socket).await;
    until_event(&mut events, TerminalEvent::ReplayComplete).await;
    (handle, events, accepted, socket)
}

/// Read events until `wanted`, discarding the ones before it (state changes and replay output the test does not
/// inspect).
async fn until_event(events: &mut TerminalEvents, wanted: TerminalEvent) {
    while next_event(events).await != wanted {}
}

/// Which detach codes stop the stream is the whole reconnect policy (SPEC.md, "Terminal experience"): a takeover, a
/// closed tab or a stall waits for the user, because reattaching would fight another client, chase a tab that is gone,
/// or walk back into the wedge that stalled it. `replaced` and `other` carry trouble that keeps recovering (a lost
/// supervisor, an unreachable host), so they back off and reattach without displacing anyone. Pinned per code, on
/// the wire.
#[tokio::test]
async fn each_detach_code_stops_or_reattaches_as_the_policy_says() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let quick = ChannelConfig {
            reconnect_initial: Duration::from_millis(10),
            ..ChannelConfig::default()
        };
        for (code, stops) in [
            ("taken_over", true),
            ("tab_closed", true),
            ("stalled", true),
            ("replaced", false),
            ("other", false),
        ] {
            let (_handle, mut events, mut accepted, mut first) = attached(quick.clone()).await;
            let notice = format!(r#"{{"type":"detached","reason":"why","code":"{code}"}}"#);
            first.socket.send(Message::text(notice)).await.unwrap();
            let expected = if stops {
                let code = serde_json::from_value(serde_json::json!(code)).unwrap();
                StreamState::Detached {
                    code: Some(code),
                    reason: "why".into(),
                }
            } else {
                StreamState::Reconnecting
            };
            assert_eq!(
                next_event(&mut events).await,
                TerminalEvent::State(expected),
                "{code}"
            );
            if !stops {
                let again = accepted.recv().await.unwrap();
                assert!(
                    again.uri.starts_with("/api/sessions/s1/term/unowned?"),
                    "{code}: {}",
                    again.uri
                );
            }
        }
    })
    .await
    .expect("the stream test finished in time");
}

/// The "Reconnect now" button must skip the backoff (the wait here is an hour, so only the request can explain the
/// prompt attach) and must still use the non-displacing route.
#[tokio::test]
async fn reconnect_now_skips_the_backoff_without_displacing() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let slow = ChannelConfig {
            reconnect_initial: Duration::from_secs(3600),
            ..ChannelConfig::default()
        };
        let (handle, mut events, mut accepted, mut first) = attached(slow).await;
        let notice = r#"{"type":"detached","reason":"host unreachable","code":"other"}"#;
        first.socket.send(Message::text(notice)).await.unwrap();
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Reconnecting)
        );
        handle.reconnect_now();
        let again = accepted.recv().await.unwrap();
        assert!(
            again.uri.starts_with("/api/sessions/s1/term/unowned?"),
            "{}",
            again.uri
        );
    })
    .await
    .expect("the stream test finished in time");
}

/// A helm that stops answering pings is a dead link even while the socket stays open; the stream must notice by
/// itself and come back, as the UI channel does.
#[tokio::test]
async fn an_unanswered_ping_reconnects_the_stream() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let fast = ChannelConfig {
            ping_interval: Duration::from_millis(50),
            watchdog: Duration::from_millis(200),
            ..ChannelConfig::default()
        };
        let (_handle, mut events, mut accepted, mut first) = attached(fast).await;
        // The server reads the ping (proving one was sent) and never answers it.
        let Some(Ok(Message::Text(ping))) = first.socket.next().await else {
            panic!("no ping frame")
        };
        assert_eq!(ping.as_str(), r#"{"type":"ping"}"#);
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Reconnecting)
        );
        let again = accepted.recv().await.unwrap();
        assert!(again.uri.contains("/term/unowned?"), "{}", again.uri);
    })
    .await
    .expect("the stream test finished in time");
}

/// Keystrokes typed into a terminal that is not attached must not land in whatever it attaches to next, so input
/// before the attachment's first output is dropped, not queued.
#[tokio::test]
async fn input_before_the_attachment_is_live_is_dropped() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let (addr, mut accepted) = server().await;
        // A short ping interval, because the first ping is the readiness oracle below; the watchdog stays long, so
        // the unanswered ping cannot end the socket during the test.
        let config = ChannelConfig {
            ping_interval: Duration::from_millis(20),
            ..ChannelConfig::default()
        };
        let (handle, mut events) =
            open_terminal(HelmEndpoint::new(addr, "test-secret"), attach("s1"), config);
        let mut socket = accepted.recv().await.unwrap();
        // The server finishes its handshake before the client does, and input the client meets while still opening
        // the socket is dropped for a different reason. Pings are sent only once the socket is being served, so the
        // first one marks the boundary under test: open but not yet attached. Only one ping is outstanding at a time,
        // so no other ping follows it into the frames read below.
        let Some(Ok(Message::Text(ping))) = socket.socket.next().await else {
            panic!("no ping frame")
        };
        assert_eq!(ping.as_str(), r#"{"type":"ping"}"#);
        handle.input(GridInput::Text {
            data: "early".into(),
        });
        // Requests are handled in order, so seeing the resize proves the early input was already handled.
        handle.resize(81, 24);
        let Some(Ok(Message::Text(resize))) = socket.socket.next().await else {
            panic!("the first frame was not the resize")
        };
        assert!(resize.contains("resize"), "{resize}");

        replay(&mut socket.socket).await;
        until_event(&mut events, TerminalEvent::ReplayComplete).await;
        handle.input(GridInput::Text {
            data: "late".into(),
        });
        let Some(Ok(Message::Binary(bytes))) = socket.socket.next().await else {
            panic!("no input frame")
        };
        assert_eq!(
            GridInput::decode(&bytes).unwrap(),
            GridInput::Text {
                data: "late".into()
            }
        );
    })
    .await
    .expect("the stream test finished in time");
}

/// A stream nobody reads is a reconnect worker nobody owns, so dropping the event feed ends the stream and closes
/// its socket even while handles remain. Pings and the watchdog are an hour apart, so nothing but the dropped feed
/// can close the socket within the test's bound.
#[tokio::test]
async fn dropping_the_event_feed_ends_the_stream() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let quiet = ChannelConfig {
            ping_interval: Duration::from_secs(3600),
            watchdog: Duration::from_secs(3600),
            ..ChannelConfig::default()
        };
        let (_handle, events, _accepted, mut socket) = attached(quiet).await;
        drop(events);
        loop {
            match socket.socket.next().await {
                None | Some(Ok(Message::Close(_))) | Some(Err(_)) => break,
                Some(Ok(_)) => {}
            }
        }
    })
    .await
    .expect("the stream closed its socket in time");
}

/// The helm's answer to a ping.
const PONG: &str = r#"{"type":"pong"}"#;

/// A slow GUI must slow the stream, not break it (SPEC.md's slow viewer is the supervisor's to detect). Two mechanisms
/// make that true and each has a failure this test would show: the event queue is bounded, so a caller that stops
/// reading stops the stream reading its socket (an unbounded queue would hold the whole flood); and the stream's
/// watchdog is paused while it waits on the caller, so the caller's delay is not charged to the link (without the
/// pause, the outstanding ping times out and the stream reconnects mid-flood).
#[tokio::test]
async fn a_caller_that_falls_behind_slows_the_stream_instead_of_breaking_it() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let config = ChannelConfig {
            ping_interval: Duration::from_millis(50),
            watchdog: Duration::from_millis(200),
            ..ChannelConfig::default()
        };
        let (_handle, mut events, mut accepted, first) = attached(config).await;
        let (mut sink, mut source) = first.socket.split();
        // Premise: a ping is outstanding before the flood starts, so the watchdog is armed while the caller is behind.
        let Some(Ok(Message::Text(ping))) = source.next().await else {
            panic!("no ping frame")
        };
        assert_eq!(ping.as_str(), r#"{"type":"ping"}"#);
        const FLOOD: usize = EVENT_QUEUE * 2 + 88;
        let frame = GridOutput::HistoryReset.encode().unwrap();
        tokio::spawn(async move {
            for _ in 0..FLOOD {
                sink.send(Message::binary(frame.clone())).await.unwrap();
            }
            // The outstanding ping is answered only behind the flood, and later pings at once.
            sink.send(Message::text(PONG)).await.unwrap();
            while let Some(Ok(message)) = source.next().await {
                if matches!(&message, Message::Text(text) if text.as_str() == r#"{"type":"ping"}"#)
                {
                    let _ = sink.send(Message::text(PONG)).await;
                }
            }
        });

        // The caller deliberately reads nothing for four watchdog periods: the delay under test is the caller's own,
        // and only real time passing can show the watchdog stayed paused through it.
        // sleep-ok: the caller's deliberate stall, four watchdog periods long
        tokio::time::sleep(Duration::from_millis(800)).await;

        // On this single-threaded runtime the stream cannot run while these non-blocking reads loop, so they see
        // exactly what the queue held: full, and no more.
        let mut received = 0;
        while let Some(event) = events.try_recv() {
            assert_eq!(event, TerminalEvent::Output(GridOutput::HistoryReset));
            received += 1;
        }
        assert_eq!(received, EVENT_QUEUE, "the queue held more than its bound");

        // The rest of the flood arrives in order once the caller reads again, with no reconnect in between.
        while received < FLOOD {
            assert_eq!(
                next_event(&mut events).await,
                TerminalEvent::Output(GridOutput::HistoryReset),
                "after {received} frames"
            );
            received += 1;
        }
        assert!(
            accepted.try_recv().is_err(),
            "the stream reconnected while the caller was behind"
        );
    })
    .await
    .expect("the stream test finished in time");
}

/// An attachment that drops before ever answering a ping has not shown the link works; reconnecting at once would let
/// a path that attaches, replays and drops loop on full replays. So only an answered ping earns the immediate
/// reconnect. The backoff here is an hour, so a prompt attach can only come from that rule (or from reconnect-now).
#[tokio::test]
async fn only_an_attachment_that_answered_a_ping_reconnects_at_once() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let config = ChannelConfig {
            ping_interval: Duration::from_millis(20),
            reconnect_initial: Duration::from_secs(3600),
            reconnect_max: Duration::from_secs(3600),
            ..ChannelConfig::default()
        };
        let (handle, mut events, mut accepted, first) = attached(config).await;

        // Attached, never answered a ping: the drop waits the backoff.
        drop(first);
        until_event(&mut events, TerminalEvent::State(StreamState::Reconnecting)).await;
        // sleep-ok: an absence window; the behavior it rules out reconnects within milliseconds.
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(
            accepted.try_recv().is_err(),
            "reconnected at once without an answered ping"
        );

        // The stream is alive and waiting in its backoff: reconnect-now ends the wait.
        handle.reconnect_now();
        let mut second = accepted.recv().await.unwrap();
        replay(&mut second.socket).await;
        until_event(&mut events, TerminalEvent::ReplayComplete).await;

        // This attachment answers its ping. The client sends its next ping only once the previous one is answered, so
        // the second ping proves the pong was taken.
        let Some(Ok(Message::Text(ping))) = second.socket.next().await else {
            panic!("no ping frame")
        };
        assert_eq!(ping.as_str(), r#"{"type":"ping"}"#);
        second.socket.send(Message::text(PONG)).await.unwrap();
        let Some(Ok(Message::Text(ping))) = second.socket.next().await else {
            panic!("no second ping frame")
        };
        assert_eq!(ping.as_str(), r#"{"type":"ping"}"#);

        // The link proved itself, so losing it reconnects at once despite the hour-long backoff.
        drop(second);
        let third = accepted.recv().await.unwrap();
        assert!(third.uri.contains("/term/unowned?"), "{}", third.uri);
    })
    .await
    .expect("the stream test finished in time");
}

/// A terminal that recovered must retry promptly the next time (SPEC.md: retained terminals retry promptly), so a
/// successful attachment resets the backoff. Without the reset, each `other` detach doubles the wait from the initial
/// 100 ms: the sixth would wait 3.2 s. With it every wait is the initial one.
#[tokio::test]
async fn a_successful_attachment_resets_the_backoff() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let config = ChannelConfig {
            reconnect_initial: Duration::from_millis(100),
            reconnect_max: Duration::from_secs(3600),
            ..ChannelConfig::default()
        };
        let (_handle, mut events, mut accepted, mut current) = attached(config).await;
        let notice = r#"{"type":"detached","reason":"host unreachable","code":"other"}"#;
        let mut waits = Vec::new();
        for _ in 0..6 {
            current.socket.send(Message::text(notice)).await.unwrap();
            let started = Instant::now();
            let mut next = accepted.recv().await.unwrap();
            waits.push(started.elapsed());
            replay(&mut next.socket).await;
            until_event(&mut events, TerminalEvent::ReplayComplete).await;
            current = next;
        }
        assert!(
            waits[5] < Duration::from_millis(1600),
            "the backoff kept growing across successful attachments: {waits:?}"
        );
    })
    .await
    .expect("the stream test finished in time");
}
