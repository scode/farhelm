//! Tests of the recent setups strip in the real launcher over the helm client's fake: when it shows, what a row says,
//! and what a click and Enter on a row do. The rules for which setups and in what order are `recents_tests`'.
//!
//! Ported from the composer cases of the old `e2e/tests/sidebar.spec.ts` at reference commit `24bc0580` ("composer
//! recent slots appear only with matches", "composer Enter on a focused recent launches the filled setup", "composer
//! recent rows lead with the harness"), and the Replace with hint of `replace.spec.ts`.

use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::{LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection, RestartOffer};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::draft::{LaunchDraft, Tab};
use super::view_tests::{codex, draft, fixture_hosts, launcher_with_hosts, name_of};
use crate::test_support::{self, keys, settle};

/// `harness` launched with `model`, `effort` and `permissions`.
pub(super) fn chosen(
    harness: LaunchHarness,
    model: Option<&str>,
    effort: Option<LaunchEffort>,
    permissions: Option<LaunchPermission>,
) -> LaunchSelection {
    LaunchSelection {
        harness,
        model: model.map(str::to_owned),
        effort,
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

/// Whether the strip is drawn.
fn strip_shown(cx: &mut VisualTestContext) -> bool {
    test_support::frame(cx);
    cx.debug_bounds("launcher-recents").is_some()
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
    test_support::click(cx, "launcher-folder-row");
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

/// A click fills the setup (the Agent tab, its choices and folder) and launches nothing; Enter on the focused row
/// launches the filled setup, once, through the ordinary path; a held Enter does nothing. Why: SPEC.md: "clicking it
/// never launches, and pressing Enter on a focused row launches the filled setup through the ordinary Launch path"
/// (old "composer Enter on a focused recent launches the filled setup").
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
        creates(&mut helm, cx).is_empty(),
        "a click launches nothing"
    );

    test_support::click(cx, "launcher-tabs-Command");
    test_support::click(cx, "launcher-search-row");
    // Tab from the search box reaches the first row.
    keys(cx, "tab");
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("launcher-recent-0"),
        "the premise: Tab from search lands on the first row"
    );
    cx.simulate_event(gpui::KeyDownEvent {
        native_key: None,
        keystroke: gpui::Keystroke::parse("enter").unwrap(),
        is_held: true,
        prefer_character_input: false,
    });
    settle(cx);
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

/// Replace with names a row "Replace: …"; Restart with shows no strip. Why: in Replace with Enter on a row replaces the
/// source, which its name must say, and Restart with keeps its setup (old `replace.spec.ts`'s recent row, and the old
/// strip's `!restart_mode`).
#[gpui::test]
fn replace_with_says_replace_and_restart_with_has_no_strip(cx: &mut TestAppContext) {
    let source = super::draft_tests::session(codex(None, None, None));
    let (mut helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::replace_with("a", &source, Some(1)),
        fixture_hosts(&[0]),
    );
    let answer = history(vec![launched(
        "/src/project",
        &chosen(LaunchHarness::Codex, None, None, None),
    )]);
    answer_history(&mut helm, cx, &answer);
    let name = name_of(cx, "launcher-recent-0").unwrap_or_default();
    assert!(name.starts_with("Replace: /src/project · "), "{name}");
    assert_eq!(
        name_of(cx, "launcher-recent-0-hint"),
        None,
        "no hint while the row lacks keyboard focus"
    );
    test_support::click(cx, "launcher-search-row");
    keys(cx, "tab");
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("launcher-recent-0"),
        "the premise: Tab from search reached the row"
    );
    assert_eq!(
        name_of(cx, "launcher-recent-0-hint").as_deref(),
        Some("⏎ Replace")
    );

    let mut source = super::draft_tests::session(codex(None, None, None));
    source.restart_offer = RestartOffer::Resume;
    let (mut helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::restart_with("missing", &source, Some(1)).unwrap(),
        fixture_hosts(&[0]),
    );
    answer_history(&mut helm, cx, &answer);
    assert!(!strip_shown(cx), "Restart with has no strip");
}

/// A setup carrying a retired container choice is refused with the shared reason under the agent choices, and the
/// draft is unchanged. Why: such a setup supports Delete only (SPEC.md, "Retired containerized sessions").
#[gpui::test]
fn a_retired_container_setup_is_refused(cx: &mut TestAppContext) {
    let mut retired = chosen(LaunchHarness::Codex, None, None, None);
    retired.container = Some(
        serde_json::from_value(json!({ "engine": "podman", "image": "old", "api_proxy_port": 1 }))
            .unwrap(),
    );
    let (_helm, cx) = new_in_tmp(cx, &history(vec![launched("/tmp", &retired)]));
    let before = draft(cx);
    test_support::click(cx, "launcher-recent-0");
    assert_eq!(draft(cx), before, "nothing changed");
    assert!(
        name_of(cx, "launcher-choice-notice").is_some_and(|notice| notice.contains("Delete only")),
        "the shared reason is said under the agent choices"
    );
}

/// A row's cells name the agent type first and only the choices the setup set: an explicit model, effort and YOLO
/// say so, and a setup that set nothing but the agent's default Guarded mode reads "Defaults". Why: the row must be
/// told apart from its neighbours at a glance (old "composer recent rows lead with the harness" and "composer recent
/// rows list only explicit choices").
#[gpui::test]
fn a_row_names_its_agent_and_only_explicit_choices(cx: &mut TestAppContext) {
    let (_helm, cx) = new_in_tmp(cx, &two_setups());
    assert_eq!(
        name_of(cx, "launcher-recent-0-harness").as_deref(),
        Some("Codex")
    );
    assert_eq!(
        name_of(cx, "launcher-recent-0-choices").as_deref(),
        Some("Model: enter-model · Effort: High · Permissions: YOLO")
    );
    assert_eq!(
        name_of(cx, "launcher-recent-1-harness").as_deref(),
        Some("Claude")
    );
    assert_eq!(
        name_of(cx, "launcher-recent-1-choices").as_deref(),
        Some("Defaults")
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

/// Keyboard focus follows the setup, not the row's place: a fill re-filters the rows, and Enter then launches the
/// setup the user moved to, though another setup now sits in its old place. Why: Enter on a row launches; launching a
/// setup the user never moved to would start the wrong session (the old launcher guarded against the same).
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
    keys(cx, "tab tab");
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("launcher-recent-1"),
        "the premise: the second row has focus"
    );
    keys(cx, "space");
    assert_eq!(
        draft(cx).agent.harness,
        Some(LaunchHarness::Codex),
        "the premise: Space filled Codex"
    );
    assert!(
        name_of(cx, "launcher-recent-1").is_some_and(|name| name.starts_with("gh:acme/bar")),
        "the premise: the checkout setup now sits in the second place"
    );
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("launcher-recent-0"),
        "focus moved with the Codex folder setup to the first place"
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

/// A row's name and hover help draw a custom model id escaped, as every host value is. Why: the model id is whatever
/// the user typed, and a hidden direction mark in it would otherwise reorder the words a screen reader and the hover
/// help say (SPEC.md: host text is escaped "including in accessible names").
#[gpui::test]
fn a_row_names_a_custom_model_escaped(cx: &mut TestAppContext) {
    let answer = history(vec![launched(
        "/tmp",
        &chosen(LaunchHarness::Codex, Some("m\u{202e}x"), None, None),
    )]);
    let (_helm, cx) = new_in_tmp(cx, &answer);
    let name = name_of(cx, "launcher-recent-0").unwrap_or_default();
    assert!(name.contains("Model: m<U+202E>x"), "{name}");
    assert!(!name.contains('\u{202e}'), "{name}");
}
