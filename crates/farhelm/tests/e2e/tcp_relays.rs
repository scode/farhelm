//! Real loopback sockets through the production supervisor connection. These
//! tests keep requests open while waiting for replies, so filling a file-size
//! frame or treating TCP EOF as a required request delimiter cannot pass.

use crate::harness::*;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;
use tokio::time::timeout;

/// Read exactly the HTTP header boundary without swallowing the first
/// websocket frame. The cap and deadline retain bounded evidence on failure.
async fn headers(socket: &mut TcpStream) -> Vec<u8> {
    timeout(Duration::from_secs(10), async {
        let mut header = Vec::new();
        while !header.ends_with(b"\r\n\r\n") {
            assert!(header.len() < 8192, "fixture HTTP header exceeded its cap");
            let mut byte = [0];
            socket
                .read_exact(&mut byte)
                .await
                .expect("peer closed before header boundary");
            header.extend(byte);
        }
        header
    })
    .await
    .expect("live TCP peer did not complete its HTTP header")
}

/// Listen on an owned random port, with one accepted socket routed through
/// the production client. JoinSet cancels socket tasks if an assertion panics.
async fn forwarded_listener(
    tasks: &mut JoinSet<()>,
    client: Arc<SupervisorClient>,
    session: String,
    address: Option<IpAddr>,
    port: u16,
) -> SocketAddr {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let local = listener.local_addr().unwrap();
    assert!(local.ip().is_loopback() && local.port() != 0);
    tasks.spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        client
            .relay_tcp(&session, address, port, socket)
            .await
            .unwrap();
    });
    local
}

/// Join only owned tasks and retain their panic diagnostics. A stalled socket
/// fails within the test's observation budget rather than hanging the suite.
async fn finish(tasks: &mut JoinSet<()>) {
    timeout(Duration::from_secs(10), async {
        while let Some(result) = tasks.join_next().await {
            result.expect("owned relay/socket fixture task failed");
        }
    })
    .await
    .expect("owned relay/socket tasks did not terminate");
}

/// A short GET must reach a real HTTP server before either side closes its
/// request stream. The HTTP client parses the reply through the same channel.
#[farhelm_testtrace::test]
async fn short_http_request_receives_response_without_half_closing() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let remote = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let port = remote.local_addr().unwrap().port();
    let mut tasks = JoinSet::new();
    tasks.spawn(async move {
        let (mut socket, _) = remote.accept().await.unwrap();
        assert!(
            headers(&mut socket)
                .await
                .starts_with(b"GET /through-relay HTTP/1.1\r\n")
        );
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello")
            .await
            .unwrap();
    });
    let local = forwarded_listener(&mut tasks, Arc::clone(&h.client), session.id, None, port).await;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();
    let response = client
        .get(format!("http://{local}/through-relay"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(response.text().await.unwrap(), "hello");
    finish(&mut tasks).await;
}

/// A valid websocket handshake and masked text exchange use a server bound
/// only to ::1. Failure of IPv4 on the exact chosen port establishes that
/// success came from the supervisor's fallback, not a second IPv4 listener.
#[farhelm_testtrace::test]
async fn websocket_round_trip_uses_ipv6_loopback_fallback() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let remote = TcpListener::bind((Ipv6Addr::LOCALHOST, 0)).await.unwrap();
    let remote_address = remote.local_addr().unwrap();
    assert_eq!(remote_address.ip(), IpAddr::V6(Ipv6Addr::LOCALHOST));
    assert!(
        TcpStream::connect((Ipv4Addr::LOCALHOST, remote_address.port()))
            .await
            .is_err(),
        "the IPv6-only fixture unexpectedly has an IPv4 listener"
    );
    let mut tasks = JoinSet::new();
    tasks.spawn(async move {
        let (mut socket, _) = remote.accept().await.unwrap();
        let request = headers(&mut socket).await;
        assert!(request.starts_with(b"GET /socket HTTP/1.1\r\n"));
        assert!(String::from_utf8_lossy(&request).contains("Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ=="));
        // RFC 6455's published key/accept pair avoids a second websocket
        // dependency. This fixture speaks a real masked client text frame.
        socket.write_all(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n").await.unwrap();
        let mut frame = [0; 11];
        socket.read_exact(&mut frame).await.unwrap();
        assert_eq!(&frame[..6], &[0x81, 0x85, 1, 2, 3, 4]);
        let decoded: Vec<u8> = frame[6..].iter().enumerate().map(|(i, byte)| byte ^ frame[2 + i % 4]).collect();
        assert_eq!(decoded, b"hello");
        socket.write_all(b"\x81\x05hello").await.unwrap();
    });
    let local = forwarded_listener(
        &mut tasks,
        Arc::clone(&h.client),
        session.id,
        None,
        remote_address.port(),
    )
    .await;
    let mut socket = TcpStream::connect(local).await.unwrap();
    socket.write_all(b"GET /socket HTTP/1.1\r\nHost: relay\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n").await.unwrap();
    assert!(
        headers(&mut socket)
            .await
            .starts_with(b"HTTP/1.1 101 Switching Protocols\r\n")
    );
    socket
        .write_all(&[
            0x81,
            0x85,
            1,
            2,
            3,
            4,
            b'h' ^ 1,
            b'e' ^ 2,
            b'l' ^ 3,
            b'l' ^ 4,
            b'o' ^ 1,
        ])
        .await
        .unwrap();
    let mut frame = [0; 7];
    timeout(Duration::from_secs(10), socket.read_exact(&mut frame))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&frame, b"\x81\x05hello");
    drop(socket);
    finish(&mut tasks).await;
}

/// Both directions exceed the initial credit window. A reader and writer run
/// concurrently at each socket, so a stopped direction cannot hide behind
/// buffering an entire test payload in the protocol demultiplexer.
#[farhelm_testtrace::test]
async fn bidirectional_tcp_bytes_cross_more_than_one_credit_window() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let remote = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let port = remote.local_addr().unwrap().port();
    let size = farhelm_proto::UPLOAD_WINDOW_BYTES as usize + 12345;
    let mut tasks = JoinSet::new();
    tasks.spawn(async move {
        let (mut socket, _) = remote.accept().await.unwrap();
        let (mut read, mut write) = socket.split();
        let mut received = vec![0; size];
        let reply = vec![0x42; size];
        tokio::try_join!(read.read_exact(&mut received), write.write_all(&reply)).unwrap();
        assert_eq!(received, vec![0x41; size]);
        // A final byte proves the server read all forward bytes before EOF.
        write.write_all(b"!").await.unwrap();
    });
    let local = forwarded_listener(
        &mut tasks,
        Arc::clone(&h.client),
        session.id,
        Some(IpAddr::V4(Ipv4Addr::LOCALHOST)),
        port,
    )
    .await;
    let mut socket = TcpStream::connect(local).await.unwrap();
    timeout(Duration::from_secs(15), async {
        let (mut read, mut write) = socket.split();
        let payload = vec![0x41; size];
        let mut received = vec![0; size];
        tokio::try_join!(read.read_exact(&mut received), write.write_all(&payload)).unwrap();
        assert_eq!(received, vec![0x42; size]);
        let mut confirmation = [0];
        read.read_exact(&mut confirmation).await.unwrap();
        assert_eq!(confirmation, [b'!']);
    })
    .await
    .expect("bidirectional credit windows did not advance");
    drop(socket);
    finish(&mut tasks).await;
}

/// Target validation happens on the supervisor, even for a peer bypassing
/// the helm API. Refusals preserve request correlation and leave other
/// operations on the same multiplexed connection usable.
#[farhelm_testtrace::test]
async fn supervisor_refuses_non_loopback_zero_port_and_missing_session() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let mut peer = RawPeer::connect(&h.sup).await;
    for (req_id, session_id, address, port, expected) in [
        (
            1,
            session.id.as_str(),
            Some(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1))),
            1234,
            ErrorKind::InvalidRequest,
        ),
        (2, session.id.as_str(), None, 0, ErrorKind::InvalidRequest),
        (3, "missing-session", None, 1234, ErrorKind::NotFound),
    ] {
        peer.control(&ControlMsg::BeginTcpRelay {
            req_id,
            session_id: session_id.into(),
            channel: req_id as u32,
            address,
            port,
        })
        .await;
        assert!(matches!(peer.next_control(5).await, ControlMsg::Error {
            req_id: id, kind, ..
        } if id == req_id && kind == expected));
    }
    peer.control(&ControlMsg::ListSessions { req_id: 4 }).await;
    assert!(
        matches!(peer.next_control(5).await, ControlMsg::SessionList { req_id: 4, sessions, .. }
        if sessions.iter().any(|row| row.id == session.id))
    );
}

/// Cancellation closes the actual host socket. Its id remains reserved for
/// every channel type, because data already queued before close can outlive
/// the task which produced it. Late data is discarded without becoming input.
#[farhelm_testtrace::test]
async fn cancelled_tcp_channel_cannot_become_a_file_or_terminal_stream() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let remote = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let port = remote.local_addr().unwrap().port();
    let mut peer = RawPeer::connect(&h.sup).await;
    peer.control(&ControlMsg::BeginTcpRelay {
        req_id: 1,
        session_id: session.id.clone(),
        channel: 7,
        address: None,
        port,
    })
    .await;
    assert!(matches!(
        peer.next_control(5).await,
        ControlMsg::TcpRelayStarted {
            req_id: 1,
            channel: 7
        }
    ));
    let (mut socket, _) = timeout(Duration::from_secs(5), remote.accept())
        .await
        .unwrap()
        .unwrap();
    peer.control(&ControlMsg::CloseTcpRelay { channel: 7 })
        .await;
    let mut byte = [0];
    assert_eq!(
        timeout(Duration::from_secs(5), socket.read(&mut byte))
            .await
            .unwrap()
            .unwrap(),
        0
    );
    peer.data(7, b"late data".to_vec()).await;
    for request in [
        ControlMsg::BeginTcpRelay {
            req_id: 2,
            session_id: session.id.clone(),
            channel: 7,
            address: None,
            port,
        },
        ControlMsg::BeginDownload {
            req_id: 3,
            session_id: session.id.clone(),
            path: "unused".into(),
            channel: 7,
        },
        ControlMsg::BeginUpload {
            req_id: 4,
            session_id: session.id.clone(),
            filename: "unused".into(),
            size: 0,
            channel: 7,
        },
        ControlMsg::Attach {
            req_id: 5,
            session_id: session.id.clone(),
            channel: 7,
            cols: 80,
            rows: 24,
            terminal: TerminalSelector::Agent,
            lease: "".into(),
            if_unowned: false,
        },
    ] {
        let req_id = request.request_req_id().unwrap();
        peer.control(&request).await;
        assert!(matches!(peer.next_control(5).await, ControlMsg::Error {
            req_id: id, kind: ErrorKind::InvalidRequest, ..
        } if id == req_id));
    }
    peer.control(&ControlMsg::ListSessions { req_id: 6 }).await;
    assert!(matches!(
        peer.next_control(5).await,
        ControlMsg::SessionList { req_id: 6, .. }
    ));
}
