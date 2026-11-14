//! Opening the helm's WebSockets, and the UI channel's real transport.
//!
//! The helm authenticates a WebSocket upgrade by subprotocol, not by header: browsers cannot set `Authorization` on
//! a WebSocket, so the helm reads the device credential from `Sec-WebSocket-Protocol: farhelm,
//! farhelm-device-<secret>` and selects plain `farhelm`, never echoing the credential. This client does the same so
//! the helm keeps one authentication path for every UI.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use farhelm_proto::ui_channel::{ClientMessage, ServerMessage, UI_CHANNEL_PATH};
use futures_util::future::BoxFuture;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::{self, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::channel::{Connector, Link};

/// Where the helm listens and the device credential to present. Always loopback: the helm refuses a `Host` header
/// that is not `127.0.0.1:<its port>`, which is also what keeps a web page from reaching it by DNS rebinding.
#[derive(Clone)]
pub struct HelmEndpoint {
    pub addr: SocketAddr,
    secret: Arc<str>,
}

impl HelmEndpoint {
    pub fn new(addr: SocketAddr, device_secret: impl Into<Arc<str>>) -> HelmEndpoint {
        HelmEndpoint {
            addr,
            secret: device_secret.into(),
        }
    }

    pub(crate) fn secret(&self) -> &str {
        &self.secret
    }
}

impl std::fmt::Debug for HelmEndpoint {
    /// Never prints the credential, so an endpoint can appear in logs and test failures.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HelmEndpoint")
            .field("addr", &self.addr)
            .finish_non_exhaustive()
    }
}

/// Why a socket did not open.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConnectError {
    /// The helm answered the upgrade with an HTTP status instead. 401 means the credential was refused, which no
    /// amount of retrying fixes; other statuses are the route's own refusal (a session that is gone, say).
    #[error("the helm refused the connection with HTTP {status}")]
    Refused { status: u16 },
    /// Anything else: nothing listening, a timeout, a broken handshake. Worth retrying.
    #[error("could not reach the helm: {0}")]
    Failed(String),
}

pub(crate) type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Open a WebSocket to `path_and_query` (already encoded by [`crate::ApiPath`]) with the device credential.
/// `max_message` caps both frames and messages, so a fragmented message cannot get past the frame cap.
pub(crate) async fn open_socket(
    endpoint: &HelmEndpoint,
    path_and_query: &str,
    max_message: usize,
    timeout: Duration,
) -> Result<Socket, ConnectError> {
    let url = format!("ws://{}{}", endpoint.addr, path_and_query);
    let mut request = url
        .into_client_request()
        .map_err(|error| ConnectError::Failed(error.to_string()))?;
    let protocols = format!("farhelm, farhelm-device-{}", endpoint.secret())
        .parse()
        .map_err(|_| {
            ConnectError::Failed("the device credential is not a valid header value".into())
        })?;
    request
        .headers_mut()
        .insert("Sec-WebSocket-Protocol", protocols);
    let config = WebSocketConfig::default()
        .max_frame_size(Some(max_message))
        .max_message_size(Some(max_message));
    // Nagle off: keystrokes and small commands must not wait for the previous frame's ACK.
    let connect = tokio_tungstenite::connect_async_with_config(request, Some(config), true);
    let (socket, response) = tokio::time::timeout(timeout, connect)
        .await
        .map_err(|_| ConnectError::Failed("timed out opening the socket".into()))?
        .map_err(|error| match error {
            tungstenite::Error::Http(response) => ConnectError::Refused {
                status: response.status().as_u16(),
            },
            error => ConnectError::Failed(error.to_string()),
        })?;
    let selected = response
        .headers()
        .get("Sec-WebSocket-Protocol")
        .and_then(|value| value.to_str().ok());
    if selected != Some("farhelm") {
        return Err(ConnectError::Failed(format!(
            "the helm selected protocol {selected:?}, not farhelm"
        )));
    }
    Ok(socket)
}

/// Bigger than any message the helm sends on the UI channel: a reply body is capped at 32 MiB before JSON framing,
/// and a snapshot is a few hundred rows at the product's scale. A bigger message is a broken peer, not a big list.
const UI_CHANNEL_MAX_MESSAGE: usize = 64 * 1024 * 1024;

/// The UI channel over a real WebSocket.
pub(crate) struct UiChannelConnector {
    endpoint: HelmEndpoint,
    timeout: Duration,
}

impl UiChannelConnector {
    pub(crate) fn new(endpoint: HelmEndpoint, timeout: Duration) -> UiChannelConnector {
        UiChannelConnector { endpoint, timeout }
    }
}

impl Connector for UiChannelConnector {
    fn connect(&self) -> BoxFuture<'static, Result<Link, ConnectError>> {
        let endpoint = self.endpoint.clone();
        let timeout = self.timeout;
        Box::pin(async move {
            let socket =
                open_socket(&endpoint, UI_CHANNEL_PATH, UI_CHANNEL_MAX_MESSAGE, timeout).await?;
            Ok(bridge(socket, timeout))
        })
    }
}

/// Turn a socket into a [`Link`]: one task writes client messages, one reads server messages.
///
/// The link ends (its receiver yields `None`) when the socket closes, errs, or carries something that is not a
/// server message; the channel treats all of those as a break. Dropping the link's sender closes the socket, which is
/// how the channel hangs up. WebSocket pings from the helm are answered by tungstenite itself as part of reading.
///
/// Neither task may outlive the link on a black-holed connection: every write (and the closing handshake) is bounded
/// by `write_timeout`, and the reader stops as soon as the channel drops the link's receiver instead of waiting for a
/// frame that may never come.
fn bridge(socket: Socket, write_timeout: Duration) -> Link {
    let (mut sink, mut stream) = socket.split();
    let (client_tx, mut client_rx) = mpsc::unbounded_channel::<ClientMessage>();
    let (server_tx, server_rx) = mpsc::unbounded_channel::<ServerMessage>();
    tokio::spawn(async move {
        while let Some(message) = client_rx.recv().await {
            let Ok(text) = serde_json::to_string(&message) else {
                break;
            };
            let write = tokio::time::timeout(write_timeout, sink.send(Message::text(text)));
            if !matches!(write.await, Ok(Ok(()))) {
                break;
            }
        }
        let _ = tokio::time::timeout(write_timeout, sink.close()).await;
    });
    tokio::spawn(async move {
        loop {
            let frame = tokio::select! {
                frame = stream.next() => frame,
                _ = server_tx.closed() => break,
            };
            let Some(frame) = frame else {
                break;
            };
            let message = match frame {
                Ok(Message::Text(text)) => match serde_json::from_str::<ServerMessage>(&text) {
                    Ok(message) => message,
                    Err(error) => {
                        tracing::warn!(%error, "the UI channel carried an undecodable message");
                        break;
                    }
                },
                Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_)) => continue,
                Ok(Message::Binary(_)) => {
                    tracing::warn!("the UI channel carried a binary frame");
                    break;
                }
                Ok(Message::Close(_)) | Err(_) => break,
            };
            if server_tx.send(message).is_err() {
                break;
            }
        }
    });
    Link {
        tx: client_tx,
        rx: server_rx,
    }
}
