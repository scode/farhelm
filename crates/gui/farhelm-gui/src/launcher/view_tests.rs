//! Tests of the launcher's dialog in the real main view over the helm client's fake: what it opens on, how input
//! reaches the draft, the tabs, and what a press of the main action shows.
//!
//! The launcher is opened through [`super::view::open`] from inside the window, as an entry point will open it; the
//! tests reach the open launcher through [`Opened`], which only test builds record. Every step ends in one of the
//! shared harness's step helpers, so the focus and layer invariants are checked after each.

use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::{Collection, CollectionState, HostView, KeyedEntry};
use farhelm_proto::{
    LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection, RestartOffer, SessionLaunch,
};
use gpui::{Global, TestAppContext, VisualTestContext, WeakEntity};

use super::draft::{LaunchDraft, Tab};
use super::view::{Launcher, open};
use crate::focus::Mark;
use crate::main_view::MainView;
use crate::session_list::tests::{listing, preferences, session};
use crate::test_support::{self, focused_id, keys, meta, settle, started};

/// The launcher [`super::view::open`] opened last, recorded in test builds only.
pub(super) struct Opened(pub(super) WeakEntity<Launcher>);

impl Global for Opened {}

/// The helm's pinned host-list fixture's first host (id 1, "host1", connected), as the hosts collection.
fn hosts() -> CollectionState {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../farhelm-helm/http-contract/host-list.json"
    ))
    .unwrap();
    let host: HostView = serde_json::from_value(fixture["hosts"][0].clone()).unwrap();
    CollectionState {
        collection: Collection::Hosts,
        meta: meta(&["1"]),
        entries: vec![KeyedEntry {
            key: "1".into(),
            value: serde_json::to_value(host).unwrap(),
        }],
    }
}

/// The real main view over the fake helm with one host, session `a` and the preferences, and the launcher opened on
/// `draft`, checked once settled.
fn launcher_on(cx: &mut TestAppContext, draft: LaunchDraft) -> (FakeHelm, &mut VisualTestContext) {
    let (mut helm, _connection) = started(cx);
    helm.connect(vec![
        listing(&["a"], &[session("a")]),
        hosts(),
        preferences(&[], None),
    ]);
    cx.run_until_parked();
    let (_, window) = test_support::open_view(cx, MainView::new);
    window.update(|window, cx| {
        let mark = Mark::take(cx);
        assert!(
            open(draft, &mark, window, cx).is_some(),
            "the launcher opened"
        );
    });
    settle(window);
    (helm, window)
}

/// Run `read` on the open launcher.
fn read<T>(
    cx: &mut VisualTestContext,
    read: impl FnOnce(&Launcher, &gpui::Window, &gpui::App) -> T,
) -> T {
    cx.update(|window, cx| {
        let launcher = cx
            .try_global::<Opened>()
            .and_then(|opened| opened.0.upgrade())
            .expect("a launcher is open");
        read(launcher.read(cx), window, cx)
    })
}

/// The open launcher's draft.
fn draft(cx: &mut VisualTestContext) -> LaunchDraft {
    read(cx, |launcher, _, _| launcher.draft.clone())
}

/// Whether the launcher is still open: its view lives as long as its layer does.
fn is_open(cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| {
        cx.try_global::<Opened>()
            .is_some_and(|opened| opened.0.upgrade().is_some())
    })
}

/// The text fields a test clicks into.
#[derive(Clone, Copy)]
enum Field {
    Folder,
    Wrapper,
    Prompt,
}

/// Whether `field` holds focus.
fn focused(cx: &mut VisualTestContext, field: Field) -> bool {
    read(cx, |launcher, window, cx| {
        let state = match field {
            Field::Folder => &launcher.folder,
            Field::Wrapper => &launcher.wrapper,
            Field::Prompt => &launcher.prompt,
        };
        state.focus_handle(cx).is_focused(window)
    })
}

/// Whether the folder field holds focus.
fn folder_focused(cx: &mut VisualTestContext) -> bool {
    focused(cx, Field::Folder)
}

/// Whether an element was drawn under `id` in a fresh frame.
fn has_node(cx: &mut VisualTestContext, id: &str) -> bool {
    test_support::frame(cx)
        .into_iter()
        .any(|node| node.element_id.as_deref() == Some(id))
}

/// The accessible name of the element drawn under `id` in a fresh frame.
fn name_of(cx: &mut VisualTestContext, id: &str) -> Option<String> {
    test_support::frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .and_then(|node| node.label.map(|label| label.to_string()))
}

/// Whether the element under `selector` was drawn in the last frame.
fn drawn(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.debug_bounds(selector).is_some()
}

/// The text of the refusal shown beside the main action, if one is.
fn shown_refusal(cx: &mut VisualTestContext) -> Option<String> {
    name_of(cx, "launcher-refusal")
}

/// A Codex launch carrying `model`, `effort` and `permissions`, with a stored resume command.
fn codex(
    model: Option<&str>,
    effort: Option<LaunchEffort>,
    permissions: Option<LaunchPermission>,
) -> SessionLaunch {
    SessionLaunch::Agent {
        selection: LaunchSelection {
            harness: LaunchHarness::Codex,
            model: model.map(str::to_owned),
            effort,
            permissions,
            permission_flags: None,
            workspace_trust: None,
            wrapper_prefix: None,
            container: None,
        },
        start: vec!["codex".into(), "{farhelm_args}".into()],
        resume: Some(vec![
            "codex".into(),
            "{conversation}".into(),
            "{farhelm_args}".into(),
        ]),
    }
}

/// SPEC.md, "Creation": New opens a dialog on the Agent tab with no agent type, the destination rows first, and focus
/// on the first field (the search box's place until it exists). The header names the mode.
#[gpui::test]
fn new_opens_on_the_agent_tab_with_focus_in_the_first_field(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    assert!(drawn(cx, "launcher"), "the launcher's dialog is drawn");
    assert!(drawn(cx, "launcher-folder-row") && drawn(cx, "launcher-name-row"));
    assert!(drawn(cx, "launcher-tabs"));
    assert!(
        drawn(cx, "launcher-wrapper-row"),
        "the Agent tab's fields show"
    );
    assert!(
        !drawn(cx, "launcher-command-row"),
        "the Command tab's do not"
    );
    assert_eq!(draft(cx).tab, Tab::Agent);
    assert!(folder_focused(cx), "focus starts in the folder field");
}

/// The footer says what Launch would start in the old footer's words, and Launch's accessible name keeps them after its
/// verb (SPEC.md, "Creation": the main action's accessible name keeps the operation and target; long summaries keep
/// their full text accessible). Why this matters: the visible Launch says only "Launch", so a screen reader user learns
/// the target from these names alone. Specified: New with no agent type names no agent ("host1 · ~") and its choices
/// all "Default"; the Command tab opens with "Command" and has no line of choices.
#[gpui::test]
fn the_footer_names_what_launch_would_start(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    assert_eq!(
        name_of(cx, "launcher-summary-target").as_deref(),
        Some("host1 · ~")
    );
    assert_eq!(
        name_of(cx, "launcher-launch").as_deref(),
        Some("Launch host1 · ~")
    );
    assert_eq!(
        name_of(cx, "launcher-summary-choices").as_deref(),
        Some("Model: Default · Effort: Default · Permissions: Default")
    );
    test_support::click(cx, "launcher-tabs-Command");
    assert_eq!(
        name_of(cx, "launcher-summary-target").as_deref(),
        Some("Command · host1 · ~")
    );
    assert_eq!(
        name_of(cx, "launcher-summary-choices"),
        None,
        "no choices line on the Command tab"
    );
}

/// The footer of an agent launch with choices: a Clone of a Codex session names Codex, and its line of choices reads
/// its model, effort and permissions in the old footer's words (effort "High", not the command line's "high"; YOLO as
/// "YOLO"), then the line count of a typed prompt.
#[gpui::test]
fn the_footer_reads_an_agent_launchs_choices(cx: &mut TestAppContext) {
    let source = super::draft_tests::session(codex(
        Some("gpt-5"),
        Some(LaunchEffort::High),
        Some(LaunchPermission::Yolo),
    ));
    let (_helm, cx) = launcher_on(cx, LaunchDraft::clone_of(&source, Some(1)));
    assert_eq!(
        name_of(cx, "launcher-summary-target").as_deref(),
        Some("Codex · host1 · /src/project")
    );
    test_support::click(cx, "launcher-prompt-row");
    assert!(
        focused(cx, Field::Prompt),
        "the premise: the click focused the prompt"
    );
    keys(cx, "h i enter t h e r e");
    assert_eq!(
        name_of(cx, "launcher-summary-choices").as_deref(),
        Some("Model: gpt-5 · Effort: High · Permissions: YOLO · Prompt: 2 lines")
    );
}

/// A press of Launch on an incomplete draft says why beside it, "Not launched." first (SPEC.md, "Creation": "an
/// incomplete attempt says why beside Launch"); the reason is not shown before the press, and an edit takes it away
/// until the next press.
#[gpui::test]
fn a_refused_press_says_why_beside_the_main_action(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    assert_eq!(shown_refusal(cx), None, "nothing is said before a press");
    test_support::click(cx, "launcher-launch");
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Not launched. Choose a structured harness before launching")
    );
    test_support::click(cx, "launcher-folder-row");
    assert!(
        folder_focused(cx),
        "the premise: the click focused the folder field"
    );
    keys(cx, "x");
    assert_eq!(shown_refusal(cx), None, "an edit takes the reason away");
    keys(cx, "enter");
    assert!(
        shown_refusal(cx).is_some(),
        "Enter in a single-line field presses the main action"
    );
}

/// Typing in a field is the draft's edit: the folder typed is the user's, so a host change would keep it.
#[gpui::test]
fn typing_in_the_folder_edits_the_draft(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "".into(), None));
    assert!(
        folder_focused(cx),
        "the premise: focus starts in the folder field"
    );
    keys(cx, "/ s r c");
    let draft = draft(cx);
    assert_eq!(draft.folder.value(), "/src");
    assert!(draft.folder.is_touched());
}

/// The tabs (SPEC.md: "two tabs, Agent and Command, each showing only its own fields; switching tabs keeps each tab's
/// draft"): Right on the focused tab switches to Command with focus on it, its fields show, and the Agent tab's typed
/// wrapper prefix is still there, in the draft and in its field, when the user comes back with Home.
#[gpui::test]
fn the_tabs_switch_fields_and_keep_each_draft(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    test_support::click(cx, "launcher-wrapper-row");
    assert!(
        focused(cx, Field::Wrapper),
        "the premise: the click focused the wrapper prefix"
    );
    keys(cx, "n i c e");
    test_support::click(cx, "launcher-tabs-Agent");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("launcher-tabs-Agent"),
        "the premise: the click focused the Agent tab"
    );
    keys(cx, "right");
    assert_eq!(draft(cx).tab, Tab::Command);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("launcher-tabs-Command"),
        "focus went with the choice"
    );
    assert!(drawn(cx, "launcher-command-row") && !drawn(cx, "launcher-wrapper-row"));
    keys(cx, "home");
    assert_eq!(draft(cx).tab, Tab::Agent);
    assert_eq!(draft(cx).agent.wrapper, "nice");
    assert_eq!(
        read(cx, |launcher, _, cx| launcher.wrapper.text(cx).to_string()),
        "nice"
    );
}

/// Restart with starts on Cancel, hides the tabs (it holds the launch kind) and the initial prompt (it holds that too;
/// a Codex start offers one elsewhere), and a refused press says "Restart error." first (SPEC.md, "Restart with" and
/// "Creation").
#[gpui::test]
fn restart_with_starts_on_cancel_and_says_restart_error(cx: &mut TestAppContext) {
    let mut source = super::draft_tests::session(codex(None, None, None));
    source.restart_offer = RestartOffer::Resume;
    let restart = LaunchDraft::restart_with("missing", &source, Some(1)).unwrap();
    let (_helm, cx) = launcher_on(cx, restart);
    assert!(
        !drawn(cx, "launcher-tabs"),
        "Restart with holds the launch kind"
    );
    assert!(!drawn(cx, "launcher-prompt-row"), "and the initial prompt");
    assert!(
        !drawn(cx, "launcher-name-row"),
        "and the name, whose field is not drawn"
    );
    assert_eq!(
        name_of(cx, "launcher-folder-held").as_deref(),
        Some("/src/project"),
        "the held folder is shown as text"
    );
    let cancel_focused = read(cx, |launcher, window, _| {
        launcher.cancel_focus.is_focused(window)
    });
    assert!(cancel_focused, "Restart with starts on Cancel");
    test_support::click(cx, "launcher-launch");
    let refusal = shown_refusal(cx).unwrap_or_default();
    assert!(refusal.starts_with("Restart error. "), "{refusal}");
}

/// Reset choices puts the wrapper prefix back to New's (empty) and the field follows the draft.
#[gpui::test]
fn reset_choices_empties_the_wrapper_field(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    test_support::click(cx, "launcher-wrapper-row");
    assert!(
        focused(cx, Field::Wrapper),
        "the premise: the click focused the wrapper prefix"
    );
    keys(cx, "n i c e");
    assert_eq!(
        draft(cx).agent.wrapper,
        "nice",
        "the premise: the prefix was typed"
    );
    test_support::click(cx, "launcher-reset");
    assert_eq!(draft(cx).agent.wrapper, "");
    let shown = read(cx, |launcher, _, cx| launcher.wrapper.text(cx).to_string());
    assert_eq!(shown, "");
}

/// Cancel, the header Close and Escape each close the launcher, discarding its draft (SPEC.md, "Creation"; the old
/// Cancel: "close without starting anything"). Why this matters: a launcher that stayed open, or kept its draft for the
/// next New, would leave the user's way out broken.
#[gpui::test]
fn cancel_close_and_escape_close_the_launcher(cx: &mut TestAppContext) {
    for way in ["cancel", "close", "escape"] {
        let (_helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
        assert!(is_open(cx), "{way}: the premise: the launcher is open");
        match way {
            "cancel" => test_support::click(cx, "launcher-cancel"),
            "close" => test_support::click(cx, "launcher-close"),
            _ => keys(cx, "escape"),
        }
        settle(cx);
        assert!(!is_open(cx), "{way} closed the launcher");
    }
}

/// Clear initial prompt empties the prompt in the draft and its field, and goes away with it (the old launcher's
/// button under a non-empty prompt).
#[gpui::test]
fn clear_initial_prompt_empties_the_prompt(cx: &mut TestAppContext) {
    let source = super::draft_tests::session(codex(None, None, None));
    let (_helm, cx) = launcher_on(cx, LaunchDraft::clone_of(&source, Some(1)));
    assert!(
        !drawn(cx, "launcher-clear-prompt"),
        "nothing to clear while the prompt is empty"
    );
    test_support::click(cx, "launcher-prompt-row");
    assert!(
        focused(cx, Field::Prompt),
        "the premise: the click focused the prompt"
    );
    keys(cx, "h i");
    test_support::click(cx, "launcher-clear-prompt");
    assert_eq!(draft(cx).prompt, "");
    assert_eq!(
        read(cx, |launcher, _, cx| launcher.prompt.text(cx).to_string()),
        ""
    );
    assert!(!drawn(cx, "launcher-clear-prompt"));
}

/// A wrapper prefix that does not parse is said at once under its field, not only after a press, as the old launcher's
/// inline error was; one that parses says nothing.
#[gpui::test]
fn a_wrapper_prefix_that_does_not_parse_is_said_at_once(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    test_support::click(cx, "launcher-wrapper-row");
    assert!(
        focused(cx, Field::Wrapper),
        "the premise: the click focused the wrapper prefix"
    );
    keys(cx, "n i c e");
    assert!(
        !has_node(cx, "launcher-wrapper-error"),
        "a prefix that parses says nothing"
    );
    keys(cx, "space '");
    assert!(
        has_node(cx, "launcher-wrapper-error"),
        "an unclosed quote is said under the field"
    );
}
