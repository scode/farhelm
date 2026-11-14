//! Tests of the connection and UI-session owner (and what its reset does to the window), the store, the ticker and the
//! notice center.
//!
//! The helm is the helm client's fake: in process, ordered like the real channel, and moved only by the test, which is
//! what GPUI's deterministic scheduler needs. Each test stages exactly the feed it is about (a reply queued before a
//! break, a displacement, a refused credential) and lets the app drain it.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use farhelm_helm_client::collections::decode_state;
use farhelm_helm_client::fake::terminal::{FakePhase, FakeTerminal};
use farhelm_helm_client::fake::{FakeDecision, FakeHelm};
use farhelm_helm_client::terminal::{
    AttachmentLease, StreamState, TerminalAttach, TerminalEvent, TerminalEvents, TerminalHandle,
};
use farhelm_helm_client::{ApiPath, ChannelState, Method, PushedState};
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionChanges, CollectionMeta, CollectionState, KeyedEntry,
    ListCounts, SessionRow,
};
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, IntoElement, Modifiers, ParentElement, Render,
    Styled, TestAppContext, Window, div, px,
};
use gpui_component::Root;

use crate::components::{Button, HoverHelp, Role};
use crate::focus::Mark;
use crate::layers::{self, Dialog};
use crate::local_state::LocalState;
use crate::main_view::MainView;
use crate::notices::{self, Notice};
use crate::session::{self, Connection, UiSession};
use crate::store::HelmStore;
use crate::terminal::attached::OpenStream;
use crate::{render_count, test_support, ticker};

// ---- Helpers -------------------------------------------------------------------------------------------------------

pub(crate) fn meta(order: &[&str]) -> CollectionMeta {
    CollectionMeta {
        availability: Availability::Available,
        order: order.iter().map(|key| key.to_string()).collect(),
        counts: None,
    }
}

/// A session row under `key`: the helm's own pinned list fixture, so the test follows the wire shape the helm serves.
pub(crate) fn session_entry(key: &str) -> KeyedEntry {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../farhelm-helm/http-contract/session-list.json"
    ))
    .unwrap();
    KeyedEntry {
        key: key.into(),
        value: fixture["sessions"][0].clone(),
    }
}

/// The sessions collection holding rows under `keys`, in that order.
pub(crate) fn sessions(keys: &[&str]) -> CollectionState {
    CollectionState {
        collection: Collection::Sessions,
        meta: meta(keys),
        entries: keys.iter().map(|key| session_entry(key)).collect(),
    }
}

/// The app, bootstrapped as for real, serving the fake helm's channel. The channel starts `Connecting`.
pub(crate) fn started(cx: &mut TestAppContext) -> (FakeHelm, Entity<Connection>) {
    test_support::install(cx);
    let (fake, handle, events) = FakeHelm::new();
    let connection = cx.update(|cx| session::start(handle, events, None, cx));
    cx.run_until_parked();
    (fake, connection)
}

fn state(connection: &Entity<Connection>, cx: &mut TestAppContext) -> ChannelState {
    cx.update(|cx| connection.read(cx).state().clone())
}

fn current(connection: &Entity<Connection>, cx: &mut TestAppContext) -> Option<Entity<UiSession>> {
    cx.update(|cx| connection.read(cx).session().cloned())
}

/// The quiet-point consistency check (SPEC_impl.md, "What is pushed"): the current session's store must equal a fresh
/// decode of `expected`, the whole state the helm holds now.
fn check_store(
    connection: &Entity<Connection>,
    expected: &[CollectionState],
    cx: &mut TestAppContext,
) {
    let fresh: Vec<PushedState> = expected
        .iter()
        .cloned()
        .map(|state| decode_state(state).unwrap())
        .collect();
    let differences = cx.update(|cx| {
        let session = connection.read(cx).session().unwrap().clone();
        let store = session.read(cx).store().clone();
        store.read(cx).collections().check_against(&fresh)
    });
    assert_eq!(
        differences,
        Vec::<String>::new(),
        "the store matches a fresh snapshot"
    );
}

/// A view that reaches the store through [`session::current_store`] while it draws and keeps no handle to it, the
/// pattern every view that shows helm state follows. It subscribes to nothing, so the only thing that can redraw it is
/// GPUI invalidating a window whose last frame read an entity that notified. `drawn` is how many sessions it drew last.
struct StoreReader {
    drawn: Rc<Cell<usize>>,
}

impl Render for StoreReader {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("reader", cx);
        let rows = session::current_store(cx).map_or(0, |store| {
            store.read(cx).collections().ordered::<SessionRow>().len()
        });
        self.drawn.set(rows);
        div().child(format!("{rows} sessions"))
    }
}

/// The current UI session's store, as a view would reach it.
fn current_store(cx: &mut TestAppContext) -> Option<Entity<HelmStore>> {
    cx.update(|cx| session::current_store(cx))
}

/// A change that adds the session `key` at the end of a list that held `before`.
fn adding(before: &[&str], key: &'static str) -> CollectionChanges {
    let mut order = before.to_vec();
    order.push(key);
    CollectionChanges {
        collection: Collection::Sessions,
        meta: meta(&order),
        upsert: vec![session_entry(key)],
        remove: vec![],
    }
}

// ---- The store and the UI-session owner ----------------------------------------------------------------------------

/// The store is fed only by the channel, and a change through it must redraw the views that read it: GPUI redraws only
/// for an entity that notifies, so a missed notification would leave the list showing a session that is gone
/// (SPEC_impl.md, "The store, and how views redraw"). Also checks the store against a fresh snapshot after each step.
#[gpui::test]
fn a_snapshot_fills_the_store_and_a_pushed_change_redraws_the_views_that_read_it(
    cx: &mut TestAppContext,
) {
    let (mut fake, connection) = started(cx);
    assert_eq!(state(&connection, cx), ChannelState::Connecting);
    assert!(
        current(&connection, cx).is_none(),
        "no session before the first snapshot"
    );

    fake.connect(vec![sessions(&["a", "b"])]);
    cx.run_until_parked();
    assert_eq!(state(&connection, cx), ChannelState::Connected);
    check_store(&connection, &[sessions(&["a", "b"])], cx);

    let drawn = Rc::new(Cell::new(0));
    let shown = drawn.clone();
    let _window = cx.add_window(move |_, _| StoreReader { drawn: shown });
    cx.run_until_parked();
    assert_eq!(drawn.get(), 2, "the premise: the reader drew the snapshot");
    cx.update(render_count::reset);

    fake.push(adding(&["a", "b"], "c"));
    cx.run_until_parked();
    check_store(&connection, &[sessions(&["a", "b", "c"])], cx);
    assert_eq!(
        cx.update(|cx| render_count::count("reader", cx)),
        1,
        "the change redrew the view that reads the store, once"
    );
    assert_eq!(drawn.get(), 3, "and it drew the change");
}

/// A reset replaces the store, and the views that read it are not rebuilt, so a view must still redraw when the new
/// store changes. Views look the store up at render and keep no handle (SPEC_impl.md, "The store, and how views
/// redraw"); one that kept the first store's handle, or observed it, would show the retired store forever. The reader
/// here holds no subscription, so a pass can only come from it reading the new store while it draws.
#[gpui::test]
fn a_change_pushed_after_a_reset_redraws_a_view_that_reads_the_store(cx: &mut TestAppContext) {
    let (mut fake, connection) = started(cx);
    fake.connect(vec![sessions(&["a"])]);
    cx.run_until_parked();
    let first = current_store(cx).unwrap();
    let drawn = Rc::new(Cell::new(0));
    let shown = drawn.clone();
    let _window = cx.add_window(move |_, _| StoreReader { drawn: shown });
    cx.run_until_parked();
    assert_eq!(
        drawn.get(),
        1,
        "the premise: the reader drew the first snapshot"
    );

    fake.break_channel();
    fake.connect(vec![sessions(&["a", "b"])]);
    cx.run_until_parked();
    let second = current_store(cx).unwrap();
    assert_ne!(
        first.entity_id(),
        second.entity_id(),
        "the premise: the reset replaced the store"
    );
    assert_eq!(
        drawn.get(),
        2,
        "the premise: the reader drew the new snapshot"
    );
    cx.update(render_count::reset);

    fake.push(adding(&["a", "b"], "c"));
    cx.run_until_parked();
    check_store(&connection, &[sessions(&["a", "b", "c"])], cx);
    assert_eq!(
        cx.update(|cx| render_count::count("reader", cx)),
        1,
        "the change to the new store redrew the reader, once"
    );
    assert_eq!(drawn.get(), 3, "and it drew the new store's change");
}

/// SPEC_impl.md, "One failure path: the reset" names this test: a completion queued across a reset must not reach the
/// new UI session. The reply here is already on the feed when the channel breaks, so only the session owner stands
/// between it and the window that was rebuilt meanwhile; without that, a stale "done" could act on the new session.
#[gpui::test]
fn a_completion_queued_across_a_reset_never_reaches_the_new_session(cx: &mut TestAppContext) {
    let (mut fake, connection) = started(cx);
    fake.connect(vec![sessions(&["a"])]);
    cx.run_until_parked();
    let first = current(&connection, cx).unwrap();

    let delivered = Rc::new(Cell::new(0));
    let seen = delivered.clone();
    cx.update(|cx| {
        first.update(cx, |session, cx| {
            session.command(
                Method::Delete,
                ApiPath::api(&["sessions", "a"]).unwrap(),
                None,
                cx,
                move |_, _, _| seen.set(seen.get() + 1),
            )
        })
    });
    cx.run_until_parked();
    let sent = fake.commands();
    assert_eq!(sent.len(), 1, "the premise: the command reached the helm");

    // The reply, the break and the reconnect all wait on the feed before the app reads any of them.
    fake.reply(sent[0].id, 200, None);
    fake.break_channel();
    fake.connect(vec![sessions(&["a"])]);
    cx.run_until_parked();

    assert_eq!(delivered.get(), 0, "the reply reached nobody");
    let second = current(&connection, cx).unwrap();
    assert_ne!(
        first.entity_id(),
        second.entity_id(),
        "the reconnect built a new session"
    );
    assert!(cx.update(|cx| first.read(cx).is_retired()));
    assert!(!cx.update(|cx| second.read(cx).is_retired()));
    check_store(&connection, &[sessions(&["a"])], cx);
}

/// The other order: the channel breaks before the reply, so the command fails with the opaque reset error. That
/// failure must not reach anyone either; the generic notice is the only report (no per-action outcome tracking).
#[gpui::test]
fn a_command_lost_in_a_break_reports_nothing_and_the_notice_covers_it(cx: &mut TestAppContext) {
    let (mut fake, connection) = started(cx);
    fake.connect(vec![sessions(&[])]);
    cx.run_until_parked();
    let first = current(&connection, cx).unwrap();
    let delivered = Rc::new(Cell::new(0));
    let seen = delivered.clone();
    cx.update(|cx| {
        first.update(cx, |session, cx| {
            session.command(
                Method::Post,
                ApiPath::api(&["sessions"]).unwrap(),
                None,
                cx,
                move |_, _, _| seen.set(seen.get() + 1),
            )
        })
    });
    cx.run_until_parked();
    assert_eq!(fake.commands().len(), 1);

    fake.break_channel();
    cx.run_until_parked();
    assert_eq!(state(&connection, cx), ChannelState::Reconnecting);
    assert!(
        current(&connection, cx).is_none(),
        "the inert window holds nothing from the helm"
    );
    assert!(
        cx.update(|cx| notices::posted(cx)).is_empty(),
        "no notice while still reconnecting"
    );

    fake.connect(vec![sessions(&[])]);
    cx.run_until_parked();
    assert_eq!(
        delivered.get(),
        0,
        "the lost command's failure reached nobody"
    );
    assert_eq!(
        cx.update(|cx| notices::posted(cx)),
        vec![Notice::ConnectionLost],
        "the recovery posts the generic notice"
    );
    cx.update(|cx| notices::withdraw(Notice::ConnectionLost, cx));
    assert!(
        cx.update(|cx| notices::posted(cx)).is_empty(),
        "dismissing withdraws it"
    );
}

/// The first connect is not a reset: a window that just opened has lost nothing and must not tell the user it might
/// have.
#[gpui::test]
fn the_first_connect_posts_no_notice(cx: &mut TestAppContext) {
    let (mut fake, _connection) = started(cx);
    fake.connect(vec![sessions(&[])]);
    cx.run_until_parked();
    assert!(cx.update(|cx| notices::posted(cx)).is_empty());
}

/// SPEC.md, "One GUI at a time": a displaced window goes wholly inert and does not reconnect on its own; only the
/// user's take-back does, and the rebuilt window then reports the gap like any other reset.
#[gpui::test]
fn a_displaced_window_waits_for_take_back_then_rebuilds(cx: &mut TestAppContext) {
    let (mut fake, connection) = started(cx);
    fake.connect(vec![sessions(&["a"])]);
    cx.run_until_parked();

    fake.displace();
    cx.run_until_parked();
    assert_eq!(state(&connection, cx), ChannelState::Displaced);
    assert!(current(&connection, cx).is_none());
    assert!(fake.decisions().is_empty(), "nothing reconnects by itself");

    cx.update(|cx| connection.read(cx).take_back());
    cx.run_until_parked();
    assert_eq!(fake.decisions(), vec![FakeDecision::TakeBack]);

    fake.connect(vec![sessions(&["a"])]);
    cx.run_until_parked();
    assert_eq!(state(&connection, cx), ChannelState::Connected);
    assert_eq!(
        cx.update(|cx| notices::posted(cx)),
        vec![Notice::ConnectionLost]
    );
}

// ---- Terminal streams through the UI session -----------------------------------------------------------------------

/// The fake terminals a test's connection handed out, each with the attach it was opened for.
type Opened = Rc<RefCell<Vec<(FakeTerminal, TerminalAttach)>>>;

/// [`started`], with the connection handing out fake terminals and recording each one in the returned list.
fn started_with_terminals(cx: &mut TestAppContext) -> (FakeHelm, Entity<Connection>, Opened) {
    let (fake, connection) = started(cx);
    let opened: Opened = Rc::default();
    let record = opened.clone();
    cx.update(|cx| {
        connection.update(cx, |connection, _| {
            connection.serve_terminals(move |attach| {
                let (terminal, handle, events) = FakeTerminal::open(&attach);
                record.borrow_mut().push((terminal, attach));
                (handle, events)
            })
        })
    });
    (fake, connection, opened)
}

/// An agent-terminal attach of session `key` at 100x30.
fn terminal_attach(key: &str) -> TerminalAttach {
    TerminalAttach {
        session: key.into(),
        tab: None,
        lease: AttachmentLease::new(),
        cols: 100,
        rows: 30,
    }
}

/// An opener of `session`'s terminals.
fn opener(session: &Entity<UiSession>, cx: &mut TestAppContext) -> OpenStream {
    cx.update(|cx| session.read(cx).terminal_opener())
}

/// The terminal view's owner opens its stream through the UI session, so the session must hand the owner's attach to
/// the connection unchanged (the session, the size the owner measured, the lease) and give back that very stream: an
/// opener that rebuilt the attach or kept the stream would attach the wrong terminal or leave the view with a feed
/// nobody fills. Specified: the connection is asked once, for exactly that attach, and the handle and feed the owner
/// gets are the ones it opened.
#[gpui::test]
fn a_live_sessions_opener_opens_the_attach_it_is_given(cx: &mut TestAppContext) {
    let (mut fake, connection, opened) = started_with_terminals(cx);
    fake.connect(vec![sessions(&["a"])]);
    cx.run_until_parked();
    let session = current(&connection, cx).unwrap();

    let open = opener(&session, cx);
    let attach = terminal_attach("a");
    let (handle, mut events) = open(attach.clone());
    let mut opened = opened.borrow_mut();
    assert_eq!(opened.len(), 1, "the connection opened one stream");
    let (terminal, asked) = &mut opened[0];
    assert_eq!(
        (
            &asked.session,
            asked.tab.as_ref(),
            asked.cols,
            asked.rows,
            &asked.lease
        ),
        (&attach.session, None, 100, 30, &attach.lease),
        "for the owner's attach, unchanged"
    );
    assert_eq!(
        events.try_recv(),
        Some(TerminalEvent::State(StreamState::Attaching)),
        "the feed is the opened stream's"
    );
    handle.close();
    assert_eq!(terminal.phase(), FakePhase::Closed, "and so is the handle");
}

/// Assert that `opened` is a stream that was already over: its feed holds `Closed` and then nothing.
fn assert_already_closed(opened: (TerminalHandle, TerminalEvents), why: &str) {
    let (_handle, mut events) = opened;
    assert_eq!(
        events.try_recv(),
        Some(TerminalEvent::State(StreamState::Closed)),
        "{why}"
    );
    assert_eq!(events.try_recv(), None, "{why}: the stream is over");
}

/// A reset or a displacement retires the UI session, and with it every way of attaching a terminal for it: a stream
/// opened for an inert window would come back by itself, and for a displaced one its first attach displaces, taking
/// the terminal from the window that now holds the helm (SPEC_impl.md, "The seat: one UI per helm"). The terminal
/// owner may still hold an opener it was given before, and after a reset nothing else holds the old session, so the
/// opener must refuse both while the retired session exists and once it is gone. Specified, for a break and then a
/// displacement: an opener taken before, one taken from the retired session, and one taken before and called once
/// the session was dropped all open nothing and hand back a stream whose feed holds only `Closed`; the session the
/// reconnect builds opens again.
#[gpui::test]
fn a_retired_session_opens_no_terminal_even_through_an_earlier_opener(cx: &mut TestAppContext) {
    let (mut fake, connection, opened) = started_with_terminals(cx);
    fake.connect(vec![sessions(&["a"])]);
    cx.run_until_parked();

    let retirements: [fn(&mut FakeHelm); 2] = [FakeHelm::break_channel, FakeHelm::displace];
    for retire in retirements {
        let session = current(&connection, cx).unwrap();
        let kept = opener(&session, cx);
        let kept_past_the_session = opener(&session, cx);
        retire(&mut fake);
        cx.run_until_parked();
        assert!(
            cx.update(|cx| session.read(cx).is_retired()),
            "the premise: the session retired"
        );

        assert_already_closed(kept(terminal_attach("a")), "an earlier opener");
        assert_already_closed(
            opener(&session, cx)(terminal_attach("a")),
            "the retired session's own opener",
        );
        // GPUI frees an entity's state only when the app next flushes its effects, after the last handle is gone; the
        // opener can see that only once the state itself, not just the handle, is gone.
        let released = Rc::new(Cell::new(false));
        let seen = released.clone();
        let _watch = cx.update(|cx| cx.observe_release(&session, move |_, _| seen.set(true)));
        drop(session);
        cx.update(|_| {});
        assert!(
            released.get(),
            "the premise: nothing keeps the retired session, and its state is gone"
        );
        assert_already_closed(
            kept_past_the_session(terminal_attach("a")),
            "an earlier opener called once the session is gone",
        );
        assert_eq!(opened.borrow().len(), 0, "no opener opened anything");

        if state(&connection, cx) == ChannelState::Displaced {
            cx.update(|cx| connection.read(cx).take_back());
        }
        fake.connect(vec![sessions(&["a"])]);
        cx.run_until_parked();
        let rebuilt = current(&connection, cx).unwrap();
        let _stream = opener(&rebuilt, cx)(terminal_attach("a"));
        assert_eq!(
            opened.borrow_mut().drain(..).count(),
            1,
            "the rebuilt session's opener opens"
        );
    }
}

/// A dialog's content: one button holding the focus handle the test gave it.
struct DialogBody {
    button: FocusHandle,
}

impl Render for DialogBody {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(300.)).p(px(20.)).child(
            Button::new(
                "dialog-button",
                "Dialog button",
                HoverHelp::tooltip("A control inside the open dialog"),
                Role::Button,
            )
            .focus(&self.button),
        )
    }
}

/// SPEC.md, "Losing the connection to the helm": open dialogs start over on a reset. A dialog closes only when its
/// opener or its subject says so, and its veil takes every click while Tab stays inside it, so a dialog that outlived
/// the break would sit over the inert window and keep the user from Take back by mouse and by keyboard. Specified: a
/// dialog open when the window is displaced is gone, focus is no longer inside it, and a click on Take back reaches the
/// helm.
#[gpui::test]
fn a_reset_closes_open_dialogs_so_take_back_stays_reachable(cx: &mut TestAppContext) {
    let (mut fake, connection) = started(cx);
    fake.connect(vec![sessions(&["a"])]);
    cx.run_until_parked();
    let (_, window) = cx.add_window_view(|window, cx| {
        let main = cx.new(|cx| MainView::new(window, cx));
        Root::new(main, window, cx)
    });
    // Simulated windows start inactive, and an inactive window has no focus path for keys or clicks to follow.
    window.update(|window, _| window.activate_window());
    window.run_until_parked();

    let (button, container) =
        window.update(|_, cx| (cx.focus_handle().tab_stop(true), cx.focus_handle()));
    let opened = window.update(|window, cx| {
        let view = cx.new(|_| DialogBody {
            button: button.clone(),
        });
        let mark = Mark::take(cx);
        layers::open_dialog(
            Dialog {
                view: view.into(),
                container: container.clone(),
                start: button.clone(),
                subject: None,
                main_action: None,
                keeps_focus_on_empty_click: true,
            },
            &mark,
            window,
            cx,
        )
    });
    window.run_until_parked();
    let Some(dialog) = opened else {
        panic!("the premise: the dialog opened");
    };
    assert!(
        window.update(|window, _| button.is_focused(window)),
        "the premise: focus is inside the dialog"
    );

    fake.displace();
    window.run_until_parked();
    assert_eq!(state(&connection, window), ChannelState::Displaced);
    assert!(
        !window.update(|window, cx| layers::is_open(dialog, window, cx)),
        "the reset closed the dialog"
    );
    assert!(
        !window.update(|window, cx| container.contains_focused(window, cx)),
        "focus left the closed dialog"
    );
    window.update(test_support::check_invariants);

    let take_back = match window.debug_bounds("connection-take-back") {
        Some(bounds) => bounds.center(),
        None => panic!("the inert window drew no Take back"),
    };
    window.simulate_click(take_back, Modifiers::none());
    window.run_until_parked();
    assert_eq!(
        fake.decisions(),
        vec![FakeDecision::TakeBack],
        "the click on Take back reached the helm"
    );
}

/// SPEC.md: a refused credential shows "cannot connect" with Retry rather than a reconnect loop.
#[gpui::test]
fn a_refused_credential_waits_for_retry(cx: &mut TestAppContext) {
    let (mut fake, connection) = started(cx);
    fake.refuse_credential();
    cx.run_until_parked();
    assert_eq!(
        state(&connection, cx),
        ChannelState::CannotConnect { status: 401 }
    );
    assert!(fake.decisions().is_empty());
    cx.update(|cx| connection.read(cx).retry());
    cx.run_until_parked();
    assert_eq!(fake.decisions(), vec![FakeDecision::Retry]);
}

/// SPEC.md: the selected session survives a reset, by id, only if it still exists. Selecting a session that is gone
/// would point the window at nothing, but only a complete list can show that it is gone: a failed read or a list cut at
/// the helm's cap keeps the selection, or a passing read error would lose what the user had open.
#[gpui::test]
fn the_selected_session_survives_a_reset_only_if_it_still_exists(cx: &mut TestAppContext) {
    let (mut fake, _connection) = started(cx);
    fake.connect(vec![sessions(&["a", "b"])]);
    cx.run_until_parked();
    cx.update(|cx| cx.global_mut::<LocalState>().selected_session = Some("a".into()));

    fake.break_channel();
    fake.connect(vec![sessions(&["a"])]);
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| cx.global::<LocalState>().selected_session.clone()),
        Some("a".into()),
        "still there, still selected"
    );

    fake.break_channel();
    fake.connect(vec![unavailable_sessions()]);
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| cx.global::<LocalState>().selected_session.clone()),
        Some("a".into()),
        "a failed read is no evidence that the session is gone"
    );

    fake.break_channel();
    fake.connect(vec![truncated_sessions(&["b"])]);
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| cx.global::<LocalState>().selected_session.clone()),
        Some("a".into()),
        "a list the helm's cap cut is no evidence either"
    );

    fake.break_channel();
    fake.connect(vec![sessions(&["b"])]);
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| cx.global::<LocalState>().selected_session.clone()),
        None,
        "gone from a complete list, so no longer selected"
    );
}

/// The sessions collection as a fresh channel sends it when the helm could not read the list.
fn unavailable_sessions() -> CollectionState {
    CollectionState {
        collection: Collection::Sessions,
        meta: CollectionMeta {
            availability: Availability::Unavailable {
                error: "the session store could not be read".into(),
            },
            order: Vec::new(),
            counts: None,
        },
        entries: Vec::new(),
    }
}

/// The sessions collection holding rows under `keys`, cut by the helm's listing cap.
fn truncated_sessions(keys: &[&str]) -> CollectionState {
    let mut state = sessions(keys);
    state.meta.counts = Some(ListCounts {
        total: 500,
        matching: None,
        truncated: true,
    });
    state
}

// ---- The ticker ----------------------------------------------------------------------------------------------------

/// A view that animates for as long as it holds the ticker's handle.
struct Spinner {
    animating: Option<ticker::Animating>,
}

impl Render for Spinner {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("spinner", cx);
        div()
    }
}

/// SPEC_impl.md, "Animation time: the ticker": nothing redraws on its own more than 10 times a second, and nothing
/// animates in a window that is not active. Without the shared ticker one animating row redraws its list at the
/// display's rate, and a background window keeps a CPU core busy. Activity comes from GPUI's own window state here (no
/// override), the path the shipped app takes.
#[gpui::test]
fn an_animating_view_redraws_at_most_ten_times_a_second_and_not_while_inactive(
    cx: &mut TestAppContext,
) {
    test_support::install(cx);
    let (spinner, cx) = cx.add_window_view(|window, cx| Spinner {
        animating: Some(ticker::animate(window, cx)),
    });
    // Simulated windows open inactive; activate it the way the platform reports a focused window.
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    assert!(
        cx.update(|window, _| window.is_window_active()),
        "premise: the window is active"
    );
    cx.update(|_, cx| render_count::reset(cx));

    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    let active = cx.update(|_, cx| render_count::count("spinner", cx));
    assert!(
        (9..=10).contains(&active),
        "about ten redraws in a second while active, never more: {active}"
    );

    cx.deactivate_window();
    assert!(
        !cx.update(|window, _| window.is_window_active()),
        "premise: the window is now inactive"
    );
    cx.update(|_, cx| render_count::reset(cx));
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(
        cx.update(|_, cx| render_count::count("spinner", cx)),
        0,
        "no redraws while the window is inactive"
    );

    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    assert!(
        cx.update(|window, _| window.is_window_active()),
        "premise: the window is active again"
    );
    cx.update(|_, cx| render_count::reset(cx));
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert!(
        cx.update(|_, cx| render_count::count("spinner", cx)) > 0,
        "ticks resume once the window is active again"
    );

    spinner.update(cx, |spinner, _| spinner.animating = None);
    cx.update(|_, cx| render_count::reset(cx));
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(
        cx.update(|_, cx| render_count::count("spinner", cx)),
        0,
        "dropping the handle stops the ticks"
    );
}

/// The headless Mutter runs of the test-automation bridge report every window inactive forever, so agent builds pin
/// activity on; the override must make an inactive window tick, or nothing would animate in those runs.
#[gpui::test]
fn the_active_override_makes_an_inactive_window_tick(cx: &mut TestAppContext) {
    test_support::install(cx);
    let (_spinner, cx) = cx.add_window_view(|window, cx| Spinner {
        animating: Some(ticker::animate(window, cx)),
    });
    cx.run_until_parked();
    cx.deactivate_window();
    cx.update(|_, cx| {
        ticker::set_active_override(Some(true), cx);
        render_count::reset(cx);
    });
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert!(
        cx.update(|_, cx| render_count::count("spinner", cx)) > 0,
        "the override counts the inactive window as active"
    );
}

/// Under reduced motion views keep their state but nothing moves on its own: no animation ticks at all, even in an
/// active window, where they would otherwise run.
#[gpui::test]
fn reduced_motion_delivers_no_animation_ticks(cx: &mut TestAppContext) {
    test_support::install(cx);
    cx.update(|cx| ticker::set_reduced_motion(true, cx));
    let (_spinner, cx) = cx.add_window_view(|window, cx| Spinner {
        animating: Some(ticker::animate(window, cx)),
    });
    // Active, so that only reduced motion can be what stops the ticks.
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    assert!(
        cx.update(|window, _| window.is_window_active()),
        "premise: the window is active"
    );
    cx.update(|_, cx| render_count::reset(cx));
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| render_count::count("spinner", cx)), 0);
}

/// One-shot deadlines are about the user's time, not drawing: they fire on time even with every window inactive, and
/// dropping one cancels it (how a hover that ends cancels its tooltip).
#[gpui::test]
fn a_deadline_fires_on_time_whatever_the_activity_and_dropping_it_cancels_it(
    cx: &mut TestAppContext,
) {
    test_support::install(cx);
    cx.update(|cx| ticker::set_active_override(Some(false), cx));
    let fired = Rc::new(Cell::new(false));
    let cancelled = Rc::new(Cell::new(false));
    let (on_fire, on_cancelled) = (fired.clone(), cancelled.clone());
    let _kept =
        cx.update(|cx| ticker::after(Duration::from_millis(500), cx, move |_| on_fire.set(true)));
    let dropped = cx.update(|cx| {
        ticker::after(Duration::from_millis(500), cx, move |_| {
            on_cancelled.set(true)
        })
    });
    drop(dropped);

    cx.executor().advance_clock(Duration::from_millis(499));
    cx.run_until_parked();
    assert!(!fired.get(), "not before its time");
    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    assert!(fired.get(), "on time, with no window active");
    assert!(!cancelled.get(), "a dropped deadline never fires");
}
