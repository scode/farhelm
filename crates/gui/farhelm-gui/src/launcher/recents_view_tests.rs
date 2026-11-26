//! Tests of recent setups in the real launcher's command pane over the helm client's fake: when they show, what a row says,
//! and what a click and Enter on a row do. The rules for which setups and in what order are `recents_tests`'.
//!
//! Ported from the composer cases of the old `e2e/tests/sidebar.spec.ts` at reference commit `24bc0580` ("composer
//! recent slots appear only with matches", "composer Enter on a focused recent launches the filled setup", "composer
//! recent rows lead with the harness"), and the Replace with hint of `replace.spec.ts`.

use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::HostView;
use farhelm_proto::{LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection, RestartOffer};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::draft::{LaunchDraft, Tab};
use super::search_view_tests::assert_stale_choice;
use super::view_tests::{
    click_in_body, codex, draft, fixture_hosts, launcher_with_hosts, name_of,
    retarget_history_host, search_focused,
};
use crate::test_support::{self, keys, settle, withdraw_presented_choice};

/// `harness` launched with `model`, `effort` and `permissions`.
pub(super) fn chosen(
    harness: LaunchHarness,
    model: Option<&str>,
    effort: Option<LaunchEffort>,
    permissions: Option<LaunchPermission>,
) -> LaunchSelection {
    LaunchSelection {
        custom_agent: None,
        harness,
        model: model.map(str::to_owned),
        effort: effort.map(Into::into),
        permissions,
        permission_flags: None,
        workspace_trust: None,
        wrapper_prefix: None,
        container: None,
    }
}

/// One launch in the helm's history, of `selection` in the folder `cwd`.
pub(super) fn launched(cwd: &str, selection: &LaunchSelection) -> Value {
    json!({
        "host": 1,
        "github_repo": null,
        "canonical_cwd": cwd,
        "cwd": cwd,
        "selection": serde_json::to_value(selection).unwrap(),
        "created_at": 1,
        "creation_seq": null,
    })
}

/// The helm's history answer holding `launches`, newest first.
pub(super) fn history(launches: Vec<Value>) -> Value {
    json!({ "launches": launches, "folders": [], "checkout_config_revision": 1 })
}

/// Answer every command the window sent since the last call: the history read with `answer`, the rest with no body.
pub(super) fn answer_history(helm: &mut FakeHelm, cx: &mut VisualTestContext, answer: &Value) {
    cx.run_until_parked();
    let mut read = false;
    for command in helm.commands() {
        if command.path.starts_with("/api/launch-history") {
            read = true;
            helm.reply(command.id, 200, Some(answer.clone()));
        } else {
            helm.reply(command.id, 200, None);
        }
    }
    assert!(read, "the premise: the launcher read the history");
    settle(cx);
}

/// The creates (`POST /api/sessions`) the window sent since the last call; everything else is answered with no body.
pub(super) fn creates(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> Vec<FakeCommand> {
    cx.run_until_parked();
    let mut creates = Vec::new();
    for command in helm.commands() {
        if command.path == "/api/sessions" {
            creates.push(command);
        } else {
            helm.reply(command.id, 200, None);
        }
    }
    creates
}

/// Whether the pane has a Recent group. The template list remains available without it.
fn strip_shown(cx: &mut VisualTestContext) -> bool {
    name_of(cx, "launcher-recent-heading").is_some()
}

/// The YOLO Codex setup and the default Claude one, both in `/tmp`.
pub(super) fn two_setups() -> Value {
    history(vec![
        launched(
            "/tmp",
            &chosen(
                LaunchHarness::Codex,
                Some("enter-model"),
                Some(LaunchEffort::High),
                Some(LaunchPermission::Yolo),
            ),
        ),
        launched("/tmp", &chosen(LaunchHarness::Claude, None, None, None)),
    ])
}

/// Two otherwise identical setups whose flags share several lines. The last argument is the only way to tell
/// the saved approvals apart, and the short wrapper must still have room in the row.
pub(crate) fn custom_flag_setups() -> Value {
    repeated_flag_setups(1)
}

/// A longer common prefix makes the footer itself scroll. Use exactly the same choices as the short fixture.
fn repeated_flag_setups(repeats: usize) -> Value {
    let setup = |policy: &str| {
        let mut selection = chosen(
            LaunchHarness::Codex,
            None,
            None,
            Some(LaunchPermission::Custom),
        );
        selection.wrapper_prefix = Some(vec!["env".into(), "CI=1".into()]);
        let mut flags: Vec<_> = [
            "--allow-workspace-reads",
            "--allow-project-edits",
            "--deny-outside-workspace",
            "--deny-network-by-default",
            "--confirm-before-running-tools",
        ]
        .repeat(repeats)
        .into_iter()
        .map(str::to_owned)
        .collect();
        if repeats > 1 {
            flags.push(format!("--allowed-path={}", "fixture-scope/".repeat(18)));
        }
        flags.push(policy.to_owned());
        selection.permission_flags = Some(flags);
        launched("/tmp", &selection)
    };
    history(vec![
        setup("--policy=review-only"),
        setup("--policy=ask-every-time"),
    ])
}

/// Down moves between indistinguishable summaries in the empty command pane. The footer identifies the setup
/// before Enter launches it, even when the last argument would not fit in the row.
#[gpui::test]
fn wide_recents_show_custom_flags_before_enter(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_in_tmp(cx, &custom_flag_setups());
    assert!(test_support::drawn(cx, "launcher-command-pane"));
    assert_recent_flag_choices(cx, ["launcher-recent-0", "launcher-recent-1"]);
    assert!(name_of(cx, "launcher-recent-permission-flags").is_none());
    keys(cx, "down");
    assert_recent_flag_detail(
        cx,
        "launcher-recents",
        "launcher-recent-0",
        "--policy=review-only",
    );
    keys(cx, "down");
    assert_recent_flag_detail(
        cx,
        "launcher-recents",
        "launcher-recent-1",
        "--policy=ask-every-time",
    );
    assert!(creates(&mut helm, cx).is_empty());
    keys(cx, "enter");
    let sent = creates(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("expected one launch from the highlighted setup, got {sent:?}");
    };
    assert_eq!(
        create.body.as_ref().unwrap()["launch"]["permission_flags"]
            .as_array()
            .unwrap()
            .last(),
        Some(&json!("--policy=ask-every-time"))
    );
}

/// Typed results in the narrow window keep their own pinned footer. Enter fills the selection without launching.
#[gpui::test]
fn narrow_recent_search_shows_custom_flags_before_enter(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_in_tmp(cx, &custom_flag_setups());
    cx.simulate_resize(crate::bootstrap::MIN_WINDOW_SIZE);
    settle(cx);
    assert!(!test_support::drawn(cx, "launcher-command-pane"));
    assert!(search_focused(cx));
    super::search_view_tests::type_in(cx, "recent:codex");
    assert_recent_flag_choices(cx, ["launcher-search-option-0", "launcher-search-option-1"]);
    assert_recent_flag_detail(
        cx,
        "launcher-search-results-viewport",
        "launcher-search-option-0",
        "--policy=review-only",
    );
    keys(cx, "down");
    assert_recent_flag_detail(
        cx,
        "launcher-search-results-viewport",
        "launcher-search-option-1",
        "--policy=ask-every-time",
    );
    assert!(creates(&mut helm, cx).is_empty());
    keys(cx, "enter");
    let flags = farhelm_proto::split_permission_flags(&draft(cx).agent.permission_flags).unwrap();
    assert_eq!(
        flags.last().map(String::as_str),
        Some("--policy=ask-every-time")
    );
    assert!(name_of(cx, "launcher-recent-permission-flags").is_none());
    assert!(creates(&mut helm, cx).is_empty());
}

/// Long flags must leave both the selected row and the command field usable at the shortest wide window height.
#[gpui::test]
fn wide_recent_flag_overflow_pages_without_moving_the_choice(cx: &mut TestAppContext) {
    assert_can_page_recent_flags(cx, false);
}

/// The narrow popover measures itself differently from the wide pane. It still owns the same independent scroll.
#[gpui::test]
fn narrow_recent_flag_overflow_pages_without_moving_the_choice(cx: &mut TestAppContext) {
    assert_can_page_recent_flags(cx, true);
}

/// Prove the last word is clipped initially, then reveal it using only the command field's page keys.
fn assert_can_page_recent_flags(cx: &mut TestAppContext, narrow: bool) {
    let (mut helm, cx) = new_in_tmp(cx, &repeated_flag_setups(6));
    let minimum = crate::bootstrap::MIN_WINDOW_SIZE;
    let (first, second) = if narrow {
        cx.simulate_resize(minimum);
        settle(cx);
        assert!(!test_support::drawn(cx, "launcher-command-pane"));
        assert!(search_focused(cx));
        super::search_view_tests::type_in(cx, "recent:codex");
        ("launcher-search-option-0", "launcher-search-option-1")
    } else {
        cx.simulate_resize(gpui::size(gpui::px(1080.), minimum.height));
        settle(cx);
        assert!(test_support::drawn(cx, "launcher-command-pane"));
        assert!(search_focused(cx));
        ("launcher-recent-0", "launcher-recent-1")
    };
    assert_recent_flag_choices(cx, [first, second]);
    if !narrow {
        keys(cx, "down");
    }
    assert!(
        test_support::frame(cx).iter().any(|node| {
            node.element_id.as_deref() == Some(first)
                && node.selected == Some(true)
                && node.visible_bounds == node.bounds
                && !node.visible_bounds.is_empty()
        }),
        "the viewport still shows the entire selected row"
    );
    let first_block = recent_flags_block(cx, "--policy=review-only");
    assert_eq!(first_block.visible_bounds.top(), first_block.bounds.top());
    assert!(
        first_block.visible_bounds.bottom() < first_block.bounds.bottom(),
        "the fixture must require scrolling to the last word"
    );
    // This fixture's prefix and its long path fit in fewer than sixteen pages in either viewport.
    let mut block = first_block;
    for _ in 0..16 {
        if block.visible_bounds.bottom() == block.bounds.bottom() {
            break;
        }
        let previous = block.bounds.top();
        keys(cx, "pagedown");
        block = recent_flags_block(cx, "--policy=review-only");
        assert!(
            block.bounds.top() < previous,
            "Page Down scrolled the detail"
        );
    }
    assert_eq!(
        block.visible_bounds.bottom(),
        block.bounds.bottom(),
        "the last flag is visible"
    );
    assert!(!block.visible_bounds.is_empty());
    assert!(recent_highlighted(cx, first));
    assert!(search_focused(cx));
    let previous = block.bounds.top();
    keys(cx, "pageup");
    assert!(recent_flags_block(cx, "--policy=review-only").bounds.top() > previous);
    assert!(recent_highlighted(cx, first));
    assert!(search_focused(cx));
    keys(cx, "pagedown");
    let scrolled = recent_flags_block(cx, "--policy=review-only");
    assert!(
        scrolled.visible_bounds.top() > scrolled.bounds.top(),
        "the previous choice still has an offset when the highlight moves"
    );
    keys(cx, "down");
    assert!(recent_highlighted(cx, second));
    let second_block = recent_flags_block(cx, "--policy=ask-every-time");
    assert_eq!(
        second_block.visible_bounds.top(),
        second_block.bounds.top(),
        "a new choice starts at its first flag"
    );
    assert!(creates(&mut helm, cx).is_empty());
}

/// Witness both saved choices before moving the highlight. Their accessibility labels already include all
/// the flags and the wrapper. The keyboard footer gives the same distinction in painted text.
fn assert_recent_flag_choices(cx: &mut VisualTestContext, rows: [&'static str; 2]) {
    assert!(search_focused(cx));
    for (row, policy) in rows
        .into_iter()
        .zip(["--policy=review-only", "--policy=ask-every-time"])
    {
        let name = name_of(cx, row).expect("both setups are offered");
        assert!(
            name.contains("Codex") && name.contains("Permissions: Custom"),
            "{name}"
        );
        assert!(
            name.contains("Wrapper: env CI=1") && name.contains(policy),
            "{name}"
        );
    }
}

/// Check the painted text block. The renderer records all wrapped lines as one block, even if the last is clipped.
fn assert_recent_flag_detail(
    cx: &mut VisualTestContext,
    list: &'static str,
    row: &'static str,
    last: &str,
) {
    assert!(recent_highlighted(cx, row));
    assert!(search_focused(cx));
    let flags = recent_flags_block(cx, last);
    assert!(
        flags.bounds.size.height > crate::theme::metrics::TEXT_SM * 2.,
        "the long flag list wraps: {flags:?}"
    );
    assert_eq!(
        flags.visible_bounds, flags.bounds,
        "the flag tail remains in view"
    );
    assert!(
        flags.bounds.top() >= test_support::bounds(cx, list).bottom(),
        "the flags are below the scroll viewport"
    );
}

/// Read painted text at the last rendered offset. The record keeps the full text even when its bounds are clipped.
fn recent_flags_block(cx: &mut VisualTestContext, last: &str) -> gpui::DriverText {
    crate::session_list::host_rail::tests::texts_within(
        &test_support::frame(cx),
        "launcher-recent-permission-flags",
    )
    .into_iter()
    .find(|block| block.text.contains(last))
    .expect("the highlighted setup supplies its last argument")
}

/// New on host1 in `/tmp` with `answer` as its history.
pub(super) fn new_in_tmp<'a>(
    cx: &'a mut TestAppContext,
    answer: &Value,
) -> (FakeHelm, &'a mut VisualTestContext) {
    let (mut helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::new(Some(1), "/tmp".into(), None),
        fixture_hosts(&[0]),
    );
    answer_history(&mut helm, cx, answer);
    (helm, cx)
}

/// The strip shows only while some setup matches the draft, takes no room otherwise, lists the matching setups agent
/// type first, and narrows as the user chooses an agent type. Why: SPEC.md, "Recent-setup rows appear only when the
/// current filter actually matches history; the dialog reserves no space for them when it does not" (old "composer
/// recent slots appear only with matches").
#[gpui::test]
fn the_strip_shows_matching_setups_only(cx: &mut TestAppContext) {
    let (_helm, cx) = new_in_tmp(cx, &two_setups());
    assert!(strip_shown(cx), "two setups match /tmp");
    assert_eq!(
        name_of(cx, "launcher-recent-0").as_deref(),
        Some("/tmp · host1 · Codex · Model: enter-model · Effort: High · Permissions: YOLO")
    );
    assert_eq!(
        name_of(cx, "launcher-recent-1").as_deref(),
        Some("/tmp · host1 · Claude · Model: Default · Effort: Default · Permissions: Guarded")
    );
    cx.simulate_input("claude");
    settle(cx);
    keys(cx, "enter");
    assert_eq!(
        draft(cx).agent.harness,
        Some(LaunchHarness::Claude),
        "the premise: Claude is chosen"
    );
    assert!(
        name_of(cx, "launcher-recent-0").is_some_and(|name| name.contains("Claude")),
        "Claude's setup is the row left"
    );
    assert!(
        name_of(cx, "launcher-recent-1").is_none(),
        "and the only one"
    );
    click_in_body(cx, "launcher-folder");
    keys(cx, "backspace backspace backspace backspace");
    cx.simulate_input("/nowhere");
    settle(cx);
    assert_eq!(
        draft(cx).folder.value(),
        "/nowhere",
        "the premise: the folder is /nowhere"
    );
    assert!(!strip_shown(cx), "no setup matches /nowhere, so no strip");
}

/// An empty history shows no strip, and no heading either. Why: SPEC.md, the dialog "reserves no space for them when
/// it does not" match (old "composer reserves no space for recents when launch history is empty").
#[gpui::test]
fn an_empty_history_shows_no_strip(cx: &mut TestAppContext) {
    let (_helm, cx) = new_in_tmp(cx, &history(Vec::new()));
    assert!(!strip_shown(cx));
}

/// A click fills the setup and returns focus to the command field. This lets the user keep refining the draft.
/// Down moves only the highlight, and plain Enter on that row launches through the ordinary path. Modified or held
/// Enter leaves the launch for a fresh plain press.
#[gpui::test]
fn a_click_fills_and_enter_launches(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_in_tmp(cx, &two_setups());
    test_support::click(cx, "launcher-tabs-Command");
    assert_eq!(draft(cx).tab, Tab::Command, "the premise: the Command tab");
    test_support::click(cx, "launcher-recent-0");
    let filled = draft(cx);
    assert_eq!(filled.tab, Tab::Agent);
    assert_eq!(filled.agent.harness, Some(LaunchHarness::Codex));
    assert_eq!(filled.agent.model.as_deref(), Some("enter-model"));
    assert_eq!(filled.agent.effort, Some(LaunchEffort::High));
    assert_eq!(filled.agent.permissions, Some(LaunchPermission::Yolo));
    assert_eq!(filled.folder.value(), "/tmp");
    assert!(
        super::view_tests::search_focused(cx),
        "the click returned focus to the command field"
    );
    assert!(
        creates(&mut helm, cx).is_empty(),
        "a click launches nothing"
    );

    test_support::click(cx, "launcher-tabs-Command");
    test_support::click(cx, "launcher-search-row");
    keys(cx, "down");
    assert!(recent_highlighted(cx, "launcher-recent-0"));
    assert!(super::view_tests::search_focused(cx));
    keys(cx, "shift-enter ctrl-enter cmd-enter");
    assert!(recent_highlighted(cx, "launcher-recent-0"));
    assert_eq!(draft(cx).tab, Tab::Command, "modified Enter fills nothing");
    assert!(creates(&mut helm, cx).is_empty(), "and launches nothing");
    test_support::press(cx, "enter", true);
    assert_eq!(draft(cx).tab, Tab::Command, "a held Enter fills nothing");
    assert!(creates(&mut helm, cx).is_empty(), "and launches nothing");
    keys(cx, "enter");
    let sent = creates(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("one create, not {}", sent.len());
    };
    let body = create.body.as_ref().unwrap();
    assert_eq!(body["cwd"], json!("/tmp"));
    assert_eq!(body["launch"]["harness"], json!("codex"));
    assert_eq!(body["launch"]["model"], json!("enter-model"));
    assert_eq!(body["launch"]["permissions"], json!("yolo"));
}

/// A replacement installation can have the same recent setup. The old highlight cannot launch that setup.
#[gpui::test]
fn enter_on_a_withdrawn_empty_pane_recent_cannot_launch(cx: &mut TestAppContext) {
    let mut launch = LaunchDraft::new(Some(1), "/tmp".into(), None);
    launch.choose_harness(LaunchHarness::Codex, None, None);
    let (mut helm, cx) = launcher_with_hosts(cx, launch, fixture_hosts(&[0]));
    answer_history(&mut helm, cx, &two_setups());
    assert!(search_focused(cx));
    keys(cx, "down");
    let row = "launcher-recent-0";
    let setup = name_of(cx, row).expect("the old installation offers the setup");
    assert!(recent_highlighted(cx, row));

    retarget_history_host(&mut helm, cx);
    assert!(!strip_shown(cx));
    answer_history(&mut helm, cx, &two_setups());
    assert_eq!(name_of(cx, row).as_deref(), Some(setup.as_str()));
    assert!(!recent_highlighted(cx, row));
    assert!(search_focused(cx));
    keys(cx, "enter");
    assert_stale_choice(cx, "");
    assert!(draft(cx).agent.model.is_none());
    assert!(creates(&mut helm, cx).is_empty());
    keys(cx, "enter");
    let sent = creates(&mut helm, cx);
    let [create] = sent.as_slice() else {
        panic!("another Enter should launch the existing form: {sent:?}");
    };
    assert!(create.body.as_ref().unwrap()["launch"]["model"].is_null());
}

/// Click the rendered setup after the live store withdraws its host and before the command pane can redraw.
#[gpui::test]
fn clicking_a_withdrawn_empty_pane_recent_warns(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_in_tmp(cx, &two_setups());
    assert!(name_of(cx, "launcher-recent-0").is_some_and(|name| name.contains("enter-model")));
    let at = test_support::center(cx, "launcher-recent-0");
    cx.update(|window, cx| {
        withdraw_presented_choice::<HostView>("1", cx);
        test_support::click_before_redraw(at, window, cx);
    });
    settle(cx);
    assert_stale_choice(cx, "");
    assert!(draft(cx).agent.model.is_none());
    assert!(creates(&mut helm, cx).is_empty());
}

/// Up at the first recent gives Enter back to the draft, so browsing a saved setup cannot change the next launch.
#[gpui::test]
fn up_from_the_first_recent_restores_the_forms_enter(cx: &mut TestAppContext) {
    let mut draft = LaunchDraft::new(Some(1), "/tmp".into(), None);
    draft.choose_harness(LaunchHarness::Codex, None, None);
    let (mut helm, cx) = launcher_with_hosts(cx, draft, fixture_hosts(&[0]));
    answer_history(&mut helm, cx, &two_setups());
    keys(cx, "down");
    assert!(recent_highlighted(cx, "launcher-recent-0"));
    assert_eq!(
        name_of(cx, "launcher-recent-enter").as_deref(),
        Some("⏎"),
        "outside Replace with the cap is the bare key"
    );
    keys(cx, "up");
    assert!(!recent_highlighted(cx, "launcher-recent-0"));
    assert!(super::view_tests::search_focused(cx));
    keys(cx, "enter");
    let sent = creates(&mut helm, cx);
    let [create] = sent.as_slice() else {
        panic!("expected one create from the form: {sent:?}");
    };
    assert!(create.body.as_ref().unwrap()["launch"]["model"].is_null());
}

/// Invoking a recent option fills the form. Enter on the empty pane's highlighted recent can replace the source, so
/// that key cap must name the replacement. Query results only fill. Restart's rows explain why its destination is held.
#[gpui::test]
fn replace_rows_name_the_action_and_restart_rows_are_inactive(cx: &mut TestAppContext) {
    let source = super::draft_tests::session(codex(None, None, None));
    let (mut helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::replace_with("a", &source, Some(1)),
        fixture_hosts(&[0]),
    );
    let answer = history(vec![launched(
        "/src/project",
        &chosen(LaunchHarness::Codex, Some("recent-model"), None, None),
    )]);
    answer_history(&mut helm, cx, &answer);
    let name = name_of(cx, "launcher-recent-0").unwrap_or_default();
    assert!(
        name.starts_with("Fill from recent setup: /src/project · "),
        "{name}"
    );
    assert!(!recent_highlighted(cx, "launcher-recent-0"));
    test_support::click(cx, "launcher-search-row");
    keys(cx, "down");
    assert!(recent_highlighted(cx, "launcher-recent-0"));
    assert_eq!(
        name_of(cx, "launcher-recent-enter").as_deref(),
        Some("⏎ Replace"),
        "the highlighted row's cap names the replacement"
    );
    super::search_view_tests::type_in(cx, "recent:project");
    assert_eq!(
        name_of(cx, "launcher-recent-enter").as_deref(),
        Some("⏎ Fill")
    );
    assert!(draft(cx).agent.model.is_none());
    keys(cx, "enter");
    assert_eq!(draft(cx).agent.model.as_deref(), Some("recent-model"));
    assert!(super::launch_tests::sent_to(&mut helm, cx, "/api/sessions/a/replace").is_empty());

    // A Guarded source, so the strip's permission narrowing keeps the recent Codex setup (which composes as Guarded).
    // A source with no recorded permission now restarts as YOLO, and its strip would hide that setup.
    let mut source =
        super::draft_tests::session(codex(None, None, Some(LaunchPermission::Guarded)));
    source.restart_offer = RestartOffer::Resume;
    let (mut helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::restart_with("missing", &source, Some(1)).unwrap(),
        fixture_hosts(&[0]),
    );
    answer_history(&mut helm, cx, &answer);
    let before = draft(cx);
    let row = test_support::frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("launcher-recent-0"))
        .expect("Restart with shows the recent row");
    assert!(!row.flags.click, "held rows cannot be taken by clicking");
    assert!(
        recent_help(cx, "launcher-recent-0")
            .is_some_and(|help| help.contains("keeps destination fixed"))
    );
    test_support::click(cx, "launcher-search-row");
    keys(cx, "down enter");
    assert_eq!(draft(cx), before);
}

/// Read the real hover help after the layer tracker's delay has elapsed.
pub(super) fn recent_help(cx: &mut VisualTestContext, id: &'static str) -> Option<String> {
    let at = test_support::center(cx, id);
    cx.simulate_mouse_move(at, None, gpui::Modifiers::none());
    test_support::advance(cx, crate::layers::HOVER_DELAY);
    cx.update(|window, cx| crate::layers::shown_tooltip(window, cx))
        .map(|shown| shown.text.to_string())
}

/// A selected row is reported without moving focus out of the command field.
fn recent_highlighted(cx: &mut VisualTestContext, id: &str) -> bool {
    test_support::frame(cx)
        .iter()
        .any(|node| node.element_id.as_deref() == Some(id) && node.selected == Some(true))
}

/// Put the retired setup beside a usable one so a refusal can be tested both alone and before a second choice.
fn retired_and_usable_setups() -> serde_json::Value {
    let mut retired = chosen(LaunchHarness::Codex, None, None, None);
    retired.container = Some(
        serde_json::from_value(json!({ "engine": "podman", "image": "old", "api_proxy_port": 1 }))
            .unwrap(),
    );
    history(vec![
        launched("/tmp", &retired),
        launched("/tmp", &chosen(LaunchHarness::Claude, None, None, None)),
    ])
}

/// A setup carrying a retired container choice supports Delete only. Refuse it with that reason and keep the draft.
#[gpui::test]
fn a_retired_container_setup_is_refused(cx: &mut TestAppContext) {
    let (_helm, cx) = new_in_tmp(cx, &retired_and_usable_setups());
    let before = draft(cx);
    test_support::click(cx, "launcher-recent-0");
    assert_eq!(draft(cx), before, "nothing changed");
    assert!(
        name_of(cx, "launcher-choice-notice").is_some_and(|notice| notice.contains("Delete only")),
        "the shared reason is said under the agent choices"
    );
}

/// A usable choice clears the refusal from the previous row, so it never appears to describe the new draft.
#[gpui::test]
fn taking_a_usable_setup_clears_the_retired_container_refusal(cx: &mut TestAppContext) {
    let (_helm, cx) = new_in_tmp(cx, &retired_and_usable_setups());
    test_support::click(cx, "launcher-recent-0");
    assert!(
        name_of(cx, "launcher-choice-notice").is_some_and(|notice| notice.contains("Delete only"))
    );
    assert!(name_of(cx, "launcher-recent-1").is_some_and(|label| label.contains("Claude")));
    test_support::click(cx, "launcher-recent-1");
    assert_eq!(draft(cx).agent.harness, Some(LaunchHarness::Claude));
    assert_eq!(name_of(cx, "launcher-choice-notice"), None);
}

/// A row's cells name the agent type first and only the choices the setup set: an explicit model, effort and YOLO
/// say so, and a setup that set nothing but the agent's default Guarded mode reads "Defaults". Why: the row must be
/// told apart from its neighbours at a glance (old "composer recent rows lead with the harness" and "composer recent
/// rows list only explicit choices").
#[gpui::test]
fn a_row_names_its_agent_and_only_explicit_choices(cx: &mut TestAppContext) {
    let (_helm, cx) = new_in_tmp(cx, &two_setups());
    let nodes = test_support::frame(cx);
    let words = |id| {
        crate::session_list::host_rail::tests::within(&nodes, id)
            .iter()
            .flat_map(|node| node.texts.iter())
            .map(|line| line.text.as_ref())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let first = words("launcher-recent-0");
    assert!(first.contains("Codex") && first.contains("YOLO"), "{first}");
    assert!(first.contains("enter-model"), "{first}");
    let full = name_of(cx, "launcher-recent-0").unwrap();
    assert!(
        full.contains("Effort: High"),
        "the truncated choice remains accessible: {full}"
    );
    let second = words("launcher-recent-1");
    assert!(
        second.contains("Claude") && second.contains("Defaults"),
        "{second}"
    );
}

/// A permission the launcher only remembered from the last launch does not narrow the rows. Why: a remembered value is
/// not a choice, and hiding setups for it would hide what the user came for (the old regression where a remembered YOLO
/// hid every setup that left its permission to the default).
#[gpui::test]
fn a_remembered_permission_does_not_narrow_the_rows(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::new(Some(1), "/tmp".into(), Some(LaunchPermission::Yolo)),
        fixture_hosts(&[0]),
    );
    answer_history(&mut helm, cx, &two_setups());
    assert_eq!(
        draft(cx).agent.permissions,
        Some(LaunchPermission::Yolo),
        "the premise: the remembered permission is the draft's"
    );
    assert!(
        name_of(cx, "launcher-recent-1").is_some_and(|name| name.contains("Claude")),
        "Claude's Guarded setup still shows"
    );
}

/// Taking a checkout setup asks for a fresh managed checkout of its repository, never the folder an earlier checkout
/// happened to get. Why: that folder belonged to that session; a new launch gets a new checkout.
#[gpui::test]
fn a_checkout_setup_chooses_its_repository(cx: &mut TestAppContext) {
    let mut checkout = launched(
        "/work/bar-1",
        &chosen(LaunchHarness::Codex, None, None, None),
    );
    checkout["github_repo"] = json!({ "owner": "acme", "name": "bar" });
    let (_helm, cx) = new_in_tmp(cx, &history(vec![checkout]));
    test_support::click(cx, "launcher-recent-0");
    let repo = draft(cx).checkout.and_then(|checkout| checkout.repo);
    assert_eq!(
        repo.map(|repo| (repo.owner, repo.name)),
        Some(("acme".to_owned(), "bar".to_owned()))
    );
}

/// The highlight follows the setup when the draft narrows the list. Launching whatever moved into the old position
/// could be a different host or directory.
#[gpui::test]
fn focus_follows_the_setup_when_the_rows_change(cx: &mut TestAppContext) {
    let codex = chosen(LaunchHarness::Codex, None, Some(LaunchEffort::High), None);
    let mut checkout = launched("/work/bar-1", &codex);
    checkout["github_repo"] = json!({ "owner": "acme", "name": "bar" });
    let answer = history(vec![
        launched("/tmp", &chosen(LaunchHarness::Claude, None, None, None)),
        launched("/tmp", &codex),
        checkout,
    ]);
    let (mut helm, cx) = new_in_tmp(cx, &answer);
    assert!(
        name_of(cx, "launcher-recent-1").is_some_and(|name| name.starts_with("/tmp · ")),
        "the premise: the Codex folder setup is the second row"
    );
    keys(cx, "down down");
    assert!(recent_highlighted(cx, "launcher-recent-1"));
    cx.update(|_, cx| {
        let launcher = crate::layers::last_opened::<super::view::Launcher>(cx).unwrap();
        launcher.update(cx, |launcher, cx| {
            launcher
                .draft
                .choose_harness(LaunchHarness::Codex, None, None);
            cx.notify();
        });
    });
    settle(cx);
    assert!(
        name_of(cx, "launcher-recent-1").is_some_and(|name| name.starts_with("gh:acme/bar")),
        "the premise: the checkout setup now sits in the second place"
    );
    assert!(
        recent_highlighted(cx, "launcher-recent-0"),
        "the highlight moved with the setup"
    );
    keys(cx, "enter");
    let sent = creates(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("one create, not {}", sent.len());
    };
    assert_eq!(
        create.body.as_ref().unwrap()["cwd"],
        json!("/tmp"),
        "the folder setup launched"
    );
}

/// One custom model with a direction override, to distinguish its displayed spelling from its launch value.
const HIDDEN_MODEL: &str = "m\u{202e}x";

/// A recent setup carrying the model that both the row and the launcher field must present safely.
fn hidden_model_setup() -> serde_json::Value {
    history(vec![launched(
        "/tmp",
        &chosen(LaunchHarness::Codex, Some(HIDDEN_MODEL), None, None),
    )])
}

/// Escape the custom model in the row's accessible name, where a hidden direction mark could reorder the words.
#[gpui::test]
fn a_row_names_a_custom_model_escaped(cx: &mut TestAppContext) {
    let (_helm, cx) = new_in_tmp(cx, &hidden_model_setup());
    let name = name_of(cx, "launcher-recent-0").unwrap_or_default();
    assert!(name.contains("Model: m<U+202E>x"), "{name}");
    assert!(!name.contains('\u{202e}'), "{name}");
}

/// The visible name distinguishes recent agents before selection, and its saved ID can still be typed as a query
/// even when the current display name spells it differently. Both command layouts draw the same identity.
#[gpui::test]
fn recent_custom_agent_uses_its_current_name_in_both_layouts(cx: &mut TestAppContext) {
    use farhelm_proto::ui_channel::{Collection, CollectionState, KeyedEntry};
    let agent = super::view_tests::acme_agent();
    let mut setup = chosen(LaunchHarness::Custom, None, None, None);
    setup.custom_agent = Some(agent.id.clone());
    let folder = format!("/tmp/{}agent-checkout", "build-artifacts/".repeat(12));
    let (mut helm, cx) = super::view_tests::launcher_over(
        cx,
        LaunchDraft::new(Some(1), folder.clone(), None),
        vec![
            fixture_hosts(&[0]),
            crate::session_list::tests::preferences(&[], None),
            CollectionState {
                collection: Collection::CustomAgents,
                meta: test_support::meta(&[&agent.id]),
                entries: vec![KeyedEntry {
                    key: agent.id.clone(),
                    value: serde_json::to_value(agent).unwrap(),
                }],
            },
        ],
    );
    answer_history(&mut helm, cx, &history(vec![launched(&folder, &setup)]));
    let paints_name = |cx: &mut VisualTestContext, row| {
        assert!(
            crate::session_list::host_rail::tests::texts_within(&test_support::frame(cx), row)
                .iter()
                .any(|line| line.text.contains("Acme Code") && !line.visible_bounds.is_empty())
        );
        assert!(name_of(cx, row).is_some_and(|name| name.contains("Acme Code (acme-code)")));
    };
    paints_name(cx, "launcher-recent-0");
    super::search_view_tests::type_in(cx, "recent:acme-code");
    paints_name(cx, "launcher-search-option-0");
    cx.simulate_resize(crate::bootstrap::MIN_WINDOW_SIZE);
    settle(cx);
    paints_name(cx, "launcher-search-option-0");
    assert!(creates(&mut helm, cx).is_empty());
}

/// Picking the row fills the draft without launching. The field shows the model escaped and Launch sends its raw value.
#[gpui::test]
fn a_recent_model_fills_escaped_and_launches_verbatim(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_in_tmp(cx, &hidden_model_setup());
    test_support::click(cx, "launcher-recent-0");
    assert_eq!(super::view_tests::model_text(cx), "m<U+202E>x");
    assert!(
        creates(&mut helm, cx).is_empty(),
        "the click fills the draft"
    );
    test_support::click(cx, "launcher-launch");
    let sent = creates(&mut helm, cx);
    let [create] = sent.as_slice() else {
        panic!("expected one create with the saved model: {sent:?}");
    };
    assert_eq!(
        create.body.as_ref().unwrap()["launch"]["model"],
        HIDDEN_MODEL
    );
}
