//! One accepted desktop-side TCP socket per protocol channel. The guard owns
//! admission and every cancellation path; the borrowed pumps create no detached
//! socket tasks. The tunnel manager keeps the listener and chooses a fresh live
//! supervisor client for each accepted socket after a host reconnect.

use super::*;
use farhelm_proto::tcp_relay::TcpRelayState;
use std::net::IpAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Owns routing before admission is sent. Drop always retires the pending
/// request and closes upstream, including cancellation while admission or a
/// socket pump is parked. Channel ids are never reused on this client.
struct RelayGuard {
    client: Arc<SupervisorClient>,
    channel: u32,
    admission_req_id: u64,
}

impl Drop for RelayGuard {
    fn drop(&mut self) {
        let client = Arc::clone(&self.client);
        let channel = self.channel;
        let req_id = self.admission_req_id;
        tokio::spawn(async move {
            client.tcp_relays.lock().await.remove(&channel);
            client.pending.lock().await.map.remove(&req_id);
            let _ = client
                .writer_tx
                .send(Frame::control(&ControlMsg::CloseTcpRelay { channel }).into())
                .await;
        });
    }
}

impl SupervisorClient {
    /// Bridge one accepted local TCP socket to a loopback port on the session
    /// host. None and wildcard literals try IPv4 then IPv6 loopback; other
    /// literals must be loopback.
    /// The caller owns desktop consent and the listener's lifetime. Dropping
    /// this future closes the socket and cancels even unanswered admission.
    /// EOF in either direction closes the whole relay; idle streams have no
    /// timeout and an existing byte stream is never replayed after reconnection.
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
        let channel = allocate_channel(&self.next_channel)?;
        let state = Arc::new(TcpRelayState::default());
        self.tcp_relays
            .lock()
            .await
            .insert(channel, Arc::clone(&state));
        let req_id = self.req_id();
        let _guard = RelayGuard {
            client: Arc::clone(self),
            channel,
            admission_req_id: req_id,
        };
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

        let (mut read, mut write) = tokio::io::split(socket);
        let outgoing = async {
            let mut bytes = vec![0; UPLOAD_CHUNK_BYTES];
            loop {
                let count = read.read(&mut bytes).await?;
                if count == 0 {
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
            Ok::<(), anyhow::Error>(())
        };
        tokio::pin!(outgoing, incoming);
        let result = tokio::select! {
            result = &mut outgoing => {
                // Remote EOF can wake a producer waiting for credit before
                // the consumer finishes its final local socket write. Drain
                // those already-received bytes before closing the socket.
                if state.outcome() == Some(Ok(())) {
                    tokio::select! {
                        result = &mut incoming => result,
                        () = self.closed() => Err(anyhow::anyhow!("host disconnected during TCP relay")),
                    }
                } else { result }
            }
            result = &mut incoming => result,
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

    /// Remote EOF promises ordered bytes, not an immortal old connection.
    /// Retiring the otherwise-live host while those final bytes are parked
    /// in a local write must release the stream and route without requiring
    /// another byte, a credit change, or a kernel-buffer timing assumption.
    #[farhelm_testtrace::test]
    async fn host_retirement_cancels_a_blocked_write_after_successful_remote_eof() {
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
            .write_control(&ControlMsg::TcpRelayEnded {
                channel,
                reason: None,
            })
            .await
            .unwrap();
        // The correlated reply behind EOF is a demux barrier. It proves
        // success was published while the local write remained parked.
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
        assert_eq!(
            client.tcp_relays.lock().await[&channel].outcome(),
            Some(Ok(()))
        );
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
