//! Helm-only loopback TCP channels, carried over the existing multiplexed link.
//! The connection owns each task through a route whose drop cancels it, even
//! while connecting or waiting for credit. Data and EOF share the bulk FIFO;
//! tiny credit acknowledgements use the existing priority queue.

use super::connection::reply_frame;
use super::core::{RequestError, Supervisor, error_kind};
use farhelm_proto::tcp_relay::TcpRelayState;
use farhelm_proto::{ControlMsg, ErrorKind, Frame, UPLOAD_CHUNK_BYTES};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

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

/// EOF on either socket direction ends the entire channel. The two pumps are
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
        Ok::<(), anyhow::Error>(())
    };
    tokio::pin!(outgoing, incoming);
    tokio::select! {
        result = &mut outgoing => result,
        result = &mut incoming => result,
        reason = state.failed() => Err(anyhow::anyhow!(reason)),
    }
}
