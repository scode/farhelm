//! Tests of the main pane's terminal: which session it attaches and when it lets go of a stream, where focus goes when
//! the user or the selection rules open a session, what a Delete of the open session draws over it, and what a reset
//! does to the terminal.
//!
//! The window is the real main view over the helm client's fake channel, and its connection hands out the fake terminal
//! (`Connection::serve_terminals`), which plays the helm's side of every stream the pane opens and records it. The
//! tests about a text field holding focus while a terminal arrives use a field of their own beside the list and pane.
//! This lets them exercise typing across the terminal handoff. The shared step helpers check the focus and layer
//! invariants after each step.

use std::cell::RefCell;
use std::rc::Rc;

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_helm_client::fake::terminal::{FakePhase, FakeTerminal, FakeTerminalRequest};
use farhelm_helm_client::fake::transfers::{FakeTransfer, FakeTransfers};
use farhelm_helm_client::terminal::TerminalAttach;
use farhelm_input_mapping::Platform;
use farhelm_proto::DetachCode;
use farhelm_proto::grid::{GridInput, GridOutput};
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionChanges, CollectionState, HostView,
};
use gpui::{
    AppContext as _, ClipboardEntry, ClipboardItem, Context, Entity, ExternalPaths,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled as _, TestAppContext,
    VisualTestContext, Window, div, px,
};

use super::SessionPane;
use crate::actions::{DELETE_SESSION_CHORD, THIS_PLATFORM};
use crate::components::{HoverHelp, TextInput, TextInputState};
use crate::launcher::launch_tests::{
    HostChange, command_draft, creates_answering, start_host_change,
};
use crate::launcher::view_tests::fixture_hosts;
use crate::main_view::MainView;
use crate::notices::{self, Notice};
use crate::operations::{Command, OpKey, Work};
use crate::session::Connection;
use crate::session_list::SessionList;
use crate::session_list::host_rail::tests::{answer_everything, filtered, node, one_query};
use crate::session_list::tests::{
    carrying, change, listing, listing_meta, painted, preferences, preferences_state, session,
};
use crate::terminal::attached::{AttachedTerminal, STATUS_RECONNECT_ID};
use crate::test_support::{self, click, keys, settle, terminal_focused};
use crate::test_support::{frame, meta, started};
use crate::welcome::PANE_START_ID;
use crate::{focus, local_state, root};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// Every fake terminal the connection handed out, in the order the pane opened them, with the attach each was for.
type Opened = Rc<RefCell<Vec<(FakeTerminal, TerminalAttach)>>>;

/// A window over the fake helm, the pane in it, and the terminals the pane opened.
struct Fleet<'a> {
    cx: &'a mut VisualTestContext,
    helm: FakeHelm,
    pane: Entity<SessionPane>,
    opened: Opened,
}

/// The two sessions most tests use, a and b, with a the last visited, so a is open at startup.
fn a_and_b() -> Vec<CollectionState> {
    vec![
        listing(&["a", "b"], &[session("a"), session("b")]),
        preferences(&["a"], None),
    ]
}

/// The app connected to the fake helm with `snapshot`, its connection handing out fake terminals, and a root view that
/// `build` makes (which must hold a [`SessionPane`] and return it from `pane`) open in an active window.
fn open<V: Render>(
    cx: &mut TestAppContext,
    snapshot: Vec<CollectionState>,
    build: impl FnOnce(&mut Window, &mut Context<V>) -> V,
    pane: impl FnOnce(&V) -> Entity<SessionPane>,
) -> Fleet<'_> {
    let (mut helm, connection) = started(cx);
    let opened = serve_terminals(&connection, cx);
    helm.connect(snapshot);
    cx.run_until_parked();
    let (view, cx) = test_support::open_view(cx, build);
    let pane = cx.update(|_, cx| pane(view.read(cx)));
    Fleet {
        cx,
        helm,
        pane,
        opened,
    }
}

/// [`open`] with the real main view.
fn open_main(cx: &mut TestAppContext, snapshot: Vec<CollectionState>) -> Fleet<'_> {
    open(cx, snapshot, MainView::new, |main| {
        main.session_pane().clone()
    })
}

/// Have `connection` hand out a fake terminal for every stream opened from now on, recorded in the list returned.
fn serve_terminals(connection: &Entity<Connection>, cx: &mut TestAppContext) -> Opened {
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
    opened
}

impl Fleet<'_> {
    /// The sessions of the streams opened so far, in order.
    fn attached(&self) -> Vec<String> {
        self.opened
            .borrow()
            .iter()
            .map(|(_, attach)| attach.session.clone())
            .collect()
    }

    /// The phase of the `index`th stream opened.
    fn phase(&self, index: usize) -> FakePhase {
        self.opened.borrow_mut()[index].0.phase()
    }

    /// The `index`th stream's attach opens and replays a complete screen, so its terminal goes live.
    fn go_live(&mut self, index: usize) {
        {
            let mut opened = self.opened.borrow_mut();
            let terminal = &mut opened[index].0;
            let asked = terminal.attach();
            for message in screen(asked.cols, asked.rows) {
                terminal.output(message);
            }
            terminal.replay_complete();
        }
        settle(self.cx);
    }

    /// The input the `index`th stream received since the last call, with whether it was delivered to its program.
    fn input(&self, index: usize) -> Vec<(GridInput, bool)> {
        self.opened.borrow_mut()[index]
            .0
            .received()
            .into_iter()
            .filter_map(|request| match request {
                FakeTerminalRequest::Input { input, delivered } => Some((input, delivered)),
                _ => None,
            })
            .collect()
    }

    /// The session selected now.
    fn selected(&mut self) -> Option<String> {
        self.cx.update(|_, cx| local_state::selected(cx))
    }
}

/// A complete first screen of `cols` by `rows`, as the supervisor replays one: hello, state, one row of text, a frame.
fn screen(cols: u16, rows: u16) -> Vec<GridOutput> {
    [
        serde_json::json!({"t":"hello","cols":cols,"rows":rows,"history_cap":0}),
        serde_json::json!({"t":"state","mouse":"off","focus":false,"alt_screen":false,
            "default_fg":null,"default_bg":null,"history_trimmed":false,"exit":null}),
        serde_json::json!({"t":"rows","rows":[[0,{"w":false,"r":[[0,2,"$ ",[0,-1,-1]]]}]]}),
        serde_json::json!({"t":"frame","rev":1,"cols":cols,"rows":rows,
            "cursor":{"x":2,"y":0,"visible":true,"shape":"block","blink":false}}),
    ]
    .into_iter()
    .map(|json| serde_json::from_value(json).unwrap())
    .collect()
}

/// The `Key` message a plain letter sends on this platform, delivered.
fn letter(key: &str, kc: u32) -> (GridInput, bool) {
    let input = GridInput::Key {
        key: key.into(),
        code: String::new(),
        kc,
        mods: 0,
        mac: THIS_PLATFORM == Platform::Mac,
    };
    (input, true)
}

// ---- Attaching -----------------------------------------------------------------------------------------------------

/// Opening a session shows its agent's terminal (SPEC.md, "Session view"), and only that one: a pane that attached
/// every listed session, or attached before the selection settled, would hold terminals nobody is looking at. The
/// startup choice is automatic, and its terminal is home, so with nothing else holding focus it takes focus. Specified:
/// with a and b listed and a last visited, the pane opens exactly one stream, for a's agent terminal (no tab), and that
/// terminal holds focus.
#[gpui::test]
fn selecting_a_session_attaches_it_and_only_it(cx: &mut TestAppContext) {
    let mut fleet = open_main(cx, a_and_b());
    assert_eq!(fleet.selected().as_deref(), Some("a"), "the premise");
    assert_eq!(fleet.attached(), ["a"]);
    assert_eq!(fleet.opened.borrow()[0].1.tab, None, "the agent terminal");
    assert!(terminal_focused(fleet.cx));
}

/// The old app remounted its session view on every switch, which tore the old terminal down and attached the new one
/// (old `sidebar.spec.ts`, "switching sessions directly tears the old view down and mounts the new one"). Keeping the
/// old stream would hold the old session's terminal attached from a window that no longer shows it, and reusing its
/// lease would make the new attach a reconnect of the old view rather than a view of its own. Specified: with a open,
/// a click on b's row closes a's stream and opens one stream for b under another lease.
#[gpui::test]
fn switching_sessions_drops_the_old_stream_and_attaches_the_new_one(cx: &mut TestAppContext) {
    let mut fleet = open_main(cx, a_and_b());
    fleet.go_live(0);
    assert_eq!(
        fleet.phase(0),
        FakePhase::Attached,
        "the premise: a is live"
    );

    click(fleet.cx, "session-row-b");
    settle(fleet.cx);
    assert_eq!(fleet.attached(), ["a", "b"]);
    assert_eq!(fleet.phase(0), FakePhase::Closed);
    assert_eq!(fleet.phase(1), FakePhase::Attaching);
    let opened = fleet.opened.borrow();
    assert_ne!(opened[0].1.lease, opened[1].1.lease, "a lease per view");
}

/// The old app once mounted the first session's socket into the second session's view when the user switched before
/// the first terminal was ready: a mount retry for the first session outlived the switch (old `terminal.spec.ts`,
/// "switching sessions before the first terminal is ready mounts the second session's socket"). Here the moment is a
/// switch while the first stream is still attaching, which the test above does not reach because it makes a live
/// first. A stream left open then would keep a's terminal attached and keep feeding a view nobody shows. Specified:
/// with a's stream still attaching (no socket open, nothing replayed), a click on b's row closes a's stream, so nothing
/// a's side sends later has a reader, and opens one stream, for b; once b is live, a typed letter reaches b's program,
/// and a's stream never received anything.
#[gpui::test]
fn switching_while_the_first_stream_attaches_closes_it(cx: &mut TestAppContext) {
    let mut fleet = open_main(cx, a_and_b());
    assert_eq!(
        fleet.phase(0),
        FakePhase::Attaching,
        "the premise: a is still attaching"
    );

    click(fleet.cx, "session-row-b");
    settle(fleet.cx);
    assert_eq!(fleet.phase(0), FakePhase::Closed, "a's stream ended");
    assert_eq!(fleet.attached(), ["a", "b"], "one stream for b");
    assert_eq!(fleet.phase(1), FakePhase::Attaching);

    fleet.go_live(1);
    keys(fleet.cx, "x");
    assert_eq!(fleet.input(1), [letter("x", 88)]);
    assert_eq!(fleet.input(0), [], "a's stream never carried anything");
}

/// The old `sidebar.spec.ts` case "filtering out the active session preserves its terminal and restores its selected
/// row", which held the very terminal element across a filter and back. SPEC.md: "Hiding the selected session changes
/// neither that selection nor its open terminal". Here the terminal's identity is its stream: a pane that rebuilt the
/// terminal when the list stopped drawing its row would open a second stream and replay the screen, and one that let
/// go of it would close the first. `carried_tests.rs` pins that the terminal stays drawn; this pins that it is the same
/// one. Specified: with a open and live, Waiting (which matches neither session, the helm carrying a outside the list)
/// leaves a's row undrawn and a selected; All and the helm's listing of a and b then draw a's row selected again; across
/// both, the pane opened no other stream and a's stream stayed attached.
#[gpui::test]
fn a_filter_that_hides_the_open_session_keeps_its_stream_and_reselects_its_row(
    cx: &mut TestAppContext,
) {
    let mut fleet = open_main(cx, a_and_b());
    fleet.go_live(0);
    answer_everything(&mut fleet.helm);

    click(fleet.cx, "session-status-Waiting");
    let (query, _) = one_query(&mut fleet.helm);
    fleet
        .helm
        .push(change(carrying(filtered(&[], 2, 0), "a"), &[], &["b"]));
    fleet.helm.reply(query.id, 204, None);
    settle(fleet.cx);
    assert!(
        node(&frame(fleet.cx), "session-row-a").is_none(),
        "the premise: Waiting hides a"
    );
    assert_eq!(fleet.selected().as_deref(), Some("a"), "a stays open");

    click(fleet.cx, "session-status-All");
    let (query, _) = one_query(&mut fleet.helm);
    fleet.helm.push(change(
        listing_meta(&["a", "b"], 2, false),
        &[session("a"), session("b")],
        &[],
    ));
    fleet.helm.reply(query.id, 204, None);
    settle(fleet.cx);
    let item = node(&frame(fleet.cx), "session-item-a").map(|item| item.selected);
    assert_eq!(item, Some(Some(true)), "a's row is back, selected");
    assert_eq!(fleet.attached(), ["a"], "no second stream");
    assert_eq!(fleet.phase(0), FakePhase::Attached, "a's stream stayed");
}

// ---- Focus ---------------------------------------------------------------------------------------------------------

/// SPEC.md, "Where focus rests": "Choosing a session, including the one already selected, focuses its terminal", taking
/// focus from whatever holds it. The click rule is set so that a press on a row focuses the row, as on Linux: focus is
/// then on a control, which home alone never takes it from, so only the choice can move it to the terminal.
/// Specified: a click on b's row focuses b's terminal; once live, a typed letter reaches b's program and nothing
/// reaches a's; a click on b's row again, the open session, focuses b's terminal again and opens no new stream.
#[gpui::test]
fn a_click_focuses_the_terminal_and_typed_keys_reach_its_program(cx: &mut TestAppContext) {
    let mut fleet = open_main(cx, a_and_b());
    fleet
        .cx
        .update(|_, cx| focus::set_click_focuses_controls(true, cx));

    click(fleet.cx, "session-row-b");
    settle(fleet.cx);
    assert!(terminal_focused(fleet.cx), "the click focused b's terminal");
    fleet.go_live(1);
    keys(fleet.cx, "x");
    assert_eq!(fleet.input(1), [letter("x", 88)]);
    assert_eq!(fleet.input(0), [], "nothing reached a's program");

    click(fleet.cx, "session-row-b");
    settle(fleet.cx);
    assert!(
        terminal_focused(fleet.cx),
        "choosing the open session focuses it"
    );
    assert_eq!(fleet.attached(), ["a", "b"], "and opens nothing");
}

/// A root view with its own text field for testing that a newly live terminal leaves focus where the user is typing.
struct WithField {
    list: Entity<SessionList>,
    pane: Entity<SessionPane>,
    field: TextInputState,
    root: root::Attachment,
}

impl WithField {
    /// The pane and session list share a root so the test can move between Welcome, the terminal and sidebar controls.
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let root = root::attach(window, cx);
        let list = cx.new(SessionList::new);
        WithField {
            root,
            list,
            pane: cx.new(|cx| SessionPane::new(window, cx)),
            field: TextInputState::new(window, cx),
        }
    }
}

impl Render for WithField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sidebar = div()
            .w(px(300.))
            .h_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .debug_selector(|| "filter".into())
                    .child(TextInput::new(
                        "field",
                        &self.field,
                        "Filter",
                        HoverHelp::tooltip("Filter the sessions"),
                    )),
            )
            .child(self.list.clone());
        let content = div()
            .size_full()
            .flex()
            .flex_row()
            .child(sidebar)
            .child(div().flex_1().h_full().child(self.pane.clone()));
        root::wrap(content, &self.root, window, cx)
    }
}

/// [`open`] with a and b over [`WithField`], and the field's state, to read what was typed into it.
fn open_with_field(cx: &mut TestAppContext) -> (Fleet<'_>, TextInputState) {
    let field = Rc::new(RefCell::new(None));
    let kept = field.clone();
    let fleet = open(cx, a_and_b(), WithField::new, move |view| {
        *kept.borrow_mut() = Some(view.field.clone());
        view.pane.clone()
    });
    let field = field.borrow_mut().take().expect("the view was built");
    (fleet, field)
}

/// SPEC.md, "Cases that settled disputed points": "The user selects a session and, before its terminal finishes
/// loading, types in the session filter. The filter keeps focus when the terminal arrives." The terminal arriving is
/// the moment a later focus move would be tempting, and the user has moved on by then. Specified: after a click on b's
/// row, a click on the field and two letters typed there before b's terminal is live, the terminal going live leaves
/// focus in the field: a third letter lands there too, and b's program gets nothing.
#[gpui::test]
fn a_click_then_typing_elsewhere_before_the_terminal_is_live_keeps_focus_there(
    cx: &mut TestAppContext,
) {
    let (mut fleet, field) = open_with_field(cx);
    click(fleet.cx, "session-row-b");
    settle(fleet.cx);
    assert_eq!(fleet.attached(), ["a", "b"], "the premise: b is attaching");

    click(fleet.cx, "filter");
    keys(fleet.cx, "a b");
    fleet.go_live(1);
    keys(fleet.cx, "c");
    assert!(!terminal_focused(fleet.cx));
    assert_eq!(fleet.input(1), [], "b's program got nothing");
    let text = fleet.cx.update(|_, cx| field.text(cx));
    assert_eq!(text.as_ref(), "abc", "every letter landed in the field");
}

/// SPEC.md, "Where focus rests": the terminal is home, and "The terminal never takes focus from a control that holds
/// it", whatever made the selection move. Only the user's own choice takes focus; a session the selection rules open
/// because the open one went away must not pull focus out of a field the user is typing in. Specified: with the field
/// focused and a open, a complete listing pushed without a opens b and attaches its terminal, and a letter typed then
/// lands in the field, not in b's live program.
#[gpui::test]
fn an_automatic_choice_never_takes_focus_from_a_control(cx: &mut TestAppContext) {
    let (mut fleet, field) = open_with_field(cx);
    click(fleet.cx, "filter");
    assert!(
        !terminal_focused(fleet.cx),
        "the premise: the field has focus"
    );

    fleet.helm.push(CollectionChanges {
        collection: Collection::Sessions,
        meta: meta(&["b"]),
        upsert: Vec::new(),
        remove: vec!["a".into()],
    });
    settle(fleet.cx);
    assert_eq!(fleet.attached(), ["a", "b"], "the premise: b opened");
    fleet.go_live(1);
    keys(fleet.cx, "x");
    assert!(!terminal_focused(fleet.cx));
    assert_eq!(fleet.input(1), []);
    let text = fleet.cx.update(|_, cx| field.text(cx));
    assert_eq!(text.as_ref(), "x");
}

/// SPEC.md, "Cases that settled disputed points": "The user's session is deleted from the command line or by an agent
/// while they type in it. The next session is selected and its terminal takes the rest of their typing (the terminal is
/// home)." Specified: with a's terminal live and typed into, a complete listing pushed without a opens b, closes a's
/// stream and attaches b's, whose terminal holds focus; once live, a typed letter reaches b's program.
#[gpui::test]
fn a_removal_pushed_from_elsewhere_selects_the_next_session_whose_terminal_takes_focus(
    cx: &mut TestAppContext,
) {
    let mut fleet = open_main(cx, a_and_b());
    fleet.go_live(0);
    keys(fleet.cx, "x");
    assert_eq!(
        fleet.input(0),
        [letter("x", 88)],
        "the premise: typing in a"
    );

    fleet.helm.push(CollectionChanges {
        collection: Collection::Sessions,
        meta: meta(&["b"]),
        upsert: Vec::new(),
        remove: vec!["a".into()],
    });
    settle(fleet.cx);
    assert_eq!(fleet.selected().as_deref(), Some("b"));
    assert_eq!(fleet.attached(), ["a", "b"]);
    assert_eq!(fleet.phase(0), FakePhase::Closed);
    assert!(terminal_focused(fleet.cx), "the next terminal is home");
    fleet.go_live(1);
    keys(fleet.cx, "y");
    assert_eq!(fleet.input(1), [letter("y", 89)]);
}

impl Fleet<'_> {
    /// The paths of the Deletes and Replaces the window sent since the last call. Every other command (the open
    /// session's seen mark, the remembered selection) is answered with success at once, as [`Fleet::deletes`] does.
    fn deletes_and_replaces(&mut self) -> Vec<String> {
        let mut found = Vec::new();
        for command in self.helm.commands() {
            if command.method == Method::Delete || command.path.ends_with("/replace") {
                found.push(command.path);
            } else {
                self.helm.reply(command.id, 200, None);
            }
        }
        found
    }
}

/// [`a_delete_question_keeps_the_keyboard_while_the_terminal_goes_live`] and its Replace twin: the header question
/// `button` asks, opened while a's terminal is still catching up, keeps focus when the terminal goes live.
fn a_question_keeps_the_keyboard_while_the_terminal_goes_live(
    cx: &mut TestAppContext,
    button: &'static str,
) {
    let mut fleet = open_main(cx, a_and_b());
    fleet
        .cx
        .update(|_, cx| focus::set_click_focuses_controls(true, cx));
    fleet.deletes_and_replaces();
    assert_eq!(
        fleet.phase(0),
        FakePhase::Attaching,
        "the premise: a is catching up"
    );
    click(fleet.cx, button);
    assert_eq!(
        test_support::focused_id(fleet.cx).as_deref(),
        Some("prompt-confirm"),
        "the premise: the question has focus"
    );

    fleet.go_live(0);
    assert_eq!(
        test_support::focused_id(fleet.cx).as_deref(),
        Some("prompt-confirm"),
        "the terminal going live left focus on the question"
    );
    keys(fleet.cx, "tab");
    assert_eq!(
        test_support::focused_id(fleet.cx).as_deref(),
        Some("prompt-cancel")
    );
    keys(fleet.cx, "enter");
    assert!(
        !fleet.drawn("prompt"),
        "Enter on Cancel closed the question"
    );
    assert_eq!(fleet.input(0), [], "and sent the program no Enter");

    click(fleet.cx, button);
    assert!(fleet.drawn("prompt"), "the premise: asked again");
    keys(fleet.cx, "escape");
    assert!(terminal_focused(fleet.cx), "Escape returns to the terminal");
    keys(fleet.cx, "x");
    assert_eq!(
        fleet.input(0),
        [letter("x", 88)],
        "only the key typed after it"
    );
    assert_eq!(fleet.deletes_and_replaces(), [] as [String; 0]);
}

/// A replay completing is automatic: nothing the user did moved focus then, so it must not move it either (SPEC.md,
/// "Where focus rests": the terminal "never takes focus from a control that holds it"). A question the user opened
/// while the terminal was still catching up owns the keyboard, and an Enter meant for it must not reach the agent
/// (old `terminal-replay-rename.spec.ts`, "replay reveal preserves a header delete question's keyboard ownership").
/// Specified: with a's terminal still attaching, a click on the header's Delete focuses the question's confirm button,
/// which keeps focus when the terminal goes live; Tab moves to Cancel, and Enter there closes the question and sends
/// the program nothing; asked again and dismissed with Escape, focus is on the terminal, a letter typed then is all
/// the program receives, and no Delete was sent.
#[gpui::test]
fn a_delete_question_keeps_the_keyboard_while_the_terminal_goes_live(cx: &mut TestAppContext) {
    a_question_keeps_the_keyboard_while_the_terminal_goes_live(cx, "session-header-delete");
}

/// [`a_delete_question_keeps_the_keyboard_while_the_terminal_goes_live`] for the header's Replace question (old
/// `terminal-replay-rename.spec.ts`, "replay reveal preserves a header replace question's keyboard ownership"), which
/// is asked and answered the same way. Specified: the same steps through the Replace question, and no Replace sent.
#[gpui::test]
fn a_replace_question_keeps_the_keyboard_while_the_terminal_goes_live(cx: &mut TestAppContext) {
    a_question_keeps_the_keyboard_while_the_terminal_goes_live(cx, "session-header-replace");
}

// ---- Welcome as home with no sessions -----------------------------------------------------------------------------

/// A keyboard user can open New from a proven empty fleet. The same action in the pane returns the sidebar to
/// Sessions while Hosts is showing, so the new session will be visible there when it starts.
#[gpui::test]
fn with_no_sessions_enter_opens_the_launcher_from_welcome(cx: &mut TestAppContext) {
    let mut fleet = open_main(cx, vec![listing(&[], &[]), fixture_hosts(&[0])]);
    assert_eq!(fleet.selected(), None, "the premise: nothing is open");
    assert_eq!(
        test_support::focused_id(fleet.cx).as_deref(),
        Some(PANE_START_ID)
    );
    test_support::keys(fleet.cx, "enter");
    assert!(node(&frame(fleet.cx), "launcher-launch").is_some());
    test_support::keys(fleet.cx, "escape");

    click(fleet.cx, "hosts-panel-toggle");
    assert!(node(&frame(fleet.cx), "hosts-panel").is_some());
    click(fleet.cx, PANE_START_ID);
    let shown = frame(fleet.cx);
    assert!(node(&shown, "launcher-launch").is_some());
    assert!(node(&shown, "hosts-panel").is_none());
    assert!(node(&shown, crate::session_list::NEW_ID).is_some());
    assert!(fleet.attached().is_empty());
}

/// The secondary actions follow Start a session in the Tab order. Dialogs return to their own links, so a keyboard
/// user can continue along the row. The guide goes through the platform opener and leaves the pane available.
#[gpui::test]
fn welcome_links_open_their_destinations_and_keep_the_users_place(cx: &mut TestAppContext) {
    let fleet = open_main(cx, vec![listing(&[], &[]), fixture_hosts(&[0])]);
    let window = fleet.cx;
    assert_eq!(
        test_support::focused_id(window).as_deref(),
        Some(PANE_START_ID)
    );

    test_support::keys(window, "tab");
    assert_eq!(
        test_support::focused_id(window).as_deref(),
        Some("welcome-pane-add-host")
    );
    test_support::keys(window, "enter");
    assert!(node(&frame(window), "add-host-cancel").is_some());
    test_support::keys(window, "escape");
    assert_eq!(
        test_support::focused_id(window).as_deref(),
        Some("welcome-pane-add-host")
    );

    test_support::keys(window, "tab");
    assert_eq!(
        test_support::focused_id(window).as_deref(),
        Some("welcome-pane-get-started")
    );
    test_support::keys(window, "enter");
    assert_eq!(
        window.opened_url().as_deref(),
        Some(crate::welcome::GET_STARTED_URL)
    );
    assert_eq!(
        test_support::focused_id(window).as_deref(),
        Some("welcome-pane-get-started")
    );

    test_support::keys(window, "tab");
    assert_eq!(
        test_support::focused_id(window).as_deref(),
        Some("welcome-pane-feedback")
    );
    test_support::keys(window, "enter");
    assert!(node(&frame(window), "feedback-cancel").is_some());
    test_support::keys(window, "escape");
    assert_eq!(
        test_support::focused_id(window).as_deref(),
        Some("welcome-pane-feedback")
    );
}

/// The marks introduce every built-in agent in the launcher's order. Names stay available to a screen reader, even
/// when the row shows only pictures.
#[gpui::test]
fn welcome_marks_name_the_built_in_agents_in_launcher_order(cx: &mut TestAppContext) {
    let fleet = open_main(cx, vec![listing(&[], &[])]);
    let expected = crate::launcher::HARNESS_PICKER_ORDER
        .into_iter()
        .filter(|agent| !agent.is_shell())
        .map(|agent| agent.display_name())
        .collect::<Vec<_>>();
    let nodes = frame(fleet.cx);
    let names = nodes
        .iter()
        .filter(|node| {
            node.element_id
                .as_deref()
                .is_some_and(|id| id.starts_with("welcome-pane-mark-"))
        })
        .map(|node| node.label.as_deref().expect("each mark names its agent"))
        .collect::<Vec<_>>();
    assert_eq!(names, expected);
    assert_eq!(
        node(&nodes, "welcome-pane-marks").and_then(|group| group.label.as_deref()),
        Some(format!("Works with {}", expected.join(", ")).as_str()),
    );
}

/// Removing the last session closes the focused terminal and transfers focus to Start a session. This lets someone
/// who deletes their last session immediately start another.
#[gpui::test]
fn removing_the_last_session_sends_focus_to_welcome(cx: &mut TestAppContext) {
    let mut fleet = open_main(
        cx,
        vec![listing(&["a"], &[session("a")]), preferences(&["a"], None)],
    );
    fleet.go_live(0);
    assert!(
        terminal_focused(fleet.cx),
        "the premise: a's terminal is home"
    );

    fleet.helm.push(CollectionChanges {
        collection: Collection::Sessions,
        meta: meta(&[]),
        upsert: Vec::new(),
        remove: vec!["a".into()],
    });
    settle(fleet.cx);
    assert_eq!(fleet.selected(), None);
    assert_eq!(fleet.phase(0), FakePhase::Closed);
    assert_eq!(
        test_support::focused_id(fleet.cx).as_deref(),
        Some(PANE_START_ID)
    );
}

/// When a session created elsewhere replaces Welcome, its terminal becomes the keyboard's resting place. Focus
/// would otherwise still name the Start button whose pane has gone away.
#[gpui::test]
fn the_first_session_pushed_into_an_empty_fleet_can_take_focus(cx: &mut TestAppContext) {
    let mut fleet = open_main(cx, vec![listing(&[], &[]), fixture_hosts(&[0])]);
    assert_eq!(
        test_support::focused_id(fleet.cx).as_deref(),
        Some(PANE_START_ID)
    );

    fleet.push_b();
    assert_eq!(fleet.selected().as_deref(), Some("b"));
    assert!(node(&frame(fleet.cx), PANE_START_ID).is_none());
    assert!(terminal_focused(fleet.cx));
}

/// The fleet can become empty while nothing is open and nothing else the pane watches moves: a listing that failed
/// (the helm could not read it, so it claimed nothing and opened nothing) comes back complete and empty. Only the card
/// follower's empty-fleet flag tells the pane then, so this is the test that keeps it. (The helm cannot deliver a first
/// listing after the snapshot: a push to a collection no snapshot installed is a protocol violation, and the reset
/// that follows syncs the pane anyway.) Its opening check also keeps the rule that Start a session is home only on proof that the
/// fleet is empty; the unread listing's case of that rule is pinned on the shared proof itself
/// (`session_list::tests::only_a_complete_available_empty_listing_is_an_empty_fleet`). Specified: a window whose
/// snapshot holds a failed session listing with no rows opens nothing and does not focus Start a session; once an available,
/// complete, empty listing is pushed, Start a session holds focus and still nothing is open.
#[gpui::test]
fn a_listing_that_comes_back_empty_sends_focus_to_welcome(cx: &mut TestAppContext) {
    let mut failed = listing(&[], &[]);
    failed.meta.availability = Availability::Unavailable {
        error: "down".into(),
    };
    let mut fleet = open_main(cx, vec![failed, preferences(&[], None)]);
    assert_eq!(fleet.selected(), None, "the premise: nothing is open");
    assert_ne!(
        test_support::focused_id(fleet.cx).as_deref(),
        Some(PANE_START_ID),
        "the premise: a failed listing does not make Start a session home"
    );

    fleet
        .helm
        .push(change(listing_meta(&[], 0, false), &[], &[]));
    settle(fleet.cx);
    assert_eq!(
        test_support::focused_id(fleet.cx).as_deref(),
        Some(PANE_START_ID)
    );
    assert_eq!(fleet.selected(), None);
    assert!(fleet.attached().is_empty());
}

/// Focus on an undrawn handle takes no keys at all. The root takes focus while Start a session is disabled so Tab,
/// Escape and the shortcuts still work. A host probe exercises the availability rule it shares with New, including
/// the return to Start a session once the probe finishes.
#[gpui::test]
fn while_start_is_disabled_focus_rests_on_the_root_and_returns_to_welcome(cx: &mut TestAppContext) {
    let mut fleet = open_main(cx, vec![listing(&[], &[]), preferences(&[], None)]);
    assert_eq!(
        test_support::focused_id(fleet.cx).as_deref(),
        Some(PANE_START_ID),
        "the premise: Start a session is home"
    );

    let probe = fleet.probe_in_flight();
    let start = node(&frame(fleet.cx), PANE_START_ID)
        .cloned()
        .expect("Start a session is drawn");
    assert!(
        !start.flags.focusable,
        "the premise: Start a session is disabled"
    );
    assert_eq!(
        test_support::focused_id(fleet.cx),
        None,
        "focus rests on the root, which no control is"
    );
    fleet.helm.reply(probe, 200, Some(serde_json::json!({})));
    settle(fleet.cx);
    assert_eq!(
        test_support::focused_id(fleet.cx).as_deref(),
        Some(PANE_START_ID),
        "Start a session drawn enabled again takes focus back"
    );
}

/// The opening both empty-fleet launch tests share: the main window over an empty fleet, with Start a session home; the launcher
/// opened on a complete Command draft (sending nothing); Launch pressed, which sends one create. Returns the fleet and
/// that create's envelope id, to answer it by.
fn launched_over_an_empty_fleet(cx: &mut TestAppContext) -> (Fleet<'_>, u64) {
    let mut fleet = open_main(
        cx,
        vec![
            listing(&[], &[]),
            fixture_hosts(&[0]),
            preferences(&[], None),
        ],
    );
    assert_eq!(
        test_support::focused_id(fleet.cx).as_deref(),
        Some(PANE_START_ID),
        "the premise: Start a session is home over an empty fleet"
    );
    fleet.cx.update(|window, cx| {
        let mark = focus::Mark::take(cx);
        assert!(
            crate::launcher::view::open(command_draft(), &mark, window, cx).is_some(),
            "the premise: the launcher opened"
        );
    });
    settle(fleet.cx);
    let none = serde_json::json!([]);
    assert!(
        creates_answering(&mut fleet.helm, fleet.cx, &none).is_empty(),
        "the premise: opening sends no create"
    );
    click(fleet.cx, "launcher-launch");
    let [create] = &creates_answering(&mut fleet.helm, fleet.cx, &none)[..] else {
        panic!("Launch sent one create");
    };
    let create = create.id;
    (fleet, create)
}

impl Fleet<'_> {
    /// The helm pushes the listing that holds the launched session b, as it does before it answers the create.
    fn push_b(&mut self) {
        let state = listing(&["b"], &[session("b")]);
        self.helm.push(CollectionChanges {
            collection: state.collection,
            meta: state.meta,
            upsert: state.entries,
            remove: Vec::new(),
        });
        settle(self.cx);
    }
}

/// A first launch over an empty fleet ends with focus on the new terminal. The helm pushes the new session's listing
/// before replying to the create, and the list holds its automatic pick until the reply. Returning to Start a session
/// during either step could strand focus there when the terminal opens.
#[gpui::test]
fn a_first_launch_over_an_empty_fleet_ends_on_the_new_sessions_terminal(cx: &mut TestAppContext) {
    let (mut fleet, create) = launched_over_an_empty_fleet(cx);
    let start_focused = |fleet: &mut Fleet<'_>| {
        test_support::focused_id(fleet.cx).as_deref() == Some(PANE_START_ID)
    };
    assert!(
        !start_focused(&mut fleet),
        "after Launch, Start a session does not hold focus"
    );

    fleet.push_b();
    assert!(
        !start_focused(&mut fleet),
        "after the listing, Start a session does not hold focus"
    );
    fleet
        .helm
        .reply(create, 200, Some(session("b").entry().value));
    settle(fleet.cx);
    assert_eq!(fleet.selected().as_deref(), Some("b"));
    assert!(
        terminal_focused(fleet.cx),
        "the new session's terminal holds focus"
    );
    assert!(
        !start_focused(&mut fleet),
        "after the reply, Start a session does not hold focus"
    );
}

/// SPEC.md, "Creation": when a launch does not open its session because the user moved on, "the selected session and
/// its remembered selection stay as they were, and focus, panels, menus and dialogs stay as the user left them", and
/// the Started report shows "including when a filter or Hosts hides the new row". Over an empty fleet "as they were"
/// is nothing selected, which the list's own rule (pick a session whenever rows exist and nothing is open) would break
/// as soon as the helm pushes the new row, before the launch's reply can check whether the user moved on. The old app
/// guarded exactly this race (its `nonblocking-launch.spec.ts` "New from an empty visible fleet preserves no selection
/// after opening Hosts"). Specified: over an empty fleet, Launch sends one create and Hosts is pressed; the listing
/// holding b selects nothing; the create's reply naming b still selects nothing, Hosts still shows and the "Started"
/// report names b.
#[gpui::test]
fn a_first_launch_the_user_moved_on_from_selects_nothing(cx: &mut TestAppContext) {
    let (mut fleet, create) = launched_over_an_empty_fleet(cx);
    click(fleet.cx, "hosts-panel-toggle");
    let hosts_shown = |fleet: &mut Fleet<'_>| node(&frame(fleet.cx), "hosts-panel").is_some();
    assert!(hosts_shown(&mut fleet), "the premise: Hosts shows");

    fleet.push_b();
    assert_eq!(fleet.selected(), None, "the pushed row opened nothing");

    fleet
        .helm
        .reply(create, 200, Some(session("b").entry().value));
    settle(fleet.cx);
    assert_eq!(fleet.selected(), None, "the reply opened nothing");
    assert!(hosts_shown(&mut fleet), "Hosts still shows");
    let report = node(&frame(fleet.cx), "launch-started-report")
        .and_then(|node| node.label.as_ref().map(ToString::to_string));
    assert_eq!(report.as_deref(), Some("Started “b”"));
}

impl Fleet<'_> {
    /// Start Add host's probe, which makes New unavailable, and return its envelope id, to answer it by. Every other
    /// command sent meanwhile is answered with success, as [`Fleet::deletes`] does.
    fn probe_in_flight(&mut self) -> u64 {
        let path = start_host_change(self.cx, HostChange::Probe);
        let mut probe = None;
        for command in self.helm.commands() {
            if command.path == path {
                probe = Some(command.id);
            } else {
                self.helm.reply(command.id, 200, None);
            }
        }
        probe.expect("the premise: the probe is in flight")
    }
}

// ---- The Delete shortcut ------------------------------------------------------------------------------------------

/// SPEC.md, "Desktop window chrome": the session shortcuts "work from terminal focus without sending their chord to the
/// program", and Delete keeps its ordinary confirmation there. The terminal is where focus usually is, so this is the
/// shortcut's common case; the key table names the chord an app shortcut, which is what lets the binding take it before
/// the terminal does. Specified: with a's terminal live and focused, the chord asks the Delete question and the program
/// receives nothing; Enter confirms, which sends a's Delete, and focus goes back to a's terminal.
#[gpui::test]
fn the_delete_shortcut_from_a_focused_terminal_asks_and_never_reaches_the_program(
    cx: &mut TestAppContext,
) {
    let mut fleet = open_main(cx, a_and_b());
    fleet.go_live(0);
    assert!(
        terminal_focused(fleet.cx),
        "the premise: a's terminal has focus"
    );
    fleet.helm.commands();

    keys(fleet.cx, DELETE_SESSION_CHORD);
    assert!(
        fleet.cx.debug_bounds("prompt").is_some(),
        "the question opened"
    );
    assert_eq!(fleet.input(0), [], "the chord never reached the program");

    let deletes = |helm: &mut farhelm_helm_client::fake::FakeHelm| {
        helm.commands()
            .into_iter()
            .filter(|command| command.method == Method::Delete)
            .map(|command| command.path)
            .collect::<Vec<_>>()
    };
    assert!(
        deletes(&mut fleet.helm).is_empty(),
        "the question waits for consent"
    );
    keys(fleet.cx, "enter");
    assert_eq!(deletes(&mut fleet.helm), ["/api/sessions/a"]);
    assert!(
        terminal_focused(fleet.cx),
        "closing the question gave focus back"
    );
}

/// The old `session-action-shortcuts.spec.ts` case "browser Delete chord honors skipped confirmations and deletes the
/// open session". The chord is the header's Delete "with its ordinary confirmation" (SPEC.md, "Desktop window chrome"),
/// and Skip session confirmations is part of that: a chord that still asked would make the setting useless from the
/// terminal, where the user's hands are. Specified: with Skip on and a's terminal live and focused, the chord sends
/// exactly one Delete, of a, opens no question and reaches the program with nothing; a's row stays drawn while the
/// Delete is unanswered; once the helm removes a and answers, b is open.
#[gpui::test]
fn the_delete_shortcut_with_questions_skipped_deletes_the_open_session_at_once(
    cx: &mut TestAppContext,
) {
    let skipping = farhelm_proto::ui_channel::Preferences {
        visit_history: vec!["a".to_owned()],
        skip_session_confirmations: Some(true),
        welcome_seen: Some(true),
        ..Default::default()
    };
    let mut fleet = open_main(
        cx,
        vec![
            listing(&["a", "b"], &[session("a"), session("b")]),
            preferences_state(skipping),
        ],
    );
    fleet.go_live(0);
    assert!(
        terminal_focused(fleet.cx),
        "the premise: a's terminal has focus"
    );
    fleet.deletes();

    keys(fleet.cx, DELETE_SESSION_CHORD);
    // As `Fleet::deletes`, but keeping each Delete's path: every other command is answered at once, in order.
    let mut deletes = Vec::new();
    for command in fleet.helm.commands() {
        if command.method == Method::Delete {
            deletes.push(command);
        } else {
            fleet.helm.reply(command.id, 200, None);
        }
    }
    let [delete] = &deletes[..] else {
        panic!("one Delete was sent, not {}", deletes.len());
    };
    assert_eq!(delete.path.split('?').next(), Some("/api/sessions/a"));
    assert!(fleet.cx.debug_bounds("prompt").is_none(), "nothing asked");
    assert_eq!(fleet.input(0), [], "the chord never reached the program");
    assert!(
        node(&frame(fleet.cx), "session-row-a").is_some(),
        "a's row stays while the Delete runs"
    );

    fleet
        .helm
        .push(change(listing_meta(&["b"], 1, false), &[], &["a"]));
    fleet.helm.reply(delete.id, 200, None);
    settle(fleet.cx);
    assert_eq!(fleet.selected().as_deref(), Some("b"));
}

/// The exception to the case above: a session with an open terminal tab asks even with Skip session confirmations on
/// (SPEC.md, "Lifecycle operations"). On a Mac the Delete chord, ⌘W, is the chord terminal emulators close a tab with,
/// so it is the one most easily pressed by accident. Specified: with Skip on (asserted as the premise), a's agent
/// terminal live and focused and a listing one shell tab, the chord opens the Delete question, sends no Delete and
/// reaches the program with nothing; with focus on the question's Confirm, Enter then sends a's Delete.
#[gpui::test]
fn the_delete_shortcut_asks_with_questions_skipped_while_tabs_are_open(cx: &mut TestAppContext) {
    let skipping = farhelm_proto::ui_channel::Preferences {
        visit_history: vec!["a".to_owned()],
        skip_session_confirmations: Some(true),
        welcome_seen: Some(true),
        ..Default::default()
    };
    let mut fleet = open_main(
        cx,
        vec![
            listing(&["a", "b"], &[session("a").tabs(1), session("b")]),
            preferences_state(skipping),
        ],
    );
    let agent = fleet
        .opened
        .borrow()
        .iter()
        .position(|(_, attach)| attach.session == "a" && attach.tab.is_none())
        .expect("the premise: a's agent terminal is attached");
    fleet.go_live(agent);
    assert!(
        fleet.cx.update(|_, cx| {
            let store = crate::session::current_store(cx).unwrap();
            crate::session_list::menu::skips_questions(store.read(cx).collections())
        }),
        "the premise: Skip session confirmations is on"
    );
    assert!(
        terminal_focused(fleet.cx),
        "the premise: a's agent terminal has focus"
    );
    fleet.deletes();

    keys(fleet.cx, DELETE_SESSION_CHORD);
    assert!(
        fleet.cx.debug_bounds("prompt").is_some(),
        "the question opened despite the preference"
    );
    assert!(
        fleet.deletes().is_empty(),
        "nothing was sent before the answer"
    );
    assert_eq!(
        fleet.input(agent),
        [],
        "the chord never reached the program"
    );
    assert_eq!(
        test_support::focused_id(fleet.cx).as_deref(),
        Some("prompt-confirm"),
        "the premise: the question's focus starts on Confirm"
    );
    keys(fleet.cx, "enter");
    assert_eq!(fleet.deletes_and_replaces(), ["/api/sessions/a"]);
}

/// The modal half of the old `session-action-shortcuts.spec.ts` case "browser Delete chord yields to a visible modal
/// and an ordinary text field" (the text field half is `actions_tests.rs`'s): SPEC.md, "A visible modal keeps keyboard
/// ownership". A chord that reached the header from under a dialog would ask, or with questions skipped delete, behind
/// the dialog the user is working in. The session list's tests pin an open menu; this pins a dialog over a live
/// terminal. Specified, under the Linux click rule: with a's terminal live, a click on the Settings gear opens
/// Settings with focus on its first switch; the chord then leaves Settings open with focus on that switch, opens no
/// question, sends no Delete and reaches the program with nothing.
#[gpui::test]
fn the_delete_shortcut_does_nothing_under_an_open_dialog(cx: &mut TestAppContext) {
    let mut fleet = open_main(cx, a_and_b());
    fleet
        .cx
        .update(|_, cx| focus::set_click_focuses_controls(true, cx));
    fleet.go_live(0);
    fleet.deletes();
    click(fleet.cx, "app-settings-toggle");
    let first_switch = "settings-skip-session-confirmations";
    assert_eq!(
        test_support::focused_id(fleet.cx).as_deref(),
        Some(first_switch),
        "the premise: Settings is open with focus on its first switch"
    );

    keys(fleet.cx, DELETE_SESSION_CHORD);
    assert!(test_support::any_open(fleet.cx), "Settings is still open");
    assert_eq!(
        test_support::focused_id(fleet.cx).as_deref(),
        Some(first_switch)
    );
    assert!(fleet.cx.debug_bounds("prompt").is_none(), "nothing asked");
    assert!(fleet.deletes().is_empty(), "no Delete was sent");
    assert_eq!(fleet.input(0), [], "the chord never reached the program");
}

// ---- A Delete of the open session ---------------------------------------------------------------------------------

/// What the reconnect card says once a Delete's detach has dropped the stream: the detach the supervisor sends when it
/// deletes a session carries `other`, which the stream answers by reconnecting.
const RECONNECTING: &str = "Session unavailable — reconnecting";

/// The reconnect card's and the red status band's selectors, which the tests tell apart: a drop the stream recovers
/// from shows the card, and must never show the band.
const CARD: &str = "terminal-reconnect-card";
const BAND: &str = "terminal-status-banner";

/// What the row and the scrim say while a Delete that stops the running agent is in flight.
const STOPPING: &str = "Stopping agent…";

/// The state message of a program that a signal ended: what the agent's terminal sends when a Delete kills it.
fn ended_by_signal(signal: i32) -> GridOutput {
    serde_json::from_value(
        serde_json::json!({"t":"state","mouse":"off","focus":false,"alt_screen":false,
        "default_fg":null,"default_bg":null,"history_trimmed":false,"exit":{"signal":signal}}),
    )
    .unwrap()
}

impl Fleet<'_> {
    /// The ids of the Deletes the window sent since the last call. Every other command (the open session's seen mark,
    /// the remembered selection) is answered with success at once, because the fake answers in order and a test
    /// answers its Delete after them.
    fn deletes(&mut self) -> Vec<u64> {
        let mut found = Vec::new();
        for command in self.helm.commands() {
            if command.method == Method::Delete {
                found.push(command.id);
            } else {
                self.helm.reply(command.id, 200, None);
            }
        }
        found
    }

    /// With a open and live, delete it with the shortcut and its confirmation, and return the Delete's id, unanswered.
    fn delete_open_session(&mut self) -> u64 {
        self.deletes();
        keys(self.cx, DELETE_SESSION_CHORD);
        keys(self.cx, "enter");
        let [id] = self.deletes()[..] else {
            panic!("one Delete was sent");
        };
        id
    }

    /// The `index`th stream sees what a Delete does to its session, or anything else that kills the agent and detaches
    /// the viewer: the agent is ended by SIGTERM, then the viewer is detached with `other`, which drops the stream into
    /// a reconnect.
    fn delete_happens_to(&mut self, index: usize) {
        {
            let mut opened = self.opened.borrow_mut();
            let terminal = &mut opened[index].0;
            terminal.output(ended_by_signal(15));
            terminal.detach(Some(DetachCode::Other), "session deleted");
        }
        settle(self.cx);
    }

    /// How far the pane's terminal lifts its shown screen now; `None` before a screen is shown and measured.
    fn lift(&mut self) -> Option<f32> {
        let pane = self.pane.clone();
        self.cx.update(|_, cx| {
            let terminal = pane.update(cx, |pane, cx| {
                pane.mounted
                    .current(cx)
                    .as_ref()
                    .and_then(|mounted| mounted.set.as_ref())
                    .map(|set| set.read(cx).agent().clone())
            });
            terminal.and_then(|terminal| terminal.read(cx).shown_lift(cx))
        })
    }

    /// The pane's agent terminal, under the current UI session; `None` while the pane shows none.
    fn agent(&mut self) -> Option<Entity<AttachedTerminal>> {
        let pane = self.pane.clone();
        self.cx.update(|_, cx| {
            pane.update(cx, |pane, cx| {
                pane.mounted
                    .current(cx)
                    .as_ref()
                    .and_then(|mounted| mounted.set.as_ref())
                    .map(|set| set.read(cx).agent().clone())
            })
        })
    }

    /// Whether the pane's terminal's stream is coming back from a drop, held or not: what its reconnect card says,
    /// whether or not the card is drawn.
    fn reconnecting(&mut self) -> bool {
        let terminal = self.agent();
        self.cx
            .update(|_, cx| terminal.is_some_and(|terminal| terminal.read(cx).reconnecting()))
    }

    /// The pane's terminal's own status line, held or not: the stream's state as the terminal knows it.
    fn status_words(&mut self) -> Option<String> {
        let pane = self.pane.clone();
        self.cx.update(|_, cx| {
            let terminal = pane.update(cx, |pane, cx| {
                pane.mounted
                    .current(cx)
                    .as_ref()
                    .and_then(|mounted| mounted.set.as_ref())
                    .map(|set| set.read(cx).agent().clone())
            });
            terminal
                .and_then(|terminal| terminal.read(cx).status_line())
                .map(|line| line.words.to_string())
        })
    }

    /// How many times a fresh frame painted `text`.
    fn painted_count(&mut self, text: &str) -> usize {
        painted(&frame(self.cx))
            .iter()
            .filter(|painted| *painted == text)
            .count()
    }

    /// Whether the last frame drew the element under `selector`.
    fn drawn(&mut self, selector: &'static str) -> bool {
        self.cx.debug_bounds(selector).is_some()
    }
}

/// SPEC.md, "Lifecycle operations": from the moment a Delete is committed until the helm answers, "a scrim over its
/// terminal says it again", as an expected wait. Without it a user watching the terminal sees nothing happen for the
/// seconds an agent takes to stop. The scrim must not take the terminal's keys or presses: the old overlay took no
/// pointer events, and focus belongs where the user left it. Specified: once a's Delete is confirmed, the scrim is
/// drawn and "Stopping agent…" is painted three times (the row, the header in place of its actions, and the scrim); a's
/// terminal holds focus and a typed letter reaches its program; after focus moves to b's menu button, a click on the
/// scrim focuses a's terminal; once the helm refuses, the scrim is gone.
#[gpui::test]
fn a_pending_delete_puts_a_scrim_over_the_terminal_until_the_helm_answers(cx: &mut TestAppContext) {
    let mut fleet = open_main(cx, a_and_b());
    fleet.go_live(0);
    assert!(!fleet.drawn("terminal-delete-scrim"), "the premise");

    let id = fleet.delete_open_session();
    assert!(fleet.drawn("terminal-delete-scrim"));
    assert_eq!(
        fleet.painted_count(STOPPING),
        3,
        "the row, the header and the scrim"
    );
    assert!(terminal_focused(fleet.cx));
    keys(fleet.cx, "x");
    assert_eq!(fleet.input(0), [letter("x", 88)], "keys still reach a");

    click(fleet.cx, "session-menu-b");
    keys(fleet.cx, "escape");
    assert!(
        !terminal_focused(fleet.cx),
        "the premise: focus is on b's menu button"
    );
    click(fleet.cx, "terminal-delete-scrim");
    assert!(
        terminal_focused(fleet.cx),
        "the press went through the scrim"
    );

    fleet
        .helm
        .reply(id, 409, Some(serde_json::json!("the agent is busy")));
    settle(fleet.cx);
    assert!(!fleet.drawn("terminal-delete-scrim"));
    assert_eq!(fleet.painted_count(STOPPING), 0);
}

/// Delete asks about the shell, so its progress must keep naming the shell after the user confirms. The row,
/// the header in place of its actions, and the scrim over the live Bash terminal each show "Stopping shell…".
#[gpui::test]
fn a_pending_shell_delete_names_the_shell_on_all_three_surfaces(cx: &mut TestAppContext) {
    let bash = session("a").launch(serde_json::json!({
        "kind": "agent",
        "selection": {"harness": "bash", "permissions": null},
        "start": ["bash", "{farhelm_args}"],
        "resume": null,
    }));
    let mut fleet = open_main(
        cx,
        vec![listing(&["a"], &[bash]), preferences(&["a"], None)],
    );
    fleet.go_live(0);
    let shell = fleet.agent().expect("the shell terminal is mounted");
    assert!(
        fleet.cx.update(|_, cx| shell.read(cx).is_live()),
        "the premise: the shell has finished replaying"
    );
    assert!(!fleet.drawn("terminal-delete-scrim"), "the premise");

    fleet.delete_open_session();
    assert!(fleet.drawn("terminal-delete-scrim"));
    assert_eq!(fleet.painted_count("Stopping shell…"), 3);
    assert_eq!(fleet.painted_count(STOPPING), 0);
}

/// SPEC.md: "The terminal's own 'Detached' notice, which the delete itself causes, is held back meanwhile". The Delete
/// kills the agent and detaches the viewer, and both would otherwise show as failures (an exit line, the "Session
/// unavailable" reconnect card) under a Delete that is going exactly as asked. Specified: with a's Delete in flight,
/// the agent ended by signal 15 and the viewer detached with `other`, the terminal's stream is reconnecting (the
/// premise), yet no exit line and no card are drawn, the card's words are not painted, and the screen is not lifted to
/// make room for either; the scrim still shows.
#[gpui::test]
fn the_terminal_holds_back_what_the_delete_does_to_it(cx: &mut TestAppContext) {
    let mut fleet = open_main(cx, a_and_b());
    fleet.go_live(0);
    fleet.delete_open_session();

    fleet.delete_happens_to(0);
    assert!(fleet.reconnecting(), "the premise");
    assert!(!fleet.drawn(CARD));
    assert_eq!(fleet.painted_count(RECONNECTING), 0);
    assert!(!fleet.drawn("terminal-exit"));
    assert_eq!(fleet.lift(), Some(0.), "no room taken for hidden lines");
    assert!(fleet.drawn("terminal-delete-scrim"));
}

/// SPEC.md: "if the delete fails, the row and header come back with the refusal shown, and a held notice appears". A
/// Delete can fail after it has killed the agent, and then what it did to the terminal is real and stays: the user
/// needs to see that the agent is gone and the terminal is not live. Specified: with a's Delete in flight and the
/// agent killed and detached as above, a refusal takes the scrim away and the held exit line appears in the frame the
/// app draws on its own (checked before any frame the test forces, so a terminal that missed the release would fail),
/// with the screen lifted for it; the row and the header show the helm's words, and the reconnect card shows "Session
/// unavailable — reconnecting", with no red band.
#[gpui::test]
fn a_refused_delete_shows_the_notices_it_held(cx: &mut TestAppContext) {
    let mut fleet = open_main(cx, a_and_b());
    fleet.go_live(0);
    let id = fleet.delete_open_session();
    fleet.delete_happens_to(0);
    assert!(!fleet.drawn("terminal-exit"), "the premise: held");

    fleet.helm.reply(
        id,
        409,
        Some(serde_json::json!("the session's tabs did not close")),
    );
    settle(fleet.cx);
    assert!(!fleet.drawn("terminal-delete-scrim"));
    assert!(fleet.drawn("terminal-exit"));
    assert!(
        fleet.lift().is_some_and(|lift| lift > 0.),
        "the exit line takes its row"
    );
    assert_eq!(
        fleet.painted_count("the session's tabs did not close"),
        2,
        "the row and the header"
    );
    assert_eq!(fleet.painted_count(RECONNECTING), 1);
    assert!(fleet.drawn(CARD));
    assert!(!fleet.drawn(BAND));
}

/// A Delete that succeeds removes its session, and the selection rules open the next one once it has settled
/// (SPEC.md, "Session list"). Nothing of the deleted session's hold may reach the next terminal: its own exit and
/// detach are real news. Specified: after a's Delete, a's death and detach, a pushed listing without a and the
/// helm's success, b is open and its stream attached; once live, no scrim is drawn, and b's own agent ending and its
/// viewer detaching show the exit line and the reconnect card ("Session unavailable — reconnecting"), with no red band.
#[gpui::test]
fn after_a_successful_delete_the_next_terminal_shows_normally(cx: &mut TestAppContext) {
    let mut fleet = open_main(cx, a_and_b());
    fleet.go_live(0);
    let id = fleet.delete_open_session();
    fleet.delete_happens_to(0);
    fleet.helm.push(CollectionChanges {
        collection: Collection::Sessions,
        meta: meta(&["b"]),
        upsert: Vec::new(),
        remove: vec!["a".into()],
    });
    settle(fleet.cx);
    fleet.helm.reply(id, 200, None);
    settle(fleet.cx);
    assert_eq!(fleet.selected().as_deref(), Some("b"));
    assert_eq!(fleet.attached(), ["a", "b"]);
    assert_eq!(fleet.phase(0), FakePhase::Closed);

    fleet.go_live(1);
    assert!(!fleet.drawn("terminal-delete-scrim"));
    assert!(terminal_focused(fleet.cx));
    fleet.delete_happens_to(1);
    assert_eq!(fleet.painted_count(RECONNECTING), 1);
    assert!(fleet.drawn(CARD));
    assert!(!fleet.drawn(BAND));
    assert!(fleet.drawn("terminal-exit"));
}

/// Uploads leave the terminal when an acknowledged Delete takes away the last open session, even under a
/// filter where omission cannot prove the session gone. The window still has to tell the user which files
/// were interrupted and what is known about each outcome. A reconnect keeps both banners until Dismiss.
#[gpui::test]
fn deleting_the_only_filtered_session_shows_interrupted_uploads_without_a_terminal(
    cx: &mut TestAppContext,
) {
    let (mut helm, connection) = started(cx);
    let opened = serve_terminals(&connection, cx);
    let (mut transfers, served) = FakeTransfers::new();
    cx.update(|cx| connection.update(cx, |connection, _| connection.serve_transfers(served)));

    let mut sessions = listing(&["a"], &[session("a").title("Build artifacts")]);
    sessions.meta = filtered(&["a"], 2, 1);
    helm.connect(vec![sessions, preferences(&["a"], None)]);
    cx.run_until_parked();
    let (main, cx) = test_support::open_view(cx, MainView::new);
    let pane = cx.update(|_, cx| main.read(cx).session_pane().clone());
    let mut fleet = Fleet {
        cx,
        helm,
        pane,
        opened,
    };
    fleet.go_live(0);
    assert_eq!(fleet.selected().as_deref(), Some("a"));
    assert!(
        terminal_focused(fleet.cx),
        "Paste goes to the live terminal"
    );
    let files = [
        "Cargo.toml",
        "src/lib.rs",
        "src/bootstrap.rs",
        "src/main_view.rs",
        "src/session.rs",
    ]
    .map(|file| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(file));
    fleet.cx.write_to_clipboard(ClipboardItem {
        entries: vec![ClipboardEntry::ExternalPaths(ExternalPaths(
            files.to_vec().into(),
        ))],
    });
    fleet.cx.dispatch_action(crate::actions::Paste);
    settle(fleet.cx);
    let started_uploads = transfers.take();
    let [
        FakeTransfer::Upload {
            session,
            filename,
            answer,
            ..
        },
    ] = started_uploads.as_slice()
    else {
        panic!("expected only the first upload to have been sent, got {started_uploads:?}");
    };
    assert_eq!(session, "a");
    assert_eq!(filename, "Cargo.toml");
    assert!(
        !answer.abandoned(),
        "the first upload is still waiting for its reply"
    );
    assert_eq!(fleet.painted_count("Attaching 5 files…"), 1);

    let id = fleet.delete_open_session();
    fleet.helm.push(change(filtered(&[], 1, 0), &[], &["a"]));
    settle(fleet.cx);
    fleet.cx.update(|_, cx| {
        let store = crate::session::current_store(cx).expect("the helm remains connected");
        assert!(
            !crate::session::listing_proves_gone(store.read(cx).collections(), "a"),
            "the remaining listing cannot prove disappearance"
        );
    });
    fleet.helm.reply(id, 200, None);
    settle(fleet.cx);
    assert_eq!(fleet.selected(), None);
    assert!(
        fleet.agent().is_none(),
        "the pane has no terminal to take the lines"
    );
    assert!(answer.abandoned(), "the request went with its terminal");
    assert!(
        transfers.take().is_empty(),
        "the waiting uploads never went out"
    );

    let posted = fleet.cx.update(|_, cx| notices::posted(cx));
    let [
        Notice::InterruptedUpload(sent),
        Notice::InterruptedUpload(waiting),
    ] = posted.as_slice()
    else {
        panic!("expected two interrupted-upload banners, got {posted:?}")
    };
    assert_eq!(
        sent.sentence().accessible_name().as_ref(),
        "Attaching Cargo.toml to Build artifacts was interrupted — whether it was attached cannot be told."
    );
    assert_eq!(
        waiting.sentence().accessible_name().as_ref(),
        "Attaching lib.rs, bootstrap.rs, main_view.rs and 1 more file to Build artifacts was interrupted before they were sent, so nothing was uploaded."
    );

    let sent_dismiss = format!("notice-dismiss-interrupted-upload-{}", sent.key());
    let waiting_dismiss = format!("notice-dismiss-interrupted-upload-{}", waiting.key());
    assert_eq!(
        test_support::label(fleet.cx, &sent_dismiss).as_deref(),
        Some("Dismiss")
    );
    assert_eq!(
        test_support::label(fleet.cx, &waiting_dismiss).as_deref(),
        Some("Dismiss")
    );
    fleet.helm.break_channel();
    settle(fleet.cx);
    let mut empty = listing(&[], &[]);
    empty.meta = filtered(&[], 1, 0);
    fleet.helm.connect(vec![empty, preferences(&[], None)]);
    settle(fleet.cx);
    let dismiss = |cx: &mut VisualTestContext, id: &str| {
        test_support::show_next_frame(cx);
        let button = frame(cx)
            .into_iter()
            .find(|node| node.element_id.as_deref() == Some(id))
            .expect("the upload banner still shows its Dismiss button after the reset");
        cx.simulate_click(button.bounds.center(), gpui::Modifiers::none());
        settle(cx);
    };
    dismiss(fleet.cx, &sent_dismiss);
    assert_eq!(
        fleet.cx.update(|_, cx| notices::posted(cx)),
        [posted[1].clone(), Notice::ConnectionLost],
        "Dismiss withdraws only the banner clicked after the reconnect"
    );
    dismiss(fleet.cx, &waiting_dismiss);
    assert_eq!(
        fleet.cx.update(|_, cx| notices::posted(cx)),
        [Notice::ConnectionLost]
    );
}

/// Undismissed uploads can outnumber the window's available lines. A second interruption of the same file must
/// scroll into view on its own without moving focus. The pane keeps its working area and older notices stay reachable.
/// Reposting the same connection warning moves its single banner to the bottom and reveals it again, including at
/// reconnect when the region is rebuilt.
#[gpui::test]
fn a_full_notice_stack_reveals_the_latest_upload_without_hiding_the_pane(cx: &mut TestAppContext) {
    let mut fleet = open_main(cx, a_and_b());
    fleet.go_live(0);
    let pane_before = test_support::bounds(fleet.cx, "main-pane");
    assert!(terminal_focused(fleet.cx));
    let uploads: Vec<_> = (1..=48)
        .map(|index| {
            Rc::new(notices::InterruptedUploads::new(
                Some(crate::host_text::HostText::from_host("Build artifacts")),
                vec![crate::host_text::HostText::from_host(format!(
                    "report-{}.zip",
                    index.min(47)
                ))],
                true,
            ))
        })
        .collect();
    let (latest, earlier) = uploads.split_last().unwrap();
    let previous = earlier.last().unwrap();
    assert_eq!(
        latest.sentence().accessible_name(),
        previous.sentence().accessible_name(),
        "the two occurrences have identical text"
    );
    fleet.cx.update(|_, cx| {
        notices::post(Notice::ConnectionLost, cx);
        for upload in earlier {
            notices::post(Notice::InterruptedUpload(upload.clone()), cx);
        }
    });
    settle(fleet.cx);
    let viewport = test_support::bounds(fleet.cx, "window-notice-banners");
    let pane = test_support::bounds(fleet.cx, "main-pane");
    let window_height = fleet.cx.update(|window, _| window.viewport_size().height);
    assert!(
        viewport.size.height <= window_height / 2.,
        "the notice region is capped"
    );
    assert!(
        pane.size.height >= pane_before.size.height * 0.4,
        "the notices leave room to use the terminal"
    );
    assert!(
        pane.top() >= viewport.bottom(),
        "the pane stays below the notices"
    );
    let visible_dismiss = |cx: &mut VisualTestContext, upload: &notices::InterruptedUploads| {
        let id = format!("notice-dismiss-interrupted-upload-{}", upload.key());
        frame(cx)
            .into_iter()
            .find(|node| {
                node.element_id.as_deref() == Some(id.as_str())
                    && node.bounds.top() >= viewport.top()
                    && node.bounds.bottom() <= viewport.bottom()
            })
            .map(|node| node.bounds)
    };
    assert!(visible_dismiss(fleet.cx, previous).is_some());
    assert!(
        visible_dismiss(fleet.cx, &earlier[0]).is_none(),
        "the existing banners overflow"
    );
    let pointer = viewport.center();
    fleet
        .cx
        .simulate_mouse_move(pointer, None, gpui::Modifiers::none());
    fleet.cx.simulate_event(gpui::ScrollEvent {
        position: pointer,
        delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(10000.))),
        ..Default::default()
    });
    settle(fleet.cx);
    assert!(
        visible_dismiss(fleet.cx, &earlier[0]).is_some(),
        "the oldest banner is still reachable"
    );
    assert!(
        visible_dismiss(fleet.cx, previous).is_none(),
        "the user is looking at old notices"
    );
    fleet
        .cx
        .update(|_, cx| notices::post(Notice::InterruptedUpload(latest.clone()), cx));
    settle(fleet.cx);
    let dismiss =
        visible_dismiss(fleet.cx, latest).expect("the newly posted interruption is visible");
    assert!(
        terminal_focused(fleet.cx),
        "revealing the notice leaves focus on the terminal"
    );
    fleet
        .cx
        .simulate_click(dismiss.center(), gpui::Modifiers::none());
    settle(fleet.cx);
    let posted = fleet.cx.update(|_, cx| notices::posted(cx));
    assert!(!posted.contains(&Notice::InterruptedUpload(latest.clone())));
    assert!(posted.contains(&Notice::InterruptedUpload(previous.clone())));

    let latest_visible = |cx: &mut VisualTestContext| {
        frame(cx).iter().any(|node| {
            node.element_id.as_deref() == Some("notice-dismiss-ConnectionLost")
                && node.bounds.top() >= viewport.top()
                && node.bounds.bottom() <= viewport.bottom()
        })
    };
    assert!(
        !latest_visible(fleet.cx),
        "the earlier connection warning is out of view"
    );
    fleet
        .cx
        .update(|_, cx| notices::post(Notice::ConnectionLost, cx));
    settle(fleet.cx);
    assert!(
        latest_visible(fleet.cx),
        "the repeated warning scrolled into view"
    );
    let posted = fleet.cx.update(|_, cx| notices::posted(cx));
    assert_eq!(posted.last(), Some(&Notice::ConnectionLost));
    assert_eq!(
        posted
            .iter()
            .filter(|notice| **notice == Notice::ConnectionLost)
            .count(),
        1
    );
    fleet.helm.break_channel();
    settle(fleet.cx);
    assert!(!fleet.drawn("window-notice-banners"));
    fleet.helm.connect(a_and_b());
    settle(fleet.cx);
    assert!(
        latest_visible(fleet.cx),
        "the rebuilt stack reveals the same latest notice"
    );
}

/// The hold and the scrim belong to the open session's own Delete: SPEC.md lets Deletes of other sessions run while
/// the user works in this one, and what happens to this one's agent then is real news. Specified: with a open and
/// live, b's Delete confirmed from its row and unanswered, a's agent ending and its viewer detaching show the exit line
/// and the reconnect card, and no scrim and no red band are drawn.
#[gpui::test]
fn a_delete_of_another_session_leaves_the_open_terminal_alone(cx: &mut TestAppContext) {
    let mut fleet = open_main(cx, a_and_b());
    fleet.go_live(0);
    fleet.deletes();
    click(fleet.cx, "session-menu-b");
    click(fleet.cx, "session-menu-delete");
    click(fleet.cx, "prompt-confirm");
    assert_eq!(
        fleet.deletes().len(),
        1,
        "the premise: b's Delete is in flight"
    );

    fleet.delete_happens_to(0);
    assert!(!fleet.drawn("terminal-delete-scrim"));
    assert_eq!(fleet.painted_count(RECONNECTING), 1);
    assert!(fleet.drawn(CARD));
    assert!(!fleet.drawn(BAND));
    assert!(fleet.drawn("terminal-exit"));
}

// ---- A Restart of the open session --------------------------------------------------------------------------------

impl Fleet<'_> {
    /// The ids of the Restarts the window sent since the last call; every other command is answered with success at
    /// once, as [`Fleet::deletes`] does.
    fn restarts(&mut self) -> Vec<u64> {
        let mut found = Vec::new();
        for command in self.helm.commands() {
            if command.method == Method::Post && command.path.ends_with("/restart") {
                found.push(command.id);
            } else {
                self.helm.reply(command.id, 200, None);
            }
        }
        found
    }

    /// With a open and live and its agent working, restart it from the header and confirm, and return the Restart's
    /// id, unanswered.
    fn restart_open_session(&mut self) -> u64 {
        self.restarts();
        click(self.cx, "session-header-restart");
        click(self.cx, "prompt-confirm");
        let [id] = self.restarts()[..] else {
            panic!("one Restart was sent");
        };
        id
    }

    /// The `index`th stream sees what a Restart does to its session: the supervisor moves the viewer to the relaunched
    /// terminal with a detach carrying `other`, which drops the stream into a reconnect.
    fn restart_happens_to(&mut self, index: usize) {
        self.opened.borrow_mut()[index]
            .0
            .detach(Some(DetachCode::Other), "session restarted");
        settle(self.cx);
    }

    /// Neither the reconnect card nor the red band is drawn, and the terminal has no status line of its own: what the
    /// pane must show at every step of a Restart that reattaches while it is pending.
    fn shows_no_outage(&mut self, step: &str) {
        assert!(!self.drawn(CARD), "{step}: no card");
        assert!(!self.drawn(BAND), "{step}: no red band");
        assert_eq!(self.status_words(), None, "{step}: no status line");
        assert_eq!(self.painted_count(RECONNECTING), 0, "{step}");
    }
}

/// The old app's `terminal-restart.spec.ts`, "a restarted session reconnects without a detach banner": a Restart moves
/// the viewer to the relaunched terminal with a detach that reconnects, and showing an outage for the reattach the user
/// just asked for reads as a failure. The old spec watched every banner change through the Restart, since a final check
/// alone would miss a flash; here every step is checked, in the order the real stream gives them: the supervisor
/// detaches the viewer, the helm answers the Restart while the stream still waits its first rung, and only then does
/// the reattach happen. Holding the reconnect card for the Restart was decided by the migration's orchestrator on
/// 2026-11-15; it covers the whole recovery the Restart began, since the helm's answer comes before the reattach.
/// Specified: with a's Restart confirmed and unanswered, the viewer's `other` detach puts the stream into a reconnect
/// (the premise), and neither the card nor the red band is drawn, nor any status line; the same holds once the helm
/// answers the Restart, while the reattach's attempt runs, while its replay arrives and once it is live; a typed key
/// then reaches the new attachment.
#[gpui::test]
fn a_restart_reattaches_without_a_card_or_a_red_band(cx: &mut TestAppContext) {
    let mut fleet = open_main(cx, a_and_b());
    fleet.go_live(0);
    let id = fleet.restart_open_session();
    fleet.shows_no_outage("restart sent");

    fleet.restart_happens_to(0);
    assert!(
        fleet.reconnecting(),
        "the premise: the restart's detach reconnects"
    );
    fleet.shows_no_outage("reconnecting");

    fleet.helm.reply(id, 200, None);
    settle(fleet.cx);
    assert!(
        fleet.reconnecting(),
        "the premise: answered before the reattach"
    );
    fleet.shows_no_outage("answered, still waiting to reattach");

    fleet.opened.borrow_mut()[0].0.end_wait();
    settle(fleet.cx);
    fleet.shows_no_outage("the reattach's attempt");

    let replay = {
        let mut opened = fleet.opened.borrow_mut();
        let asked = opened[0].0.attach();
        let replay = screen(asked.cols, asked.rows);
        opened[0].0.output(replay[0].clone());
        replay
    };
    settle(fleet.cx);
    fleet.shows_no_outage("catching up");
    {
        let mut opened = fleet.opened.borrow_mut();
        for message in replay.into_iter().skip(1) {
            opened[0].0.output(message);
        }
        opened[0].0.replay_complete();
    }
    settle(fleet.cx);
    assert!(!fleet.reconnecting(), "the premise: live again");
    fleet.shows_no_outage("live");

    // Choosing the open session again focuses its terminal, wherever the Restart's question left focus.
    click(fleet.cx, "session-row-a");
    assert!(
        terminal_focused(fleet.cx),
        "the premise: a's terminal has focus"
    );
    fleet.input(0);
    keys(fleet.cx, "x");
    assert_eq!(fleet.input(0), [letter("x", 88)]);
}

/// The hold covers the reattach the Restart asked for, not an outage: once that reattach fails, the stream is down for
/// reasons of its own, and the user needs the card and its Reconnect now (decided by the migration's orchestrator on
/// 2026-11-15: show the card if the reattach fails). The card must appear in the frame the app draws on its own when
/// the attempt fails, or a window the user only watches would keep hiding it. Specified: with a's Restart confirmed,
/// the viewer detached with `other` and the helm's answer in, no card is drawn; when the reattach's first attempt
/// fails, the card is drawn (checked before any frame the test forces) with its words painted once, and no red band;
/// once the stream reattaches and goes live, the card goes.
#[gpui::test]
fn a_restarts_failed_reattach_shows_the_card(cx: &mut TestAppContext) {
    let mut fleet = open_main(cx, a_and_b());
    fleet.go_live(0);
    let id = fleet.restart_open_session();
    fleet.restart_happens_to(0);
    fleet.helm.reply(id, 200, None);
    settle(fleet.cx);
    assert!(fleet.reconnecting(), "the premise: reconnecting");
    assert!(!fleet.drawn(CARD), "the premise: held past the answer");

    fleet.opened.borrow_mut()[0].0.drop_link();
    settle(fleet.cx);
    assert!(fleet.drawn(CARD));
    assert_eq!(fleet.painted_count(RECONNECTING), 1);
    assert!(!fleet.drawn(BAND));

    fleet.go_live(0);
    assert!(!fleet.drawn(CARD));
}

/// A reattach can also fail after its socket attached: it drops in the middle of its replay. The stream then starts
/// its ladder over (an attachment resets it), so the next wait is numbered attempt 1 again and the attempt number
/// alone never shows the failure; held, the old screen would sit there taking no input with nothing saying so, for as
/// long as the reattaches keep dropping. Specified: with a's Restart confirmed, the viewer detached with `other` and
/// the helm's answer in, no card is drawn; the reattach attaches and its replay begins (still no card), then its link
/// drops before the replay completes, and the card is drawn with no red band.
#[gpui::test]
fn a_restarts_reattach_that_drops_mid_replay_shows_the_card(cx: &mut TestAppContext) {
    let mut fleet = open_main(cx, a_and_b());
    fleet.go_live(0);
    let id = fleet.restart_open_session();
    fleet.restart_happens_to(0);
    fleet.helm.reply(id, 200, None);
    settle(fleet.cx);
    assert!(!fleet.drawn(CARD), "the premise: held past the answer");

    {
        let mut opened = fleet.opened.borrow_mut();
        let asked = opened[0].0.attach();
        opened[0]
            .0
            .output(screen(asked.cols, asked.rows)[0].clone());
    }
    settle(fleet.cx);
    assert!(!fleet.drawn(CARD), "the premise: catching up, still held");

    fleet.opened.borrow_mut()[0].0.drop_link();
    settle(fleet.cx);
    assert!(fleet.reconnecting(), "the premise: reconnecting again");
    assert!(fleet.drawn(CARD));
    assert!(!fleet.drawn(BAND));
}

/// The Restart hold is narrower than the Delete's on purpose: a Restart detaches the viewer only to move it, with a
/// detach the stream recovers from by itself, so a stall that lands while it is pending is not the Restart's doing,
/// and its band and Reconnect are the user's only way back. Specified: with a's Restart confirmed and unanswered, a
/// detach as stalled draws the red band with Reconnect.
#[gpui::test]
fn a_restart_does_not_hold_back_a_stall(cx: &mut TestAppContext) {
    let mut fleet = open_main(cx, a_and_b());
    fleet.go_live(0);
    fleet.restart_open_session();
    fleet.opened.borrow_mut()[0].0.detach(
        Some(DetachCode::Stalled),
        farhelm_proto::DETACH_REASON_STALLED,
    );
    settle(fleet.cx);
    assert!(fleet.drawn(BAND));
    assert!(fleet.drawn(STATUS_RECONNECT_ID));
    assert!(!fleet.drawn(CARD));
}

/// Old `terminal-restart.spec.ts`, "a restart whose response is lost still recovers the terminal", the pane's half.
/// Why: a Restart relaunches the agent whether or not its answer gets back, and the supervisor has moved the viewer by
/// then, so a terminal whose recovery waited on that answer would sit dead for a session that is running perfectly
/// well. The stream reattaches by itself, and no part of showing the reattach may wait on the answer. The helm's half,
/// what it records from a restart's reply, is farhelm-helm `a_reply_carrying_unknown_never_erases_a_known_status`; a
/// reply lost to a broken channel is a reset, whose reattach is `a_reset_reattaches_the_surviving_selection`. The old
/// case's other assertion, that the failure is surfaced, is the header's
/// `a_restart_says_after_two_seconds_how_long_it_has_waited_and_then_its_refusal`.
///
/// Spec: with a's Restart confirmed and never answered, the viewer's `other` detach puts the stream into a reconnect
/// (the premise); its reattach attaches and replays, and the terminal is live with no card, no red band and no status
/// line, while the Restart is still in flight; a typed key reaches the new attachment.
#[gpui::test]
fn a_restart_whose_answer_never_comes_still_recovers_the_terminal(cx: &mut TestAppContext) {
    let mut fleet = open_main(cx, a_and_b());
    fleet.go_live(0);
    fleet.restart_open_session();
    fleet.restart_happens_to(0);
    assert!(
        fleet.reconnecting(),
        "the premise: the restart's detach reconnects"
    );

    fleet.go_live(0);
    assert!(!fleet.reconnecting(), "live again");
    fleet.shows_no_outage("live, the Restart unanswered");
    assert!(
        fleet
            .cx
            .update(|_, cx| crate::session_list::restart::in_flight("a", cx)),
        "the premise: the Restart is still in flight"
    );

    click(fleet.cx, "session-row-a");
    assert!(
        terminal_focused(fleet.cx),
        "the premise: a's terminal has focus"
    );
    fleet.input(0);
    keys(fleet.cx, "x");
    assert_eq!(fleet.input(0), [letter("x", 88)]);
}

// ---- A Suspend of the open session's host -------------------------------------------------------------------------

impl Fleet<'_> {
    /// Start `work` on `key` in the window's operation store, as the control that sends it would, after answering
    /// whatever else the window sent, and return its id, unanswered. The path is a stand-in: the pane's hold reads only
    /// the work and the key.
    fn start_work(&mut self, key: OpKey, work: Work) -> u64 {
        self.restarts();
        self.cx.update(|_, cx| {
            let operations = crate::session::current_operations(cx).expect("a UI session");
            let path = farhelm_helm_client::ApiPath::api(&["work"]).unwrap();
            let command = Command::new(work, Method::Post, path, None, "POST /api/work");
            operations.update(cx, |operations, cx| {
                operations.start(&key, command, cx, |_, _| {}).unwrap();
            });
        });
        settle(self.cx);
        let sent = self.helm.commands();
        let [command] = &sent[..] else {
            panic!("one command was sent");
        };
        command.id
    }
}

/// During Suspend the terminal stream can end before the pushed session row goes stale. The window's own request
/// holds back the reconnect card during that gap. The sidebar-and-session-view area decided on the hold on
/// 2026-11-16, extending the orchestrator's 2026-11-15 Restart hold. Specified: with
/// a, on host 2, live, and its Suspend in flight on a's key (what the header and row send), a's `other` detach puts the
/// stream into a reconnect (the premise) and no card is drawn; once the helm refuses it and the stream's next attempt
/// fails, the card is drawn; with the stream still reconnecting (the premise before each check), a Stop and suspend on
/// a's key hides it until its refusal, a Suspend on host 2's key (the Hosts panel's) hides it again, and the push that
/// lists a stale before that Suspend's answer leaves it hidden after.
#[gpui::test]
fn a_suspend_from_this_window_holds_back_the_reconnect_card(cx: &mut TestAppContext) {
    let snapshot = vec![
        listing(&["a", "b"], &[session("a").host(2), session("b")]),
        preferences(&["a"], None),
    ];
    let mut fleet = open_main(cx, snapshot);
    fleet.go_live(0);
    let id = fleet.start_work(OpKey::session("a"), Work::Suspend);
    fleet.opened.borrow_mut()[0]
        .0
        .detach(Some(DetachCode::Other), "host suspending");
    settle(fleet.cx);
    assert!(fleet.reconnecting(), "the premise: the stream ended");
    assert!(!fleet.drawn(CARD), "the session's Suspend");

    fleet.helm.reply(id, 409, None);
    fleet.opened.borrow_mut()[0].0.drop_link();
    settle(fleet.cx);
    assert!(fleet.drawn(CARD), "refused, and the next attempt failed");

    let id = fleet.start_work(OpKey::session("a"), Work::StopAndSuspend);
    assert!(fleet.reconnecting(), "the premise");
    assert!(!fleet.drawn(CARD), "the session's Stop and suspend");
    fleet.helm.reply(id, 409, None);
    settle(fleet.cx);
    assert!(fleet.drawn(CARD), "refused");

    let id = fleet.start_work(OpKey::host(&HostView::key(2)), Work::Suspend);
    assert!(fleet.reconnecting(), "the premise");
    assert!(!fleet.drawn(CARD), "the Hosts panel's Suspend");
    let stale = [session("a").host(2).stale()];
    fleet
        .helm
        .push(change(listing_meta(&["a", "b"], 2, false), &stale, &[]));
    fleet.helm.reply(id, 200, None);
    settle(fleet.cx);
    assert!(fleet.reconnecting(), "the premise");
    assert!(!fleet.drawn(CARD), "stale once answered");
}

// ---- Resets --------------------------------------------------------------------------------------------------------

/// SPEC_impl.md, "One failure path: the reset": nothing made for an old connection outlives it, and SPEC.md, "Losing
/// the connection to the helm", keeps the selection by id. So the terminal attached under the old UI session goes, and
/// the session the snapshot builds attaches the same selection again, under a new lease, with its own terminal home.
/// Specified: with b open and live, a break closes b's stream and opens nothing; the reconnect's snapshot, still
/// listing b, opens one new stream for b, and once it is live a typed letter reaches it and not the old one.
#[gpui::test]
fn a_reset_reattaches_the_surviving_selection(cx: &mut TestAppContext) {
    let snapshot = || {
        vec![
            listing(&["a", "b"], &[session("a"), session("b")]),
            preferences(&["b"], None),
        ]
    };
    let mut fleet = open_main(cx, snapshot());
    fleet.go_live(0);
    assert_eq!(fleet.attached(), ["b"], "the premise: b is open");

    fleet.helm.break_channel();
    settle(fleet.cx);
    assert_eq!(fleet.phase(0), FakePhase::Closed);
    assert_eq!(fleet.attached(), ["b"], "nothing attaches while inert");

    fleet.helm.connect(snapshot());
    settle(fleet.cx);
    assert_eq!(fleet.attached(), ["b", "b"]);
    {
        let opened = fleet.opened.borrow();
        assert_ne!(opened[0].1.lease, opened[1].1.lease);
    }
    fleet.go_live(1);
    assert!(terminal_focused(fleet.cx));
    keys(fleet.cx, "x");
    assert_eq!(fleet.input(1), [letter("x", 88)]);
    assert_eq!(fleet.input(0), []);
}

/// The reconnect creates fresh terminal streams while the tab choice outlives the UI session. After replay, typing
/// reaches the remembered shell through its new stream and the agent receives nothing.
#[gpui::test]
fn a_reset_restores_the_chosen_tab_on_its_new_stream(cx: &mut TestAppContext) {
    let snapshot = || {
        vec![
            listing(&["a", "b"], &[session("a"), session("b").tabs(1)]),
            preferences(&["b"], None),
        ]
    };
    let mut fleet = open_main(cx, snapshot());
    assert_eq!(
        fleet.attached(),
        ["b", "b"],
        "the premise: b and its tab are attached"
    );
    assert!(
        fleet
            .opened
            .borrow()
            .iter()
            .any(|(_, attach)| attach.tab.as_deref() == Some("tab-0")),
        "the snapshot listed the shell tab"
    );
    click(fleet.cx, "session-terminal-tab-tab-0");
    assert!(
        terminal_focused(fleet.cx),
        "the tab accepted focus before reset"
    );

    fleet.helm.break_channel();
    settle(fleet.cx);
    assert_eq!(fleet.phase(0), FakePhase::Closed);
    assert_eq!(fleet.phase(1), FakePhase::Closed);
    assert_eq!(fleet.attached(), ["b", "b"]);

    fleet.helm.connect(snapshot());
    settle(fleet.cx);
    let (agent, tab) = {
        let opened = fleet.opened.borrow();
        assert_eq!(opened.len(), 4);
        let find = |tab: Option<&str>| {
            opened[2..]
                .iter()
                .position(|(_, attach)| attach.session == "b" && attach.tab.as_deref() == tab)
                .expect("the reconnect attaches both terminals of b")
                + 2
        };
        (find(None), find(Some("tab-0")))
    };
    fleet.go_live(agent);
    fleet.go_live(tab);
    assert!(terminal_focused(fleet.cx));
    keys(fleet.cx, "x");
    assert_eq!(fleet.input(tab), [letter("x", 88)]);
    assert_eq!(fleet.input(agent), []);
}
