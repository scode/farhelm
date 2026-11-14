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
use farhelm_proto::ui_channel::{Collection, CollectionChanges, CollectionState};
use gpui::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, Styled as _, TestAppContext, VisualTestContext, Window, div, px,
};

use super::SessionPane;
use crate::actions::{DELETE_SESSION_CHORD, THIS_PLATFORM};
use crate::components::{HoverHelp, TextInput, TextInputState};
use crate::main_view::MainView;
use crate::session::Connection;
use crate::session_list::SessionList;
use crate::session_list::tests::{listing, painted, preferences, session};
use crate::test_support::{self, click, keys, settle};
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

    /// Whether the pane shows a terminal and that terminal holds focus.
    fn terminal_focused(&mut self) -> bool {
        let pane = self.pane.clone();
        self.cx.update(|window, cx| {
            let terminal = pane.update(cx, |pane, cx| {
                pane.mounted
                    .current(cx)
                    .as_ref()
                    .map(|mounted| mounted.terminal.clone())
            });
            terminal.is_some_and(|terminal| terminal.read(cx).focus_handle().is_focused(window))
        })
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
/// every listed session, or attached before the selection settled, would take each session's terminal from whatever
/// other client holds it. The startup choice is automatic, and its terminal is home, so with nothing else holding
/// focus it takes focus. Specified: with a and b listed and a last visited, the pane opens exactly one stream, for a's
/// agent terminal (no tab), and that terminal holds focus.
#[gpui::test]
fn selecting_a_session_attaches_it_and_only_it(cx: &mut TestAppContext) {
    let mut fleet = open_main(cx, a_and_b());
    assert_eq!(fleet.selected().as_deref(), Some("a"), "the premise");
    assert_eq!(fleet.attached(), ["a"]);
    assert_eq!(fleet.opened.borrow()[0].1.tab, None, "the agent terminal");
    assert!(fleet.terminal_focused());
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
    assert!(fleet.terminal_focused(), "the click focused b's terminal");
    fleet.go_live(1);
    keys(fleet.cx, "x");
    assert_eq!(fleet.input(1), [letter("x", 88)]);
    assert_eq!(fleet.input(0), [], "nothing reached a's program");

    click(fleet.cx, "session-row-b");
    settle(fleet.cx);
    assert!(
        fleet.terminal_focused(),
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
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        WithField {
            root: root::attach(window, cx),
            list: cx.new(SessionList::new),
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
    assert!(!fleet.terminal_focused());
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
        !fleet.terminal_focused(),
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
    assert!(!fleet.terminal_focused());
    assert_eq!(fleet.input(1), []);
    let text = fleet.cx.update(|_, cx| field.text(cx));
    assert_eq!(text.as_ref(), "x");
}

/// SPEC.md, "Cases that settled disputed points": "The user's session is deleted from another window while they type
/// in it. The next session is selected and its terminal takes the rest of their typing (the terminal is home)."
/// Specified: with a's terminal live and typed into, a complete listing pushed without a opens b, closes a's stream
/// and attaches b's, whose terminal holds focus; once live, a typed letter reaches b's program.
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
    assert!(fleet.terminal_focused(), "the next terminal is home");
    fleet.go_live(1);
    keys(fleet.cx, "y");
    assert_eq!(fleet.input(1), [letter("y", 89)]);
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
        fleet.terminal_focused(),
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
        fleet.terminal_focused(),
        "closing the question gave focus back"
    );
}

// ---- A Delete of the open session ---------------------------------------------------------------------------------

/// What the status line says once a Delete's detach has dropped the stream: the detach the supervisor sends when it
/// deletes a session carries `other`, which the stream answers by reconnecting.
const RECONNECTING: &str = "Session unavailable — reconnecting";

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
                    .map(|mounted| mounted.terminal.clone())
            });
            terminal.and_then(|terminal| terminal.read(cx).shown_lift(cx))
        })
    }

    /// The pane's terminal's own status line, held or not: the stream's state as the terminal knows it.
    fn status_words(&mut self) -> Option<String> {
        let pane = self.pane.clone();
        self.cx.update(|_, cx| {
            let terminal = pane.update(cx, |pane, cx| {
                pane.mounted
                    .current(cx)
                    .as_ref()
                    .map(|mounted| mounted.terminal.clone())
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
/// drawn and "Stopping agent…" is painted twice (row and scrim); a's terminal holds focus and a typed letter reaches
/// its program; after focus moves to b's menu button, a click on the scrim focuses a's terminal; once the helm
/// refuses, the scrim is gone.
#[gpui::test]
fn a_pending_delete_puts_a_scrim_over_the_terminal_until_the_helm_answers(cx: &mut TestAppContext) {
    let mut fleet = open_main(cx, a_and_b());
    fleet.go_live(0);
    assert!(!fleet.drawn("terminal-delete-scrim"), "the premise");

    let id = fleet.delete_open_session();
    assert!(fleet.drawn("terminal-delete-scrim"));
    assert_eq!(fleet.painted_count(STOPPING), 2, "the row and the scrim");
    assert!(fleet.terminal_focused());
    keys(fleet.cx, "x");
    assert_eq!(fleet.input(0), [letter("x", 88)], "keys still reach a");

    click(fleet.cx, "session-menu-b");
    keys(fleet.cx, "escape");
    assert!(
        !fleet.terminal_focused(),
        "the premise: focus is on b's menu button"
    );
    click(fleet.cx, "terminal-delete-scrim");
    assert!(fleet.terminal_focused(), "the press went through the scrim");

    fleet
        .helm
        .reply(id, 409, Some(serde_json::json!("the agent is busy")));
    settle(fleet.cx);
    assert!(!fleet.drawn("terminal-delete-scrim"));
    assert_eq!(fleet.painted_count(STOPPING), 0);
}

/// SPEC.md: "The terminal's own 'Detached' notice, which the delete itself causes, is held back meanwhile". The Delete
/// kills the agent and detaches the viewer, and both would otherwise show as failures (an exit line, a red "Session
/// unavailable" band) under a Delete that is going exactly as asked. Specified: with a's Delete in flight, the agent
/// ended by signal 15 and the viewer detached with `other`, the terminal's own status line says it is reconnecting
/// (the premise), yet no exit line is drawn, the band's words are not painted, and the screen is not lifted to make
/// room for either; the scrim still shows.
#[gpui::test]
fn the_terminal_holds_back_what_the_delete_does_to_it(cx: &mut TestAppContext) {
    let mut fleet = open_main(cx, a_and_b());
    fleet.go_live(0);
    fleet.delete_open_session();

    fleet.delete_happens_to(0);
    assert_eq!(
        fleet.status_words().as_deref(),
        Some(RECONNECTING),
        "the premise"
    );
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
/// with the screen lifted for it; the row shows the helm's words and "Session unavailable — reconnecting" is painted.
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
    assert_eq!(fleet.painted_count("the session's tabs did not close"), 1);
    assert_eq!(fleet.painted_count(RECONNECTING), 1);
}

/// A Delete that succeeds removes its session, and the selection rules open the next one once it has settled
/// (SPEC.md, "Session list"). Nothing of the deleted session's hold may reach the next terminal: its own exit and
/// detach are real news. Specified: after a's Delete, a's death and detach, a pushed listing without a and the
/// helm's success, b is open and its stream attached; once live, no scrim is drawn, and b's own agent ending and its
/// viewer detaching show the exit line and "Session unavailable — reconnecting".
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
    assert!(fleet.terminal_focused());
    fleet.delete_happens_to(1);
    assert_eq!(fleet.painted_count(RECONNECTING), 1);
    assert!(fleet.drawn("terminal-exit"));
}

/// The hold and the scrim belong to the open session's own Delete: SPEC.md lets Deletes of other sessions run while
/// the user works in this one, and what happens to this one's agent then is real news. Specified: with a open and
/// live, b's Delete confirmed from its row and unanswered, a's agent ending and its viewer detaching show the exit line
/// and "Session unavailable — reconnecting", and no scrim is drawn.
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
    assert!(fleet.drawn("terminal-exit"));
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
    assert!(fleet.terminal_focused());
    keys(fleet.cx, "x");
    assert_eq!(fleet.input(1), [letter("x", 88)]);
    assert_eq!(fleet.input(0), []);
}
