//! Tests of the quick switcher: the matcher and ranking as pure functions, and everything else in the real main view
//! over the helm client's fake, which hands the test the switcher's read to answer, fail or hold.
//!
//! The switcher is opened the way the user opens it, with the chord, mostly from the open session's terminal; the
//! tests reach the open switcher through [`Opened`], which only test builds record. Its rows are found by their ids,
//! `quick-switcher-row-<key>`, and its field by the `quick-switcher-field` selector. Every step ends in one of the
//! shared harness's step helpers, so the focus and layer invariants are checked after each.
//!
//! No test here depends on glyph widths, so the simulated platform's fake text system is enough: the scroll test needs
//! only the rows' heights, which come from the line height.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::terminal::{FakeTerminal, FakeTerminalRequest};
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_helm_client::terminal::TerminalAttach;
use farhelm_proto::grid::GridOutput;
use farhelm_proto::ui_channel::{CollectionState, SessionRow};
use gpui::{DriverNode, Entity, Global, TestAppContext, VisualTestContext, WeakEntity};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::rc::Rc;

use super::{CAP_NOTICE, QuickSwitcher, Read, mark_positions, matching_positions, ranked};
use crate::actions::{QUICK_SWITCHER_CHORD, TERMINAL_CONTEXT};
use crate::layers;
use crate::local_state;
use crate::main_view::MainView;
use crate::session_list::tests::{Fixture, listing, painted, preferences, session};
use crate::test_support::{self, click, focused_id, frame, keys, settle, started};

/// The switcher [`super::open`] opened last, recorded in test builds only, so a test can read its state.
pub(super) struct Opened(pub(super) WeakEntity<QuickSwitcher>);

impl Global for Opened {}

/// The chord on this platform (SPEC.md, "Desktop window chrome").
const CHORD: &str = QUICK_SWITCHER_CHORD;

/// The path of the switcher's read.
const READ: &str = "/api/sessions?sort=activity";

// ---- Pure functions ------------------------------------------------------------------------------------------------

/// The matcher is the old one exactly, and the old unit cases pin it: which characters it takes when several would do
/// decides what a later highlight marks, and a matcher that took query characters out of order would find sessions the
/// user did not type. Specified: a greedy, in-order, case-insensitive subsequence over original character positions
/// (`İ`, which lowercases to two characters, counted once; `É` matching `é`), `None` for characters out of order, and
/// an empty query matching at no positions.
#[test]
fn subsequences_keep_the_original_character_positions() {
    assert_eq!(
        matching_positions("API-refactor", "apr"),
        Some(vec![0, 1, 4])
    );
    assert_eq!(matching_positions("İstanbul", "İS"), Some(vec![0, 1]));
    assert_eq!(matching_positions("café", "FÉ"), Some(vec![2, 3]));
    assert_eq!(matching_positions("api", "pa"), None);
    assert_eq!(matching_positions("any title", ""), Some(vec![]));
}

/// A row as the helm's listing serves it: `fixture`'s, on the host the helm names `host`.
fn row(fixture: Fixture, host: &str) -> SessionRow {
    let mut value = fixture.entry().value;
    value["host_name"] = json!(host);
    serde_json::from_value(value).unwrap()
}

/// SPEC.md: "title matches come before matches only in host name or directory, with most recent activity first within
/// each group", and nothing else reorders them. The old case is the spec: a scattered title match outranks a newer
/// contiguous match in the host's name, and title matches keep the helm's order among themselves. Specified: over
/// host (its host named `api-host`), scattered, contiguous and folder (its directory `/api`), in that activity order,
/// `api` lists scattered, contiguous, host, folder; an empty query (and one of spaces, which is trimmed) lists all in
/// the helm's order; a query nothing has lists nothing; an empty title does not match its own key.
#[test]
fn title_matches_come_first_and_each_group_keeps_the_helms_order() {
    let sessions = [
        row(session("host").title("first").cwd("/work"), "api-host"),
        row(
            session("scattered")
                .title("a project implementation")
                .cwd("/work"),
            "build",
        ),
        row(session("contiguous").title("api").cwd("/work"), "build"),
        row(session("folder").title("last").cwd("/api"), "build"),
    ];
    let keys = |query| -> Vec<&str> {
        ranked(&sessions, query)
            .into_iter()
            .map(|index| sessions[index].info.id.as_str())
            .collect()
    };
    assert_eq!(keys("api"), ["scattered", "contiguous", "host", "folder"]);
    assert_eq!(keys(""), ["host", "scattered", "contiguous", "folder"]);
    assert_eq!(keys("   "), ["host", "scattered", "contiguous", "folder"]);
    assert_eq!(keys("absent"), Vec::<&str>::new());
    let empty = [row(session("empty").title("").cwd("/work"), "build")];
    assert_eq!(ranked(&empty, "empty"), Vec::<usize>::new());
}

/// Matching must read the host's raw value, never its escaped spelling: otherwise a session whose title hides a
/// zero-width character could not be found by pasting that character, and typing the escape notation would find a
/// session whose title does not contain it. Specified: a title holding U+200B is found by a query of U+200B and not
/// by the text `<U+200B>`.
#[test]
fn matching_reads_raw_values_not_their_escaped_spelling() {
    let sessions = [row(
        session("peer").title("left\u{200B}right").cwd("/work"),
        "build",
    )];
    assert_eq!(ranked(&sessions, "\u{200B}"), [0]);
    assert_eq!(ranked(&sessions, "<U+200B>"), Vec::<usize>::new());
}

/// The marks show the user which characters matched, so they are found on what the user sees, the escaped spelling,
/// and only in a value that matched: marking raw positions would land beside or inside an escape, and marking any value
/// whose shown spelling happens to contain the query would mark a value that took no part in the match. Specified: a
/// title found only through an invisible character, shown as its escape, marks nothing; a title that did not match
/// marks nothing even when its escape spells the query (`xd` in `x` followed by U+200D, shown `x<U+200D>`); a matched
/// value marks the query's positions in its shown spelling; an empty query marks nothing.
#[test]
fn marks_are_found_on_the_shown_spelling_of_a_matched_value() {
    // The spelling the host-text element shows, made the way it makes it.
    let hidden = "left\u{200B}right";
    let shown = farhelm_proto::text::escaped(hidden);
    assert_eq!(
        shown, "left<U+200B>right",
        "the premise: the character shows as its escape"
    );
    assert!(
        matching_positions(hidden, "\u{200B}").is_some(),
        "the premise: the raw title matches"
    );
    assert_eq!(
        mark_positions(hidden, &shown, "\u{200B}"),
        Vec::<usize>::new()
    );

    let joined = "x\u{200D}";
    let shown = farhelm_proto::text::escaped(joined);
    assert_eq!(
        shown, "x<U+200D>",
        "the premise: the joiner shows as its escape"
    );
    assert!(
        matching_positions(joined, "xd").is_none(),
        "the premise: the raw title does not match"
    );
    assert!(
        matching_positions(&shown, "xd").is_some(),
        "the premise: its shown spelling would"
    );
    assert_eq!(mark_positions(joined, &shown, "xd"), Vec::<usize>::new());

    let shown = farhelm_proto::text::escaped("left\u{200B}right");
    assert_eq!(mark_positions("left\u{200B}right", &shown, "lr"), [0, 12]);
    assert_eq!(
        mark_positions("anything", "anything", ""),
        Vec::<usize>::new()
    );
}

// ---- The window ----------------------------------------------------------------------------------------------------

/// Every fake terminal the connection handed out, in the order the pane opened them.
type Terminals = Rc<RefCell<Vec<FakeTerminal>>>;

/// The real main view over the fake helm, and the fake terminals it opened.
struct Main<'a> {
    cx: &'a mut VisualTestContext,
    helm: FakeHelm,
    terminals: Terminals,
}

/// Sessions a, b and c, in that order, with a the last visited, so a is open at startup.
fn fleet() -> Vec<CollectionState> {
    vec![
        listing(
            &["a", "b", "c"],
            &[session("a"), session("b"), session("c")],
        ),
        preferences(&["a"], None),
    ]
}

/// The main view connected to the fake helm with `snapshot`, its connection handing out fake terminals, open in an
/// active window and checked once settled.
fn open_main(cx: &mut TestAppContext, snapshot: Vec<CollectionState>) -> Main<'_> {
    let (mut helm, connection) = started(cx);
    let terminals: Terminals = Rc::default();
    let record = terminals.clone();
    cx.update(|cx| {
        connection.update(cx, |connection, _| {
            connection.serve_terminals(move |attach: TerminalAttach| {
                let (terminal, handle, events) = FakeTerminal::open(&attach);
                record.borrow_mut().push(terminal);
                (handle, events)
            })
        })
    });
    helm.connect(snapshot);
    cx.run_until_parked();
    let (_, cx) = test_support::open_view(cx, MainView::new);
    Main {
        cx,
        helm,
        terminals,
    }
}

impl Main<'_> {
    /// The reads of the session list sent since the last call. Every other command (the selection remembered, a seen
    /// mark) is answered at once, as the helm answers in order.
    fn reads(&mut self) -> Vec<FakeCommand> {
        let mut reads = Vec::new();
        for command in self.helm.commands() {
            if command.method == Method::Get && command.path == READ {
                reads.push(command);
            } else {
                self.helm.reply(command.id, 200, None);
            }
        }
        reads
    }

    /// Press the chord, which must send one read; its id, to answer it by.
    fn open_switcher(&mut self) -> u64 {
        self.reads();
        keys(self.cx, CHORD);
        let reads = self.reads();
        assert_eq!(reads.len(), 1, "one read per opening: {reads:?}");
        assert_eq!(reads[0].body, None);
        reads[0].id
    }

    /// Answer read `id` with a listing of `rows` (on the host the fixture names), cut at the cap when `truncated`.
    fn answer(&mut self, id: u64, rows: &[Fixture], truncated: bool) {
        let sessions: Vec<Value> = rows.iter().map(|row| row.entry().value).collect();
        let total = sessions.len();
        self.helm.reply(
            id,
            200,
            Some(json!({ "sessions": sessions, "total": total, "truncated": truncated })),
        );
        settle(self.cx);
    }

    /// The first terminal the pane opened attaches and replays a complete screen, so it goes live and takes input.
    fn first_terminal_live(&mut self) {
        {
            let mut terminals = self.terminals.borrow_mut();
            let terminal = &mut terminals[0];
            let asked = terminal.attach();
            for message in screen(asked.cols, asked.rows) {
                terminal.output(message);
            }
            terminal.replay_complete();
        }
        settle(self.cx);
    }

    /// The input requests the first terminal received since the last call.
    fn first_terminal_input(&mut self) -> usize {
        self.terminals.borrow_mut()[0]
            .received()
            .into_iter()
            .filter(|request| matches!(request, FakeTerminalRequest::Input { .. }))
            .count()
    }
}

/// A complete first screen of `cols` by `rows`, as the supervisor replays one.
fn screen(cols: u16, rows: u16) -> Vec<GridOutput> {
    [
        json!({"t":"hello","cols":cols,"rows":rows,"history_cap":0}),
        json!({"t":"state","mouse":"off","focus":false,"alt_screen":false,
            "default_fg":null,"default_bg":null,"history_trimmed":false,"exit":null}),
        json!({"t":"frame","rev":1,"cols":cols,"rows":rows,
            "cursor":{"x":0,"y":0,"visible":true,"shape":"block","blink":false}}),
    ]
    .into_iter()
    .map(|json| serde_json::from_value(json).unwrap())
    .collect()
}

/// The switcher opened last, which must still be open.
fn switcher(cx: &mut VisualTestContext) -> Entity<QuickSwitcher> {
    let switcher = cx.update(|_, cx| {
        cx.try_global::<Opened>()
            .and_then(|opened| opened.0.upgrade())
    });
    switcher.expect("the premise: a switcher is open")
}

fn any_open(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| layers::any_open(window, cx))
}

/// Whether the switcher's search field holds focus.
fn field_focused(cx: &mut VisualTestContext) -> bool {
    let switcher = switcher(cx);
    cx.update(|window, cx| switcher.read(cx).input.focus_handle(cx).is_focused(window))
}

/// Whether a session terminal holds focus: the focused element's key context stack is a terminal's.
fn terminal_focused(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, _| {
        window
            .context_stack()
            .iter()
            .any(|context| context.contains(TERMINAL_CONTEXT))
    })
}

/// The keys of the switcher's rows in the order they are listed, and the key of the selected one.
fn listed(cx: &mut VisualTestContext) -> (Vec<String>, Option<String>) {
    let switcher = switcher(cx);
    cx.update(|_, cx| {
        let switcher = switcher.read(cx);
        let Read::Loaded(list) = &switcher.read else {
            return (Vec::new(), None);
        };
        let keys: Vec<String> = ranked(&list.sessions, &switcher.input.text(cx))
            .into_iter()
            .map(|index| list.sessions[index].info.id.clone())
            .collect();
        let selected = keys
            .get(switcher.selected.min(keys.len().saturating_sub(1)))
            .cloned();
        (keys, selected)
    })
}

/// Type `text` into the switcher's field, which holds focus.
fn type_query(cx: &mut VisualTestContext, text: &str) {
    let keystrokes: Vec<String> = text.chars().map(|ch| ch.to_string()).collect();
    keys(cx, &keystrokes.join(" "));
}

/// The rows the frame drew, top to bottom, by key.
fn drawn_rows(nodes: &[DriverNode]) -> Vec<String> {
    let mut rows: Vec<_> = nodes
        .iter()
        .filter_map(|node| {
            let key = node
                .element_id
                .as_deref()?
                .strip_prefix("quick-switcher-row-")?;
            Some((node.bounds.origin.y, key.to_owned()))
        })
        .collect();
    rows.sort_by(|a, b| f32::from(a.0).total_cmp(&f32::from(b.0)));
    rows.into_iter().map(|(_, key)| key).collect()
}

// ---- Opening -------------------------------------------------------------------------------------------------------

/// SPEC.md: the chord "opens a quick switcher, including from a focused terminal", and the chord "never reaches the
/// terminal's program". The terminal is where focus usually is when the user reaches for the switcher, so this is its
/// common case. Specified: with a's terminal live and focused, the chord opens the switcher with focus in its search
/// field, sends exactly one `GET /api/sessions?sort=activity` with no body, says "Loading sessions…" under "Recent
/// sessions", and the program receives nothing; pressing the chord again inside it opens no second switcher and sends
/// no second read.
#[gpui::test]
fn the_chord_from_a_terminal_opens_the_switcher_and_never_reaches_the_program(
    cx: &mut TestAppContext,
) {
    let mut main = open_main(cx, fleet());
    main.first_terminal_live();
    assert!(
        terminal_focused(main.cx),
        "the premise: a's terminal has focus"
    );
    main.first_terminal_input();

    main.open_switcher();
    assert!(any_open(main.cx));
    assert!(field_focused(main.cx), "focus starts in the search field");
    assert_eq!(
        main.first_terminal_input(),
        0,
        "the chord never reached the program"
    );
    let text = painted(&frame(main.cx));
    assert!(
        text.iter().any(|line| line == "Recent sessions"),
        "{text:?}"
    );
    assert!(
        text.iter().any(|line| line == "Loading sessions…"),
        "{text:?}"
    );

    let first = switcher(main.cx);
    keys(main.cx, CHORD);
    assert_eq!(
        switcher(main.cx),
        first,
        "no second switcher over the first"
    );
    assert_eq!(main.reads(), [], "and no second read");
}

/// SPEC.md: the switcher "does nothing while another modal is open". Opening over a menu would leave two layers
/// fighting over the keyboard, and the old app refused it the same way. Specified: with b's row menu open, the chord
/// leaves the menu the only layer, opens no switcher and reads nothing.
#[gpui::test]
fn the_chord_does_nothing_over_a_menu(cx: &mut TestAppContext) {
    let mut main = open_main(cx, fleet());
    click(main.cx, "session-menu-b");
    assert!(any_open(main.cx), "the premise: b's menu is open");
    main.reads();
    keys(main.cx, CHORD);
    assert!(any_open(main.cx), "the menu is still open");
    assert_eq!(main.reads(), [], "nothing was read");
    assert!(
        main.cx.update(|_, cx| cx.try_global::<Opened>().is_none()),
        "and no switcher opened"
    );
}

/// SPEC.md: the switcher does nothing "before preferences finish loading"; here that is a snapshot without the
/// preferences entry, the state the old app's preferences gate covered. Specified: with sessions listed but no
/// preferences entry, the chord opens nothing and reads nothing.
#[gpui::test]
fn the_chord_does_nothing_before_the_preferences_arrive(cx: &mut TestAppContext) {
    let mut main = open_main(cx, vec![listing(&["a"], &[session("a")])]);
    main.reads();
    keys(main.cx, CHORD);
    assert!(!any_open(main.cx), "nothing opens before the preferences");
    assert_eq!(main.reads(), [], "and nothing was read");
}

// ---- The read ------------------------------------------------------------------------------------------------------

/// SPEC.md: "A loading list says loading, rather than no matches, and Enter picks nothing until a reply arrives; a
/// failed read says so." Picking from a list that has not arrived would open whatever happened to be first. Specified:
/// while the read is unanswered, typing and Enter leave the switcher open, the selection unchanged and nothing sent;
/// a refusal then shows "Couldn't load sessions: " with the helm's words, and no "No sessions match".
#[gpui::test]
fn loading_picks_nothing_and_a_failed_read_says_so(cx: &mut TestAppContext) {
    let mut main = open_main(cx, fleet());
    let read = main.open_switcher();
    type_query(main.cx, "b");
    keys(main.cx, "enter");
    assert!(any_open(main.cx), "Enter picked nothing while loading");
    assert_eq!(
        main.cx.update(|_, cx| local_state::selected(cx)).as_deref(),
        Some("a")
    );
    assert_eq!(main.reads(), []);

    main.helm.reply(read, 503, Some(json!("the helm is busy")));
    settle(main.cx);
    let nodes = frame(main.cx);
    let failure = nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some("quick-switcher-failure"))
        .and_then(|node| node.label.clone());
    assert_eq!(
        failure.as_deref(),
        Some("Couldn't load sessions: the helm is busy")
    );
    assert!(
        !painted(&nodes)
            .iter()
            .any(|line| line == "No sessions match")
    );
    assert!(
        !painted(&nodes)
            .iter()
            .any(|line| line == "Loading sessions…")
    );
}

/// The switcher searches its own listing, not the sidebar's, and lists it in the ranking order; a listing the helm's
/// cap cut must say so, or a session past the cap would look as if it did not exist. Specified: over a reply of a, b,
/// c (b on the host `api-box`, c titled `api work`), an empty query lists a, b, c under "Recent sessions" with no cap
/// notice; `api` lists c then b under "Sessions"; a query nothing matches says "No sessions match"; a reply cut at the
/// cap shows the cap notice.
#[gpui::test]
fn a_reply_is_listed_in_ranking_order_and_a_cut_reply_says_so(cx: &mut TestAppContext) {
    let mut main = open_main(cx, fleet());
    let read = main.open_switcher();
    let mut b = session("b").entry().value;
    b["host_name"] = json!("api-box");
    let rows = [session("a"), session("c").title("api work")];
    let mut sessions: Vec<Value> = rows.iter().map(|row| row.entry().value).collect();
    sessions.insert(1, b);
    main.helm.reply(
        read,
        200,
        Some(json!({ "sessions": sessions, "total": 3, "truncated": false })),
    );
    settle(main.cx);
    let nodes = frame(main.cx);
    assert_eq!(drawn_rows(&nodes), ["a", "b", "c"]);
    assert!(painted(&nodes).iter().any(|line| line == "Recent sessions"));
    assert!(!painted(&nodes).iter().any(|line| line == CAP_NOTICE));

    type_query(main.cx, "api");
    let nodes = frame(main.cx);
    assert_eq!(drawn_rows(&nodes), ["c", "b"], "the title match first");
    assert!(painted(&nodes).iter().any(|line| line == "Sessions"));

    type_query(main.cx, "zzz");
    assert!(
        painted(&frame(main.cx))
            .iter()
            .any(|line| line == "No sessions match")
    );

    keys(main.cx, "escape");
    let read = main.open_switcher();
    main.answer(read, &[session("a")], true);
    assert!(
        painted(&frame(main.cx))
            .iter()
            .any(|line| line == CAP_NOTICE)
    );
}

/// A reply belongs to the opening that asked for it: one that landed in a switcher opened since would replace the
/// listing the user is searching with an older one. Each opening is a switcher entity of its own and the reply holds
/// only its own switcher, so this pins that separation even while the earlier switcher is kept alive (here by the
/// test's own handle). Specified: with the first switcher held, closed and a second one opened, answering the first
/// read leaves the second loading; answering the second read lists its rows.
#[gpui::test]
fn a_reply_to_an_earlier_opening_never_reaches_a_later_switcher(cx: &mut TestAppContext) {
    let mut main = open_main(cx, fleet());
    let first = main.open_switcher();
    let held = switcher(main.cx);
    keys(main.cx, "escape");
    assert!(!any_open(main.cx), "the premise: the first switcher closed");
    let second = main.open_switcher();
    let reopened = switcher(main.cx);
    assert_ne!(
        reopened, held,
        "the premise: the reopening is a new switcher"
    );
    main.answer(first, &[session("a"), session("b")], false);
    assert!(
        main.cx
            .update(|_, cx| matches!(reopened.read(cx).read, Read::Loading)),
        "the first opening's reply did not land in the second"
    );
    main.answer(second, &[session("c")], false);
    assert_eq!(listed(main.cx).0, ["c"]);
}

// ---- Choosing and picking -----------------------------------------------------------------------------------------

/// SPEC.md: "Arrow keys choose and Enter opens the selected session ...; the first match is initially selected." The
/// arrows must not move the field's caret, and the selection must stop at both ends rather than wrap. Specified: over
/// a, b, c the first row is selected; Down three times stops at c; Up three times stops at a; typing resets the
/// selection to the first match; focus stays in the field throughout.
#[gpui::test]
fn the_arrows_move_the_selection_and_stop_at_both_ends(cx: &mut TestAppContext) {
    let mut main = open_main(cx, fleet());
    let read = main.open_switcher();
    main.answer(read, &[session("a"), session("b"), session("c")], false);
    assert_eq!(listed(main.cx).1.as_deref(), Some("a"));
    keys(main.cx, "down down down");
    assert_eq!(
        listed(main.cx).1.as_deref(),
        Some("c"),
        "Down stops at the last"
    );
    keys(main.cx, "up up up");
    assert_eq!(
        listed(main.cx).1.as_deref(),
        Some("a"),
        "Up stops at the first"
    );
    keys(main.cx, "down");
    // c's title matches; a and b match only in their folders (`/home/user/src/...`), so they follow it.
    type_query(main.cx, "c");
    let (keys, selected) = listed(main.cx);
    assert_eq!(keys, ["c", "a", "b"]);
    assert_eq!(
        selected.as_deref(),
        Some("c"),
        "an edit selected the first match again"
    );
    assert!(field_focused(main.cx), "focus never left the field");
}

/// Every match is reachable from the keyboard (SPEC.md: "Every match is reachable by scrolling"), and the selected row
/// must stay in view while the arrows walk past the visible part of the list, or the user picks blind. The old case
/// used 41 matches. Specified: with 41 sessions, 40 presses of Down select the last, and that row is drawn inside the
/// results list's visible area.
#[gpui::test]
fn the_selected_row_stays_in_view(cx: &mut TestAppContext) {
    const KEYS: [&str; 41] = [
        "s00", "s01", "s02", "s03", "s04", "s05", "s06", "s07", "s08", "s09", "s10", "s11", "s12",
        "s13", "s14", "s15", "s16", "s17", "s18", "s19", "s20", "s21", "s22", "s23", "s24", "s25",
        "s26", "s27", "s28", "s29", "s30", "s31", "s32", "s33", "s34", "s35", "s36", "s37", "s38",
        "s39", "s40",
    ];
    let mut main = open_main(cx, fleet());
    let read = main.open_switcher();
    let rows: Vec<Fixture> = KEYS.iter().map(|key| session(key)).collect();
    main.answer(read, &rows, false);
    keys(main.cx, &["down"; 40].join(" "));
    assert_eq!(listed(main.cx).1.as_deref(), Some("s40"));
    let nodes = frame(main.cx);
    let results = nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some("quick-switcher-results"))
        .map(|node| node.visible_bounds)
        .expect("the results list was drawn");
    let last = nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some("quick-switcher-row-s40"))
        .map(|node| node.bounds)
        .expect("the last row was drawn");
    assert!(
        last.top() >= results.top() && last.bottom() <= results.bottom() + gpui::px(0.5),
        "the last row {last:?} is inside the list's visible area {results:?}"
    );
    let first = nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some("quick-switcher-row-s00"))
        .map(|node| node.bounds)
        .expect("the first row was drawn");
    assert!(
        first.bottom() <= results.top(),
        "the premise: the list scrolled, so the first row is above its visible area"
    );
}

/// Picking is the switcher's whole point, and it must open the session exactly as a click on its row does: selected,
/// remembered with the helm, and with its terminal focused, the already open one included (SPEC.md, "Where focus
/// rests": "Choosing a session, including the one already selected, focuses its terminal"). Specified: typing `c` and
/// Enter closes the switcher, selects c, writes c as the `last_selected` preference and focuses its terminal; opened
/// again from c's row actions button, a click on c's row, the open session, closes it with focus in c's terminal
/// (not back on the button) and nothing written.
#[gpui::test]
fn a_pick_opens_the_session_remembers_it_and_focuses_its_terminal(cx: &mut TestAppContext) {
    let mut main = open_main(cx, fleet());
    let read = main.open_switcher();
    main.answer(read, &[session("a"), session("b"), session("c")], false);
    type_query(main.cx, "c");
    assert_eq!(
        listed(main.cx).1.as_deref(),
        Some("c"),
        "the premise: c is selected"
    );
    keys(main.cx, "enter");
    assert!(!any_open(main.cx), "the pick closed the switcher");
    assert_eq!(
        main.cx.update(|_, cx| local_state::selected(cx)).as_deref(),
        Some("c")
    );
    let mut sent: Vec<(String, Value)> = Vec::new();
    for command in main.helm.commands() {
        main.helm.reply(command.id, 200, None);
        sent.push((command.path, command.body.unwrap_or_default()));
    }
    settle(main.cx);
    assert!(
        sent.contains(&("/api/preferences".into(), json!({ "last_selected": "c" }))),
        "{sent:?}"
    );
    // The pane shows only the selected session's terminal, so with c selected a focused terminal is c's.
    assert!(terminal_focused(main.cx), "c's terminal took focus");

    // Opened from c's row actions button this time, so that returning focus to the opener (the old app's rule) and
    // focusing the chosen session's terminal (SPEC.md's) end in different places.
    click(main.cx, "session-menu-c");
    keys(main.cx, "escape");
    assert_eq!(
        focused_id(main.cx).as_deref(),
        Some("session-menu-c"),
        "the premise: c's actions button holds focus"
    );
    let read = main.open_switcher();
    main.answer(read, &[session("a"), session("c")], false);
    click(main.cx, "quick-switcher-row-c");
    assert!(!any_open(main.cx));
    assert_eq!(
        main.cx.update(|_, cx| local_state::selected(cx)).as_deref(),
        Some("c")
    );
    assert!(
        terminal_focused(main.cx),
        "picking the open session focused its terminal, not the button it was opened from"
    );
    assert!(
        !main
            .helm
            .commands()
            .iter()
            .any(|command| command.path == "/api/preferences"),
        "choosing the open session writes nothing"
    );
}

/// SPEC.md: "Escape or a click outside closes and restores the previous focus", and the old footer's Close does the
/// same. Focus going home instead would be wrong whenever the user opened the switcher from somewhere else. Specified:
/// opened from a's focused terminal, Escape (after moving the selection) closes it with focus back on the terminal and
/// nothing picked; opened again, a click on Close does the same.
#[gpui::test]
fn escape_and_close_return_focus_to_where_it_was(cx: &mut TestAppContext) {
    let mut main = open_main(cx, fleet());
    assert!(
        terminal_focused(main.cx),
        "the premise: a's terminal has focus"
    );
    let read = main.open_switcher();
    main.answer(read, &[session("a"), session("b")], false);
    keys(main.cx, "down escape");
    assert!(!any_open(main.cx));
    assert!(terminal_focused(main.cx), "Escape gave focus back");
    assert_eq!(
        main.cx.update(|_, cx| local_state::selected(cx)).as_deref(),
        Some("a"),
        "and picked nothing"
    );

    main.open_switcher();
    click(main.cx, "quick-switcher-close");
    assert!(!any_open(main.cx));
    assert!(terminal_focused(main.cx), "Close gave focus back");
}

/// SPEC.md: "Escape or a click outside closes and restores the previous focus." A press outside the card is the
/// pointer's way out, and it must not pass through to what lies under it: the veil takes it, so the session row under
/// the pointer is not opened. Specified: opened from a's focused terminal, a press on b's sidebar row, outside the
/// card, closes the switcher with focus back on a's terminal and a, not b, still the open session.
#[gpui::test]
fn a_press_outside_closes_it_and_returns_focus(cx: &mut TestAppContext) {
    let mut main = open_main(cx, fleet());
    assert!(
        terminal_focused(main.cx),
        "the premise: a's terminal has focus"
    );
    let read = main.open_switcher();
    main.answer(read, &[session("a"), session("b")], false);
    let card = test_support::bounds(main.cx, "quick-switcher");
    // Near the row's left edge, which the card's side margin always leaves uncovered; the row's center may be under
    // the card in a narrow window.
    let row = test_support::bounds(main.cx, "session-row-b");
    let outside = gpui::point(row.left() + gpui::px(8.), row.center().y);
    assert!(
        row.contains(&outside) && !card.contains(&outside),
        "the premise: {outside:?} is on b's row {row:?} and outside the card {card:?}"
    );
    main.cx.simulate_click(outside, gpui::Modifiers::none());
    settle(main.cx);
    assert!(!any_open(main.cx), "the press closed the switcher");
    assert!(
        terminal_focused(main.cx),
        "and focus went back to the terminal"
    );
    assert_eq!(
        main.cx.update(|_, cx| local_state::selected(cx)).as_deref(),
        Some("a")
    );
}
