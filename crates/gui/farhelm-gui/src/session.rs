//! The connection to the helm and the resettable UI-session owner.
//!
//! SPEC_impl.md, "One failure path: the reset", and SPEC.md, "Losing the connection to the helm". The GUI holds one
//! [`Connection`] for the life of the app. It drains the UI channel's event feed in order and turns it into three
//! things a view can read: the channel's state (which decides whether the window is live or inert), the current
//! [`UiSession`] (everything that came from the helm, and the work started on its behalf), and the notices a reset
//! leaves behind.
//!
//! Here's the TLDR of a reset. However the channel broke, the same code runs: the channel announces its new state
//! (`Reconnecting`, `Displaced`, `CannotConnect` or `HelmEnded`), and the connection retires the current UI session
//! before anything else happens. Retiring drops the session's store and cancels every task the session owns, including
//! the task waiting on a completion that was already on its way, so that completion is never delivered. Then every
//! window's dialogs and menus close, through the layer manager. Only then does the channel fail the commands it lost,
//! so those failures land on nobody. When the channel is back, its snapshot builds a brand-new session, and the
//! connection posts the generic connection-lost notice. Nothing in between tracks
//! what became of an individual action.
//!
//! Terminal streams are not on the UI channel (SPEC_impl.md, "Bulk data stays separate"), but they are opened through
//! the UI session all the same ([`UiSession::terminal_opener`]), so that a terminal view made for a session that a
//! reset retired cannot attach anything afterwards. The streams themselves stay with their owners, which close them
//! when they see the session retire.
//!
//! The one thing besides the helm's own state that survives a reset, the selected session's id, lives outside the
//! session ([`crate::local_state`]); the connection drops it only when a complete snapshot no longer has that session.
//!
//! The connection's pump is the one task that reads the feed. It never awaits a command (that would deadlock, see the
//! helm client's `ChannelEvents`); commands are awaited by tasks the UI session owns.

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use farhelm_helm_client::terminal::{
    TerminalAttach, TerminalEvents, TerminalHandle, closed_terminal,
};
use farhelm_helm_client::{
    ApiPath, ChannelConfig, ChannelEvent, ChannelEvents, ChannelHandle, ChannelState, CommandError,
    HelmConnection, HelmEndpoint, Method,
};
use farhelm_proto::ui_channel::{Availability, Collection, Reply, SessionRow};
use gpui::{App, AppContext as _, Context, Entity, Global, Task, WeakEntity};
use serde_json::Value;

use crate::layers;
use crate::local_state::LocalState;
use crate::notices::{self, Notice};
use crate::store::HelmStore;
use crate::terminal::attached::OpenStream;

/// The app's connection to its helm: the channel's state and the current UI session.
pub struct Connection {
    handle: ChannelHandle,
    state: ChannelState,
    session: Option<Entity<UiSession>>,
    /// Whether a channel opened before: the first connect is not a reset and posts no notice.
    was_connected: bool,
    /// Whether the channel broke since it was last connected, so the next connect is a recovery.
    lost: bool,
    /// Where each new UI session's terminal streams come from.
    terminals: TerminalSource,
    /// The reader of the channel's event feed.
    _pump: Task<()>,
    /// The helm client's connection, whose runtime must outlive the channel; `None` in tests, which serve a fake. The
    /// one strong reference: terminal openers hold it weakly.
    _owner: Option<Rc<HelmConnection>>,
}

/// Everything the window got from the helm in one connection, and the work started on its behalf.
///
/// A reset retires it: its tasks are cancelled and any completion still on its way is dropped, so nothing done for an
/// old connection can reach the window after the reset. One rule for views: reach the store through [`current_store`]
/// when drawing, and never keep a session or store handle across draws, so a view cannot go on drawing a retired
/// session's data.
pub struct UiSession {
    store: Entity<HelmStore>,
    handle: ChannelHandle,
    /// Where this session's terminal streams come from: the connection's source when the snapshot made the session.
    terminals: TerminalSource,
    /// Set once, when a reset retires the session. Shared so that a terminal opener handed out earlier, which runs
    /// with no GPUI context to read the session through, can still see the retirement (it holds this weakly, so a
    /// dropped session counts as retired too).
    retired: Rc<Cell<bool>>,
    /// Commands in flight, each awaited by a task this session owns. Dropping a task cancels its callback.
    pending: HashMap<u64, Task<()>>,
    next_pending: u64,
}

/// Opens terminal streams for a connection's UI sessions; each session's [`UiSession::terminal_opener`] calls it.
/// Never fails: where no stream may be opened, the one it hands back is already over.
type TerminalSource = Rc<dyn Fn(TerminalAttach) -> (TerminalHandle, TerminalEvents)>;

/// The source of a connection that owns the helm client's connection `helm`. It holds `helm` weakly, so an opener a
/// view keeps never keeps the helm client's runtime alive after the app's connection is gone; a stream asked for then
/// is already closed.
fn helm_terminals(helm: &Rc<HelmConnection>) -> TerminalSource {
    let helm = Rc::downgrade(helm);
    Rc::new(move |attach| match helm.upgrade() {
        Some(helm) => helm.terminal(attach, ChannelConfig::default()),
        None => closed_terminal(),
    })
}

struct ConnectionGlobal(Entity<Connection>);

impl Global for ConnectionGlobal {}

/// Open the UI channel to the helm at `endpoint` and start serving it. The real app's entry; tests use [`start`] with
/// the fake helm's handle and feed.
pub fn connect(endpoint: HelmEndpoint, cx: &mut App) -> anyhow::Result<Entity<Connection>> {
    let (connection, events) = HelmConnection::open(endpoint, ChannelConfig::default())?;
    let handle = connection.channel().clone();
    Ok(start(handle, events, Some(connection), cx))
}

/// Serve a UI channel: drain `events` in order, keeping the connection's state and UI session current. `owner` is kept
/// alive as long as the connection (the helm client's runtime, in the real app).
pub fn start(
    handle: ChannelHandle,
    mut events: ChannelEvents,
    owner: Option<HelmConnection>,
    cx: &mut App,
) -> Entity<Connection> {
    let owner = owner.map(Rc::new);
    // Without a helm client (a test serving the fake channel that set no source), every stream is already closed.
    let terminals: TerminalSource = match &owner {
        Some(helm) => helm_terminals(helm),
        None => Rc::new(|_| closed_terminal()),
    };
    let connection = cx.new(|cx| {
        let weak = cx.entity().downgrade();
        let pump = cx.spawn(async move |_, cx| {
            while let Some(event) = events.recv().await {
                if !cx.update(|cx| receive(&weak, event, cx)) {
                    break;
                }
            }
        });
        Connection {
            handle,
            state: ChannelState::Connecting,
            session: None,
            was_connected: false,
            lost: false,
            terminals,
            _pump: pump,
            _owner: owner,
        }
    });
    cx.set_global(ConnectionGlobal(connection.clone()));
    connection
}

/// The app's connection, once the bootstrap started it.
pub fn connection(cx: &App) -> Option<Entity<Connection>> {
    cx.try_global::<ConnectionGlobal>()
        .map(|global| global.0.clone())
}

/// The current UI session's store, for a view to read while it draws; `None` while the window is inert. The one way a
/// view reaches pushed state: looked up at render, so it is always the live session's.
pub fn current_store(cx: &App) -> Option<Entity<HelmStore>> {
    let connection = connection(cx)?;
    let session = connection.read(cx).session.clone()?;
    Some(session.read(cx).store.clone())
}

/// Apply one event from the feed. Returns whether the connection still exists.
fn receive(connection: &WeakEntity<Connection>, event: ChannelEvent, cx: &mut App) -> bool {
    let Some(connection) = connection.upgrade() else {
        return false;
    };
    match event {
        ChannelEvent::State(state) => {
            let (recovered, retiring) = connection.update(cx, |connection, cx| {
                let outcome = connection.set_state(state);
                cx.notify();
                outcome
            });
            if let Some(session) = retiring {
                retire(&session, cx);
                // SPEC.md: open dialogs and menus start over on a reset. Popovers would close anyway, with the live
                // ground they are anchored to, but a dialog closes only when its opener or its subject says so, and
                // its veil would cover Take back or Retry on the inert window.
                layers::close_all(cx);
            }
            if recovered {
                notices::post(Notice::ConnectionLost, cx);
            }
        }
        ChannelEvent::Snapshot(snapshot) => {
            let session = cx.new(|cx| {
                let connection = connection.read(cx);
                let handle = connection.handle.clone();
                let terminals = connection.terminals.clone();
                UiSession {
                    store: cx.new(|_| HelmStore::from_snapshot(&snapshot)),
                    handle,
                    terminals,
                    retired: Rc::new(Cell::new(false)),
                    pending: HashMap::new(),
                    next_pending: 0,
                }
            });
            keep_survivors(&session, cx);
            connection.update(cx, |connection, cx| {
                if let Some(old) = connection.session.replace(session) {
                    retire(&old, cx);
                }
                cx.notify();
            });
        }
        ChannelEvent::Changes(changes) => {
            if let Some(session) = connection.read(cx).session.clone() {
                let store = session.read(cx).store.clone();
                store.update(cx, |store, cx| store.apply(&changes, cx));
            }
        }
    }
    true
}

/// Drop the selected session if the new snapshot shows it is gone.
///
/// Only a complete, successful read can show that: the sessions collection is the channel's session query, cut at the
/// helm's listing cap, and a failed read arrives as `Unavailable` with no entries. A missing row in a truncated,
/// filtered or unavailable list is not evidence that the session was deleted (SPEC_impl.md, "What is pushed, and how
/// changes are found"), so the selection is kept then.
fn keep_survivors(session: &Entity<UiSession>, cx: &mut App) {
    // Checked rather than assumed, as the notice center and the layer manager do: `global` panics on a missing global.
    let Some(key) = cx
        .try_global::<LocalState>()
        .and_then(|local| local.selected_session.clone())
    else {
        return;
    };
    let collections = session.read(cx).store.read(cx).collections();
    let complete = collections.meta(Collection::Sessions).is_some_and(|meta| {
        meta.availability == Availability::Available
            && meta
                .counts
                .is_none_or(|counts| !counts.truncated && counts.matching.is_none())
    });
    if complete && collections.get::<SessionRow>(&key).is_none() {
        cx.global_mut::<LocalState>().selected_session = None;
    }
}

/// Retire `session`: cancel its work and mark it, so a completion still on its way is dropped.
fn retire(session: &Entity<UiSession>, cx: &mut App) {
    session.update(cx, |session, cx| {
        session.retired.set(true);
        session.pending.clear();
        cx.notify();
    });
}

impl Connection {
    /// Where the channel stands. Anything but `Connected` means the window is inert.
    pub fn state(&self) -> &ChannelState {
        &self.state
    }

    /// The current UI session; `None` while the window is inert.
    pub fn session(&self) -> Option<&Entity<UiSession>> {
        self.session.as_ref()
    }

    /// The user takes this helm back from the window that displaced this one.
    pub fn take_back(&self) {
        self.handle.take_back();
    }

    /// The user tries again after the helm refused the app's credential.
    pub fn retry(&self) {
        self.handle.retry();
    }

    /// Hand terminal streams out from `open` instead of the helm client, for UI sessions made from here on (a session
    /// takes its source when a snapshot makes it). Tests serve fake terminals through this.
    #[cfg(test)]
    pub(crate) fn serve_terminals(
        &mut self,
        open: impl Fn(TerminalAttach) -> (TerminalHandle, TerminalEvents) + 'static,
    ) {
        self.terminals = Rc::new(open);
    }

    /// Record the channel's new state. Returns whether this connect recovers from a break (when the connection-lost
    /// notice is due) and, when the window goes inert, the session to retire.
    fn set_state(&mut self, state: ChannelState) -> (bool, Option<Entity<UiSession>>) {
        let connected = state == ChannelState::Connected;
        let retiring = if connected { None } else { self.session.take() };
        // `lost` is only ever set after a first connect, so it alone marks a recovery.
        let recovered = connected && self.lost;
        if connected {
            self.was_connected = true;
            self.lost = false;
        } else if self.was_connected {
            self.lost = true;
        }
        self.state = state;
        (recovered, retiring)
    }
}

impl UiSession {
    /// This session's store.
    pub fn store(&self) -> &Entity<HelmStore> {
        &self.store
    }

    /// Whether a reset retired this session. A retired session runs nothing and delivers nothing.
    pub fn is_retired(&self) -> bool {
        self.retired.get()
    }

    /// An opener for one terminal stream through this session's connection to the helm, for a terminal view's owner
    /// to call once it knows the terminal's size.
    ///
    /// The opener checks the session when it is called, not when it is handed out. If a reset or a displacement has
    /// retired the session by then, or the session is gone, it opens nothing and hands back a stream that is already
    /// closed, whose feed holds only `Closed`. So an opener kept across a reset never attaches a terminal for a window
    /// that went inert. For a displaced window that matters beyond
    /// tidiness: a stream's first attach displaces, so it could take the terminal from the window that now holds the
    /// helm (SPEC_impl.md, "The seat: one UI per helm"). Retiring does not close streams opened before it; their
    /// owners do that when they see the session retire.
    pub fn terminal_opener(&self) -> OpenStream {
        let retired = Rc::downgrade(&self.retired);
        let terminals = self.terminals.clone();
        Box::new(move |attach| {
            if retired.upgrade().is_none_or(|retired| retired.get()) {
                closed_terminal()
            } else {
                terminals(attach)
            }
        })
    }

    /// Send one command, and call `on_done` with the helm's reply once it arrives, unless a reset retired this session
    /// first. The reply arrives after the store already shows the command's effects (the channel orders replies after
    /// the changes before them), so `on_done` needs no optimistic update. A command sent from a retired session is
    /// dropped.
    pub fn command(
        &mut self,
        method: Method,
        path: ApiPath,
        body: Option<Value>,
        cx: &mut Context<UiSession>,
        on_done: impl FnOnce(&mut UiSession, Result<Reply, CommandError>, &mut Context<UiSession>)
        + 'static,
    ) {
        if self.retired.get() {
            return;
        }
        let id = self.next_pending;
        self.next_pending += 1;
        let reply = self.handle.command(method, path, body);
        let task = cx.spawn(async move |session, cx| {
            let result = reply.await;
            // Retiring the session drops this task with the rest of `pending`, so a task that runs at all belongs to a
            // live session.
            let _ = session.update(cx, |session, cx| {
                session.pending.remove(&id);
                on_done(session, result, cx);
            });
        });
        self.pending.insert(id, task);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use farhelm_helm_client::terminal::{AttachmentLease, StreamState, TerminalEvent};

    /// The app's own terminal source is the one path the GUI tests' fake terminals never reach, and two things about it
    /// matter: it opens real streams through the helm client's connection, and it holds that connection weakly, so a
    /// view that keeps an opener does not keep the helm client's runtime (its threads and sockets) alive once the app
    /// has let go of it. Specified: while the connection lives, a stream from the source is not the already-closed
    /// one; once the connection is dropped, it is.
    ///
    /// No helm runs here. The listener, on a port the system picks, only gives the stream an address that is not
    /// someone else's and never accepts. The live case reads the feed without waiting: a real stream may not have
    /// shown `Attaching` yet, but only an already-closed one shows `Closed` at once.
    #[test]
    fn the_helm_source_opens_real_streams_only_while_the_helm_connection_lives() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = HelmEndpoint::new(listener.local_addr().unwrap(), "test-secret");
        let (helm, _channel) = HelmConnection::open(endpoint, ChannelConfig::default()).unwrap();
        let helm = Rc::new(helm);
        let terminals = helm_terminals(&helm);
        let attach = || TerminalAttach {
            session: "s1".into(),
            tab: None,
            lease: AttachmentLease::new(),
            cols: 80,
            rows: 24,
        };
        let closed = Some(TerminalEvent::State(StreamState::Closed));

        let (_live, mut events) = terminals(attach());
        assert_ne!(
            events.try_recv(),
            closed,
            "a live connection opens a real stream"
        );

        drop(helm);
        let (_gone, mut events) = terminals(attach());
        assert_eq!(
            events.try_recv(),
            closed,
            "a dropped connection opens nothing"
        );
        assert_eq!(events.try_recv(), None, "and that stream is over");
    }
}
