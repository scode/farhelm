//! Terminal streams: one WebSocket per attached terminal (a session's agent terminal or one of its tabs), carrying
//! grid messages both ways. The caller renders; this module only moves messages and keeps the stream attached.
//!
//! Each stream reconnects on its own, independently of the UI channel and of other streams. The rules come from how
//! the helm and supervisor decide who holds a terminal (SPEC.md, "Terminal experience"):
//!
//! - The first attach takes control (the user opened this terminal). Every automatic reconnect attaches with the
//!   non-displacing `/term/unowned` route, so a stream coming back after a network blip can never take a terminal
//!   away from another client that took it meanwhile.
//! - A dropped socket or an unanswered ping reconnects. It reconnects at once only if the attachment had answered a
//!   ping, which shows the link worked; otherwise it waits a backoff that doubles while attempts keep failing, so a
//!   path that attaches, replays and drops again cannot loop on full replays. A successful attachment resets the
//!   backoff, so a terminal that recovered retries promptly the next time.
//! - A `replaced` or `other` detach waits the backoff and reattaches: `other` carries transport trouble (a lost
//!   supervisor, an unreachable host) that keeps recovering. `replaced` means a newer attachment with this stream's
//!   own lease took the terminal; the supervisor lets a same-lease attach replace the incumbent rather than defer to
//!   it, so the reattach takes the terminal back. Two live streams of one lease on one terminal would therefore keep
//!   replacing each other, each time with a full replay: a caller keeps at most one live stream per terminal per
//!   lease (see [`AttachmentLease`]).
//! - `taken_over`, `tab_closed` and `stalled` stop the stream in [`StreamState::Detached`]: someone else holds the
//!   terminal, it is gone, or this client fell behind and attaching again would walk back into the same wedge.
//!   [`TerminalHandle::take_control`] attaches again, displacing whoever holds it.
//! - [`TerminalHandle::reconnect_now`] skips a running backoff (the "Reconnect now" button) without displacing
//!   anyone.
//!
//! Every attachment starts with a full replay of the screen and history, ended by [`TerminalEvent::ReplayComplete`].
//! A [`StreamState::Attached`] event precedes each attachment's first output, and the caller must reset its grid
//! model there: the replay rebuilds it from nothing.
//!
//! The event queue to the caller is bounded. When the caller falls behind, the stream stops reading its socket, and
//! the supervisor's flow control sees a slow viewer (eventually a `stalled` detach) instead of this process buffering
//! output without limit. While the stream waits on the caller its own watchdog is paused, because the delay is ours,
//! not the link's. The stream task ends when it is closed, when every [`TerminalHandle`] is gone, or when its
//! [`TerminalEvents`] is dropped: a stream nobody reads is a reconnect worker nobody owns.

use std::collections::VecDeque;

use farhelm_proto::DetachCode;
use farhelm_proto::grid::{GRID_MAX_MESSAGE, GridInput, GridOutput};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio::time::{Instant, sleep_until, timeout};
use tokio_tungstenite::tungstenite::Message;

use crate::ChannelConfig;
use crate::ws::{ConnectError, HelmEndpoint, open_socket};
use farhelm_proto::ui_channel::ApiPath;

/// How many events may wait for the caller before the stream stops reading its socket. A grid output message is
/// usually a few lines of changes, so this is a few screens of backlog; the number is a judgment, not a measurement.
const EVENT_QUEUE: usize = 256;

/// The identity every terminal of one session view shares, so the supervisor can tell this client's own reconnect
/// (which replaces quietly) from another client's takeover. Mint one per session view and reuse it for its agent
/// terminal, tabs and reconnects; sharing one between clients would hide a takeover.
///
/// Keep at most one live stream per terminal per lease: close or drop the old stream before opening another for the
/// same terminal. The supervisor replaces a same-lease attachment instead of deferring to it, so two such streams take
/// the terminal from each other on every reconnect, each time with a full replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentLease(String);

impl AttachmentLease {
    pub fn new() -> AttachmentLease {
        AttachmentLease(uuid::Uuid::new_v4().to_string())
    }
}

impl Default for AttachmentLease {
    fn default() -> AttachmentLease {
        AttachmentLease::new()
    }
}

/// Which terminal to attach, and at what size.
#[derive(Debug, Clone)]
pub struct TerminalAttach {
    /// The session's served id, as the sessions collection keys it.
    pub session: String,
    /// `None` is the agent terminal; `Some` is exactly that tab, as the session's tab list names it.
    pub tab: Option<String>,
    pub lease: AttachmentLease,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamState {
    Attaching,
    /// A new attachment began; reset the grid model, a full replay follows.
    Attached,
    /// The stream dropped and is coming back by itself.
    Reconnecting,
    /// Stopped by a `taken_over`, `tab_closed` or `stalled` detach notice. Waits for
    /// [`TerminalHandle::take_control`].
    Detached {
        code: Option<DetachCode>,
        reason: String,
    },
    /// The helm refused the upgrade (401 credential, 404 session gone, ...). Waits for `take_control`.
    Refused {
        status: u16,
    },
    /// [`TerminalHandle::close`] was called, or every handle was dropped. Final.
    Closed,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TerminalEvent {
    State(StreamState),
    Output(GridOutput),
    /// The attachment's replay is done; what follows is live.
    ReplayComplete,
}

enum Request {
    Input(GridInput),
    Resize { cols: u16, rows: u16 },
    TakeControl,
    ReconnectNow,
    Close,
}

/// The caller's side of one stream. Cheap to clone.
#[derive(Clone)]
pub struct TerminalHandle {
    requests: mpsc::UnboundedSender<Request>,
}

impl TerminalHandle {
    /// Send input. Input while the stream is not attached is dropped, not replayed later: keystrokes typed into a
    /// dead terminal must not land in whatever it reconnects to.
    pub fn input(&self, input: GridInput) {
        let _ = self.requests.send(Request::Input(input));
    }

    /// Fit the terminal to a new size; also the size every later reconnect asks for.
    pub fn resize(&self, cols: u16, rows: u16) {
        let _ = self.requests.send(Request::Resize { cols, rows });
    }

    /// Attach again, taking control from whoever holds the terminal. Only meaningful when detached or refused.
    pub fn take_control(&self) {
        let _ = self.requests.send(Request::TakeControl);
    }

    /// Skip the current reconnect backoff and attach now, through the non-displacing route. Ignored in every other
    /// state: it never takes a terminal from anyone.
    pub fn reconnect_now(&self) {
        let _ = self.requests.send(Request::ReconnectNow);
    }

    pub fn close(&self) {
        let _ = self.requests.send(Request::Close);
    }
}

/// The stream's ordered event feed. Bounded: a caller that stops reading stops the stream reading its socket, and
/// dropping this ends the stream.
pub struct TerminalEvents {
    events: mpsc::Receiver<TerminalEvent>,
}

impl TerminalEvents {
    pub async fn recv(&mut self) -> Option<TerminalEvent> {
        self.events.recv().await
    }

    pub fn try_recv(&mut self) -> Option<TerminalEvent> {
        self.events.try_recv().ok()
    }
}

/// Attach one terminal on the current tokio runtime. Panics outside one.
///
/// At most one live stream per terminal per [`AttachmentLease`]; see there.
pub fn open_terminal(
    endpoint: HelmEndpoint,
    attach: TerminalAttach,
    config: ChannelConfig,
) -> (TerminalHandle, TerminalEvents) {
    let (requests_tx, requests) = mpsc::unbounded_channel();
    let (events_tx, events) = mpsc::channel(EVENT_QUEUE);
    let stream = Stream {
        endpoint,
        attach,
        config,
        requests,
        events: events_tx,
        reconnecting_shown: false,
    };
    tokio::spawn(stream.run());
    (
        TerminalHandle {
            requests: requests_tx,
        },
        TerminalEvents { events },
    )
}

/// The path for one attach. `take_control` picks the displacing route.
pub(crate) fn attach_path(
    attach: &TerminalAttach,
    take_control: bool,
) -> Result<ApiPath, crate::PathError> {
    let mut segments = vec!["sessions", attach.session.as_str(), "term"];
    if !take_control {
        segments.push("unowned");
    }
    let (cols, rows) = (attach.cols.to_string(), attach.rows.to_string());
    let mut query = vec![
        ("cols", cols.as_str()),
        ("rows", rows.as_str()),
        ("lease", attach.lease.0.as_str()),
    ];
    if let Some(tab) = &attach.tab {
        query.push(("tab", tab.as_str()));
    }
    ApiPath::api(&segments)?.with_query(&query)
}

/// The helm's text frames on a terminal socket. Anything else is ignored, so a newer helm can add notices.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Notice {
    Pong,
    ReplayComplete,
    Detached {
        reason: String,
        #[serde(default)]
        code: Option<DetachCode>,
    },
    #[serde(other)]
    Unknown,
}

/// How one socket ended.
enum Ended {
    /// The socket dropped, its watchdog fired, or the helm detached it for a reason that keeps recovering
    /// (`replaced`, `other`). Two separate facts decide what happens next: `attached` (the socket got as far as an
    /// attachment, so the backoff starts over) and `skip_wait` (the attachment answered a ping and then lost its
    /// socket, so the link was working and an immediate reconnect is earned). A detach notice never skips the wait.
    Dropped {
        attached: bool,
        skip_wait: bool,
    },
    /// A detach that waits for the user: `taken_over`, `tab_closed` or `stalled`.
    Detached {
        code: Option<DetachCode>,
        reason: String,
    },
    Closed,
}

/// Whether a detach notice stops the stream until the user takes control. The match is exhaustive so a new code is
/// a decision someone has to make here. A missing code comes only from a helm older than the codes and is treated
/// like `other`.
fn detach_stops(code: Option<DetachCode>) -> bool {
    match code {
        Some(DetachCode::TakenOver | DetachCode::TabClosed | DetachCode::Stalled) => true,
        Some(DetachCode::Replaced | DetachCode::Other) | None => false,
    }
}

struct Stream {
    endpoint: HelmEndpoint,
    attach: TerminalAttach,
    config: ChannelConfig,
    requests: mpsc::UnboundedReceiver<Request>,
    events: mpsc::Sender<TerminalEvent>,
    /// Whether `Reconnecting` is the state the caller last saw, so an outage is announced once however many attempts
    /// it takes. Cleared by every other state.
    reconnecting_shown: bool,
}

impl Stream {
    async fn run(mut self) {
        let mut take_control = true;
        let mut backoff = self.config.reconnect_initial;
        if !self
            .emit(TerminalEvent::State(StreamState::Attaching))
            .await
        {
            return;
        }
        loop {
            let ended = match self.connect(take_control).await {
                None => Ended::Closed,
                Some(Ok(socket)) => self.serve(socket).await,
                Some(Err(ConnectError::Refused { status })) => {
                    self.reconnecting_shown = false;
                    if !self
                        .emit(TerminalEvent::State(StreamState::Refused { status }))
                        .await
                        || !self.wait_for_take_control().await
                        || !self
                            .emit(TerminalEvent::State(StreamState::Attaching))
                            .await
                    {
                        return;
                    }
                    take_control = true;
                    backoff = self.config.reconnect_initial;
                    continue;
                }
                Some(Err(error)) => {
                    tracing::debug!(%error, "terminal attach attempt failed");
                    Ended::Dropped {
                        attached: false,
                        skip_wait: false,
                    }
                }
            };
            take_control = false;
            match ended {
                Ended::Closed => break,
                Ended::Dropped {
                    attached,
                    skip_wait,
                } => {
                    // Announced once per outage, not once per failed attempt.
                    if !self.reconnecting_shown {
                        self.reconnecting_shown = true;
                        if !self
                            .emit(TerminalEvent::State(StreamState::Reconnecting))
                            .await
                        {
                            return;
                        }
                    }
                    if attached {
                        backoff = self.config.reconnect_initial;
                    }
                    if !skip_wait {
                        let until = Instant::now() + backoff;
                        backoff = (backoff * 2).min(self.config.reconnect_max);
                        if !self.idle(Some(until)).await {
                            break;
                        }
                    }
                }
                Ended::Detached { code, reason } => {
                    self.reconnecting_shown = false;
                    if !self
                        .emit(TerminalEvent::State(StreamState::Detached { code, reason }))
                        .await
                        || !self.wait_for_take_control().await
                        || !self
                            .emit(TerminalEvent::State(StreamState::Attaching))
                            .await
                    {
                        return;
                    }
                    take_control = true;
                    backoff = self.config.reconnect_initial;
                }
            }
        }
        self.emit(TerminalEvent::State(StreamState::Closed)).await;
    }

    /// Open the socket, serving requests meanwhile. `None` when the stream was closed.
    async fn connect(
        &mut self,
        take_control: bool,
    ) -> Option<Result<crate::ws::Socket, ConnectError>> {
        let path = match attach_path(&self.attach, take_control) {
            Ok(path) => path,
            // An empty or dot session id has no route; the helm would answer 404, so say that.
            Err(_) => return Some(Err(ConnectError::Refused { status: 404 })),
        };
        let endpoint = self.endpoint.clone();
        let watchdog = self.config.watchdog;
        let opening =
            async move { open_socket(&endpoint, path.as_str(), GRID_MAX_MESSAGE, watchdog).await };
        tokio::pin!(opening);
        loop {
            tokio::select! {
                result = &mut opening => return Some(result),
                _ = self.events.closed() => return None,
                request = self.requests.recv() => match request {
                    None | Some(Request::Close) => return None,
                    // The size goes into the next attach's query; the one in flight already asked for the old one.
                    Some(Request::Resize { cols, rows }) => (self.attach.cols, self.attach.rows) = (cols, rows),
                    Some(Request::Input(_) | Request::TakeControl | Request::ReconnectNow) => {}
                },
            }
        }
    }

    /// Serve one open socket until it ends.
    async fn serve(&mut self, socket: crate::ws::Socket) -> Ended {
        let (mut sink, mut stream) = socket.split();
        let watchdog_period = self.config.watchdog;
        let mut attached = false;
        // Whether this attachment answered a ping: the evidence that the link worked, which alone earns an immediate
        // reconnect when the socket later drops.
        let mut answered = false;
        let mut ping_sent: Option<Instant> = None;
        let mut next_ping = Instant::now() + self.config.ping_interval;
        // Events read from the socket and not yet accepted by the caller's bounded queue. While this is non-empty the
        // socket is not read, so a slow caller slows the stream instead of growing this.
        let mut pending: VecDeque<TerminalEvent> = VecDeque::new();
        let ended = loop {
            let waiting_on_caller = !pending.is_empty();
            let watchdog = async {
                match ping_sent {
                    Some(sent) if !waiting_on_caller => sleep_until(sent + watchdog_period).await,
                    _ => std::future::pending().await,
                }
            };
            tokio::select! {
                biased;
                permit = self.events.reserve(), if waiting_on_caller => match permit {
                    Ok(permit) => {
                        if let Some(event) = pending.pop_front() {
                            permit.send(event);
                        }
                        if pending.is_empty() && ping_sent.is_some() {
                            // The watchdog was paused while the caller was behind; restart its clock now rather than
                            // charging the link for the caller's delay.
                            ping_sent = Some(Instant::now());
                        }
                    }
                    Err(_) => break Ended::Closed,
                },
                _ = self.events.closed(), if !waiting_on_caller => break Ended::Closed,
                frame = stream.next(), if !waiting_on_caller => match frame {
                    Some(Ok(Message::Binary(bytes))) => match GridOutput::decode(&bytes) {
                        Ok(output) => {
                            Self::mark_attached(&mut attached, &mut pending, &mut self.reconnecting_shown);
                            pending.push_back(TerminalEvent::Output(output));
                        }
                        Err(error) => {
                            tracing::warn!(%error, "the terminal stream carried an undecodable grid message");
                            // A protocol fault, not a lost link: it would likely recur, so it never skips the wait.
                            break Ended::Dropped { attached, skip_wait: false };
                        }
                    },
                    Some(Ok(Message::Text(text))) => match serde_json::from_str::<Notice>(&text) {
                        Ok(Notice::Pong) => {
                            answered |= attached && ping_sent.is_some();
                            ping_sent = None;
                        }
                        Ok(Notice::ReplayComplete) => {
                            Self::mark_attached(&mut attached, &mut pending, &mut self.reconnecting_shown);
                            pending.push_back(TerminalEvent::ReplayComplete);
                        }
                        Ok(Notice::Detached { reason, code }) if detach_stops(code) => {
                            break Ended::Detached { code, reason };
                        }
                        Ok(Notice::Detached { reason, code }) => {
                            tracing::debug!(?code, %reason, "terminal detached; backing off and reattaching");
                            break Ended::Dropped { attached, skip_wait: false };
                        }
                        Ok(Notice::Unknown) | Err(_) => {}
                    },
                    Some(Ok(_)) => {}
                    Some(Err(_)) | None => break Ended::Dropped { attached, skip_wait: answered },
                },
                request = self.requests.recv() => {
                    let message = match request {
                        None | Some(Request::Close) => break Ended::Closed,
                        Some(Request::TakeControl | Request::ReconnectNow) => continue,
                        Some(Request::Input(input)) => match input.encode() {
                            Ok(bytes) if attached => Message::binary(bytes),
                            Ok(_) => continue,
                            Err(error) => {
                                tracing::warn!(%error, "dropping terminal input that does not encode");
                                continue;
                            }
                        },
                        Some(Request::Resize { cols, rows }) => {
                            (self.attach.cols, self.attach.rows) = (cols, rows);
                            Message::text(serde_json::json!({"type": "resize", "cols": cols, "rows": rows}).to_string())
                        }
                    };
                    // A write the peer never drains would otherwise block this loop, and with it the watchdog.
                    if !matches!(timeout(watchdog_period, sink.send(message)).await, Ok(Ok(()))) {
                        break Ended::Dropped { attached, skip_wait: answered };
                    }
                },
                _ = watchdog => {
                    tracing::info!("a terminal stream's watchdog fired");
                    break Ended::Dropped { attached, skip_wait: answered };
                }
                _ = sleep_until(next_ping), if !waiting_on_caller => {
                    next_ping = Instant::now() + self.config.ping_interval;
                    if ping_sent.is_none() {
                        ping_sent = Some(Instant::now());
                        let ping = sink.send(Message::text(r#"{"type":"ping"}"#));
                        if !matches!(timeout(watchdog_period, ping).await, Ok(Ok(()))) {
                            break Ended::Dropped { attached, skip_wait: answered };
                        }
                    }
                }
            }
        };
        let _ = timeout(watchdog_period, sink.close()).await;
        // Hand over what the socket delivered before it ended (the tail of a replay, say), unless the caller is gone.
        if !matches!(ended, Ended::Closed) {
            for event in pending {
                if !self.emit(event).await {
                    return Ended::Closed;
                }
            }
        }
        ended
    }

    /// Queue `Attached` ahead of an attachment's first output.
    ///
    /// Takes the stream's fields rather than `self` because it runs inside the select that borrows the event sender.
    fn mark_attached(
        attached: &mut bool,
        pending: &mut VecDeque<TerminalEvent>,
        reconnecting_shown: &mut bool,
    ) {
        if !*attached {
            *attached = true;
            *reconnecting_shown = false;
            pending.push_back(TerminalEvent::State(StreamState::Attached));
        }
    }

    /// Wait outside a socket until `until`, or for the user when `until` is `None`. False when the stream was closed.
    ///
    /// During a backoff (`until` set) a reconnect-now request ends the wait early; while waiting for the user only
    /// take-control does.
    async fn idle(&mut self, until: Option<Instant>) -> bool {
        loop {
            let timeout = async {
                match until {
                    Some(at) => sleep_until(at).await,
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                _ = self.events.closed() => return false,
                request = self.requests.recv() => match request {
                    None | Some(Request::Close) => return false,
                    Some(Request::TakeControl) if until.is_none() => return true,
                    Some(Request::ReconnectNow) if until.is_some() => return true,
                    Some(Request::Resize { cols, rows }) => (self.attach.cols, self.attach.rows) = (cols, rows),
                    Some(Request::Input(_) | Request::TakeControl | Request::ReconnectNow) => {}
                },
                _ = timeout => return true,
            }
        }
    }

    /// Wait for the user to take control. False when the stream was closed.
    async fn wait_for_take_control(&mut self) -> bool {
        self.idle(None).await
    }

    /// Hand one event to the caller, waiting for room in the queue. False when the caller dropped its feed.
    async fn emit(&self, event: TerminalEvent) -> bool {
        self.events.send(event).await.is_ok()
    }
}

#[cfg(test)]
mod tests;
