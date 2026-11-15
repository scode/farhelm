//! Tests of the composer search box in the real launcher over the helm client's fake: what typing shows, what Enter,
//! the arrows, Escape and a click on a row do, and which rows a mode refuses. Which rows a query offers, and in what
//! order, is `search_tests`' (the search's own rules, without a window); these tests show that the box draws those
//! rows and that taking one makes the same edit as the hand control it stands for.
//!
//! Every step ends in one of the shared harness's step helpers, so the focus and layer invariants are checked after
//! each.

use farhelm_proto::{LaunchHarness, LaunchPermission, RestartOffer};
use gpui::{TestAppContext, VisualTestContext};

use super::draft::{LaunchDraft, Tab};
use super::view_tests::{
    answer_catalog, codex, draft, fixture_hosts, launcher_on, launcher_with_hosts, name_of, read,
    shown_refusal,
};
use crate::test_support::{self, keys, settle};

/// Whether the search box holds focus.
fn search_focused(cx: &mut VisualTestContext) -> bool {
    read(cx, |launcher, window, cx| {
        launcher.search.focus_handle(cx).is_focused(window)
    })
}

/// What the search box holds.
fn search_text(cx: &mut VisualTestContext) -> String {
    read(cx, |launcher, _, cx| launcher.search.text(cx).to_string())
}

/// Whether the search results were drawn in the last frame.
fn results_shown(cx: &mut VisualTestContext) -> bool {
    test_support::frame(cx);
    cx.debug_bounds("launcher-search-results").is_some()
}

/// Type `text` into the focused field, as one input, and let it settle.
fn type_in(cx: &mut VisualTestContext, text: &str) {
    cx.simulate_input(text);
    settle(cx);
}

/// New on host1 at `~`, with focus where the launcher starts it.
fn new_launcher(
    cx: &mut TestAppContext,
) -> (farhelm_helm_client::fake::FakeHelm, &mut VisualTestContext) {
    launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None))
}

/// SPEC.md, "Creation": New "starts focus on search", and a word and Enter make one choice: typing `codex` shows the
/// results with the agent type, and Enter chooses it, empties the box, closes the results and leaves focus in the box
/// for the next word. Why: this is the keyboard path a whole setup is built by, word, Enter, word, Enter.
#[gpui::test]
fn a_word_and_enter_choose_it_and_empty_the_box(cx: &mut TestAppContext) {
    let (_helm, cx) = new_launcher(cx);
    assert!(search_focused(cx), "focus starts in the search box");
    type_in(cx, "codex");
    assert!(results_shown(cx), "typing shows the results");
    assert_eq!(
        name_of(cx, "launcher-search-option-0").as_deref(),
        Some("Harness: Codex")
    );
    keys(cx, "enter");
    assert_eq!(draft(cx).agent.harness, Some(LaunchHarness::Codex));
    assert_eq!(search_text(cx), "", "the box is emptied");
    assert!(!results_shown(cx), "and the results close");
    assert!(search_focused(cx), "focus stays in the box");
}

/// Text that matches nothing offers "Set session name", and Enter names the session, the name field showing it. Why:
/// typing a name and pressing Enter is how the old launcher named a session from search (SPEC.md: "the visible Set
/// session name row for unmatched text").
#[gpui::test]
fn unmatched_text_and_enter_name_the_session(cx: &mut TestAppContext) {
    let (_helm, cx) = new_launcher(cx);
    type_in(cx, "fix parser");
    assert_eq!(
        name_of(cx, "launcher-search-option-0").as_deref(),
        Some("Set session name: fix parser")
    );
    keys(cx, "enter");
    assert_eq!(draft(cx).name.value(), "fix parser");
    assert_eq!(
        read(cx, |launcher, _, cx| launcher.name.text(cx).to_string()),
        "fix parser",
        "the name field follows the draft"
    );
    // The box is empty again, so a second Enter presses the main action (the old "Clone search Enter picks an
    // unmatched name before a second Enter launches"); this New has no agent type, so it says why instead.
    keys(cx, "enter");
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Not launched. Choose a structured harness before launching")
    );
}

/// Before an agent type is chosen, `yolo` (bare, or after `perms:`) chooses YOLO and leaves the agent unchosen. Why:
/// SPEC.md, "Creation": "On the Agent tab, Guarded and YOLO can also be chosen before a harness, by their bare words
/// or `perms:` choices" (the old `${query} selects YOLO before choosing a harness`).
#[gpui::test]
fn yolo_from_search_before_an_agent_type(cx: &mut TestAppContext) {
    for query in ["yolo", "perms:yolo"] {
        let (_helm, cx) = new_launcher(cx);
        type_in(cx, query);
        keys(cx, "enter");
        let draft = draft(cx);
        assert_eq!(
            draft.agent.permissions,
            Some(LaunchPermission::Yolo),
            "{query}"
        );
        assert_eq!(draft.agent.harness, None, "{query}");
    }
}

/// Enter on an empty box presses the main action, so an incomplete draft says why beside it (SPEC.md: "Enter on an
/// empty box launches only a complete selection"); a box holding only spaces is a query, and says it has nothing to
/// offer instead of pressing. Why: Enter in the box must never launch by accident while a query is in it.
#[gpui::test]
fn enter_on_an_empty_box_presses_launch_and_spaces_do_not(cx: &mut TestAppContext) {
    let (_helm, cx) = new_launcher(cx);
    type_in(cx, "  ");
    keys(cx, "enter");
    assert_eq!(shown_refusal(cx), None, "spaces are a query, not a press");
    assert_eq!(
        name_of(cx, "launcher-search-message").as_deref(),
        Some("Type a search or clear the search box to launch")
    );
    keys(cx, "backspace backspace enter");
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Not launched. Choose a structured harness before launching")
    );
}

/// The arrows move the highlighted row, wrapping, and Enter takes the highlighted one; a click takes the row clicked.
/// `c` names no row exactly, so the first row is highlighted at first, and Up from it wraps to the last. Why: Enter
/// must take the row the user moved to, which is the one drawn highlighted.
#[gpui::test]
fn arrows_move_the_highlight_and_enter_or_a_click_takes_a_row(cx: &mut TestAppContext) {
    let (_helm, cx) = new_launcher(cx);
    type_in(cx, "c");
    assert_eq!(
        name_of(cx, "launcher-search-option-1").as_deref(),
        Some("Harness: Claude"),
        "the premise: Claude is the second row"
    );
    keys(cx, "down enter");
    assert_eq!(draft(cx).agent.harness, Some(LaunchHarness::Claude));
    type_in(cx, "c");
    let rows = read(cx, |launcher, _, cx| launcher.search_results(cx).rows);
    assert_eq!(
        rows.last(),
        Some(&super::search::Row::Harness(LaunchHarness::OpenCode)),
        "the premise: OpenCode is the last row"
    );
    keys(cx, "up enter");
    assert_eq!(draft(cx).agent.harness, Some(LaunchHarness::OpenCode));
    type_in(cx, "cod");
    test_support::click(cx, "launcher-search-option-0");
    assert_eq!(draft(cx).agent.harness, Some(LaunchHarness::Codex));
    assert!(search_focused(cx), "a click leaves focus in the box");
}

/// Escape closes open results and keeps what was typed; with them closed it is the dialog's, and closes the launcher.
/// Enter on closed results opens them again rather than taking a row the user cannot see. Why: SPEC.md, "Escape or a
/// click outside closes the rows without clearing the query".
#[gpui::test]
fn escape_closes_the_results_and_keeps_the_query(cx: &mut TestAppContext) {
    let (_helm, cx) = new_launcher(cx);
    type_in(cx, "cod");
    keys(cx, "escape");
    assert!(!results_shown(cx), "Escape closes the results");
    assert_eq!(search_text(cx), "cod", "and keeps the query");
    assert_eq!(draft(cx).agent.harness, None);
    keys(cx, "enter");
    assert!(results_shown(cx), "Enter opens them again");
    assert_eq!(draft(cx).agent.harness, None, "without taking a row");
    keys(cx, "escape escape");
    assert!(
        cx.update(|_, cx| cx
            .try_global::<super::view_tests::Opened>()
            .is_none_or(|opened| opened.0.upgrade().is_none())),
        "Escape with the results closed closes the launcher"
    );
}

/// A labelled query with no match says why in the results, in the warning ink, and Enter changes nothing. Why: a
/// scoped query that matched nothing must not quietly become a name.
#[gpui::test]
fn a_scoped_miss_says_why_and_enter_changes_nothing(cx: &mut TestAppContext) {
    let (_helm, cx) = new_launcher(cx);
    type_in(cx, "host:nowhere");
    assert_eq!(
        name_of(cx, "launcher-search-message").as_deref(),
        Some("No host matches")
    );
    let before = draft(cx);
    keys(cx, "enter");
    assert_eq!(draft(cx), before, "nothing changed");
    assert_eq!(search_text(cx), "host:nowhere", "and the query stays");
}

/// `host:` rows choose the host as the host picker does, and `folder:` a path as typing it would. Why: these are the
/// destination's hand edits, reached from the keyboard.
#[gpui::test]
fn host_and_folder_rows_set_the_destination(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::new(Some(1), "~".into(), None),
        fixture_hosts(&[0, 1]),
    );
    type_in(cx, "host:host2");
    keys(cx, "enter");
    assert_eq!(draft(cx).host, Some(2));
    type_in(cx, "folder:/src/app");
    keys(cx, "enter");
    assert_eq!(draft(cx).folder.value(), "/src/app");
    assert_eq!(
        read(cx, |launcher, _, cx| launcher.folder.text(cx).to_string()),
        "/src/app",
        "the folder field follows the draft"
    );
}

/// An agent type or model taken on the Command tab switches to the Agent tab, where it belongs, even a model of the
/// agent type already chosen (which changes no agent). Why: SPEC.md, "Command launch": search on that tab "still
/// offers agent types, models and recent setups, which switch to the Agent tab".
#[gpui::test]
fn a_model_taken_on_the_command_tab_switches_to_the_agent_tab(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_launcher(cx);
    answer_catalog(
        &mut helm,
        cx,
        Some(serde_json::json!([{ "id": "gpt-5", "harness": "codex", "efforts": [] }])),
    );
    test_support::click(cx, "launcher-tabs-Command");
    assert_eq!(
        draft(cx).tab,
        Tab::Command,
        "the premise: the click chose Command"
    );
    test_support::click(cx, "launcher-search-row");
    assert!(search_focused(cx), "the premise: the click focused the box");
    type_in(cx, "codex");
    keys(cx, "enter");
    let chosen = draft(cx);
    assert_eq!(chosen.agent.harness, Some(LaunchHarness::Codex));
    assert_eq!(
        chosen.tab,
        Tab::Agent,
        "an agent type taken on Command switches to Agent"
    );
    // Back on Command with Codex chosen, a Codex model changes no agent: only the row's own switch moves the tab.
    test_support::click(cx, "launcher-tabs-Command");
    assert_eq!(draft(cx).tab, Tab::Command, "the premise: back on Command");
    test_support::click(cx, "launcher-search-row");
    type_in(cx, "gpt-5");
    keys(cx, "enter");
    let draft = draft(cx);
    assert_eq!(draft.tab, Tab::Agent);
    assert_eq!(draft.agent.harness, Some(LaunchHarness::Codex));
    assert_eq!(draft.agent.model.as_deref(), Some("gpt-5"));
}

/// Enter on an empty box launches a complete draft, once; a query that offers nothing sends nothing; and a held Enter
/// right after taking a row, which empties the box, sends nothing either (SPEC.md: "A held Enter never launches").
/// Why: the auto-repeat of the Enter that took a row would otherwise land on the emptied box and launch the setup
/// the user was still building.
#[gpui::test]
fn only_a_fresh_enter_on_an_empty_box_launches(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, super::launch_tests::command_draft());
    super::launch_tests::creates(&mut helm, cx);
    assert!(search_focused(cx), "the premise: focus starts in the box");
    type_in(cx, "host:nowhere");
    assert_eq!(
        name_of(cx, "launcher-search-message").as_deref(),
        Some("No host matches"),
        "the premise: the query offers nothing"
    );
    keys(cx, "enter");
    assert!(
        super::launch_tests::creates(&mut helm, cx).is_empty(),
        "a query that offers nothing sends nothing"
    );
    for _ in 0.."host:nowhere".len() {
        keys(cx, "backspace");
    }
    type_in(cx, "fix parser");
    keys(cx, "enter");
    assert_eq!(
        draft(cx).name.value(),
        "fix parser",
        "the premise: Enter took the row"
    );
    assert!(
        search_focused(cx) && search_text(cx).is_empty(),
        "the premise: the box has focus and is empty"
    );
    cx.simulate_event(gpui::KeyDownEvent {
        native_key: None,
        keystroke: gpui::Keystroke::parse("enter").unwrap(),
        is_held: true,
        prefer_character_input: false,
    });
    settle(cx);
    assert!(
        super::launch_tests::creates(&mut helm, cx).is_empty(),
        "a held Enter sends nothing"
    );
    keys(cx, "enter");
    assert_eq!(
        super::launch_tests::creates(&mut helm, cx).len(),
        1,
        "a fresh Enter on the empty box launches"
    );
}

/// Custom taken from search moves focus on to its flags, as choosing it by hand does; every other row leaves focus in
/// the box. Why: Custom is useless without its flags, and the old launcher moved focus there the same way.
#[gpui::test]
fn custom_from_search_moves_focus_to_its_flags(cx: &mut TestAppContext) {
    let (_helm, cx) = new_launcher(cx);
    type_in(cx, "codex");
    keys(cx, "enter");
    type_in(cx, "perms:custom");
    keys(cx, "enter");
    assert_eq!(draft(cx).agent.permissions, Some(LaunchPermission::Custom));
    settle(cx);
    assert!(
        read(cx, |launcher, window, cx| launcher
            .flags
            .focus_handle(cx)
            .is_focused(window)),
        "focus moved to the flags"
    );
}

/// Restart with refuses a row that would change what it holds, even one naming the current value, and says why above
/// the box in the old words; the box keeps its text and the draft is unchanged. Rows it does not hold still apply.
/// Why: SPEC.md, "Restart with": "Search results and recent setups that set a held field are still refused even when
/// they name its current value".
#[gpui::test]
fn restart_with_refuses_a_held_field_and_says_why(cx: &mut TestAppContext) {
    let mut source = super::draft_tests::session(codex(None, None, None));
    source.restart_offer = RestartOffer::Resume;
    let restart = LaunchDraft::restart_with("missing", &source, Some(1)).unwrap();
    let (_helm, cx) = launcher_on(cx, restart);
    test_support::click(cx, "launcher-search-row");
    assert!(search_focused(cx), "the premise: the click focused the box");
    let before = draft(cx);
    type_in(cx, "codex");
    keys(cx, "enter");
    assert_eq!(
        name_of(cx, "launcher-search-refusal").as_deref(),
        Some("agent type cannot change here: this dialog keeps agent type fixed for restart")
    );
    assert_eq!(draft(cx), before, "nothing changed");
    assert_eq!(search_text(cx), "codex", "the box keeps its text");
    assert!(
        !results_shown(cx),
        "the results close, so they cannot cover the reason"
    );
    keys(cx, "backspace backspace backspace backspace backspace");
    type_in(cx, "folder:/elsewhere");
    keys(cx, "enter");
    assert_eq!(
        name_of(cx, "launcher-search-refusal").as_deref(),
        Some("destination cannot change here: this dialog keeps destination fixed for restart")
    );
    for _ in 0.."folder:/elsewhere".len() {
        keys(cx, "backspace");
    }
    type_in(cx, "name:x");
    keys(cx, "enter");
    assert_eq!(
        name_of(cx, "launcher-search-refusal").as_deref(),
        Some("session name cannot change here: this dialog keeps session name fixed for restart")
    );
    for _ in 0.."name:x".len() {
        keys(cx, "backspace");
    }
    assert_eq!(
        name_of(cx, "launcher-search-refusal"),
        None,
        "typing takes the refusal away"
    );
    type_in(cx, "yolo");
    keys(cx, "enter");
    assert_eq!(
        draft(cx).agent.permissions,
        Some(LaunchPermission::Yolo),
        "a permission is not held"
    );
}

/// Clone and Replace with start focus in the search box too; only Restart with starts on Cancel. Why: SPEC.md,
/// "Ordinary launches still start focus on search", and the old Clone and Replace with tests typed into search first.
#[gpui::test]
fn clone_and_replace_with_start_focus_in_the_search_box(cx: &mut TestAppContext) {
    let source = super::draft_tests::session(codex(None, None, None));
    for opened in [
        LaunchDraft::clone_of(&source, Some(1)),
        LaunchDraft::replace_with("a", &source, Some(1)),
    ] {
        let (_helm, cx) = launcher_on(cx, opened);
        assert!(search_focused(cx), "focus starts in the search box");
    }
}

/// Replace with keeps its host fixed, so a host row is refused above the box with Replace with's word, and the host
/// stays (a recorded deviation: the old search did not refuse it). Why: Replace with keeps its source's host (SPEC.md:
/// "Replace with can change the harness or folder", not the host), so a search must not move it.
#[gpui::test]
fn replace_with_refuses_a_host_row(cx: &mut TestAppContext) {
    let source = super::draft_tests::session(codex(None, None, None));
    let (_helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::replace_with("a", &source, Some(1)),
        fixture_hosts(&[0, 1]),
    );
    type_in(cx, "host:host2");
    keys(cx, "enter");
    assert_eq!(
        name_of(cx, "launcher-search-refusal").as_deref(),
        Some("host cannot change here: this dialog keeps host fixed for replace")
    );
    assert_eq!(draft(cx).host, Some(1), "the host stays");
}
