//! Tests of the quick switcher: the matcher and ranking as pure functions, and everything else in the real main view
//! over the helm client's fake, which hands the test the switcher's read to answer, fail or hold.
//!
//! The switcher is opened the way the user opens it, with the chord, mostly from the open session's terminal; the
//! tests reach the open switcher through [`Opened`], which only test builds record. Its rows are found by their ids,
//! `quick-switcher-row-<key>`, and its field by the `quick-switcher-field` selector. Every step ends in one of the
//! shared harness's step helpers, so the focus and layer invariants are checked after each.
//!
//! The last tests pick sessions the sidebar's host and status filters hide, to pin the reset of both filters, which the
//! session list makes once the pick has opened the session (`filters.rs`).
//!
//! No test here depends on glyph widths, so the simulated platform's fake text system is enough: the scroll test needs
//! only the rows' heights, which come from the line height.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeCommand;
use farhelm_proto::ui_channel::{CollectionState, SESSION_QUERY_PATH, SessionRow};
use gpui::{DriverNode, Entity, Global, TestAppContext, VisualTestContext, WeakEntity};
use serde_json::{Value, json};

use super::{
    CAP_NOTICE, NEW_ROW_HELP, NEW_ROW_ID, QuickSwitcher, Read, Selected, mark_positions,
    matching_positions, ranked,
};
use crate::actions::QUICK_SWITCHER_CHORD;
use crate::launcher::launch_tests::{HostChange, start_host_change};
use crate::layers;
use crate::local_state;
use crate::session_list::host_rail::tests::{
    filtered, hosts, hover_help, local, node, pressed, remote, texts_within,
};
use crate::session_list::tests::{
    Fixture, change, launcher_drawn, listing, painted, preferences, selected, session,
};
use crate::test_support::{
    self, any_open, click, focused_id, frame, keys, open_main, settle, terminal_focused,
};

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

/// The real main view over the fake helm ([`test_support::MainWindow`]), with this module's steps.
type Main<'a> = test_support::MainWindow<'a>;

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
}

/// The switcher opened last, which must still be open.
fn switcher(cx: &mut VisualTestContext) -> Entity<QuickSwitcher> {
    let switcher = cx.update(|_, cx| {
        cx.try_global::<Opened>()
            .and_then(|opened| opened.0.upgrade())
    });
    switcher.expect("the premise: a switcher is open")
}

/// Whether the switcher's search field holds focus.
fn field_focused(cx: &mut VisualTestContext) -> bool {
    let switcher = switcher(cx);
    cx.update(|window, cx| switcher.read(cx).input.focus_handle(cx).is_focused(window))
}

/// The keys of the switcher's session rows in the order they are listed, and the key of the selected row: a session's
/// key, or [`NEW_ROW_ID`] for the New row.
fn listed(cx: &mut VisualTestContext) -> (Vec<String>, Option<String>) {
    let switcher = switcher(cx);
    cx.update(|_, cx| {
        let switcher = switcher.read(cx);
        let rows = switcher.rows(cx);
        let key = |index: usize| match &switcher.read {
            Read::Loaded(list) => list.sessions[index].info.id.clone(),
            Read::Loading | Read::Failed(_) => unreachable!("only a loaded read has matches"),
        };
        let keys: Vec<String> = rows.matches.iter().map(|&index| key(index)).collect();
        let selected = rows.selected(switcher.selected).map(|row| match row {
            Selected::Session(index) => key(index),
            Selected::New => NEW_ROW_ID.to_owned(),
        });
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
    main.terminal_live();

    main.open_switcher();
    assert!(any_open(main.cx));
    assert!(field_focused(main.cx), "focus starts in the search field");
    assert_eq!(
        main.program_input(),
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

/// Fill a's terminal with a program's hyperlink across its whole screen, so the pointer anywhere over it rests on the
/// link, and return a point over the terminal.
fn link_screen(main: &mut Main<'_>) -> gpui::Point<gpui::Pixels> {
    {
        let mut terminals = main.terminals.borrow_mut();
        let terminal = &mut terminals[0];
        let asked = terminal.attach();
        let text = "x".repeat(usize::from(asked.cols));
        let rows: Vec<Value> = (0..asked.rows)
            .map(|y| json!([y, {"w":false,"r":[[0,asked.cols,text,[0,-1,-1],0]]}]))
            .collect();
        let mut replay = test_support::blank_screen(asked.cols, asked.rows);
        replay.insert(
            2,
            serde_json::from_value(
                json!({"t":"rows","rows":rows,"links":[["1","https://example.com/"]]}),
            )
            .unwrap(),
        );
        for message in replay {
            terminal.output(message);
        }
        terminal.replay_complete();
    }
    settle(main.cx);
    let size = main.cx.update(|window, _| window.viewport_size());
    gpui::point(size.width * 0.75, size.height * 0.5)
}

/// A terminal link's destination display is a popover that never takes focus, not a modal: the switcher's chord still
/// opens the switcher while one shows, as Rename's and Delete's do (they share the rule, asking whether a layer that
/// holds focus is open rather than whether any layer is). And the display closing beneath the switcher, as it does
/// once the switcher's veil ends the hover, must not take the switcher with it: closing a layer once closed every layer
/// above it. Specified: with the pointer resting on a hyperlink in a's terminal, a layer is open but none holds focus;
/// the chord opens the switcher, which stays open through redraws and a press in its search field; after Escape the
/// display shows again for the link under the pointer.
#[gpui::test]
fn the_chord_opens_the_switcher_while_a_link_display_shows(cx: &mut TestAppContext) {
    let mut main = open_main(cx, fleet());
    let over = link_screen(&mut main);
    main.cx
        .simulate_mouse_move(over, None, gpui::Modifiers::none());
    settle(main.cx);
    test_support::redraw(main.cx);
    assert!(
        main.cx.update(
            |window, cx| layers::any_open(window, cx) && !layers::focus_layer_open(window, cx)
        ),
        "the premise: the link's display shows, holding no focus"
    );
    main.open_switcher();
    settle(main.cx);
    test_support::redraw(main.cx);
    settle(main.cx);
    let _ = switcher(main.cx);
    assert!(
        main.cx
            .update(|window, cx| layers::focus_layer_open(window, cx)),
        "the switcher opened and stays open"
    );
    let field = main
        .cx
        .debug_bounds("quick-switcher-field")
        .expect("the premise: the search field is drawn")
        .center();
    main.cx
        .simulate_mouse_down(field, gpui::MouseButton::Left, gpui::Modifiers::none());
    main.cx
        .simulate_mouse_up(field, gpui::MouseButton::Left, gpui::Modifiers::none());
    settle(main.cx);
    test_support::redraw(main.cx);
    assert!(
        main.cx
            .update(|window, cx| layers::focus_layer_open(window, cx)),
        "a press inside the switcher leaves it open"
    );
    test_support::keys(main.cx, "escape");
    settle(main.cx);
    test_support::redraw(main.cx);
    settle(main.cx);
    test_support::redraw(main.cx);
    assert!(
        !main
            .cx
            .update(|window, cx| layers::focus_layer_open(window, cx)),
        "the premise: Escape closed the switcher"
    );
    assert!(
        main.cx.debug_bounds("terminal-link-disclosure").is_some(),
        "and the display shows for the link under the pointer"
    );
}

/// A menu has no veil, so the terminal keeps getting hover while one is open, but no destination display may open
/// above the menu; once the menu closes, the link the pointer rests on must get its display, or a click on it is
/// refused. Once, the display asked for under the menu stayed pending and nothing showed until the pointer left the
/// link. Specified: with b's row menu open, resting on a's hyperlink shows no display; after Escape closes the menu,
/// the display shows without the pointer moving.
#[gpui::test]
fn a_link_hovered_under_a_menu_shows_its_display_once_the_menu_closes(cx: &mut TestAppContext) {
    let mut main = open_main(cx, fleet());
    let over = link_screen(&mut main);
    click(main.cx, "session-menu-b");
    assert!(
        main.cx
            .update(|window, cx| layers::focus_layer_open(window, cx)),
        "the premise: b's menu is open"
    );
    main.cx
        .simulate_mouse_move(over, None, gpui::Modifiers::none());
    settle(main.cx);
    test_support::redraw(main.cx);
    settle(main.cx);
    assert!(
        main.cx
            .update(|_, cx| crate::terminal::attached::any_hovered_link(cx)),
        "the premise: with no veil, the terminal sees the link under the pointer"
    );
    assert!(
        main.cx.debug_bounds("terminal-link-disclosure").is_none(),
        "no display above the menu"
    );
    test_support::keys(main.cx, "escape");
    settle(main.cx);
    test_support::redraw(main.cx);
    settle(main.cx);
    test_support::redraw(main.cx);
    assert!(
        !main
            .cx
            .update(|window, cx| layers::focus_layer_open(window, cx)),
        "the premise: Escape closed the menu"
    );
    assert!(
        main.cx.debug_bounds("terminal-link-disclosure").is_some(),
        "the display shows once the menu is gone"
    );
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
/// again from c's row actions button, with `c` typed so that c is listed first though the read lists it second, a click
/// on c's row, the open session, closes it with focus in c's terminal (not back on the button) and nothing written.
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
    // Typed first, so c is listed first though it is second in the read: a click must pick by the read's index, not
    // by the row's place in the list.
    type_query(main.cx, "c");
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

// ---- A pick the sidebar's filters hide -----------------------------------------------------------------------------

/// The sessions of the filter tests: a running on host 2 (open), b idle on host 2, c idle on host 3, over the rail's
/// local host, host 3 and host 2.
fn filtered_fleet() -> Vec<CollectionState> {
    let idle = || json!({ "state": "idle" });
    vec![
        listing(
            &["a", "b", "c"],
            &[
                session("a").host(2),
                session("b").host(2).status(idle()),
                session("c").host(3).status(idle()),
            ],
        ),
        preferences(&["a"], None),
        hosts(&[
            local(),
            remote(3, "ops@build.example.invalid"),
            remote(2, "ops@render.example.invalid"),
        ]),
    ]
}

impl Main<'_> {
    /// Choose host 2 on the rail and Idle in the segments, and have the helm accept both, with the listing it then
    /// serves: b alone, 1 of the fleet's 3.
    fn filter_to_idle_on_host_2(&mut self) {
        self.reads();
        click(self.cx, "host-rail-host-2");
        click(self.cx, "session-status-Idle");
        self.helm
            .push(change(filtered(&["b"], 3, 1), &[], &["a", "c"]));
        self.accept_commands();
        let nodes = frame(self.cx);
        assert!(
            pressed(&nodes, "host-rail-host-2") && pressed(&nodes, "session-status-Idle"),
            "the premise: host 2 and Idle are chosen"
        );
    }

    /// Accept every command the window sent since the last look, and return the session queries among them, by body.
    fn accept_commands(&mut self) -> Vec<Value> {
        let mut queries = Vec::new();
        for command in self.helm.commands() {
            if command.path == SESSION_QUERY_PATH {
                queries.push(command.body.clone().unwrap_or_default());
            }
            self.helm.reply(command.id, 204, None);
        }
        settle(self.cx);
        queries
    }

    /// Open the switcher, answer its read with a, b and c as the helm lists them, type `query` and press Enter.
    fn pick(&mut self, query: &str) {
        let read = self.open_switcher();
        self.answer(
            read,
            &[
                session("a").host(2),
                session("b").host(2).status(json!({ "state": "idle" })),
                session("c").host(3).status(json!({ "state": "idle" })),
            ],
            false,
        );
        type_query(self.cx, query);
        keys(self.cx, "enter");
        assert!(!any_open(self.cx), "the pick closed the switcher");
    }
}

/// SPEC.md: "Opening a session hidden by the host choice or status filter resets both to All after the open is
/// accepted ... A pick already visible through both leaves them in place" (the old `status-filter.spec.ts` cases), so
/// the user sees what they opened in the list without losing filters that already show it. Every pick changes the open
/// session, which the query names. Specified, with host 2 and Idle chosen and answered (b listed): picking b, which the
/// list shows, opens it and sends one query, the filters in force naming b, with host 2 and Idle still chosen; picking
/// c, idle but on host 3, opens it and sends one query, `{"selected": "c"}`, pressing All on the rail and checking All
/// in the segments; with the filters chosen again, picking a, on host 2 but running, does the same with a.
#[gpui::test]
fn a_pick_the_filters_hide_resets_both_and_a_shown_pick_keeps_them(cx: &mut TestAppContext) {
    let mut main = open_main(cx, filtered_fleet());
    main.filter_to_idle_on_host_2();

    main.pick("b");
    assert_eq!(selected(main.cx).as_deref(), Some("b"));
    assert_eq!(
        main.accept_commands(),
        [json!({ "host": 2, "status": "idle", "selected": "b" })],
        "a shown pick keeps the filters"
    );
    let nodes = frame(main.cx);
    assert!(pressed(&nodes, "host-rail-host-2") && pressed(&nodes, "session-status-Idle"));

    for (pick, hidden_by) in [("c", "its host"), ("a", "its status")] {
        main.pick(pick);
        assert_eq!(selected(main.cx).as_deref(), Some(pick));
        assert_eq!(
            main.accept_commands(),
            [json!({ "selected": pick })],
            "{pick}, hidden by {hidden_by}"
        );
        let nodes = frame(main.cx);
        assert!(pressed(&nodes, "host-rail-all"), "{pick}: the rail's All");
        assert!(
            pressed(&nodes, "session-status-All"),
            "{pick}: the segments' All"
        );
        main.helm
            .push(change(filtered(&["a", "b", "c"], 3, 3), &[], &[]));
        settle(main.cx);
        main.filter_to_idle_on_host_2();
    }
}

/// SPEC.md: "All or a host avatar returns to Sessions, as does an accepted quick-switcher pick". A pick is never
/// refused in this app (as at the reference commit), so the refused pick SPEC.md mentions has no case; what can be
/// refused is the query that widens the list, and then both choices must come back, as for any refused query.
/// Specified: with a open, host 2 and Idle chosen and answered and the Hosts panel shown, picking c (hidden by its
/// host) opens c, shows Sessions again and sends `{"selected": "c"}`; when the helm refuses it, host 2 and Idle are
/// chosen again, and the query is sent once more as host 2 and Idle naming c, since the helm still serves the query
/// that named a and would otherwise not carry the session now open, with the refusal's words still shown under the
/// heading.
#[gpui::test]
fn a_pick_leaves_the_hosts_panel_and_a_refused_widening_keeps_both_choices(
    cx: &mut TestAppContext,
) {
    let mut main = open_main(cx, filtered_fleet());
    main.filter_to_idle_on_host_2();
    click(main.cx, "hosts-panel-toggle");
    assert!(
        node(&frame(main.cx), "hosts-panel").is_some(),
        "the premise: the Hosts panel shows"
    );

    main.pick("c");
    assert_eq!(selected(main.cx).as_deref(), Some("c"));
    assert!(
        node(&frame(main.cx), "session-list").is_some(),
        "the pick showed Sessions again"
    );
    for command in main.helm.commands() {
        let refused =
            command.path == SESSION_QUERY_PATH && command.body == Some(json!({ "selected": "c" }));
        let status = if refused { 400 } else { 204 };
        main.helm.reply(command.id, status, None);
    }
    settle(main.cx);
    let nodes = frame(main.cx);
    assert!(
        pressed(&nodes, "host-rail-host-2") && pressed(&nodes, "session-status-Idle"),
        "the refused widening put both choices back"
    );
    assert_eq!(
        main.accept_commands(),
        [json!({ "host": 2, "status": "idle", "selected": "c" })],
        "the filters the helm serves are sent again, naming the picked c"
    );
    assert!(
        node(&frame(main.cx), "filter-refusal").is_some(),
        "the re-send leaves the refusal's words shown"
    );
}

// ---- New session named X -------------------------------------------------------------------------------------------

/// A name none of the fixture sessions match: no title, host or folder (`/home/user/src/<key>`) holds an `n`.
const NAME: &str = "nightly";

/// Whether the switcher's card is drawn.
fn switcher_drawn(cx: &mut VisualTestContext) -> bool {
    cx.debug_bounds("quick-switcher").is_some()
}

/// The text the launcher's Name field paints.
fn launcher_name(cx: &mut VisualTestContext) -> Vec<String> {
    texts_within(&frame(cx), "launcher-name")
        .into_iter()
        .map(|text| text.text.to_string())
        .collect()
}

/// Replace the open switcher's query with `text` and settle, for a query the test's keystrokes cannot type (spaces).
fn set_query(cx: &mut VisualTestContext, text: &str) {
    let switcher = switcher(cx);
    cx.update(|window, cx| {
        switcher.update(cx, |switcher, cx| {
            switcher.input.set_text(text.to_owned(), window, cx);
            cx.notify();
        })
    });
    test_support::settle(cx);
}

/// The New row's accessible name as the frame drew it, `None` when it is not drawn.
fn new_row_label(cx: &mut VisualTestContext) -> Option<String> {
    node(&frame(cx), NEW_ROW_ID).and_then(|row| row.label.as_deref().map(str::to_owned))
}

/// SPEC.md: "While text is typed, a pinned row below the scrolling session matches offers "New session named X". It
/// follows the sessions in arrow-key order and is selected when no session matches". The row is how a user who typed
/// a name nothing has gets to New without leaving the keyboard, so it must be reachable after the matches and never in
/// the way of the first match. Specified, over a, b and c: with nothing typed there is no New row; with `c` typed
/// (matching c's title and a's and b's folders) the New row is drawn below the scrolling matches, named `New session
/// named "c"` and helped "Open New with this name; nothing launches yet", c is selected, three presses of Down select
/// the New row (drawn selected, with b not) and a fourth stays there, and Up selects b again; with a query nothing
/// matches, "No sessions match" shows and the New row is selected; a query with spaces around it names the row by the
/// trimmed text.
#[gpui::test]
fn the_new_row_follows_the_matches_and_is_selected_when_nothing_matches(cx: &mut TestAppContext) {
    let mut main = open_main(cx, fleet());
    let read = main.open_switcher();
    main.answer(read, &[session("a"), session("b"), session("c")], false);
    assert_eq!(new_row_label(main.cx), None, "no New row without a query");

    type_query(main.cx, "c");
    assert_eq!(
        listed(main.cx),
        (vec!["c".into(), "a".into(), "b".into()], Some("c".into()))
    );
    let nodes = frame(main.cx);
    let row = node(&nodes, NEW_ROW_ID).expect("the New row is drawn");
    assert_eq!(row.label.as_deref(), Some("New session named \"c\""));
    assert_eq!(
        row.selected,
        Some(false),
        "the first match is selected, not New"
    );
    let results = node(&nodes, "quick-switcher-results").expect("the matches are drawn");
    assert!(
        row.bounds.top() >= results.bounds.bottom(),
        "the New row {:?} is pinned below the matches {:?}",
        row.bounds,
        results.bounds
    );
    assert_eq!(
        hover_help(main.cx, NEW_ROW_ID).as_deref(),
        Some(NEW_ROW_HELP)
    );

    keys(main.cx, "down down down");
    assert_eq!(
        listed(main.cx).1.as_deref(),
        Some(NEW_ROW_ID),
        "the New row follows the matches"
    );
    let nodes = frame(main.cx);
    assert_eq!(
        node(&nodes, NEW_ROW_ID).and_then(|row| row.selected),
        Some(true)
    );
    assert_eq!(
        node(&nodes, "quick-switcher-row-b").and_then(|row| row.selected),
        Some(false)
    );
    keys(main.cx, "down");
    assert_eq!(
        listed(main.cx).1.as_deref(),
        Some(NEW_ROW_ID),
        "Down stops at the New row"
    );
    keys(main.cx, "up");
    assert_eq!(listed(main.cx).1.as_deref(), Some("b"));

    type_query(main.cx, "zzz");
    assert!(
        painted(&frame(main.cx))
            .iter()
            .any(|line| line == "No sessions match")
    );
    assert_eq!(listed(main.cx), (Vec::new(), Some(NEW_ROW_ID.into())));

    set_query(main.cx, "  two words  ");
    assert_eq!(
        new_row_label(main.cx).as_deref(),
        Some("New session named \"two words\"")
    );
}

/// SPEC.md: "With a leading `tl:` (case-insensitive), sessions and the New row are replaced by matching templates".
/// The templates section is the templates area's, but the New row is this one's, and offered there it would name a
/// session after a template search. Specified: with `tl:claude`, `TL:claude` and `  Tl:` (spaces around) as the query
/// there is no New row, and Enter opens nothing; `tlx` and `t:l` still offer it.
#[gpui::test]
fn a_template_query_offers_no_new_row(cx: &mut TestAppContext) {
    let mut main = open_main(cx, fleet());
    let read = main.open_switcher();
    main.answer(read, &[session("a")], false);
    for query in ["tl:claude", "TL:claude", "  Tl:  "] {
        set_query(main.cx, query);
        assert_eq!(new_row_label(main.cx), None, "{query:?} offers no New row");
        assert_ne!(listed(main.cx).1.as_deref(), Some(NEW_ROW_ID), "{query:?}");
    }
    keys(main.cx, "enter");
    assert!(
        switcher_drawn(main.cx) && !launcher_drawn(main.cx),
        "Enter opened nothing"
    );
    for query in ["tlx", "t:l"] {
        set_query(main.cx, query);
        assert!(
            new_row_label(main.cx).is_some(),
            "{query:?} still offers the New row"
        );
    }
}

/// SPEC.md: the New row is selected when no session matches "except while the session list is still loading, when
/// arrow keys cannot reach it", and "After a failed session read, New and template search remain usable". Enter on a
/// list that has not arrived must not act on a guess, but a failed read must not strand a user who only wanted New.
/// Specified: with the read unanswered and a name typed, the New row is drawn but not selected, and Down then Enter
/// leave the switcher open with nothing opened; once the read fails, the New row is selected, and Enter closes the
/// switcher and opens New.
#[gpui::test]
fn loading_keeps_the_new_row_from_the_keys_and_a_failed_read_leaves_it_usable(
    cx: &mut TestAppContext,
) {
    let mut main = open_main(cx, fleet());
    let read = main.open_switcher();
    type_query(main.cx, NAME);
    let row = node(&frame(main.cx), NEW_ROW_ID)
        .cloned()
        .expect("the New row is drawn");
    assert_eq!(
        row.selected,
        Some(false),
        "nothing is selected while loading"
    );
    keys(main.cx, "down enter");
    assert!(
        switcher_drawn(main.cx),
        "Enter picked nothing while loading"
    );
    assert!(!launcher_drawn(main.cx));

    main.helm.reply(read, 503, Some(json!("the helm is busy")));
    test_support::settle(main.cx);
    assert_eq!(listed(main.cx).1.as_deref(), Some(NEW_ROW_ID));
    keys(main.cx, "enter");
    assert!(!switcher_drawn(main.cx), "the pick closed the switcher");
    assert!(launcher_drawn(main.cx), "and opened New");
}

/// SPEC.md: "Picking it opens New with that name", and "Closing something the user opened returns focus to where it
/// was before it opened". New refuses while any dialog is open, so the switcher must close before New opens, and New
/// then returns focus to where the switcher found it. The name is the query trimmed, as the row shows it. Specified:
/// from a's focused terminal, with `  nightly  ` as the query and its row selected, Enter closes the switcher and opens
/// New with focus inside it and `nightly` in its Name field; Escape closes New with focus back in a's terminal.
#[gpui::test]
fn enter_on_the_new_row_opens_new_named_and_closing_it_returns_focus(cx: &mut TestAppContext) {
    let mut main = open_main(cx, fleet());
    main.terminal_live();
    let read = main.open_switcher();
    main.answer(read, &[session("a"), session("b"), session("c")], false);
    set_query(main.cx, "  nightly  ");
    assert_eq!(
        listed(main.cx).1.as_deref(),
        Some(NEW_ROW_ID),
        "the premise: the New row is selected"
    );

    keys(main.cx, "enter");
    assert!(!switcher_drawn(main.cx), "the switcher closed");
    assert!(launcher_drawn(main.cx), "New opened");
    assert!(
        main.cx
            .update(|window, cx| layers::focus_layer_open(window, cx))
            && !terminal_focused(main.cx),
        "focus is inside New"
    );
    assert_eq!(
        launcher_name(main.cx),
        [NAME],
        "the name is the trimmed query"
    );

    keys(main.cx, "escape");
    assert!(!any_open(main.cx));
    assert!(
        terminal_focused(main.cx),
        "focus went back to where the switcher found it"
    );
}

/// The pointer's way to the same row: the old row took clicks, and SPEC.md has an accepted New return the sidebar to
/// Sessions ("All or a host avatar returns to Sessions, as does an accepted quick-switcher pick or New handoff"). The
/// Hosts panel is what tells a click that goes through the list's New from one that opens the launcher directly.
/// Specified: with the Hosts panel shown, a click on the New row for `nightly` closes the switcher, opens New, and
/// shows the list's column again in place of the panel.
#[gpui::test]
fn a_click_on_the_new_row_opens_new_and_returns_from_the_hosts_panel(cx: &mut TestAppContext) {
    let mut main = open_main(cx, fleet());
    click(main.cx, "hosts-panel-toggle");
    assert!(
        node(&frame(main.cx), "hosts-panel").is_some(),
        "the premise: the Hosts panel shows"
    );
    let read = main.open_switcher();
    main.answer(read, &[session("a"), session("b"), session("c")], false);
    type_query(main.cx, NAME);
    click(main.cx, NEW_ROW_ID);
    assert!(!switcher_drawn(main.cx));
    assert!(launcher_drawn(main.cx));
    let nodes = frame(main.cx);
    assert!(node(&nodes, "hosts-panel").is_none(), "the panel went");
    assert!(
        node(&nodes, "session-list").is_some(),
        "Sessions shows again"
    );
}

/// SPEC.md: the switcher "works during a pending launch", and a pick of the New row while New is unavailable (a launch
/// pending, a host change in flight) must not leave the switcher up with a dead row or open anything; the old switcher
/// closed and gave focus back. A host probe stands in for the pending launch, as both make New unavailable through the
/// one answer. Specified: from a's focused terminal with Add host's probe in flight, Enter on the New row closes the
/// switcher, opens nothing, and leaves focus in a's terminal.
#[gpui::test]
fn a_refused_new_closes_the_switcher_and_returns_focus(cx: &mut TestAppContext) {
    let mut main = open_main(cx, fleet());
    main.terminal_live();
    let read = main.open_switcher();
    main.answer(read, &[session("a")], false);
    // Started once the switcher is open: opening answers every other command the window sent, which would settle it.
    let probe = start_host_change(main.cx, HostChange::Probe);
    assert!(
        main.helm
            .commands()
            .iter()
            .any(|command| command.path == probe),
        "the premise: the probe is in flight"
    );
    type_query(main.cx, NAME);
    keys(main.cx, "enter");
    assert!(!switcher_drawn(main.cx), "the switcher closed");
    assert!(!launcher_drawn(main.cx), "New was refused");
    assert!(!any_open(main.cx));
    assert!(terminal_focused(main.cx), "focus is back in a's terminal");
}
