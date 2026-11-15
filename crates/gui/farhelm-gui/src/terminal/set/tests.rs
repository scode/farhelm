//! Tests of the session view's terminal set: which streams it opens for a session's tabs and under which lease, what
//! the strip lists and chooses, where focus goes, how tabs opened, closed and reaped elsewhere come and go, the
//! takeover rule, and the bound on attached tabs.
//!
//! The window is the real main view over the helm client's fake channel, and its connection hands out fake terminals
//! (`Connection::serve_terminals`), which play the helm's side of every stream and record what each was asked. A tab
//! list changes the way the helm changes it: a pushed change to the open session's row.

use std::cell::RefCell;
use std::rc::Rc;

use farhelm_helm_client::fake::FakeHelm;
use farhelm_helm_client::fake::terminal::{FakePhase, FakeTerminal, FakeTerminalRequest};
use farhelm_helm_client::terminal::TerminalAttach;
use farhelm_proto::DetachCode;
use farhelm_proto::grid::GridOutput;
use farhelm_proto::ui_channel::{Collection, CollectionChanges, CollectionState, KeyedEntry};
use gpui::{Entity, TestAppContext, VisualTestContext};
use serde_json::json;

use super::{AGENT_TAB_ID, MAX_ATTACHED_TABS, NOT_ATTACHED_ID, TerminalSet, tab_element_id};
use crate::focus::Mark;
use crate::main_view::MainView;
use crate::session::Connection;
use crate::session_list::tests::{listing_meta, painted, preferences, session};
use crate::session_pane::SessionPane;
use crate::test_support::{self, click, keys, settle, started};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// Tab t1's element id, as [`tab_element_id`] names it (the click helper takes a static id).
const T1_TAB: &str = "session-terminal-tab-t1";

/// Every fake terminal the connection handed out, in the order they were opened, with the attach each was for.
type Opened = Rc<RefCell<Vec<(FakeTerminal, TerminalAttach)>>>;

/// The main window over the fake helm, and the terminals it opened.
struct Window<'a> {
    cx: &'a mut VisualTestContext,
    helm: FakeHelm,
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
    let (mut helm, connection) = started(cx);
    let opened = serve_terminals(&connection, cx);
    helm.connect(snapshot);
    cx.run_until_parked();
    let (main, cx) = test_support::open_view(cx, MainView::new);
    let pane = cx.update(|_, cx| main.read(cx).session_pane().clone());
    Window {
        cx,
        helm,
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

// ---- Tabs that come and go -----------------------------------------------------------------------------------------

/// The tab list is the helm's: a tab opened from another client must appear and attach without a reload, and one
/// closed elsewhere must go, with its stream, while its siblings stay attached; the names follow the places. Specified:
/// from t1 and t2, a push of t1, t2 and t3 opens one stream, for t3, at the shared size, and the strip lists three
/// terminals; a push of t1 and t3 closes t2's stream, leaves t1's and t3's open, and the strip names t3 "Terminal 2".
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

/// SPEC.md, "Session view": "A tab whose process exits on its own is reaped automatically and silently", and the
/// settled focus case: "A shell tab exits on its own while the user types in it. The session's agent tab takes focus."
/// The helm detaches a reaped tab's viewer before its row stops listing the tab, so the terminal must not flash a
/// Detached line in between. Specified: with t1 chosen, live and focused, t1's stream detaching as a closed tab draws
/// no status line; once the row lists no tabs, the agent terminal is chosen and a typed key reaches it.
#[gpui::test]
fn a_reaped_tab_goes_silently_and_the_agent_terminal_takes_focus(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1"]));
    let (agent, t1) = (window.stream_of(None), window.stream_of(Some("t1")));
    window.go_live(agent, "agent");
    window.go_live(t1, "shell");
    click(window.cx, T1_TAB);
    keys(window.cx, "x");
    assert!(window.typed_into(t1), "the premise: t1 has focus");

    window.opened.borrow_mut()[t1]
        .0
        .detach(Some(DetachCode::TabClosed), "the tab was closed");
    settle(window.cx);
    let painted = painted(&test_support::frame(window.cx));
    assert!(
        !painted.iter().any(|text| text.contains("Detached")),
        "no Detached line: {painted:?}"
    );

    window.push_tabs(&[]);
    assert_eq!(window.chosen(), None);
    assert_eq!(window.strip_labels(), ["Agent"]);
    keys(window.cx, "y");
    assert!(window.typed_into(agent), "focus is on the agent terminal");
}

// ---- Takeover ------------------------------------------------------------------------------------------------------

/// The helm hands all of a lease's terminals to the newest attach, so once another window took this view's terminals,
/// attaching a tab it learns about would take everything back without the user asking, and the two windows would take
/// turns (the old app's takeover latch). Take control on any terminal is the user asking. Specified: with the agent
/// terminal and t1 live and both detached as taken over, a push adding t2 opens no stream, and t2's terminal says it
/// is detached with Take control; Take control on the agent terminal asks the agent's and t1's streams to take control
/// and opens t2's.
#[gpui::test]
fn a_takeover_holds_back_new_tabs_until_take_control(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1"]));
    let (agent, t1) = (window.stream_of(None), window.stream_of(Some("t1")));
    window.go_live(agent, "agent");
    window.go_live(t1, "shell");
    for index in [agent, t1] {
        window.opened.borrow_mut()[index]
            .0
            .detach(Some(DetachCode::TakenOver), "another client attached");
    }
    settle(window.cx);

    window.push_tabs(&["t1", "t2"]);
    assert_eq!(window.opened.borrow().len(), 2, "t2 is not attached");
    let set = window.set();
    let parked = window.cx.update(|_, cx| {
        let terminals: Vec<_> = set.read(cx).terminals().cloned().collect();
        terminals[2].read(cx).status_line()
    });
    let parked = parked.expect("t2 says why it shows nothing");
    assert_eq!(parked.words, "Detached: ");
    assert!(parked.take_control);

    test_support::click(window.cx, "terminal-take-control");
    assert!(
        window
            .received(agent)
            .contains(&FakeTerminalRequest::TakeControl)
    );
    // The fake tells the agent terminal it is attaching again once it acts on the request (above); the set hears that.
    settle(window.cx);
    assert!(
        window
            .received(t1)
            .contains(&FakeTerminalRequest::TakeControl)
    );
    assert_eq!(
        window.tabs_attached().last().cloned().flatten().as_deref(),
        Some("t2")
    );
}

// ---- The bound -----------------------------------------------------------------------------------------------------

/// The window builds terminals from a tab list it did not write (a supervisor reached over SSH, say), so it attaches at
/// most 32 tabs, as the old app did, while still listing every one so none is hidden from the user. Specified: with 35
/// tabs listed (t1 to t35), 33 streams open (the agent terminal and t1 to t32), the strip draws a tab for the agent
/// terminal and each of the 35, and choosing t35 shows the not-attached notice instead of a terminal.
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
    // Chosen through the set rather than a click: t35's tab is scrolled out of the strip's view.
    window.cx.update(|w, cx| {
        set.update(cx, |set, cx| {
            set.choose(Some("t35".to_owned()), &Mark::take(cx), w, cx)
        })
    });
    settle(window.cx);
    assert!(window.cx.debug_bounds(NOT_ATTACHED_ID).is_some());
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

/// SPEC.md, "Session list": a terminal of an unreachable host "keeps its last screen and tab strip dimmed and inert
/// beneath the card", and the card covers whichever terminal is shown, so every terminal goes inert, hidden tabs
/// included, and the strip takes no presses (choosing a tab would show a terminal the card does not explain).
/// Specified: once the set is made inert, every terminal reports inert and a click on Terminal 1 chooses nothing; made
/// usable again, the click chooses t1.
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

    window
        .cx
        .update(|_, cx| set.update(cx, |set, cx| set.set_inert(false, cx)));
    settle(window.cx);
    click(window.cx, T1_TAB);
    assert_eq!(window.chosen().as_deref(), Some("t1"));
}

/// The other half of the takeover rule: only the user asking releases it. A tab closed or reaped in the window that
/// took over leaves this window's set while it waits, and if that counted as taking control, this window would take
/// every terminal back from the other without anyone asking, and the two would take turns. Specified: with the agent
/// terminal and t1 live and both detached as taken over, a push without t1 sends the agent terminal's stream nothing,
/// and the agent terminal still waits for Take control.
#[gpui::test]
fn a_tab_leaving_while_taken_over_takes_nothing_back(cx: &mut TestAppContext) {
    let mut window = open(cx, snapshot(&["t1"]));
    let (agent, t1) = (window.stream_of(None), window.stream_of(Some("t1")));
    window.go_live(agent, "agent");
    window.go_live(t1, "shell");
    for index in [agent, t1] {
        window.opened.borrow_mut()[index]
            .0
            .detach(Some(DetachCode::TakenOver), "another client attached");
    }
    settle(window.cx);
    window.received(agent);

    window.push_tabs(&[]);
    assert_eq!(window.received(agent), [], "no Take control");
    let set = window.set();
    let waits = window
        .cx
        .update(|_, cx| set.read(cx).agent().read(cx).taken_over());
    assert!(waits);
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
