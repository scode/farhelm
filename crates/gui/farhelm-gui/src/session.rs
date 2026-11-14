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
//! before anything else happens. Retiring drops the session's store and cancels every task the session owns, its
//! operation store's included ([`crate::operations`]), down to the task waiting on a completion that was already on its
//! way, so that completion is never delivered. Then every window's dialogs and menus close, through the layer manager.
//! Only then does the channel fail the commands it lost, so those failures land on nobody. When the channel is back,
//! its snapshot builds a brand-new session, and the connection posts the generic connection-lost notice. Nothing in
//! between tracks what became of an individual action.
//!
//! Terminal streams are not on the UI channel (SPEC_impl.md, "Bulk data stays separate"), but they are opened through
//! the UI session all the same ([`UiSession::terminal_opener`]), so that a terminal view made for a session that a
//! reset retired cannot attach anything afterwards. The streams themselves stay with their owners, which close them
//! when they see the session retire.
//!
//! The one thing besides the helm's own state that survives a reset, the selected session's id, lives outside the
//! session ([`crate::local_state`]); the connection drops it only when a complete snapshot no longer has that session.
//!
//! A view that must act on pushed state as it changes, not only draw it (apply a stored preference, settle a rule),
//! follows the current session with [`observe_current`]. That is the one place that knows a reset replaces the store
//! under the view, so the view never keeps a subscription to a store the reset retired, and never misses the first
//! change of the store that replaced it. The session list and the session pane predate it and still follow the
//! session by hand.
//!
//! The connection's pump is the one task that reads the feed. It never awaits a command (that would deadlock, see the
//! helm client's `ChannelEvents`); commands are awaited by tasks the UI session owns.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use farhelm_helm_client::terminal::{
    TerminalAttach, TerminalEvents, TerminalHandle, closed_terminal,
};
use farhelm_helm_client::{
    ApiPath, ChannelConfig, ChannelEvent, ChannelEvents, ChannelHandle, ChannelState, Collections,
    CommandError, HelmConnection, HelmEndpoint, Method,
};
use farhelm_proto::ui_channel::{Availability, Collection, Reply, SessionRow};
use gpui::{
    App, AppContext as _, Context, Entity, EntityId, Global, Subscription, Task, WeakEntity,
};
use serde_json::Value;

use crate::layers::{self, Subject};
use crate::local_state;
use crate::notices::{self, Notice};
use crate::operations::Operations;
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
    /// Work that finishes later (on sessions, hosts, templates, the preferences, launches), retired with the UI session.
    operations: Entity<Operations>,
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

/// The current UI session's operation store, looked up the same way as [`current_store`] and for the same reason: a
/// view that reads it while drawing, or starts work from a handler, always reaches the live session's. `None` while the
/// window is inert.
pub fn current_operations(cx: &App) -> Option<Entity<Operations>> {
    let connection = connection(cx)?;
    let session = connection.read(cx).session.clone()?;
    Some(session.read(cx).operations.clone())
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
                    operations: cx.new(|_| Operations::new(handle.clone())),
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

/// Whether `collections` prove that the session filed under `key` is gone: the one evidence rule for "this session no
/// longer exists", shared by the selected session's survivor ([`keep_survivors`]) and the session subject of a layer
/// ([`Subject::session`]), so a selection and a confirmation about the same session can never disagree.
///
/// Only a complete, successful read can show that: the sessions collection is the channel's session query, cut at the
/// helm's listing cap, and a failed read arrives as `Unavailable` with no entries. A missing row in a truncated,
/// filtered or unavailable list is not evidence that the session was deleted (SPEC_impl.md, "What is pushed, and how
/// changes are found"), so this says `false` then, as it does before the collection has arrived at all.
pub(crate) fn listing_proves_gone(collections: &Collections, key: &str) -> bool {
    let complete = collections.meta(Collection::Sessions).is_some_and(|meta| {
        meta.availability == Availability::Available
            && meta
                .counts
                .is_none_or(|counts| !counts.truncated && counts.matching.is_none())
    });
    complete && collections.get::<SessionRow>(key).is_none()
}

impl Subject {
    /// A layer about the session the helm files under `key`, for a question or editor that must go when the session
    /// does: it closes once the current UI session's store proves the session gone ([`listing_proves_gone`]). A
    /// truncated, filtered or unavailable listing proves nothing, so a confirmation stays open through a passing read
    /// error rather than vanishing under the user's pointer. A layer asked about a session already proven gone closes as
    /// soon as it opens.
    ///
    /// `None` while the window is inert: with no UI session there is no store to watch and no evidence, and the layer is
    /// left to its opener. It watches the store current when it is made. A reset replaces that store, but a reset also
    /// closes every layer (`layers::close_all`, called when the session retires), so no layer outlives the store it
    /// watches.
    ///
    /// It lives here rather than in the layer manager so that the layer manager knows nothing of the store: it takes the
    /// store and the evidence rule through [`Subject::when`].
    pub fn session(key: impl Into<String>, cx: &App) -> Option<Subject> {
        let key: String = key.into();
        let store = current_store(cx)?;
        Some(Subject::when(store, move |store: &HelmStore, _| {
            listing_proves_gone(store.collections(), &key)
        }))
    }
}

/// Drop the selected session if the new snapshot proves it gone ([`listing_proves_gone`]); a list that proves nothing
/// keeps the selection.
fn keep_survivors(session: &Entity<UiSession>, cx: &mut App) {
    let Some(key) = local_state::selected(cx) else {
        return;
    };
    let collections = session.read(cx).store.read(cx).collections();
    if listing_proves_gone(collections, &key) {
        local_state::clear(cx);
    }
}

/// State a view keeps for one UI session, started over when the session changes.
///
/// Views must not keep a session's handles across draws or events (see [`UiSession`]), but some keep state that only
/// means something for the session it was made under: the session list's subscription to the current store, and what
/// it last stamped as seen. A reset makes a new UI session, and state from the old one would act for a connection that
/// is gone. So the view keeps that state here and reaches it only through [`PerSession::current`], which compares the
/// current UI session's entity id with the one the state was made under and starts the state over from its default when
/// they differ. While the window is inert there is no session, and the state is the default too.
///
/// The comparison happens when the view asks, not when the reset happens, so a view must reach the state only through
/// `current`.
#[derive(Default)]
pub struct PerSession<T: Default> {
    session: Option<EntityId>,
    state: T,
}

impl<T: Default> PerSession<T> {
    /// The state for the current UI session, started over if the session changed since the last call.
    pub fn current(&mut self, cx: &App) -> &mut T {
        let current = connection(cx).and_then(|connection| {
            connection
                .read(cx)
                .session
                .as_ref()
                .map(|session| session.entity_id())
        });
        if current != self.session || current.is_none() {
            self.session = current;
            self.state = T::default();
        }
        &mut self.state
    }
}

/// Call `on_change` on the view once just after this is made, on every change of the connection (its state, and so
/// its UI session), and on every change the current UI session's store applies, until the view drops the handle this
/// returns.
///
/// It is for a view that must act on pushed state rather than only draw it, such as applying the stored appearance to
/// the theme when the first snapshot arrives and whenever it changes. Drawing needs none of this (a view that reads the
/// store while it draws is redrawn by GPUI), but acting does, and following the current store by hand means watching
/// the connection, subscribing to each new session's store, and dropping the old subscription on a reset. This does
/// all three, so a view built on it can neither miss the first change of a store a reset brought in nor go on
/// observing a retired one. `on_change` reads whatever it needs through [`current_store`] each time; with the window
/// inert there is no store, and it is called then too, so it can tell the window went inert.
///
/// `on_change` runs inside an update of the view `V` (it is handed `&mut V`), never nested in the store's or the
/// connection's update: from GPUI's own observer callbacks or, for the first call, deferred to after the update that
/// made the handle (usually the view's own construction, when the view does not exist yet). So it may read the store
/// and the connection, and change the view through the `&mut V` it is given, but must not update the view again
/// through an entity handle, which would be a re-entrant update. Several
/// changes made in one update may reach it as one call, so it works from the state as it is then and never counts
/// calls.
///
/// It gets no `Window`, as GPUI's `Context::observe` gives none; applying a preference to the theme
/// (`theme::set_preference`) is app-wide and needs none.
///
/// It must be made after the connection ([`start`]): it watches the connection it finds when it is made, and without
/// one it makes only the first call. The bootstrap starts the connection before it opens a window.
pub fn observe_current<V: 'static>(
    cx: &mut Context<V>,
    on_change: impl Fn(&mut V, &mut Context<V>) + 'static,
) -> ObservingCurrent {
    let on_change: OnChange<V> = Rc::new(on_change);
    let watched = Rc::new(RefCell::new(None));
    follow_store(&watched, &on_change, cx);
    let connection = connection(cx).map(|connection| {
        // Weak, so dropping the handle drops the store subscription at once; a strong reference would not leak (GPUI
        // drops the observer with the view), but it would keep the old store observed until the view went.
        let follows = Rc::downgrade(&watched);
        let on_change = on_change.clone();
        cx.observe(&connection, move |view, _, cx| {
            let Some(watched) = follows.upgrade() else {
                return;
            };
            // Resubscribe before calling. A view must not notify the store from `on_change`, so the order only keeps
            // the subscription current for whatever `on_change` reads next.
            follow_store(&watched, &on_change, cx);
            on_change(view, cx);
        })
    });
    let view = cx.weak_entity();
    let alive = Rc::downgrade(&watched);
    cx.defer(move |cx| {
        if alive.upgrade().is_some() {
            let _ = view.update(cx, |view, cx| on_change(view, cx));
        }
    });
    ObservingCurrent {
        _connection: connection,
        _watched: watched,
    }
}

/// What [`observe_current`] hands back: the view keeps it as a field, and dropping it, or the view that holds it, ends
/// the calls at once, the first one included if it has not run yet.
///
/// It is the only strong owner of the subscription to the current store: the connection's observer reaches it
/// weakly, so the store subscription ends with the handle rather than with the view.
#[must_use = "dropping it ends the calls at once"]
pub struct ObservingCurrent {
    _connection: Option<Subscription>,
    _watched: Rc<RefCell<Option<WatchedStore>>>,
}

/// The store an [`ObservingCurrent`] is subscribed to. Its entity id is how a change of the connection tells whether
/// a reset replaced the store: a new UI session always brings a new store entity.
struct WatchedStore {
    store: EntityId,
    _changes: Subscription,
}

/// The callback an [`ObservingCurrent`] calls, shared by its connection and store subscriptions.
type OnChange<V> = Rc<dyn Fn(&mut V, &mut Context<V>)>;

/// Point `watched` at the current UI session's store: keep it if the store is the one already watched, otherwise drop
/// the old subscription (so a retired store is no longer observed) and subscribe to the new store, or to nothing while
/// the window is inert.
fn follow_store<V: 'static>(
    watched: &RefCell<Option<WatchedStore>>,
    on_change: &OnChange<V>,
    cx: &mut Context<V>,
) {
    let store = current_store(cx);
    let current = store.as_ref().map(|store| store.entity_id());
    if watched.borrow().as_ref().map(|watched| watched.store) == current {
        return;
    }
    let next = store.map(|store| {
        let on_change = on_change.clone();
        WatchedStore {
            store: store.entity_id(),
            _changes: cx.observe(&store, move |view, _, cx| on_change(view, cx)),
        }
    });
    // The old subscription drops here, once the borrow has ended.
    drop(watched.replace(next));
}

/// Retire `session`: cancel its work and mark it, so a completion still on its way is dropped.
fn retire(session: &Entity<UiSession>, cx: &mut App) {
    session.update(cx, |session, cx| {
        session.retired.set(true);
        session.pending.clear();
        session
            .operations
            .update(cx, |operations, cx| operations.retire(cx));
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
