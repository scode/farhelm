//! A terminal stream against a scripted WebSocket server standing in for the helm's terminal route.
//!
//! A real helm cannot serve a terminal here (that needs a supervisor and a running session), so this server checks
//! the parts the helm checks (path, query and credential subprotocol) and plays the frames a helm would send. The
//! tests use socket and stream events for readiness. Bounded observation windows cover deliberate silence and
//! watchdog deadlines, where waiting for the next event alone would miss the behavior under test.

use super::*;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::handshake::server::{
    ErrorResponse, Request as Upgrade, Response,
};
use tokio_tungstenite::tungstenite::http::StatusCode;

mod catch_up;
mod ladder;
mod parity;

/// A terminal ladder of `retries` waits, then probes every `probe`, all in milliseconds.
fn ladder(retries: &[u64], probe: u64) -> TerminalLadder {
    TerminalLadder {
        retries: retries.iter().copied().map(Duration::from_millis).collect(),
        probe_every: Duration::from_millis(probe),
    }
}

/// The `Reconnecting` event for `attempt` on a ladder of `of` retries, waiting `ms` milliseconds (`None`: in flight).
fn reconnecting(attempt: u32, of: u32, phase: ReconnectPhase, ms: Option<u64>) -> TerminalEvent {
    TerminalEvent::State(StreamState::Reconnecting(ReconnectStatus {
        attempt,
        of,
        phase,
        next_attempt_in: ms.map(Duration::from_millis),
    }))
}

/// One accepted socket and the path-and-query it was opened with.
struct Accepted {
    uri: String,
    socket: WebSocketStream<TcpStream>,
}

/// Accept sockets as a helm would, refusing any upgrade without the device credential.
async fn server() -> (SocketAddr, mpsc::UnboundedReceiver<Accepted>) {
    let (addr, mut connections) = listen().await;
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(tcp) = connections.recv().await {
            if tx.send(accept_upgrade(tcp).await).is_err() {
                break;
            }
        }
    });
    (addr, rx)
}

/// Accept TCP connections and hand them over before any handshake, so the test decides when and how each attach is
/// answered ([`accept_upgrade`], [`refuse_upgrade`]) or fails (by dropping the connection). Until then the stream
/// waits in its connect.
async fn listen() -> (SocketAddr, mpsc::UnboundedReceiver<TcpStream>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Ok((tcp, _)) = listener.accept().await {
            if tx.send(tcp).is_err() {
                break;
            }
        }
    });
    (addr, rx)
}

/// Accept one attach's upgrade as the helm would, after checking the device credential.
async fn accept_upgrade(tcp: TcpStream) -> Accepted {
    let (uri, socket) = handshake(tcp, None).await;
    match (uri, socket) {
        (Some(uri), Ok(socket)) => Accepted { uri, socket },
        (_, Err(error)) => panic!("the upgrade failed: {error}"),
        (None, Ok(_)) => panic!("an upgrade went through without reaching the credential check"),
    }
}

/// Refuse one attach's upgrade with HTTP `status` after checking the device credential, as the helm refuses a revoked
/// credential (401) or a session that is gone (404).
async fn refuse_upgrade(tcp: TcpStream, status: u16) {
    let (_, socket) = handshake(tcp, Some(status)).await;
    assert!(
        socket.is_err(),
        "an upgrade refused with {status} went through"
    );
}

/// The server side of one upgrade: check the credential subprotocol, then answer with `refusal`'s status or accept.
/// Returns the path-and-query the attach asked for (`None` when the handshake ended before the request was read).
async fn handshake(
    tcp: TcpStream,
    refusal: Option<u16>,
) -> (
    Option<String>,
    Result<WebSocketStream<TcpStream>, tokio_tungstenite::tungstenite::Error>,
) {
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
        uri_tx.send(request.uri().to_string()).unwrap();
        if let Some(status) = refusal {
            let mut refused = ErrorResponse::new(None);
            *refused.status_mut() = StatusCode::from_u16(status).unwrap();
            return Err(refused);
        }
        response
            .headers_mut()
            .insert("Sec-WebSocket-Protocol", "farhelm".parse().unwrap());
        Ok(response)
    };
    let socket = tokio_tungstenite::accept_hdr_async(tcp, callback).await;
    (uri_rx.try_recv().ok(), socket)
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

/// The lease is how the supervisor tells this stream's own reattach (replaced quietly) from an attach under another
/// lease, so every attach must carry the same one; and a takeover notice must be answered by attaching again, since in
/// the single-UI app its only source is a window that was just displaced reconnecting a terminal before its Displaced
/// notice arrived, and stopping would leave the window that holds the seat without its terminal (SPEC_impl.md, "The
/// seat: one UI per helm"). Pinned on the wire: every attach (the first, a reconnect after a drop, the reattach after
/// a takeover, and take-control after a stall) goes to the one terminal route with the same lease; a reconnect asks
/// for the latest size; a takeover shows the GUI the ladder's first wait and its attempt, as a `replaced` detach
/// does, and never a `Detached`; and a stall stops the stream until take-control. Every attach is the ordinary one
/// because the app is single-UI.
#[tokio::test]
async fn every_attach_takes_the_one_route_and_a_takeover_reattaches() {
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

        // First attach: the session id stays one segment.
        let mut first = accepted.recv().await.unwrap();
        assert_eq!(
            first.uri,
            format!("/api/sessions/host%3A1%2Fs%201/term?cols=80&rows=24&lease={lease}&tab=t1")
        );
        replay(&mut first.socket).await;
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Attached(Attachment(1)))
        );
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::Output(GridOutput::HistoryReset)
        );
        assert_eq!(next_event(&mut events).await, TerminalEvent::ReplayComplete);

        // Input and resizes reach the helm once attached.
        handle.input(
            Attachment(1),
            vec![GridInput::Text {
                data: "ls\r".into(),
            }],
        );
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

        // A drop reconnects through the same route with the same lease, at the new size (after the ladder's first
        // wait: this attachment never answered a ping).
        drop(first);
        assert_eq!(
            next_event(&mut events).await,
            reconnecting(1, 6, ReconnectPhase::Retrying, Some(500))
        );
        assert_eq!(
            next_event(&mut events).await,
            reconnecting(1, 6, ReconnectPhase::Retrying, None)
        );
        let mut second = accepted.recv().await.unwrap();
        assert_eq!(
            second.uri,
            format!("/api/sessions/host%3A1%2Fs%201/term?cols=100&rows=30&lease={lease}&tab=t1")
        );
        replay(&mut second.socket).await;
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Attached(Attachment(2)))
        );
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::Output(GridOutput::HistoryReset)
        );
        assert_eq!(next_event(&mut events).await, TerminalEvent::ReplayComplete);

        // An attach under another lease took the terminal: the stream waits the ladder's first rung (the attachment
        // was live, so the ladder starts over) and attaches again under its own lease, with no Detached in between.
        let notice = r#"{"type":"detached","reason":"taken over","code":"taken_over"}"#;
        second.socket.send(Message::text(notice)).await.unwrap();
        assert_eq!(
            next_event(&mut events).await,
            reconnecting(1, 6, ReconnectPhase::Retrying, Some(500))
        );
        assert_eq!(
            next_event(&mut events).await,
            reconnecting(1, 6, ReconnectPhase::Retrying, None)
        );
        let mut third = accepted.recv().await.unwrap();
        assert_eq!(
            third.uri,
            format!("/api/sessions/host%3A1%2Fs%201/term?cols=100&rows=30&lease={lease}&tab=t1")
        );
        replay(&mut third.socket).await;
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Attached(Attachment(3)))
        );
        until_event(&mut events, TerminalEvent::ReplayComplete).await;

        // A stall stops the stream, and only the user's take-control attaches again, through the same route with the
        // same lease.
        let notice = r#"{"type":"detached","reason":"too slow","code":"stalled"}"#;
        third.socket.send(Message::text(notice)).await.unwrap();
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Detached {
                code: Some(DetachCode::Stalled),
                reason: "too slow".into(),
                tab_unavailable: false,
            })
        );
        handle.take_control();
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Attaching)
        );
        let fourth = accepted.recv().await.unwrap();
        assert_eq!(
            fourth.uri,
            format!("/api/sessions/host%3A1%2Fs%201/term?cols=100&rows=30&lease={lease}&tab=t1")
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

/// `Closed` is how the GUI learns a stream is over ([`StreamState::Closed`]: a close, or every handle dropped), and the
/// states that wait for the user are where a GUI most often lets a terminal go. The stream once returned from those
/// waits without emitting it, so the feed just ended. Pinned for a refusal ended by `close` and a stalled detach
/// ended by dropping the last handle.
#[tokio::test]
async fn a_stream_closed_while_waiting_for_the_user_still_reports_closed() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let (addr, _accepted) = server().await;
        let (handle, mut events) = open_terminal(
            HelmEndpoint::new(addr, "test-secret"),
            attach(".."),
            ChannelConfig::default(),
        );
        until_event(
            &mut events,
            TerminalEvent::State(StreamState::Refused { status: 404 }),
        )
        .await;
        handle.close();
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Closed)
        );

        let (handle, mut events, _accepted, mut first) = attached(ChannelConfig::default()).await;
        let notice = r#"{"type":"detached","reason":"too slow","code":"stalled"}"#;
        first.socket.send(Message::text(notice)).await.unwrap();
        until_event(
            &mut events,
            TerminalEvent::State(StreamState::Detached {
                code: Some(DetachCode::Stalled),
                reason: "too slow".into(),
                tab_unavailable: false,
            }),
        )
        .await;
        drop(handle);
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Closed)
        );
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

/// Which detach codes stop the stream is the whole reconnect policy (SPEC.md, "Terminal experience"): a closed tab or
/// a stall waits for the user, because reattaching would chase a tab that is gone or walk back into the wedge that
/// stalled it. `other` carries trouble that keeps recovering (a lost supervisor, an unreachable host) and `replaced`
/// is a newer attach under this stream's own lease, so they wait their rung and reattach. `taken_over` does the same,
/// through the same path: in the single-UI app its only source is a window that was just displaced reconnecting a
/// terminal before its Displaced notice arrived, and the window that holds the seat takes the terminal back by
/// attaching again (SPEC_impl.md, "The seat: one UI per helm"). Pinned per code, on the wire: a stopping code shows
/// `Detached`; every other shows the ladder's first wait and then attaches again with the very query it first
/// attached with, so the same lease and size.
#[tokio::test]
async fn each_detach_code_stops_or_reattaches_as_the_policy_says() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let quick = ChannelConfig {
            terminal_ladder: ladder(&[10], 10),
            ..ChannelConfig::default()
        };
        for (code, stops) in [
            ("tab_closed", true),
            ("stalled", true),
            ("replaced", false),
            ("taken_over", false),
            ("other", false),
        ] {
            let (_handle, mut events, mut accepted, mut first) = attached(quick.clone()).await;
            let notice = format!(r#"{{"type":"detached","reason":"why","code":"{code}"}}"#);
            first.socket.send(Message::text(notice)).await.unwrap();
            let expected = if stops {
                let code = serde_json::from_value(serde_json::json!(code)).unwrap();
                TerminalEvent::State(StreamState::Detached {
                    code: Some(code),
                    reason: "why".into(),
                    tab_unavailable: false,
                })
            } else {
                reconnecting(1, 1, ReconnectPhase::Retrying, Some(10))
            };
            assert_eq!(next_event(&mut events).await, expected, "{code}");
            if !stops {
                let again = accepted.recv().await.unwrap();
                assert_eq!(again.uri, first.uri, "{code}: the reattach's query");
            }
        }
    })
    .await
    .expect("the stream test finished in time");
}

/// The "Reconnect now" button must skip the wait (an hour here, so only the request can explain the prompt attach),
/// show the attempt starting with its number kept, and attach through the one terminal route like any other attempt.
#[tokio::test]
async fn reconnect_now_skips_the_wait() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let slow = ChannelConfig {
            terminal_ladder: ladder(&[3_600_000], 3_600_000),
            ..ChannelConfig::default()
        };
        let (handle, mut events, mut accepted, mut first) = attached(slow).await;
        let notice = r#"{"type":"detached","reason":"host unreachable","code":"other"}"#;
        first.socket.send(Message::text(notice)).await.unwrap();
        assert_eq!(
            next_event(&mut events).await,
            reconnecting(1, 1, ReconnectPhase::Retrying, Some(3_600_000))
        );
        handle.reconnect_now();
        assert_eq!(
            next_event(&mut events).await,
            reconnecting(1, 1, ReconnectPhase::Retrying, None)
        );
        let again = accepted.recv().await.unwrap();
        assert!(
            again.uri.starts_with("/api/sessions/s1/term?"),
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
            reconnecting(1, 6, ReconnectPhase::Retrying, Some(500))
        );
        let again = accepted.recv().await.unwrap();
        assert!(
            again.uri.starts_with("/api/sessions/s1/term?"),
            "{}",
            again.uri
        );
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
        // Stamped for the attachment this socket will carry, so only its timing can keep it out.
        handle.input(
            Attachment(1),
            vec![GridInput::Text {
                data: "early".into(),
            }],
        );
        // Requests are handled in order, so seeing the resize proves the early input was already handled.
        handle.resize(81, 24);
        let Some(Ok(Message::Text(resize))) = socket.socket.next().await else {
            panic!("the first frame was not the resize")
        };
        assert!(resize.contains("resize"), "{resize}");

        replay(&mut socket.socket).await;
        until_event(&mut events, TerminalEvent::ReplayComplete).await;
        handle.input(
            Attachment(1),
            vec![GridInput::Text {
                data: "late".into(),
            }],
        );
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

/// Input typed for an attachment that was lost must never reach the next one: the stream may read it only after the
/// new attachment's first output, when it is live again, and without the stamp it would write keystrokes typed at one
/// screen into another. A group stamped for the live attachment arrives whole and in order, one message per part; one
/// stamped for the old attachment arrives not at all.
#[tokio::test]
async fn input_reaches_only_the_attachment_it_was_stamped_for() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let (handle, mut events, mut accepted, first) = attached(ChannelConfig {
            terminal_ladder: ladder(&[10], 10),
            ..ChannelConfig::default()
        })
        .await;
        drop(first);
        let mut second = accepted.recv().await.unwrap();
        replay(&mut second.socket).await;
        until_event(
            &mut events,
            TerminalEvent::State(StreamState::Attached(Attachment(2))),
        )
        .await;
        until_event(&mut events, TerminalEvent::ReplayComplete).await;
        let text = |data: &str| GridInput::Text { data: data.into() };
        handle.input(Attachment(1), vec![text("stale"), text("stale too")]);
        handle.input(Attachment(2), vec![text("one"), text("two")]);
        // Requests are handled in order, so the resize after them shows whether the stale group was written.
        handle.resize(81, 24);
        let mut frames = Vec::new();
        loop {
            match second.socket.next().await {
                Some(Ok(Message::Binary(bytes))) => frames.push(GridInput::decode(&bytes).unwrap()),
                Some(Ok(Message::Text(text))) if text.contains("resize") => break,
                Some(Ok(_)) => {}
                other => panic!("the socket ended before the resize: {other:?}"),
            }
        }
        assert_eq!(frames, [text("one"), text("two")]);
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
/// reconnect, which the stream shows as the first attempt in flight with no wait before it. Every wait here is an
/// hour, so a prompt attach can only come from that rule (or from reconnect-now).
#[tokio::test]
async fn only_an_attachment_that_answered_a_ping_reconnects_at_once() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let config = ChannelConfig {
            ping_interval: Duration::from_millis(20),
            terminal_ladder: ladder(&[3_600_000], 3_600_000),
            ..ChannelConfig::default()
        };
        let (handle, mut events, mut accepted, first) = attached(config).await;

        // Attached, never answered a ping: the drop waits its rung.
        drop(first);
        until_event(
            &mut events,
            reconnecting(1, 1, ReconnectPhase::Retrying, Some(3_600_000)),
        )
        .await;
        // sleep-ok: an absence window; the behavior it rules out reconnects within milliseconds.
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(
            accepted.try_recv().is_err(),
            "reconnected at once without an answered ping"
        );

        // The stream is alive and waiting for its next attempt: reconnect-now ends the wait.
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

        // The link proved itself, so losing it reconnects at once despite the hour-long wait.
        drop(second);
        assert_eq!(
            next_event(&mut events).await,
            reconnecting(1, 1, ReconnectPhase::Retrying, None)
        );
        let third = accepted.recv().await.unwrap();
        assert!(
            third.uri.starts_with("/api/sessions/s1/term?"),
            "{}",
            third.uri
        );
    })
    .await
    .expect("the stream test finished in time");
}

/// A terminal that recovered must retry promptly the next time (SPEC.md: retained terminals retry promptly), so a
/// successful attachment starts the ladder over. The ladder's second rung is an hour, so without the reset the second
/// `other` detach would show attempt 2 and wait it; with it every outage starts at attempt 1 and its 100 ms.
#[tokio::test]
async fn a_successful_attachment_starts_the_ladder_over() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let config = ChannelConfig {
            terminal_ladder: ladder(&[100, 3_600_000], 3_600_000),
            ..ChannelConfig::default()
        };
        let (_handle, mut events, mut accepted, mut current) = attached(config).await;
        let notice = r#"{"type":"detached","reason":"host unreachable","code":"other"}"#;
        for outage in 0..3 {
            current.socket.send(Message::text(notice)).await.unwrap();
            assert_eq!(
                next_event(&mut events).await,
                reconnecting(1, 2, ReconnectPhase::Retrying, Some(100)),
                "outage {outage}"
            );
            let mut next = accepted.recv().await.unwrap();
            replay(&mut next.socket).await;
            until_event(&mut events, TerminalEvent::ReplayComplete).await;
            current = next;
        }
    })
    .await
    .expect("the stream test finished in time");
}

/// What the stream read from a socket before it ended is the caller's, even when the caller was behind at the time: a
/// replay's tail, say, that the stream had taken off the socket and was holding until the caller's queue had room. It
/// reaches the caller, in order and before `Reconnecting`, and handing it over does not end the stream, which goes on
/// to reconnect as after any other drop. Losing it would leave the caller drawing a screen with its last update
/// missing; ending the stream there would leave a terminal that never comes back after a network blip.
///
/// Only a socket that ends while the caller is behind has anything to hand over, and while the caller is behind the
/// stream reads nothing from the socket, so the end has to be found by a write. Here the helm floods one event more
/// than the caller's queue holds and then stops reading, and the caller types far more than the socket's buffers take:
/// the write stalls until the watchdog gives up on the socket, and while it stalls the stream cannot hand the held
/// event over by the ordinary route, however fast the caller reads. Two premises are checked before the caller reads
/// anything: the queue is full (so one event is held), and the typing has reached the helm's side (so the stream took
/// the input while it still held the event, and is in the write). Without the second, the caller could drain the queue
/// before the stream ran, the stream would deliver the held event the ordinary way first, and the test would pass
/// whatever the hand-over did.
#[tokio::test]
async fn events_read_before_a_drop_reach_a_caller_that_was_behind_and_the_stream_reconnects() {
    tokio::time::timeout(Duration::from_secs(60), async {
        // No pings, so nothing but the flood and the input crosses the socket; a short watchdog, which also bounds
        // each write, and a short reconnect wait.
        let config = ChannelConfig {
            ping_interval: Duration::from_secs(3600),
            watchdog: Duration::from_millis(300),
            terminal_ladder: ladder(&[10], 10),
            ..ChannelConfig::default()
        };
        let (handle, mut events, mut accepted, mut first) = attached(config).await;
        // Numbered, so the events show their order and that none is missing or repeated.
        let output = |n: usize| GridOutput::Hello {
            cols: 80,
            rows: 24,
            history_cap: u32::try_from(n).unwrap(),
        };
        let flood = EVENT_QUEUE + 1;
        for n in 0..flood {
            let frame = output(n).encode().unwrap();
            first.socket.send(Message::binary(frame)).await.unwrap();
        }
        // The readiness oracle: the caller's queue is full. The stream read one more frame than that before it stopped
        // reading, and holds it for the caller.
        while events.events.len() < EVENT_QUEUE {
            tokio::task::yield_now().await;
        }
        assert_eq!(
            events.events.len(),
            EVENT_QUEUE,
            "the premise: the queue is full"
        );

        // About 24 MiB of typing, which no loopback socket buffers while its reader is stopped (Linux's default
        // ceilings are 4 MiB to send and 6 MiB to receive). The helm side keeps its socket open and never reads it.
        let part = GridInput::Text {
            data: "x".repeat(16_000),
        };
        handle.input(Attachment(1), vec![part; 1_500]);
        // The stream writes a whole group before it looks at its queue again, so once the helm's side holds some of the
        // typing (peeked, never read: reading would let the write go on) the stream is in that write, holding the
        // event, and stays there until the watchdog ends the socket.
        let mut arrived = vec![0; 16 * 1024];
        while first.socket.get_ref().peek(&mut arrived).await.unwrap() < arrived.len() {
            tokio::task::yield_now().await;
        }

        for n in 0..flood {
            assert_eq!(
                next_event(&mut events).await,
                TerminalEvent::Output(output(n)),
                "event {n} of {flood}"
            );
        }
        // The end comes only from the stalled write. On a host whose socket buffer ceilings were raised far enough to
        // take all the typing, the write never stalls and nothing ends the socket, so say that rather than letting the
        // outer timeout report a bare hang.
        let state = tokio::time::timeout(Duration::from_secs(20), next_event(&mut events))
            .await
            .expect(
                "the premise: the socket ends because the typing stalls its write; it did not, so this host's \
                 socket buffers took all the typing",
            );
        assert_eq!(state, reconnecting(1, 1, ReconnectPhase::Retrying, Some(10)));
        let again = accepted.recv().await.unwrap();
        assert!(again.uri.starts_with("/api/sessions/s1/term?"), "{}", again.uri);
        drop(first);
    })
    .await
    .expect("the stream test finished in time");
}

// ---- Recovery past the first attach --------------------------------------------------------------------------------

/// The helm's side of one attach that went silent: read `socket` up to the stream's ping, which it sends only once it
/// serves the socket, so the stream is past connecting and its watchdog is armed. Panics on anything else first.
async fn read_ping(socket: &mut WebSocketStream<TcpStream>) {
    match socket.next().await {
        Some(Ok(Message::Text(text))) if text.as_str() == r#"{"type":"ping"}"# => {}
        other => panic!("expected the stream's ping, got {other:?}"),
    }
}

/// Read `socket`, which the helm side never closes, until it ends: proof that the stream ended it. Panics on input,
/// which no test here types.
async fn ended_by_the_stream(socket: &mut WebSocketStream<TcpStream>) {
    loop {
        match socket.next().await {
            None | Some(Ok(Message::Close(_)) | Err(_)) => return,
            Some(Ok(Message::Text(_))) => {}
            other => panic!("the stream sent {other:?} on a silent socket"),
        }
    }
}

/// Pings every 20 ms and a 250 ms watchdog, so a silent socket ends well within the test's bound, and three short
/// retries. The watchdog also bounds opening each socket, so it is kept well above a loopback handshake on a busy host.
fn quick_watchdog() -> ChannelConfig {
    ChannelConfig {
        ping_interval: Duration::from_millis(20),
        watchdog: Duration::from_millis(250),
        terminal_ladder: ladder(&[10, 10, 10], 10),
        ..ChannelConfig::default()
    }
}

/// Answer two attaches by accepting their upgrades and then saying nothing, not even a pong, as a helm does while
/// the supervisor has not answered the attach (it reads nothing from the socket until then). Each must end as a
/// failed attempt on the ladder: the GUI is shown attempt `first` and then `first + 1`, each wait and each attempt in
/// flight, with no `Attached` between. The sockets stay open on the helm side, so only the stream's watchdog can have
/// ended them, which each one shows by closing.
async fn two_silent_attaches(
    accepted: &mut mpsc::UnboundedReceiver<Accepted>,
    events: &mut TerminalEvents,
    first: u32,
) {
    let mut silent = Vec::new();
    for attempt in first..first + 2 {
        let mut socket = accepted.recv().await.unwrap();
        read_ping(&mut socket.socket).await;
        assert_eq!(
            next_event(events).await,
            reconnecting(attempt, 3, ReconnectPhase::Retrying, Some(10)),
            "a silent attach is a failed attempt"
        );
        assert_eq!(
            next_event(events).await,
            reconnecting(attempt, 3, ReconnectPhase::Retrying, None)
        );
        silent.push(socket);
    }
    for socket in &mut silent {
        ended_by_the_stream(&mut socket.socket).await;
    }
}

/// Old `terminal-replay-rename.spec.ts`, case 11, "replay-unconnected: a stalled first mount retries on the reconnect
/// ladder". Why: a helm can accept a terminal's upgrade and then stall it (the helm reads nothing from the socket until
/// the supervisor answers the attach), and a terminal that waited on that forever would sit on its first mount with
/// nothing to say why; the old app learned this from an island-cap burst. The silence is the stream's to notice, since
/// the helm answers nothing then, not even a ping.
///
/// Spec: a stream whose first two attaches are accepted and then silent shows `Attaching`, then attempt 1's wait and
/// attempt in flight, then attempt 2's, as each attach's unanswered ping times out; each silent socket is closed by
/// the stream; the third attach replays and is the stream's first attachment (`Attached(1)`), live.
#[tokio::test]
async fn a_first_attach_the_helm_accepts_and_never_answers_is_retried_on_the_ladder() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let (addr, mut accepted) = server().await;
        let (_handle, mut events) = open_terminal(
            HelmEndpoint::new(addr, "test-secret"),
            attach("s1"),
            quick_watchdog(),
        );
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Attaching)
        );
        two_silent_attaches(&mut accepted, &mut events, 1).await;

        let mut answered = accepted.recv().await.unwrap();
        replay(&mut answered.socket).await;
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Attached(Attachment(1)))
        );
        until_event(&mut events, TerminalEvent::ReplayComplete).await;
    })
    .await
    .expect("the stream test finished in time");
}

/// Old `terminal-reconnect.spec.ts`, "silent-attach-is-a-failed-attempt-not-a-recovery". Why: during a recovery, an
/// attach that the helm accepts but that never replays must not end the outage or start the ladder over, or a helm
/// that keeps stalling attaches would hold the terminal on attempt 1 behind the card forever, never reaching the probes.
///
/// Spec: after a live attachment drops, the GUI is shown attempt 1; two reattaches accepted and then silent (no pong,
/// no output) each fail by the stream's watchdog, showing attempts 2 and 3 in turn rather than attempt 1 again, and
/// the stream closes each; the next reattach replays and is the stream's second attachment (`Attached(2)`), live.
#[tokio::test]
async fn a_reattach_the_helm_accepts_and_never_answers_is_a_failed_attempt() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let (_handle, mut events, mut accepted, first) = attached(quick_watchdog()).await;
        drop(first);
        assert_eq!(
            next_event(&mut events).await,
            reconnecting(1, 3, ReconnectPhase::Retrying, Some(10))
        );
        assert_eq!(
            next_event(&mut events).await,
            reconnecting(1, 3, ReconnectPhase::Retrying, None)
        );
        two_silent_attaches(&mut accepted, &mut events, 2).await;

        let mut answered = accepted.recv().await.unwrap();
        replay(&mut answered.socket).await;
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Attached(Attachment(2)))
        );
        until_event(&mut events, TerminalEvent::ReplayComplete).await;
    })
    .await
    .expect("the stream test finished in time");
}

/// Old `terminal-reconnect.spec.ts`, "background-probes-recover-without-a-click". Why: SPEC.md, "Terminal experience",
/// has a terminal whose network comes back overnight "simply there again": past the bounded retries, a probe that
/// finds the helm must attach by itself, with nobody pressing Reconnect now, and the recovered terminal must retry
/// promptly the next time rather than resume probing every 30 s. The probes past the sixth rung are reached here over
/// real sockets, each retry failing as a real connection that drops during its handshake.
///
/// Spec: on a ladder of six 5 ms retries and a 20 ms probe, after a live attachment drops and six reattaches fail,
/// the GUI is shown attempt 7 waiting and then in flight as a probe; that probe's attach replays and becomes
/// `Attached(2)` with no request on the handle; when it drops, the GUI is shown attempt 1 of the retries again.
#[tokio::test]
async fn a_probe_past_the_retries_recovers_by_itself_and_starts_the_ladder_over() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let config = ChannelConfig {
            terminal_ladder: ladder(&[5, 5, 5, 5, 5, 5], 20),
            ..ChannelConfig::default()
        };
        let (addr, mut connections) = listen().await;
        // Never used: the recovery must need no request.
        let (_handle, mut events) =
            open_terminal(HelmEndpoint::new(addr, "test-secret"), attach("s1"), config);
        let mut first = accept_upgrade(connections.recv().await.unwrap()).await;
        replay(&mut first.socket).await;
        until_event(&mut events, TerminalEvent::ReplayComplete).await;

        drop(first);
        for attempt in 1..=6 {
            assert_eq!(
                next_event(&mut events).await,
                reconnecting(attempt, 6, ReconnectPhase::Retrying, Some(5))
            );
            assert_eq!(
                next_event(&mut events).await,
                reconnecting(attempt, 6, ReconnectPhase::Retrying, None)
            );
            drop(connections.recv().await.unwrap());
        }
        assert_eq!(
            next_event(&mut events).await,
            reconnecting(7, 6, ReconnectPhase::Probing, Some(20))
        );
        assert_eq!(
            next_event(&mut events).await,
            reconnecting(7, 6, ReconnectPhase::Probing, None)
        );
        let mut probe = accept_upgrade(connections.recv().await.unwrap()).await;
        replay(&mut probe.socket).await;
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Attached(Attachment(2)))
        );
        until_event(&mut events, TerminalEvent::ReplayComplete).await;

        drop(probe);
        assert_eq!(
            next_event(&mut events).await,
            reconnecting(1, 6, ReconnectPhase::Retrying, Some(5)),
            "the ladder started over"
        );
    })
    .await
    .expect("the stream test finished in time");
}

/// Old `terminal-replay-rename.spec.ts`, case 13, "replay-starved: a watchdog expiry that fires late does not end a
/// live catch-up". Why: the watchdog measures silence, and a stream whose runtime was held busy heard none. When the
/// stream next runs, its deadline has passed and the helm's answer is already waiting in the socket; if the expiry
/// won, a healthy attachment the helm had accepted would be thrown away and replayed again (the old app hit this in
/// WebKit when one task built 33 terminals).
///
/// Spec: with a ping outstanding on the first attach, the helm answers it and sends the replay, and the runtime's only
/// thread is then blocked for four watchdog periods; afterwards the GUI is shown `Attached(1)`, the replay and its end,
/// and the stream sends its next ping on that same socket (it pings again only once the last was answered), having
/// opened no other.
#[tokio::test]
async fn a_watchdog_overdue_behind_a_starved_runtime_does_not_end_an_answered_attach() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let (addr, mut accepted) = server().await;
        let (_handle, mut events) = open_terminal(
            HelmEndpoint::new(addr, "test-secret"),
            attach("s1"),
            quick_watchdog(),
        );
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Attaching)
        );
        let mut socket = accepted.recv().await.unwrap();
        // The premise: a ping is outstanding, so the watchdog is armed.
        read_ping(&mut socket.socket).await;
        socket.socket.send(Message::text(PONG)).await.unwrap();
        replay(&mut socket.socket).await;
        // The test runs on a single-threaded runtime, so blocking its thread keeps the stream from running while its
        // watchdog's deadline passes with the helm's answer already in its socket.
        // sleep-ok: the starvation under test, four watchdog periods long
        std::thread::sleep(Duration::from_millis(1_000));

        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::State(StreamState::Attached(Attachment(1)))
        );
        assert_eq!(
            next_event(&mut events).await,
            TerminalEvent::Output(GridOutput::HistoryReset)
        );
        assert_eq!(next_event(&mut events).await, TerminalEvent::ReplayComplete);
        read_ping(&mut socket.socket).await;
        // Only a redial that already happened shows here; the events above are what rule out a reconnect.
        assert!(
            accepted.try_recv().is_err(),
            "the stream had already opened another socket"
        );
    })
    .await
    .expect("the stream test finished in time");
}
