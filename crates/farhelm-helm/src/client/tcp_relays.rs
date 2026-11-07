//! One accepted TCP socket per protocol channel. The guard owns
//! admission and every cancellation path; the borrowed pumps create no detached
//! socket tasks. Desktop listeners resolve a fresh client after reconnect;
//! recipe listeners belong to a port claim on one supervisor connection.

use super::*;
use farhelm_proto::tcp_relay::TcpRelayState;
use std::net::IpAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// One recipe-port claim, including the interval before its reply arrives.
/// Installing the sender first makes an accepted event racing that reply safe.
#[derive(Default)]
pub(super) enum TunnelClaim {
    #[default]
    Unclaimed,
    Receiving {
        req_id: u64,
        sender: mpsc::Sender<TcpTunnelAccepted>,
    },
    Closed,
}

/// A supervisor-reported connection, with no authority to select a destination.
/// The recipe owner resolves `port` against its own configuration. The permit
/// follows this event through queueing, dialing and forwarding; dropping an
/// unused event queues a refusal while retaining the permit. Events themselves
/// keep only a weak client reference; bounded cleanup temporarily owns a client
/// until it can enqueue the refusal or the connection ends.
/// Events belong to the connection which delivered them and cannot be relayed
/// through a replacement client after reconnecting.
pub struct TcpTunnelAccepted {
    accepted: u64,
    port: u16,
    client: std::sync::Weak<SupervisorClient>,
    permit: Option<OwnedSemaphorePermit>,
    reject_on_drop: bool,
}

impl TcpTunnelAccepted {
    /// The sandbox listening port reported by the supervisor. Resolve it only
    /// against the caller's trusted recipe snapshot; it grants no authority to
    /// dial a destination absent from that configuration.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// The opaque id used to refuse this connection on its originating client.
    /// It has no meaning on another connection, including after a reconnect.
    /// Keeping it read-only binds forwarding and Drop cleanup to the same id.
    pub fn accepted_id(&self) -> u64 {
        self.accepted
    }
}

impl Drop for TcpTunnelAccepted {
    fn drop(&mut self) {
        if self.reject_on_drop
            && let Some(client) = self.client.upgrade()
            && let Some(permit) = self.permit.take()
        {
            let accepted = self.accepted;
            tokio::spawn(async move {
                let _permit = permit;
                let _ = client.reject_tcp_tunnel(accepted).await;
            });
        }
    }
}

/// A cancelled claim may already have installed remote listeners, even without
/// a reply. Retiring the connection is the only unambiguous way to release them;
/// an explicit refusal instead leaves the connection available for retry.
struct ClaimGuard {
    client: Arc<SupervisorClient>,
    req_id: u64,
    complete: bool,
}

impl Drop for ClaimGuard {
    fn drop(&mut self) {
        if !self.complete {
            self.client.retire();
        }
    }
}

/// Owns routing before admission is sent. Drop always retires the pending
/// request and closes upstream, including cancellation while admission or a
/// socket pump is parked. Channel ids are never reused on this client.
struct RelayGuard {
    client: Arc<SupervisorClient>,
    channel: u32,
    admission_req_id: u64,
    accepted: Option<u64>,
    _permit: Option<OwnedSemaphorePermit>,
}

impl Drop for RelayGuard {
    fn drop(&mut self) {
        let client = Arc::clone(&self.client);
        let channel = self.channel;
        let req_id = self.admission_req_id;
        let accepted = self.accepted;
        let permit = self._permit.take();
        tokio::spawn(async move {
            // Recipe cleanup still counts against admission while waiting for
            // writer capacity. Cancelling many relays cannot create an
            // unbounded population of tasks waiting to send cleanup.
            let _permit = permit;
            client.tcp_relays.lock().await.remove(&channel);
            client.pending.lock().await.map.remove(&req_id);
            let _ = client
                .send_tcp_relay_control(ControlMsg::CloseTcpRelay { channel })
                .await;
            if let Some(accepted) = accepted {
                let _ = client.reject_tcp_tunnel(accepted).await;
            }
        });
    }
}

impl SupervisorClient {
    /// Claim a recipe's listening ports atomically on this connection. Only a
    /// failed claim may be retried. The receiver exists before the request can
    /// reach the supervisor, since an accept can precede the successful reply.
    /// The receiver and every event belong to this connection. Its owner must
    /// drop the receiver and pending destination dials when [`Self::closed`]
    /// fires; closing the sender cannot discard events already in the receiver.
    /// Destinations come from the caller's trusted configuration, never the
    /// supervisor's event. Admission covers queueing, dialing, forwarding, and
    /// cancellation cleanup until its refusal or close has been enqueued.
    pub async fn claim_tcp_tunnel_ports(
        self: &Arc<Self>,
        ports: Vec<u16>,
    ) -> anyhow::Result<mpsc::Receiver<TcpTunnelAccepted>> {
        anyhow::ensure!(
            ports.len() <= farhelm_proto::MAX_TCP_TUNNEL_PORTS
                && ports.iter().all(|port| *port != 0 && *port != 22),
            "TCP tunnel ports must be nonzero, exclude 22, and fit the port limit"
        );
        let req_id = self.req_id();
        let (sender, receiver) = mpsc::channel(farhelm_proto::MAX_TCP_TUNNEL_CONNECTIONS);
        {
            let mut claim = self
                .tcp_tunnel_claim
                .lock()
                .expect("TCP tunnel claim mutex poisoned");
            // The failure drain closes this registry before publishing
            // `closed()`. Preserve transport classification in that window
            // so a caller does not mistake connection loss for a bind refusal.
            if matches!(*claim, TunnelClaim::Closed) {
                return Err(anyhow::Error::new(SupervisorTransportError::NotSent));
            }
            anyhow::ensure!(
                matches!(*claim, TunnelClaim::Unclaimed),
                "TCP tunnel ports already claimed"
            );
            *claim = TunnelClaim::Receiving { req_id, sender };
        }
        let mut guard = ClaimGuard {
            client: Arc::clone(self),
            req_id,
            complete: false,
        };
        let reply = self
            .request(req_id, ControlMsg::ClaimTcpTunnelPorts { req_id, ports })
            .await;
        match reply {
            Ok(ControlMsg::TcpTunnelPortsClaimed { .. }) => {
                guard.complete = true;
                Ok(receiver)
            }
            result => {
                let mut claim = self
                    .tcp_tunnel_claim
                    .lock()
                    .expect("TCP tunnel claim mutex poisoned");
                if matches!(&*claim, TunnelClaim::Receiving { req_id: active, .. } if *active == guard.req_id)
                {
                    *claim = TunnelClaim::Unclaimed;
                }
                guard.complete = true;
                match result {
                    Err(error) => Err(error),
                    Ok(other) => {
                        self.retire();
                        Err(wrong_reply("ClaimTcpTunnelPorts", &other))
                    }
                }
            }
        }
    }

    /// Refuse an accepted connection whose configured destination could not be
    /// reached. The caller retains the event while awaiting this method, so
    /// waiting for writer capacity still consumes its admission slot. Dropping
    /// the event provides a cancellation-safe fallback; a second refusal after
    /// an explicit one is harmless because accepted ids are not reused.
    pub async fn reject_tcp_tunnel(&self, accepted: u64) -> anyhow::Result<()> {
        self.send_tcp_relay_control(ControlMsg::RejectTcpTunnel { accepted })
            .await
    }

    /// Admitted cleanup follows ordinary writer backpressure. Connection death
    /// still cancels a parked enqueue, including when another sender keeps the
    /// queue's channel alive after transport retirement.
    async fn send_tcp_relay_control(&self, msg: ControlMsg) -> anyhow::Result<()> {
        tokio::select! {
            biased;
            () = self.closed() => anyhow::bail!("host disconnected during TCP relay cleanup"),
            result = self.writer_tx.send(Frame::control(&msg).into()) => {
                result.map_err(|_| anyhow::anyhow!("host disconnected during TCP relay cleanup"))
            }
        }
    }

    /// Keep demux refusals bounded even when an untrusted supervisor floods
    /// accepts. A writer that cannot enqueue the refusal loses the connection;
    /// spawning one blocked writer task per refusal would defeat admission.
    fn refuse_unadmitted_tcp_tunnel(&self, accepted: u64) {
        if self
            .writer_tx
            .try_send(Frame::control(&ControlMsg::RejectTcpTunnel { accepted }).into())
            .is_err()
        {
            // This helper is called only by the serialized demultiplexer.
            // Retirement publishes closed synchronously, so subsequent
            // refusals on this connection cannot repeat the warning.
            if !*self.closed.borrow() {
                warn!(
                    accepted,
                    "an unadmitted TCP tunnel refusal could not be enqueued; \
                     retiring the supervisor connection (logged once per connection)"
                );
            }
            self.retire();
        }
    }

    /// Acquire admission before the event enters any queue. The supervisor can
    /// choose only a port and opaque id; neither is a destination instruction.
    pub(super) fn deliver_tcp_tunnel(self: &Arc<Self>, accepted: u64, port: u16) {
        let sender = match &*self
            .tcp_tunnel_claim
            .lock()
            .expect("TCP tunnel claim mutex poisoned")
        {
            TunnelClaim::Receiving { sender, .. } if !sender.is_closed() => sender.clone(),
            _ => {
                self.refuse_unadmitted_tcp_tunnel(accepted);
                return;
            }
        };
        let Ok(permit) = Arc::clone(&self.tcp_tunnel_admission).try_acquire_owned() else {
            self.refuse_unadmitted_tcp_tunnel(accepted);
            return;
        };
        let event = TcpTunnelAccepted {
            accepted,
            port,
            client: Arc::downgrade(self),
            permit: Some(permit),
            reject_on_drop: true,
        };
        if let Err(error) = sender.try_send(event) {
            // The receiver may disappear after the check above. An event
            // which never entered its queue has no admitted owner: return its
            // permit and use the same nonblocking refusal as overflow.
            let mut event = error.into_inner();
            event.reject_on_drop = false;
            drop(event);
            self.refuse_unadmitted_tcp_tunnel(accepted);
        }
    }

    /// Start a helm-chosen channel for one event after the recipe manager has
    /// dialed its trusted destination. The owned admission slot survives until
    /// cleanup is enqueued, including cancellation before the admission reply.
    /// The event must come from this client, and the caller owns the choice of
    /// destination socket. Dropping this future closes that socket and refuses
    /// or closes its upstream connection; events cannot move across reconnects.
    pub async fn relay_accepted_tcp(
        self: &Arc<Self>,
        mut event: TcpTunnelAccepted,
        socket: TcpStream,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            event.client.ptr_eq(&Arc::downgrade(self)) && !*self.closed.borrow(),
            "TCP tunnel belongs to a different or closed connection"
        );
        let (channel, state, mut guard) = self.prepare_tcp_relay().await?;
        guard.accepted = Some(event.accepted);
        guard._permit = event.permit.take();
        event.reject_on_drop = false;
        let req_id = guard.admission_req_id;
        let reply = self
            .request(
                req_id,
                ControlMsg::BeginAcceptedTcpRelay {
                    req_id,
                    accepted: event.accepted,
                    channel,
                },
            )
            .await?;
        match reply {
            ControlMsg::TcpRelayStarted {
                channel: started, ..
            } if started == channel => {}
            other => return Err(wrong_reply("BeginAcceptedTcpRelay", &other)),
        }
        // Started consumed the accepted id. Channel close now owns cleanup;
        // only cancellation while admission is pending needs both identities.
        guard.accepted = None;
        self.bridge_tcp_stream(channel, state, socket).await
    }

    /// Bridge one accepted local TCP socket to a loopback port on the session
    /// host. None and wildcard literals try IPv4 then IPv6 loopback; other
    /// literals must be loopback.
    /// The caller owns desktop consent and the listener's lifetime. Dropping
    /// this future closes the socket and cancels even unanswered admission.
    /// EOF closes only that sending direction; the opposite side can finish
    /// its response. Idle streams have no timeout and are never replayed.
    pub async fn relay_tcp(
        self: &Arc<Self>,
        session_id: &str,
        address: Option<IpAddr>,
        port: u16,
        socket: TcpStream,
    ) -> anyhow::Result<()> {
        self.relay_stream(session_id, address, port, socket).await
    }

    /// Keep socket ownership and cancellation identical for production TCP and
    /// a controlled test writer. The latter can establish a blocked write
    /// without depending on kernel buffer sizes or scheduling delays.
    async fn relay_stream(
        self: &Arc<Self>,
        session_id: &str,
        address: Option<IpAddr>,
        port: u16,
        socket: impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            port != 0 && address.is_none_or(|ip| ip.is_loopback() || ip.is_unspecified()),
            "TCP relay target must be loopback with a nonzero port"
        );
        let (channel, state, guard) = self.prepare_tcp_relay().await?;
        let req_id = guard.admission_req_id;
        match self
            .request(
                req_id,
                ControlMsg::BeginTcpRelay {
                    req_id,
                    session_id: session_id.into(),
                    channel,
                    address,
                    port,
                },
            )
            .await?
        {
            ControlMsg::TcpRelayStarted {
                channel: started, ..
            } if started == channel => {}
            other => return Err(wrong_reply("BeginTcpRelay", &other)),
        }

        self.bridge_tcp_stream(channel, state, socket).await
    }

    /// Reserve routing before either admission request can reach the peer. The
    /// guard is installed before another await, so an unanswered request never
    /// leaves a channel or socket outside the caller's cancellation scope.
    async fn prepare_tcp_relay(
        self: &Arc<Self>,
    ) -> anyhow::Result<(u32, Arc<TcpRelayState>, RelayGuard)> {
        let channel = allocate_channel(&self.next_channel)?;
        let state = Arc::new(TcpRelayState::default());
        self.tcp_relays
            .lock()
            .await
            .insert(channel, Arc::clone(&state));
        let guard = RelayGuard {
            client: Arc::clone(self),
            channel,
            admission_req_id: self.req_id(),
            accepted: None,
            _permit: None,
        };
        Ok((channel, state, guard))
    }

    /// Both relay directions borrow the owned stream. Orderly EOF drains and
    /// shuts down one half while the other keeps running; failure or transport
    /// retirement cancels both, including a socket write which never wakes.
    async fn bridge_tcp_stream(
        &self,
        channel: u32,
        state: Arc<TcpRelayState>,
        socket: impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    ) -> anyhow::Result<()> {
        let (mut read, mut write) = tokio::io::split(socket);
        let outgoing = async {
            let mut bytes = vec![0; UPLOAD_CHUNK_BYTES];
            loop {
                let count = read.read(&mut bytes).await?;
                if count == 0 {
                    self.writer_tx
                        .send(Frame::control(&ControlMsg::TcpRelayEof { channel }).into())
                        .await
                        .map_err(|_| anyhow::anyhow!("host disconnected during TCP relay"))?;
                    return Ok::<(), anyhow::Error>(());
                }
                state.reserve(count).await.map_err(anyhow::Error::msg)?;
                let allowance = Arc::clone(&self.upload_enqueue).acquire_owned().await?;
                self.writer_tx
                    .send(Outbound {
                        frame: Frame::data(channel, bytes[..count].to_vec()),
                        _allowance: Some(allowance),
                    })
                    .await
                    .map_err(|_| anyhow::anyhow!("host disconnected during TCP relay"))?;
            }
        };
        let incoming = async {
            while let Some(bytes) = state.next().await.map_err(anyhow::Error::msg)? {
                write.write_all(&bytes).await?;
                let received = state.written(bytes.len());
                self.writer_tx
                    .send(Frame::control(&ControlMsg::TcpRelayAck { channel, received }).into())
                    .await
                    .map_err(|_| anyhow::anyhow!("host disconnected during TCP relay"))?;
            }
            write.shutdown().await?;
            Ok::<(), anyhow::Error>(())
        };
        let pumps = async {
            tokio::try_join!(outgoing, incoming)?;
            state.finished().await.map_err(anyhow::Error::msg)
        };
        let result = tokio::select! {
            result = pumps => result,
            reason = state.failed() => Err(anyhow::anyhow!(reason)),
            // Successful channel EOF is separate from connection retirement.
            // A client which stops reading cannot keep the old connection's
            // resources alive after the host is withdrawn.
            () = self.closed() => Err(anyhow::anyhow!("host disconnected during TCP relay")),
        };
        if let Some(Err(reason)) = state.outcome() {
            return Err(anyhow::anyhow!(reason));
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{DuplexStream, ReadHalf, WriteHalf};
    use tokio::net::TcpListener;
    use tokio::time::timeout;

    /// A live scripted supervisor can withhold admission independently of
    /// transport failure. Keeping both peer halves proves cancellation owns
    /// its cleanup rather than relying on connection teardown to drain maps.
    async fn peer() -> (
        Arc<SupervisorClient>,
        FrameReader<ReadHalf<DuplexStream>>,
        FrameWriter<WriteHalf<DuplexStream>>,
    ) {
        let (client_side, server_side) = tokio::io::duplex(64 * 1024);
        let server = tokio::spawn(async move {
            let (read, write) = tokio::io::split(server_side);
            let mut reader = FrameReader::new(read);
            let mut writer = FrameWriter::new(write);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });
        let (read, write) = tokio::io::split(client_side);
        let client = SupervisorClient::start(read, write).await.unwrap();
        let (reader, writer) = server.await.unwrap();
        (client, reader, writer)
    }

    /// An owned random-port socket pair lets the test observe that dropping
    /// the relay future closes its local TCP half as well as protocol state.
    async fn socket_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let (connected, accepted) = tokio::join!(TcpStream::connect(address), listener.accept());
        (connected.unwrap(), accepted.unwrap().0)
    }

    /// Bound every scripted-peer read; a missing cancellation must produce
    /// a useful test failure instead of an indefinitely parked future.
    async fn control(reader: &mut FrameReader<ReadHalf<DuplexStream>>) -> ControlMsg {
        let frame = timeout(Duration::from_secs(5), reader.read_frame())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        parse_control(&frame).unwrap()
    }

    /// TCP may split one write into any number of short reads. Collect only
    /// this channel's data through its directional EOF, with one deadline for
    /// the whole exchange and a byte bound supplied by the test's expectation.
    async fn bytes_until_eof(
        reader: &mut FrameReader<ReadHalf<DuplexStream>>,
        channel: u32,
        max_bytes: usize,
    ) -> Vec<u8> {
        timeout(Duration::from_secs(5), async {
            let mut bytes = Vec::new();
            loop {
                let frame = reader
                    .read_frame()
                    .await
                    .unwrap()
                    .expect("peer closed before relay EOF");
                match frame.kind {
                    FrameKind::Data => {
                        assert_eq!(frame.channel, channel);
                        assert!(
                            !frame.body.is_empty(),
                            "empty data cannot make progress toward EOF"
                        );
                        assert!(
                            bytes.len() + frame.body.len() <= max_bytes,
                            "relay sent more than the expected byte bound"
                        );
                        bytes.extend(frame.body);
                    }
                    FrameKind::Control => {
                        assert_eq!(
                            parse_control(&frame).unwrap(),
                            ControlMsg::TcpRelayEof { channel }
                        );
                        return bytes;
                    }
                }
            }
        })
        .await
        .expect("relay did not reach directional EOF within the exchange deadline")
    }

    /// A cleanup task retains admission until its last control frame is
    /// enqueued. Observe the resulting permit return rather than assuming the
    /// peer reading that frame also means the task has already dropped it.
    async fn wait_for_tunnel_permits(client: &SupervisorClient, expected: usize) {
        timeout(Duration::from_secs(5), async {
            while client.tcp_tunnel_admission.available_permits() != expected {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("TCP tunnel cleanup did not return admission");
    }

    /// The protocol reply itself is the readiness oracle for the recipe's
    /// ownership. Tests retain the scripted transport so dropping an event must
    /// send a refusal rather than succeeding through incidental disconnection.
    async fn claim(
        client: &Arc<SupervisorClient>,
        reader: &mut FrameReader<ReadHalf<DuplexStream>>,
        writer: &mut FrameWriter<WriteHalf<DuplexStream>>,
    ) -> mpsc::Receiver<TcpTunnelAccepted> {
        let claiming = tokio::spawn({
            let client = Arc::clone(client);
            async move { client.claim_tcp_tunnel_ports(vec![3000]).await }
        });
        let ControlMsg::ClaimTcpTunnelPorts { req_id, ports } = control(reader).await else {
            panic!("expected tunnel port claim");
        };
        assert_eq!(ports, vec![3000]);
        writer
            .write_control(&ControlMsg::TcpTunnelPortsClaimed { req_id })
            .await
            .unwrap();
        timeout(Duration::from_secs(5), claiming)
            .await
            .unwrap()
            .unwrap()
            .unwrap()
    }

    /// Accepted events can precede the claim reply. A refused claim must remove
    /// its sender so the existing host connection can retry instead of getting
    /// stuck in an already-claimed state.
    #[farhelm_testtrace::test]
    async fn claim_refusal_retries_and_accept_before_reply_is_retained() {
        let (client, mut reader, mut writer) = peer().await;
        for refused in [true, false] {
            let claiming = tokio::spawn({
                let client = Arc::clone(&client);
                async move { client.claim_tcp_tunnel_ports(vec![3000]).await }
            });
            let ControlMsg::ClaimTcpTunnelPorts { req_id, .. } = control(&mut reader).await else {
                panic!("expected tunnel port claim");
            };
            if refused {
                writer
                    .write_control(&ControlMsg::Error {
                        req_id,
                        kind: ErrorKind::Conflict,
                        message: "port in use".into(),
                        create_unaccepted: false,
                    })
                    .await
                    .unwrap();
                assert!(
                    timeout(Duration::from_secs(5), claiming)
                        .await
                        .unwrap()
                        .unwrap()
                        .is_err()
                );
            } else {
                writer
                    .write_control(&ControlMsg::TcpTunnelAccepted {
                        accepted: 7,
                        port: 3000,
                    })
                    .await
                    .unwrap();
                writer
                    .write_control(&ControlMsg::TcpTunnelPortsClaimed { req_id })
                    .await
                    .unwrap();
                let mut events = timeout(Duration::from_secs(5), claiming)
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap();
                let event = timeout(Duration::from_secs(5), events.recv())
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!((event.accepted_id(), event.port()), (7, 3000));
                assert_eq!(
                    client.tcp_tunnel_admission.available_permits(),
                    farhelm_proto::MAX_TCP_TUNNEL_CONNECTIONS - 1
                );
                drop(event);
                assert_eq!(
                    control(&mut reader).await,
                    ControlMsg::RejectTcpTunnel { accepted: 7 }
                );
                wait_for_tunnel_permits(&client, farhelm_proto::MAX_TCP_TUNNEL_CONNECTIONS).await;
                assert!(client.claim_tcp_tunnel_ports(vec![3000]).await.is_err());
            }
        }
    }

    /// The helm counts queued accepts and destination work before any relay
    /// channel exists. Moving one event into unanswered admission must retain
    /// its slot, and cancellation must release it while retiring both ids.
    #[farhelm_testtrace::test]
    async fn accepted_admission_cap_includes_queue_and_cancelled_pending_relay() {
        let (client, mut reader, mut writer) = peer().await;
        let mut events = claim(&client, &mut reader, &mut writer).await;
        for accepted in 1..=farhelm_proto::MAX_TCP_TUNNEL_CONNECTIONS as u64 {
            writer
                .write_control(&ControlMsg::TcpTunnelAccepted {
                    accepted,
                    port: 3000,
                })
                .await
                .unwrap();
        }
        // A rejected extra event proves the earlier accepts all passed demux,
        // even though their receiver has consumed nothing yet.
        writer
            .write_control(&ControlMsg::TcpTunnelAccepted {
                accepted: 1000,
                port: 3000,
            })
            .await
            .unwrap();
        assert_eq!(
            control(&mut reader).await,
            ControlMsg::RejectTcpTunnel { accepted: 1000 }
        );
        assert_eq!(client.tcp_tunnel_admission.available_permits(), 0);
        let event = events.recv().await.unwrap();
        let accepted_id = event.accepted_id();
        let (mut local, destination) = socket_pair().await;
        let relay = tokio::spawn({
            let client = Arc::clone(&client);
            async move { client.relay_accepted_tcp(event, destination).await }
        });
        let ControlMsg::BeginAcceptedTcpRelay {
            req_id,
            accepted,
            channel,
        } = control(&mut reader).await
        else {
            panic!("expected accepted relay admission");
        };
        assert_eq!(accepted, accepted_id);
        assert_eq!(client.tcp_tunnel_admission.available_permits(), 0);
        relay.abort();
        assert!(relay.await.unwrap_err().is_cancelled());
        assert_eq!(
            control(&mut reader).await,
            ControlMsg::CloseTcpRelay { channel }
        );
        assert_eq!(
            control(&mut reader).await,
            ControlMsg::RejectTcpTunnel { accepted }
        );
        wait_for_retired_route(&client, channel, req_id).await;
        wait_for_tunnel_permits(&client, 1).await;
        assert_eq!(
            timeout(Duration::from_secs(5), local.read(&mut [0]))
                .await
                .unwrap()
                .unwrap(),
            0
        );
        client.retire();
        timeout(Duration::from_secs(5), client.closed())
            .await
            .unwrap();
        drop(events);
        wait_for_tunnel_permits(&client, farhelm_proto::MAX_TCP_TUNNEL_CONNECTIONS).await;
    }

    /// An accepted event already owns bounded admission. Destination failure
    /// during legitimate writer backpressure must wait to refuse that socket,
    /// keep its permit while waiting, and leave the rest of the host connected.
    #[farhelm_testtrace::test]
    async fn dropped_admitted_event_waits_for_writer_capacity() {
        let (client, mut reader, mut writer) = peer().await;
        let mut events = claim(&client, &mut reader, &mut writer).await;
        client.deliver_tcp_tunnel(7, 3000);
        let event = events.recv().await.unwrap();
        assert_eq!(
            client.tcp_tunnel_admission.available_permits(),
            farhelm_proto::MAX_TCP_TUNNEL_CONNECTIONS - 1
        );
        // Reserving every valid slot parks sends without depending on socket
        // buffer sizes or injecting malformed frames into the shared writer.
        let slots = client
            .writer_tx
            .reserve_many(SUPERVISOR_WRITER_QUEUE)
            .await
            .unwrap();
        assert_eq!(client.writer_tx.capacity(), 0);
        drop(event);
        assert!(!*client.closed.borrow());
        assert_eq!(
            client.tcp_tunnel_admission.available_permits(),
            farhelm_proto::MAX_TCP_TUNNEL_CONNECTIONS - 1
        );
        drop(slots);
        assert_eq!(
            control(&mut reader).await,
            ControlMsg::RejectTcpTunnel { accepted: 7 }
        );
        wait_for_tunnel_permits(&client, farhelm_proto::MAX_TCP_TUNNEL_CONNECTIONS).await;
        assert!(!*client.closed.borrow());
    }

    /// Cancelling a started desktop or recipe relay must preserve the host
    /// connection when its writer is temporarily full. A recipe retains its
    /// admission until close is enqueued, and Started removes the need for a
    /// second accepted-id refusal after that close.
    #[farhelm_testtrace::test]
    async fn admitted_relay_cancellation_waits_for_writer_capacity() {
        for recipe in [false, true] {
            let (client, mut reader, mut writer) = peer().await;
            let (mut local, socket) = socket_pair().await;
            let relay = if recipe {
                let mut events = claim(&client, &mut reader, &mut writer).await;
                client.deliver_tcp_tunnel(7, 3000);
                let event = events.recv().await.unwrap();
                tokio::spawn({
                    let client = Arc::clone(&client);
                    async move { client.relay_accepted_tcp(event, socket).await }
                })
            } else {
                tokio::spawn({
                    let client = Arc::clone(&client);
                    async move { client.relay_tcp("session", None, 3000, socket).await }
                })
            };
            let (req_id, channel) = match control(&mut reader).await {
                ControlMsg::BeginTcpRelay {
                    req_id, channel, ..
                } if !recipe => (req_id, channel),
                ControlMsg::BeginAcceptedTcpRelay {
                    req_id,
                    accepted: 7,
                    channel,
                } if recipe => (req_id, channel),
                other => panic!("expected relay admission, got {other:?}"),
            };
            writer
                .write_control(&ControlMsg::TcpRelayStarted { req_id, channel })
                .await
                .unwrap();
            local.write_all(b"x").await.unwrap();
            // A real pump frame proves Started reached the relay future, not
            // only its pending-reply registry in the demultiplexer.
            let frame = timeout(Duration::from_secs(5), reader.read_frame())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert_eq!(frame.kind, FrameKind::Data);
            assert_eq!(frame.channel, channel);
            assert_eq!(frame.body, b"x");
            let slots = client
                .writer_tx
                .reserve_many(SUPERVISOR_WRITER_QUEUE)
                .await
                .unwrap();
            assert_eq!(client.writer_tx.capacity(), 0);
            relay.abort();
            assert!(relay.await.unwrap_err().is_cancelled());
            wait_for_retired_route(&client, channel, req_id).await;
            assert!(!*client.closed.borrow());
            if recipe {
                assert_eq!(
                    client.tcp_tunnel_admission.available_permits(),
                    farhelm_proto::MAX_TCP_TUNNEL_CONNECTIONS - 1
                );
            }
            drop(slots);
            assert_eq!(
                control(&mut reader).await,
                ControlMsg::CloseTcpRelay { channel }
            );
            wait_for_tunnel_permits(&client, farhelm_proto::MAX_TCP_TUNNEL_CONNECTIONS).await;
            let listed = tokio::spawn({
                let client = Arc::clone(&client);
                async move { client.list_sessions().await }
            });
            let ControlMsg::ListSessions { req_id } = control(&mut reader).await else {
                panic!("started relay sent an unnecessary accepted-id refusal");
            };
            writer
                .write_control(&ControlMsg::SessionList {
                    req_id,
                    sessions: vec![],
                    truncated: false,
                })
                .await
                .unwrap();
            timeout(Duration::from_secs(5), listed)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert!(!*client.closed.borrow());
        }
    }

    /// Without a receiver, an event is refused synchronously by demux. Keeping
    /// only a receiver after dropping the client must not create a strong cycle
    /// through the accepted events or the sender stored in the client.
    #[farhelm_testtrace::test]
    async fn unclaimed_accepts_are_refused_and_receiver_does_not_retain_client() {
        let (client, mut reader, mut writer) = peer().await;
        writer
            .write_control(&ControlMsg::TcpTunnelAccepted {
                accepted: 1,
                port: 3000,
            })
            .await
            .unwrap();
        assert_eq!(
            control(&mut reader).await,
            ControlMsg::RejectTcpTunnel { accepted: 1 }
        );
        let mut events = claim(&client, &mut reader, &mut writer).await;
        let weak = Arc::downgrade(&client);
        drop(client);
        assert!(
            timeout(Duration::from_secs(5), events.recv())
                .await
                .unwrap()
                .is_none()
        );
        assert!(weak.upgrade().is_none());
    }

    /// Cancellation before a claim reply cannot tell whether remote listeners
    /// exist. Closing that connection releases them and clears its pending id;
    /// keeping the connection for retry would strand an ambiguous first claim.
    #[farhelm_testtrace::test]
    async fn cancelling_an_unanswered_claim_retires_the_connection() {
        let (client, mut reader, _writer) = peer().await;
        let claiming = tokio::spawn({
            let client = Arc::clone(&client);
            async move { client.claim_tcp_tunnel_ports(vec![3000]).await }
        });
        let ControlMsg::ClaimTcpTunnelPorts { req_id, .. } = control(&mut reader).await else {
            panic!("expected tunnel claim");
        };
        assert!(client.pending.lock().await.map.contains_key(&req_id));
        claiming.abort();
        assert!(claiming.await.unwrap_err().is_cancelled());
        timeout(Duration::from_secs(5), client.closed())
            .await
            .unwrap();
        timeout(Duration::from_secs(5), async {
            loop {
                if client.pending.lock().await.closed {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(!client.pending.lock().await.map.contains_key(&req_id));
        assert!(matches!(
            *client.tcp_tunnel_claim.lock().unwrap(),
            TunnelClaim::Closed
        ));
        let error = client.claim_tcp_tunnel_ports(vec![3000]).await.unwrap_err();
        assert!(matches!(
            error.downcast_ref::<SupervisorTransportError>(),
            Some(SupervisorTransportError::NotSent)
        ));
    }

    /// A service may wait for request EOF before sending its response. Local
    /// shutdown must send only directional EOF; a later response still drains
    /// through to the local socket before its read half sees EOF.
    #[farhelm_testtrace::test]
    async fn local_half_close_keeps_the_response_direction_open() {
        let (client, mut reader, mut writer) = peer().await;
        let (mut local, socket) = socket_pair().await;
        let relay = tokio::spawn({
            let client = Arc::clone(&client);
            async move { client.relay_tcp("session", None, 3000, socket).await }
        });
        let ControlMsg::BeginTcpRelay {
            req_id, channel, ..
        } = control(&mut reader).await
        else {
            panic!("expected relay admission");
        };
        writer
            .write_control(&ControlMsg::TcpRelayStarted { req_id, channel })
            .await
            .unwrap();
        local.write_all(b"request").await.unwrap();
        local.shutdown().await.unwrap();
        assert_eq!(bytes_until_eof(&mut reader, channel, 7).await, b"request");
        assert!(!relay.is_finished());
        writer
            .write_frame(&Frame::data(channel, b"response".to_vec()))
            .await
            .unwrap();
        writer
            .write_control(&ControlMsg::TcpRelayEof { channel })
            .await
            .unwrap();
        let mut response = Vec::new();
        timeout(Duration::from_secs(5), local.read_to_end(&mut response))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(response, b"response");
        assert_eq!(
            control(&mut reader).await,
            ControlMsg::TcpRelayAck {
                channel,
                received: 8
            }
        );
        writer
            .write_control(&ControlMsg::TcpRelayEnded {
                channel,
                reason: None,
            })
            .await
            .unwrap();
        timeout(Duration::from_secs(5), relay)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }

    /// Peer EOF ends only the local read half. A sender that sees that EOF may
    /// still write its final bytes, which must reach the supervisor before the
    /// local EOF and the full terminal outcome can finish the channel.
    #[farhelm_testtrace::test]
    async fn peer_half_close_keeps_the_local_sender_open() {
        let (client, mut reader, mut writer) = peer().await;
        let (mut local, socket) = socket_pair().await;
        let relay = tokio::spawn({
            let client = Arc::clone(&client);
            async move { client.relay_tcp("session", None, 3000, socket).await }
        });
        let ControlMsg::BeginTcpRelay {
            req_id, channel, ..
        } = control(&mut reader).await
        else {
            panic!("expected relay admission");
        };
        writer
            .write_control(&ControlMsg::TcpRelayStarted { req_id, channel })
            .await
            .unwrap();
        writer
            .write_control(&ControlMsg::TcpRelayEof { channel })
            .await
            .unwrap();
        assert_eq!(
            timeout(Duration::from_secs(5), local.read(&mut [0]))
                .await
                .unwrap()
                .unwrap(),
            0
        );
        assert!(!relay.is_finished());
        local.write_all(b"after peer EOF").await.unwrap();
        local.shutdown().await.unwrap();
        assert_eq!(
            bytes_until_eof(&mut reader, channel, 14).await,
            b"after peer EOF"
        );
        writer
            .write_control(&ControlMsg::TcpRelayEnded {
                channel,
                reason: None,
            })
            .await
            .unwrap();
        timeout(Duration::from_secs(5), relay)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }

    /// A local recipient which never accepts a write. The notification marks
    /// the actual write boundary, and Drop proves cancellation released the
    /// owned stream rather than only removing a routing-table entry.
    struct BlockedRecipient {
        writing: Arc<tokio::sync::Notify>,
        dropped: Arc<AtomicBool>,
    }

    impl Drop for BlockedRecipient {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::Release);
        }
    }

    impl tokio::io::AsyncRead for BlockedRecipient {
        fn poll_read(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
            _: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Pending
        }
    }

    impl tokio::io::AsyncWrite for BlockedRecipient {
        fn poll_write(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
            _: &[u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
            self.writing.notify_one();
            std::task::Poll::Pending
        }

        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }

        fn poll_shutdown(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }
    }

    /// Drop cleanup is an owned async task because destructors cannot await.
    /// Observe its registry retirement directly after the connection writer
    /// has stopped; a protocol close cannot be delivered on that transport.
    async fn wait_for_retired_route(client: &SupervisorClient, channel: u32, req_id: u64) {
        timeout(Duration::from_secs(5), async {
            loop {
                if !client.tcp_relays.lock().await.contains_key(&channel)
                    && !client.pending.lock().await.map.contains_key(&req_id)
                {
                    return;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("retired relay kept its route or pending admission");
    }

    /// Directional EOF promises ordered bytes, not an immortal old connection.
    /// Retiring the otherwise-live host while those final bytes are parked
    /// in a local write must release the stream and route without requiring
    /// another byte, a credit change, or a kernel-buffer timing assumption.
    #[farhelm_testtrace::test]
    async fn host_retirement_cancels_a_blocked_write_after_directional_remote_eof() {
        let (client, mut reader, mut writer) = peer().await;
        let writing = Arc::new(tokio::sync::Notify::new());
        let dropped = Arc::new(AtomicBool::new(false));
        let recipient = BlockedRecipient {
            writing: Arc::clone(&writing),
            dropped: Arc::clone(&dropped),
        };
        let relay = tokio::spawn({
            let client = Arc::clone(&client);
            async move { client.relay_stream("session", None, 3000, recipient).await }
        });
        let ControlMsg::BeginTcpRelay {
            req_id, channel, ..
        } = control(&mut reader).await
        else {
            panic!("expected relay admission");
        };
        writer
            .write_control(&ControlMsg::TcpRelayStarted { req_id, channel })
            .await
            .unwrap();
        writer
            .write_frame(&Frame::data(channel, b"final response".to_vec()))
            .await
            .unwrap();
        timeout(Duration::from_secs(5), writing.notified())
            .await
            .unwrap();
        assert!(!dropped.load(Ordering::Acquire));
        writer
            .write_control(&ControlMsg::TcpRelayEof { channel })
            .await
            .unwrap();
        // The correlated reply behind EOF is a demux barrier. It proves
        // directional EOF was published while the local write remained parked.
        let listed = tokio::spawn({
            let client = Arc::clone(&client);
            async move { client.list_sessions().await }
        });
        let ControlMsg::ListSessions { req_id: list_id } = control(&mut reader).await else {
            panic!("expected EOF barrier request");
        };
        writer
            .write_control(&ControlMsg::SessionList {
                req_id: list_id,
                sessions: vec![],
                truncated: false,
            })
            .await
            .unwrap();
        timeout(Duration::from_secs(5), listed)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(client.tcp_relays.lock().await[&channel].outcome(), None);
        assert!(!dropped.load(Ordering::Acquire));
        client.retire();
        assert!(
            timeout(Duration::from_secs(5), relay)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
        assert!(dropped.load(Ordering::Acquire));
        wait_for_retired_route(&client, channel, req_id).await;
        assert!(!client.tcp_relays.lock().await.contains_key(&channel));
        assert!(!client.pending.lock().await.map.contains_key(&req_id));
    }

    /// Cancelling either unanswered admission or an established idle stream
    /// retires its route and pending id, sends close, and closes TCP while the
    /// shared supervisor connection remains usable for an unrelated request.
    #[farhelm_testtrace::test]
    async fn cancellation_owns_pending_admission_idle_socket_and_upstream_close() {
        let (client, mut reader, mut writer) = peer().await;
        for admitted in [false, true] {
            let (mut local, accepted) = socket_pair().await;
            let relay = tokio::spawn({
                let client = Arc::clone(&client);
                async move { client.relay_tcp("session", None, 3000, accepted).await }
            });
            let ControlMsg::BeginTcpRelay {
                req_id, channel, ..
            } = control(&mut reader).await
            else {
                panic!("expected relay admission");
            };
            assert!(client.pending.lock().await.map.contains_key(&req_id));
            assert!(client.tcp_relays.lock().await.contains_key(&channel));
            if admitted {
                writer
                    .write_control(&ControlMsg::TcpRelayStarted { req_id, channel })
                    .await
                    .unwrap();
                // A same-connection request behind Started pins that demux
                // processed admission without a scheduling sleep.
                let listed = tokio::spawn({
                    let client = Arc::clone(&client);
                    async move { client.list_sessions().await }
                });
                let ControlMsg::ListSessions { req_id: list_id } = control(&mut reader).await
                else {
                    panic!("expected unrelated list");
                };
                writer
                    .write_control(&ControlMsg::SessionList {
                        req_id: list_id,
                        sessions: vec![],
                        truncated: false,
                    })
                    .await
                    .unwrap();
                listed.await.unwrap().unwrap();
                assert!(!client.pending.lock().await.map.contains_key(&req_id));
            }
            relay.abort();
            assert!(relay.await.unwrap_err().is_cancelled());
            assert_eq!(
                control(&mut reader).await,
                ControlMsg::CloseTcpRelay { channel }
            );
            assert!(!client.tcp_relays.lock().await.contains_key(&channel));
            assert!(!client.pending.lock().await.map.contains_key(&req_id));
            assert!(!client.pending.lock().await.closed);
            let mut byte = [0];
            assert_eq!(
                timeout(Duration::from_secs(5), local.read(&mut byte))
                    .await
                    .unwrap()
                    .unwrap(),
                0
            );
        }
        let listed = tokio::spawn({
            let client = Arc::clone(&client);
            async move { client.list_sessions().await }
        });
        let ControlMsg::ListSessions { req_id } = control(&mut reader).await else {
            panic!("expected a usable connection after cancellation");
        };
        writer
            .write_control(&ControlMsg::SessionList {
                req_id,
                sessions: vec![],
                truncated: false,
            })
            .await
            .unwrap();
        listed.await.unwrap().unwrap();
    }

    /// Losing a host ends an already-admitted, otherwise-idle relay promptly.
    /// There are no socket bytes or closed credit window to wake its pumps;
    /// fail_all must publish the transport failure out of band.
    #[farhelm_testtrace::test]
    async fn host_disconnect_ends_an_idle_tcp_relay() {
        let (client, mut reader, mut writer) = peer().await;
        let (mut local, accepted) = socket_pair().await;
        let relay = tokio::spawn({
            let client = Arc::clone(&client);
            async move { client.relay_tcp("session", None, 3000, accepted).await }
        });
        let ControlMsg::BeginTcpRelay {
            req_id, channel, ..
        } = control(&mut reader).await
        else {
            panic!("expected relay admission");
        };
        writer
            .write_control(&ControlMsg::TcpRelayStarted { req_id, channel })
            .await
            .unwrap();
        let listed = tokio::spawn({
            let client = Arc::clone(&client);
            async move { client.list_sessions().await }
        });
        let ControlMsg::ListSessions { req_id: list_id } = control(&mut reader).await else {
            panic!("expected an admission barrier request");
        };
        writer
            .write_control(&ControlMsg::SessionList {
                req_id: list_id,
                sessions: vec![],
                truncated: false,
            })
            .await
            .unwrap();
        timeout(Duration::from_secs(5), listed)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(!client.pending.lock().await.map.contains_key(&req_id));
        assert!(client.tcp_relays.lock().await.contains_key(&channel));
        // Admission is processed and both pumps are idle. Only the actual
        // transport-loss notice can release this relay now.
        drop(reader);
        drop(writer);
        assert!(
            timeout(Duration::from_secs(5), relay)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
        let mut byte = [0];
        assert_eq!(
            timeout(Duration::from_secs(5), local.read(&mut byte))
                .await
                .unwrap()
                .unwrap(),
            0
        );
    }
}
