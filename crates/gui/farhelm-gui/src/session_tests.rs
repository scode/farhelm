//! Tests of the connection and UI-session owner, and of what its reset does to the window: the replies and failures it
//! keeps from the new session, the notice it posts, the dialogs it closes, the terminal streams it refuses to open for a
//! retired session, and the selected session that survives it.
//!
//! The helm is the helm client's fake: in process, ordered like the real channel, and moved only by the test, which is
//! what GPUI's deterministic scheduler needs. Each test stages exactly the feed it is about (a reply queued before a
//! break, a displacement, a refused credential) and lets the app drain it.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use farhelm_helm_client::fake::terminal::{FakePhase, FakeTerminal};
use farhelm_helm_client::fake::{FakeDecision, FakeHelm};
use farhelm_helm_client::terminal::{
    AttachmentLease, StreamState, TerminalAttach, TerminalEvent, TerminalEvents, TerminalHandle,
};
use farhelm_helm_client::{ApiPath, ChannelState, Method};
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionMeta, CollectionState, ListCounts,
};
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, IntoElement, Modifiers, ParentElement, Render,
    Styled, TestAppContext, Window, div, px,
};
use gpui_component::Root;

use crate::components::{Button, HoverHelp, Role};
use crate::focus::{Mark, ReturnTo};
use crate::layers::{self, Dialog};
use crate::local_state;
use crate::main_view::MainView;
use crate::notices::{self, Notice};
use crate::session::{Connection, UiSession};
use crate::terminal::attached::OpenStream;
use crate::test_support::{self, check_store, current, sessions, started, state};

// ---- The UI-session owner -------------------------------------------------------------------------------------------

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
                returns_to: ReturnTo::Opener,
                may_dismiss: None,
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
    cx.update(|cx| local_state::select("a", cx));

    fake.break_channel();
    fake.connect(vec![sessions(&["a"])]);
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| local_state::selected(cx)),
        Some("a".into()),
        "still there, still selected"
    );

    fake.break_channel();
    fake.connect(vec![unavailable_sessions()]);
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| local_state::selected(cx)),
        Some("a".into()),
        "a failed read is no evidence that the session is gone"
    );

    fake.break_channel();
    fake.connect(vec![truncated_sessions(&["b"])]);
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| local_state::selected(cx)),
        Some("a".into()),
        "a list the helm's cap cut is no evidence either"
    );

    fake.break_channel();
    fake.connect(vec![sessions(&["b"])]);
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| local_state::selected(cx)),
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
