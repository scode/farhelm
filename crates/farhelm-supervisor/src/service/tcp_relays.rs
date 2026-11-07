//! Helm-only loopback TCP channels, carried over the existing multiplexed link.
//! The connection owns each task through a route whose drop cancels it, even
//! while connecting or waiting for credit. Data and EOF share the bulk FIFO;
//! tiny credit acknowledgements use the existing priority queue.

use super::connection::reply_frame;
use super::core::{RequestError, Supervisor, error_kind};
use farhelm_proto::tcp_relay::TcpRelayState;
use farhelm_proto::{ControlMsg, ErrorKind, Frame, MAX_TCP_TUNNEL_PORTS, UPLOAD_CHUNK_BYTES};
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpSocket, TcpStream};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc};

/// Connection-owned listeners and undecided sockets. Drop closes pending
/// sockets synchronously and cancels accept loops even if the reader unwinds.
/// A failed claim installs nothing, so a later request can retry it.
pub(crate) struct TunnelClaims {
    claimed: bool,
    listeners: tokio::task::JoinSet<()>,
    pending: Arc<Mutex<PendingTunnels>>,
}

/// Id allocation and insertion share a lock so a dropped connection cannot
/// acquire a new pending socket after its synchronous cleanup has run.
struct PendingTunnels {
    next_id: Option<u64>,
    closed: bool,
    sockets: HashMap<u64, AcceptedSocket>,
    capacity_refusals: u64,
    accept_failures: u64,
    id_refusals: u64,
}

/// The permit follows the socket from pending admission through relay exit.
/// Removing a pending socket cannot briefly free its host-wide budget slot.
pub(crate) struct AcceptedSocket {
    socket: TcpStream,
    _permit: OwnedSemaphorePermit,
}

impl Default for TunnelClaims {
    fn default() -> Self {
        Self {
            claimed: false,
            listeners: tokio::task::JoinSet::new(),
            pending: Arc::new(Mutex::new(PendingTunnels {
                next_id: Some(1),
                closed: false,
                sockets: HashMap::new(),
                capacity_refusals: 0,
                accept_failures: 0,
                id_refusals: 0,
            })),
        }
    }
}

impl Drop for TunnelClaims {
    fn drop(&mut self) {
        self.close();
    }
}

impl TunnelClaims {
    /// Bind the entire claim before publishing any accept task. A successful
    /// empty claim still spends this connection's single successful claim.
    /// Binding uses only IPv4 loopback; the peer never supplies an address.
    pub(crate) fn claim(
        &mut self,
        ports: Vec<u16>,
        budget: Arc<Semaphore>,
        events: mpsc::Sender<Frame>,
    ) -> anyhow::Result<()> {
        if self.claimed {
            return Err(
                RequestError::new(ErrorKind::Conflict, "TCP tunnel ports already claimed").into(),
            );
        }
        if ports.len() > MAX_TCP_TUNNEL_PORTS {
            return Err(RequestError::new(
                ErrorKind::InvalidRequest,
                "TCP tunnel claim exceeds the 32-port limit",
            )
            .into());
        }
        let unique: HashSet<_> = ports.iter().copied().collect();
        if unique.len() != ports.len() || ports.iter().any(|port| *port == 0 || *port == 22) {
            return Err(RequestError::new(
                ErrorKind::InvalidRequest,
                "TCP tunnel ports must be unique, nonzero, not 22, and at most 32",
            )
            .into());
        }
        let mut listeners = Vec::with_capacity(ports.len());
        for port in ports {
            // Linux reuse excludes every live listener, including wildcard
            // binds, while permitting a reconnect past closed TCP remnants.
            // BSD-derived stacks can instead shadow a wildcard listener, so
            // keep reuse off there even if a closed socket delays a retry.
            let socket = TcpSocket::new_v4()?;
            socket.set_reuseaddr(cfg!(target_os = "linux"))?;
            socket
                .bind((Ipv4Addr::LOCALHOST, port).into())
                .map_err(|error| {
                    RequestError::new(
                        ErrorKind::Conflict,
                        format!("cannot bind TCP tunnel port {port}: {error}"),
                    )
                })?;
            listeners.push((port, socket.listen(1024)?));
        }
        self.claimed = true;
        for (port, listener) in listeners {
            let pending = Arc::clone(&self.pending);
            let budget = Arc::clone(&budget);
            let events = events.clone();
            self.listeners.spawn(async move {
                loop {
                    let socket = match listener.accept().await {
                        Ok((socket, _)) => socket,
                        Err(error) => {
                            // A reset racing accept does not invalidate the
                            // listener. Other errors, including descriptor
                            // exhaustion, need a pause to avoid a readable-fd
                            // retry loop while preserving the successful claim.
                            if !matches!(
                                error.kind(),
                                std::io::ErrorKind::Interrupted
                                    | std::io::ErrorKind::WouldBlock
                                    | std::io::ErrorKind::ConnectionAborted
                                    | std::io::ErrorKind::ConnectionReset
                            ) {
                                let first = {
                                    let mut pending = pending.lock().expect("TCP tunnel mutex poisoned");
                                    pending.accept_failures = pending.accept_failures.saturating_add(1);
                                    pending.accept_failures == 1
                                };
                                if first {
                                    tracing::warn!(port, %error, "TCP tunnel accept failed; retrying (further failures counted until claim ends)");
                                }
                                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                            }
                            continue;
                        }
                    };
                    let Ok(permit) = Arc::clone(&budget).try_acquire_owned() else {
                        let first = {
                            let mut pending = pending.lock().expect("TCP tunnel mutex poisoned");
                            pending.capacity_refusals = pending.capacity_refusals.saturating_add(1);
                            pending.capacity_refusals == 1
                        };
                        if first {
                            tracing::warn!(port, "TCP tunnel connection refused: host connection limit reached (further refusals counted until claim ends)");
                        }
                        drop(socket);
                        continue;
                    };
                    let accepted = {
                        let mut pending = pending.lock().expect("TCP tunnel mutex poisoned");
                        if pending.closed {
                            return;
                        }
                        let Some(id) = pending.next_id else {
                            pending.id_refusals = pending.id_refusals.saturating_add(1);
                            if pending.id_refusals == 1 {
                                tracing::warn!(port, "TCP tunnel connection refused: accepted id space exhausted (further refusals counted until claim ends)");
                            }
                            continue;
                        };
                        pending.next_id = id.checked_add(1);
                        pending.sockets.insert(
                            id,
                            AcceptedSocket {
                                socket,
                                _permit: permit,
                            },
                        );
                        id
                    };
                    if events
                        .send(Frame::control(&ControlMsg::TcpTunnelAccepted {
                            accepted,
                            port,
                        }))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
            });
        }
        Ok(())
    }

    /// Consume only a socket accepted by this connection. Refusal and relay
    /// admission use the same removal, preventing a second answer from acting
    /// on an already-admitted socket.
    pub(crate) fn take(&mut self, accepted: u64) -> Option<AcceptedSocket> {
        self.pending
            .lock()
            .expect("TCP tunnel mutex poisoned")
            .sockets
            .remove(&accepted)
    }

    /// End tunnel ownership before connection teardown can wait on unrelated
    /// attachments or requests. Drop repeats this safely on cancellation.
    pub(crate) fn close(&mut self) {
        let mut pending = self.pending.lock().expect("TCP tunnel mutex poisoned");
        // A sandbox program can connect indefinitely. Report each reason once
        // per claim, then one bounded summary instead of logging its traffic.
        if !pending.closed
            && (pending.capacity_refusals > 1
                || pending.accept_failures > 1
                || pending.id_refusals > 1)
        {
            tracing::info!(
                capacity_refusals = pending.capacity_refusals,
                accept_failures = pending.accept_failures,
                id_refusals = pending.id_refusals,
                "TCP tunnel claim ended after repeated refusals or accept failures"
            );
        }
        pending.closed = true;
        pending.sockets.clear();
        self.listeners.abort_all();
    }

    /// Reap cancelled accept loops before waiting for other connection work,
    /// proving their listening descriptors have closed on orderly teardown.
    pub(crate) async fn shutdown(&mut self) {
        self.close();
        while self.listeners.join_next().await.is_some() {}
    }
}

/// The reader retains this route until completion or cancellation. Channel
/// reservation is separate and survives the route, because old bulk frames
/// can still be queued after the producer is gone.
pub(crate) struct RelayRoute {
    pub(crate) state: Arc<TcpRelayState>,
    pub(crate) task: tokio::task::AbortHandle,
}

impl Drop for RelayRoute {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Fully parsed admission parameters. Address validation remains host-side;
/// a helm must not turn this operation into an arbitrary network dialer.
pub(crate) struct RelayRequest {
    pub(crate) req_id: u64,
    pub(crate) session_id: String,
    pub(crate) channel: u32,
    pub(crate) address: Option<IpAddr>,
    pub(crate) port: u16,
}

/// Register state before spawning, so immediate credit or cancellation can
/// find the owner. Slow connect and socket I/O never occupy the shared reader.
pub(crate) fn start(
    tasks: &mut tokio::task::JoinSet<()>,
    sup: Arc<Supervisor>,
    bulk: mpsc::Sender<Frame>,
    priority: mpsc::Sender<Frame>,
    request: RelayRequest,
) -> RelayRoute {
    let state = Arc::new(TcpRelayState::default());
    let owned = Arc::clone(&state);
    let task = tasks.spawn(async move {
        let opened = async {
            if sup.store.session(&request.session_id).await?.is_none() {
                return Err(RequestError::new(ErrorKind::NotFound, "session not found").into());
            }
            connect_loopback(request.address, request.port).await
        };
        let socket = tokio::select! {
            result = opened => result,
            reason = owned.failed() => Err(anyhow::anyhow!(reason)),
        };
        let socket = match socket {
            Ok(socket) => socket,
            Err(error) => {
                let _ = bulk
                    .send(reply_frame(&ControlMsg::Error {
                        create_unaccepted: false,
                        req_id: request.req_id,
                        kind: error_kind(&error),
                        message: format!("{error:#}"),
                    }))
                    .await;
                return;
            }
        };
        if bulk
            .send(Frame::control(&ControlMsg::TcpRelayStarted {
                req_id: request.req_id,
                channel: request.channel,
            }))
            .await
            .is_err()
        {
            return;
        }
        let result = bridge(socket, request.channel, &owned, &bulk, &priority).await;
        owned.finish(result.map_err(|error| error.to_string()));
        let _ = bulk
            .send(Frame::control(&ControlMsg::TcpRelayEnded {
                channel: request.channel,
                reason: owned.outcome().and_then(Result::err),
            }))
            .await;
    });
    RelayRoute { state, task }
}

/// No DNS is involved. Wildcard spellings mean the same two loopback attempts
/// as localhost; other literals must already be loopback. IPv4-first preserves
/// ordinary dev-server behavior while allowing an IPv6-only server to work.
async fn connect_loopback(address: Option<IpAddr>, port: u16) -> anyhow::Result<TcpStream> {
    if port == 0 || address.is_some_and(|ip| !ip.is_loopback() && !ip.is_unspecified()) {
        return Err(RequestError::new(
            ErrorKind::InvalidRequest,
            "TCP relay target must be loopback with a nonzero port",
        )
        .into());
    }
    if let Some(address) = address.filter(|ip| !ip.is_unspecified()) {
        return Ok(TcpStream::connect((address, port)).await?);
    }
    match TcpStream::connect((Ipv4Addr::LOCALHOST, port)).await {
        Ok(socket) => Ok(socket),
        Err(_) => Ok(TcpStream::connect((Ipv6Addr::LOCALHOST, port)).await?),
    }
}

/// Each direction closes independently; only both EOFs end the channel. The pumps are
/// borrowed futures, so cancelling their owner drops both socket halves rather
/// than leaving detached tasks behind. A peer error also cancels blocked writes.
async fn bridge(
    mut socket: TcpStream,
    channel: u32,
    state: &TcpRelayState,
    bulk: &mpsc::Sender<Frame>,
    priority: &mpsc::Sender<Frame>,
) -> anyhow::Result<()> {
    let (mut read, mut write) = socket.split();
    let outgoing = async {
        let mut bytes = vec![0; UPLOAD_CHUNK_BYTES];
        loop {
            let count = read.read(&mut bytes).await?;
            if count == 0 {
                bulk.send(Frame::control(&ControlMsg::TcpRelayEof { channel }))
                    .await
                    .map_err(|_| anyhow::anyhow!("supervisor connection closed"))?;
                return Ok::<(), anyhow::Error>(());
            }
            state.reserve(count).await.map_err(anyhow::Error::msg)?;
            bulk.send(Frame::data(channel, bytes[..count].to_vec()))
                .await
                .map_err(|_| anyhow::anyhow!("supervisor connection closed"))?;
        }
    };
    let incoming = async {
        while let Some(bytes) = state.next().await.map_err(anyhow::Error::msg)? {
            write.write_all(&bytes).await?;
            let received = state.written(bytes.len());
            priority
                .send(Frame::control(&ControlMsg::TcpRelayAck {
                    channel,
                    received,
                }))
                .await
                .map_err(|_| anyhow::anyhow!("supervisor connection closed"))?;
        }
        write.shutdown().await?;
        Ok::<(), anyhow::Error>(())
    };
    tokio::select! {
        result = async { tokio::try_join!(outgoing, incoming).map(|_| ()) } => result,
        reason = state.failed() => Err(anyhow::anyhow!(reason)),
    }
}

/// An accepted socket uses exactly the same pumps and wire ordering as an
/// outgoing loopback relay. Its budget permit stays owned until both socket
/// directions end, failure occurs, or its connection cancels the route.
pub(crate) fn start_accepted(
    tasks: &mut tokio::task::JoinSet<()>,
    accepted: AcceptedSocket,
    bulk: mpsc::Sender<Frame>,
    priority: mpsc::Sender<Frame>,
    req_id: u64,
    channel: u32,
) -> RelayRoute {
    let state = Arc::new(TcpRelayState::default());
    let owned = Arc::clone(&state);
    let task = tasks.spawn(async move {
        let AcceptedSocket { socket, _permit } = accepted;
        if bulk
            .send(Frame::control(&ControlMsg::TcpRelayStarted {
                req_id,
                channel,
            }))
            .await
            .is_err()
        {
            return;
        }
        let result = bridge(socket, channel, &owned, &bulk, &priority).await;
        owned.finish(result.map_err(|error| error.to_string()));
        let _ = bulk
            .send(Frame::control(&ControlMsg::TcpRelayEnded {
                channel,
                reason: owned.outcome().and_then(Result::err),
            }))
            .await;
        drop(_permit);
    });
    RelayRoute { state, task }
}

#[cfg(test)]
mod tests {
    use super::super::core::tests::{StateDir, dummy_exe};
    use super::*;
    use farhelm_proto::MAX_TCP_TUNNEL_CONNECTIONS;
    use farhelm_proto::io::{FrameReader, FrameWriter};
    use std::time::Duration;
    use tokio::net::TcpListener;

    /// These tests require no fixed host port: the kernel chooses a candidate,
    /// which the claim immediately rebinds under its production validation.
    fn port_reservation() -> std::net::TcpListener {
        std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap()
    }

    /// A broken pump must fail this owned fixture within a finite observation
    /// window rather than leave a test parked on its frame receiver forever.
    async fn next_frame(rx: &mut mpsc::Receiver<Frame>) -> Frame {
        tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("the owned relay must make frame progress")
            .expect("relay frame channel open")
    }

    /// A received event is the readiness boundary for a pending socket; a TCP
    /// connect alone would establish only the kernel's accept backlog.
    async fn accepted(rx: &mut mpsc::Receiver<Frame>, port: u16) -> u64 {
        let frame = next_frame(rx).await;
        let event: ControlMsg = serde_json::from_slice(&frame.body).unwrap();
        match event {
            ControlMsg::TcpTunnelAccepted {
                accepted,
                port: actual,
            } => {
                assert_eq!(actual, port);
                accepted
            }
            other => panic!("expected accepted socket event, got {other:?}"),
        }
    }

    /// Prove a refused or cancelled socket actually closed, rather than infer
    /// cleanup from the disappearance of a route or a released budget slot.
    async fn closed(socket: &mut TcpStream) {
        let mut byte = [0];
        let result = tokio::time::timeout(Duration::from_secs(5), socket.read(&mut byte))
            .await
            .expect("socket must close within its owner's teardown");
        match result {
            Ok(0) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::BrokenPipe
                ) => {}
            other => panic!("expected EOF or reset, got {other:?}"),
        }
    }

    /// Listener cancellation is delivered by the runtime. Poll the actual bind
    /// endpoint under one deadline so a socket's earlier EOF cannot race the
    /// accept task's descriptor drop; only connection refusal proves closure.
    async fn listener_refused(port: u16) {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                match TcpStream::connect((Ipv4Addr::LOCALHOST, port)).await {
                    Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => return,
                    Err(error) => panic!("unexpected listener probe error: {error}"),
                    Ok(socket) => drop(socket),
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("listener must refuse before unrelated teardown can finish");
    }

    /// One real supervisor reader with no session or terminal dependency.
    /// Tests retain this owner outside their assertion task so a failed
    /// observation still disconnects and joins the connection before its
    /// StateDir is removed. The duplex transport exercises normal dispatch,
    /// framing and teardown, without introducing SSH or a fixed socket path.
    struct ReaderPeer {
        reader: Option<FrameReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>>,
        writer: Option<FrameWriter<tokio::io::WriteHalf<tokio::io::DuplexStream>>>,
        server: Option<tokio::task::JoinHandle<anyhow::Result<()>>>,
    }

    impl Drop for ReaderPeer {
        fn drop(&mut self) {
            if let Some(server) = &self.server {
                server.abort();
            }
        }
    }

    impl ReaderPeer {
        /// Retain task ownership before the first protocol assertion. Tests
        /// authenticate as a full-authority helm through the actual hello.
        async fn connect(sup: Arc<Supervisor>) -> Self {
            let (client, server) = tokio::io::duplex(1 << 20);
            let server = tokio::spawn(super::super::connection::handle_connection(sup, server));
            let (read, write) = tokio::io::split(client);
            let mut peer = Self {
                reader: Some(FrameReader::new(read)),
                writer: Some(FrameWriter::new(write)),
                server: Some(server),
            };
            tokio::time::timeout(
                Duration::from_secs(10),
                farhelm_proto::io::handshake(
                    peer.reader.as_mut().unwrap(),
                    peer.writer.as_mut().unwrap(),
                    "helm",
                ),
            )
            .await
            .expect("owned supervisor must answer its hello")
            .expect("full-authority hello");
            peer
        }

        /// Refuse to write once the owned supervisor is already known dead.
        /// Every write is bounded independently of the test's cleanup tail.
        async fn send(&mut self, frame: Frame) {
            assert!(
                !self.server.as_ref().unwrap().is_finished(),
                "owned supervisor reader ended before send"
            );
            tokio::time::timeout(
                Duration::from_secs(10),
                self.writer.as_mut().unwrap().write_frame(&frame),
            )
            .await
            .expect("frame write must progress")
            .expect("write to owned reader");
        }

        /// A frame's arrival is the readiness oracle; bound both the wait and
        /// the retained payload rather than infer readiness from elapsed time.
        async fn frame(&mut self) -> Frame {
            tokio::time::timeout(
                Duration::from_secs(10),
                self.reader.as_mut().unwrap().read_frame(),
            )
            .await
            .expect("owned reader must make frame progress")
            .expect("read owned reader frame")
            .expect("owned reader transport remains open")
        }

        /// These fixtures have no session hints or terminal output. Any other
        /// frame is therefore an ordering failure, not traffic to skip over.
        async fn control(&mut self) -> ControlMsg {
            let frame = self.frame().await;
            assert_eq!(frame.kind, farhelm_proto::FrameKind::Control);
            serde_json::from_slice(&frame.body).unwrap()
        }

        /// Claim a kernel-chosen nonzero port and wait for the actual admission
        /// reply before allowing an application socket to connect to it.
        async fn claim(&mut self) -> u16 {
            let reservation = port_reservation();
            let port = reservation.local_addr().unwrap().port();
            drop(reservation);
            self.send(Frame::control(&ControlMsg::ClaimTcpTunnelPorts {
                req_id: 1,
                ports: vec![port],
            }))
            .await;
            assert_eq!(
                self.control().await,
                ControlMsg::TcpTunnelPortsClaimed { req_id: 1 }
            );
            port
        }

        /// Connect through the listener and observe its published pending id.
        /// The TCP handshake alone would prove only that the backlog is open.
        async fn accept(&mut self, port: u16) -> (TcpStream, u64) {
            let socket = tokio::time::timeout(
                Duration::from_secs(10),
                TcpStream::connect((Ipv4Addr::LOCALHOST, port)),
            )
            .await
            .expect("claimed listener accepts promptly")
            .expect("connect claimed listener");
            match self.control().await {
                ControlMsg::TcpTunnelAccepted {
                    accepted,
                    port: actual,
                } => {
                    assert_eq!(actual, port);
                    (socket, accepted)
                }
                other => panic!("expected accepted event, got {other:?}"),
            }
        }

        /// Only the real BeginAccepted dispatch may install the active route.
        /// Its reply establishes the same channel the later wire frames use.
        async fn begin(&mut self, accepted: u64, channel: u32) {
            self.send(Frame::control(&ControlMsg::BeginAcceptedTcpRelay {
                req_id: 2,
                accepted,
                channel,
            }))
            .await;
            assert_eq!(
                self.control().await,
                ControlMsg::TcpRelayStarted { req_id: 2, channel }
            );
        }

        /// End both transport halves without dropping the server task owner.
        /// This lets an assertion distinguish early tunnel teardown from the
        /// connection task's eventual completion after an unrelated gate opens.
        fn disconnect(&mut self) {
            self.reader.take();
            self.writer.take();
        }

        /// Cleanup runs even after an assertion task panics. A timeout aborts
        /// and reaps the owned task before returning, never detaching it over
        /// the state directory's lifetime.
        async fn finish(&mut self) -> Result<(), String> {
            self.disconnect();
            let Some(mut server) = self.server.take() else {
                return Ok(());
            };
            match tokio::time::timeout(Duration::from_secs(10), &mut server).await {
                Ok(joined) => joined
                    .map_err(|error| error.to_string())?
                    .map_err(|error| format!("{error:#}")),
                Err(_) => {
                    server.abort();
                    let _ = server.await;
                    Err("owned supervisor reader did not finish cleanup".into())
                }
            }
        }
    }

    /// Propagate the observation only after its independent owner has cleaned
    /// up. Keep cleanup errors visible alongside the original assertion panic.
    fn assert_reader_observation(
        observed: Result<(), tokio::task::JoinError>,
        cleaned: Result<(), String>,
    ) {
        if let Err(error) = observed {
            if let Err(cleanup) = cleaned {
                eprintln!("reader fixture cleanup also failed: {cleanup}");
            }
            if error.is_panic() {
                std::panic::resume_unwind(error.into_panic());
            }
            panic!("reader observation was cancelled: {error}");
        }
        cleaned.expect("owned reader cleanup must complete");
    }

    /// A dead helm's listeners, pending sockets and admitted sockets must close
    /// before the reader waits for an unrelated listing handler. Holding every
    /// list permit makes that teardown wait real; the following control reply
    /// proves the reader installed the listing task before transport loss.
    #[farhelm_testtrace::test]
    async fn reader_teardown_closes_tunnels_before_waiting_for_a_held_handler() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .unwrap();
        let shared = Arc::new(tokio::sync::Mutex::new(
            ReaderPeer::connect(Arc::clone(&sup)).await,
        ));
        let held = Arc::clone(&sup.list_admission)
            .acquire_many_owned(super::super::core::LIST_ADMISSION_PERMITS as u32)
            .await
            .unwrap();
        let observed = tokio::spawn({
            let shared = Arc::clone(&shared);
            let sup = Arc::clone(&sup);
            async move {
                let mut peer = shared.lock().await;
                let port = peer.claim().await;
                let (mut active, active_id) = peer.accept(port).await;
                peer.begin(active_id, 7).await;
                // A post-admission round trip establishes a live socket pump;
                // Started alone would not rule out an already-ended route.
                active.write_all(b"q").await.unwrap();
                let request = peer.frame().await;
                assert_eq!(request.kind, farhelm_proto::FrameKind::Data);
                assert_eq!(request.channel, 7);
                assert_eq!(request.body, b"q");
                peer.send(Frame::control(&ControlMsg::TcpRelayAck {
                    channel: 7,
                    received: 1,
                }))
                .await;
                peer.send(Frame::data(7, b"r".to_vec())).await;
                assert_eq!(
                    peer.control().await,
                    ControlMsg::TcpRelayAck {
                        channel: 7,
                        received: 1
                    }
                );
                let mut response = [0];
                tokio::time::timeout(Duration::from_secs(10), active.read_exact(&mut response))
                    .await
                    .expect("admitted socket must remain live before disconnect")
                    .unwrap();
                assert_eq!(&response, b"r");
                let (mut pending, pending_id) = peer.accept(port).await;
                assert_ne!(active_id, pending_id);
                let mut probe = [0];
                assert_eq!(
                    pending.try_read(&mut probe).unwrap_err().kind(),
                    std::io::ErrorKind::WouldBlock,
                    "pending socket must still be open before disconnect"
                );
                assert_eq!(
                    sup.tcp_tunnel_budget.available_permits(),
                    MAX_TCP_TUNNEL_CONNECTIONS - 2
                );
                peer.send(Frame::control(&ControlMsg::ListSessions { req_id: 70 }))
                    .await;
                peer.send(Frame::control(&ControlMsg::SetBrowserForwarding {
                    req_id: 71,
                    enabled: false,
                }))
                .await;
                assert_eq!(
                    peer.control().await,
                    ControlMsg::BrowserForwardingSet { req_id: 71 }
                );
                assert_eq!(
                    sup.list_admission.available_permits(),
                    0,
                    "the unrelated handler's list gate is held"
                );
                assert!(!peer.server.as_ref().unwrap().is_finished());
                peer.disconnect();
                closed(&mut pending).await;
                closed(&mut active).await;
                listener_refused(port).await;
                assert_eq!(
                    sup.tcp_tunnel_budget.available_permits(),
                    MAX_TCP_TUNNEL_CONNECTIONS
                );
                assert_eq!(
                    sup.list_admission.available_permits(),
                    0,
                    "the gate must remain held through the teardown observation"
                );
                assert!(
                    !peer.server.as_ref().unwrap().is_finished(),
                    "reader must still await its gated listing handler"
                );
            }
        })
        .await;
        drop(held);
        let cleaned = shared.lock().await.finish().await;
        assert_reader_observation(observed, cleaned);
    }

    /// Exchange more than one credit window on the actual reader in each
    /// direction. Run both half-close orders: finishing the request must not
    /// destroy the response path, regardless of which endpoint sends first.
    #[farhelm_testtrace::test]
    async fn reader_accepted_relays_exceed_the_window_with_both_half_close_orders() {
        for application_first in [true, false] {
            let state = StateDir::new();
            let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
                .await
                .unwrap();
            let shared = Arc::new(tokio::sync::Mutex::new(
                ReaderPeer::connect(Arc::clone(&sup)).await,
            ));
            let observed = tokio::spawn({
                let shared = Arc::clone(&shared);
                async move {
                    let mut peer = shared.lock().await;
                    exchange_through_reader(&mut peer, application_first).await;
                }
            })
            .await;
            let cleaned = shared.lock().await.finish().await;
            assert_reader_observation(observed, cleaned);
        }
    }

    /// These generated byte patterns differ in the two directions, so an echo
    /// or crossed route cannot satisfy the byte-for-byte comparison by chance.
    async fn exchange_through_reader(peer: &mut ReaderPeer, application_first: bool) {
        let port = peer.claim().await;
        let (application, accepted) = peer.accept(port).await;
        peer.begin(accepted, 7).await;
        let length = farhelm_proto::UPLOAD_WINDOW_BYTES as usize + 4 * UPLOAD_CHUNK_BYTES;
        let from_application: Vec<u8> = (0..length).map(|index| (index % 251) as u8).collect();
        let from_helm: Vec<u8> = (0..length).map(|index| (250 - index % 251) as u8).collect();
        let (read, write) = application.into_split();
        let mut application_tasks = tokio::task::JoinSet::new();
        if application_first {
            let bytes = from_application.clone();
            application_tasks.spawn(write_application_then_eof(write, bytes));
            read_wire_then_eof(peer, &from_application).await;
            application_tasks.join_next().await.unwrap().unwrap();
            application_tasks.spawn(read_application_through_eof(read, from_helm.clone()));
            write_wire_then_eof(peer, &from_helm).await;
        } else {
            application_tasks.spawn(read_application_through_eof(read, from_helm.clone()));
            write_wire_then_eof(peer, &from_helm).await;
            application_tasks.join_next().await.unwrap().unwrap();
            application_tasks.spawn(write_application_then_eof(write, from_application.clone()));
            read_wire_then_eof(peer, &from_application).await;
        }
        assert_eq!(
            peer.control().await,
            ControlMsg::TcpRelayEnded {
                channel: 7,
                reason: None
            }
        );
        while let Some(joined) =
            tokio::time::timeout(Duration::from_secs(10), application_tasks.join_next())
                .await
                .expect("application pumps must finish after terminal success")
        {
            joined.unwrap();
        }
    }

    /// Cumulative ACKs are sent only after each frame is observed. Reaching
    /// EOF after a payload larger than the window therefore proves the actual
    /// demultiplexer delivered credit and preserved bulk data/EOF ordering.
    async fn read_wire_then_eof(peer: &mut ReaderPeer, expected: &[u8]) {
        let mut received = 0;
        while received < expected.len() {
            let frame = peer.frame().await;
            assert_eq!(
                frame.kind,
                farhelm_proto::FrameKind::Data,
                "terminal success or EOF must not overtake the payload"
            );
            assert_eq!(frame.channel, 7);
            assert!(!frame.body.is_empty());
            let end = received + frame.body.len();
            assert!(end <= expected.len());
            assert_eq!(frame.body, expected[received..end]);
            received = end;
            peer.send(Frame::control(&ControlMsg::TcpRelayAck {
                channel: 7,
                received: received as u64,
            }))
            .await;
        }
        assert_eq!(peer.control().await, ControlMsg::TcpRelayEof { channel: 7 });
    }

    /// Respect actual peer credit while sending, rather than merely pushing
    /// an over-window payload into a generous test transport. Each ACK proves
    /// that the accepted application's socket accepted that prefix.
    async fn write_wire_then_eof(peer: &mut ReaderPeer, bytes: &[u8]) {
        let mut sent = 0;
        for chunk in bytes.chunks(UPLOAD_CHUNK_BYTES) {
            peer.send(Frame::data(7, chunk.to_vec())).await;
            sent += chunk.len();
            assert_eq!(
                peer.control().await,
                ControlMsg::TcpRelayAck {
                    channel: 7,
                    received: sent as u64
                }
            );
        }
        peer.send(Frame::control(&ControlMsg::TcpRelayEof { channel: 7 }))
            .await;
    }

    /// TCP write boundaries are arbitrary. The wire-side consumer checks the
    /// exact payload and the EOF marker independently of these write calls.
    async fn write_application_then_eof(
        mut write: tokio::net::tcp::OwnedWriteHalf,
        bytes: Vec<u8>,
    ) {
        tokio::time::timeout(Duration::from_secs(10), async {
            write.write_all(&bytes).await.unwrap();
            write.shutdown().await.unwrap();
        })
        .await
        .expect("application write and half-close must progress with live credit");
    }

    /// Read exactly the bounded response and then require socket EOF. This
    /// verifies write shutdown after the peer EOF, rather than just observing
    /// bytes that could have arrived while the other direction was destroyed.
    async fn read_application_through_eof(
        mut read: tokio::net::tcp::OwnedReadHalf,
        expected: Vec<u8>,
    ) {
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut bytes = vec![0; expected.len()];
            read.read_exact(&mut bytes).await.unwrap();
            assert_eq!(bytes, expected);
            let mut tail = [0];
            assert_eq!(read.read(&mut tail).await.unwrap(), 0);
        })
        .await
        .expect("application must receive the response and its half-close");
    }

    /// One busy port rolls the whole claim back, including ports bound before
    /// it. Invalid requests and binding failures must leave the same connection
    /// able to retry; only success spends its single claim.
    #[farhelm_testtrace::test]
    async fn claims_are_validated_all_or_nothing_and_retry_only_after_failure() {
        let mut claims = TunnelClaims::default();
        let budget = Arc::new(Semaphore::new(MAX_TCP_TUNNEL_CONNECTIONS));
        let (tx, _rx) = mpsc::channel(64);
        for ports in [vec![0], vec![22], vec![3000, 3000], (3000..3033).collect()] {
            let error = claims
                .claim(ports, Arc::clone(&budget), tx.clone())
                .unwrap_err();
            assert_eq!(error_kind(&error), ErrorKind::InvalidRequest);
            assert!(!claims.claimed);
            assert!(claims.listeners.is_empty());
        }
        let first = port_reservation();
        let occupied = port_reservation();
        let first_port = first.local_addr().unwrap().port();
        let occupied_port = occupied.local_addr().unwrap().port();
        drop(first);
        let error = claims
            .claim(
                vec![first_port, occupied_port],
                Arc::clone(&budget),
                tx.clone(),
            )
            .unwrap_err();
        assert_eq!(error_kind(&error), ErrorKind::Conflict);
        assert!(!claims.claimed);
        let rolled_back = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, first_port))
            .expect("the first listener must be rolled back when the second bind fails");
        drop(rolled_back);
        drop(occupied);
        claims
            .claim(
                vec![first_port, occupied_port],
                Arc::clone(&budget),
                tx.clone(),
            )
            .unwrap();
        assert!(claims.claimed);
        assert_eq!(claims.listeners.len(), 2);
        let error = claims.claim(Vec::new(), budget, tx).unwrap_err();
        assert_eq!(error_kind(&error), ErrorKind::Conflict);
        claims.shutdown().await;
    }

    /// Each connection owns its listeners and pending-id table. A failed
    /// overlapping claim cannot take over the first connection's listener or
    /// refuse its accepted socket; the owner can consume that socket only once.
    #[farhelm_testtrace::test]
    async fn other_connections_cannot_take_over_or_consume_pending_sockets() {
        let mut first = TunnelClaims::default();
        let mut other = TunnelClaims::default();
        let budget = Arc::new(Semaphore::new(MAX_TCP_TUNNEL_CONNECTIONS));
        let (tx, mut rx) = mpsc::channel(64);
        let reservation = port_reservation();
        let port = reservation.local_addr().unwrap().port();
        drop(reservation);
        first
            .claim(vec![port], Arc::clone(&budget), tx.clone())
            .unwrap();
        let error = other
            .claim(vec![port], Arc::clone(&budget), tx.clone())
            .unwrap_err();
        assert_eq!(error_kind(&error), ErrorKind::Conflict);
        let mut peer = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
            .await
            .unwrap();
        let id = accepted(&mut rx, port).await;
        assert!(other.take(id).is_none());
        assert_eq!(budget.available_permits(), MAX_TCP_TUNNEL_CONNECTIONS - 1);
        let socket = first
            .take(id)
            .expect("only the claiming connection owns this pending id");
        assert!(first.take(id).is_none());
        assert_eq!(budget.available_permits(), MAX_TCP_TUNNEL_CONNECTIONS - 1);
        drop(socket);
        closed(&mut peer).await;
        assert_eq!(budget.available_permits(), MAX_TCP_TUNNEL_CONNECTIONS);
        first.shutdown().await;
    }

    /// A closed connection's TCP remnant must not delay Linux reconnects.
    /// Force a supervisor-first FIN, then prove that the port still excludes
    /// a reuse-off bind after listener shutdown. Reclaim must succeed through
    /// the production policy while live-listener conflicts remain refused by
    /// the separate overlap and wildcard tests.
    #[cfg(target_os = "linux")]
    #[farhelm_testtrace::test]
    async fn linux_reclaims_a_port_while_its_closed_connection_still_reserves_it() {
        let mut first = TunnelClaims::default();
        let budget = Arc::new(Semaphore::new(MAX_TCP_TUNNEL_CONNECTIONS));
        let (tx, mut rx) = mpsc::channel(64);
        let reservation = port_reservation();
        let port = reservation.local_addr().unwrap().port();
        drop(reservation);
        first
            .claim(vec![port], Arc::clone(&budget), tx.clone())
            .unwrap();
        let mut peer = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
            .await
            .unwrap();
        let id = accepted(&mut rx, port).await;
        let mut accepted = first.take(id).expect("own pending socket");
        accepted.socket.write_all(b"live").await.unwrap();
        let mut marker = [0; 4];
        tokio::time::timeout(Duration::from_secs(5), peer.read_exact(&mut marker))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            &marker, b"live",
            "the accepted socket was live before closing"
        );
        accepted.socket.shutdown().await.unwrap();
        let mut tail = [0];
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), peer.read(&mut tail))
                .await
                .unwrap()
                .unwrap(),
            0,
            "supervisor initiated a clean FIN, not a reset"
        );
        peer.shutdown().await.unwrap();
        drop(peer);
        drop(accepted);
        first.shutdown().await;
        let strict = TcpSocket::new_v4().unwrap();
        strict.set_reuseaddr(false).unwrap();
        assert_eq!(
            strict
                .bind((Ipv4Addr::LOCALHOST, port).into())
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::AddrInUse,
            "closed TCP state still reserves the port"
        );
        let mut next = TunnelClaims::default();
        next.claim(vec![port], budget, tx)
            .expect("Linux reuse bypasses only closed TCP state");
        next.shutdown().await;
    }

    /// Ending a claim releases listeners immediately when no accepted TCP
    /// lifetime leaves TIME_WAIT on that port. Socket-isolation tests above
    /// deliberately make no immediate-rebind promise after real connections.
    #[farhelm_testtrace::test]
    async fn an_unconnected_claim_can_be_reclaimed_after_owner_shutdown() {
        let mut first = TunnelClaims::default();
        let mut other = TunnelClaims::default();
        let budget = Arc::new(Semaphore::new(MAX_TCP_TUNNEL_CONNECTIONS));
        let (tx, _rx) = mpsc::channel(64);
        let reservation = port_reservation();
        let port = reservation.local_addr().unwrap().port();
        drop(reservation);
        first
            .claim(vec![port], Arc::clone(&budget), tx.clone())
            .unwrap();
        assert!(
            other
                .claim(vec![port], Arc::clone(&budget), tx.clone())
                .is_err()
        );
        first.shutdown().await;
        other
            .claim(vec![port], budget, tx)
            .expect("shutdown released the unconnected listener");
        other.shutdown().await;
    }

    /// A loopback claim must fail when another program owns the wildcard
    /// listener, including on BSD-derived stacks that allow reusable binds to
    /// shadow wildcard traffic. A failed claim remains available for retry.
    #[farhelm_testtrace::test]
    async fn tunnel_claim_never_shadows_a_wildcard_listener() {
        let wildcard = std::net::TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
        let port = wildcard.local_addr().unwrap().port();
        let mut owner = TunnelClaims::default();
        let budget = Arc::new(Semaphore::new(MAX_TCP_TUNNEL_CONNECTIONS));
        let (tx, _rx) = mpsc::channel(64);
        let error = owner
            .claim(vec![port], Arc::clone(&budget), tx.clone())
            .unwrap_err();
        assert_eq!(error_kind(&error), ErrorKind::Conflict);
        assert!(!owner.claimed);
        assert!(owner.listeners.is_empty());
        drop(wildcard);
        owner.claim(vec![port], budget, tx).unwrap();
        owner.shutdown().await;
    }

    /// Admission cannot leave an unbounded set of undecided sockets. Pending
    /// sockets and sockets already handed to a relay consume the same budget,
    /// shared even by distinct connection owners; the next socket closes.
    #[farhelm_testtrace::test]
    async fn host_cap_counts_pending_and_admitted_sockets_across_connections() {
        let mut owners = [TunnelClaims::default(), TunnelClaims::default()];
        let budget = Arc::new(Semaphore::new(MAX_TCP_TUNNEL_CONNECTIONS));
        let (tx, mut rx) = mpsc::channel(64);
        let mut ports = Vec::new();
        for owner in &mut owners {
            let reservation = port_reservation();
            let port = reservation.local_addr().unwrap().port();
            drop(reservation);
            owner
                .claim(vec![port], Arc::clone(&budget), tx.clone())
                .unwrap();
            ports.push(port);
        }
        let mut peers = Vec::new();
        let mut admitted = Vec::new();
        for index in 0..MAX_TCP_TUNNEL_CONNECTIONS {
            let owner = index % owners.len();
            let peer = TcpStream::connect((Ipv4Addr::LOCALHOST, ports[owner]))
                .await
                .unwrap();
            let id = accepted(&mut rx, ports[owner]).await;
            if index % 3 == 0 {
                admitted.push(owners[owner].take(id).unwrap());
            }
            peers.push(peer);
        }
        assert_eq!(budget.available_permits(), 0);
        for _ in 0..3 {
            let mut excess = TcpStream::connect((Ipv4Addr::LOCALHOST, ports[1]))
                .await
                .unwrap();
            closed(&mut excess).await;
        }
        assert_eq!(
            owners[1].pending.lock().unwrap().capacity_refusals,
            3,
            "repeated refusals remain counted after their first warning"
        );
        assert!(
            rx.try_recv().is_err(),
            "excess sockets must never reach the helm"
        );
        drop(admitted.pop());
        assert_eq!(budget.available_permits(), 1);
        let replacement = TcpStream::connect((Ipv4Addr::LOCALHOST, ports[0]))
            .await
            .unwrap();
        accepted(&mut rx, ports[0]).await;
        assert_eq!(budget.available_permits(), 0);
        drop(replacement);
        for owner in &mut owners {
            owner.shutdown().await;
        }
        drop(admitted);
        for peer in &mut peers {
            closed(peer).await;
        }
        assert_eq!(budget.available_permits(), MAX_TCP_TUNNEL_CONNECTIONS);
    }

    /// Listener and pending-socket ownership must survive neither normal
    /// teardown nor an unwinding/cancelled reader. Closing pending sockets is
    /// synchronous even before accept-task cancellation can be scheduled.
    #[farhelm_testtrace::test]
    async fn dropping_claims_closes_pending_sockets_and_releases_listeners() {
        let mut owner = TunnelClaims::default();
        let budget = Arc::new(Semaphore::new(MAX_TCP_TUNNEL_CONNECTIONS));
        let (tx, mut rx) = mpsc::channel(64);
        let reservation = port_reservation();
        let port = reservation.local_addr().unwrap().port();
        drop(reservation);
        owner.claim(vec![port], Arc::clone(&budget), tx).unwrap();
        let mut peer = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
            .await
            .unwrap();
        accepted(&mut rx, port).await;
        assert_eq!(budget.available_permits(), MAX_TCP_TUNNEL_CONNECTIONS - 1);
        drop(owner);
        assert_eq!(budget.available_permits(), MAX_TCP_TUNNEL_CONNECTIONS);
        closed(&mut peer).await;
        // The accept loop holds the last event sender. Its disappearance proves
        // Drop cancelled that task and released its listener, not just its map.
        assert!(
            tokio::time::timeout(Duration::from_secs(5), rx.recv())
                .await
                .expect("cancelled accept task must drop its event sender")
                .is_none()
        );
        assert!(
            TcpStream::connect((Ipv4Addr::LOCALHOST, port))
                .await
                .is_err()
        );
    }

    /// An exhausted id space must refuse new sockets rather than reuse an id
    /// that a delayed helm answer could still name. The last representable id
    /// remains valid and its removal does not reset the allocation sequence.
    #[farhelm_testtrace::test]
    async fn accepted_ids_never_wrap_or_reuse_a_consumed_id() {
        let mut owner = TunnelClaims::default();
        owner.pending.lock().unwrap().next_id = Some(u64::MAX);
        let budget = Arc::new(Semaphore::new(MAX_TCP_TUNNEL_CONNECTIONS));
        let (tx, mut rx) = mpsc::channel(64);
        let reservation = port_reservation();
        let port = reservation.local_addr().unwrap().port();
        drop(reservation);
        owner.claim(vec![port], Arc::clone(&budget), tx).unwrap();
        let mut last = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
            .await
            .unwrap();
        assert_eq!(accepted(&mut rx, port).await, u64::MAX);
        drop(
            owner
                .take(u64::MAX)
                .expect("the final id is a valid pending socket"),
        );
        closed(&mut last).await;
        let mut excess = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
            .await
            .unwrap();
        closed(&mut excess).await;
        assert!(rx.try_recv().is_err());
        assert!(owner.pending.lock().unwrap().next_id.is_none());
        assert_eq!(budget.available_permits(), MAX_TCP_TUNNEL_CONNECTIONS);
        owner.shutdown().await;
    }

    /// A socket's read EOF is only a half-close. The other pump must still
    /// deliver a response and shut down its write half on peer EOF before
    /// publishing full success, with data preceding EOF in the bulk FIFO.
    #[farhelm_testtrace::test]
    async fn shared_bridge_preserves_response_after_request_half_close() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let mut peer = TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let (socket, _) = listener.accept().await.unwrap();
        let budget = Arc::new(Semaphore::new(1));
        let permit = budget.clone().try_acquire_owned().unwrap();
        let (bulk, mut rx) = mpsc::channel(8);
        let (priority, mut acks) = mpsc::channel(8);
        let mut tasks = tokio::task::JoinSet::new();
        let route = start_accepted(
            &mut tasks,
            AcceptedSocket {
                socket,
                _permit: permit,
            },
            bulk,
            priority,
            3,
            7,
        );
        let started: ControlMsg = serde_json::from_slice(&next_frame(&mut rx).await.body).unwrap();
        assert_eq!(
            started,
            ControlMsg::TcpRelayStarted {
                req_id: 3,
                channel: 7
            }
        );
        peer.write_all(b"request").await.unwrap();
        peer.shutdown().await.unwrap();
        let mut request = Vec::new();
        while request.len() < 7 {
            let data = next_frame(&mut rx).await;
            assert_eq!(data.kind, farhelm_proto::FrameKind::Data);
            assert_eq!(data.channel, 7);
            request.extend_from_slice(&data.body);
            route.state.ack(request.len() as u64);
        }
        assert_eq!(request, b"request");
        let eof: ControlMsg = serde_json::from_slice(&next_frame(&mut rx).await.body).unwrap();
        assert_eq!(eof, ControlMsg::TcpRelayEof { channel: 7 });
        assert!(
            !route.task.is_finished(),
            "request EOF must leave the response direction alive"
        );
        route.state.push(b"response");
        route.state.eof();
        let mut response = Vec::new();
        tokio::time::timeout(Duration::from_secs(5), peer.read_to_end(&mut response))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(response, b"response");
        let ack: ControlMsg = serde_json::from_slice(&next_frame(&mut acks).await.body).unwrap();
        assert_eq!(
            ack,
            ControlMsg::TcpRelayAck {
                channel: 7,
                received: 8
            }
        );
        let ended: ControlMsg = serde_json::from_slice(&next_frame(&mut rx).await.body).unwrap();
        assert_eq!(
            ended,
            ControlMsg::TcpRelayEnded {
                channel: 7,
                reason: None
            }
        );
        tokio::time::timeout(Duration::from_secs(5), tasks.join_next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(budget.available_permits(), 1);
    }
}
