//! The UI channel client: commands, heartbeats, the watchdog, and reconnecting.
//!
//! One task owns the channel and is the only thing that ever connects, so there is never more than one connection
//! attempt at a time. The GUI talks to it through a [`ChannelHandle`] (commands and the few explicit user decisions)
//! and hears from it through [`ChannelEvents`] (state changes and pushed collections).
//!
//! Here's the TLDR of the state machine:
//!
//! - `Connecting` on start, `Reconnecting` after a break. A connection counts as up (`Connected`) once its snapshot
//!   has arrived, not when the socket opens, so the GUI never shows a live channel over stale collections.
//! - A break (the socket drops, the watchdog fires, or the helm sends something the client cannot use) reconnects
//!   automatically. After a connection that had proven itself healthy (it answered a heartbeat) the first attempt is
//!   immediate; after that, and while attempts keep failing, waits double up to a cap. A helm that drops every
//!   connection right after its snapshot therefore gets a backoff, not a hot loop of snapshots and resets.
//! - `Displaced` (the helm said another UI took its seat) and `CannotConnect` (the helm refused the credential with a
//!   401) wait for the user: [`ChannelHandle::take_back`] and [`ChannelHandle::retry`]. Reconnecting by itself from
//!   `Displaced` would steal the seat back from the other UI, and retrying a refused credential cannot work.
//! - `HelmEnded` is final. The embedder reports it ([`ChannelHandle::helm_ended`]) when the helm it ran exited; a new
//!   helm gets a new connection.
//!
//! The watchdog is the one liveness rule: the channel is broken when the snapshot, an admitted command, or a ping
//! stays unanswered for [`ChannelConfig::watchdog`]. "Admitted" matters because the helm runs commands one at a time:
//! a command queued behind a slow one has not started, so its clock starts when the reply before it arrives. The
//! default of 30 s sits above the helm's own 20 s bound on waiting for a host, so a slow host shows up as a failed
//! reply rather than a broken channel.
//!
//! However the channel breaks, the GUI hears the new state first and only then sees its pending commands fail, all
//! with the one opaque [`CommandError::Reset`] (SPEC_impl.md, "One failure path: the reset"). The GUI drops its
//! UI-session owner when it reads the state, so those failures land on waiters that no longer exist; nothing on the
//! GUI side tracks what became of a command. One gap is left open on purpose: a command sent after a break but before
//! the GUI has read the new state can run on the next connection. That is the "may or may not have happened" case the
//! reset already tells the user about, so the client does not stamp commands with a connection generation to stop it.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use farhelm_proto::ui_channel::{ClientMessage, Collection, Command, Reply, ServerMessage};
use futures_util::future::BoxFuture;
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Instant, sleep_until};

use crate::collections::{PushedChanges, PushedState, decode_changes, decode_state};
use crate::path::ApiPath;
use crate::ws::ConnectError;

/// Timing for one channel, and for each terminal stream ([`crate::terminal`]), which follows the same liveness and
/// reconnect rules. Tests shrink or pause these; nothing here reads the wall clock (all of it is tokio time,
/// so a paused test clock drives it).
#[derive(Debug, Clone)]
pub struct ChannelConfig {
    /// How long the snapshot, an admitted command or a ping may stay unanswered before the channel counts as broken.
    pub watchdog: Duration,
    /// How often to ping while connected. Only one ping is outstanding at a time.
    pub ping_interval: Duration,
    /// The first wait after a failed attempt (or after a break that does not earn an immediate reconnect); it doubles
    /// per further failure up to `reconnect_max`.
    pub reconnect_initial: Duration,
    pub reconnect_max: Duration,
}

impl Default for ChannelConfig {
    fn default() -> ChannelConfig {
        ChannelConfig {
            watchdog: Duration::from_secs(30),
            ping_interval: Duration::from_secs(5),
            reconnect_initial: Duration::from_secs(1),
            reconnect_max: Duration::from_secs(8),
        }
    }
}

/// Where the channel stands, as the GUI should show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelState {
    Connecting,
    Connected,
    /// The connection broke and the channel is getting it back by itself.
    Reconnecting,
    /// Another UI took this helm's seat. Waits for [`ChannelHandle::take_back`].
    Displaced,
    /// The helm refused the device credential. Waits for [`ChannelHandle::retry`].
    CannotConnect {
        status: u16,
    },
    /// The embedder reported that the helm exited. Final.
    HelmEnded,
}

/// What the channel tells the GUI, in the order the helm sent it. Pushed state arrives already decoded.
#[derive(Debug, Clone, PartialEq)]
pub enum ChannelEvent {
    State(ChannelState),
    /// The whole pushed state, on every (re)connect. Replaces everything held before.
    Snapshot(Vec<PushedState>),
    Changes(PushedChanges),
}

/// Why a command has no reply: the session is resetting, or was never connected. Do nothing about it.
///
/// Deliberately one opaque variant with no advice attached. Whether the command reached the helm is not something
/// the GUI acts on: the channel announced its new state before this error fired, the reset that state starts drops
/// the UI session that sent the command, and the generic "connection lost" notice is the only report the user gets
/// (SPEC_impl.md, "One failure path: the reset"). A variant that said "safe to resend" would invite exactly the
/// per-action tracking that rule forbids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CommandError {
    #[error("the connection to the helm is resetting")]
    Reset,
}

/// A command's HTTP method. A closed set so a typo cannot reach the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Delete => "DELETE",
            Method::Patch => "PATCH",
        }
    }
}

/// How a command's completion travels. It rides the event queue, so it is delivered in order with the events.
pub(crate) type Completion = oneshot::Sender<Result<Reply, CommandError>>;

/// What a handle asks of the channel task. Public within the crate so [`crate::fake`] can serve the same handles.
pub(crate) enum Request {
    Command {
        method: Method,
        path: ApiPath,
        body: Option<Value>,
        done: Completion,
    },
    TakeBack,
    Retry,
    HelmEnded,
}

/// One item on the event queue: an event for the GUI, or a command completion to fire when the GUI reaches it.
pub(crate) enum Item {
    Event(ChannelEvent),
    Complete(Completion, Result<Reply, CommandError>),
}

/// The GUI's side of the channel's requests. Cheap to clone; every clone talks to the same channel.
#[derive(Clone)]
pub struct ChannelHandle {
    requests: mpsc::UnboundedSender<Request>,
}

impl ChannelHandle {
    pub(crate) fn new(requests: mpsc::UnboundedSender<Request>) -> ChannelHandle {
        ChannelHandle { requests }
    }

    /// Send one command. The future resolves with the helm's reply (any HTTP status, 202 included; a 202 settles the
    /// watchdog but promises nothing about the outcome) once every event sent before that reply has been handed out
    /// by [`ChannelEvents`]. A GUI that applies each event before asking for the next therefore sees the command's
    /// effects in its collections by the time the reply arrives.
    pub fn command(&self, method: Method, path: ApiPath, body: Option<Value>) -> CommandFuture {
        let (done, reply) = oneshot::channel();
        let _ = self.requests.send(Request::Command {
            method,
            path,
            body,
            done,
        });
        CommandFuture { reply }
    }

    /// The user chose to take this helm back from the UI that displaced this one. Ignored unless `Displaced`.
    pub fn take_back(&self) {
        let _ = self.requests.send(Request::TakeBack);
    }

    /// The user asked to try again after the helm refused the credential. Ignored in every other state: there is no
    /// user-facing way to hurry an automatic reconnect.
    pub fn retry(&self) {
        let _ = self.requests.send(Request::Retry);
    }

    /// The helm this channel talks to has exited. The channel stops for good.
    pub fn helm_ended(&self) {
        let _ = self.requests.send(Request::HelmEnded);
    }
}

/// A command's pending reply. Runtime-agnostic: any executor can poll it.
pub struct CommandFuture {
    reply: oneshot::Receiver<Result<Reply, CommandError>>,
}

impl Future for CommandFuture {
    type Output = Result<Reply, CommandError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // A dropped completion means the channel (or the event receiver) went away before answering, which is a reset
        // like any other.
        Pin::new(&mut self.reply)
            .poll(cx)
            .map(|result| result.unwrap_or(Err(CommandError::Reset)))
    }
}

/// The channel's ordered event feed.
///
/// NOTE: The GUI must keep draining this. Command completions are queued among the events and fire only when
/// [`ChannelEvents::recv`] reaches them, which is what orders a reply after the changes the helm pushed before it. A
/// GUI that stops reading also stops getting replies.
///
/// NOTE: The task that drains this must never itself await a [`CommandFuture`]. That future resolves only when this
/// feed is read past its completion, so awaiting it from the reader deadlocks. Await commands from other tasks (or
/// select over both, as the tests do).
pub struct ChannelEvents {
    items: mpsc::UnboundedReceiver<Item>,
}

impl ChannelEvents {
    pub(crate) fn new(items: mpsc::UnboundedReceiver<Item>) -> ChannelEvents {
        ChannelEvents { items }
    }

    /// The next event, firing every completion queued before it. `None` once the channel is gone.
    pub async fn recv(&mut self) -> Option<ChannelEvent> {
        loop {
            match self.items.recv().await? {
                Item::Event(event) => return Some(event),
                Item::Complete(done, result) => {
                    let _ = done.send(result);
                }
            }
        }
    }

    /// The next event if one is queued, without waiting. For synchronous test drivers.
    pub fn try_recv(&mut self) -> Option<ChannelEvent> {
        loop {
            match self.items.try_recv().ok()? {
                Item::Event(event) => return Some(event),
                Item::Complete(done, result) => {
                    let _ = done.send(result);
                }
            }
        }
    }
}

/// One live connection to the helm, whatever carries it. Dropping `tx` hangs up; `rx` yielding `None` means the
/// connection is gone.
pub struct Link {
    pub tx: mpsc::UnboundedSender<ClientMessage>,
    pub rx: mpsc::UnboundedReceiver<ServerMessage>,
}

/// Opens links. The real one is a WebSocket; channel tests script it.
pub trait Connector: Send + Sync + 'static {
    fn connect(&self) -> BoxFuture<'static, Result<Link, ConnectError>>;
}

/// Start a channel task on the current tokio runtime. Panics outside one.
pub fn spawn_channel(
    connector: Arc<dyn Connector>,
    config: ChannelConfig,
) -> (ChannelHandle, ChannelEvents) {
    let (requests_tx, requests) = mpsc::unbounded_channel();
    let (items, items_rx) = mpsc::unbounded_channel();
    let task = Task {
        backoff: config.reconnect_initial,
        connector,
        config,
        requests,
        items,
        next_id: 1,
        state: None,
        immediate_reconnect: false,
    };
    tokio::spawn(task.run());
    (
        ChannelHandle::new(requests_tx),
        ChannelEvents::new(items_rx),
    )
}

// ---------------------------------------------------------------------------
// The channel task
// ---------------------------------------------------------------------------

/// How one connection ended.
enum Ended {
    /// Dropped, watchdog, or a protocol violation. `was_up` says whether the snapshot had arrived.
    Broken {
        was_up: bool,
    },
    Displaced,
    Refused {
        status: u16,
    },
    HelmExited,
    /// Every handle is gone; nobody can use the channel any more.
    Abandoned,
}

/// What ends a wait outside a connection.
enum Wake {
    Retry,
    TakeBack,
    Exit,
}

struct Task {
    connector: Arc<dyn Connector>,
    config: ChannelConfig,
    requests: mpsc::UnboundedReceiver<Request>,
    items: mpsc::UnboundedSender<Item>,
    next_id: u64,
    state: Option<ChannelState>,
    /// The wait before the next attempt after a failure; doubles per failure up to `reconnect_max`.
    backoff: Duration,
    /// Whether the next break after being up may reconnect without waiting. Earned by a connection answering a
    /// heartbeat and spent by the first immediate reconnect, so a helm that keeps dropping connections right after
    /// their snapshot falls back to the backoff instead of a loop of snapshots and resets.
    immediate_reconnect: bool,
}

/// A command sent on the current link and not yet answered.
struct InFlight {
    id: u64,
    done: Completion,
}

impl Task {
    async fn run(mut self) {
        let mut reconnecting = false;
        loop {
            self.set_state(if reconnecting {
                ChannelState::Reconnecting
            } else {
                ChannelState::Connecting
            });
            let ended = self.connect_and_serve().await;
            reconnecting = true;
            match ended {
                Ended::Broken { was_up } => {
                    if was_up && std::mem::take(&mut self.immediate_reconnect) {
                        continue;
                    }
                    let until = Instant::now() + self.backoff;
                    self.backoff = (self.backoff * 2).min(self.config.reconnect_max);
                    // Nothing ends the wait early: the user has no way to hurry an automatic reconnect, and a
                    // take-back or retry means nothing while the channel is getting back by itself.
                    loop {
                        match self.idle_until(Some(until)).await {
                            None => break,
                            Some(Wake::Retry | Wake::TakeBack) => continue,
                            Some(Wake::Exit) => return,
                        }
                    }
                }
                Ended::Displaced => {
                    if !self.wait_for(Wake::TakeBack).await {
                        return;
                    }
                    self.backoff = self.config.reconnect_initial;
                }
                Ended::Refused { status } => {
                    self.set_state(ChannelState::CannotConnect { status });
                    if !self.wait_for(Wake::Retry).await {
                        return;
                    }
                    self.backoff = self.config.reconnect_initial;
                }
                Ended::HelmExited => {
                    self.set_state(ChannelState::HelmEnded);
                    self.serve_ended().await;
                    return;
                }
                Ended::Abandoned => return,
            }
        }
    }

    /// One connection attempt and, if it opens, the connection's whole life.
    async fn connect_and_serve(&mut self) -> Ended {
        let mut connecting = self.connector.connect();
        let link = loop {
            tokio::select! {
                result = &mut connecting => break result,
                request = self.requests.recv() => match request {
                    None => return Ended::Abandoned,
                    Some(request) => if let Some(Wake::Exit) = self.refuse(request) {
                        return Ended::HelmExited;
                    },
                },
            }
        };
        match link {
            Ok(link) => self.serve(link).await,
            // 401 is the credential; any other refusal of the upgrade is a helm that is not ready, so retry it.
            Err(ConnectError::Refused { status: 401 }) => Ended::Refused { status: 401 },
            Err(error) => {
                tracing::debug!(%error, "UI channel connection attempt failed");
                Ended::Broken { was_up: false }
            }
        }
    }

    /// Serve one open link until it ends.
    async fn serve(&mut self, mut link: Link) -> Ended {
        let opened = Instant::now();
        let mut up = false;
        // The collections the current snapshot installed. Changes to any other collection are a protocol violation.
        let mut installed: Vec<Collection> = Vec::new();
        let mut in_flight: VecDeque<InFlight> = VecDeque::new();
        // When the oldest in-flight command was admitted: sent onto an idle queue, or the reply before it arrived.
        let mut admitted = opened;
        let mut ping: Option<(u64, Instant)> = None;
        let mut next_ping_n = 1;
        let mut next_ping_at = opened + self.config.ping_interval;

        let ended = loop {
            let mut deadline = None::<Instant>;
            let mut watch =
                |at: Instant| deadline = Some(deadline.map_or(at, |d: Instant| d.min(at)));
            if !up {
                watch(opened + self.config.watchdog);
            }
            if !in_flight.is_empty() {
                watch(admitted + self.config.watchdog);
            }
            if let Some((_, sent)) = ping {
                watch(sent + self.config.watchdog);
            }
            let watchdog = async {
                match deadline {
                    Some(at) => sleep_until(at).await,
                    None => std::future::pending().await,
                }
            };

            tokio::select! {
                biased;
                message = link.rx.recv() => match message {
                    None => break Ended::Broken { was_up: up },
                    Some(ServerMessage::Snapshot { collections }) => {
                        let decoded = match collections.into_iter().map(decode_state).collect::<Result<Vec<_>, _>>() {
                            Ok(decoded) => decoded,
                            Err(error) => {
                                tracing::warn!(%error, "the helm's snapshot does not decode");
                                break Ended::Broken { was_up: up };
                            }
                        };
                        installed = decoded.iter().map(|state| state.collection).collect();
                        self.emit(ChannelEvent::Snapshot(decoded));
                        if !up {
                            up = true;
                            self.set_state(ChannelState::Connected);
                        }
                    }
                    Some(ServerMessage::Changes(changes)) => {
                        if !installed.contains(&changes.collection) {
                            tracing::warn!(collection = ?changes.collection, "the helm pushed changes before their snapshot");
                            break Ended::Broken { was_up: up };
                        }
                        match decode_changes(changes) {
                            Ok(decoded) => self.emit(ChannelEvent::Changes(decoded)),
                            Err(error) => {
                                tracing::warn!(%error, "the helm's changes do not decode");
                                break Ended::Broken { was_up: up };
                            }
                        }
                    }
                    Some(ServerMessage::Reply(reply)) => {
                        let Some(front) = in_flight.pop_front().filter(|front| front.id == reply.id) else {
                            tracing::warn!(id = reply.id, "the helm answered a command out of order");
                            break Ended::Broken { was_up: up };
                        };
                        self.complete(front.done, Ok(reply));
                        admitted = Instant::now();
                    }
                    Some(ServerMessage::Pong { n }) => {
                        if ping.is_some_and(|(sent, _)| sent == n) {
                            ping = None;
                            // This connection works end to end, so its next break may reconnect at once.
                            self.immediate_reconnect = true;
                            self.backoff = self.config.reconnect_initial;
                        }
                    }
                    Some(ServerMessage::Displaced) => break Ended::Displaced,
                },
                request = self.requests.recv() => match request {
                    None => break Ended::Abandoned,
                    Some(Request::Command { method, path, body, done }) => {
                        if !up {
                            self.complete(done, Err(CommandError::Reset));
                            continue;
                        }
                        let id = self.next_id;
                        self.next_id += 1;
                        if in_flight.is_empty() {
                            admitted = Instant::now();
                        }
                        in_flight.push_back(InFlight { id, done });
                        // No local queue or cap: the helm closes a channel whose queue overflows, and that is the
                        // same break as any other.
                        let command = Command { id, method: method.as_str().into(), path: path.as_str().into(), body };
                        if link.tx.send(ClientMessage::Command(command)).is_err() {
                            break Ended::Broken { was_up: up };
                        }
                    }
                    Some(Request::HelmEnded) => break Ended::HelmExited,
                    Some(Request::TakeBack | Request::Retry) => {}
                },
                _ = watchdog => {
                    tracing::info!("the UI channel's watchdog fired");
                    break Ended::Broken { was_up: up };
                }
                _ = sleep_until(next_ping_at) => {
                    next_ping_at += self.config.ping_interval;
                    if ping.is_none() && up {
                        ping = Some((next_ping_n, Instant::now()));
                        if link.tx.send(ClientMessage::Ping { n: next_ping_n }).is_err() {
                            break Ended::Broken { was_up: up };
                        }
                        next_ping_n += 1;
                    }
                }
            }
        };

        // Announce the new state before failing anything, so the GUI drops the UI session that is waiting on these
        // commands before their failures reach it.
        match ended {
            Ended::Broken { .. } => self.set_state(ChannelState::Reconnecting),
            Ended::Displaced => self.set_state(ChannelState::Displaced),
            Ended::HelmExited => self.set_state(ChannelState::HelmEnded),
            Ended::Refused { .. } | Ended::Abandoned => {}
        }
        for command in in_flight {
            self.complete(command.done, Err(CommandError::Reset));
        }
        ended
    }

    /// Wait outside a connection for `wanted`, refusing commands meanwhile. False when the channel should stop.
    async fn wait_for(&mut self, wanted: Wake) -> bool {
        loop {
            match self.idle_until(None).await {
                Some(Wake::Exit) => return false,
                Some(wake) if std::mem::discriminant(&wake) == std::mem::discriminant(&wanted) => {
                    return true;
                }
                _ => {}
            }
        }
    }

    /// Serve requests while not connected, until `until` (`None` on timeout) or a request that wakes the caller.
    async fn idle_until(&mut self, until: Option<Instant>) -> Option<Wake> {
        loop {
            let timeout = async {
                match until {
                    Some(at) => sleep_until(at).await,
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                request = self.requests.recv() => match request {
                    None => return Some(Wake::Exit),
                    Some(request) => if let Some(wake) = self.refuse(request) {
                        if let Wake::Exit = wake {
                            self.set_state(ChannelState::HelmEnded);
                            self.serve_ended().await;
                        }
                        return Some(wake);
                    },
                },
                _ = timeout => return None,
            }
        }
    }

    /// After the helm ended: refuse commands until every handle is gone.
    async fn serve_ended(&mut self) {
        while let Some(request) = self.requests.recv().await {
            self.refuse(request);
        }
    }

    /// Handle a request while not connected: commands fail at once, the rest become wakes.
    fn refuse(&mut self, request: Request) -> Option<Wake> {
        match request {
            Request::Command { done, .. } => {
                self.complete(done, Err(CommandError::Reset));
                None
            }
            Request::TakeBack => Some(Wake::TakeBack),
            Request::Retry => Some(Wake::Retry),
            Request::HelmEnded => Some(Wake::Exit),
        }
    }

    fn set_state(&mut self, state: ChannelState) {
        if self.state.as_ref() != Some(&state) {
            self.state = Some(state.clone());
            self.emit(ChannelEvent::State(state));
        }
    }

    fn emit(&self, event: ChannelEvent) {
        let _ = self.items.send(Item::Event(event));
    }

    fn complete(&self, done: Completion, result: Result<Reply, CommandError>) {
        let _ = self.items.send(Item::Complete(done, result));
    }
}

#[cfg(test)]
mod tests;
