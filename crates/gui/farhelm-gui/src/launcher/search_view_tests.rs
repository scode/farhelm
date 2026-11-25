//! Tests of the composer search box in the real launcher over the helm client's fake: what typing shows, what Enter,
//! the arrows, Escape and a click on a row do, and which rows a mode refuses. Which rows a query offers, and in what
//! order, is `search_tests`' (the search's own rules, without a window); these tests show that the box draws those
//! rows and that taking one makes the same edit as the hand control it stands for.
//!
//! Every step ends in one of the shared harness's step helpers, so the focus and layer invariants are checked after
//! each.

use farhelm_proto::launcher::LaunchTemplate;
use farhelm_proto::ui_channel::HostView;
use farhelm_proto::{LaunchEffort, LaunchHarness, LaunchPermission, RestartOffer};
use gpui::{TestAppContext, VisualTestContext};

use super::draft::{LaunchDraft, Tab};
use super::recents_view_tests::{
    answer_history, chosen, creates, history, launched, new_in_tmp, two_setups,
};
use super::view_tests::{
    answer_catalog, click_in_body, codex, draft, fixture_hosts, launcher_on, launcher_with_hosts,
    name_of, read, retarget_history_host, search_focused, search_text, selected_state,
    shown_refusal,
};
use crate::layers;
use crate::test_support::{self, keys, settle, withdraw_presented_choice};

/// Whether the search results were drawn in the last frame.
fn results_shown(cx: &mut VisualTestContext) -> bool {
    test_support::frame(cx);
    cx.debug_bounds("launcher-search-results").is_some()
}

/// The rejection is beneath the command field with the query still ready to edit.
pub(super) fn assert_stale_choice(cx: &mut VisualTestContext, query: &str) {
    assert_eq!(
        name_of(cx, "launcher-search-refusal").as_deref(),
        Some("That choice is no longer available; choose a current result")
    );
    assert_eq!(search_text(cx), query);
    assert!(search_focused(cx));
    assert!(!results_shown(cx), "the results cannot cover the warning");
}

/// Type `text` into the focused field, as one input, and let it settle.
pub(super) fn type_in(cx: &mut VisualTestContext, text: &str) {
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

/// An exact shell word preselects its result, and the second Enter sends
/// the normal create. An existing Guarded choice belongs to the agent draft
/// and must never become a shell option or trigger an escalation banner.
fn assert_shell_shortcut(cx: &mut TestAppContext, word: &str, harness: LaunchHarness) {
    let (mut helm, cx) = new_launcher(cx);
    assert!(search_focused(cx));
    type_in(cx, "codex");
    keys(cx, "enter");
    assert_eq!(draft(cx).agent.permissions, Some(LaunchPermission::Guarded));
    test_support::frame(cx);
    assert!(cx.debug_bounds("launcher-permissions-row").is_some());

    type_in(cx, word);
    assert_eq!(search_text(cx), word);
    assert!(results_shown(cx), "typing a shell word opens its results");
    let results = read(cx, |launcher, _, cx| launcher.search_results(cx));
    assert_eq!(
        results.rows.get(results.default),
        Some(&super::search::Row::Harness(harness))
    );
    assert_eq!(
        results.rows[results.default].group(),
        super::search::Group::Shells
    );
    keys(cx, "enter");
    assert_eq!(draft(cx).agent.harness, Some(harness));
    assert_eq!(draft(cx).agent.permissions, None);
    assert!(search_focused(cx));
    assert_eq!(search_text(cx), "");
    test_support::frame(cx);
    assert!(cx.debug_bounds("launcher-permissions-row").is_none());
    assert!(cx.debug_bounds("launcher-escalation").is_none());
    keys(cx, "enter");
    let sent = super::launch_tests::creates(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("expected one create for {word}, got {}", sent.len());
    };
    let launch = &create.body.as_ref().unwrap()["launch"];
    assert_eq!(launch["harness"], word);
    assert!(launch["permissions"].is_null());
}

/// The Bash shortcut follows the normal create path and clears agent options.
#[gpui::test]
fn exact_bash_word_picks_and_launches_without_agent_options(cx: &mut TestAppContext) {
    assert_shell_shortcut(cx, "bash", LaunchHarness::Bash);
}

/// Zsh reaches the same keyboard path with its own launch type on the wire.
#[gpui::test]
fn exact_zsh_word_picks_and_launches_without_agent_options(cx: &mut TestAppContext) {
    assert_shell_shortcut(cx, "zsh", LaunchHarness::Zsh);
}

/// Composer search must draw its older-model group while keeping the group's rows in keyboard order.
/// A partial model query lets Down move from the current match through the heading to the older match.
#[gpui::test]
fn model_search_draws_the_older_group_and_selects_with_enter(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_launcher(cx);
    answer_catalog(
        &mut helm,
        cx,
        Some(serde_json::json!([
            {"id":"sol-old", "family":"sol", "latest":false, "harness":"codex", "efforts":["high"]},
            {"id":"sol-current", "family":"sol", "latest":true, "harness":"codex", "efforts":["high"]},
        ])),
    );
    assert!(search_focused(cx));
    type_in(cx, "model:sol");
    assert!(results_shown(cx));
    assert_eq!(
        name_of(cx, "launcher-search-option-0").as_deref(),
        Some("Model: sol-current (Codex)")
    );
    assert_eq!(
        name_of(cx, "launcher-search-group-Older models").as_deref(),
        Some("Older models")
    );
    assert_eq!(
        name_of(cx, "launcher-search-option-1").as_deref(),
        Some("Model: sol-old (Codex)")
    );

    keys(cx, "down enter");
    assert_eq!(draft(cx).agent.harness, Some(LaunchHarness::Codex));
    assert_eq!(draft(cx).agent.model.as_deref(), Some("sol-old"));
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
        cx.update(|_, cx| layers::last_opened::<super::view::Launcher>(cx).is_none()),
        "Escape with the results closed closes the launcher"
    );
}

/// The narrow popover closes through the layer manager. Reopening starts at the query's default, with no edit yet.
#[gpui::test]
fn reopening_the_narrow_popover_drops_its_old_arrow_pick(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_launcher_with_templates(cx, None);
    cx.simulate_resize(crate::bootstrap::MIN_WINDOW_SIZE);
    settle(cx);
    answer_catalog(&mut helm, cx, Some(serde_json::json!([])));
    type_in(cx, "tl:");
    keys(cx, "down");
    assert_eq!(selected_state(cx, "launcher-search-option-1"), Some(true));
    keys(cx, "escape");
    assert!(!results_shown(cx));
    let before = draft(cx);
    keys(cx, "enter");
    assert!(results_shown(cx));
    assert_eq!(selected_state(cx, "launcher-search-option-0"), Some(true));
    assert_eq!(draft(cx), before);
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

/// Arrow navigation scrolls the chosen result into view. Going through an unmatched scoped query and back
/// restores the first result as the selection, so Enter cannot use the previous position.
#[gpui::test]
fn navigation_reveals_the_highlight_and_a_miss_clears_it(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_launcher(cx);
    let models: Vec<_> = (0..24)
        .map(|n| {
            serde_json::json!({
                "id": format!("scroll-model-{n:02}"), "harness": "codex", "efforts": []
            })
        })
        .collect();
    answer_catalog(&mut helm, cx, Some(serde_json::Value::Array(models)));
    assert!(search_focused(cx), "the search has focus");
    type_in(cx, "scroll-model");
    assert_eq!(
        read(cx, |launcher, _, cx| launcher.search_results(cx).rows.len()),
        24
    );
    let target = |cx: &mut VisualTestContext| {
        test_support::frame(cx)
            .into_iter()
            .find(|node| node.element_id.as_deref() == Some("launcher-search-option-20"))
            .expect("the catalog contributed the distant row")
    };
    let before = target(cx);
    assert_ne!(
        before.visible_bounds, before.bounds,
        "the row begins below the clip"
    );
    for _ in 0..20 {
        keys(cx, "down");
    }
    let after = target(cx);
    assert_eq!(after.selected, Some(true));
    assert_eq!(
        after.visible_bounds, after.bounds,
        "the whole highlighted row is shown"
    );

    crate::test_support::select_all_text(cx);
    type_in(cx, "host:no-such-composer-result");
    assert_eq!(
        name_of(cx, "launcher-search-message").as_deref(),
        Some("No host matches")
    );
    assert!(test_support::frame(cx).iter().all(|node| {
        !node
            .element_id
            .as_deref()
            .is_some_and(|id| id.starts_with("launcher-search-option-"))
    }));
    assert!(search_focused(cx));
    crate::test_support::select_all_text(cx);
    type_in(cx, "scroll-model");
    assert_eq!(selected_state(cx, "launcher-search-option-0"), Some(true));
    assert_eq!(
        target(cx).selected,
        Some(false),
        "the old position is no longer selected"
    );
}

/// A defaults-only setup replaces all explicit model, effort and permission choices. Accepting its search result
/// fills the draft and leaves Launch to a separate press.
#[gpui::test]
fn a_defaults_only_setup_clears_explicit_choices_without_launching(cx: &mut TestAppContext) {
    let saved = history(vec![launched(
        "/saved-defaults",
        &chosen(LaunchHarness::Codex, None, None, None),
    )]);
    let (mut helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "/tmp".into(), None));
    let catalog =
        serde_json::json!([{ "id": "custom-model", "harness": "codex", "efforts": ["high"] }]);
    assert!(
        super::launch_tests::creates_answering_with(&mut helm, cx, &catalog, &saved)
            .0
            .is_empty()
    );
    for query in [
        "harness:codex",
        "model:custom-model",
        "effort:high",
        "perms:yolo",
    ] {
        assert!(search_focused(cx));
        type_in(cx, query);
        keys(cx, "enter");
    }
    let before = draft(cx);
    assert_eq!(before.agent.model.as_deref(), Some("custom-model"));
    assert_eq!(before.agent.effort, Some(LaunchEffort::High));
    assert_eq!(before.agent.permissions, Some(LaunchPermission::Yolo));

    type_in(cx, "recent:saved-defaults");
    assert!(
        name_of(cx, "launcher-search-option-0")
            .is_some_and(|label| label.contains("/saved-defaults"))
    );
    keys(cx, "enter");
    let after = draft(cx);
    assert_eq!(after.folder.value(), "/saved-defaults");
    assert_eq!(after.agent.harness, Some(LaunchHarness::Codex));
    assert_eq!((after.agent.model, after.agent.effort), (None, None));
    // Codex's request permission defaults to Guarded after it drops the earlier explicit YOLO.
    assert_eq!(after.agent.permissions, Some(LaunchPermission::Guarded));
    assert!(creates(&mut helm, cx).is_empty());
}

/// `host:` rows choose the host as the host picker does. Taking a typed path in search edits only the folder,
/// so the user can choose a destination without losing the Command tab or the command being written.
#[gpui::test]
fn host_and_folder_rows_set_the_destination(cx: &mut TestAppContext) {
    let mut start = LaunchDraft::new(Some(1), "~".into(), None);
    start.choose_tab(Tab::Command);
    start.edit_command("make watch".into());
    let (_helm, cx) = launcher_with_hosts(cx, start, fixture_hosts(&[0, 1]));
    type_in(cx, "host:host2");
    keys(cx, "enter");
    assert_eq!(draft(cx).host, Some(2));
    let before = draft(cx);
    assert_eq!(before.tab, Tab::Command);
    assert_eq!(before.command.command, "make watch");
    type_in(cx, "/src/app");
    let rows = read(cx, |launcher, _, cx| launcher.search_results(cx).rows);
    assert_eq!(
        rows.first(),
        Some(&super::search::Row::UsePath("/src/app".into()))
    );
    test_support::click(cx, "launcher-search-option-0");
    let after = draft(cx);
    assert_eq!(after.folder.value(), "/src/app");
    assert_eq!(after.tab, Tab::Command);
    assert_eq!(after.command.command, "make watch");
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

/// Specified: taking `name:fix-login` on Command keeps that tab and its typed `make watch` command while setting the
/// name. Why: naming is shared by both tabs and must not throw away the command being written ("search on the command
/// tab switches to the agent tab only for an agent choice", `e2e/tests/launcher-tabs.spec.ts` at `24bc0580`, its `name:`
/// half).
#[gpui::test]
fn a_name_row_on_the_command_tab_keeps_the_tab_and_its_command(cx: &mut TestAppContext) {
    let (_helm, cx) = new_launcher(cx);
    test_support::click(cx, "launcher-tabs-Command");
    assert_eq!(
        draft(cx).tab,
        Tab::Command,
        "the premise: Command is chosen"
    );
    click_in_body(cx, "launcher-command");
    type_in(cx, "make watch");
    test_support::click(cx, "launcher-search-row");
    assert!(search_focused(cx), "the premise: the search box has focus");
    type_in(cx, "name:fix-login");
    keys(cx, "enter");
    let chosen = draft(cx);
    assert_eq!(chosen.tab, Tab::Command);
    assert_eq!(chosen.name.value(), "fix-login");
    assert_eq!(chosen.command.command, "make watch");
}

/// Specified: words and Enter in one search box choose Codex, `gpt-5`, High, Default effort, High again, YOLO and
/// Guarded; each choice leaves the emptied box focused, and a final Enter sends one create carrying Codex, `gpt-5`,
/// High and Guarded. Why: separate picker tests do not prove that the whole keyboard path reaches the ordinary launch
/// with its choices intact ("composer word search drives a structured launch", `e2e/tests/composer-word-search.spec.ts`
/// at `24bc0580`).
#[gpui::test]
fn the_whole_keyboard_path_builds_a_launch_and_sends_it(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_launcher(cx);
    answer_catalog(
        &mut helm,
        cx,
        Some(
            serde_json::json!([{ "id": "gpt-5", "harness": "codex", "efforts": ["low", "high"] }]),
        ),
    );
    assert!(search_focused(cx), "the premise: search starts focused");
    type_in(cx, "codex");
    keys(cx, "enter");
    assert_eq!(draft(cx).agent.harness, Some(LaunchHarness::Codex));
    assert_eq!(search_text(cx), "");
    assert!(search_focused(cx), "after codex");

    type_in(cx, "gpt-5");
    keys(cx, "enter");
    assert_eq!(draft(cx).agent.model.as_deref(), Some("gpt-5"));
    assert_eq!(search_text(cx), "");
    assert!(search_focused(cx), "after gpt-5");

    type_in(cx, "high");
    keys(cx, "enter");
    assert_eq!(draft(cx).agent.effort, Some(LaunchEffort::High));
    assert_eq!(search_text(cx), "");
    assert!(search_focused(cx), "after high");

    type_in(cx, "effort:default");
    keys(cx, "enter");
    assert_eq!(draft(cx).agent.effort, None);
    assert_eq!(search_text(cx), "");
    assert!(search_focused(cx), "after effort:default");

    type_in(cx, "high");
    keys(cx, "enter");
    assert_eq!(draft(cx).agent.effort, Some(LaunchEffort::High));
    assert_eq!(search_text(cx), "");
    assert!(search_focused(cx), "after high again");

    type_in(cx, "perms:yolo");
    keys(cx, "enter");
    assert_eq!(draft(cx).agent.permissions, Some(LaunchPermission::Yolo));
    assert_eq!(search_text(cx), "");
    assert!(search_focused(cx), "after perms:yolo");

    type_in(cx, "guarded");
    keys(cx, "enter");
    assert_eq!(draft(cx).agent.permissions, Some(LaunchPermission::Guarded));
    assert_eq!(
        search_text(cx),
        "",
        "the premise: the launch Enter sees an empty box"
    );
    assert!(search_focused(cx), "after guarded");

    keys(cx, "enter");
    let sent = super::launch_tests::creates(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("one create, not {}", sent.len());
    };
    let launch = &create.body.as_ref().unwrap()["launch"];
    assert_eq!(launch["harness"], serde_json::json!("codex"));
    assert_eq!(launch["model"], serde_json::json!("gpt-5"));
    assert_eq!(launch["effort"], serde_json::json!("high"));
    assert_eq!(launch["permissions"], serde_json::json!("guarded"));
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
    test_support::press(cx, "enter", true);
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
        Some("Agent type cannot change here: this dialog keeps agent type fixed for restart")
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
        Some("Destination cannot change here: this dialog keeps destination fixed for restart")
    );
    for _ in 0.."folder:/elsewhere".len() {
        keys(cx, "backspace");
    }
    type_in(cx, "name:x");
    keys(cx, "enter");
    assert_eq!(
        name_of(cx, "launcher-search-refusal").as_deref(),
        Some("Session name cannot change here: this dialog keeps session name fixed for restart")
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

/// The example in the empty header uses a short query that still selects its host. A full SSH login and domain
/// would consume space meant for the other examples.
#[gpui::test]
fn the_command_field_shows_a_searchable_short_host_example(cx: &mut TestAppContext) {
    let mut hosts = fixture_hosts(&[0, 1]);
    let host = &mut hosts.entries[1].value;
    host["name"] = serde_json::json!("deploy@buildbox.example.invalid");
    host["alias"] = serde_json::Value::Null;
    host["destination"] = host["name"].clone();
    let (_helm, cx) = launcher_with_hosts(cx, LaunchDraft::new(Some(1), "~".into(), None), hosts);
    assert!(
        crate::session_list::tests::painted(&test_support::frame(cx))
            .iter()
            .any(|text| text == "host:buildbox"),
        "the header offers the actual host's short name"
    );
    assert!(search_focused(cx), "typing goes to the command editor");
    type_in(cx, "host:buildbox");
    assert!(
        name_of(cx, "launcher-search-option-0")
            .is_some_and(|name| name.contains("deploy@buildbox.example.invalid")),
        "the displayed example matched the host"
    );
    keys(cx, "enter");
    assert_eq!(draft(cx).host, Some(2));
}

/// A host result can be withdrawn between paint and click. In a narrow launcher the old popover must close so the
/// inline warning is visible, with the query and destination still intact.
#[gpui::test]
fn clicking_a_withdrawn_host_result_preserves_the_query(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::new(Some(1), "~".into(), None),
        fixture_hosts(&[0, 1]),
    );
    cx.simulate_resize(crate::bootstrap::MIN_WINDOW_SIZE);
    settle(cx);
    assert!(search_focused(cx));
    type_in(cx, "host:host2");
    assert!(name_of(cx, "launcher-search-option-0").is_some_and(|name| name.contains("host2")));
    let at = test_support::center(cx, "launcher-search-option-0");
    cx.update(|window, cx| {
        withdraw_presented_choice::<HostView>("2", cx);
        test_support::click_before_redraw(at, window, cx);
    });
    settle(cx);
    assert_stale_choice(cx, "host:host2");
    assert_eq!(draft(cx).host, Some(1));
    assert!(creates(&mut helm, cx).is_empty());
}

/// Replace with keeps its host fixed. Even if the user has scrolled the form down, taking a host result must explain
/// that refusal at the command field and leave the launch action reachable.
#[gpui::test]
fn replace_with_refuses_a_host_row(cx: &mut TestAppContext) {
    let source = super::draft_tests::session(codex(None, None, None));
    let (_helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::replace_with("a", &source, Some(1)),
        fixture_hosts(&[0, 1]),
    );
    cx.simulate_resize(crate::bootstrap::MIN_WINDOW_SIZE);
    settle(cx);
    let top = test_support::bounds(cx, "launcher-folder-row").top();
    super::browse_tests::scroll_body(cx, -4000.);
    assert!(
        test_support::bounds(cx, "launcher-folder-row").top() < top,
        "the premise: the form scrolled down separately from the field"
    );
    assert!(search_focused(cx), "the command editor still has focus");
    type_in(cx, "host:host2");
    keys(cx, "enter");
    assert_eq!(
        name_of(cx, "launcher-search-refusal").as_deref(),
        Some("Host cannot change here: this dialog keeps host fixed for replace")
    );
    assert_eq!(draft(cx).host, Some(1), "the host stays");
    let nodes = test_support::frame(cx);
    for id in ["launcher-search-refusal", "launcher-launch"] {
        let node = nodes
            .iter()
            .find(|node| node.element_id.as_deref() == Some(id))
            .expect("the command refusal and launch action are both drawn");
        assert_eq!(node.visible_bounds, node.bounds, "{id} stays in view");
    }
}

// ---- Templates -------------------------------------------------------------------------------------------------------

/// The templates collection as the helm sends it: `partial` sets only a folder, `go` is a command launch of `true`,
/// and `nowhere` names a host no install has; with `unavailable`, marked unavailable with the helm's words for why.
fn templates_state(unavailable: Option<&str>) -> farhelm_proto::ui_channel::CollectionState {
    use farhelm_proto::launcher::{
        LaunchTemplate, LauncherKind, TemplateDestination, TemplateFields,
    };
    use farhelm_proto::ui_channel::{Availability, Collection, CollectionState, KeyedEntry};
    let all = [
        LaunchTemplate {
            name: "partial".into(),
            fields: TemplateFields {
                destination: Some(TemplateDestination::Folder("/srv/partial".into())),
                ..TemplateFields::default()
            },
        },
        LaunchTemplate {
            name: "go".into(),
            fields: TemplateFields {
                kind: Some(LauncherKind::Command),
                command: Some("true".into()),
                yolo: Some(false),
                ..TemplateFields::default()
            },
        },
        LaunchTemplate {
            name: "nowhere".into(),
            fields: TemplateFields {
                host: Some("no-such-install".into()),
                ..TemplateFields::default()
            },
        },
        LaunchTemplate {
            name: "reset-codex".into(),
            fields: TemplateFields {
                agent: Some(LaunchHarness::Codex),
                permissions: Some(None),
                ..TemplateFields::default()
            },
        },
        LaunchTemplate {
            name: "reset-goose".into(),
            fields: TemplateFields {
                agent: Some(LaunchHarness::Goose),
                permissions: Some(None),
                ..TemplateFields::default()
            },
        },
        // An agent and nothing about permissions: applied over an explicit Guarded, it escalates.
        LaunchTemplate {
            name: "to-opencode".into(),
            fields: TemplateFields {
                agent: Some(LaunchHarness::OpenCode),
                ..TemplateFields::default()
            },
        },
    ];
    let mut meta = test_support::meta(&[
        "partial",
        "go",
        "nowhere",
        "reset-codex",
        "reset-goose",
        "to-opencode",
    ]);
    if let Some(error) = unavailable {
        meta.availability = Availability::Unavailable {
            error: error.into(),
        };
    }
    CollectionState {
        collection: Collection::Templates,
        meta,
        entries: all
            .iter()
            .map(|template| KeyedEntry {
                key: template.name.clone(),
                value: serde_json::to_value(template).unwrap(),
            })
            .collect(),
    }
}

/// New on host1 at `~` in the real main view over the fake helm, whose first snapshot carries [`templates_state`].
fn new_launcher_with_templates<'a>(
    cx: &'a mut TestAppContext,
    unavailable: Option<&str>,
) -> (
    farhelm_helm_client::fake::FakeHelm,
    &'a mut VisualTestContext,
) {
    launcher_with_templates_on(cx, unavailable, LaunchDraft::new(Some(1), "~".into(), None))
}

/// [`new_launcher_with_templates`] with the launcher opened on `draft` instead of New's bare draft.
fn launcher_with_templates_on<'a>(
    cx: &'a mut TestAppContext,
    unavailable: Option<&str>,
    draft: LaunchDraft,
) -> (
    farhelm_helm_client::fake::FakeHelm,
    &'a mut VisualTestContext,
) {
    launcher_with_these_templates(cx, templates_state(unavailable), draft)
}

/// [`launcher_with_templates_on`] with the templates collection `templates` in place of the shared set, for a test that
/// needs templates of its own.
fn launcher_with_these_templates(
    cx: &mut TestAppContext,
    templates: farhelm_proto::ui_channel::CollectionState,
    draft: LaunchDraft,
) -> (farhelm_helm_client::fake::FakeHelm, &mut VisualTestContext) {
    let (mut helm, _connection) = crate::test_support::started(cx);
    helm.connect(vec![
        crate::session_list::tests::listing(&["a"], &[crate::session_list::tests::session("a")]),
        fixture_hosts(&[0]),
        crate::session_list::tests::preferences(&[], None),
        templates,
    ]);
    cx.run_until_parked();
    let (_, window) = test_support::open_view(cx, crate::main_view::MainView::new);
    window.update(|window, cx| {
        let mark = crate::focus::Mark::take(cx);
        assert!(
            super::view::open(draft, &mark, window, cx).is_some(),
            "the launcher opened"
        );
    });
    settle(window);
    (helm, window)
}

/// SPEC.md, "Creation": "`tl:name` in the search box offers templates by name, and accepting one applies it", and
/// applying keeps the launch kind unless the template sets one. Taking the row is the keyboard path to a template's
/// edits, so it must make them exactly as applying elsewhere does. Specified: with New open and its catalog read,
/// `tl:partial` offers "Template: partial", and Enter puts its folder in the draft, keeps the Agent tab and empties the
/// box; `tl:go` and Enter then switch to the Command tab with the command `true`, the template's own launch kind.
#[gpui::test]
fn a_tl_row_applies_its_template_and_keeps_the_launch_kind(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_launcher_with_templates(cx, None);
    answer_catalog(&mut helm, cx, Some(serde_json::json!([])));
    type_in(cx, "tl:partial");
    assert_eq!(
        name_of(cx, "launcher-search-option-0").as_deref(),
        Some("Template: partial")
    );
    keys(cx, "enter");
    let applied = draft(cx);
    assert_eq!(applied.folder.value(), "/srv/partial", "the folder applied");
    assert_eq!(applied.tab, Tab::Agent, "the launch kind kept");
    assert_eq!(search_text(cx), "", "the box is emptied");

    type_in(cx, "tl:go");
    keys(cx, "enter");
    let applied = draft(cx);
    assert_eq!(applied.tab, Tab::Command, "the template's launch kind");
    assert_eq!(applied.command.command, "true");
}

/// With and without a query, clicking a template from a catalog frame that has since changed keeps the draft intact.
/// The template would edit the folder if the handler accidentally used the withdrawn row.
#[gpui::test]
fn clicking_a_withdrawn_template_warns_in_both_lists(cx: &mut TestAppContext) {
    for (query, id) in [
        ("tl:partial", "launcher-search-option-0"),
        ("", "launcher-template-partial"),
    ] {
        let (mut helm, cx) = new_launcher_with_templates(cx, None);
        if !query.is_empty() {
            assert!(search_focused(cx));
            type_in(cx, query);
        }
        assert_eq!(name_of(cx, id).as_deref(), Some("Template: partial"));
        let at = test_support::center(cx, id);
        cx.update(|window, cx| {
            withdraw_presented_choice::<LaunchTemplate>("partial", cx);
            test_support::click_before_redraw(at, window, cx);
        });
        settle(cx);
        assert_stale_choice(cx, query);
        assert_eq!(draft(cx).folder.value(), "~");
        assert!(creates(&mut helm, cx).is_empty());
    }
}

/// Save as template shows a captured draft. A click on a visible template must leave both the draft and name field alone.
#[gpui::test]
fn a_template_click_leaves_the_save_panel_and_draft_untouched(cx: &mut TestAppContext) {
    let mut launch = LaunchDraft::new(Some(1), "~".into(), None);
    launch.choose_harness(LaunchHarness::Codex, None, None);
    let (mut helm, cx) = launcher_with_templates_on(cx, None, launch);
    answer_catalog(&mut helm, cx, Some(serde_json::json!([])));
    test_support::click(cx, "launcher-save-template");
    let name_field = test_support::bounds(cx, "save-template-name");
    let name_has_focus = |cx: &mut VisualTestContext| {
        test_support::frame(cx)
            .iter()
            .any(|node| node.flags.focused && name_field.contains(&node.bounds.center()))
    };
    assert!(name_has_focus(cx));
    assert_eq!(
        name_of(cx, "launcher-template-partial").as_deref(),
        Some("Template: partial")
    );
    let before = draft(cx);
    test_support::click(cx, "launcher-template-partial");
    assert_eq!(draft(cx), before);
    assert!(name_has_focus(cx));
}

/// Clearing the keyboard highlight uses one Escape press. Its repeats must leave the editable launcher open.
#[gpui::test]
fn a_held_escape_clears_the_template_highlight_without_closing_the_launcher(
    cx: &mut TestAppContext,
) {
    let (_helm, cx) = new_launcher_with_templates(cx, None);
    assert!(search_focused(cx));
    keys(cx, "down");
    assert_eq!(selected_state(cx, "launcher-template-partial"), Some(true));
    test_support::press(cx, "escape", false);
    assert_eq!(selected_state(cx, "launcher-template-partial"), Some(false));
    test_support::press(cx, "escape", true);
    assert!(test_support::any_open(cx));
    assert!(search_focused(cx));
    test_support::press(cx, "escape", false);
    assert!(
        !test_support::any_open(cx),
        "a fresh Escape can still close it"
    );
}

/// The empty field normally launches with Enter. A removed template must consume the Enter meant for that template.
#[gpui::test]
fn enter_on_a_withdrawn_empty_pane_template_cannot_launch(cx: &mut TestAppContext) {
    let mut launch = LaunchDraft::new(Some(1), "~".into(), None);
    launch.choose_harness(LaunchHarness::Codex, None, None);
    let (mut helm, cx) = launcher_with_templates_on(cx, None, launch);
    assert!(search_focused(cx));
    keys(cx, "down");
    assert_eq!(selected_state(cx, "launcher-template-partial"), Some(true));
    cx.update(|_, cx| withdraw_presented_choice::<LaunchTemplate>("partial", cx));
    settle(cx);
    assert_eq!(name_of(cx, "launcher-template-partial"), None);
    assert!(search_focused(cx));
    keys(cx, "enter");
    assert_stale_choice(cx, "");
    assert_eq!(draft(cx).folder.value(), "~");
    assert!(creates(&mut helm, cx).is_empty());
}

/// Taking a template from the empty command field applies its edits once and keeps the editor ready for a query.
/// The same source mark remains while the form is edited and clears on Reset choices.
#[gpui::test]
fn the_empty_field_can_apply_a_template_and_reset_its_source(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_launcher_with_templates(cx, None);
    cx.simulate_resize(gpui::size(gpui::px(1080.), gpui::px(900.)));
    answer_catalog(&mut helm, cx, Some(serde_json::json!([])));
    assert_eq!(
        name_of(cx, "launcher-template-partial").as_deref(),
        Some("Template: partial")
    );
    assert_eq!(selected_state(cx, "launcher-template-partial"), Some(false));
    keys(cx, "down");
    assert_eq!(selected_state(cx, "launcher-template-partial"), Some(true));
    assert!(search_focused(cx));
    keys(cx, "enter");
    assert_eq!(draft(cx).folder.value(), "/srv/partial");
    assert!(search_focused(cx));
    test_support::click(cx, "launcher-harness-Codex");
    assert_eq!(draft(cx).agent.harness, Some(LaunchHarness::Codex));
    assert_eq!(
        name_of(cx, "launcher-provenance-label").as_deref(),
        Some("From a template: partial")
    );
    assert_eq!(selected_state(cx, "launcher-template-partial"), Some(true));
    assert!(super::recents_view_tests::creates(&mut helm, cx).is_empty());
    test_support::click(cx, "launcher-reset");
    assert_eq!(name_of(cx, "launcher-provenance-label"), None);
    assert_eq!(selected_state(cx, "launcher-template-partial"), Some(false));
}

/// A resize can hide the template that Enter was about to apply. Replace with must wait until the main action is shown.
#[gpui::test]
fn narrowing_replace_with_cannot_turn_a_template_pick_into_a_launch(cx: &mut TestAppContext) {
    let source = super::draft_tests::session(super::view_tests::codex(None, None, None));
    let draft = LaunchDraft::replace_with("a", &source, Some(1));
    let (mut helm, cx) = launcher_with_templates_on(cx, None, draft);
    cx.simulate_resize(gpui::size(gpui::px(1080.), gpui::px(900.)));
    answer_catalog(&mut helm, cx, Some(serde_json::json!([])));
    assert!(search_focused(cx));
    keys(cx, "down");
    test_support::show_next_frame(cx);
    assert_eq!(selected_state(cx, "launcher-template-partial"), Some(true));

    cx.simulate_resize(crate::bootstrap::MIN_WINDOW_SIZE);
    cx.simulate_keystrokes("enter");
    let route = "/api/sessions/a/replace";
    assert!(super::launch_tests::sent_to(&mut helm, cx, route).is_empty());
    keys(cx, "enter");
    assert_eq!(super::launch_tests::sent_to(&mut helm, cx, route).len(), 1);
}

/// Resizing preserves the query and its highlighted choice while moving between the pane and the popover. A press
/// on the form closes the wide results and still reaches the control the user pressed.
#[gpui::test]
fn the_inline_search_reflows_and_a_form_click_still_edits(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_launcher_with_templates(cx, None);
    let wide = gpui::size(gpui::px(1080.), gpui::px(900.));
    cx.simulate_resize(wide);
    settle(cx);
    answer_catalog(&mut helm, cx, Some(serde_json::json!([])));
    type_in(cx, "tl:");
    assert!(results_shown(cx));
    assert!(
        test_support::bounds(cx, "launcher-search-results").right()
            <= test_support::bounds(cx, "launcher-body").left()
    );
    keys(cx, "down");
    assert_eq!(selected_state(cx, "launcher-search-option-1"), Some(true));
    cx.simulate_resize(crate::bootstrap::MIN_WINDOW_SIZE);
    settle(cx);
    assert!(cx.debug_bounds("launcher-command-pane").is_none());
    assert!(results_shown(cx));
    assert_eq!(search_text(cx), "tl:");
    assert_eq!(selected_state(cx, "launcher-search-option-1"), Some(true));
    cx.simulate_resize(wide);
    settle(cx);
    assert!(results_shown(cx));
    assert_eq!(selected_state(cx, "launcher-search-option-1"), Some(true));
    test_support::click(cx, "launcher-harness-Codex");
    assert!(!results_shown(cx));
    assert_eq!(draft(cx).agent.harness, Some(LaunchHarness::Codex));
    assert_eq!(search_text(cx), "tl:");
}

/// A live template edit can move a highlighted row. Enter still takes the user's choice, not its former position.
#[gpui::test]
fn the_arrow_choice_follows_a_template_after_a_catalog_push(cx: &mut TestAppContext) {
    use farhelm_proto::ui_channel::{Collection, CollectionChanges};
    let (mut helm, cx) = new_launcher_with_templates(cx, None);
    answer_catalog(&mut helm, cx, Some(serde_json::json!([])));
    type_in(cx, "tl:");
    keys(cx, "down");
    assert_eq!(
        name_of(cx, "launcher-search-option-1").as_deref(),
        Some("Template: go")
    );
    assert_eq!(selected_state(cx, "launcher-search-option-1"), Some(true));
    helm.push(CollectionChanges {
        collection: Collection::Templates,
        meta: test_support::meta(&["go", "nowhere", "reset-codex", "reset-goose", "to-opencode"]),
        upsert: Vec::new(),
        remove: vec!["partial".into()],
    });
    settle(cx);
    assert_eq!(
        name_of(cx, "launcher-search-option-0").as_deref(),
        Some("Template: go")
    );
    assert_eq!(selected_state(cx, "launcher-search-option-0"), Some(true));
    keys(cx, "enter");
    assert_eq!(draft(cx).tab, Tab::Command);
    assert_eq!(draft(cx).command.command, "true");
}

/// A template disabled while a model catalog is pending must never enqueue an edit in case its name is reused.
#[gpui::test]
fn enter_on_a_held_template_keeps_its_query_and_never_queues_the_edit(cx: &mut TestAppContext) {
    use farhelm_proto::launcher::{LaunchTemplate, TemplateDestination, TemplateFields};
    use farhelm_proto::ui_channel::{Collection, CollectionChanges, KeyedEntry};
    let source = super::draft_tests::session(codex(None, None, None));
    let (mut helm, cx) =
        launcher_with_templates_on(cx, None, LaunchDraft::replace_with("a", &source, Some(1)));
    type_in(cx, "tl:nowhere");
    assert_eq!(
        name_of(cx, "launcher-search-option-0").as_deref(),
        Some("Template: nowhere")
    );
    let before = draft(cx);
    keys(cx, "enter");
    assert_eq!(search_text(cx), "tl:nowhere");
    assert!(
        name_of(cx, "launcher-search-refusal")
            .is_some_and(|words| words.contains("keeps the session's host"))
    );
    let replacement = LaunchTemplate {
        name: "nowhere".into(),
        fields: TemplateFields {
            destination: Some(TemplateDestination::Folder("/other".into())),
            ..TemplateFields::default()
        },
    };
    helm.push(CollectionChanges {
        collection: Collection::Templates,
        meta: test_support::meta(&["nowhere"]),
        upsert: vec![KeyedEntry {
            key: "nowhere".into(),
            value: serde_json::to_value(replacement).unwrap(),
        }],
        remove: Vec::new(),
    });
    settle(cx);
    answer_catalog(&mut helm, cx, Some(serde_json::json!([])));
    assert_eq!(draft(cx), before);
}

/// A template whose agent cannot run an explicitly chosen permission raises the escalation banner, as the hand switch
/// would, judged once for the whole template. Undo must also restore the prior source, which names the draft it restored.
#[gpui::test]
fn a_tl_row_that_escalates_raises_the_banner(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_launcher_with_templates(cx, None);
    answer_catalog(&mut helm, cx, Some(serde_json::json!([])));
    type_in(cx, "tl:partial");
    keys(cx, "enter");
    type_in(cx, "perms:guarded");
    keys(cx, "enter");
    assert_eq!(super::escalation_tests::banner(cx), None, "the premise");
    type_in(cx, "tl:to-opencode");
    keys(cx, "enter");
    assert_eq!(
        draft(cx).agent.harness,
        Some(LaunchHarness::OpenCode),
        "the premise"
    );
    assert_eq!(
        super::escalation_tests::banner(cx).as_deref(),
        Some("Permission changed: Guarded → YOLO, because OpenCode does not support Guarded.")
    );
    assert_eq!(
        selected_state(cx, "launcher-template-to-opencode"),
        Some(true)
    );
    test_support::click(cx, "launcher-escalation-undo");
    assert_eq!(
        name_of(cx, "launcher-provenance-label").as_deref(),
        Some("From a template: partial")
    );
}

/// SPEC.md: a template that does not apply "is refused with a message naming the field, and nothing from it is
/// applied", and the launcher says so beneath its command field. Applying part of it would leave a draft the user did
/// not ask for. Specified: `tl:nowhere` and Enter leave the folder `~` and show the refusal, naming `nowhere`, in the
/// launcher's note, drawn below the field.
#[gpui::test]
fn a_refused_tl_row_changes_nothing_and_says_why_below_the_field(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_launcher_with_templates(cx, None);
    answer_catalog(&mut helm, cx, Some(serde_json::json!([])));
    type_in(cx, "tl:nowhere");
    keys(cx, "enter");
    assert_eq!(draft(cx).folder.value(), "~", "nothing applied");
    let shown = name_of(cx, "launcher-note").expect("the refusal is shown");
    assert!(shown.contains("nowhere"), "{shown:?}");
    let (note, field) = (
        test_support::bounds(cx, "launcher-note"),
        test_support::bounds(cx, "launcher-search-box"),
    );
    assert!(
        note.top() >= field.bottom(),
        "the refusal is drawn below the field: {note:?} after {field:?}"
    );
}

/// A taken `tl:` row waits for New's catalog read, and the store can change meanwhile: the batch applies the template
/// the store holds when it applies, and says so in the old launcher's words when it cannot (the old `tl:` acceptance's
/// "No template is named …; it may have been deleted" and "Couldn't load templates: …; search tl: and retry").
/// Applying a template the user saw deleted, or saying nothing, would hide what happened. Specified: `tl:partial` and
/// Enter before the catalog answers, then `partial` deleted, then the answer: the folder stays `~` and the note reads
/// `No template is named "partial"; it may have been deleted`; in a second launcher, the same press with the templates
/// made unavailable ("busy") meanwhile says "Couldn't load templates: busy; search tl: and retry".
#[gpui::test]
fn a_tl_row_waiting_for_the_catalog_refuses_when_its_template_goes(cx: &mut TestAppContext) {
    use farhelm_proto::ui_channel::{Collection, CollectionChanges};
    let (mut helm, cx) = new_launcher_with_templates(cx, None);
    type_in(cx, "tl:partial");
    keys(cx, "enter");
    helm.push(CollectionChanges {
        collection: Collection::Templates,
        meta: test_support::meta(&["go", "nowhere"]),
        upsert: Vec::new(),
        remove: vec!["partial".into()],
    });
    settle(cx);
    answer_catalog(&mut helm, cx, Some(serde_json::json!([])));
    assert_eq!(draft(cx).folder.value(), "~", "nothing applied");
    assert_eq!(
        name_of(cx, "launcher-note").as_deref(),
        Some("No template is named \"partial\"; it may have been deleted")
    );
}

/// The unavailable half of [`a_tl_row_waiting_for_the_catalog_refuses_when_its_template_goes`]: the old words for a
/// failed read, with the helm's, when the templates become unavailable while the taken row waits for the catalog.
#[gpui::test]
fn a_tl_row_waiting_for_the_catalog_refuses_when_the_templates_become_unavailable(
    cx: &mut TestAppContext,
) {
    use farhelm_proto::ui_channel::{Availability, Collection, CollectionChanges};
    let (mut helm, cx) = new_launcher_with_templates(cx, None);
    type_in(cx, "tl:partial");
    keys(cx, "enter");
    let mut meta = test_support::meta(&["partial", "go", "nowhere"]);
    meta.availability = Availability::Unavailable {
        error: "busy".into(),
    };
    helm.push(CollectionChanges {
        collection: Collection::Templates,
        meta,
        upsert: Vec::new(),
        remove: Vec::new(),
    });
    settle(cx);
    answer_catalog(&mut helm, cx, Some(serde_json::json!([])));
    assert_eq!(draft(cx).folder.value(), "~", "nothing applied");
    assert_eq!(
        name_of(cx, "launcher-note").as_deref(),
        Some("Couldn't load templates: busy; search tl: and retry")
    );
}

/// SPEC.md: a template list that cannot be read is "a reason shown in the results area", in the old launcher's words.
/// Saying "No template matches" instead would tell the user their templates are gone. Specified: with the helm's
/// templates unavailable ("busy"), `tl:` shows "Couldn't load templates: busy" in the results and offers no row.
#[gpui::test]
fn unavailable_templates_are_the_reason_in_the_results(cx: &mut TestAppContext) {
    let (_helm, cx) = new_launcher_with_templates(cx, Some("busy"));
    type_in(cx, "tl:");
    assert!(results_shown(cx), "the results open with the reason");
    assert_eq!(
        name_of(cx, "launcher-search-reason").as_deref(),
        Some("Couldn't load templates: busy")
    );
    assert_eq!(name_of(cx, "launcher-search-option-0"), None, "no row");
}

// ---- Rows from the launch history ----------------------------------------------------------------------------------

/// The helm's history answer holding no launches and the one saved folder `path`.
fn saved_folder(path: &str) -> serde_json::Value {
    let mut answer = history(Vec::new());
    answer["folders"] = serde_json::json!([{
        "host": 1,
        "canonical_cwd": path,
        "canonical_proven": true,
        "display_cwd": path,
        "created_at": 1,
        "creation_seq": null,
    }]);
    answer
}

/// A recent setup taken from search fills the draft as a click on the strip's row does (its choices, its folder, the
/// Agent tab), empties the box and keeps focus there, and launches nothing, from Enter either. Why: the old search's
/// Recent row only filled; SPEC.md's "Enter on a focused row launches" is the strip's row, which has focus, not a
/// search row picked from the box (old `sidebar.spec.ts` composer search recents, and `apply_search_result`'s Recent
/// arm).
#[gpui::test]
fn a_recent_setup_from_search_fills_and_launches_nothing(cx: &mut TestAppContext) {
    // The launcher starts at home, so only taking the setup can put the folder in `/tmp`.
    let (mut helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::new(Some(1), "~".into(), None),
        fixture_hosts(&[0]),
    );
    answer_history(&mut helm, cx, &two_setups());
    assert_eq!(
        draft(cx).folder.value(),
        "~",
        "the premise: the launcher starts at home"
    );
    test_support::click(cx, "launcher-tabs-Command");
    test_support::click(cx, "launcher-search-row");
    type_in(cx, "recent:claude");
    let name = name_of(cx, "launcher-search-option-0").unwrap_or_default();
    assert!(
        name.starts_with("Recent setup: /tmp · ") && name.contains(" · Claude · "),
        "the premise: the Claude setup is the first row: {name}"
    );
    keys(cx, "enter");
    let filled = draft(cx);
    assert_eq!(filled.tab, Tab::Agent);
    assert_eq!(filled.agent.harness, Some(LaunchHarness::Claude));
    assert_eq!(filled.folder.value(), "/tmp");
    assert_eq!(search_text(cx), "");
    assert!(search_focused(cx));
    assert!(
        creates(&mut helm, cx).is_empty(),
        "Enter here launches nothing"
    );
}

/// A saved folder taken from search becomes the folder, and the field shows it (that the folder list then stops
/// filtering is `folder_list_tests::a_path_from_search_does_not_filter_the_next_list`, through the same arm). Why:
/// SPEC.md, "Creation": the launch-composer search matches "folders and recent setups", and "Folder search uses that
/// bounded history".
#[gpui::test]
fn a_saved_folder_from_search_sets_the_folder(cx: &mut TestAppContext) {
    let (_helm, cx) = new_in_tmp(cx, &saved_folder("/srv/project"));
    type_in(cx, "project");
    assert_eq!(
        name_of(cx, "launcher-search-option-0").as_deref(),
        Some("Use this path: /srv/project"),
        "the premise: the saved folder is offered"
    );
    keys(cx, "enter");
    assert_eq!(draft(cx).folder.value(), "/srv/project");
    assert_eq!(
        read(cx, |launcher, _, cx| launcher.folder.text(cx).to_string()),
        "/srv/project",
        "the folder field follows the draft"
    );
}

/// A model from a past launch that the catalog does not list chooses the agent type it ran with and the model as that
/// type's custom id. Why: a custom model typed once is offered again, and it only means something to its own agent
/// type (old `search_offers_a_used_custom_model_with_its_harness`, taken).
#[gpui::test]
fn a_past_model_from_search_chooses_its_agent_type(cx: &mut TestAppContext) {
    let answer = history(vec![launched(
        "/tmp",
        &chosen(LaunchHarness::Claude, Some("custom-x"), None, None),
    )]);
    let (_helm, cx) = new_in_tmp(cx, &answer);
    assert_eq!(draft(cx).agent.harness, None, "the premise: no agent type");
    type_in(cx, "model:custom");
    keys(cx, "enter");
    let chosen = draft(cx);
    assert_eq!(chosen.agent.harness, Some(LaunchHarness::Claude));
    assert_eq!(chosen.agent.model.as_deref(), Some("custom-x"));
    assert_eq!(chosen.agent.model_owner, Some(LaunchHarness::Claude));
}

/// Restart with refuses a recent setup and a saved folder from search with the destination's words, and Replace with
/// takes a recent setup (it holds only the host). A setup with a retired container choice is refused above the box
/// with the shared reason. Why: SPEC.md, "Restart with": "Search results and recent setups that set a held field are
/// still refused", and a container setup supports Delete only (old `retired_recent_is_refused_without_changing_the_draft`).
#[gpui::test]
fn history_rows_are_refused_where_they_cannot_apply(cx: &mut TestAppContext) {
    let mut answer = history(vec![launched(
        "/src/project",
        &chosen(LaunchHarness::Codex, None, None, None),
    )]);
    answer["folders"] = saved_folder("/srv/project")["folders"].clone();
    let mut source = super::draft_tests::session(codex(None, None, None));
    source.restart_offer = RestartOffer::Resume;
    let (mut helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::restart_with("missing", &source, Some(1)).unwrap(),
        fixture_hosts(&[0]),
    );
    answer_history(&mut helm, cx, &answer);
    test_support::click(cx, "launcher-search-row");
    let before = draft(cx);
    type_in(cx, "recent:");
    assert!(
        name_of(cx, "launcher-search-option-0")
            .is_some_and(|name| name.starts_with("Recent setup: /src/project · ")),
        "the first result is the recent setup"
    );
    keys(cx, "enter");
    assert_eq!(
        name_of(cx, "launcher-search-refusal").as_deref(),
        Some("Destination cannot change here: this dialog keeps destination fixed for restart")
    );
    assert_eq!(draft(cx), before, "nothing changed");
    for _ in 0.."recent:".len() {
        keys(cx, "backspace");
    }
    type_in(cx, "folder:project");
    assert_eq!(
        name_of(cx, "launcher-search-option-0").as_deref(),
        Some("Use this path: /srv/project"),
        "the premise: the saved folder is offered"
    );
    keys(cx, "enter");
    assert_eq!(
        name_of(cx, "launcher-search-refusal").as_deref(),
        Some("Destination cannot change here: this dialog keeps destination fixed for restart")
    );
    assert_eq!(draft(cx), before, "nothing changed");

    let replaced = super::draft_tests::session(codex(None, None, None));
    let (mut helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::replace_with("a", &replaced, Some(1)),
        fixture_hosts(&[0]),
    );
    answer_history(&mut helm, cx, &answer);
    type_in(cx, "recent:project");
    keys(cx, "enter");
    assert_eq!(name_of(cx, "launcher-search-refusal"), None);
    assert_eq!(
        draft(cx).folder.value(),
        "/src/project",
        "Replace with takes it"
    );

    let mut retired = chosen(LaunchHarness::Codex, None, None, None);
    retired.container = Some(
        serde_json::from_value(
            serde_json::json!({ "engine": "podman", "image": "old", "api_proxy_port": 1 }),
        )
        .unwrap(),
    );
    let (_helm, cx) = new_in_tmp(cx, &history(vec![launched("/tmp", &retired)]));
    let before = draft(cx);
    type_in(cx, "recent:");
    keys(cx, "enter");
    assert!(
        name_of(cx, "launcher-search-refusal").is_some_and(|words| words.contains("Delete only")),
        "the shared reason is said above the box"
    );
    assert_eq!(draft(cx), before, "nothing changed");
    assert_eq!(search_text(cx), "recent:", "the box keeps its text");
}

/// A saved folder taken from search while a managed checkout is chosen leaves the checkout for that folder, and keeps
/// the tab and its command as they were. Why: the folder is an ordinary launch's destination, so it must not clone a
/// repository or take a checkout's ownership just because one was chosen before (old `github-checkouts.spec.ts`, "an
/// unlabeled folder result replaces fresh intent without cloning").
#[gpui::test]
fn a_saved_folder_from_search_leaves_a_managed_checkout(cx: &mut TestAppContext) {
    let mut checkout = LaunchDraft::new(Some(1), "/tmp".into(), None);
    checkout.choose_tab(Tab::Command);
    checkout.choose_destination(super::draft::DestinationKind::Checkout);
    checkout.choose_repository(farhelm_proto::GithubRepo {
        owner: "acme".into(),
        name: "bar".into(),
    });
    let (mut helm, cx) = launcher_with_hosts(cx, checkout, fixture_hosts(&[0]));
    answer_history(&mut helm, cx, &saved_folder("/srv/project"));
    assert_eq!(
        draft(cx).destination_kind(),
        super::draft::DestinationKind::Checkout,
        "the premise: a managed checkout is chosen"
    );
    type_in(cx, "project");
    keys(cx, "enter");
    let after = draft(cx);
    assert_eq!(
        after.destination_kind(),
        super::draft::DestinationKind::Folder
    );
    assert_eq!(after.folder.value(), "/srv/project");
    assert_eq!(after.tab, Tab::Command, "the tab stays");
}

/// Type `o`, choose the second agent and let `answer` rebuild the rows. That agent must stay selected even while the
/// search picks up new catalog or history entries.
fn arrow_then_answer(
    cx: &mut TestAppContext,
    answer: impl FnOnce(&mut farhelm_helm_client::fake::FakeHelm, &mut VisualTestContext),
) -> (Option<LaunchHarness>, LaunchHarness) {
    let (mut helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::new(Some(1), "~".into(), None),
        fixture_hosts(&[0]),
    );
    type_in(cx, "o");
    let first = read(cx, |launcher, _, cx| launcher.search_results(cx).rows);
    let Some(super::search::Row::Harness(picked)) = first.get(1) else {
        panic!("the premise: `o` offers an agent type in position two: {first:?}");
    };
    let picked = *picked;
    keys(cx, "down");
    assert_eq!(
        selected_state(cx, "launcher-search-option-1"),
        Some(true),
        "the premise: Down highlighted the second row"
    );
    answer(&mut helm, cx);
    let rows = read(cx, |launcher, _, cx| launcher.search_results(cx).rows);
    assert_ne!(rows, first, "the premise: the answer rebuilt the rows");
    let index = rows
        .iter()
        .position(|row| *row == super::search::Row::Harness(picked))
        .unwrap();
    assert_eq!(
        selected_state(cx, &format!("launcher-search-option-{index}")),
        Some(true)
    );
    keys(cx, "enter");
    (draft(cx).agent.harness, picked)
}

/// A history answer may add folders, recent setups and past models. Enter still takes the user-selected agent.
#[gpui::test]
fn a_history_answer_preserves_the_arrow_choice(cx: &mut TestAppContext) {
    let (chosen_harness, picked) = arrow_then_answer(cx, |helm, cx| {
        answer_history(
            helm,
            cx,
            &history(vec![launched(
                "/tmp",
                &chosen(LaunchHarness::Codex, Some("o-model"), None, None),
            )]),
        )
    });
    assert_eq!(
        chosen_harness,
        Some(picked),
        "Enter took the row chosen with the arrow"
    );
}

/// A reconnect replaces the history underneath a query. Template choices belong to the helm catalog, so a
/// reconnect between Down and Enter must leave the deliberately chosen template selected.
#[gpui::test]
fn a_reconnect_preserves_the_template_arrow_choice(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_launcher_with_templates(cx, None);
    answer_catalog(&mut helm, cx, Some(serde_json::json!([])));
    type_in(cx, "tl:reset");
    keys(cx, "down");
    assert_eq!(
        name_of(cx, "launcher-search-option-1").as_deref(),
        Some("Template: reset-goose"),
    );
    assert_eq!(selected_state(cx, "launcher-search-option-1"), Some(true));

    let mut hosts = fixture_hosts(&[0]);
    let host = &mut hosts.entries[0].value;
    host["incarnation"] = serde_json::json!(host["incarnation"].as_u64().unwrap() + 1);
    helm.push(farhelm_proto::ui_channel::CollectionChanges {
        collection: hosts.collection,
        meta: hosts.meta,
        upsert: hosts.entries,
        remove: Vec::new(),
    });
    settle(cx);
    assert!(
        helm.commands()
            .iter()
            .any(|cmd| cmd.path.starts_with("/api/launch-history")),
        "the premise: the replacement connection caused another history read"
    );
    assert_eq!(selected_state(cx, "launcher-search-option-1"), Some(true));
    keys(cx, "enter");
    assert_eq!(draft(cx).agent.harness, Some(LaunchHarness::Goose));
}

/// The default host row is still usable after the arrow-picked host goes away. Enter belongs to the explicit
/// choice even then, and another Enter reopens search so the user can choose what's left.
#[gpui::test]
fn enter_on_a_withdrawn_typed_host_does_not_take_the_default(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::new(Some(1), "~".into(), None),
        fixture_hosts(&[0, 1, 2]),
    );
    assert!(search_focused(cx));
    type_in(cx, "host:host");
    keys(cx, "down");
    assert!(name_of(cx, "launcher-search-option-1").is_some_and(|name| name.contains("host2")));
    assert_eq!(selected_state(cx, "launcher-search-option-1"), Some(true));
    cx.update(|_, cx| withdraw_presented_choice::<HostView>("2", cx));
    settle(cx);
    assert!(name_of(cx, "launcher-search-option-1").is_some_and(|name| name.contains("host3")));
    assert_eq!(selected_state(cx, "launcher-search-option-0"), Some(false));
    assert!(search_focused(cx));
    keys(cx, "enter");
    assert_stale_choice(cx, "host:host");
    assert!(creates(&mut helm, cx).is_empty());
    keys(cx, "enter");
    assert!(results_shown(cx));
    keys(cx, "down enter");
    assert_eq!(draft(cx).host, Some(3));
    assert_eq!(search_text(cx), "");
    assert_eq!(name_of(cx, "launcher-search-refusal"), None);
}

/// Matching folder text on the same registry row does not identify the installation that offered it.
#[gpui::test]
fn enter_refuses_a_folder_from_the_previous_installation(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_launcher(cx);
    let answer = saved_folder("/srv/project");
    answer_history(&mut helm, cx, &answer);
    assert!(search_focused(cx));
    type_in(cx, "folder:project");
    let row = "launcher-search-option-0";
    let saved = Some("Use this path: /srv/project");
    assert_eq!(name_of(cx, row).as_deref(), saved);
    keys(cx, "down");
    assert_eq!(selected_state(cx, row), Some(true));
    retarget_history_host(&mut helm, cx);
    assert_ne!(name_of(cx, row).as_deref(), saved);
    answer_history(&mut helm, cx, &answer);
    assert_eq!(name_of(cx, row).as_deref(), saved);
    assert_eq!(selected_state(cx, row), Some(false));
    let before = draft(cx).folder.value().to_owned();
    keys(cx, "enter");
    assert_stale_choice(cx, "folder:project");
    assert_eq!(draft(cx).folder.value(), &before);
    assert!(creates(&mut helm, cx).is_empty());
}

/// A catalog answer may add models or replace a past model. Unrelated changes keep the user's chosen agent.
#[gpui::test]
fn a_catalog_answer_preserves_the_arrow_choice(cx: &mut TestAppContext) {
    let (chosen_harness, picked) = arrow_then_answer(cx, |helm, cx| {
        answer_catalog(
            helm,
            cx,
            Some(serde_json::json!([{ "id": "o-cat", "harness": "codex", "efforts": [] }])),
        )
    });
    assert_eq!(
        chosen_harness,
        Some(picked),
        "Enter took the row chosen with the arrow"
    );
}

/// New with `harness` and `chosen` picked by hand, then `tl:<template>` (a template of that agent type with
/// `permissions: null`) taken with Enter and Launch pressed: checks that the template reset the permission, and returns
/// the permission the one create carried.
fn reset_by_template_and_launch(
    cx: &mut TestAppContext,
    harness: LaunchHarness,
    chosen: LaunchPermission,
    template: &str,
) -> serde_json::Value {
    let mut start = LaunchDraft::new(Some(1), "~".into(), None);
    let _ = start.choose_harness(harness, None, None);
    start.choose_permissions(Some(chosen));
    let (mut helm, cx) = launcher_with_templates_on(cx, None, start);
    answer_catalog(&mut helm, cx, Some(serde_json::json!([])));
    assert_eq!(
        draft(cx).agent.permissions,
        Some(chosen),
        "the premise: the hand choice"
    );
    type_in(cx, &format!("tl:{template}"));
    keys(cx, "enter");
    let applied = draft(cx);
    assert_eq!(applied.agent.harness, Some(harness));
    assert_eq!(
        applied.agent.permissions, None,
        "the template reset the permission"
    );
    test_support::click(cx, "launcher-launch");
    let sent = super::launch_tests::creates(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("one create, not {}", sent.len());
    };
    create.body.clone().unwrap_or_default()["launch"]["permissions"].clone()
}

/// A template's `permissions: null` is "a real template reset, not a visible permission click" (the old
/// terminal-create-idempotency spec): it puts back the agent type's own default, overriding a choice the user made by
/// hand, and the launch carries that default. Keeping the hand choice would launch with a permission the template meant
/// to clear. Specified: New with Codex and YOLO chosen by hand, `tl:reset-codex` and Enter leave the permission unset
/// (the default), and Launch sends a create with `guarded`, Codex's default.
#[gpui::test]
fn a_tl_reset_of_codexs_permission_launches_guarded(cx: &mut TestAppContext) {
    let sent = reset_by_template_and_launch(
        cx,
        LaunchHarness::Codex,
        LaunchPermission::Yolo,
        "reset-codex",
    );
    assert_eq!(sent, serde_json::json!("guarded"));
}

/// The Goose twin of the case above, whose default is the other way round (the old spec ran both): a reset must give
/// the agent type's own default, not a fixed one. Specified: New with Goose and Approve chosen by hand,
/// `tl:reset-goose` and Enter leave the permission unset, and Launch sends a create with `yolo`, Goose's default.
#[gpui::test]
fn a_tl_reset_of_gooses_permission_launches_yolo(cx: &mut TestAppContext) {
    let sent = reset_by_template_and_launch(
        cx,
        LaunchHarness::Goose,
        LaunchPermission::Approve,
        "reset-goose",
    );
    assert_eq!(sent, serde_json::json!("yolo"));
}

// ---- Templates applied over an open YOLO question -------------------------------------------------------------------

/// The templates collection for the YOLO question tests: `yolo-codex` (Codex with YOLO), `guarded` (Guarded
/// permissions), `cmd-true` (the command `true`, asserted YOLO) and `cmd-other` (another command, asserted YOLO).
fn yolo_templates() -> farhelm_proto::ui_channel::CollectionState {
    use farhelm_proto::launcher::{LaunchTemplate, LauncherKind, TemplateFields};
    use farhelm_proto::ui_channel::{Collection, CollectionState, KeyedEntry};
    let command = |name: &str, command: &str| LaunchTemplate {
        name: name.into(),
        fields: TemplateFields {
            kind: Some(LauncherKind::Command),
            command: Some(command.into()),
            yolo: Some(true),
            ..TemplateFields::default()
        },
    };
    let all = [
        LaunchTemplate {
            name: "yolo-codex".into(),
            fields: TemplateFields {
                agent: Some(LaunchHarness::Codex),
                permissions: Some(Some(LaunchPermission::Yolo)),
                ..TemplateFields::default()
            },
        },
        LaunchTemplate {
            name: "guarded".into(),
            fields: TemplateFields {
                permissions: Some(Some(LaunchPermission::Guarded)),
                ..TemplateFields::default()
            },
        },
        command("cmd-true", "true"),
        command("cmd-other", "printf second"),
    ];
    CollectionState {
        collection: Collection::Templates,
        meta: test_support::meta(&["yolo-codex", "guarded", "cmd-true", "cmd-other"]),
        entries: all
            .iter()
            .map(|template| KeyedEntry {
                key: template.name.clone(),
                value: serde_json::to_value(template).unwrap(),
            })
            .collect(),
    }
}

/// The shared body of the two YOLO question tests below: open the launcher on `draft`, press Launch and have the helm
/// refuse for YOLO; apply `same` (a template that leaves the launch as it was) and check the question stays and its
/// one-off answer sends the first create again with consent; have that refused, press Launch again for a new question;
/// apply `other` (one that changes the launch) and check the question goes; apply `same` again and check it does not
/// come back. Applying a template sends no create.
fn yolo_question_across_templates(
    cx: &mut TestAppContext,
    draft: LaunchDraft,
    same: &str,
    other: &str,
) {
    let (mut helm, cx) = launcher_with_these_templates(cx, yolo_templates(), draft);
    // Read now, so a template's application never waits for the catalog and the step after it sees it applied.
    answer_catalog(&mut helm, cx, Some(serde_json::json!([])));
    let question_up = |cx: &mut VisualTestContext| name_of(cx, "yolo-reason").is_some();
    let apply = |cx: &mut VisualTestContext, name: &str| {
        test_support::click(cx, "launcher-search-row");
        type_in(cx, &format!("tl:{name}"));
        keys(cx, "enter");
    };
    let launch = |helm: &mut farhelm_helm_client::fake::FakeHelm, cx: &mut VisualTestContext| {
        test_support::click(cx, "launcher-launch");
        let sent = super::launch_tests::creates(helm, cx);
        let [create] = &sent[..] else {
            panic!("Launch sent one create, not {}", sent.len());
        };
        create.clone()
    };

    let first = launch(&mut helm, cx);
    super::yolo_tests::refuse_for_yolo(&mut helm, cx, first.id);
    assert!(question_up(cx), "the premise: the question is up");
    apply(cx, same);
    // The premise: the template applied (the box emptied, no note says it was refused) and the launcher took no new
    // question, which would have moved focus to its Cancel.
    assert_eq!(search_text(cx), "", "the premise: {same} applied");
    assert!(
        name_of(cx, "launcher-note").is_none(),
        "the premise: nothing refused {same}"
    );
    assert!(search_focused(cx), "focus stays in the search box");
    assert!(
        question_up(cx),
        "{same} left the launch as it was, so the question stays"
    );
    assert!(
        super::launch_tests::creates(&mut helm, cx).is_empty(),
        "applying sent nothing"
    );
    test_support::click(cx, "prompt-confirm");
    let sent = super::launch_tests::creates(&mut helm, cx);
    let [consented] = &sent[..] else {
        panic!("the one-off answer sent one create, not {}", sent.len());
    };
    let mut expected = first.body.clone().expect("a create body");
    expected["confirm_yolo"] = serde_json::json!(true);
    assert_eq!(
        consented.body.as_ref(),
        Some(&expected),
        "the same create, with consent"
    );
    helm.reply(
        consented.id,
        503,
        Some(serde_json::json!("refused in this test")),
    );
    settle(cx);

    let second = launch(&mut helm, cx);
    super::yolo_tests::refuse_for_yolo(&mut helm, cx, second.id);
    assert!(question_up(cx), "the premise: a new question is up");
    apply(cx, other);
    assert!(
        !question_up(cx),
        "{other} changed the launch, so the question went"
    );
    apply(cx, same);
    assert!(!question_up(cx), "and going back does not bring it back");
    assert!(
        super::launch_tests::creates(&mut helm, cx).is_empty(),
        "nothing more was sent"
    );
}

/// An agent template applied over an open YOLO question keeps it when the launch stays the same, and retires it when
/// the permission changes (yolo-guard.spec's "template edits retain an unchanged YOLO question and retire a changed
/// one"). Why: the question answers for the launch that was sent, so an application that leaves it so must not make
/// the user launch again to be asked again. Specified: on a Codex YOLO draft, `tl:yolo-codex` keeps the question and its
/// one-off answer resends the first create with consent; `tl:guarded` retires a new question; `tl:yolo-codex` again
/// does not bring it back.
#[gpui::test]
fn an_unchanged_agent_template_keeps_the_yolo_question(cx: &mut TestAppContext) {
    yolo_question_across_templates(
        cx,
        super::yolo_tests::yolo_codex_draft(),
        "yolo-codex",
        "guarded",
    );
}

/// The Command tab's case (yolo-guard.spec's "command templates preserve equal YOLO questions and retire changed
/// commands"). Specified: on a YOLO-asserted `true`, `tl:cmd-true` keeps the question and its one-off answer resends the
/// first create with consent; `tl:cmd-other` retires a new question; `tl:cmd-true` again does not bring it back.
#[gpui::test]
fn an_unchanged_command_template_keeps_the_yolo_question(cx: &mut TestAppContext) {
    yolo_question_across_templates(
        cx,
        super::yolo_tests::yolo_command_draft(),
        "cmd-true",
        "cmd-other",
    );
}
