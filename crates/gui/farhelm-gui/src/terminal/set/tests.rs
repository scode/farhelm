//! Tests of the session view's terminal set: which streams it opens for a session's tabs and under which lease, what
//! the strip lists and chooses, where focus goes, how tabs opened, closed and reaped elsewhere come and go, that a
//! terminal whose stream stopped holds back no other, the bound on attached tabs, and New terminal and each tab's ×
//! (where it sits and when it shows, its question, which tab it closes, and a refused close).
//!
//! The window is the real main view over the helm client's fake channel, and its connection hands out fake terminals
//! (`Connection::serve_terminals`), which play the helm's side of every stream and record what each was asked. A tab
//! list changes the way the helm changes it: a pushed change to the open session's row.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use farhelm_helm_client::fake::FakeHelm;
use farhelm_helm_client::fake::terminal::{FakePhase, FakeTerminal, FakeTerminalRequest};
use farhelm_helm_client::fake::transfers::{FakeAnswer, FakeTransfer, FakeTransfers};
use farhelm_helm_client::terminal::TerminalAttach;
use farhelm_helm_client::transfers::Transfers;
use farhelm_proto::DetachCode;
use farhelm_proto::grid::{GridInput, GridOutput};
use farhelm_proto::ui_channel::{Collection, CollectionChanges, CollectionState, KeyedEntry};
use gpui::{
    Bounds, Entity, FocusHandle, Modifiers, Pixels, Point, SharedString, TestAppContext,
    VisualTestContext,
};
use serde_json::json;

use super::{
    AGENT_TAB_ID, MAX_ATTACHED_TABS, NOT_ATTACHED_ID, STRIP_ID, TerminalSet, tab_element_id,
};
use crate::focus::Mark;
use crate::main_view::MainView;
use crate::notices::{self, Notice};
use crate::session::Connection;
use crate::session_list::host_rail::tests::{answer_everything, filtered};
use crate::session_list::tests::{listing_meta, painted, preferences, session};
use crate::session_pane::SessionPane;
use crate::terminal::attached::STATUS_RECONNECT_ID;
use crate::terminal::attachments::{self, Offered};
use crate::test_support::{self, click, drawn, keys, settle, started};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// Tab t1's element id, as [`tab_element_id`] names it (the click helper takes a static id).
const T1_TAB: &str = "session-terminal-tab-t1";

/// Tab t2's element id, as the strip names it.
const T2_TAB: &str = "session-terminal-tab-t2";

/// Tab t3's element id, as the strip names it.
const T3_TAB: &str = "session-terminal-tab-t3";

/// The real main view receives the local platform's chords. The physical-key tests cover both native key identities.
fn tab_chords() -> (&'static str, &'static str) {
    if cfg!(target_os = "macos") {
        ("cmd-shift-[", "cmd-shift-]")
    } else {
        ("ctrl-shift-[", "ctrl-shift-]")
    }
}

/// Every fake terminal the connection handed out, in the order they were opened, with the attach each was for.
type Opened = Rc<RefCell<Vec<(FakeTerminal, TerminalAttach)>>>;

/// The main window over the fake helm, and the terminals it opened.
struct Window<'a> {
    cx: &'a mut VisualTestContext,
    helm: FakeHelm,
    /// The main view, for the tests that drive its Delete shortcut on another platform than the build's
    /// ([`MainView::delete_or_close_tab`]).
    main: Entity<MainView>,
    pane: Entity<SessionPane>,
    opened: Opened,
}

/// Session `key`'s row listing the tabs `tabs` (by id, in the helm's order).
fn row(key: &'static str, tabs: &[&str]) -> KeyedEntry {
    let mut entry = session(key).entry();
    entry.value["tabs"] = json!(
        tabs.iter()
            .map(|id| json!({ "id": id }))
            .collect::<Vec<_>>()
    );
    entry
}

/// Sessions a (open, with `tabs`) and b (no tabs).
fn snapshot(tabs: &[&str]) -> Vec<CollectionState> {
    vec![
        CollectionState {
            collection: Collection::Sessions,
            meta: listing_meta(&["a", "b"], 2, false),
            entries: vec![row("a", tabs), row("b", &[])],
        },
        preferences(&["a"], None),
    ]
}

/// The app connected with `snapshot`, its connection handing out fake terminals, and the main view open.
fn open(cx: &mut TestAppContext, snapshot: Vec<CollectionState>) -> Window<'_> {
    open_with_transfers(cx, snapshot, None)
}

/// Set up a live view with optional fake transfers so a test can leave an upload pending across a remount.
fn open_with_transfers(
    cx: &mut TestAppContext,
    snapshot: Vec<CollectionState>,
    transfers: Option<Transfers>,
) -> Window<'_> {
    let (mut helm, connection) = started(cx);
    let opened = serve_terminals(&connection, cx);
    if let Some(transfers) = transfers {
        cx.update(|cx| {
            connection.update(cx, |connection, _| connection.serve_transfers(transfers));
        });
    }
    helm.connect(snapshot);
    cx.run_until_parked();
    let (main, cx) = test_support::open_view(cx, MainView::new);
    let pane = cx.update(|_, cx| main.read(cx).session_pane().clone());
    Window {
        cx,
        helm,
        main,
        pane,
        opened,
    }
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

/// A complete first screen of `cols` by `rows` showing `text`, as the supervisor replays one.
fn screen(cols: u16, rows: u16, text: &str) -> Vec<GridOutput> {
    [
        json!({"t":"hello","cols":cols,"rows":rows,"history_cap":0}),
        json!({"t":"state","mouse":"off","focus":false,"alt_screen":false,
            "default_fg":null,"default_bg":null,"history_trimmed":false,"exit":null}),
        json!({"t":"rows","rows":[[0,{"w":false,"r":[[0,text.chars().count(),text,[0,-1,-1]]]}]]}),
        json!({"t":"frame","rev":1,"cols":cols,"rows":rows,
            "cursor":{"x":0,"y":0,"visible":true,"shape":"block","blink":false}}),
    ]
    .into_iter()
    .map(|json| serde_json::from_value(json).unwrap())
    .collect()
}

impl Window<'_> {
    /// The tab each stream opened so far was for (`None` for the agent terminal), in order.
    fn tabs_attached(&self) -> Vec<Option<String>> {
        self.opened
            .borrow()
            .iter()
            .map(|(_, attach)| attach.tab.clone())
            .collect()
    }

    /// The index of the stream opened for `tab` (`None` for the agent terminal); panics if there is none.
    fn stream_of(&self, tab: Option<&str>) -> usize {
        self.opened
            .borrow()
            .iter()
            .position(|(_, attach)| attach.tab.as_deref() == tab)
            .unwrap_or_else(|| panic!("no stream for {tab:?}"))
    }

    /// Find session a's replacement stream after a remount or connection reset, avoiding the retired stream.
    fn reopened_stream(&self, first: usize, tab: Option<&str>) -> usize {
        self.opened
            .borrow()
            .iter()
            .enumerate()
            .skip(first)
            .find(|(_, (_, attach))| attach.session == "a" && attach.tab.as_deref() == tab)
            .unwrap_or_else(|| panic!("session a did not reopen {tab:?}"))
            .0
    }

    /// The upload notice strip of the named terminal, including when that terminal is hidden.
    fn upload_lines(&mut self, id: Option<&str>) -> Vec<SharedString> {
        let set = self.set();
        self.cx.update(|_, cx| {
            let set = set.read(cx);
            let terminal = if let Some(id) = id {
                set.tabs
                    .iter()
                    .find(|tab| tab.id == id)
                    .and_then(|tab| tab.terminal.as_ref())
                    .expect("the shell has an attached-terminal view")
            } else {
                set.agent()
            };
            terminal.read(cx).attachments().read(cx).lines_shown()
        })
    }

    /// Stream `index`'s attach opens and replays a screen showing `text`, so its terminal goes live.
    fn go_live(&mut self, index: usize, text: &str) {
        {
            let mut opened = self.opened.borrow_mut();
            let terminal = &mut opened[index].0;
            let asked = terminal.attach();
            for message in screen(asked.cols, asked.rows, text) {
                terminal.output(message);
            }
            terminal.replay_complete();
        }
        settle(self.cx);
    }

    /// What stream `index` received since the last call.
    fn received(&self, index: usize) -> Vec<FakeTerminalRequest> {
        self.opened.borrow_mut()[index].0.received()
    }

    /// Stream `index`'s phase.
    fn phase(&self, index: usize) -> FakePhase {
        self.opened.borrow_mut()[index].0.phase()
    }

    /// Whether stream `index` received any typed input since the last call.
    fn typed_into(&self, index: usize) -> bool {
        self.received(index)
            .iter()
            .any(|request| matches!(request, FakeTerminalRequest::Input { .. }))
    }

    /// Push session a's row listing `tabs`, as the helm does when its tabs change.
    fn push_tabs(&mut self, tabs: &[&str]) {
        self.helm.push(CollectionChanges {
            collection: Collection::Sessions,
            meta: listing_meta(&["a", "b"], 2, false),
            upsert: vec![row("a", tabs)],
            remove: Vec::new(),
        });
        settle(self.cx);
    }

    /// The set the pane shows.
    fn set(&mut self) -> Entity<TerminalSet> {
        let pane = self.pane.clone();
        self.cx
            .update(|_, cx| pane.update(cx, |pane, cx| pane.terminal_set(cx)))
            .expect("a session is open")
    }

    /// The chosen tab's id, `None` for the agent terminal.
    fn chosen(&mut self) -> Option<String> {
        let set = self.set();
        self.cx
            .update(|_, cx| set.read(cx).chosen().map(str::to_owned))
    }

    /// The shell tab a new pane would try to restore for session a.
    fn remembered(&mut self) -> Option<String> {
        self.cx.update(|_, cx| {
            cx.try_global::<super::ShownTerminals>()
                .and_then(|shown| shown.0.get("a"))
                .cloned()
        })
    }

    /// The words the strip's tabs paint, in order.
    fn strip_labels(&mut self) -> Vec<String> {
        let nodes = test_support::frame(self.cx);
        let mut labels: Vec<(f32, String)> = nodes
            .iter()
            .filter(|node| {
                node.element_id
                    .as_deref()
                    .is_some_and(|id| id.starts_with("session-terminal-tab-"))
            })
            .filter_map(|node| {
                node.label
                    .as_ref()
                    .map(|label| (f32::from(node.bounds.origin.x), label.to_string()))
            })
            .collect();
        labels.sort_by(|a, b| a.0.total_cmp(&b.0));
        labels.into_iter().map(|(_, label)| label).collect()
    }

    /// Whether the element holding focus, in a fresh frame, is drawn under the element `ancestor` (or is it), by the
    /// frame's own parent links. The id the app gives a text field (`rename-title`, say) is on a wrapper `div` that
    /// tracks no focus handle, with GPUI Kit's input drawn inside it tracking its own (`components/text_input.rs`), so
    /// a field's focus is looked for under that id rather than only at it.
    fn focus_inside(&mut self, ancestor: &str) -> bool {
        let nodes = test_support::frame(self.cx);
        let Some(mut at) = nodes.iter().position(|node| node.flags.focused) else {
            return false;
        };
        loop {
            if nodes[at].element_id.as_deref() == Some(ancestor) {
                return true;
            }
            match nodes[at].parent {
                Some(parent) => at = parent,
                None => return false,
            }
        }
    }

    /// Where every close control in the strip is drawn, left to right. Found by an accessible name starting with
    /// "Close" anywhere inside the strip rather than by the shell tabs' close ids, so a close control beside the Agent
    /// tab counts whatever it is named; the × is an icon now, so there is no painted "×" to look for.
    fn strip_closes(&mut self) -> Vec<Bounds<Pixels>> {
        let nodes = test_support::frame(self.cx);
        let in_strip = |mut at: usize| loop {
            if nodes[at].element_id.as_deref() == Some(STRIP_ID) {
                return true;
            }
            match nodes[at].parent {
                Some(parent) => at = parent,
                None => return false,
            }
        };
        let mut closes: Vec<Bounds<Pixels>> = nodes
            .iter()
            .enumerate()
            .filter(|(at, node)| {
                node.label
                    .as_ref()
                    .is_some_and(|label| label.starts_with("Close"))
                    && in_strip(*at)
            })
            .map(|(_, node)| node.bounds)
            .collect();
        closes.sort_by(|a, b| f32::from(a.origin.x).total_cmp(&f32::from(b.origin.x)));
        closes
    }

    /// Whether shell tab `tab`'s × shows (`Some(true)`) or is drawn transparent (`Some(false)`) in a fresh frame, by
    /// its wrapper's `:shown` or `:hidden` debug selector, since the frame's record carries no opacity. `None` when the
    /// strip draws neither, which no caller expects. Only the tabs these tests use have selectors here, because the
    /// test context looks selectors up by `&'static str`.
    fn close_shown(&mut self, tab: &str) -> Option<bool> {
        let (shown, hidden) = match tab {
            "t1" => (
                "session-terminal-close-t1:shown",
                "session-terminal-close-t1:hidden",
            ),
            "t2" => (
                "session-terminal-close-t2:shown",
                "session-terminal-close-t2:hidden",
            ),
            _ => panic!("no reveal selectors for {tab}"),
        };
        test_support::redraw(self.cx);
        match (drawn(self.cx, shown), drawn(self.cx, hidden)) {
            (true, false) => Some(true),
            (false, true) => Some(false),
            _ => None,
        }
    }

    /// Rest the pointer at `at`, as a real pointer reaches a place: one move, then whatever it wakes.
    fn point_at(&mut self, at: Point<Pixels>) {
        self.cx.simulate_mouse_move(at, None, Modifiers::none());
        settle(self.cx);
    }

    /// Put keyboard focus on `handle` through the focus coordinator, as Tab would.
    fn focus(&mut self, handle: &FocusHandle) {
        self.cx
            .update(|w, cx| crate::focus::focus_control(handle, &Mark::take(cx), w, cx));
        settle(self.cx);
    }

    /// Shell tab `id`'s button's and ×'s focus handles.
    fn tab_handles(&mut self, id: &str) -> (FocusHandle, FocusHandle) {
        let set = self.set();
        self.cx.update(|_, cx| {
            let set = set.read(cx);
            let tab = set
                .tabs
                .iter()
                .find(|tab| tab.id == id)
                .unwrap_or_else(|| panic!("{id} is listed"));
            (tab.focus.clone(), tab.close_focus.clone())
        })
    }

    /// The accessible name of the open question (its title, "Close Terminal 2?" for a tab's close), and how many
    /// questions are drawn, so a test can tell one question that moved from a second one beside the first.
    fn questions(&mut self) -> (Option<String>, usize) {
        let nodes = test_support::frame(self.cx);
        let mut drawn = nodes
            .iter()
            .filter(|node| node.element_id.as_deref() == Some("prompt"));
        let first = drawn
            .next()
            .and_then(|node| node.label.as_ref().map(ToString::to_string));
        (first.clone(), drawn.count() + usize::from(first.is_some()))
    }
}

// ---- Attaching -----------------------------------------------------------------------------------------------------

/// SPEC.md, "Session view": tabs "survive client disconnects ... exactly like the agent terminal", and the old app kept
/// every tab attached, choosing one only by showing it, because attaching on each choice would pay a full replay. The
/// helm groups a view's streams by lease, so the agent terminal and its tabs must share one, or the view's own tabs
/// would take each other over. Specified: with session a listing tabs t1 and t2, the window opens three streams, for the
/// agent terminal, t1 and t2 in that order, all under one lease, the hidden tabs at the agent terminal's size; the strip
/// paints "Agent", "Terminal 1" and "Terminal 2" left to right, and the agent terminal is the one shown.
#[gpui::test]
fn every_tab_attaches_under_the_views_lease_at_the_shown_terminals_size(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1", "t2"]));
    assert_eq!(
        window.tabs_attached(),
        [None, Some("t1".to_owned()), Some("t2".to_owned())]
    );
    {
        let opened = window.opened.borrow();
        let lease = &opened[0].1.lease;
        assert!(
            opened.iter().all(|(_, attach)| &attach.lease == lease),
            "one lease"
        );
        let size = (opened[0].1.cols, opened[0].1.rows);
        assert!(
            size.0 > 0 && size.1 > 0,
            "the premise: the agent terminal was fitted"
        );
        for (_, attach) in opened.iter() {
            assert_eq!(
                (attach.cols, attach.rows),
                size,
                "{:?} at the agent terminal's size",
                attach.tab
            );
        }
    }
    assert_eq!(window.strip_labels(), ["Agent", "Terminal 1", "Terminal 2"]);
    assert_eq!(window.chosen(), None);
}

/// SPEC.md, "Session view": a shell tab "can be closed individually, which kills that shell and its processes"; the
/// agent terminal is not such a tab, and its agent stops only through the session's own Stop or Delete, each with its
/// own question. A × beside Agent would offer to kill the agent from a control that reads as closing a shell (old
/// `terminal-tabs.spec.ts`, "the strip labels tabs positionally and gives the agent terminal no close control"). And
/// a shell tab's × sits at its tab's left, inside the tab, so that none sits beside New terminal, where a press meant
/// for New terminal used to close the last shell. Specified: with no shell tabs the strip draws "Agent" and no close
/// control; once the row lists t1 and t2, it draws "Agent", "Terminal 1" and "Terminal 2" and exactly two close
/// controls, the first inside Terminal 1's tab and the second inside Terminal 2's, each in the left half of its tab,
/// none over the Agent tab, and New terminal right of both.
#[gpui::test]
fn the_agent_terminals_tab_has_no_close_control(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&[]));
    assert_eq!(window.strip_labels(), ["Agent"]);
    assert_eq!(window.strip_closes(), [], "no close control beside Agent");

    window.push_tabs(&["t1", "t2"]);
    assert_eq!(window.strip_labels(), ["Agent", "Terminal 1", "Terminal 2"]);
    let closes = window.strip_closes();
    assert_eq!(
        closes.len(),
        2,
        "one close control per shell tab: {closes:?}"
    );
    let agent = test_support::bounds(window.cx, AGENT_TAB_ID);
    let add = test_support::bounds(window.cx, super::ADD_ID);
    for (close, tab) in closes.iter().zip([T1_TAB, T2_TAB]) {
        let tab = test_support::bounds(window.cx, tab);
        assert!(
            close.left() >= tab.left()
                && close.right() <= tab.center().x
                && close.top() >= tab.top()
                && close.bottom() <= tab.bottom(),
            "a close control inside the left half of its own tab: {close:?} in {tab:?}"
        );
        assert!(close.left() >= agent.right(), "none over Agent: {close:?}");
        assert!(
            close.right() < add.left(),
            "none beside New terminal: {close:?}"
        );
    }
}

/// A shell session's main terminal runs the shell itself, and its row reads "Bash shell · No agent"; a tab called Agent
/// beside that would name something the session does not have. Specified: with the open session launched as Bash and
/// one tab listed, the strip paints "Shell" then "Terminal 1", and session b, an agent session, still paints "Agent".
#[gpui::test]
fn a_shell_sessions_main_tab_names_the_shell(cx: &mut TestAppContext) {
    let mut shell = session("a")
        .launch(json!({
            "kind": "agent",
            "selection": {"harness": "bash", "permissions": null},
            "start": ["bash", "{farhelm_args}"],
            "resume": null,
        }))
        .entry();
    shell.value["tabs"] = json!([{ "id": "t1" }]);
    let snapshot = vec![
        CollectionState {
            collection: Collection::Sessions,
            meta: listing_meta(&["a", "b"], 2, false),
            entries: vec![shell, row("b", &[])],
        },
        preferences(&["a"], None),
    ];
    let mut window = open(cx, snapshot);
    assert_eq!(window.strip_labels(), ["Shell", "Terminal 1"]);
    click(window.cx, "session-row-b");
    assert_eq!(window.strip_labels(), ["Agent"]);
}

/// Choosing a tab shows its terminal and puts focus there, so typing reaches its shell without a click on the pane
/// (the old spec's "selecting a terminal focuses it"); it attaches nothing new, since the tab was attached all along.
/// Specified: with the agent terminal and t1 live and the agent terminal focused, a click on Terminal 1 chooses t1,
/// opens no stream, and a typed key reaches t1 and not the agent; a click on Agent goes back the same way.
#[gpui::test]
fn choosing_a_tab_shows_and_focuses_its_terminal(cx: &mut TestAppContext) {
    assert_eq!(tab_element_id("t1"), T1_TAB);
    let mut window = open(cx, snapshot(&["t1"]));
    let (agent, t1) = (window.stream_of(None), window.stream_of(Some("t1")));
    window.go_live(agent, "agent");
    window.go_live(t1, "shell");
    keys(window.cx, "x");
    assert!(
        window.typed_into(agent),
        "the premise: the agent terminal has focus"
    );

    click(window.cx, T1_TAB);
    assert_eq!(window.chosen().as_deref(), Some("t1"));
    assert_eq!(
        window.opened.borrow().len(),
        2,
        "nothing attaches on a choice"
    );
    keys(window.cx, "y");
    assert!(window.typed_into(t1));
    assert!(!window.typed_into(agent));

    click(window.cx, AGENT_TAB_ID);
    assert_eq!(window.chosen(), None);
    keys(window.cx, "z");
    assert!(window.typed_into(agent));
    assert!(!window.typed_into(t1));
}

/// The shortcut follows the displayed order, including the main terminal. A confirmed close can precede its roster
/// push. The closed shell must be skipped in either direction while its button still occupies a strip position.
#[gpui::test]
fn tab_shortcuts_wrap_skip_a_confirmed_close_and_focus_the_destination(cx: &mut TestAppContext) {
    let (previous, next) = tab_chords();
    let mut window = open(cx, skipping(&["t1", "t2", "t3"]));
    let agent = window.stream_of(None);
    let t1 = window.stream_of(Some("t1"));
    let t3 = window.stream_of(Some("t3"));
    for (stream, text) in [(agent, "agent"), (t1, "first"), (t3, "last")] {
        window.go_live(stream, text);
        window.received(stream);
    }
    tab_requests(&mut window.helm);
    click(window.cx, "session-terminal-close-t2");
    let requests = tab_requests(&mut window.helm);
    assert_eq!(requests.len(), 1, "one close request");
    assert_eq!(requests[0].2, "/api/sessions/a/tabs/t2");
    window.helm.reply(requests[0].0, 200, Some(json!({})));
    settle(window.cx);
    let set = window.set();
    window.cx.update(|w, cx| {
        let set = set.read(cx);
        assert!(set.confirmed_closed.contains("t2"), "close is confirmed");
        assert_eq!(
            set.tab_ids(),
            ["t1", "t2", "t3"],
            "the roster still lists t2"
        );
        let focus = set.agent_focus.clone();
        crate::focus::focus_control(&focus, &Mark::take(cx), w, cx);
        assert!(focus.is_focused(w), "focus starts on the tab strip");
    });
    assert_eq!(window.chosen(), None);

    for (chord, shown) in [
        (previous, Some("t3")),
        (previous, Some("t1")),
        (next, Some("t3")),
        (next, None),
        (next, Some("t1")),
    ] {
        keys(window.cx, chord);
        assert_eq!(window.chosen().as_deref(), shown, "{chord}");
        for stream in [agent, t1, t3] {
            assert!(!window.typed_into(stream), "{chord} leaked to {stream}");
        }
    }
    keys(window.cx, "x");
    assert!(window.typed_into(t1), "typing follows the new tab");
    assert!(!window.typed_into(agent));
    assert!(!window.typed_into(t3));
}

/// Choosing a tab from the keyboard puts focus in its terminal. The strip has to reveal the chosen button even
/// though that button never takes focus.
#[gpui::test]
fn a_tab_shortcut_reveals_the_chosen_button(cx: &mut TestAppContext) {
    let ids: Vec<String> = (1..=20).map(|n| format!("t{n}")).collect();
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let mut window = open(cx, snapshot(&refs));
    let tab_is_visible = |window: &mut Window| {
        test_support::frame(window.cx)
            .iter()
            .find(|node| node.element_id.as_deref() == Some("session-terminal-tab-t20"))
            .is_some_and(|node| {
                node.visible_bounds == node.bounds && node.bounds.size.width > gpui::px(0.)
            })
    };
    assert!(!tab_is_visible(&mut window), "the last tab starts clipped");
    keys(window.cx, tab_chords().0);
    assert_eq!(window.chosen().as_deref(), Some("t20"));
    assert!(
        tab_is_visible(&mut window),
        "the chosen tab was scrolled into view"
    );
}

/// With only the main terminal, either bracket chord is consumed without changing focus or feeding the program.
#[gpui::test]
fn tab_shortcuts_do_nothing_with_only_the_main_terminal(cx: &mut TestAppContext) {
    let (previous, next) = tab_chords();
    let mut window = open(cx, snapshot(&[]));
    let agent = window.stream_of(None);
    window.go_live(agent, "agent");
    let set = window.set();
    window.cx.update(|w, cx| {
        let set = set.read(cx);
        assert!(
            set.tab_ids().is_empty(),
            "the premise: only the main terminal"
        );
        let focus = set.agent_focus.clone();
        crate::focus::focus_control(&focus, &Mark::take(cx), w, cx);
        assert!(focus.is_focused(w));
    });

    keys(window.cx, previous);
    keys(window.cx, next);
    assert_eq!(window.chosen(), None);
    assert!(
        window
            .cx
            .update(|w, cx| set.read(cx).agent_focus.is_focused(w))
    );
    assert!(!window.typed_into(agent));
}

// ---- Tabs that come and go -----------------------------------------------------------------------------------------

/// The tab list is the helm's: a tab opened elsewhere (by an agent, say) must appear and attach without a reload, and
/// one closed elsewhere must go, with its stream, while its siblings stay attached; the names follow the places.
/// Specified: from t1 and t2, a push of t1, t2 and t3 opens one stream, for t3, at the shared size, and the strip lists
/// three terminals; a push of t1 and t3 closes t2's stream, leaves t1's and t3's open, and the strip names t3 "Terminal
/// 2".
#[gpui::test]
fn tabs_opened_and_closed_elsewhere_come_and_go(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1", "t2"]));
    window.push_tabs(&["t1", "t2", "t3"]);
    assert_eq!(window.opened.borrow().len(), 4, "one more stream");
    let t3 = window.stream_of(Some("t3"));
    {
        let opened = window.opened.borrow();
        assert_eq!(
            (opened[t3].1.cols, opened[t3].1.rows),
            (opened[0].1.cols, opened[0].1.rows),
            "attached at once at the shared size"
        );
    }
    assert_eq!(
        window.strip_labels(),
        ["Agent", "Terminal 1", "Terminal 2", "Terminal 3"]
    );

    window.push_tabs(&["t1", "t3"]);
    let (t1, t2) = (window.stream_of(Some("t1")), window.stream_of(Some("t2")));
    assert_eq!(
        window.phase(t2),
        FakePhase::Closed,
        "the closed tab's stream ends"
    );
    assert_ne!(window.phase(t1), FakePhase::Closed);
    assert_ne!(window.phase(t3), FakePhase::Closed);
    assert_eq!(window.strip_labels(), ["Agent", "Terminal 1", "Terminal 2"]);
}

/// At dispatch, Delete, Rename and Keyboard shortcuts leave a focused terminal's marked text alone, including when
/// Delete would send immediately. Clearing the mark lets the same chord open the overview.
#[gpui::test]
fn session_shortcuts_ignore_a_focused_terminal_composition(cx: &mut TestAppContext) {
    let overview = if cfg!(target_os = "macos") {
        "cmd-/"
    } else {
        "ctrl-shift-/"
    };
    for skip in [false, true] {
        let mut app = cx.new_app();
        let mut window = open(&mut app, if skip { skipping(&[]) } else { snapshot(&[]) });
        let agent = window.stream_of(None);
        window.go_live(agent, "agent");
        let set = window.set();
        let composition = |window: &mut Window<'_>, marked: &str| {
            window.cx.update(|window, cx| {
                let agent = set.read(cx).agent().clone();
                agent.update(cx, |agent, _| {
                    assert!(agent.is_live(), "the terminal can send input");
                    assert!(
                        agent.focus_handle().is_focused(window),
                        "the terminal has focus"
                    );
                    agent.mark_text(marked);
                    assert_eq!(agent.is_composing(window), !marked.is_empty());
                });
            });
        };
        composition(&mut window, "日本");
        window.helm.commands();
        keys(window.cx, crate::actions::RENAME_SESSION_CHORD);
        keys(window.cx, crate::actions::DELETE_SESSION_CHORD);
        keys(window.cx, overview);
        assert!(!test_support::any_open(window.cx));
        window.cx.update(|window, cx| {
            assert!(set.read(cx).agent().read(cx).is_composing(window));
        });
        assert!(
            window
                .helm
                .commands()
                .iter()
                .all(|cmd| cmd.method != farhelm_helm_client::Method::Delete)
        );
        assert!(!window.typed_into(agent));
        composition(&mut window, "");
        keys(window.cx, overview);
        assert!(
            test_support::frame(window.cx)
                .iter()
                .any(|node| node.element_id.as_deref() == Some("keyboard-shortcuts")),
            "the same chord opens the overview after composition"
        );
        keys(window.cx, "escape");
        keys(window.cx, crate::actions::DELETE_SESSION_CHORD);
        assert_eq!(test_support::any_open(window.cx), !skip);
        assert_eq!(
            window
                .helm
                .commands()
                .iter()
                .any(|cmd| cmd.method == farhelm_helm_client::Method::Delete),
            skip
        );
        app.quit();
    }
}

/// A tab shortcut stays consumed while a dialog owns the keyboard or the shown terminal holds marked text.
/// Leaving those states makes the same shortcut choose and focus the shell.
#[gpui::test]
fn tab_shortcuts_leave_dialog_focus_and_terminal_composition_alone(cx: &mut TestAppContext) {
    let (previous, next) = tab_chords();
    let mut window = open(cx, snapshot(&["t1"]));
    let agent = window.stream_of(None);
    let shell = window.stream_of(Some("t1"));
    window.go_live(agent, "agent");
    window.go_live(shell, "shell");
    let set = window.set();

    keys(window.cx, crate::actions::RENAME_SESSION_CHORD);
    assert!(window.focus_inside("rename-title"), "Rename owns focus");
    // One chord at a time: with one shell tab, previous then next would come back to the main terminal anyway.
    for chord in [next, previous] {
        keys(window.cx, chord);
        assert_eq!(window.chosen(), None, "the dialog prevents switching");
    }
    assert!(window.focus_inside("rename-title"), "Rename keeps focus");
    keys(window.cx, "escape");

    let mark_text = |window: &mut Window, text: &str| {
        window.cx.update(|w, cx| {
            let terminal = set.read(cx).agent().clone();
            terminal.update(cx, |terminal, _| {
                assert!(terminal.is_live() && terminal.focus_handle().is_focused(w));
                terminal.mark_text(text);
                assert_eq!(terminal.is_composing(w), !text.is_empty());
            });
        });
    };
    mark_text(&mut window, "日本");
    for chord in [next, previous] {
        keys(window.cx, chord);
        assert_eq!(window.chosen(), None, "marked text prevents switching");
    }
    assert!(
        window
            .cx
            .update(|w, cx| set.read(cx).agent().read(cx).is_composing(w))
    );
    assert!(
        !window.typed_into(agent),
        "the chords never reached the agent"
    );
    assert!(
        !window.typed_into(shell),
        "the chords never reached the shell"
    );
    mark_text(&mut window, "");

    keys(window.cx, next);
    assert_eq!(window.chosen().as_deref(), Some("t1"));
    keys(window.cx, "x");
    assert!(window.typed_into(shell));
}

/// A reaped tab goes quietly and the preceding shell takes over typing. The
/// helm detaches the reaped tab before updating the roster. That interval
/// keeps the existing screen without drawing a Detached line.
#[gpui::test]
fn a_reaped_tab_goes_silently_and_its_predecessor_takes_focus(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1", "t2"]));
    let (agent, t1, t2) = (
        window.stream_of(None),
        window.stream_of(Some("t1")),
        window.stream_of(Some("t2")),
    );
    window.go_live(agent, "agent");
    window.go_live(t1, "first shell");
    window.go_live(t2, "second shell");
    click(window.cx, T2_TAB);
    keys(window.cx, "x");
    assert!(window.typed_into(t2), "the premise: t2 has focus");

    window.opened.borrow_mut()[t2]
        .0
        .detach(Some(DetachCode::TabClosed), "the tab was closed");
    settle(window.cx);
    let painted = painted(&test_support::frame(window.cx));
    assert!(
        !painted.iter().any(|text| text.contains("Detached")),
        "no Detached line: {painted:?}"
    );

    window.push_tabs(&["t1"]);
    assert_eq!(window.chosen().as_deref(), Some("t1"));
    assert_eq!(window.remembered().as_deref(), Some("t1"));
    assert_eq!(window.strip_labels(), ["Agent", "Terminal 1"]);
    keys(window.cx, "y");
    assert!(window.typed_into(t1), "focus is on the preceding shell");
    assert!(!window.typed_into(agent), "the agent received no input");
}

/// A reaped tab makes its predecessor home while the header question holds
/// focus. The next key belongs to the question. Dismissing the question
/// restores focus to the shell that now shows.
#[gpui::test]
fn a_reaped_tab_leaves_focus_on_an_open_header_question(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1", "t2"]));
    let (t1, t2) = (window.stream_of(Some("t1")), window.stream_of(Some("t2")));
    window.go_live(t1, "first shell");
    window.go_live(t2, "second shell");
    click(window.cx, T2_TAB);
    assert_eq!(
        window.chosen().as_deref(),
        Some("t2"),
        "the premise: the tab is chosen"
    );
    click(window.cx, "session-header-delete");
    assert_eq!(
        test_support::focused_id(window.cx).as_deref(),
        Some("prompt-confirm"),
        "the premise: the question has focus"
    );

    window.opened.borrow_mut()[t2]
        .0
        .detach(Some(DetachCode::TabClosed), "the tab was closed");
    settle(window.cx);
    window.push_tabs(&["t1"]);
    assert_eq!(window.chosen().as_deref(), Some("t1"));
    assert_eq!(
        test_support::focused_id(window.cx).as_deref(),
        Some("prompt-confirm"),
        "the question kept focus"
    );

    keys(window.cx, "escape");
    assert!(
        window.cx.debug_bounds("prompt").is_none(),
        "the question closed"
    );
    keys(window.cx, "y");
    assert!(window.typed_into(t1), "focus is on the preceding shell");
}

/// The old app's settled focus case with Rename open (old `modal-focus.spec.ts`, "rename keeps focus in its field when
/// the selected tab goes away"). A reaped tab changes the chosen terminal under the dialog without the user doing
/// anything, and that spec's comment at the reference commit records that before the old terminal's veto covered
/// every open modal dialog, "the agent terminal took focus here and the rest of the typed title went to the agent".
/// Rename is about the session, which outlives its tab, so it stays open (SPEC.md, "Opening and closing":
/// "Nothing closes a dialog ... that it did not open"), and its field keeps focus (SPEC.md, "Where focus rests": the
/// terminal "never takes focus from a control that holds it").
/// Specified: with t1 chosen and live and Rename opened from a's row menu with "draft" typed into its title field, t1's
/// stream detaching as a closed tab and a row listing no tabs choose the agent terminal and leave focus inside the
/// title field; typing "ok" and Enter then sends one rename of a titled "draftok", and the agent terminal got no key.
#[gpui::test]
fn a_reaped_tab_leaves_focus_in_an_open_rename_field(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1"]));
    let (agent, t1) = (window.stream_of(None), window.stream_of(Some("t1")));
    window.go_live(agent, "agent");
    window.go_live(t1, "shell");
    click(window.cx, T1_TAB);
    assert_eq!(
        window.chosen().as_deref(),
        Some("t1"),
        "the premise: the tab is chosen"
    );
    click(window.cx, "session-menu-a");
    click(window.cx, "session-menu-rename");
    assert!(
        window.focus_inside("rename-title"),
        "the premise: Rename's title field has focus"
    );
    crate::test_support::select_all_text(window.cx);
    keys(window.cx, "d r a f t");

    window.opened.borrow_mut()[t1]
        .0
        .detach(Some(DetachCode::TabClosed), "the tab was closed");
    settle(window.cx);
    window.push_tabs(&[]);
    assert_eq!(window.chosen(), None, "the agent terminal is chosen");
    assert!(
        window.focus_inside("rename-title"),
        "the title field kept focus"
    );

    keys(window.cx, "o k enter");
    let renames: Vec<_> = window
        .helm
        .commands()
        .into_iter()
        .filter(|command| command.path == "/api/sessions/a/rename")
        .collect();
    assert_eq!(renames.len(), 1, "one rename was sent: {renames:?}");
    assert_eq!(
        renames[0].body.as_ref().map(|body| body["title"].clone()),
        Some(json!("draftok")),
        "the typing before and after the reap both landed in the field"
    );
    assert!(!window.typed_into(agent), "the agent terminal got no key");
}

// ---- Staged replay -------------------------------------------------------------------------------------------------

/// Each tab has its own catch-up (old `terminal-replay-rename.spec.ts`, "reattach-lands-at-tail (tab)"). The tab's
/// replay must stay behind its own placeholder until the helm says it is complete: shown early it would draw half a
/// replay, and showing the agent terminal in its place would put typing meant for the shell into the agent. Specified:
/// with the agent terminal live and t1's replay delivered (a screen with "shell" on its first row and 100 history lines
/// above it) but not complete, choosing t1 shows t1's catch-up placeholder and no screen; once t1's replay completes,
/// t1 shows its screen (following the bottom, so no history line is in view), and a typed key reaches t1 and not the
/// agent terminal.
#[gpui::test]
fn a_tabs_replay_is_staged_on_its_own_and_shown_at_the_bottom(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1"]));
    let (agent, t1) = (window.stream_of(None), window.stream_of(Some("t1")));
    window.go_live(agent, "agent");
    {
        let mut opened = window.opened.borrow_mut();
        let terminal = &mut opened[t1].0;
        let asked = terminal.attach();
        let mut replay = screen(asked.cols, asked.rows, "shell");
        replay[0] = serde_json::from_value(
            json!({"t":"hello","cols":asked.cols,"rows":asked.rows,"history_cap":1000}),
        )
        .unwrap();
        let rows: Vec<_> = (0..100)
            .map(|i| {
                let text = format!("h{i}");
                json!({"w":false,"r":[[0,text.len(),text,[0,-1,-1]]]})
            })
            .collect();
        replay.push(
            serde_json::from_value(json!({"t":"history","first":0,"rows":rows,"cols":asked.cols}))
                .unwrap(),
        );
        for message in replay {
            terminal.output(message);
        }
    }
    settle(window.cx);

    click(window.cx, T1_TAB);
    assert_eq!(window.chosen().as_deref(), Some("t1"));
    let shown_text = |window: &mut Window| {
        let set = window.set();
        window.cx.update(|_, cx| {
            set.read(cx)
                .shown()
                .and_then(|terminal| terminal.read(cx).terminal_text(cx))
        })
    };
    assert!(
        window.cx.debug_bounds("terminal-connecting").is_some(),
        "t1's placeholder is drawn"
    );
    assert_eq!(shown_text(&mut window), None, "no half replay");

    window.opened.borrow_mut()[t1].0.replay_complete();
    settle(window.cx);
    assert!(window.cx.debug_bounds("terminal-connecting").is_none());
    let text = shown_text(&mut window).expect("t1's screen is shown");
    assert_eq!(text.lines().next(), Some("shell"));
    assert!(
        !text.lines().any(|line| line.starts_with('h')),
        "following the bottom: {text:?}"
    );
    keys(window.cx, "x");
    assert!(window.typed_into(t1));
    assert!(!window.typed_into(agent));
}

// ---- A stopped stream -----------------------------------------------------------------------------------------------

/// One UI holds the helm at a time, so no other window can hold this view's terminals, and a terminal whose stream
/// stopped waits alone, for its own Reconnect (decided by the migration's orchestrator on 2026-11-15). The set once
/// kept new tabs from attaching, and reconnected every waiting terminal together, while one of them waited to take the
/// session back; this is the fence against either coming back for a stream that merely stopped. Specified: with the
/// agent terminal, t1 and t3 live and all three detached as stalled, a push adding t2 opens t2's stream; a push without
/// t1 sends the agent terminal's stream nothing, and the agent terminal still offers Reconnect; Reconnect there asks
/// the agent terminal's stream to attach again, and neither t3's, which waits too, nor t2's.
#[gpui::test]
fn a_stalled_terminal_holds_back_no_other(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1", "t3"]));
    let (agent, t1) = (window.stream_of(None), window.stream_of(Some("t1")));
    let t3 = window.stream_of(Some("t3"));
    window.go_live(agent, "agent");
    window.go_live(t1, "shell");
    window.go_live(t3, "other shell");
    for index in [agent, t1, t3] {
        window.opened.borrow_mut()[index].0.detach(
            Some(DetachCode::Stalled),
            farhelm_proto::DETACH_REASON_STALLED,
        );
    }
    settle(window.cx);
    let set = window.set();
    let offers_reconnect = |window: &mut Window| {
        window.cx.update(|_, cx| {
            set.read(cx)
                .agent()
                .read(cx)
                .status_line()
                .is_some_and(|line| line.offers_reconnect)
        })
    };
    assert!(
        offers_reconnect(&mut window),
        "the premise: the agent terminal waits"
    );

    window.push_tabs(&["t1", "t3", "t2"]);
    assert_eq!(
        window.tabs_attached().last().cloned().flatten().as_deref(),
        Some("t2"),
        "t2 attaches"
    );
    let t2 = window.stream_of(Some("t2"));
    window.received(agent);
    window.push_tabs(&["t3", "t2"]);
    assert_eq!(window.phase(t1), FakePhase::Closed, "the premise: t1 went");
    assert_eq!(
        window.received(agent),
        [],
        "a tab leaving reconnects nothing"
    );
    assert!(offers_reconnect(&mut window));

    assert_eq!(
        window.phase(t3),
        FakePhase::Detached,
        "the premise: t3 waits too"
    );
    window.received(t2);
    window.received(t3);
    click(window.cx, STATUS_RECONNECT_ID);
    assert!(
        window
            .received(agent)
            .contains(&FakeTerminalRequest::TakeControl)
    );
    settle(window.cx);
    assert!(
        !window
            .received(t3)
            .contains(&FakeTerminalRequest::TakeControl),
        "the waiting tab is not reconnected with the agent terminal"
    );
    assert!(
        !window
            .received(t2)
            .contains(&FakeTerminalRequest::TakeControl)
    );
}

// ---- The bound -----------------------------------------------------------------------------------------------------

/// The window builds terminals from a tab list it did not write (a supervisor reached over SSH, say), so it attaches at
/// most 32 tabs, as the old app did, while still listing every one so none is hidden from the user. Specified: with 35
/// tabs listed (t1 to t35), 33 streams open (the agent terminal and t1 to t32), the strip draws a tab for the agent
/// terminal and each of the 35. The shortcuts wrap through the attached tabs, where there is a terminal to focus.
/// Choosing t35 still shows the not-attached notice, and reopening shows the agent terminal.
#[gpui::test]
fn tabs_past_the_bound_are_listed_but_not_attached(cx: &mut TestAppContext) {
    let ids: Vec<String> = (1..=35).map(|n| format!("t{n}")).collect();
    let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
    let mut window = open(cx, snapshot(&ids));
    assert_eq!(window.opened.borrow().len(), MAX_ATTACHED_TABS + 1);
    let set = window.set();
    let listed = window.cx.update(|_, cx| set.read(cx).tab_ids().len());
    assert_eq!(listed, 35);
    assert_eq!(window.strip_labels().len(), 36, "the strip lists every tab");
    let (previous, next) = tab_chords();
    keys(window.cx, previous);
    assert_eq!(window.chosen().as_deref(), Some("t32"));
    assert!(test_support::terminal_focused(window.cx));
    keys(window.cx, next);
    assert_eq!(window.chosen(), None);

    // Chosen through the set rather than a click: t35's tab is scrolled out of the strip's view.
    window.cx.update(|w, cx| {
        set.update(cx, |set, cx| {
            set.choose(Some("t35".to_owned()), &Mark::take(cx), w, cx)
        })
    });
    settle(window.cx);
    assert!(window.cx.debug_bounds(NOT_ATTACHED_ID).is_some());
    keys(window.cx, previous);
    keys(window.cx, next);
    assert_eq!(window.chosen().as_deref(), Some("t35"));
    assert!(window.cx.debug_bounds(NOT_ATTACHED_ID).is_some());
    drop(set);

    click(window.cx, "session-row-b");
    click(window.cx, "session-row-a");
    assert_eq!(window.chosen(), None);
    assert!(test_support::terminal_focused(window.cx));
}

/// The bound counts places, not tabs: when an attached tab goes, the first one past the bound moves into its place and
/// must attach then, or it would stay unattached for as long as the session lives. Specified: with 33 tabs listed (32
/// attached), a push without t1 opens one stream, for t33.
#[gpui::test]
fn a_tab_moving_under_the_bound_attaches(cx: &mut TestAppContext) {
    let ids: Vec<String> = (1..=33).map(|n| format!("t{n}")).collect();
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let mut window = open(cx, snapshot(&refs));
    assert_eq!(
        window.opened.borrow().len(),
        MAX_ATTACHED_TABS + 1,
        "the premise"
    );
    window.push_tabs(&refs[1..]);
    assert_eq!(window.opened.borrow().len(), MAX_ATTACHED_TABS + 2);
    assert_eq!(
        window.tabs_attached().last().cloned().flatten().as_deref(),
        Some("t33")
    );
}

// ---- Size, switching and the inert state -----------------------------------------------------------------------

/// SPEC.md, "Terminal experience": a text size step reaches "every open terminal's text size together, hidden tabs
/// included", and each is resized; the window's own resize reaches hidden tabs the same way (the old spec's "a viewport
/// resize reflows every island, hidden ones included"). A hidden tab left at the old size would show a mangled screen
/// when chosen. Specified: with the agent terminal and t1 live and the agent terminal shown, a text size step up sends
/// t1's stream exactly one resize, to the agent terminal's new size.
#[gpui::test]
fn a_hidden_tab_is_resized_with_the_shown_terminal(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1"]));
    let (agent, t1) = (window.stream_of(None), window.stream_of(Some("t1")));
    window.go_live(agent, "agent");
    window.go_live(t1, "shell");
    assert_eq!(
        window.received(t1),
        [],
        "the premise: t1 is live at its size"
    );
    window.received(agent);

    window
        .cx
        .update(|_, cx| crate::terminal::text_size::step(1, cx));
    settle(window.cx);
    let agent_resize: Vec<_> = window
        .received(agent)
        .into_iter()
        .filter(|request| matches!(request, FakeTerminalRequest::Resize { .. }))
        .collect();
    assert_eq!(
        agent_resize.len(),
        1,
        "the premise: the shown terminal resized"
    );
    assert_eq!(
        window.received(t1),
        agent_resize,
        "the hidden tab follows it"
    );
}

/// Choosing another session ends the open session's terminals, tabs included (the old spec's "leaving a session with
/// tabs tears down every island"): a tab left attached would keep a stream, and a viewer, for a session no longer
/// shown. Specified: with a's agent terminal and t1 attached, selecting b closes both streams and opens b's agent
/// terminal alone.
#[gpui::test]
fn choosing_another_session_ends_every_terminal_of_the_open_one(cx: &mut TestAppContext) {
    let window = open(cx, snapshot(&["t1"]));
    let (agent, t1) = (window.stream_of(None), window.stream_of(Some("t1")));
    click(window.cx, "session-row-b");
    assert_eq!(window.phase(agent), FakePhase::Closed);
    assert_eq!(window.phase(t1), FakePhase::Closed);
    let opened = window.opened.borrow();
    assert_eq!(opened.len(), 3);
    assert_eq!(
        (opened[2].1.session.as_str(), opened[2].1.tab.as_deref()),
        ("b", None)
    );
}

/// The remembered tab should get the user's next keystroke after the pane opens again, including when the agent
/// is waiting. Re-selecting that session keeps the tab shown, and explicitly choosing Agent clears the memory.
#[gpui::test]
fn reopening_a_session_focuses_the_remembered_tab_until_agent_is_chosen(cx: &mut TestAppContext) {
    let mut state = snapshot(&["t1"]);
    state[0].entries[0].value["status"] = json!({"state": "waiting"});
    let mut window = open(cx, state);
    click(window.cx, T1_TAB);
    assert_eq!(window.chosen().as_deref(), Some("t1"));

    click(window.cx, "session-row-b");
    let first_new_stream = window.opened.borrow().len();
    click(window.cx, "session-row-a");
    let (agent, tab) = {
        let opened = window.opened.borrow();
        let find = |tab: Option<&str>| {
            opened[first_new_stream..]
                .iter()
                .position(|(_, attach)| attach.session == "a" && attach.tab.as_deref() == tab)
                .expect("the reopened session attaches both terminals")
                + first_new_stream
        };
        (find(None), find(Some("t1")))
    };
    window.go_live(agent, "agent");
    window.go_live(tab, "shell");
    assert_eq!(window.chosen().as_deref(), Some("t1"));
    assert!(test_support::terminal_focused(window.cx));
    keys(window.cx, "x");
    assert!(window.typed_into(tab));
    assert!(!window.typed_into(agent));

    click(window.cx, "session-row-a");
    keys(window.cx, "y");
    assert!(window.typed_into(tab));
    assert!(!window.typed_into(agent));

    click(window.cx, AGENT_TAB_ID);
    assert_eq!(window.chosen(), None, "Agent is an explicit choice");
    click(window.cx, "session-row-b");
    click(window.cx, "session-row-a");
    assert_eq!(window.chosen(), None);
    assert!(test_support::terminal_focused(window.cx));
}

/// A short tab list can hide an open tab during remount. The pane shows Agent, and a later visit that can attach the
/// originally chosen tab still restores it.
#[gpui::test]
fn a_missing_tab_falls_back_to_agent_without_erasing_the_choice(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1"]));
    click(window.cx, T1_TAB);
    assert_eq!(window.chosen().as_deref(), Some("t1"));
    click(window.cx, "session-row-b");
    window.push_tabs(&[]);

    click(window.cx, "session-row-a");
    assert_eq!(window.chosen(), None);
    assert!(test_support::terminal_focused(window.cx));
    click(window.cx, "session-row-b");
    window.push_tabs(&["t1"]);
    click(window.cx, "session-row-a");
    assert_eq!(window.chosen().as_deref(), Some("t1"));
}

/// A temporary disconnect while restoring a remembered tab must leave that tab selected and trying. If the next
/// attach gets a definitive refusal, the main terminal takes focus. A deliberate selection still explains the
/// tab's refusal and gives the user a Reconnect button.
#[gpui::test]
fn a_remembered_shell_tab_falls_back_after_a_retry_is_refused(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1"]));
    click(window.cx, T1_TAB);
    assert_eq!(window.chosen().as_deref(), Some("t1"));
    click(window.cx, "session-row-b");
    let first = window.opened.borrow().len();
    click(window.cx, "session-row-a");
    let main = window.reopened_stream(first, None);
    let shell = window.reopened_stream(first, Some("t1"));
    window.go_live(main, "main terminal");
    assert_eq!(window.chosen().as_deref(), Some("t1"));

    window.opened.borrow_mut()[shell].0.drop_link();
    settle(window.cx);
    assert_eq!(window.phase(shell), FakePhase::Reconnecting);
    assert_eq!(window.chosen().as_deref(), Some("t1"));

    let reason = "The shell tab's terminal cannot be read";
    {
        let mut opened = window.opened.borrow_mut();
        let terminal = &mut opened[shell].0;
        terminal.attach();
        terminal.refuse_tab_attach(reason);
    }
    settle(window.cx);
    assert_eq!(window.chosen(), None, "the main terminal is shown");
    assert_eq!(window.phase(shell), FakePhase::Detached);
    assert!(
        window.cx.debug_bounds(T1_TAB).is_some(),
        "the shell remains listed"
    );
    keys(window.cx, "x");
    assert!(window.typed_into(main), "focus follows the fallback");
    assert!(!window.typed_into(shell));
    assert_eq!(
        window
            .cx
            .update(|_, cx| cx.global::<super::ShownTerminals>().0.get("a").cloned())
            .as_deref(),
        Some("t1"),
        "automatic fallback preserves the user's remembered tab"
    );

    click(window.cx, T1_TAB);
    assert_eq!(
        window.chosen().as_deref(),
        Some("t1"),
        "deliberate choice remains shown"
    );
    assert_eq!(window.phase(shell), FakePhase::Detached);
    let text = painted(&test_support::frame(window.cx)).join(" ");
    assert!(
        text.contains("Detached:") && text.contains(reason),
        "{text}"
    );
    click(window.cx, STATUS_RECONNECT_ID);
    assert_eq!(window.phase(shell), FakePhase::Attaching);
}

/// Leave a sent upload waiting for an answer in the first shell tab. Its cancellation can then be distinguished
/// from a queued upload that never reached the fake.
fn pending_upload_in_first_tab(
    window: &mut Window<'_>,
    transfers: &mut FakeTransfers,
) -> FakeAnswer<String> {
    click(window.cx, T1_TAB);
    assert_eq!(window.chosen().as_deref(), Some("t1"));
    let tab = window.stream_of(Some("t1"));
    window.go_live(tab, "shell");
    {
        let set = window.set();
        window.cx.update(|_, cx| {
            let owner = set.read(cx).shown().expect("the shell is shown").read(cx);
            assert!(owner.is_live(), "the shell can take an upload");
            owner.attachments().clone().update(cx, |attachments, cx| {
                attachments.attach(vec![Offered::File(PathBuf::from("Cargo.toml"))], cx);
            });
        });
    }
    settle(window.cx);
    let requests = transfers.take();
    let [FakeTransfer::Upload { answer, .. }]: [FakeTransfer; 1] =
        requests.try_into().expect("one pending request")
    else {
        panic!("the pending request was an upload");
    };
    assert!(!answer.abandoned(), "the upload is still in flight");
    answer
}

/// A connection reset keeps the selected session. Return its new two streams while the remembered tab is still
/// attaching, with the retired tab's upload waiting to be reported.
fn reset_with_interrupted_tab_upload(
    window: &mut Window<'_>,
    transfers: &mut FakeTransfers,
) -> (usize, usize) {
    let answer = pending_upload_in_first_tab(window, transfers);
    window.helm.break_channel();
    settle(window.cx);
    assert!(answer.abandoned(), "the old tab's upload was interrupted");
    let first = window.opened.borrow().len();
    window.helm.connect(snapshot(&["t1"]));
    settle(window.cx);
    let main = window.reopened_stream(first, None);
    let shell = window.reopened_stream(first, Some("t1"));
    assert_eq!(window.chosen().as_deref(), Some("t1"));
    assert_eq!(window.phase(shell), FakePhase::Attaching);
    assert!(window.upload_lines(None).is_empty());
    assert!(window.upload_lines(Some("t1")).is_empty());
    (main, shell)
}

/// Even if the roster still lists a tab that has closed, its initial detach settles the restore. The main
/// terminal receives the pending notice once, and a deliberate visit to the stale tab cannot consume it.
#[gpui::test]
fn a_closed_remembered_tab_routes_the_upload_notice_to_main(cx: &mut TestAppContext) {
    let (mut transfers, handle) = FakeTransfers::new();
    let mut window = open_with_transfers(cx, snapshot(&["t1"]), Some(handle));
    let (main, shell) = reset_with_interrupted_tab_upload(&mut window, &mut transfers);
    window.go_live(main, "main terminal");
    {
        let mut opened = window.opened.borrow_mut();
        let terminal = &mut opened[shell].0;
        terminal.attach();
        terminal.detach(Some(DetachCode::TabClosed), "the shell has closed");
    }
    settle(window.cx);
    assert_eq!(window.chosen(), None);
    let lines = window.upload_lines(None);
    assert_eq!(lines.len(), 1);
    assert!(
        lines[0].contains("Attaching Cargo.toml was interrupted"),
        "{lines:?}"
    );
    assert!(window.upload_lines(Some("t1")).is_empty());
    assert_eq!(
        window.cx.update(|_, cx| notices::posted(cx)),
        [Notice::ConnectionLost]
    );
}

/// Close can settle before the roster drops the tab or the in-flight attach answers. Its success must release
/// the carried notice to the main terminal immediately, because the tab may never report another attach state.
#[gpui::test]
fn closing_a_restoring_tab_releases_the_notice_before_the_roster_changes(cx: &mut TestAppContext) {
    let (mut transfers, handle) = FakeTransfers::new();
    let mut window = open_with_transfers(cx, snapshot(&["t1"]), Some(handle));
    let (_, shell) = reset_with_interrupted_tab_upload(&mut window, &mut transfers);
    click(window.cx, "session-terminal-close-t1");
    click(window.cx, "prompt-confirm");
    let sent = tab_requests(&mut window.helm);
    assert_eq!(sent.len(), 1, "one tab close: {sent:?}");
    assert_eq!(
        (sent[0].1, sent[0].2.as_str()),
        (
            farhelm_helm_client::Method::Delete,
            "/api/sessions/a/tabs/t1"
        )
    );
    window.helm.reply(sent[0].0, 200, None);
    settle(window.cx);

    assert_eq!(window.chosen(), None);
    assert!(
        window.cx.debug_bounds(T1_TAB).is_some(),
        "the roster has not changed"
    );
    assert_eq!(
        window.phase(shell),
        FakePhase::Attaching,
        "no initial attach result"
    );
    let lines = window.upload_lines(None);
    assert_eq!(lines.len(), 1);
    assert!(
        lines[0].contains("Attaching Cargo.toml was interrupted"),
        "{lines:?}"
    );
    window.push_tabs(&[]);
    assert_eq!(window.upload_lines(None), lines);
}

/// A single roster push can remove the remembered shell and list the shell the user just opened. Route the
/// carried notice after honoring that choice, so it reaches the new shell instead of a hidden main terminal.
#[gpui::test]
fn replacing_the_restoring_tab_in_one_push_routes_the_notice_to_the_new_shell(
    cx: &mut TestAppContext,
) {
    let (mut transfers, handle) = FakeTransfers::new();
    let mut window = open_with_transfers(cx, snapshot(&["t1"]), Some(handle));
    reset_with_interrupted_tab_upload(&mut window, &mut transfers);
    click(window.cx, super::ADD_ID);
    let sent = tab_requests(&mut window.helm);
    assert_eq!(sent.len(), 1, "one tab open: {sent:?}");
    assert_eq!(
        (sent[0].1, sent[0].2.as_str()),
        (farhelm_helm_client::Method::Post, "/api/sessions/a/tabs")
    );
    window
        .helm
        .reply(sent[0].0, 200, Some(json!({ "tab": { "id": "t2" } })));
    settle(window.cx);
    assert_eq!(window.chosen().as_deref(), Some("t1"));
    assert!(window.upload_lines(None).is_empty());

    window.push_tabs(&["t2"]);
    assert_eq!(window.chosen().as_deref(), Some("t2"));
    let lines = window.upload_lines(Some("t2"));
    assert_eq!(lines.len(), 1);
    assert!(
        lines[0].contains("Attaching Cargo.toml was interrupted"),
        "{lines:?}"
    );
    assert!(window.upload_lines(None).is_empty());
}

/// If the remembered tab's attachment succeeds, the upload notice goes to that tab even if the main terminal
/// connected first.
#[gpui::test]
fn a_restored_shell_tab_receives_its_interrupted_upload_notice(cx: &mut TestAppContext) {
    let (mut transfers, handle) = FakeTransfers::new();
    let mut window = open_with_transfers(cx, snapshot(&["t1"]), Some(handle));
    let (main, shell) = reset_with_interrupted_tab_upload(&mut window, &mut transfers);
    window.go_live(main, "main terminal");
    assert!(window.upload_lines(None).is_empty());
    assert!(window.upload_lines(Some("t1")).is_empty());
    window.go_live(shell, "shell terminal");
    assert_eq!(window.chosen().as_deref(), Some("t1"));
    let lines = window.upload_lines(Some("t1"));
    assert_eq!(lines.len(), 1);
    assert!(
        lines[0].contains("Attaching Cargo.toml was interrupted"),
        "{lines:?}"
    );
    assert!(window.upload_lines(None).is_empty());
}

/// Leaving while the restored tab is still trying moves the pending upload notice to the main window, where it
/// remains visible without a terminal of the source session mounted.
#[gpui::test]
fn leaving_during_tab_restore_posts_the_pending_upload_notice(cx: &mut TestAppContext) {
    let (mut transfers, handle) = FakeTransfers::new();
    let mut window = open_with_transfers(cx, snapshot(&["t1"]), Some(handle));
    let (_, shell) = reset_with_interrupted_tab_upload(&mut window, &mut transfers);
    window.opened.borrow_mut()[shell].0.drop_link();
    settle(window.cx);
    assert_eq!(window.phase(shell), FakePhase::Reconnecting);
    assert_eq!(window.chosen().as_deref(), Some("t1"));
    click(window.cx, "session-row-b");
    let posted = window.cx.update(|_, cx| notices::posted(cx));
    assert!(
        matches!(
            posted.as_slice(),
            [Notice::ConnectionLost, Notice::InterruptedUpload(_)]
        ),
        "{posted:?}"
    );
    assert_eq!(
        posted[1].text().as_ref(),
        "Attaching Cargo.toml to a was interrupted — whether it was attached cannot be told."
    );
}

/// A recovery card can replace the shell while its session stays selected. The pending upload then needs a window
/// banner because no terminal will show it.
#[gpui::test]
fn a_recovery_card_reports_the_selected_sessions_interrupted_upload(cx: &mut TestAppContext) {
    let (mut transfers, handle) = FakeTransfers::new();
    let mut window = open_with_transfers(cx, snapshot(&["t1"]), Some(handle));
    let answer = pending_upload_in_first_tab(&mut window, &mut transfers);
    let mut interrupted = row("a", &["t1"]);
    interrupted.value["status"] = json!({"state": "interrupted"});
    window.helm.push(CollectionChanges {
        collection: Collection::Sessions,
        meta: listing_meta(&["a", "b"], 2, false),
        upsert: vec![interrupted],
        remove: Vec::new(),
    });
    settle(window.cx);
    assert_eq!(
        window
            .cx
            .update(|_, cx| crate::local_state::selected(cx))
            .as_deref(),
        Some("a")
    );
    assert!(answer.abandoned());
    let posted = window.cx.update(|_, cx| notices::posted(cx));
    let [Notice::InterruptedUpload(upload)] = posted.as_slice() else {
        panic!("expected an interrupted upload above the recovery card: {posted:?}");
    };
    let dismiss = format!("notice-dismiss-interrupted-upload-{}", upload.key());
    assert_eq!(
        test_support::label(window.cx, &dismiss).as_deref(),
        Some("Dismiss")
    );
    assert!(test_support::drawn(window.cx, "session-interrupted-card"));
}

/// Leaving a session reports its shell's interrupted upload in a banner right away. Restoring the shell
/// keeps the banner and has no second interruption to show in the strip.
#[gpui::test]
fn an_interrupted_upload_is_reported_once_across_a_tab_restore(cx: &mut TestAppContext) {
    let (mut transfers, handle) = FakeTransfers::new();
    let mut window = open_with_transfers(cx, snapshot(&["t1"]), Some(handle));
    let answer = pending_upload_in_first_tab(&mut window, &mut transfers);
    click(window.cx, "session-row-b");
    assert!(
        answer.abandoned(),
        "leaving the session stopped waiting for it"
    );
    let posted = window.cx.update(|_, cx| notices::posted(cx));
    assert!(matches!(posted.as_slice(), [Notice::InterruptedUpload(_)]));
    assert_eq!(
        posted[0].text().as_ref(),
        "Attaching Cargo.toml to a was interrupted — whether it was attached cannot be told."
    );
    click(window.cx, "session-row-a");
    assert_eq!(window.chosen().as_deref(), Some("t1"));
    assert!(
        test_support::frame(window.cx)
            .iter()
            .all(|node| { node.element_id.as_deref() != Some(attachments::STRIP_ID) })
    );
    assert_eq!(window.cx.update(|_, cx| notices::posted(cx)), posted);
}

/// A closed shell's interruption has to wait while the chosen tab is beyond the attach limit. A hidden Agent
/// strip cannot report it. Leaving then shows the banner, which stays visible when a later Delete removes the
/// source under a filter without the user reopening it.
#[gpui::test]
fn leaving_an_unattached_tab_reports_an_upload_even_after_a_filtered_delete(
    cx: &mut TestAppContext,
) {
    let ids: Vec<String> = (1..=MAX_ATTACHED_TABS + 2)
        .map(|n| format!("t{n}"))
        .collect();
    let tabs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let (mut transfers, handle) = FakeTransfers::new();
    let mut window = open_with_transfers(cx, snapshot(&tabs), Some(handle));
    let answer = pending_upload_in_first_tab(&mut window, &mut transfers);
    let set = window.set();
    let last = tabs.last().unwrap().to_string();
    window.cx.update(|w, cx| {
        set.update(cx, |set, cx| set.choose(Some(last), &Mark::take(cx), w, cx));
    });
    drop(set);
    settle(window.cx);
    assert!(
        window.cx.debug_bounds(NOT_ATTACHED_ID).is_some(),
        "the chosen tab has no terminal"
    );
    window.push_tabs(&tabs[1..]);
    assert!(
        window.cx.debug_bounds(NOT_ATTACHED_ID).is_some(),
        "the chosen tab remains beyond the attach limit"
    );
    assert!(
        answer.abandoned(),
        "closing the uploading tab interrupted it"
    );
    assert!(
        window.cx.update(|_, cx| notices::posted(cx)).is_empty(),
        "the source session is still open"
    );

    window.helm.push(CollectionChanges {
        collection: Collection::Sessions,
        meta: filtered(&["a", "b"], 3, 2),
        upsert: Vec::new(),
        remove: Vec::new(),
    });
    settle(window.cx);
    click(window.cx, "session-row-b");
    let posted = window.cx.update(|_, cx| notices::posted(cx));
    let [Notice::InterruptedUpload(upload)] = posted.as_slice() else {
        panic!("expected an upload notice on departure, got {posted:?}");
    };
    let wanted = format!("notice-dismiss-interrupted-upload-{}", upload.key());
    let banner_is_drawn = |cx: &mut VisualTestContext| {
        test_support::frame(cx)
            .iter()
            .any(|node| node.element_id.as_deref() == Some(wanted.as_str()))
    };
    assert!(banner_is_drawn(window.cx));

    answer_everything(&mut window.helm);
    click(window.cx, "session-menu-a");
    click(window.cx, "session-menu-delete");
    click(window.cx, "prompt-confirm");
    let (id, _) = test_support::one_command(
        &mut window.helm,
        farhelm_helm_client::Method::Delete,
        "/api/sessions/a",
    );
    window.helm.push(CollectionChanges {
        collection: Collection::Sessions,
        meta: filtered(&["b"], 2, 1),
        upsert: Vec::new(),
        remove: vec!["a".into()],
    });
    window.helm.reply(id, 200, None);
    settle(window.cx);
    assert_eq!(
        window
            .cx
            .update(|_, cx| crate::local_state::selected(cx))
            .as_deref(),
        Some("b")
    );
    assert_eq!(window.cx.update(|_, cx| notices::posted(cx)), posted);
    assert!(banner_is_drawn(window.cx));
}

/// Old `terminal-reconnect.spec.ts`, "leaving-a-session-cancels-a-pending-reconnect". Why: a wait to reattach that
/// outlived the view would attach later for a session no longer shown, under a lease nothing on screen uses, and every
/// attach takes the terminal from whatever holds it. Two waits can be running when the user leaves: a stream's own
/// reconnect wait, which ends with its stream (helm-client
/// `letting_go_of_a_stream_during_its_reconnect_wait_cancels_the_reconnect`), and the wait before a stream closed over
/// a protocol fault is opened again, which is the terminal's own timer and must end with the terminal.
///
/// Spec: with session a's agent terminal and its tab t1 live, t1's link drops, so its stream waits to reconnect, and
/// the agent terminal's stream breaks the grid format, so it is closed and its restart waits; choosing b closes t1's
/// stream and opens one for b's agent terminal; a minute later, past every wait, no other stream has been opened.
#[gpui::test]
fn leaving_a_session_cancels_its_terminals_pending_reconnects(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1"]));
    let (agent, t1) = (window.stream_of(None), window.stream_of(Some("t1")));
    window.go_live(agent, "agent");
    window.go_live(t1, "shell");
    window.opened.borrow_mut()[t1].0.drop_link();
    // A second hello on one attachment, which the grid model refuses.
    let hello: GridOutput =
        serde_json::from_value(json!({"t":"hello","cols":80,"rows":24,"history_cap":0})).unwrap();
    window.opened.borrow_mut()[agent].0.output(hello);
    settle(window.cx);
    assert_eq!(
        window.phase(t1),
        FakePhase::Reconnecting,
        "the premise: t1's stream waits"
    );
    assert_eq!(
        window.phase(agent),
        FakePhase::Closed,
        "the premise: the faulted stream is closed"
    );
    {
        // Scoped: a handle kept on the set would keep its terminals alive past the switch.
        let set = window.set();
        assert!(
            window
                .cx
                .update(|_, cx| set.read(cx).agent().read(cx).reconnecting()),
            "the premise: the agent terminal's restart waits"
        );
    }

    click(window.cx, "session-row-b");
    assert_eq!(window.phase(t1), FakePhase::Closed);
    test_support::advance(window.cx, std::time::Duration::from_secs(60));
    let opened: Vec<(String, Option<String>)> = window
        .opened
        .borrow()
        .iter()
        .map(|(_, attach)| (attach.session.clone(), attach.tab.clone()))
        .collect();
    assert_eq!(
        opened,
        [
            ("a".to_owned(), None),
            ("a".to_owned(), Some("t1".to_owned())),
            ("b".to_owned(), None)
        ]
    );
}

/// SPEC.md, "Session list": a terminal of an unreachable host "keeps its last screen and tab strip dimmed and inert
/// beneath the card", and the card covers whichever terminal is shown, so every terminal goes inert, hidden tabs
/// included, and the strip takes no presses (choosing a tab would show a terminal the card does not explain).
/// Specified: once the set is made inert, every terminal reports inert, and neither a click on Terminal 1 nor a tab
/// shortcut chooses anything; made usable again, the click chooses t1, and from the main terminal a single next-tab
/// press chooses t1, so the inert assertions above would fail if the set stopped ignoring the shortcut.
#[gpui::test]
fn an_inert_set_makes_every_terminal_and_the_strip_inert(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1"]));
    let set = window.set();
    window
        .cx
        .update(|_, cx| set.update(cx, |set, cx| set.set_inert(true, cx)));
    settle(window.cx);
    let all_inert = window.cx.update(|_, cx| {
        set.read(cx)
            .terminals()
            .all(|terminal| terminal.read(cx).is_inert())
    });
    assert!(all_inert);
    click(window.cx, T1_TAB);
    assert_eq!(window.chosen(), None, "the strip takes no presses");
    let (previous, next) = tab_chords();
    for chord in [next, previous] {
        keys(window.cx, chord);
        assert_eq!(
            window.chosen(),
            None,
            "the shortcuts leave the inert set alone"
        );
    }

    window
        .cx
        .update(|_, cx| set.update(cx, |set, cx| set.set_inert(false, cx)));
    settle(window.cx);
    click(window.cx, T1_TAB);
    assert_eq!(window.chosen().as_deref(), Some("t1"));
    keys(window.cx, next);
    assert_eq!(
        window.chosen(),
        None,
        "next wraps around to the main terminal"
    );
    keys(window.cx, next);
    assert_eq!(
        window.chosen().as_deref(),
        Some("t1"),
        "one press from the main terminal chooses t1"
    );
}

/// SPEC.md, "Session view": the strip "scrolls horizontally when needed", and every control stays keyboard reachable.
/// A tab that Tab reaches while scrolled out of view would hold focus where the user cannot see it, so the strip scrolls
/// it into view, as a browser does for a focused element, once: a strip that scrolled back to the focused tab every
/// frame would undo every wheel turn while a tab holds focus. Specified: with 35 tabs, Terminal 35's button is clipped
/// by the strip; once keyboard focus moves to it, it is drawn whole; a wheel turn over the strip then scrolls it out of
/// view again, and it stays out over the next frames.
#[gpui::test]
fn a_tab_keyboard_focus_reaches_is_scrolled_into_view(cx: &mut TestAppContext) {
    let ids: Vec<String> = (1..=35).map(|n| format!("t{n}")).collect();
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let mut window = open(cx, snapshot(&refs));
    let whole = |window: &mut Window| {
        test_support::frame(window.cx)
            .iter()
            .find(|node| node.element_id.as_deref() == Some("session-terminal-tab-t35"))
            .is_some_and(|node| {
                node.visible_bounds == node.bounds && node.bounds.size.width > gpui::px(0.)
            })
    };
    assert!(
        !whole(&mut window),
        "the premise: Terminal 35 is out of view"
    );
    let set = window.set();
    let handle = window
        .cx
        .update(|_, cx| set.read(cx).tab_focus("t35"))
        .unwrap();
    // Through the focus coordinator, as the keyboard would reach it.
    window
        .cx
        .update(|w, cx| crate::focus::focus_control(&handle, &Mark::take(cx), w, cx));
    // One frame sees the focus and asks the strip to scroll, the next lays the strip out scrolled.
    test_support::redraw(window.cx);
    test_support::redraw(window.cx);
    assert!(whole(&mut window), "focus scrolled it into view");

    let strip = window
        .cx
        .debug_bounds(super::STRIP_ID)
        .expect("the strip is drawn");
    window.cx.simulate_event(gpui::ScrollEvent {
        position: strip.center(),
        delta: gpui::ScrollDelta::Pixels(gpui::point(gpui::px(2000.), gpui::px(0.))),
        ..Default::default()
    });
    test_support::redraw(window.cx);
    test_support::redraw(window.cx);
    assert!(
        !whole(&mut window),
        "the wheel moved the strip, and focus did not pull it back"
    );
    assert!(
        window.cx.update(|w, _| handle.is_focused(w)),
        "the premise: Terminal 35 still holds focus"
    );
}

/// Tunnels belong to the session (SPEC.md: the top row's pill lists the session's tunnels), so a tunnel a tab opens
/// must show in the pill, which reads the agent terminal's list, and a remote session's tabs must not each read the
/// list again. Specified: every tab's terminal uses the agent terminal's tunnel list.
#[gpui::test]
fn the_tabs_share_the_agent_terminals_tunnels(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1", "t2"]));
    let set = window.set();
    let shared = window.cx.update(|_, cx| {
        let set = set.read(cx);
        let agent = set.agent().read(cx).tunnels().entity_id();
        set.terminals()
            .all(|terminal| terminal.read(cx).tunnels().entity_id() == agent)
    });
    assert!(shared);
}

/// The other half of sharing the tunnel list: a remote session's terminals all hear the same pushed change and all find
/// the list due, and the helm should hear one read, not one per tab (up to 33 for a session at the bound). Specified:
/// with session a on the pinned host list's SSH host 2 and two tabs, opening it sends exactly one read of a's tunnels.
#[gpui::test]
fn a_remote_sessions_terminals_read_their_tunnels_once(cx: &mut TestAppContext) {
    let hosts: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../farhelm-helm/http-contract/host-list.json"
    ))
    .unwrap();
    let entries: Vec<KeyedEntry> = hosts["hosts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|host| KeyedEntry {
            key: farhelm_proto::ui_channel::HostView::key(host["id"].as_i64().unwrap()),
            value: host.clone(),
        })
        .collect();
    let keys: Vec<String> = entries.iter().map(|entry| entry.key.clone()).collect();
    let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
    let mut snapshot = snapshot(&["t1", "t2"]);
    let mut a = session("a").host(2).entry();
    a.value["tabs"] = json!([{ "id": "t1" }, { "id": "t2" }]);
    snapshot[0].entries[0] = a;
    snapshot.push(CollectionState {
        collection: Collection::Hosts,
        meta: test_support::meta(&keys),
        entries,
    });
    let mut window = open(cx, snapshot);
    assert_eq!(
        window.opened.borrow().len(),
        3,
        "the premise: the agent terminal and two tabs"
    );
    let reads = window
        .helm
        .commands()
        .into_iter()
        .filter(|command| command.path == "/api/sessions/a/tunnels")
        .count();
    assert_eq!(reads, 1);
}

// ---- New terminal and Close ----------------------------------------------------------------------------------------

/// The tab requests the window sent since the last call, as (id, method, path), left unanswered; anything else it sent
/// (a seen mark, a tunnel read) is answered with success at once, since the fake answers in order.
fn tab_requests(helm: &mut FakeHelm) -> Vec<(u64, farhelm_helm_client::Method, String)> {
    let mut found = Vec::new();
    for command in helm.commands() {
        if command.path.starts_with("/api/sessions/a/tabs") {
            found.push((command.id, command.method, command.path));
        } else {
            helm.reply(command.id, 200, None);
        }
    }
    found
}

/// [`snapshot`] with the helm-wide Skip session confirmations switch on.
fn skipping(tabs: &[&str]) -> Vec<CollectionState> {
    let mut snapshot = snapshot(tabs);
    snapshot[1].entries[0].value["skip_session_confirmations"] = json!(true);
    snapshot
}

/// SPEC.md, "Session view": the view "supports additional terminal tabs", and the old strip's New terminal opened one
/// and showed it; the helm's reply and its push of the new tab list can come in either order, and when the user waits
/// the tab must be chosen either way, or a press would leave the user looking at the old terminal. Specified: New terminal sends one
/// `POST /api/sessions/a/tabs`; answered with tab t1 before the row lists it, nothing is chosen yet; once the push lists
/// t1, t1 is chosen and a typed key reaches its stream. A second press, answered after the push of t2, chooses t2 at
/// once. The automatically chosen second shell is also the one shown on returning to the session.
#[gpui::test]
fn new_terminal_opens_a_tab_and_shows_it(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&[]));
    let agent = window.stream_of(None);
    window.go_live(agent, "agent");
    tab_requests(&mut window.helm);

    click(window.cx, super::ADD_ID);
    let sent = tab_requests(&mut window.helm);
    assert_eq!(
        sent.iter()
            .map(|(_, method, path)| (*method, path.as_str()))
            .collect::<Vec<_>>(),
        [(farhelm_helm_client::Method::Post, "/api/sessions/a/tabs")]
    );
    window
        .helm
        .reply(sent[0].0, 200, Some(json!({ "tab": { "id": "t1" } })));
    settle(window.cx);
    assert_eq!(window.chosen(), None, "not listed yet");
    window.push_tabs(&["t1"]);
    assert_eq!(window.chosen().as_deref(), Some("t1"));
    let t1 = window.stream_of(Some("t1"));
    window.go_live(t1, "shell");
    keys(window.cx, "x");
    assert!(window.typed_into(t1), "the new tab has focus");

    click(window.cx, super::ADD_ID);
    let sent = tab_requests(&mut window.helm);
    assert_eq!(sent.len(), 1);
    window.push_tabs(&["t1", "t2"]);
    window
        .helm
        .reply(sent[0].0, 200, Some(json!({ "tab": { "id": "t2" } })));
    settle(window.cx);
    assert_eq!(window.chosen().as_deref(), Some("t2"));
    click(window.cx, "session-row-b");
    click(window.cx, "session-row-a");
    assert_eq!(window.chosen().as_deref(), Some("t2"));
}

/// The accident this plan set out to remove, in the form its landing found: pressing New terminal twice without moving
/// the pointer. When New terminal sat right after the last tab, the tab the first press opened took its place, and the
/// second press landed on that tab's × (shown, since the pointer was over the tab), which asked to close the shell just
/// opened, or with Skip session confirmations on closed it at once. New terminal is now pinned at the tab area's right
/// end (the plan's 2026-11-27 decision; SPEC.md, "Session view"). Specified, with Skip session confirmations off and
/// on, and Terminal 1 already open: a press at New terminal's center sends one `POST /api/sessions/a/tabs`; once it is
/// answered with t2 and the row lists t1 and t2 (the premise: Terminal 2 and its × are drawn, away from that point, and
/// New terminal's center has not moved), a second press at the same point sends another `POST /api/sessions/a/tabs`
/// and nothing else, and draws no close question.
#[gpui::test]
fn a_second_new_terminal_press_at_the_same_spot_opens_another_tab(cx: &mut TestAppContext) {
    second_new_terminal_press(cx, false);
}

/// [`a_second_new_terminal_press_at_the_same_spot_opens_another_tab`] with Skip session confirmations on, where a
/// press on the new tab's × would have closed it with no question at all.
#[gpui::test]
fn a_second_new_terminal_press_at_the_same_spot_opens_another_tab_when_skipping_questions(
    cx: &mut TestAppContext,
) {
    second_new_terminal_press(cx, true);
}

/// The double-press scenario of the two tests above, with Skip session confirmations `skip`.
fn second_new_terminal_press(cx: &mut TestAppContext, skip: bool) {
    let mut window = open(
        cx,
        if skip {
            skipping(&["t1"])
        } else {
            snapshot(&["t1"])
        },
    );
    let agent = window.stream_of(None);
    window.go_live(agent, "agent");
    tab_requests(&mut window.helm);
    let posts = |sent: &[(u64, farhelm_helm_client::Method, String)]| {
        sent.iter()
            .map(|(_, method, path)| (*method, path.clone()))
            .collect::<Vec<_>>()
    };
    let open_one = vec![(
        farhelm_helm_client::Method::Post,
        "/api/sessions/a/tabs".to_owned(),
    )];

    let at = test_support::center(window.cx, super::ADD_ID);
    window.cx.simulate_click(at, gpui::Modifiers::none());
    settle(window.cx);
    let sent = tab_requests(&mut window.helm);
    assert_eq!(posts(&sent), open_one, "the first press opens a tab");
    window.push_tabs(&["t1", "t2"]);
    window
        .helm
        .reply(sent[0].0, 200, Some(json!({ "tab": { "id": "t2" } })));
    settle(window.cx);
    // The premise: Terminal 2 and its × are drawn, so whatever moved to make room for them has moved, and New terminal
    // has not. Without the new tab on screen, a press at `at` would reach New terminal under any layout.
    let nodes = test_support::frame(window.cx);
    let drawn = |id: &str| {
        nodes
            .iter()
            .find(|node| node.element_id.as_deref() == Some(id))
            .map(|node| node.bounds)
            .unwrap_or_else(|| panic!("the premise: {id} is drawn"))
    };
    let (tab, close) = (
        drawn(&tab_element_id("t2")),
        drawn(&super::close_element_id("t2")),
    );
    assert!(
        !close.contains(&at) && !tab.contains(&at),
        "the premise: the second press is not aimed at Terminal 2: tab {tab:?}, × {close:?}, press {at:?}"
    );
    assert_eq!(
        test_support::center(window.cx, super::ADD_ID),
        at,
        "the premise: New terminal stayed where it was"
    );

    window.cx.simulate_click(at, gpui::Modifiers::none());
    settle(window.cx);
    let sent = tab_requests(&mut window.helm);
    assert_eq!(
        posts(&sent),
        open_one,
        "the second press opens another tab and closes nothing (skip {skip})"
    );
    assert!(
        !test_support::drawn(window.cx, "prompt-confirm"),
        "no close question opened"
    );
}

/// SPEC.md, "Who may move focus": once the user has moved on, a result moves no focus. Here the reply comes before the
/// row lists the new shell, and the user has clicked back into the agent and typed; when the row arrives the shell must
/// not be chosen, because hiding the agent would send focus home to the shell, where Enter could run the rest of what
/// the user was typing.
#[gpui::test]
fn new_terminal_reply_before_row_preserves_agent_input_after_moving_on(cx: &mut TestAppContext) {
    new_terminal_preserves_agent_input_after_moving_on(cx, false);
}

/// SPEC.md, "Who may move focus", with the row listing the new shell before the reply arrives: once the user has moved
/// on, the reply must leave the agent shown and typing routed to it. The immediate selection path can redirect input
/// just as the deferred one can.
#[gpui::test]
fn new_terminal_row_before_reply_preserves_agent_input_after_moving_on(cx: &mut TestAppContext) {
    new_terminal_preserves_agent_input_after_moving_on(cx, true);
}

/// Hold both helm messages until the user has resumed typing, then prove where the rest of the input lands with
/// both streams live. An attaching shell would drop input and could conceal a redirect.
fn new_terminal_preserves_agent_input_after_moving_on(cx: &mut TestAppContext, row_first: bool) {
    let mut window = open(cx, snapshot(&[]));
    let agent = window.stream_of(None);
    window.go_live(agent, "agent");
    tab_requests(&mut window.helm);

    click(window.cx, super::ADD_ID);
    let sent = tab_requests(&mut window.helm);
    assert_eq!(sent.len(), 1, "the open request is pending");
    let agent_grid = test_support::frame(window.cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("terminal-grid"))
        .expect("the agent terminal is drawn")
        .bounds;
    window
        .cx
        .simulate_click(agent_grid.center(), gpui::Modifiers::none());
    assert!(test_support::terminal_focused(window.cx));
    keys(window.cx, "x");
    assert!(window.typed_into(agent), "typing resumed in the agent");

    if row_first {
        window.push_tabs(&["t1"]);
    }
    window
        .helm
        .reply(sent[0].0, 200, Some(json!({ "tab": { "id": "t1" } })));
    settle(window.cx);
    if !row_first {
        window.push_tabs(&["t1"]);
    }
    let shell = window.stream_of(Some("t1"));
    window.go_live(shell, "shell");
    assert_eq!(window.phase(shell), FakePhase::Attached);

    keys(window.cx, "y enter");
    let agent_input: Vec<_> = window
        .received(agent)
        .into_iter()
        .filter(|request| matches!(request, FakeTerminalRequest::Input { .. }))
        .collect();
    let shell_input = window.typed_into(shell);
    assert!(
        matches!(agent_input.as_slice(), [
            FakeTerminalRequest::Input { input: GridInput::Key { key: letter, .. }, delivered: true },
            FakeTerminalRequest::Input { input: GridInput::Key { key: enter, .. }, delivered: true },
        ] if letter == "y" && enter == "Enter"),
        "the agent must receive the rest of the typing and Enter: {agent_input:?}; shell received input: {shell_input}"
    );
    assert!(!shell_input, "the shell received no input");
    assert_eq!(window.chosen(), None, "the agent stays chosen");

    click(window.cx, T1_TAB);
    assert_eq!(window.chosen().as_deref(), Some("t1"));
}

/// SPEC.md, "Lifecycle operations": a pending request refuses "another lifecycle action, New terminal or Clone on that
/// session", and the old spec counted the POSTs: repeated presses while an open is in flight make one tab. A refusal
/// must say why in the helm's words (a vanished folder, a shell that died at once) and leave the button usable.
/// Specified: two presses send one POST; a 409 with the helm's words draws "Open tab: " and the words, and a third press
/// sends another POST.
#[gpui::test]
fn new_terminal_sends_once_and_a_refusal_says_why(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&[]));
    tab_requests(&mut window.helm);
    click(window.cx, super::ADD_ID);
    click(window.cx, super::ADD_ID);
    let sent = tab_requests(&mut window.helm);
    assert_eq!(sent.len(), 1, "one open while one is in flight");
    window.helm.reply(
        sent[0].0,
        409,
        Some(json!({ "error": "the session's folder is gone" })),
    );
    settle(window.cx);
    let painted = painted(&test_support::frame(window.cx)).join(" ");
    assert!(
        painted.contains("Open tab: ") && painted.contains("the session's folder is gone"),
        "{painted}"
    );
    click(window.cx, super::ADD_ID);
    assert_eq!(tab_requests(&mut window.helm).len(), 1, "usable again");
}

/// Closing a tab kills its shell and whatever it started, which the window cannot see, so the × asks first, naming the
/// tab and that consequence (the old `CLOSE_TAB_CONSEQUENCE`), with Confirm close focused; Cancel sends nothing. A
/// chosen tab that closed shows the agent terminal. Specified: with t1 chosen, the × draws the question with the
/// consequence and "Terminal 1", focus on its confirming answer; Cancel sends nothing; asked again and confirmed, one
/// `DELETE /api/sessions/a/tabs/t1` goes out, and its success shows the agent terminal.
#[gpui::test]
fn closing_a_tab_asks_first_then_sends_one_delete(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1"]));
    tab_requests(&mut window.helm);
    click(window.cx, T1_TAB);
    assert_eq!(
        window.chosen().as_deref(),
        Some("t1"),
        "the premise: t1 is shown"
    );
    assert_eq!(window.remembered().as_deref(), Some("t1"));
    click(window.cx, "session-terminal-close-t1");
    let painted = painted(&test_support::frame(window.cx));
    assert!(
        painted
            .iter()
            .any(|text| text == super::CLOSE_TAB_CONSEQUENCE),
        "{painted:?}"
    );
    // Once in the strip and once in the question's own line.
    assert_eq!(
        painted.iter().filter(|text| *text == "Terminal 1").count(),
        2,
        "the question names the tab"
    );
    assert_eq!(
        test_support::focused_id(window.cx).as_deref(),
        Some("prompt-confirm")
    );
    click(window.cx, "prompt-cancel");
    assert_eq!(tab_requests(&mut window.helm), []);

    click(window.cx, "session-terminal-close-t1");
    click(window.cx, "prompt-confirm");
    let sent = tab_requests(&mut window.helm);
    assert_eq!(
        sent.iter()
            .map(|(_, method, path)| (*method, path.as_str()))
            .collect::<Vec<_>>(),
        [(
            farhelm_helm_client::Method::Delete,
            "/api/sessions/a/tabs/t1"
        )]
    );
    window.helm.reply(sent[0].0, 200, Some(json!({})));
    settle(window.cx);
    assert_eq!(window.chosen(), None, "the agent terminal shows");
    assert_eq!(window.remembered(), None);
}

/// Closing the second of three tabs chooses the first in strip order, even
/// when the third was the user's previous choice. A pane remount must
/// restore that fallback so the next keystroke reaches the first shell.
#[gpui::test]
fn closing_the_second_tab_remembers_its_predecessor_after_remount(cx: &mut TestAppContext) {
    let mut window = open(cx, skipping(&["t1", "t2", "t3"]));
    tab_requests(&mut window.helm);
    click(window.cx, T3_TAB);
    click(window.cx, T2_TAB);
    assert_eq!(window.chosen().as_deref(), Some("t2"));
    click(window.cx, "session-terminal-close-t2");
    let sent = tab_requests(&mut window.helm);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].1, farhelm_helm_client::Method::Delete);
    assert_eq!(sent[0].2, "/api/sessions/a/tabs/t2");
    window.helm.reply(sent[0].0, 200, Some(json!({})));
    settle(window.cx);
    assert_eq!(window.chosen().as_deref(), Some("t1"));
    assert_eq!(
        window.strip_labels(),
        ["Agent", "Terminal 1", "Terminal 2", "Terminal 3"]
    );

    window.push_tabs(&["t1", "t3"]);
    assert_eq!(window.chosen().as_deref(), Some("t1"));
    assert_eq!(window.strip_labels(), ["Agent", "Terminal 1", "Terminal 2"]);
    click(window.cx, "session-row-b");
    let first_new_stream = window.opened.borrow().len();
    click(window.cx, "session-row-a");
    let remounted = window.opened.borrow()[first_new_stream..]
        .iter()
        .position(|(_, attach)| attach.session == "a" && attach.tab.as_deref() == Some("t1"))
        .expect("t1 has a freshly attached stream")
        + first_new_stream;
    window.go_live(remounted, "first shell after remount");
    assert_eq!(window.chosen().as_deref(), Some("t1"));
    keys(window.cx, "x");
    assert!(window.typed_into(remounted));
}

/// A roster push can complete the visible close before its reply. The push saves the fallback, and the later reply
/// leaves the user's newer selection in place.
#[gpui::test]
fn an_explicit_close_can_reach_the_roster_before_its_reply(cx: &mut TestAppContext) {
    let mut window = open(cx, skipping(&["t1", "t2", "t3"]));
    tab_requests(&mut window.helm);
    click(window.cx, T2_TAB);
    assert_eq!(window.chosen().as_deref(), Some("t2"));
    click(window.cx, "session-terminal-close-t2");
    let sent = tab_requests(&mut window.helm);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].1, farhelm_helm_client::Method::Delete);
    assert_eq!(sent[0].2, "/api/sessions/a/tabs/t2");

    window.push_tabs(&["t1", "t3"]);
    assert_eq!(window.chosen().as_deref(), Some("t1"));
    assert_eq!(window.remembered().as_deref(), Some("t1"));
    click(window.cx, T3_TAB);
    assert_eq!(window.chosen().as_deref(), Some("t3"));
    window.helm.reply(sent[0].0, 200, Some(json!({})));
    settle(window.cx);
    assert_eq!(window.chosen().as_deref(), Some("t3"));
    assert_eq!(window.remembered().as_deref(), Some("t3"));
}

/// Sibling closes have independent replies. A completed background close
/// keeps the active tab and rules out that sibling as a fallback even while
/// the pushed strip still lists it.
#[gpui::test]
fn concurrent_closes_skip_an_acknowledged_predecessor(cx: &mut TestAppContext) {
    let mut window = open(cx, skipping(&["t1", "t2", "t3"]));
    tab_requests(&mut window.helm);
    click(window.cx, T3_TAB);
    assert_eq!(window.chosen().as_deref(), Some("t3"));
    click(window.cx, "session-terminal-close-t2");
    click(window.cx, "session-terminal-close-t3");
    let sent = tab_requests(&mut window.helm);
    assert_eq!(
        sent.iter()
            .map(|(_, method, path)| (*method, path.as_str()))
            .collect::<Vec<_>>(),
        [
            (
                farhelm_helm_client::Method::Delete,
                "/api/sessions/a/tabs/t2"
            ),
            (
                farhelm_helm_client::Method::Delete,
                "/api/sessions/a/tabs/t3"
            ),
        ],
    );
    window.helm.reply(sent[0].0, 200, Some(json!({})));
    settle(window.cx);
    assert_eq!(window.chosen().as_deref(), Some("t3"));
    assert_eq!(
        window.strip_labels(),
        ["Agent", "Terminal 1", "Terminal 2", "Terminal 3"]
    );
    click(window.cx, T2_TAB);
    assert_eq!(
        window.chosen().as_deref(),
        Some("t3"),
        "a completed close cannot be selected"
    );
    window.helm.reply(sent[1].0, 200, Some(json!({})));
    settle(window.cx);
    assert_eq!(window.chosen().as_deref(), Some("t1"));
    window.push_tabs(&["t1"]);
    assert_eq!(window.chosen().as_deref(), Some("t1"));
}

/// The DELETE requests for session a's tabs among [`tab_requests`], as the closed tabs' ids, in the order sent.
fn deleted(helm: &mut FakeHelm) -> Vec<String> {
    tab_requests(helm)
        .into_iter()
        .filter(|(_, method, _)| *method == farhelm_helm_client::Method::Delete)
        .filter_map(|(_, _, path)| {
            path.strip_prefix("/api/sessions/a/tabs/")
                .map(str::to_owned)
        })
        .collect()
}

/// A user who opened one tab's close question and then presses another tab's × has changed their mind about which
/// shell to kill. The second press must leave one question, about the second tab, with its confirming answer focused,
/// so that Enter closes the tab the user last pointed at and nothing else (old `terminal-tabs.spec.ts`, "retargeting an
/// open tab-close question focuses the new answer"). A first question left behind, or Enter still bound to it, would
/// kill the shell the user moved away from. "Retargeting" here is close-then-open: the second × is a press outside the
/// open question, which closes it unanswered on the mouse-down, so by the ×'s click no question is open, and a guard
/// inside `ask_to_close` against an open question never runs. Specified: with t1 and t2, Terminal 1's × opens one
/// question named "Close
/// Terminal 1?", hung under that ×, with focus on its confirm; Terminal 2's × then leaves one question, named "Close
/// Terminal 2?" and hung under Terminal 2's ×, with focus on its confirm and nothing sent; Enter sends exactly one
/// DELETE, for t2.
#[gpui::test]
fn a_second_tabs_close_takes_over_the_question_and_enter_closes_that_tab(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1", "t2"]));
    tab_requests(&mut window.helm);
    // Where a question hangs: an anchored question sits aligned with the left edge of the control it is about.
    let hangs_under = |window: &mut Window, close: &'static str| {
        test_support::bounds(window.cx, "prompt").left()
            == test_support::bounds(window.cx, close).left()
    };
    click(window.cx, "session-terminal-close-t1");
    assert_eq!(
        window.questions(),
        (Some("Close Terminal 1?".to_owned()), 1),
        "the premise: Terminal 1's question is open"
    );
    assert!(
        hangs_under(&mut window, "session-terminal-close-t1"),
        "the premise: it hangs under Terminal 1's ×"
    );
    assert_eq!(
        test_support::focused_id(window.cx).as_deref(),
        Some("prompt-confirm"),
        "the premise: its confirm has focus"
    );

    click(window.cx, "session-terminal-close-t2");
    assert_eq!(
        window.questions(),
        (Some("Close Terminal 2?".to_owned()), 1),
        "one question, now Terminal 2's"
    );
    assert!(
        hangs_under(&mut window, "session-terminal-close-t2"),
        "it hangs under Terminal 2's ×"
    );
    assert_eq!(
        test_support::focused_id(window.cx).as_deref(),
        Some("prompt-confirm"),
        "the new question's confirm has focus"
    );
    assert_eq!(
        deleted(&mut window.helm),
        [] as [String; 0],
        "nothing sent yet"
    );

    keys(window.cx, "enter");
    assert_eq!(deleted(&mut window.helm), ["t2"]);
}

/// The close question acts on the tab it was opened for, by its id, not on whatever tab now sits at the place its name
/// gave: tabs come and go from other clients while the question is open, and a question that followed the place would
/// kill a shell the user never chose (old `terminal-tabs.spec.ts`, "a confirm prompt renumbers with the strip while
/// still targeting its own tab"; here the question keeps the name it opened with, a recorded deviation). A tab list
/// push is not an answer either, so the question stays open through it. Specified: with t1, t2 and t3, Terminal 2's ×
/// opens "Close Terminal 2?"; a push listing t2 and t3 renumbers the strip to "Terminal 1" (t2) and "Terminal 2" (t3)
/// and leaves the question open, still named "Close Terminal 2?"; confirming it sends exactly one DELETE, for t2.
#[gpui::test]
fn a_close_question_closes_its_own_tab_after_the_strip_renumbers(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1", "t2", "t3"]));
    tab_requests(&mut window.helm);
    click(window.cx, "session-terminal-close-t2");
    assert_eq!(
        window.questions(),
        (Some("Close Terminal 2?".to_owned()), 1),
        "the premise: t2's question is open"
    );

    window.push_tabs(&["t2", "t3"]);
    let set = window.set();
    assert_eq!(
        window.cx.update(|_, cx| set.read(cx).tab_ids()),
        ["t2", "t3"],
        "the premise: t1 went"
    );
    assert_eq!(window.strip_labels(), ["Agent", "Terminal 1", "Terminal 2"]);
    assert_eq!(
        window.questions(),
        (Some("Close Terminal 2?".to_owned()), 1),
        "the renumbering did not close the question, which keeps its name"
    );

    click(window.cx, "prompt-confirm");
    assert_eq!(
        deleted(&mut window.helm),
        ["t2"],
        "t2, not t3 now at its place"
    );
}

/// A refused close must change nothing but say so: the tab stays listed, its terminal stays attached and takes typing,
/// the reason shows on that tab, and its × works again (old `terminal-tabs.spec.ts`, "a failed close surfaces on that
/// tab and leaves it attached"). A view that dropped the tab's terminal on the press, ahead of the helm's answer, would
/// leave a shell running that the window no longer shows, and a × left busy would leave no way to try again. The old
/// case's other half, a later successful close of the other tab leaving the refusal shown, is covered in part by
/// [`skipped_questions_close_at_once_side_by_side_and_a_refusal_shows_on_its_tab`], where the sibling's close succeeds
/// alongside the refused one. Specified: with t1 and t2 live, Terminal 1's close confirmed and refused with a 500 and
/// the helm's words draws t1's refusal line with "Close tab: " and those words; t1 is still listed and its stream is
/// still attached; choosing Terminal 1 and typing reaches t1's stream; Terminal 1's × then opens "Close Terminal 1?"
/// again.
#[gpui::test]
fn a_refused_close_leaves_its_tab_listed_attached_and_live(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1", "t2"]));
    let (agent, t1) = (window.stream_of(None), window.stream_of(Some("t1")));
    window.go_live(agent, "agent");
    window.go_live(t1, "shell");
    assert_eq!(
        window.phase(t1),
        FakePhase::Attached,
        "the premise: t1 is live"
    );
    tab_requests(&mut window.helm);

    click(window.cx, "session-terminal-close-t1");
    click(window.cx, "prompt-confirm");
    let sent = tab_requests(&mut window.helm);
    assert_eq!(sent.len(), 1, "the premise: one DELETE, for t1: {sent:?}");
    window.helm.reply(
        sent[0].0,
        500,
        Some(json!({ "error": "injected close failure" })),
    );
    settle(window.cx);

    assert!(
        window
            .cx
            .debug_bounds("session-terminal-close-refusal-t1")
            .is_some(),
        "t1's refusal line"
    );
    let painted = painted(&test_support::frame(window.cx)).join(" ");
    assert!(
        painted.contains("Close tab: ") && painted.contains("injected close failure"),
        "{painted}"
    );
    assert_eq!(window.strip_labels(), ["Agent", "Terminal 1", "Terminal 2"]);
    assert_eq!(window.phase(t1), FakePhase::Attached, "t1 stays attached");

    window.received(t1);
    click(window.cx, T1_TAB);
    keys(window.cx, "x");
    assert!(window.typed_into(t1), "t1 still takes typing");

    click(window.cx, "session-terminal-close-t1");
    assert_eq!(
        window.questions(),
        (Some("Close Terminal 1?".to_owned()), 1),
        "t1's × asks again"
    );
}

/// SPEC.md: Skip session confirmations skips "terminal-tab Close", which then runs on the first press; each tab closes
/// under its own key, so a sibling closes alongside one still in flight (the old spec's "a sibling can close alongside
/// it"), a second press on a closing tab sends nothing, and a refused close says why under the strip, on that tab
/// alone. Specified: with the switch on and tabs t1 and t2, the ×s of t1, t1 again and t2 send one DELETE for each tab;
/// t1's refused with the helm's words draws "Close tab: " and the words in t1's refusal line and none for t2.
#[gpui::test]
fn skipped_questions_close_at_once_side_by_side_and_a_refusal_shows_on_its_tab(
    cx: &mut TestAppContext,
) {
    let mut window = open(cx, skipping(&["t1", "t2"]));
    tab_requests(&mut window.helm);
    click(window.cx, "session-terminal-close-t1");
    click(window.cx, "session-terminal-close-t1");
    click(window.cx, "session-terminal-close-t2");
    assert!(window.cx.debug_bounds("prompt").is_none(), "no question");
    let sent = tab_requests(&mut window.helm);
    assert_eq!(
        sent.iter()
            .map(|(_, _, path)| path.as_str())
            .collect::<Vec<_>>(),
        ["/api/sessions/a/tabs/t1", "/api/sessions/a/tabs/t2"]
    );
    window.helm.reply(
        sent[0].0,
        404,
        Some(json!({ "error": "no such terminal tab" })),
    );
    window.helm.reply(sent[1].0, 200, Some(json!({})));
    settle(window.cx);
    let refusal = window
        .cx
        .debug_bounds("session-terminal-close-refusal-t1")
        .is_some();
    assert!(refusal, "t1's refusal line");
    assert!(
        window
            .cx
            .debug_bounds("session-terminal-close-refusal-t2")
            .is_none()
    );
    let painted = painted(&test_support::frame(window.cx)).join(" ");
    assert!(
        painted.contains("Close tab: ") && painted.contains("no such terminal tab"),
        "{painted}"
    );
}

// ---- The ×'s place and when it shows ---------------------------------------------------------------------------------

/// A point on shell tab `tab`'s name, clear of the leading slot the × covers: its right end.
fn on_name(window: &mut Window, tab: &'static str) -> Point<Pixels> {
    let tab = test_support::bounds(window.cx, tab);
    gpui::point(tab.right() - gpui::px(4.), tab.center().y)
}

/// A point well away from the strip, in the shown terminal.
fn away_from_the_strip(window: &mut Window) -> Point<Pixels> {
    let strip = test_support::bounds(window.cx, STRIP_ID);
    gpui::point(strip.center().x, strip.bottom() + gpui::px(200.))
}

/// The × moved from the always-shown right end of its tab, where the last tab's × sat beside New terminal, to the
/// tab's leading slot, so that a press meant for New terminal stops closing shells. It must still show the moment the
/// user points at the tab, on any shell tab and only that one, and stay while the pointer moves onto it; a key typed
/// with the pointer resting there must not hide it (GPUI's own hover styles stand down after a key press, which is why
/// the strip records the hover itself). An inert strip takes no presses, so pointing at it shows nothing. Specified:
/// with t1 and t2 and the pointer away from the strip, both ×s are hidden; with the pointer on Terminal 2's name,
/// Terminal 2's × shows and Terminal 1's does not; on Terminal 2's × it still shows, and still after a typed key; on
/// the Agent tab both are hidden; once the set is inert, the pointer on Terminal 1's name leaves its × hidden.
#[gpui::test]
fn a_tabs_close_shows_while_the_pointer_is_over_its_tab(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1", "t2"]));
    let away = away_from_the_strip(&mut window);
    window.point_at(away);
    assert_eq!(window.close_shown("t1"), Some(false), "hidden at rest");
    assert_eq!(window.close_shown("t2"), Some(false), "hidden at rest");

    let name = on_name(&mut window, T2_TAB);
    window.point_at(name);
    assert_eq!(window.close_shown("t2"), Some(true), "the pointed-at tab's");
    assert_eq!(window.close_shown("t1"), Some(false), "only that tab's");

    let close = test_support::center(window.cx, "session-terminal-close-t2");
    window.point_at(close);
    assert_eq!(window.close_shown("t2"), Some(true), "on the × itself");
    keys(window.cx, "x");
    assert_eq!(
        window.close_shown("t2"),
        Some(true),
        "a key typed with the pointer resting there"
    );

    let agent = test_support::center(window.cx, AGENT_TAB_ID);
    window.point_at(agent);
    assert_eq!(window.close_shown("t1"), Some(false));
    assert_eq!(window.close_shown("t2"), Some(false));

    let set = window.set();
    window
        .cx
        .update(|_, cx| set.update(cx, |set, cx| set.set_inert(true, cx)));
    settle(window.cx);
    let name = on_name(&mut window, T1_TAB);
    window.point_at(name);
    assert_eq!(
        window.close_shown("t1"),
        Some(false),
        "an inert strip shows none"
    );
}

/// A keyboard user never points at a tab, so keyboard focus on a tab or on its × must show the ×: Tab reaches the × even
/// while it is hidden, and focus on a control the user cannot see is the failure this rules out. Specified: with t1 and
/// t2 and the pointer away from the strip, focus on Terminal 1's button shows Terminal 1's × and not Terminal 2's;
/// focus on Terminal 1's × shows it; focus on New terminal hides both.
#[gpui::test]
fn keyboard_focus_on_a_tab_or_its_close_shows_the_close(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1", "t2"]));
    let away = away_from_the_strip(&mut window);
    window.point_at(away);
    let (tab, close) = window.tab_handles("t1");

    window.focus(&tab);
    assert_eq!(window.close_shown("t1"), Some(true), "focus on the tab");
    assert_eq!(window.close_shown("t2"), Some(false), "only that tab's");
    window.focus(&close);
    assert_eq!(window.close_shown("t1"), Some(true), "focus on the ×");

    let set = window.set();
    let add = window.cx.update(|_, cx| set.read(cx).add_focus.clone());
    window.focus(&add);
    assert_eq!(window.close_shown("t1"), Some(false));
    assert_eq!(window.close_shown("t2"), Some(false));
}

/// The close question hangs under the ×, and the user's pointer leaves the tab to reach the question's answers. A ×
/// that vanished then would leave the question hanging from nothing and hide whose shell it is about to kill.
/// Specified: with t1 and t2, a click on Terminal 1's × opens "Close Terminal 1?" with focus on its confirm; with the
/// pointer on the question, off the strip, Terminal 1's × shows and Terminal 2's does not; once Cancel closes the
/// question and focus is on New terminal, with the pointer off the strip, Terminal 1's × is hidden.
#[gpui::test]
fn an_open_close_question_keeps_its_close_shown(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1", "t2"]));
    click(window.cx, "session-terminal-close-t1");
    assert_eq!(
        window.questions(),
        (Some("Close Terminal 1?".to_owned()), 1),
        "the premise: Terminal 1's question is open"
    );
    assert_eq!(
        test_support::focused_id(window.cx).as_deref(),
        Some("prompt-confirm"),
        "the premise: focus is in the question, not on the tab or its ×"
    );
    let question = test_support::bounds(window.cx, "prompt");
    let strip = test_support::bounds(window.cx, STRIP_ID);
    assert!(
        question.top() >= strip.bottom(),
        "the premise: the question is off the strip: {question:?}, strip {strip:?}"
    );
    window.point_at(question.center());
    assert_eq!(window.close_shown("t1"), Some(true), "its question is open");
    assert_eq!(window.close_shown("t2"), Some(false));

    click(window.cx, "prompt-cancel");
    let set = window.set();
    let add = window.cx.update(|_, cx| set.read(cx).add_focus.clone());
    window.focus(&add);
    let away = away_from_the_strip(&mut window);
    window.point_at(away);
    assert_eq!(window.questions(), (None, 0), "the premise: no question");
    assert_eq!(window.close_shown("t1"), Some(false), "the question closed");
}

/// A close in flight draws its × busy, and the user must still see which tab is going while the helm answers, wherever
/// the pointer and focus have gone since. The × is also drawn over its tab, so a press on it lands on the tab too, and
/// the tab must not take it: closing a tab the user is not looking at must not first show it and move focus into its
/// shell. Specified: with Skip session confirmations on, t1 and t2 listed and the Agent terminal shown, a click on
/// Terminal 1's × sends one DELETE for t1 and leaves the Agent terminal shown; with focus on New terminal and the
/// pointer on the Agent tab, Terminal 1's × shows and Terminal 2's does not; once the helm refuses the close, Terminal
/// 1's × is hidden.
#[gpui::test]
fn a_closing_tabs_close_stays_shown_until_the_helm_answers(cx: &mut TestAppContext) {
    let mut window = open(cx, skipping(&["t1", "t2"]));
    tab_requests(&mut window.helm);
    assert_eq!(window.chosen(), None, "the premise: Agent is shown");
    click(window.cx, "session-terminal-close-t1");
    let sent = tab_requests(&mut window.helm);
    assert_eq!(
        sent.iter()
            .map(|(_, _, path)| path.as_str())
            .collect::<Vec<_>>(),
        ["/api/sessions/a/tabs/t1"]
    );
    assert_eq!(window.chosen(), None, "the press did not choose Terminal 1");

    let set = window.set();
    let add = window.cx.update(|_, cx| set.read(cx).add_focus.clone());
    window.focus(&add);
    let agent = test_support::center(window.cx, AGENT_TAB_ID);
    window.point_at(agent);
    assert_eq!(
        window.close_shown("t1"),
        Some(true),
        "its close is in flight"
    );
    assert_eq!(window.close_shown("t2"), Some(false));

    window.helm.reply(
        sent[0].0,
        500,
        Some(json!({ "error": "injected close failure" })),
    );
    settle(window.cx);
    assert_eq!(window.close_shown("t1"), Some(false), "the close settled");
}

/// A press on Terminal N's × must not also choose Terminal N, in the asking case too: the question takes focus, and a
/// tab chosen by the same press would show a terminal the user did not ask for behind it. Specified: with t1 and t2 and
/// the Agent terminal shown, a click on Terminal 2's × opens "Close Terminal 2?" with focus on its confirm, and the
/// Agent terminal is still the one shown.
#[gpui::test]
fn pressing_a_tabs_close_does_not_choose_the_tab(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1", "t2"]));
    assert_eq!(window.chosen(), None, "the premise: Agent is shown");
    click(window.cx, "session-terminal-close-t2");
    assert_eq!(
        window.questions(),
        (Some("Close Terminal 2?".to_owned()), 1)
    );
    assert_eq!(
        test_support::focused_id(window.cx).as_deref(),
        Some("prompt-confirm")
    );
    assert_eq!(window.chosen(), None, "Terminal 2 was not chosen");
}

/// The point of moving the × was to stop presses landing on the wrong control. A × that pushed tabs or New terminal
/// aside as it appeared would move targets under the pointer again, and a strip whose layout followed the pointer
/// would also break GPUI's frame (`components::copy_field` has the rule). Specified: with t1 and t2, the bounds of both
/// tabs, both ×s and New terminal are the same with the pointer off the strip, with the pointer on Terminal 2's name
/// (its × shown), and with focus on Terminal 1's × (its × shown).
#[gpui::test]
fn showing_a_close_moves_nothing_in_the_strip(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1", "t2"]));
    let away = away_from_the_strip(&mut window);
    let layout = |window: &mut Window| {
        test_support::redraw(window.cx);
        [
            T1_TAB,
            T2_TAB,
            "session-terminal-close-t1",
            "session-terminal-close-t2",
            super::ADD_ID,
        ]
        .map(|id| test_support::bounds(window.cx, id))
    };
    window.point_at(away);
    assert_eq!(
        (window.close_shown("t1"), window.close_shown("t2")),
        (Some(false), Some(false)),
        "the premise: both hidden"
    );
    let rest = layout(&mut window);

    let name = on_name(&mut window, T2_TAB);
    window.point_at(name);
    assert_eq!(window.close_shown("t2"), Some(true), "the premise: shown");
    assert_eq!(layout(&mut window), rest, "pointing at a tab");

    window.point_at(away);
    let (_, close) = window.tab_handles("t1");
    window.focus(&close);
    assert_eq!(window.close_shown("t1"), Some(true), "the premise: shown");
    assert_eq!(layout(&mut window), rest, "focus on a ×");
}

/// The × must stay reachable from the keyboard while it is hidden, and come right after its own tab, so a keyboard user
/// meets each tab's close where they expect it; drawing it over the tab must not reorder Tab. Specified: with t1 and t2
/// and focus on the Agent tab, five Tab presses reach Terminal 1, Close Terminal 1, Terminal 2, Close Terminal 2 and New
/// terminal, in that order.
#[gpui::test]
fn tab_reaches_each_tab_then_its_close_then_new_terminal(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1", "t2"]));
    let set = window.set();
    let agent = window.cx.update(|_, cx| set.read(cx).agent_focus.clone());
    window.focus(&agent);
    assert_eq!(
        test_support::focused_id(window.cx).as_deref(),
        Some(AGENT_TAB_ID),
        "the premise: focus starts on Agent"
    );
    let mut reached = Vec::new();
    for _ in 0..5 {
        keys(window.cx, "tab");
        reached.push(test_support::focused_id(window.cx).unwrap_or_default());
    }
    assert_eq!(
        reached,
        [
            T1_TAB,
            "session-terminal-close-t1",
            T2_TAB,
            "session-terminal-close-t2",
            super::ADD_ID,
        ]
    );
}

/// Opening and closing a tab touch neither a session's creation nor the host registry, which launches and registry
/// changes refuse each other on (SPEC.md, "Launches and changes to the host registry still exclude each other"); a
/// wrong answer would block a launch during a tab's close or let one race a registry change. Specified: both kinds of
/// tab work answer no to both questions.
#[test]
fn tab_work_neither_creates_a_session_nor_changes_the_host_registry() {
    use crate::operations::Work;
    for work in [Work::TabOpen, Work::TabClose] {
        assert!(!work.creates_a_session(), "{work:?}");
        assert!(!work.changes_host_registry(), "{work:?}");
    }
}

/// SPEC.md, "Lifecycle operations": "an open lifecycle confirmation or a pending request refuses another lifecycle
/// action, New terminal or Clone on that session". The session's questions hang from its row and header and are not
/// modal, so New terminal must refuse by itself, or a press would open a shell beside a Stop the user is about to
/// confirm, or while a Stop is closing every tab. Specified: with the header's Stop question open, a press on New
/// terminal sends nothing.
#[gpui::test]
fn new_terminal_is_refused_while_a_lifecycle_question_is_open(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1"]));
    tab_requests(&mut window.helm);
    click(window.cx, "session-header-stop");
    assert!(
        window.cx.debug_bounds("prompt").is_some(),
        "the premise: Stop asks"
    );
    click(window.cx, super::ADD_ID);
    assert_eq!(tab_requests(&mut window.helm), []);
}

/// [`new_terminal_is_refused_while_a_lifecycle_question_is_open`]'s other half: a pending request on the session.
/// Specified: with Skip session confirmations on, the header's Stop goes out at once and, while it is unanswered, a
/// press on New terminal sends nothing. The operation store refuses a second request on the session's key by itself;
/// New terminal's dimmed look while one is out is not checked here (the test frame does not report a button's
/// disabled state).
#[gpui::test]
fn new_terminal_is_refused_while_a_lifecycle_request_is_pending(cx: &mut TestAppContext) {
    let mut window = open(cx, skipping(&["t1"]));
    tab_requests(&mut window.helm);
    click(window.cx, "session-header-stop");
    let stops = window
        .helm
        .commands()
        .into_iter()
        .filter(|command| command.path == "/api/sessions/a/stop")
        .count();
    assert_eq!(stops, 1, "the premise: the Stop is out, unanswered");
    click(window.cx, super::ADD_ID);
    assert_eq!(tab_requests(&mut window.helm), []);
}

/// A Restart detaches only the agent's terminal, so the pane's hold of the reconnect card during it must not reach the
/// session's tabs: a tab whose own stream drops meanwhile has an outage of its own, and holding its card would leave
/// its screen taking no input with nothing saying so until the helm answered. Specified: with a's agent terminal and
/// tab t1 live and t1 shown, a Restart of a confirmed and unanswered, t1's link drops and its reconnect card is drawn.
#[gpui::test]
fn a_restart_does_not_hold_a_tabs_reconnect_card(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1"]));
    let (agent, t1) = (window.stream_of(None), window.stream_of(Some("t1")));
    window.go_live(agent, "agent");
    window.go_live(t1, "shell");
    click(window.cx, T1_TAB);
    assert_eq!(
        window.chosen().as_deref(),
        Some("t1"),
        "the premise: t1 is shown"
    );
    click(window.cx, "session-header-restart");
    click(window.cx, "prompt-confirm");
    let restarts = window
        .helm
        .commands()
        .into_iter()
        .filter(|command| command.path.ends_with("/restart"))
        .count();
    assert_eq!(restarts, 1, "the premise: a Restart is in flight");

    window.opened.borrow_mut()[t1].0.drop_link();
    settle(window.cx);
    assert!(
        window.cx.debug_bounds("terminal-reconnect-card").is_some(),
        "the tab's own outage shows its card"
    );
}

// ---- New terminal tab and Close terminal tab shortcuts -------------------------------------------------------------

/// The close chord of this build: Ctrl+Shift+W on Linux, and on a Mac Cmd+W, the Delete chord, which closes the shown
/// shell tab (SPEC.md, "Session view"). Either way the chord closes a shown shell tab, which is what the tests that
/// press it check; what the Mac's Cmd+W does on the main terminal is checked through the main view's own entry point.
fn close_chord() -> &'static str {
    if cfg!(target_os = "macos") {
        crate::actions::DELETE_SESSION_CHORD
    } else {
        crate::actions::CLOSE_TERMINAL_TAB_LINUX
    }
}

/// SPEC.md, "Session view": Cmd+T on a Mac and Ctrl+Shift+T on Linux open a terminal tab like a terminal emulator's,
/// from wherever focus is, the terminal included, and the chord never reaches the program (a shell would read a stray
/// Ctrl+T as transpose). It is New terminal's press, so the tab it opens is chosen and takes the keyboard when the user
/// waited. Specified: with a's agent terminal live and focused (the premise), the chord sends one
/// `POST /api/sessions/a/tabs` and types nothing into the agent; once the reply names t1 and the row lists it, t1 is
/// shown and a typed key reaches its stream.
#[gpui::test]
fn the_new_terminal_tab_chord_opens_a_tab_from_a_focused_terminal(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&[]));
    let agent = window.stream_of(None);
    window.go_live(agent, "agent");
    let set = window.set();
    assert!(
        window
            .cx
            .update(|w, cx| set.read(cx).agent().read(cx).focus_handle().is_focused(w)),
        "the premise: the agent terminal has focus"
    );
    tab_requests(&mut window.helm);

    keys(window.cx, crate::actions::NEW_TERMINAL_TAB_CHORD);
    let sent = tab_requests(&mut window.helm);
    assert_eq!(
        sent.iter()
            .map(|(_, method, path)| (*method, path.as_str()))
            .collect::<Vec<_>>(),
        [(farhelm_helm_client::Method::Post, "/api/sessions/a/tabs")]
    );
    assert!(
        !window.typed_into(agent),
        "the chord never reached the agent"
    );
    window
        .helm
        .reply(sent[0].0, 200, Some(json!({ "tab": { "id": "t1" } })));
    window.push_tabs(&["t1"]);
    assert_eq!(window.chosen().as_deref(), Some("t1"));
    let t1 = window.stream_of(Some("t1"));
    window.go_live(t1, "shell");
    keys(window.cx, "x");
    assert!(window.typed_into(t1), "the new tab has focus");
}

/// The chord is New terminal's press, so it is refused where the button is (SPEC.md, "Lifecycle operations": an open
/// lifecycle confirmation or a pending request refuses New terminal on that session), and silently, since a chord has
/// no hover help to say why. A chord that slipped past would open a shell beside a Stop the user is about to confirm.
/// Specified: with the header's Stop question open, the chord sends nothing and leaves the question open; with Skip
/// session confirmations on and the header's Stop sent and unanswered (the premise), the chord sends nothing. The
/// operation store refuses a second request on the session's key by itself, so the second half holds even without the
/// shortcut's own check, which keeps the chord in step with the button's disabled state (a mutation run that removed
/// the check still passed).
#[gpui::test]
fn the_new_terminal_tab_chord_is_refused_where_new_terminal_is(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1"]));
    tab_requests(&mut window.helm);
    click(window.cx, "session-header-stop");
    assert!(
        window.cx.debug_bounds("prompt").is_some(),
        "the premise: Stop asks"
    );
    keys(window.cx, crate::actions::NEW_TERMINAL_TAB_CHORD);
    assert_eq!(tab_requests(&mut window.helm), []);
    assert!(
        window.cx.debug_bounds("prompt").is_some(),
        "the question stays open"
    );

    let mut app = cx.new_app();
    let mut window = open(&mut app, skipping(&["t1"]));
    tab_requests(&mut window.helm);
    click(window.cx, "session-header-stop");
    let stops = window
        .helm
        .commands()
        .into_iter()
        .filter(|command| command.path == "/api/sessions/a/stop")
        .count();
    assert_eq!(stops, 1, "the premise: the Stop is out, unanswered");
    keys(window.cx, crate::actions::NEW_TERMINAL_TAB_CHORD);
    assert_eq!(tab_requests(&mut window.helm), []);
    app.quit();
}

/// SPEC.md, "Session view": the close chord closes only the shown shell tab, through the ×'s own close, so it asks
/// first under that × unless session confirmations are skipped, and never reaches the shell. A chord that asked
/// somewhere else, or closed a tab other than the one on screen, would kill the wrong shell. Specified: with t1 and t2
/// listed, t2 shown and its terminal live and focused, the chord opens one question, "Close Terminal 2?", hung under
/// Terminal 2's × with focus on its confirm, sends nothing and types nothing into t2; Enter then sends exactly one
/// `DELETE /api/sessions/a/tabs/t2`.
#[gpui::test]
fn the_close_chord_asks_under_the_shown_tabs_close_and_enter_closes_it(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1", "t2"]));
    let t2 = window.stream_of(Some("t2"));
    click(window.cx, T2_TAB);
    window.go_live(t2, "second");
    assert_eq!(
        window.chosen().as_deref(),
        Some("t2"),
        "the premise: t2 is shown"
    );
    let set = window.set();
    assert!(
        window.cx.update(|w, cx| {
            let set = set.read(cx);
            set.shown()
                .is_some_and(|shown| shown.read(cx).focus_handle().is_focused(w))
        }),
        "the premise: t2's terminal has focus"
    );
    window.received(t2);
    tab_requests(&mut window.helm);

    keys(window.cx, close_chord());
    assert_eq!(
        window.questions(),
        (Some("Close Terminal 2?".to_owned()), 1)
    );
    assert_eq!(
        test_support::bounds(window.cx, "prompt").left(),
        test_support::bounds(window.cx, "session-terminal-close-t2").left(),
        "the question hangs under Terminal 2's ×"
    );
    assert_eq!(
        test_support::focused_id(window.cx).as_deref(),
        Some("prompt-confirm")
    );
    assert_eq!(tab_requests(&mut window.helm), []);
    assert!(!window.typed_into(t2), "the chord never reached the shell");
    keys(window.cx, "enter");
    assert_eq!(deleted(&mut window.helm), ["t2"]);
}

/// With Skip session confirmations on, a × closes its tab at once (SPEC.md: the switch skips "terminal-tab Close"), and
/// the chord is that ×'s press, so it does too. Specified: with the switch on and t1 shown, the chord sends one
/// `DELETE /api/sessions/a/tabs/t1` and draws no question.
#[gpui::test]
fn the_close_chord_closes_at_once_with_questions_skipped(cx: &mut TestAppContext) {
    let mut window = open(cx, skipping(&["t1"]));
    click(window.cx, T1_TAB);
    assert_eq!(
        window.chosen().as_deref(),
        Some("t1"),
        "the premise: t1 is shown"
    );
    tab_requests(&mut window.helm);
    keys(window.cx, close_chord());
    assert!(window.cx.debug_bounds("prompt").is_none(), "no question");
    assert_eq!(deleted(&mut window.helm), ["t1"]);
}

/// A tab scrolled out of the strip can still be the shown one (the wheel moved the strip after it was chosen), and its
/// close question must hang from its × on screen rather than from a place the user cannot see, so the chord scrolls the
/// × into view before it asks. Specified: with 35 tabs, Terminal 35 chosen, and the strip then wheeled back to its start
/// so Terminal 35's × is out of view (the premise), the chord draws Terminal 35's question, its × drawn whole in the
/// strip and the question aligned with that ×'s left edge.
#[gpui::test]
fn the_close_chord_scrolls_an_off_screen_tab_into_view_before_asking(cx: &mut TestAppContext) {
    let ids: Vec<String> = (1..=35).map(|n| format!("t{n}")).collect();
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let mut window = open(cx, snapshot(&refs));
    let set = window.set();
    window.cx.update(|w, cx| {
        let mark = Mark::take(cx);
        set.update(cx, |set, cx| {
            set.choose(Some("t35".to_owned()), &mark, w, cx)
        });
    });
    test_support::redraw(window.cx);
    let strip = window
        .cx
        .debug_bounds(STRIP_ID)
        .expect("the strip is drawn");
    window.cx.simulate_event(gpui::ScrollEvent {
        position: strip.center(),
        delta: gpui::ScrollDelta::Pixels(gpui::point(gpui::px(20000.), gpui::px(0.))),
        ..Default::default()
    });
    test_support::redraw(window.cx);
    let close_whole = |window: &mut Window| {
        test_support::frame(window.cx)
            .iter()
            .find(|node| node.element_id.as_deref() == Some("session-terminal-close-t35"))
            .is_some_and(|node| {
                node.visible_bounds == node.bounds && node.bounds.size.width > gpui::px(0.)
            })
    };
    assert_eq!(window.chosen().as_deref(), Some("t35"));
    assert!(
        !close_whole(&mut window),
        "the premise: Terminal 35's × is out of view"
    );
    tab_requests(&mut window.helm);

    keys(window.cx, close_chord());
    assert_eq!(
        window.questions(),
        (Some("Close Terminal 35?".to_owned()), 1)
    );
    assert!(close_whole(&mut window), "the × was scrolled into view");
    assert_eq!(
        test_support::bounds(window.cx, "prompt").left(),
        test_support::bounds(window.cx, "session-terminal-close-t35").left(),
        "the question hangs under Terminal 35's ×"
    );
    assert_eq!(tab_requests(&mut window.helm), []);
}

/// On the main terminal there is no shell tab to close, and the close action must do nothing rather than reach for
/// some other tab or the session (SPEC.md, "Session view": Ctrl+Shift+W is a no-op on the main tab). Dispatched as the
/// action rather than typed, so the Linux rule is checked whichever build runs it. Specified: with t1 listed and the
/// agent terminal shown, Close terminal tab draws no question and sends no request.
#[gpui::test]
fn close_terminal_tab_on_the_main_terminal_does_nothing(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1"]));
    assert_eq!(
        window.chosen(),
        None,
        "the premise: the agent terminal shows"
    );
    window.helm.commands();
    window.cx.dispatch_action(crate::actions::CloseTerminalTab);
    settle(window.cx);
    assert!(window.cx.debug_bounds("prompt").is_none(), "no question");
    assert!(window.helm.commands().is_empty(), "nothing sent");
}

/// The Delete shortcut per platform (SPEC.md, "Session view" and "Desktop window chrome"): on a Mac, Cmd+W closes the
/// shown shell tab as a terminal emulator's does, and deletes the session only from its main terminal; on Linux the
/// Delete chord is Ctrl+Shift+Backspace, which deletes whatever is shown, since Ctrl+Shift+W closes tabs there. Driven
/// through the main view's own entry point with each platform named, so a Linux run checks the Mac's rule too.
/// Specified: with t1 shown, the Mac's Delete asks "Close Terminal 1?" and sends nothing, and Escape closes it; the
/// Linux Delete with t1 still shown asks the Delete question; with the agent terminal shown, the Mac's Delete asks the
/// Delete question too.
#[gpui::test]
fn the_delete_shortcut_closes_the_shown_tab_on_a_mac_and_deletes_elsewhere(
    cx: &mut TestAppContext,
) {
    use farhelm_input_mapping::Platform;
    let mut window = open(cx, snapshot(&["t1"]));
    click(window.cx, T1_TAB);
    assert_eq!(
        window.chosen().as_deref(),
        Some("t1"),
        "the premise: t1 is shown"
    );
    tab_requests(&mut window.helm);
    let delete = |window: &mut Window, platform: Platform| {
        let main = window.main.clone();
        window
            .cx
            .update(|w, cx| main.update(cx, |main, cx| main.delete_or_close_tab(platform, w, cx)));
        settle(window.cx);
        window.questions().0
    };

    assert_eq!(
        delete(&mut window, Platform::Mac).as_deref(),
        Some("Close Terminal 1?")
    );
    assert_eq!(tab_requests(&mut window.helm), []);
    keys(window.cx, "escape");

    let asked = delete(&mut window, Platform::Linux);
    assert!(
        asked
            .as_deref()
            .is_some_and(|title| title.starts_with("Delete")),
        "the Linux chord deletes from a shell tab too: {asked:?}"
    );
    keys(window.cx, "escape");

    click(window.cx, AGENT_TAB_ID);
    assert_eq!(
        window.chosen(),
        None,
        "the premise: the agent terminal shows"
    );
    let asked = delete(&mut window, Platform::Mac);
    assert!(
        asked
            .as_deref()
            .is_some_and(|title| title.starts_with("Delete")),
        "the Mac's Cmd+W deletes from the main terminal: {asked:?}"
    );
}

/// New terminal names its shortcut in its hover help, as the header's Delete does, so a mouse user learns the chord
/// without opening the Keyboard shortcuts dialog (SPEC.md, "Session view": "its tooltip names the chord"). Specified:
/// New terminal's help ends with this build's New terminal tab shortcut in parentheses.
#[gpui::test]
fn new_terminal_names_its_shortcut(cx: &mut TestAppContext) {
    let window = open(cx, snapshot(&[]));
    let at = test_support::center(window.cx, super::ADD_ID);
    window.cx.simulate_mouse_move(at, None, Modifiers::none());
    test_support::advance(window.cx, crate::layers::HOVER_DELAY);
    let help = window
        .cx
        .update(|w, cx| crate::layers::shown_tooltip(w, cx))
        .map(|shown| shown.text.to_string());
    assert_eq!(
        help,
        Some(format!(
            "New terminal: open a shell tab in the session's folder ({})",
            crate::actions::NEW_TERMINAL_TAB_SHORTCUT_NAME
        ))
    );
}

/// A double tap of a Mac's Cmd+W on a shell tab must not reach the session: with session confirmations skipped the
/// first press closes the tab at once, and if the second fell through to Delete it would end the session too (SPEC.md,
/// "Session view": with a shell tab shown whose close is already in flight, Cmd+W does nothing). Specified: with the
/// switch on and t1 shown, the Mac's Delete shortcut sends `DELETE /api/sessions/a/tabs/t1` and is left unanswered; a
/// second press then sends nothing and draws no question.
#[gpui::test]
fn a_second_cmd_w_while_the_tabs_close_is_in_flight_does_nothing(cx: &mut TestAppContext) {
    use farhelm_input_mapping::Platform;
    let mut window = open(cx, skipping(&["t1"]));
    click(window.cx, T1_TAB);
    assert_eq!(
        window.chosen().as_deref(),
        Some("t1"),
        "the premise: t1 is shown"
    );
    window.helm.commands();
    let press = |window: &mut Window| {
        let main = window.main.clone();
        window.cx.update(|w, cx| {
            main.update(cx, |main, cx| {
                main.delete_or_close_tab(Platform::Mac, w, cx)
            })
        });
        settle(window.cx);
    };
    press(&mut window);
    let first: Vec<String> = window
        .helm
        .commands()
        .into_iter()
        .map(|command| command.path)
        .collect();
    assert_eq!(
        first,
        ["/api/sessions/a/tabs/t1"],
        "the first press closes t1"
    );
    press(&mut window);
    assert!(window.cx.debug_bounds("prompt").is_none(), "no question");
    assert!(
        window.helm.commands().is_empty(),
        "the second press sends nothing"
    );
}

/// A held chord auto-repeats, and New terminal tab is a once-per-press shortcut (`actions.rs`, `ONCE_PER_PRESS`): a
/// held Cmd+T that opened a tab per repeat would fill the strip. The first tab's request has to be answered before the
/// repeats arrive, or the operation store's own refusal of a second request on the session would hide a repeat that
/// got through. Specified: with a's agent terminal live and focused, a press of the chord sends one
/// `POST /api/sessions/a/tabs`, answered with t1 and listed; two held repeats and the release then send nothing more;
/// a fresh press after the release sends a second POST.
#[gpui::test]
fn a_held_new_terminal_tab_chord_opens_one_tab(cx: &mut TestAppContext) {
    let chord = crate::actions::NEW_TERMINAL_TAB_CHORD;
    let mut window = open(cx, snapshot(&[]));
    let agent = window.stream_of(None);
    window.go_live(agent, "agent");
    let set = window.set();
    assert!(
        window
            .cx
            .update(|w, cx| set.read(cx).agent().read(cx).focus_handle().is_focused(w)),
        "the premise: the agent terminal has focus"
    );
    tab_requests(&mut window.helm);
    test_support::press(window.cx, chord, false);
    let sent = tab_requests(&mut window.helm);
    assert_eq!(sent.len(), 1, "the press opens one tab");
    window
        .helm
        .reply(sent[0].0, 200, Some(json!({ "tab": { "id": "t1" } })));
    window.push_tabs(&["t1"]);
    test_support::press(window.cx, chord, true);
    test_support::press(window.cx, chord, true);
    test_support::release(window.cx, chord);
    assert_eq!(
        tab_requests(&mut window.helm),
        [],
        "the repeats open nothing"
    );
    test_support::press(window.cx, chord, false);
    test_support::release(window.cx, chord);
    assert_eq!(
        tab_requests(&mut window.helm).len(),
        1,
        "a fresh press opens another"
    );
}

/// While a card covers the terminals the strip is inert and takes no presses (SPEC.md: a terminal "keeps its last
/// screen and tab strip dimmed and inert beneath the card"), so its shortcuts must not act on it either: the new-tab
/// chord does nothing, and a Mac's Cmd+W, with no tab it could close, deletes as it does from the main terminal.
/// Specified: with t1 shown and the set made inert, as the unavailable card makes it, the new-tab chord sends nothing,
/// and the Mac's Delete shortcut asks the Delete question rather than "Close Terminal 1?".
#[gpui::test]
fn the_tab_shortcuts_leave_an_inert_strip_alone(cx: &mut TestAppContext) {
    use farhelm_input_mapping::Platform;
    let mut window = open(cx, snapshot(&["t1"]));
    click(window.cx, T1_TAB);
    assert_eq!(
        window.chosen().as_deref(),
        Some("t1"),
        "the premise: t1 is shown"
    );
    let set = window.set();
    window
        .cx
        .update(|_, cx| set.update(cx, |set, cx| set.set_inert(true, cx)));
    settle(window.cx);
    assert!(
        window.cx.update(|_, cx| set.read(cx).is_inert()),
        "the premise: the set is inert"
    );
    tab_requests(&mut window.helm);
    keys(window.cx, crate::actions::NEW_TERMINAL_TAB_CHORD);
    assert_eq!(tab_requests(&mut window.helm), [], "no tab opened");
    let main = window.main.clone();
    window.cx.update(|w, cx| {
        main.update(cx, |main, cx| {
            main.delete_or_close_tab(Platform::Mac, w, cx)
        })
    });
    settle(window.cx);
    let asked = window.questions().0;
    assert!(
        asked
            .as_deref()
            .is_some_and(|title| title.starts_with("Delete")),
        "Cmd+W over an inert strip deletes: {asked:?}"
    );
}
