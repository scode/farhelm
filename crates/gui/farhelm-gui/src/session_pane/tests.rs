//! Tests of the main pane's terminal: which session it attaches and when it lets go of a stream, where focus goes when
//! the user or the selection rules open a session, what a Delete of the open session draws over it, and what a reset
//! does to the terminal.
//!
//! The window is the real main view over the helm client's fake channel, and its connection hands out the fake terminal
//! (`Connection::serve_terminals`), which plays the helm's side of every stream the pane opens and records it. The
//! exceptions are the two tests about a text field that holds focus while a terminal arrives: the main screen has no
//! text field yet (the session filter is a later area's), so those tests draw the session list and the pane beside a
//! field of their own. Every step ends in one of the shared harness's step helpers, so the focus and layer invariants
//! are checked after each.

use std::cell::RefCell;
use std::rc::Rc;

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_helm_client::fake::terminal::{FakePhase, FakeTerminal, FakeTerminalRequest};
use farhelm_helm_client::terminal::TerminalAttach;
use farhelm_input_mapping::Platform;
use farhelm_proto::DetachCode;
use farhelm_proto::grid::{GridInput, GridOutput};
use farhelm_proto::ui_channel::{Availability, Collection, CollectionChanges, CollectionState};
use gpui::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, Styled as _, TestAppContext, VisualTestContext, Window, div, px,
};

use super::SessionPane;
use crate::actions::{DELETE_SESSION_CHORD, THIS_PLATFORM};
use crate::components::{HoverHelp, TextInput, TextInputState};
use crate::launcher::launch_tests::{
    HostChange, command_draft, creates_answering, start_host_change,
};
use crate::launcher::view_tests::fixture_hosts;
use crate::main_view::MainView;
use crate::session::Connection;
use crate::session_list::SessionList;
use crate::session_list::host_rail::tests::node;
use crate::session_list::tests::{change, listing, listing_meta, painted, preferences, session};
use crate::terminal::attached::{AttachedTerminal, STATUS_RECONNECT_ID};
use crate::test_support::{self, click, keys, settle, terminal_focused};
use crate::test_support::{frame, meta, started};
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

/// A root view with a text field above the session list, and the pane beside them: what the main screen will be once
/// the session filter exists, which is the field SPEC.md's case is about.
struct WithField {
    list: Entity<SessionList>,
    pane: Entity<SessionPane>,
    field: TextInputState,
    root: root::Attachment,
}

impl WithField {
    /// The root view, with the pane given the drawn list's New handle, as the main view gives it, so New is home here
    /// over an empty fleet exactly as on the main screen.
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let root = root::attach(window, cx);
        let list = cx.new(SessionList::new);
        let new = list.read(cx).new_focus().clone();
        WithField {
            root,
            list,
            pane: cx.new(|cx| SessionPane::new(new, window, cx)),
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

// ---- New as home with no sessions ---------------------------------------------------------------------------------

/// SPEC.md, "Where focus rests": "With no terminal on screen, focus rests on New when there are no sessions." Without
/// it a window with no sessions starts with nothing focused, and the first Tab or Enter has nowhere obvious to go.
/// Specified: a window whose first listing is complete, available and empty opens with New focused and opens no
/// terminal stream.
#[gpui::test]
fn with_no_sessions_focus_rests_on_new(cx: &mut TestAppContext) {
    let mut fleet = open_main(cx, vec![listing(&[], &[])]);
    assert_eq!(fleet.selected(), None, "the premise: nothing is open");
    assert_eq!(
        test_support::focused_id(fleet.cx).as_deref(),
        Some(crate::session_list::NEW_ID)
    );
    assert!(fleet.attached().is_empty());
}

/// The same rule when the fleet empties while the window is open, which is the path a user deleting their last
/// session takes: the terminal that held focus goes, and New, not the root, is where focus lands. Specified: with the
/// only session a's terminal live and focused, a complete listing pushed without a closes a's stream and leaves New
/// focused.
#[gpui::test]
fn removing_the_last_session_sends_focus_to_new(cx: &mut TestAppContext) {
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
        Some(crate::session_list::NEW_ID)
    );
}

/// The fleet can become empty while nothing is open and nothing else the pane watches moves: a listing that failed
/// (the helm could not read it, so it claimed nothing and opened nothing) comes back complete and empty. Only the card
/// follower's empty-fleet flag tells the pane then, so this is the test that keeps it. (The helm cannot deliver a first
/// listing after the snapshot: a push to a collection no snapshot installed is a protocol violation, and the reset
/// that follows syncs the pane anyway.) Its opening check also keeps the rule that New is home only on proof that the
/// fleet is empty; the unread listing's case of that rule is pinned on the shared proof itself
/// (`session_list::tests::only_a_complete_available_empty_listing_is_an_empty_fleet`). Specified: a window whose
/// snapshot holds a failed session listing with no rows opens nothing and does not focus New; once an available,
/// complete, empty listing is pushed, New holds focus and still nothing is open.
#[gpui::test]
fn a_listing_that_comes_back_empty_sends_focus_to_new(cx: &mut TestAppContext) {
    let mut failed = listing(&[], &[]);
    failed.meta.availability = Availability::Unavailable {
        error: "down".into(),
    };
    let mut fleet = open_main(cx, vec![failed, preferences(&[], None)]);
    assert_eq!(fleet.selected(), None, "the premise: nothing is open");
    assert_ne!(
        test_support::focused_id(fleet.cx).as_deref(),
        Some(crate::session_list::NEW_ID),
        "the premise: a failed listing does not make New home"
    );

    fleet
        .helm
        .push(change(listing_meta(&[], 0, false), &[], &[]));
    settle(fleet.cx);
    assert_eq!(
        test_support::focused_id(fleet.cx).as_deref(),
        Some(crate::session_list::NEW_ID)
    );
    assert_eq!(fleet.selected(), None);
    assert!(fleet.attached().is_empty());
}

/// New is not drawn as a tab stop while it is disabled (a launch or a host change in flight), and focus on a handle
/// the frame does not draw takes no keys at all: before the focus coordinator counted such a home as none, focus
/// parked on the undrawn New and Tab, Escape and every Farhelm shortcut did nothing until New came back. That is a new
/// user's first run, an empty fleet with a first launch or a first host in flight. This is the integration guard for
/// New as that undrawn home; the coordinator's own tests cover the general rule (`focus_tests.rs`). A host probe stands
/// in for both causes (New reads one answer for either, `launcher::entry::new_available`). Specified, over an empty
/// fleet with New focused: with a probe in flight New is drawn disabled and focus rests on the window's root, which the
/// per-step focus invariant checks is drawn (no control holds it); once the probe is answered, New holds focus again.
#[gpui::test]
fn while_new_is_disabled_focus_rests_on_the_root_and_returns_to_new(cx: &mut TestAppContext) {
    let mut fleet = open_main(cx, vec![listing(&[], &[]), preferences(&[], None)]);
    assert_eq!(
        test_support::focused_id(fleet.cx).as_deref(),
        Some(crate::session_list::NEW_ID),
        "the premise: New is home"
    );

    let probe = fleet.probe_in_flight();
    let new = node(&frame(fleet.cx), crate::session_list::NEW_ID)
        .cloned()
        .expect("New is drawn");
    assert!(!new.flags.focusable, "the premise: New is disabled");
    assert_eq!(
        test_support::focused_id(fleet.cx),
        None,
        "focus rests on the root, which no control is"
    );
    fleet.helm.reply(probe, 200, Some(serde_json::json!({})));
    settle(fleet.cx);
    assert_eq!(
        test_support::focused_id(fleet.cx).as_deref(),
        Some(crate::session_list::NEW_ID),
        "New drawn enabled again takes focus back"
    );
}

/// The daily path through New as home: a user's first launch over an empty fleet. Focus goes from New (home) to the
/// window's root while New is disabled during the create, and must end on the new session's terminal, never back on
/// New: SPEC.md, "Who may move focus", has a launch the user waited for open "with its terminal focused". This holds
/// because the helm pushes every change a command causes before the command's reply (SPEC_impl.md), so the listing
/// that holds the new session, and so makes the fleet non-empty and opens the session, arrives before the reply
/// re-enables New. Specified: over an empty fleet with New focused, the launcher opened on a complete Command draft and
/// Launch pressed send one create; once the helm pushes a listing holding b and then answers the create with b, b is
/// selected and its terminal holds focus; New holds focus after none of these steps.
#[gpui::test]
fn a_first_launch_over_an_empty_fleet_ends_on_the_new_sessions_terminal(cx: &mut TestAppContext) {
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
        Some(crate::session_list::NEW_ID),
        "the premise: New is home over an empty fleet"
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
    let new_focused = |fleet: &mut Fleet<'_>| {
        test_support::focused_id(fleet.cx).as_deref() == Some(crate::session_list::NEW_ID)
    };
    assert!(
        !new_focused(&mut fleet),
        "after Launch, New does not hold focus"
    );

    let state = listing(&["b"], &[session("b")]);
    fleet.helm.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: state.entries,
        remove: Vec::new(),
    });
    settle(fleet.cx);
    assert!(
        !new_focused(&mut fleet),
        "after the listing, New does not hold focus"
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
        !new_focused(&mut fleet),
        "after the reply, New does not hold focus"
    );
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

    keys(fleet.cx, "enter");
    assert!(
        fleet
            .helm
            .commands()
            .iter()
            .any(|command| command.method == Method::Delete && command.path == "/api/sessions/a")
    );
    assert!(
        terminal_focused(fleet.cx),
        "closing the question gave focus back"
    );
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
