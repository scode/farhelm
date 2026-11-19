//! Tests of the permission-escalation banner in the real launcher over the helm client's fake: what raises it and what
//! does not, what Undo and Dismiss do, how long it shows and what holds it, and the Reset choices button beside it. When
//! an agent switch escalates is the draft's rule, tested in `draft_tests` and the templates' `apply_tests`; these tests
//! show the launcher showing what the draft says.
//!
//! Ported from the old `e2e/tests/permission-switch.spec.ts` at reference commit `24bc0580` (named in each test's docs).

use std::time::Duration;

use farhelm_proto::{LaunchHarness, LaunchPermission};
use gpui::{Modifiers, TestAppContext, VisualTestContext};

use super::draft::LaunchDraft;
use super::view_tests::{draft, launcher_on, read};
use crate::test_support::{self, keys, settle};

/// The banner's words for an explicit Guarded moved to OpenCode, which runs only in YOLO.
const OPENCODE_FROM_GUARDED: &str =
    "Permission changed: Guarded → YOLO, because OpenCode does not support Guarded.";

/// Type `text` into the focused field as one input, and let it settle.
fn type_in(cx: &mut VisualTestContext, text: &str) {
    cx.simulate_input(text);
    settle(cx);
}

/// Type `word` into the search box, which holds focus, and take its highlighted row.
fn search(cx: &mut VisualTestContext, word: &str) {
    type_in(cx, word);
    keys(cx, "enter");
}

/// The banner's accessible name, if it shows. Its element id carries the banner's count
/// (`launcher-escalation-<n>`), so each new banner is a new element to assistive technology; this finds whichever is
/// drawn.
pub(super) fn banner(cx: &mut VisualTestContext) -> Option<String> {
    test_support::frame(cx)
        .into_iter()
        .find(|node| {
            node.element_id
                .as_deref()
                .and_then(|id| id.strip_prefix("launcher-escalation-"))
                .is_some_and(|count| !count.is_empty() && count.chars().all(|c| c.is_ascii_digit()))
        })
        .and_then(|node| node.label.map(|label| label.to_string()))
}

/// New with Guarded chosen explicitly and then OpenCode, which cannot run Guarded: the switch makes the permission
/// YOLO by itself, which raises the banner.
fn escalated(
    cx: &mut TestAppContext,
) -> (farhelm_helm_client::fake::FakeHelm, &mut VisualTestContext) {
    let (helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    search(cx, "perms:guarded");
    assert!(
        draft(cx).agent.permissions_explicit,
        "the premise: Guarded chosen deliberately"
    );
    assert_eq!(banner(cx), None, "the premise: a direct choice raises none");
    search(cx, "opencode");
    assert_eq!(draft(cx).agent.harness, Some(LaunchHarness::OpenCode));
    assert_eq!(
        banner(cx).as_deref(),
        Some(OPENCODE_FROM_GUARDED),
        "the switch must show a banner before later actions can clear it"
    );
    (helm, cx)
}

/// An explicit Guarded, then an agent that cannot run it, raises the banner naming both modes; an edit after the switch
/// leaves it up, and Undo puts back the whole draft
/// from before the switch (no agent, Guarded chosen) and goes on in the search box. Why: SPEC.md, "Creation", and the
/// old "first harness escalation warns after Guarded chosen by ${source}" and "… escalation Undo restores the
/// pre-switch draft".
#[gpui::test]
fn an_escalating_switch_shows_the_banner_and_undo_restores_the_draft(cx: &mut TestAppContext) {
    let (_helm, cx) = escalated(cx);
    search(cx, "name:after the switch");
    assert_eq!(draft(cx).name.value(), "after the switch", "the premise");
    assert_eq!(
        banner(cx).as_deref(),
        Some(OPENCODE_FROM_GUARDED),
        "an edit leaves it up"
    );
    test_support::click(cx, "launcher-escalation-undo");
    let undone = draft(cx);
    assert_eq!(undone.agent.harness, None);
    assert_eq!(undone.agent.permissions, Some(LaunchPermission::Guarded));
    assert!(undone.agent.permissions_explicit);
    assert_eq!(undone.name.value(), "", "the draft from before the switch");
    assert_eq!(banner(cx), None);
    assert!(
        read(cx, |launcher, window, cx| launcher
            .search
            .focus_handle(cx)
            .is_focused(window)),
        "focus goes on in the search box"
    );
    // Undo put back where the user's choice stood, so the same switch warns again.
    search(cx, "opencode");
    assert_eq!(banner(cx).as_deref(), Some(OPENCODE_FROM_GUARDED));
}

/// Dismiss takes the banner away and keeps the switch. Why: SPEC.md, "dismiss closes the banner" (old "… escalation
/// Undo restores the pre-switch draft", its Dismiss step).
#[gpui::test]
fn dismiss_keeps_the_switch(cx: &mut TestAppContext) {
    let (_helm, cx) = escalated(cx);
    test_support::click(cx, "launcher-escalation-dismiss");
    assert_eq!(banner(cx), None);
    assert_eq!(draft(cx).agent.harness, Some(LaunchHarness::OpenCode));
    assert_eq!(draft(cx).agent.permissions, Some(LaunchPermission::Yolo));
}

/// Another agent switch takes an earlier banner away, since its Undo would restore a draft from before a switch that
/// is no longer the last. Why: the old banner was cleared on any later harness change.
#[gpui::test]
fn a_later_switch_clears_the_banner(cx: &mut TestAppContext) {
    let (_helm, cx) = escalated(cx);
    search(cx, "harness:codex");
    assert_eq!(
        draft(cx).agent.harness,
        Some(LaunchHarness::Codex),
        "the premise"
    );
    assert_eq!(banner(cx), None, "a later switch takes it away");
}

/// The banner shows for eight seconds of not being held: hovering it, or focus on one of its buttons, stops the count,
/// and the rest of the eight seconds runs once both end. Why: SPEC.md, "lasts about eight seconds, paused while hovered
/// or focused" (old "permission banner lifetime pauses for hover and focus …").
#[gpui::test]
fn the_banner_shows_eight_seconds_unless_hovered_or_focused(cx: &mut TestAppContext) {
    let (_helm, cx) = escalated(cx);
    test_support::advance(cx, Duration::from_millis(6_000));
    assert!(banner(cx).is_some(), "still up after six seconds");
    // Hovered: the count stops with two seconds left.
    let over = test_support::center(cx, "launcher-escalation");
    cx.simulate_mouse_move(over, None, Modifiers::none());
    settle(cx);
    test_support::advance(cx, Duration::from_secs(5));
    assert!(banner(cx).is_some(), "held while hovered");
    // A key typed with the pointer still resting on it does not count as the pointer leaving.
    type_in(cx, "x");
    keys(cx, "backspace");
    test_support::advance(cx, Duration::from_secs(5));
    assert!(
        banner(cx).is_some(),
        "still held after typing under the pointer"
    );
    // The pointer leaves for the search box: the last two seconds run.
    let away = test_support::center(cx, "launcher-search-row");
    cx.simulate_mouse_move(away, None, Modifiers::none());
    settle(cx);
    test_support::advance(cx, Duration::from_millis(1_500));
    assert!(banner(cx).is_some(), "the rest still running");
    // Focus on Dismiss (Shift+Tab from the search box, the band coming before the body) stops it again.
    keys(cx, "shift-tab");
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("launcher-escalation-dismiss"),
        "the premise: focus on the banner's Dismiss"
    );
    test_support::advance(cx, Duration::from_secs(10));
    assert!(banner(cx).is_some(), "held while focused");
    keys(cx, "tab");
    test_support::advance(cx, Duration::from_millis(400));
    assert!(banner(cx).is_some(), "half a second was left");
    test_support::advance(cx, Duration::from_millis(200));
    assert_eq!(banner(cx), None, "gone once its eight seconds are up");
}

/// A permission chosen directly never raises the banner, even a more dangerous one. Why: SPEC.md, "Choosing a
/// permission directly never triggers it" (old "… direct permission choices do not").
#[gpui::test]
fn a_direct_permission_choice_raises_no_banner(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    search(cx, "codex");
    search(cx, "perms:yolo");
    assert_eq!(
        draft(cx).agent.permissions,
        Some(LaunchPermission::Yolo),
        "the premise"
    );
    assert_eq!(banner(cx), None);
}

/// Taking another agent's model from Show all also changes the agent. The banner must explain an automatic
/// permission escalation from this path just as it does when the user picks the agent itself.
#[gpui::test]
fn a_cross_agent_model_pick_shows_the_escalation_banner(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    super::view_tests::answer_catalog(
        &mut helm,
        cx,
        Some(serde_json::json!([{ "id": "pi-warning-model", "harness": "pi", "efforts": [] }])),
    );
    search(cx, "harness:claude");
    search(cx, "perms:guarded");
    let before = draft(cx);
    assert_eq!(before.agent.harness, Some(LaunchHarness::Claude));
    assert_eq!(before.agent.permissions, Some(LaunchPermission::Guarded));
    assert!(before.agent.permissions_explicit);
    assert_eq!(banner(cx), None);

    test_support::click(cx, "launcher-model-row");
    test_support::click(cx, "launcher-model-show-all");
    test_support::click(cx, "launcher-model-option-Pi-pi-warning-model");
    assert_eq!(draft(cx).agent.model.as_deref(), Some("pi-warning-model"));
    assert_eq!(
        banner(cx).as_deref(),
        Some("Permission changed: Guarded → YOLO, because Pi does not support Guarded.")
    );
    test_support::click(cx, "launcher-escalation-undo");
    assert_eq!(draft(cx), before);
    assert_eq!(banner(cx), None);
}

/// Reset choices empties the search box and closes its results, as the old button did, besides resetting the agent
/// choices. Why: Reset starts the agent choices over, and a half-typed word in the box belongs to the choices it
/// replaces.
#[gpui::test]
fn reset_choices_empties_the_search_box(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    search(cx, "codex");
    type_in(cx, "gpt");
    assert_eq!(
        read(cx, |launcher, _, cx| launcher.search.text(cx).to_string()),
        "gpt",
        "the premise"
    );
    test_support::frame(cx);
    assert!(
        cx.debug_bounds("launcher-search-results").is_some(),
        "the premise: the results are open"
    );
    test_support::click(cx, "launcher-reset");
    assert_eq!(draft(cx).agent.harness, None);
    assert_eq!(
        read(cx, |launcher, _, cx| launcher.search.text(cx).to_string()),
        ""
    );
    test_support::frame(cx);
    assert!(
        cx.debug_bounds("launcher-search-results").is_none(),
        "and the results closed"
    );
}

/// Taking a recent setup takes the banner away. Why: its Undo would put back the draft from before the earlier switch
/// and silently throw the taken setup away (the old strip cleared it whatever the setup's agent).
#[gpui::test]
fn taking_a_recent_setup_of_another_agent_clears_the_banner(cx: &mut TestAppContext) {
    let (_helm, cx) =
        super::recents_view_tests::new_in_tmp(cx, &super::recents_view_tests::two_setups());
    search(cx, "perms:guarded");
    search(cx, "opencode");
    assert_eq!(
        banner(cx).as_deref(),
        Some(OPENCODE_FROM_GUARDED),
        "the premise"
    );
    search(cx, "recent:codex");
    assert_eq!(
        draft(cx).agent.harness,
        Some(LaunchHarness::Codex),
        "the premise"
    );
    assert_eq!(banner(cx), None);
}

/// A model of another agent taken from the search box while the Command tab is open raises no banner: the switch
/// starts from the Command tab, whose launch has no permission to escalate. Why: the draft's rule, and the old app,
/// which judged the switch from the tab the user was on (old `capture_permission_switch`).
#[gpui::test]
fn a_model_row_from_the_command_tab_raises_no_banner(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    super::view_tests::answer_catalog(
        &mut helm,
        cx,
        Some(serde_json::json!([{ "id": "pi-model", "harness": "pi", "efforts": [] }])),
    );
    search(cx, "harness:codex");
    search(cx, "perms:guarded");
    test_support::click(cx, "launcher-tabs-Command");
    let before = draft(cx);
    assert!(
        before.agent.permissions == Some(LaunchPermission::Guarded)
            && before.agent.permissions_explicit
            && before.tab == super::draft::Tab::Command,
        "the premise: Guarded chosen deliberately, then the Command tab"
    );
    test_support::click(cx, "launcher-search-row");
    search(cx, "model:pi-model");
    assert_eq!(
        draft(cx).agent.harness,
        Some(LaunchHarness::Pi),
        "the premise: the switch"
    );
    assert_eq!(banner(cx), None);
}

/// Under reduced motion the permission control's outline stays while the banner shows, and goes when the banner does
/// (that it does not fade is the transition's own test, `components::tests::transition`). Why: SPEC.md, "Reduced motion replaces slide and fade with a static outline while the banner is
/// shown": a static outline that outlived its banner would warn about nothing.
#[gpui::test]
fn under_reduced_motion_the_outline_lasts_as_long_as_the_banner(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    cx.update(|_, cx| crate::ticker::set_reduced_motion(true, cx));
    search(cx, "perms:guarded");
    search(cx, "opencode");
    assert!(banner(cx).is_some(), "the premise: the banner shows");
    test_support::frame(cx);
    assert!(
        cx.debug_bounds("launcher-escalation-outline").is_some(),
        "the outline shows with it"
    );
    test_support::advance(cx, Duration::from_secs(5));
    test_support::frame(cx);
    assert!(
        cx.debug_bounds("launcher-escalation-outline").is_some(),
        "and stays while it shows"
    );
    test_support::click(cx, "launcher-escalation-dismiss");
    test_support::frame(cx);
    assert!(
        cx.debug_bounds("launcher-escalation-outline").is_none(),
        "and goes with it"
    );
}
