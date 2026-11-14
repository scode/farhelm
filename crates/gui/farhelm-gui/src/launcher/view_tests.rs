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

/// The checked state an element under `id` reports in a fresh frame.
fn toggled(cx: &mut VisualTestContext, id: &str) -> Option<gpui::Toggled> {
    test_support::frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .and_then(|node| node.toggled)
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
    assert!(
        has_node(cx, "launcher-folder"),
        "the folder is an editable field"
    );
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

/// A host the store does not list, and no host at all, are "Unavailable host" in the footer, the old footer's words, so
/// the target never silently loses its host part.
#[gpui::test]
fn the_footer_says_unavailable_host_for_a_host_it_cannot_name(cx: &mut TestAppContext) {
    for host in [Some(99), None] {
        let (_helm, cx) = launcher_on(cx, LaunchDraft::new(host, "~".into(), None));
        assert_eq!(
            name_of(cx, "launcher-summary-target").as_deref(),
            Some("Unavailable host · ~"),
            "host {host:?}"
        );
    }
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
    assert!(
        !has_node(cx, "launcher-folder"),
        "and its field is not drawn"
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

// ---- The Agent tab's choices -----------------------------------------------------------------------------------------

/// Answer the launcher's catalog read with `rows` (`GET /api/launch-catalog`), or with a 500 when `rows` is `None`.
/// The fake answers commands in the order they were sent, so every other command sent since the last call (the main
/// view's own) is answered too, in its place, with a plain success, as the helm answers them.
fn answer_catalog(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    rows: Option<serde_json::Value>,
) {
    cx.run_until_parked();
    let mut read = false;
    for command in helm.commands() {
        if command.path.ends_with("launch-catalog") {
            read = true;
            match &rows {
                Some(rows) => helm.reply(command.id, 200, Some(rows.clone())),
                None => helm.reply(command.id, 500, None),
            }
        } else {
            helm.reply(command.id, 200, None);
        }
    }
    assert!(read, "the premise: the launcher read the catalog");
    settle(cx);
}

/// A catalog with one Codex model offering `efforts`.
fn codex_catalog(efforts: &[&str]) -> serde_json::Value {
    serde_json::json!([{ "id": "gpt-5", "harness": "codex", "efforts": efforts }])
}

/// Whether a fresh frame painted `words` anywhere.
fn painted(cx: &mut VisualTestContext, words: &str) -> bool {
    test_support::frame(cx)
        .into_iter()
        .any(|node| node.texts.iter().any(|text| text.text.contains(words)))
}

/// A harness chip chooses its agent type, which then offers its own effort and permissions (SPEC.md, "Creation": the
/// Agent tab's choices follow the agent type; the old launcher's harness row, effort and permission segments). Why
/// this matters: an effort or a permission the agent type does not offer would be refused at launch with no way to
/// see why. Specified: before any agent type, effort offers only Default and permissions Guarded and YOLO; after Codex,
/// with a catalog model offering low, medium and high, effort offers Default and those three, and permissions Guarded,
/// YOLO and Custom with the "About permission modes" help beside them.
#[gpui::test]
fn a_harness_chip_chooses_the_agent_and_its_offered_choices(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    answer_catalog(
        &mut helm,
        cx,
        Some(codex_catalog(&["low", "medium", "high"])),
    );
    assert!(has_node(cx, "launcher-effort-Default") && !has_node(cx, "launcher-effort-High"));
    assert!(
        has_node(cx, "launcher-permissions-Guarded") && has_node(cx, "launcher-permissions-YOLO")
    );
    assert_eq!(
        toggled(cx, "launcher-permissions-Guarded"),
        Some(gpui::Toggled::True),
        "before any agent type, New's default asks for Guarded and shows it chosen"
    );
    assert!(
        !has_node(cx, "launcher-permissions-Custom"),
        "Custom waits for an agent type that offers it"
    );

    test_support::click(cx, "launcher-harness-Codex");
    assert_eq!(draft(cx).agent.harness, Some(LaunchHarness::Codex));
    for effort in ["Default", "Low", "Medium", "High"] {
        let id = format!("launcher-effort-{effort}");
        assert!(has_node(cx, &id), "effort {effort} is offered");
    }
    for permission in ["Guarded", "YOLO", "Custom"] {
        let id = format!("launcher-permissions-{permission}");
        assert!(has_node(cx, &id), "permission {permission} is offered");
    }
    assert!(has_node(cx, "launcher-permission-help"));
    test_support::click(cx, "launcher-effort-High");
    assert_eq!(draft(cx).agent.effort, Some(LaunchEffort::High));
}

/// More than five effort choices, Default included, are a select instead of segments, as the old launcher switched to
/// a native select past five.
#[gpui::test]
fn many_efforts_are_a_select(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    answer_catalog(
        &mut helm,
        cx,
        Some(codex_catalog(&["low", "medium", "high", "xhigh", "max"])),
    );
    test_support::click(cx, "launcher-harness-Codex");
    assert!(
        has_node(cx, "launcher-effort"),
        "the effort control is drawn"
    );
    assert!(
        !has_node(cx, "launcher-effort-Default"),
        "and it is not segments"
    );
}

/// Enter on a harness chip chooses it and then presses Launch (SPEC.md, "Creation": "Enter on a choice button ...
/// applies that choice and then launches"). Specified, with no host chosen so the press is refused and says why: Tab
/// from the focused Codex chip reaches Claude, and Enter there chooses Claude and shows the refusal.
#[gpui::test]
fn enter_on_a_harness_chip_chooses_it_and_presses_launch(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_on(cx, LaunchDraft::new(None, "~".into(), None));
    test_support::click(cx, "launcher-harness-Codex");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("launcher-harness-Codex"),
        "the premise: the click focused the chip"
    );
    keys(cx, "tab");
    assert_eq!(focused_id(cx).as_deref(), Some("launcher-harness-Claude"));
    assert_eq!(shown_refusal(cx), None, "nothing said before the press");
    keys(cx, "enter");
    assert_eq!(draft(cx).agent.harness, Some(LaunchHarness::Claude));
    assert!(
        shown_refusal(cx).is_some(),
        "the press said why nothing launched"
    );
}

/// Choosing Custom opens its flags editor, which shows the words the flags parse to, or why they do not. Enter on the
/// Custom segment only chooses: a Custom choice has no flags yet, so launching from it could only fail. Specified, on
/// Codex: Enter on the focused Custom segment chooses Custom and says nothing beside Launch; in the flags field,
/// `--a 'b c'` shows its two words, apart, and an unclosed quote shows the error instead.
#[gpui::test]
fn custom_opens_its_flags_editor_and_enter_there_only_chooses(cx: &mut TestAppContext) {
    // No host, so a press would be refused and say so: the check that Enter on Custom pressed nothing means something.
    let (_helm, cx) = launcher_on(cx, LaunchDraft::new(None, "~".into(), None));
    test_support::click(cx, "launcher-harness-Codex");
    test_support::click(cx, "launcher-permissions-YOLO");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("launcher-permissions-YOLO"),
        "the premise: the click focused YOLO"
    );
    keys(cx, "tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("launcher-permissions-Custom")
    );
    keys(cx, "enter");
    assert_eq!(draft(cx).agent.permissions, Some(LaunchPermission::Custom));
    assert_eq!(
        shown_refusal(cx),
        None,
        "Enter on Custom did not press Launch"
    );
    test_support::click(cx, "launcher-flags-row");
    assert!(
        read(cx, |launcher, window, cx| launcher
            .flags
            .focus_handle(cx)
            .is_focused(window)),
        "the premise: the click focused the flags field"
    );
    keys(cx, "- - a space ' b space c '");
    assert_eq!(draft(cx).agent.permission_flags, "--a 'b c'");
    assert_eq!(
        name_of(cx, "launcher-flag-words").as_deref(),
        Some("Permission arguments: --a, b c")
    );
    keys(cx, "space '");
    assert!(has_node(cx, "launcher-flags-error") && !has_node(cx, "launcher-flag-words"));
}

/// Switching to an agent type that cannot keep a deliberate choice says what it cleared, under the agent choices, as
/// the old launcher's choice error did. Specified: High effort chosen on Codex, then Cursor (which offers no effort):
/// the effort is cleared and a notice says so; Cursor's note about tracking and Resume shows too.
#[gpui::test]
fn a_switch_that_clears_a_choice_says_so(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    answer_catalog(
        &mut helm,
        cx,
        Some(codex_catalog(&["low", "medium", "high"])),
    );
    test_support::click(cx, "launcher-harness-Codex");
    test_support::click(cx, "launcher-effort-High");
    assert!(
        !has_node(cx, "launcher-choice-notice"),
        "the premise: nothing to say yet"
    );
    test_support::click(cx, "launcher-harness-Cursor");
    assert_eq!(draft(cx).agent.effort, None);
    assert!(
        has_node(cx, "launcher-choice-notice"),
        "the cleared effort is said"
    );
    assert!(!drawn(cx, "launcher-effort-row"), "Cursor offers no effort");
    assert!(painted(
        cx,
        "Cursor session tracking and Resume are not supported."
    ));
}

/// A failed catalog read says so with the read's own words beside Retry, and Retry reads it again, the line going away
/// while the new read is out (the old launcher's "Model catalog unavailable"). Why this matters: without it a failed
/// read leaves the model choices silently empty, and the only way to try again was to reopen the launcher and lose the
/// draft.
#[gpui::test]
fn a_failed_catalog_read_says_so_and_retry_reads_it_again(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    answer_catalog(&mut helm, cx, None);
    let said = name_of(cx, "launcher-catalog-error").unwrap_or_default();
    assert!(said.starts_with("Model catalog unavailable: "), "{said}");
    test_support::click(cx, "launcher-catalog-retry");
    assert!(
        !has_node(cx, "launcher-catalog-error"),
        "the line goes while the read is out"
    );
    answer_catalog(&mut helm, cx, Some(codex_catalog(&["high"])));
    assert!(
        !has_node(cx, "launcher-catalog-error"),
        "a good read says nothing"
    );
}

/// Each agent type offers exactly its own permission vocabulary and shows exactly one segment chosen, its omitted
/// default with nothing chosen by hand (the old specs `guarded-permissions`, `omp-composer`, `goose-pi-composer` and
/// `grok-composer`, their launcher cases). Why this matters: a mode an agent does not have would be refused at launch,
/// and a missing one could not be chosen. Specified, choosing each chip in turn in one launcher: the segments offered,
/// in order, the one shown chosen, and whether effort is offered at all.
#[gpui::test]
fn each_agent_type_offers_its_own_permission_vocabulary(cx: &mut TestAppContext) {
    let all = [
        "Agent's own setting",
        "Guarded",
        "YOLO",
        "Custom",
        "Approve",
        "Smart approve",
        "Chat",
    ];
    let cases: [(&'static str, &[&str], &str, bool); 9] = [
        (
            "launcher-harness-Codex",
            &["Guarded", "YOLO", "Custom"],
            "Guarded",
            true,
        ),
        (
            "launcher-harness-Claude",
            &["Guarded", "YOLO", "Custom"],
            "Guarded",
            true,
        ),
        (
            "launcher-harness-Muse",
            &["Guarded", "YOLO", "Custom"],
            "Guarded",
            true,
        ),
        (
            "launcher-harness-Cursor",
            &["Agent's own setting", "YOLO", "Custom"],
            "Agent's own setting",
            false,
        ),
        (
            "launcher-harness-Grok",
            &["Guarded", "YOLO", "Custom"],
            "Guarded",
            false,
        ),
        (
            "launcher-harness-Goose",
            &["YOLO", "Approve", "Smart approve", "Chat"],
            "YOLO",
            true,
        ),
        ("launcher-harness-Pi", &["YOLO"], "YOLO", true),
        (
            "launcher-harness-OMP",
            &["Guarded", "YOLO", "Custom", "Approve"],
            "Guarded",
            true,
        ),
        ("launcher-harness-OpenCode", &["YOLO"], "YOLO", false),
    ];
    let (_helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    for (chip, offered, shown, effort) in cases {
        test_support::click(cx, chip);
        assert!(
            !draft(cx).agent.permissions_explicit,
            "{chip}: the premise: no permission chosen by hand, so the chosen segment is the draft's default"
        );
        let nodes = test_support::frame(cx);
        let segment = |label: &str| {
            let id = format!("launcher-permissions-{label}");
            nodes
                .iter()
                .find(|node| node.element_id.as_deref() == Some(id.as_str()))
                .cloned()
        };
        let drawn_now: Vec<&str> = all
            .iter()
            .copied()
            .filter(|label| segment(label).is_some())
            .collect();
        assert_eq!(drawn_now, offered, "{chip}: the permissions offered");
        let chosen: Vec<&str> = drawn_now
            .iter()
            .copied()
            .filter(|label| {
                segment(label).and_then(|node| node.toggled) == Some(gpui::Toggled::True)
            })
            .collect();
        assert_eq!(chosen, [shown], "{chip}: exactly {shown} shows as chosen");
        assert_eq!(
            drawn(cx, "launcher-effort-row"),
            effort,
            "{chip}: effort offered"
        );
        // Grok alone has no model choice, so its composer hides the model field (the old grok-composer spec).
        let model = chip != "launcher-harness-Grok";
        assert_eq!(
            drawn(cx, "launcher-model-row"),
            model,
            "{chip}: model field drawn"
        );
    }
}

/// Effort stays segments up to five options, Default included, and becomes a select past that, as the old launcher's
/// `efforts.len() + 1 > 5`. Specified at the boundary: four offered efforts are five segments.
#[gpui::test]
fn five_effort_options_are_still_segments(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    answer_catalog(
        &mut helm,
        cx,
        Some(codex_catalog(&["low", "medium", "high", "xhigh"])),
    );
    test_support::click(cx, "launcher-harness-Codex");
    for effort in ["Default", "Low", "Medium", "High", "xhigh"] {
        let id = format!("launcher-effort-{effort}");
        assert!(has_node(cx, &id), "effort {effort} is a segment");
    }
}

/// An effort the launch already carries stays offered, in its place, when the model does not offer it (old
/// `displayed_efforts`), so a Clone of an older launch shows what it will send and can keep it. Specified: a Clone of
/// a Codex launch with Max effort, against a catalog whose Codex model offers only low, medium and high, offers Max
/// after High, chosen.
#[gpui::test]
fn a_carried_effort_the_model_does_not_offer_stays_offered(cx: &mut TestAppContext) {
    let source = super::draft_tests::session(codex(None, Some(LaunchEffort::Max), None));
    let (mut helm, cx) = launcher_on(cx, LaunchDraft::clone_of(&source, Some(1)));
    answer_catalog(
        &mut helm,
        cx,
        Some(codex_catalog(&["low", "medium", "high"])),
    );
    assert_eq!(
        toggled(cx, "launcher-effort-Max"),
        Some(gpui::Toggled::True)
    );
    let order: Vec<String> = test_support::frame(cx)
        .into_iter()
        .filter_map(|node| node.element_id.map(|id| id.to_string()))
        .filter(|id| id.starts_with("launcher-effort-") && !id.ends_with("-row"))
        .collect();
    assert_eq!(
        order,
        [
            "launcher-effort-Default",
            "launcher-effort-Low",
            "launcher-effort-Medium",
            "launcher-effort-High",
            "launcher-effort-Max"
        ]
    );
}

/// Restart with's stored effort stays offered beside the ones the model offers, even after the user picks another, so
/// going back to the session's own effort is one click (old `displayed_efforts`'s baseline). Specified: Restart with a
/// Codex session stored with Max, against a catalog whose model offers low, medium and high: after choosing Default,
/// Max is still a segment.
#[gpui::test]
fn restart_with_keeps_its_stored_effort_offered(cx: &mut TestAppContext) {
    let mut source = super::draft_tests::session(codex(None, Some(LaunchEffort::Max), None));
    source.restart_offer = RestartOffer::Resume;
    let restart = LaunchDraft::restart_with("missing", &source, Some(1)).unwrap();
    let (mut helm, cx) = launcher_on(cx, restart);
    answer_catalog(
        &mut helm,
        cx,
        Some(codex_catalog(&["low", "medium", "high"])),
    );
    test_support::click(cx, "launcher-effort-Default");
    assert_eq!(
        draft(cx).agent.effort,
        None,
        "the premise: Default is chosen"
    );
    assert!(
        has_node(cx, "launcher-effort-Max"),
        "the stored Max is still offered"
    );
}

// ---- The model field -----------------------------------------------------------------------------------------------

/// A catalog with two Codex models and one Claude model.
fn model_catalog() -> serde_json::Value {
    serde_json::json!([
        { "id": "gpt-5", "harness": "codex", "efforts": ["high"] },
        { "id": "gpt-5-mini", "harness": "codex", "efforts": ["high"] },
        { "id": "sonnet", "harness": "claude", "efforts": ["high"] }
    ])
}

/// The launcher on New on `host` with Codex chosen, the model catalog answered and the model field focused by a click,
/// its list open.
fn model_field_open(
    cx: &mut TestAppContext,
    host: Option<i64>,
) -> (FakeHelm, &mut VisualTestContext) {
    let (mut helm, cx) = launcher_on(cx, LaunchDraft::new(host, "~".into(), None));
    answer_catalog(&mut helm, cx, Some(model_catalog()));
    test_support::click(cx, "launcher-harness-Codex");
    test_support::click(cx, "launcher-model-row");
    assert!(
        read(cx, |launcher, window, cx| launcher
            .model_input
            .focus_handle(cx)
            .is_focused(window)),
        "the premise: the click focused the model field"
    );
    assert!(
        has_node(cx, "launcher-model-list"),
        "the premise: focusing the field opened its list"
    );
    (helm, cx)
}

/// The text the model field shows.
fn model_text(cx: &mut VisualTestContext) -> String {
    read(cx, |launcher, _, cx| {
        launcher.model_input.text(cx).to_string()
    })
}

/// Focusing the model field empties it and opens its list under it: the default, the chosen agent type's catalog
/// models and the show-all row, filtered by what is typed; Down moves the highlight and Enter picks it, closing the
/// list with the model chosen and shown (the old launcher's model combobox). Why this matters: the model is how the
/// user picks what the agent runs on, and Enter must apply only the row the user moved to.
#[gpui::test]
fn the_model_list_filters_and_enter_picks_the_highlighted_row(cx: &mut TestAppContext) {
    let (_helm, cx) = model_field_open(cx, Some(1));
    assert_eq!(model_text(cx), "", "the field empties for typing");
    for id in [
        "launcher-model-default",
        "launcher-model-option-Codex-gpt-5",
        "launcher-model-show-all",
    ] {
        assert!(has_node(cx, id), "{id} is listed");
    }
    assert!(
        !has_node(cx, "launcher-model-option-Claude-sonnet"),
        "another agent's models wait for show-all"
    );
    keys(cx, "m i n i");
    assert!(
        !has_node(cx, "launcher-model-option-Codex-gpt-5"),
        "typing filters the rows"
    );
    keys(cx, "down down enter");
    assert_eq!(draft(cx).agent.model.as_deref(), Some("gpt-5-mini"));
    assert!(
        !has_node(cx, "launcher-model-list"),
        "picking closes the list"
    );
    assert_eq!(model_text(cx), "gpt-5-mini");
}

/// Enter on typed text with no row moved to applies what the text means: a custom id under the chosen agent type is
/// kept exactly; another agent type's model is said, not applied, since typing never switches the agent (SPEC.md,
/// "Creation"). Enter in New only chooses: it presses nothing, which the launcher, having no host, would otherwise
/// answer with its refusal.
#[gpui::test]
fn enter_on_typed_text_applies_a_custom_id_or_says_who_owns_it(cx: &mut TestAppContext) {
    let (_helm, cx) = model_field_open(cx, None);
    keys(cx, "s o n n e t enter");
    assert_eq!(
        draft(cx).agent.harness,
        Some(LaunchHarness::Codex),
        "typing did not switch the agent"
    );
    assert_eq!(
        draft(cx).agent.model,
        None,
        "nor apply the other agent's model"
    );
    assert_eq!(
        name_of(cx, "launcher-model-error").as_deref(),
        Some("sonnet is offered by Claude; choose Claude to use it")
    );
    keys(
        cx,
        "backspace backspace backspace backspace backspace backspace m y - m o d e l enter",
    );
    assert_eq!(
        draft(cx).agent.model.as_deref(),
        Some("my-model"),
        "a custom id is kept as typed"
    );
    assert!(!has_node(cx, "launcher-model-error"));
    assert_eq!(shown_refusal(cx), None, "Enter in New presses nothing");
}

/// Escape closes the list, not the launcher, and the field shows the chosen model again in place of what was typed;
/// the launcher's own Escape comes after (the layer manager closes the topmost layer first). Specified with a model
/// already chosen, so the restored text is the choice, not the field's starting text.
#[gpui::test]
fn escape_closes_the_model_list_first(cx: &mut TestAppContext) {
    let (_helm, cx) = model_field_open(cx, Some(1));
    keys(cx, "down down enter");
    assert_eq!(
        draft(cx).agent.model.as_deref(),
        Some("gpt-5"),
        "the premise: gpt-5 is chosen"
    );
    // The field kept focus after the pick, so typing, not a click, opens the list again, as in the old combobox.
    keys(cx, "x y z");
    assert!(
        has_node(cx, "launcher-model-list"),
        "the premise: typing opened the list again"
    );
    keys(cx, "escape");
    assert!(!has_node(cx, "launcher-model-list"), "the list closed");
    assert!(is_open(cx), "the launcher did not");
    assert_eq!(
        model_text(cx),
        "gpt-5",
        "the field shows the chosen model again"
    );
}

/// Tab out of the field closes its list and puts the chosen model back in the field (the old combobox closed on blur,
/// and a list left open would float over the next control).
#[gpui::test]
fn leaving_the_model_field_closes_its_list(cx: &mut TestAppContext) {
    let (_helm, cx) = model_field_open(cx, Some(1));
    keys(cx, "g p t");
    keys(cx, "tab");
    assert!(
        !read(cx, |launcher, window, cx| launcher
            .model_input
            .focus_handle(cx)
            .is_focused(window)),
        "the premise: Tab left the field"
    );
    assert!(!has_node(cx, "launcher-model-list"), "the list closed");
    assert_eq!(model_text(cx), "Harness default");
}

/// In Restart with, which holds the agent type, the list offers no other agents' models, and Enter in the field
/// chooses and then presses Restart, as the old launcher did there. Specified: no show-all row; a typed custom id and
/// Enter choose it and show Restart's reason beside it (the source session is not in the store, so the press is refused).
#[gpui::test]
fn restart_with_model_enter_chooses_then_presses_restart(cx: &mut TestAppContext) {
    let mut source = super::draft_tests::session(codex(None, None, None));
    source.restart_offer = RestartOffer::Resume;
    let restart = LaunchDraft::restart_with("missing", &source, Some(1)).unwrap();
    let (mut helm, cx) = launcher_on(cx, restart);
    answer_catalog(&mut helm, cx, Some(model_catalog()));
    test_support::click(cx, "launcher-model-row");
    assert!(
        has_node(cx, "launcher-model-list"),
        "the premise: the list is open"
    );
    assert!(
        !has_node(cx, "launcher-model-show-all"),
        "no other agents' models in Restart with"
    );
    keys(cx, "m y - m o d e l enter");
    assert_eq!(draft(cx).agent.model.as_deref(), Some("my-model"));
    let refusal = shown_refusal(cx).unwrap_or_default();
    assert!(
        refusal.starts_with("Restart error. "),
        "Enter pressed Restart: {refusal}"
    );
    // A model that is not applied refuses the restart, so it cannot quietly keep the old model. The field shows the
    // picked model now, and typing would add to it, so it is focused afresh first, which empties it.
    keys(cx, "tab");
    test_support::click(cx, "launcher-model-row");
    assert_eq!(
        model_text(cx),
        "",
        "the premise: the field is empty for typing"
    );
    keys(cx, "s o n n e t enter");
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Restart error. Choose a model before restarting")
    );
}

/// Show-all lists every agent type's models, each with its agent's name; clicking another agent's model switches to
/// that agent on purpose (SPEC.md: "choosing another harness's model from the full model list switches the harness on
/// purpose").
#[gpui::test]
fn show_all_lists_every_agents_models_and_picking_one_switches_the_agent(cx: &mut TestAppContext) {
    let (_helm, cx) = model_field_open(cx, Some(1));
    test_support::click(cx, "launcher-model-show-all");
    assert!(
        has_node(cx, "launcher-model-option-Claude-sonnet"),
        "the other agent's model is listed"
    );
    assert_eq!(
        name_of(cx, "launcher-model-option-Claude-sonnet").as_deref(),
        Some("sonnet (Claude)")
    );
    test_support::click(cx, "launcher-model-option-Claude-sonnet");
    assert_eq!(draft(cx).agent.harness, Some(LaunchHarness::Claude));
    assert_eq!(draft(cx).agent.model.as_deref(), Some("sonnet"));
}

/// With no agent type chosen, a typed id no single agent type owns needs one first, and says so.
#[gpui::test]
fn a_custom_id_with_no_agent_says_to_choose_one(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    answer_catalog(&mut helm, cx, Some(model_catalog()));
    test_support::click(cx, "launcher-model-row");
    assert!(
        read(cx, |launcher, window, cx| launcher
            .model_input
            .focus_handle(cx)
            .is_focused(window)),
        "the premise: the click focused the model field"
    );
    keys(cx, "x y z enter");
    assert_eq!(
        name_of(cx, "launcher-model-error").as_deref(),
        Some("Choose a harness before a custom model id")
    );
    assert_eq!(draft(cx).agent.model, None);
}

/// The error under the model field lasts until it is dealt with: a completed pick or Escape clears it, but leaving the
/// field keeps it, so the user can go and choose the agent it names (the old combobox's rule). Specified: a typed
/// Claude model under Codex shows the error; Tab away keeps it; back in the field, Escape clears it; shown again, a
/// pick of the default row clears it.
#[gpui::test]
fn the_model_error_lasts_until_a_pick_or_escape(cx: &mut TestAppContext) {
    let (_helm, cx) = model_field_open(cx, Some(1));
    keys(cx, "s o n n e t enter");
    assert!(
        has_node(cx, "launcher-model-error"),
        "the premise: the error shows"
    );
    keys(cx, "tab");
    assert!(
        has_node(cx, "launcher-model-error"),
        "leaving the field keeps it"
    );
    test_support::click(cx, "launcher-model-row");
    assert!(
        !has_node(cx, "launcher-model-error"),
        "focusing the field again starts over"
    );
    keys(cx, "s o n n e t enter");
    assert!(
        has_node(cx, "launcher-model-error"),
        "the premise: the error shows again"
    );
    keys(cx, "escape");
    assert!(!has_node(cx, "launcher-model-error"), "Escape clears it");
    // The field shows the chosen model again, and typing would add to it, so it is focused afresh, which empties it.
    keys(cx, "tab");
    test_support::click(cx, "launcher-model-row");
    assert_eq!(
        model_text(cx),
        "",
        "the premise: the field is empty for typing"
    );
    keys(cx, "s o n n e t enter");
    assert!(
        has_node(cx, "launcher-model-error"),
        "the premise: the error shows once more"
    );
    keys(cx, "down enter");
    assert_eq!(draft(cx).agent.model, None, "the default row was picked");
    assert!(
        !has_node(cx, "launcher-model-error"),
        "a completed pick clears it"
    );
}

/// The model list stops at 260 pixels and scrolls, as the old one did; a full catalog lists dozens of models, and a
/// list taller than the window would hide its lower rows and float over the field being typed in.
#[gpui::test]
fn the_model_list_stops_at_260_pixels(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    let rows: Vec<serde_json::Value> = (0..30)
        .map(|index| serde_json::json!({ "id": format!("model-{index}"), "harness": "codex", "efforts": [] }))
        .collect();
    answer_catalog(&mut helm, cx, Some(serde_json::Value::Array(rows)));
    test_support::click(cx, "launcher-harness-Codex");
    test_support::click(cx, "launcher-model-row");
    let list = test_support::bounds(cx, "launcher-model-list");
    assert!(
        list.size.height <= gpui::px(262.),
        "the list is capped: {list:?}"
    );
}
