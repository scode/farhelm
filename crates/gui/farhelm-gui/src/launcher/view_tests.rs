//! Tests of the launcher's dialog in the real main view over the helm client's fake: what it opens on, how input
//! reaches the draft, the tabs, and what a press of the main action shows.
//!
//! The launcher is opened through [`super::view::open`] from inside the window, as an entry point will open it; the
//! tests reach the open launcher through [`layers::last_opened`] (test builds only). Every step ends in one
//! of the shared harness's step helpers, so the focus and layer invariants are checked after each.

use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::{Collection, CollectionState, HostView, KeyedEntry};
use farhelm_proto::{
    LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection, RestartOffer, SessionLaunch,
};
use gpui::{Background, Bounds, Modifiers, TestAppContext, VisualTestContext, point, px};

use super::draft::{LaunchDraft, Tab};
use super::view::{Launcher, open};
use crate::components::{Role, TextInputState};
use crate::focus::Mark;
use crate::layers;
use crate::main_view::MainView;
use crate::session_list::tests::{listing, preferences, session};
use crate::test_support::{self, focused_id, keys, meta, settle, started};
use crate::theme::{self, Preference};

/// The helm's pinned host-list fixture's first host (id 1, "host1", connected), as the hosts collection.
fn hosts() -> CollectionState {
    fixture_hosts(&[0])
}

/// The helm's pinned host-list fixture's hosts at `positions`, in that order, as the hosts collection. The fixture's
/// first three are host1 (connected), host2 (connecting) and host3 (unreachable); the eleventh, host11, is owned by a
/// disposable environment.
pub(crate) fn fixture_hosts(positions: &[usize]) -> CollectionState {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../farhelm-helm/http-contract/host-list.json"
    ))
    .unwrap();
    let hosts: Vec<HostView> = positions
        .iter()
        .map(|&index| serde_json::from_value(fixture["hosts"][index].clone()).unwrap())
        .collect();
    let keys: Vec<String> = hosts.iter().map(|host| host.id.to_string()).collect();
    let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
    CollectionState {
        collection: Collection::Hosts,
        meta: meta(&keys),
        entries: hosts
            .into_iter()
            .map(|host| KeyedEntry {
                key: host.id.to_string(),
                value: serde_json::to_value(host).unwrap(),
            })
            .collect(),
    }
}

/// The real main view over the fake helm with one host, session `a` and the preferences, and the launcher opened on
/// `draft`, checked once settled.
pub(super) fn launcher_on(
    cx: &mut TestAppContext,
    draft: LaunchDraft,
) -> (FakeHelm, &mut VisualTestContext) {
    launcher_with_hosts(cx, draft, hosts())
}

/// [`launcher_on`] with the hosts collection `hosts`.
pub(super) fn launcher_with_hosts(
    cx: &mut TestAppContext,
    draft: LaunchDraft,
    hosts: CollectionState,
) -> (FakeHelm, &mut VisualTestContext) {
    launcher_over(
        cx,
        draft,
        vec![
            listing(&["a"], &[session("a")]),
            hosts,
            preferences(&[], None),
        ],
    )
}

/// The real main view over the fake helm's first snapshot `state`, with the launcher opened on `draft`, checked once
/// settled: the window every launcher test opens, over whichever fleet it needs.
pub(super) fn launcher_over(
    cx: &mut TestAppContext,
    draft: LaunchDraft,
    state: Vec<CollectionState>,
) -> (FakeHelm, &mut VisualTestContext) {
    let (mut helm, _connection) = started(cx);
    helm.connect(state);
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
pub(crate) fn read<T>(
    cx: &mut VisualTestContext,
    read: impl FnOnce(&Launcher, &gpui::Window, &gpui::App) -> T,
) -> T {
    cx.update(|window, cx| {
        let launcher = layers::last_opened::<Launcher>(cx).expect("a launcher is open");
        read(launcher.read(cx), window, cx)
    })
}

/// Whether the open launcher's search box holds focus, for tests outside this module, which cannot reach the box.
pub(crate) fn search_focused(cx: &mut VisualTestContext) -> bool {
    read(cx, |launcher, window, cx| {
        launcher.search.focus_handle(cx).is_focused(window)
    })
}

/// What the open launcher's search box holds, for tests outside this module.
pub(crate) fn search_text(cx: &mut VisualTestContext) -> String {
    read(cx, |launcher, _, cx| launcher.search.text(cx).to_string())
}

/// The draft of the launcher the window shows now, found as the dispatcher finds it (`entry::open_launcher`) rather
/// than through the test global [`draft`] reads, which a launcher a quick launch prepared out of sight never sets.
pub(crate) fn shown_draft(cx: &mut VisualTestContext) -> LaunchDraft {
    cx.update(|window, cx| {
        super::entry::open_launcher(window, cx)
            .expect("a launcher is shown")
            .read(cx)
            .draft
            .clone()
    })
}

/// Whether the shown launcher's Name field holds focus, and whether it has text selected, found as [`shown_draft`]
/// finds the launcher, so it works for a launcher a quick launch revealed.
pub(crate) fn shown_name_focus_and_selection(cx: &mut VisualTestContext) -> (bool, bool) {
    cx.update(|window, cx| {
        let launcher = super::entry::open_launcher(window, cx).expect("a launcher is shown");
        let name = &launcher.read(cx).name;
        (
            name.focus_handle(cx).is_focused(window),
            !name.selection_empty(cx),
        )
    })
}

/// The open launcher's draft.
pub(crate) fn draft(cx: &mut VisualTestContext) -> LaunchDraft {
    read(cx, |launcher, _, _| launcher.draft.clone())
}

/// Whether the launcher is still open: its view lives as long as its layer does, until a press launches it; a launcher
/// whose create is out lives on hidden (`launch_tests` checks the layer instead).
fn is_open(cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| layers::last_opened::<Launcher>(cx).is_some())
}

/// The text fields a test clicks into.
#[derive(Clone, Copy)]
pub(crate) enum Field {
    Folder,
    Wrapper,
    Prompt,
    /// The Custom permission flags.
    Flags,
    /// The model field.
    Model,
}

/// The platform's Select All chord, for a test that replaces a field's whole text.
pub(super) const SELECT_ALL: &str = if cfg!(target_os = "macos") {
    "cmd-a"
} else {
    "ctrl-a"
};

/// Whether `field` holds focus.
pub(crate) fn focused(cx: &mut VisualTestContext, field: Field) -> bool {
    read(cx, |launcher, window, cx| {
        let state = match field {
            Field::Folder => &launcher.folder,
            Field::Wrapper => &launcher.wrapper,
            Field::Prompt => &launcher.prompt,
            Field::Flags => &launcher.flags,
            Field::Model => &launcher.model_input,
        };
        state.focus_handle(cx).is_focused(window)
    })
}

/// Whether the folder field holds focus.
fn folder_focused(cx: &mut VisualTestContext) -> bool {
    focused(cx, Field::Folder)
}

/// Whether an element was drawn under `id` in a fresh frame.
pub(super) fn has_node(cx: &mut VisualTestContext, id: &str) -> bool {
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
pub(super) fn name_of(cx: &mut VisualTestContext, id: &str) -> Option<String> {
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
pub(super) fn shown_refusal(cx: &mut VisualTestContext) -> Option<String> {
    name_of(cx, "launcher-refusal")
}

/// A Codex launch carrying `model`, `effort` and `permissions`, with a stored resume command.
pub(super) fn codex(
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

/// SPEC.md, "Creation": New opens a dialog on the Agent tab with no agent type, the search box first, then the
/// destination rows, and focus in the search box ("Ordinary launches still start focus on search"). The header names
/// the mode.
#[gpui::test]
fn new_opens_on_the_agent_tab_with_focus_in_the_search_box(cx: &mut TestAppContext) {
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
    assert!(
        read(cx, |launcher, window, cx| launcher
            .search
            .focus_handle(cx)
            .is_focused(window)),
        "focus starts in the search box"
    );
    let search = test_support::bounds(cx, "launcher-search-row");
    let folder = test_support::bounds(cx, "launcher-folder-row");
    assert!(
        search.top() < folder.top(),
        "the search box is above the destination"
    );
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

/// An Agent draft ready to launch paints Launch as primary and Reset choices as neutral in both appearances. The
/// border distinguishes the neutral look from the unavailable look, which uses the same fill.
#[gpui::test]
fn the_footer_keeps_launch_primary_and_reset_neutral(cx: &mut TestAppContext) {
    let mut draft = LaunchDraft::new(Some(1), "~".into(), None);
    draft.choose_harness(LaunchHarness::Codex, None, None);
    let (_helm, cx) = launcher_on(cx, draft);
    assert_eq!(
        launch_help(cx).as_deref(),
        Some("Launch: start the session"),
        "the premise: Launch is available"
    );
    test_support::click(cx, "launcher-folder-row");
    assert!(
        folder_focused(cx),
        "the premise: neither footer button owns focus"
    );
    cx.simulate_mouse_move(point(px(0.), px(0.)), None, Modifiers::none());
    for appearance in [Preference::Light, Preference::Dark] {
        cx.update(|_, cx| theme::set_preference(appearance, cx));
        settle(cx);
        let (palette, pointer, quads) = cx.update(|window, cx| {
            (
                theme::palette(cx),
                window.mouse_position(),
                window.painted_quads(),
            )
        });
        assert_eq!(palette.dark, appearance == Preference::Dark);
        for (id, fill, border) in [
            ("launcher-launch", palette.accent_fill, palette.accent_edge),
            ("launcher-reset", palette.bg_2, palette.border),
        ] {
            let bounds = test_support::bounds(cx, id);
            assert!(!bounds.contains(&pointer), "the premise: {id} is unhovered");
            // The scene snaps the paint to device pixels. Match the button's own quad, since its parent may
            // already paint the same color behind the button.
            let painted_bounds = cx.update(|window, _| {
                Bounds::from_corners(
                    window.pixel_snap_point(bounds.origin),
                    window.pixel_snap_point(bounds.bottom_right()),
                )
                .scale(window.scale_factor())
            });
            assert!(
                quads.iter().any(|quad| {
                    quad.bounds == painted_bounds
                        && quad.content_mask.bounds.contains(&painted_bounds.center())
                        && quad.background == Background::from(theme::color(fill))
                }),
                "{appearance:?}: {id} paints its tier's resting fill"
            );
            // GPUI paints the border separately and clips its quads to strips. Test the top edge away
            // from the rounded corners, where the mask of the top strip includes the border.
            let top_edge = painted_bounds.top_center();
            assert!(
                quads.iter().any(|quad| {
                    quad.bounds == painted_bounds
                        && quad.content_mask.bounds.contains(&top_edge)
                        && quad.border_color == theme::color(border)
                }),
                "{appearance:?}: {id} paints its tier's resting border"
            );
        }
    }
}

/// At the minimum window size the body can scroll while the pinned footer and header stay in view. Tab and
/// Shift+Tab wrap between Launch and Close, and the footer's other actions keep their reading order.
#[gpui::test]
fn the_footer_actions_follow_reading_order_and_wrap_to_close(cx: &mut TestAppContext) {
    let mut launch = LaunchDraft::new(Some(1), "~".into(), None);
    let _ = launch.choose_harness(LaunchHarness::Codex, None, None);
    let (_helm, cx) = launcher_on(cx, launch);
    cx.simulate_resize(crate::bootstrap::MIN_WINDOW_SIZE);
    settle(cx);
    assert_eq!(
        name_of(cx, "launcher-launch").as_deref(),
        Some("Launch Codex · host1 · ~"),
    );
    assert!(
        search_focused(cx),
        "search holds the keyboard with no results open"
    );
    let at_endpoint = |cx: &mut VisualTestContext, id| {
        let node = test_support::frame(cx)
            .into_iter()
            .find(|node| node.element_id.as_deref() == Some(id))
            .expect("the endpoint is drawn");
        assert!(node.flags.focused, "{id} has focus");
        assert_eq!(node.visible_bounds, node.bounds, "{id} is visible");
    };
    let start = test_support::bounds(cx, "launcher-search-box").top();
    keys(cx, "shift-tab shift-tab");
    at_endpoint(cx, "launcher-launch");
    super::browse_tests::scroll_body(cx, -4000.);
    assert!(
        test_support::bounds(cx, "launcher-search-box").top() < start,
        "the body scrolls independently of the footer"
    );
    at_endpoint(cx, "launcher-launch");
    keys(cx, "tab");
    at_endpoint(cx, "launcher-close");
    keys(cx, "tab");
    assert!(search_focused(cx));
    super::browse_tests::scroll_body(cx, 4000.);
    assert_eq!(test_support::bounds(cx, "launcher-search-box").top(), start);
    keys(cx, "shift-tab");
    at_endpoint(cx, "launcher-close");
    keys(cx, "shift-tab");
    at_endpoint(cx, "launcher-launch");
    for id in [
        "launcher-save-template",
        "launcher-reset",
        "launcher-cancel",
    ] {
        keys(cx, "shift-tab");
        assert_eq!(focused_id(cx).as_deref(), Some(id));
    }
}

/// With every placement offered, Tab follows the visible launch groups: search and recents, destination and
/// placement, host and path, then the active launch tab and its agent choices.
#[gpui::test]
fn tab_follows_the_launch_groups_when_all_placements_are_offered(cx: &mut TestAppContext) {
    use super::recents_view_tests::{chosen, history, launched};

    let mut start = LaunchDraft::new(Some(1), "/tmp".into(), None);
    let _ = start.choose_harness(LaunchHarness::Codex, None, None);
    let (mut helm, cx) = super::environment_launch_tests::window_with(
        cx,
        start,
        fixture_hosts(&[0]),
        super::environment_launch_tests::plain_preferences(),
    );
    let recent = history(vec![launched(
        "/tmp",
        &chosen(LaunchHarness::Codex, None, None, None),
    )]);
    let account = super::environment_tests::account(
        true,
        farhelm_proto::tensorlake::SshKeyStatus::NotCreated,
    );
    for command in helm.commands() {
        let body = if command.path == "/api/tensorlake" {
            Some(serde_json::to_value(&account).unwrap())
        } else if command.path.starts_with("/api/launch-history?") {
            Some(recent.clone())
        } else {
            None
        };
        helm.reply(command.id, 200, body);
    }
    settle(cx);
    assert!(search_focused(cx));
    for id in [
        "launcher-recent-0",
        "launcher-destination-Folder",
        "launcher-destination-Managed checkout",
        "launcher-runs-on-Existing host",
        "launcher-runs-on-New container",
        "launcher-runs-on-New sandbox",
        "launcher-host",
        "launcher-browse",
        "launcher-folder",
        "launcher-name",
        "launcher-tabs-Agent",
        "launcher-harness-Codex",
    ] {
        keys(cx, "tab");
        let nodes = test_support::frame(cx);
        assert!(
            crate::session_list::host_rail::tests::within(&nodes, id)
                .iter()
                .any(|node| node.flags.focused),
            "Tab reached {id}"
        );
    }
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
/// incomplete attempt says why beside Launch"); before the press the reason is Launch's hover help only, and an edit
/// takes the line away until the next press (launcher-enter.spec's "launcher ${attempt} explains an incomplete setup
/// beside Launch"). Why: Launch stays pressable while refused, so that a press can say why (`Button::refused`), and its
/// hover help is how a user learns the reason without trying, as the old `aria-disabled` button's tooltip was. The
/// refused look follows the same refusal and is not read here. Specified: New with no agent type gives "Choose a
/// structured harness before launching" as Launch's help and says nothing beside it; a press says it after "Not
/// launched."; an edit of the folder takes it away and Enter there says it again; once Codex completes the draft,
/// Launch's help is its own, "Launch: start the session".
#[gpui::test]
fn a_refused_press_says_why_beside_the_main_action(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    assert_eq!(
        launch_help(cx).as_deref(),
        Some("Choose a structured harness before launching")
    );
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
    test_support::click(cx, "launcher-harness-Codex");
    assert_eq!(
        draft(cx).agent.harness,
        Some(LaunchHarness::Codex),
        "the premise: the click chose Codex"
    );
    assert_eq!(
        launch_help(cx).as_deref(),
        Some("Launch: start the session"),
        "a complete draft gets Launch's own help back"
    );
}

/// Specified: pressing Launch on New with no agent draws the refusal beside Launch with the Alert role. Why: the
/// refusal must be exposed as an alert, not just ordinary text ("launcher refusals use warning colors in ${appearance}
/// appearance", `e2e/tests/composer-word-search.spec.ts` at `24bc0580`, its `role="alert"` assertion).
#[gpui::test]
fn the_refusal_beside_launch_is_an_alert(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    test_support::click(cx, "launcher-launch");
    let refusal = test_support::frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("launcher-refusal"))
        .expect("the premise: the refusal is drawn");
    assert_eq!(refusal.role, Some(Role::Alert));
}

/// The hover help Launch shows once the pointer has rested on it for the hover delay, which for a draft Launch would
/// refuse is the refusal's reason. The pointer stays on Launch afterwards; a later click elsewhere moves it.
pub(crate) fn launch_help(cx: &mut VisualTestContext) -> Option<String> {
    let launch = test_support::center(cx, "launcher-launch");
    cx.simulate_mouse_move(launch, None, Modifiers::none());
    test_support::advance(cx, layers::HOVER_DELAY);
    cx.update(|window, cx| layers::shown_tooltip(window, cx))
        .map(|shown| shown.text.to_string())
}

/// Typing in a field is the draft's edit: the folder typed is the user's, so a host change would keep it.
#[gpui::test]
fn typing_in_the_folder_edits_the_draft(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "".into(), None));
    test_support::click(cx, "launcher-folder-row");
    assert!(
        folder_focused(cx),
        "the premise: the click focused the folder field"
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
pub(super) fn answer_catalog(
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

/// Choosing Custom opens its flags editor and moves focus into it, and the editor shows the words the flags parse to,
/// or why they do not. Enter on the Custom segment only chooses: a Custom choice has no flags yet, so launching from it
/// could only fail. Specified, on Codex: Enter on the focused Custom segment chooses Custom, says nothing beside Launch
/// and puts focus in the flags field; `--a 'b c'` there shows its two words, apart, and an unclosed quote shows the
/// error instead.
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
    assert!(
        read(cx, |launcher, window, cx| launcher
            .flags
            .focus_handle(cx)
            .is_focused(window)),
        "choosing Custom moved focus into its flags field"
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
/// the effort is cleared and a notice says so; Cursor's note about tracking and Resume shows too, and goes on the
/// Command tab, which would not launch Cursor (the old `cursor-composer` spec's last step).
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
    // The note is the Agent tab's: the Command tab keeps the Cursor draft but does not launch it.
    test_support::click(cx, "launcher-tabs-Command");
    assert_eq!(draft(cx).tab, Tab::Command, "the premise: the Command tab");
    assert!(
        drawn(cx, "launcher-command-row"),
        "the premise: its fields show"
    );
    assert_eq!(
        draft(cx).agent.harness,
        Some(LaunchHarness::Cursor),
        "the Agent tab's draft is kept"
    );
    assert!(!painted(
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
/// `displayed_efforts`), so a Clone of an older launch shows what it carries; but it refuses the press, and once the
/// user picks another effort it is no longer offered, since a Clone has no stored launch to go back to, unlike Restart
/// with ([`restart_with_keeps_its_stored_effort_offered`]). Ported from launcher-enter.spec's "launcher effort dropdown
/// preserves an unsupported inherited choice until a native edit". Why: showing Max chosen while quietly sending another
/// effort would launch something other than what the launcher shows. Specified: a Clone of a Codex launch with Max,
/// against a catalog whose Codex model offers only low, medium and high, offers Max after High, chosen; Launch says
/// "Not launched. This saved choice is no longer supported by the current catalog; choose a compatible model or
/// effort" and sends nothing; after a click on High, Max is gone and Launch sends effort high.
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
    test_support::click(cx, "launcher-launch");
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some(
            "Not launched. This saved choice is no longer supported by the current catalog; choose a compatible \
             model or effort"
        )
    );
    assert!(
        super::launch_tests::creates(&mut helm, cx).is_empty(),
        "the refused press sent nothing"
    );
    test_support::click(cx, "launcher-effort-High");
    assert_eq!(draft(cx).agent.effort, Some(LaunchEffort::High));
    assert!(
        !has_node(cx, "launcher-effort-Max"),
        "with no stored launch to keep it for, Max is no longer offered"
    );
    let create = super::launch_tests::press(&mut helm, cx, "/api/sessions");
    let body = create.body.expect("a create carries a body");
    assert_eq!(body["launch"]["effort"], serde_json::json!("high"));
}

/// The old launcher kept a flat 24 pixels above and below itself in windows at least 1000 pixels wide, where other
/// dialogs keep 5% of the window's height (`app.css` at the reference commit, line 6629: "Leave room for populated
/// launch rows rather than spending five percent of a desktop window above and below the dialog"). Specified: a Clone
/// of a Codex launch, which fills the window's height, is 48 pixels shorter than a 1000 by 600 window, and 60 pixels
/// shorter (5% of the height each side) than a 999 by 600 one. That other dialogs keep 5% in a wide window is
/// `settings::tests::the_frame_keeps_one_size_across_sections_capped_by_the_window`'s 1000 by 600 case.
#[gpui::test]
fn the_launcher_keeps_a_flat_margin_in_a_wide_window(cx: &mut TestAppContext) {
    let source = super::draft_tests::session(codex(None, Some(LaunchEffort::High), None));
    let (_helm, cx) = launcher_on(cx, LaunchDraft::clone_of(&source, Some(1)));
    cx.simulate_resize(gpui::size(gpui::px(1000.), gpui::px(600.)));
    settle(cx);
    assert_eq!(
        test_support::bounds(cx, "launcher").size.height,
        gpui::px(600. - 2. * 24.)
    );
    cx.simulate_resize(gpui::size(gpui::px(999.), gpui::px(600.)));
    settle(cx);
    assert_eq!(
        test_support::bounds(cx, "launcher").size.height,
        gpui::px(600. - 2. * 30.)
    );
}

/// Open a Clone of a Codex launch with High effort, fail its catalog read when `fail`, and check that it shows High
/// chosen and that Launch sends its create with the catalog unanswered or failed.
fn clone_launches_without_the_catalog(cx: &mut TestAppContext, fail: bool) {
    let source = super::draft_tests::session(codex(None, Some(LaunchEffort::High), None));
    let (mut helm, cx) = launcher_on(cx, LaunchDraft::clone_of(&source, Some(1)));
    if fail {
        answer_catalog(&mut helm, cx, None);
        assert!(
            has_node(cx, "launcher-catalog-error"),
            "the premise: the read failed"
        );
    } else {
        assert!(
            helm.commands()
                .iter()
                .any(|command| command.path.ends_with("launch-catalog")),
            "the premise: the catalog read went out, and this test leaves it unanswered"
        );
    }
    assert_eq!(
        toggled(cx, "launcher-effort-High"),
        Some(gpui::Toggled::True),
        "the copied effort shows"
    );
    test_support::click(cx, "launcher-launch");
    cx.run_until_parked();
    let creates = helm
        .commands()
        .into_iter()
        .filter(|command| command.path == "/api/sessions")
        .count();
    assert_eq!(creates, 1, "Launch sent the create");
}

/// A Clone's copied choices need no catalog to launch (the old `clone.spec.ts` case "a clone with an effort can launch
/// while the catalog is still loading"): the catalog only offers choices, and the helm validates the launch again, so
/// a slow read must not hold back a copy the user can already see in full. Specified: a Clone of a Codex launch with
/// High effort shows High chosen and sends its create on Launch while the catalog read is still out.
#[gpui::test]
fn a_clone_launches_while_the_catalog_read_is_out(cx: &mut TestAppContext) {
    clone_launches_without_the_catalog(cx, false);
}

/// The failed-read twin of [`a_clone_launches_while_the_catalog_read_is_out`] (the old case "a failed catalog read
/// shows a retry and leaves the clone launchable"; the Retry line itself is
/// [`a_failed_catalog_read_says_so_and_retry_reads_it_again`]). Specified: with the catalog read failed, the Clone still
/// shows High chosen and sends its create on Launch.
#[gpui::test]
fn a_clone_launches_after_a_failed_catalog_read(cx: &mut TestAppContext) {
    clone_launches_without_the_catalog(cx, true);
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
pub(super) fn model_catalog() -> serde_json::Value {
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
pub(super) fn model_text(cx: &mut VisualTestContext) -> String {
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
    let (mut helm, cx) = model_field_open(cx, Some(1));
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
    assert!(
        focused(cx, Field::Model),
        "the closed field still has focus"
    );
    keys(cx, "enter");
    assert_eq!(draft(cx).agent.model.as_deref(), Some("gpt-5-mini"));
    assert_eq!(
        model_text(cx),
        "gpt-5-mini",
        "Enter cannot apply the old filter"
    );
    assert!(super::launch_tests::creates(&mut helm, cx).is_empty());
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

// ---- Stored values shown escaped ----------------------------------------------------------------------------------

/// The contract every field shown escaped keeps (`view::stored`): the field in row `field` shows `shown`; a click and
/// a typed key there change nothing, since editing the escape text would turn it into new launch words; `clear`
/// empties the field, and the field then takes typing. `input` picks the field off the launcher; the click is checked
/// to have focused it, so a key that went elsewhere cannot pass for a refused one.
pub(super) fn escaped_until_cleared(
    cx: &mut VisualTestContext,
    field: &'static str,
    clear: &'static str,
    shown: &str,
    input: fn(&Launcher) -> &TextInputState,
) {
    let field_text = |cx: &mut VisualTestContext| {
        read(cx, |launcher, _, cx| input(launcher).text(cx).to_string())
    };
    assert_eq!(field_text(cx), shown, "{field} shows the escaped spelling");
    test_support::click(cx, field);
    assert!(
        read(cx, |launcher, window, cx| input(launcher)
            .focus_handle(cx)
            .is_focused(window)),
        "the premise: the click focused {field}"
    );
    keys(cx, "x");
    assert_eq!(field_text(cx), shown, "{field} is read-only");
    test_support::click(cx, clear);
    assert_eq!(field_text(cx), "", "{clear} emptied it");
    test_support::click(cx, field);
    keys(cx, "x");
    assert_eq!(field_text(cx), "x", "{field} takes typing again");
}

/// A stored wrapper prefix may have come from another host, so Restart with must show its direction-changing bytes
/// without letting the escape text become new launch words (restart-with.spec's "restart with shows an unsafe stored
/// wrapper prefix without editing its escape text"). Specified: Restart with of a Codex launch whose prefix is
/// `wrapper<U+202E> --` shows it so, read-only, until Clear wrapper prefix empties it, in the field and the draft; an
/// invisible character typed afterwards is shown escaped at once.
#[gpui::test]
fn restart_with_shows_an_unsafe_stored_wrapper_prefix_escaped(cx: &mut TestAppContext) {
    let mut launch = codex(None, None, None);
    if let SessionLaunch::Agent { selection, .. } = &mut launch {
        selection.wrapper_prefix = Some(vec!["wrapper\u{202e}".into(), "--".into()]);
    }
    let mut source = super::draft_tests::session(launch);
    source.restart_offer = RestartOffer::Resume;
    let restart = LaunchDraft::restart_with("missing", &source, Some(1)).unwrap();
    let (_helm, cx) = launcher_on(cx, restart);
    assert_eq!(
        draft(cx).agent.wrapper,
        "wrapper\u{202e} --",
        "the premise: the draft holds the raw words"
    );
    escaped_until_cleared(
        cx,
        "launcher-wrapper-row",
        "launcher-clear-wrapper",
        "wrapper<U+202E> --",
        |launcher| &launcher.wrapper,
    );
    assert_eq!(draft(cx).agent.wrapper, "x");
    // Typed or pasted text is judged the same way, at once.
    cx.simulate_input("\u{202e}");
    settle(cx);
    assert_eq!(
        read(cx, |launcher, _, cx| launcher.wrapper.text(cx).to_string()),
        "x<U+202E>",
        "a typed invisible character is shown escaped"
    );
    assert_eq!(draft(cx).agent.wrapper, "x\u{202e}");
    keys(cx, "y");
    assert_eq!(
        read(cx, |launcher, _, cx| launcher.wrapper.text(cx).to_string()),
        "x<U+202E>",
        "and the field is read-only again"
    );
    assert!(
        cx.debug_bounds("launcher-clear-wrapper").is_some(),
        "with its Clear"
    );
}

/// A remembered prefix loaded by an agent switch is shown the same way, since the old launcher judged the prefix the
/// field holds, whatever put it there (launcher-tabs.spec's "agent switch loads that agent's remembered wrapper
/// prefix": "unsafe stored bytes are visible but cannot be edited as escaped text"). Specified: with Codex's and
/// Claude's prefixes remembered, choosing Codex loads its prefix as written; choosing Claude shows
/// `env CLAUDE_PROFILE=work<U+202E>` read-only until cleared; choosing Codex again loads Codex's.
#[gpui::test]
fn an_agent_switch_shows_an_unsafe_remembered_prefix_escaped(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    let remembered =
        crate::session_list::tests::preferences_state(farhelm_proto::ui_channel::Preferences {
            welcome_seen: Some(true),
            remembered_wrapper_prefixes: [
                (
                    LaunchHarness::Codex,
                    vec![
                        "with-endpoint".into(),
                        "--profile".into(),
                        "'codex profile'".into(),
                        "--".into(),
                    ],
                ),
                (
                    LaunchHarness::Claude,
                    vec!["env".into(), "CLAUDE_PROFILE=work\u{202e}".into()],
                ),
            ]
            .into(),
            ..Default::default()
        });
    helm.push(farhelm_proto::ui_channel::CollectionChanges {
        collection: remembered.collection,
        meta: remembered.meta,
        upsert: remembered.entries,
        remove: Vec::new(),
    });
    settle(cx);
    let wrapper = |cx: &mut VisualTestContext| {
        read(cx, |launcher, _, cx| launcher.wrapper.text(cx).to_string())
    };
    test_support::click(cx, "launcher-harness-Codex");
    assert_eq!(wrapper(cx), "with-endpoint --profile 'codex profile' --");
    test_support::click(cx, "launcher-harness-Claude");
    escaped_until_cleared(
        cx,
        "launcher-wrapper-row",
        "launcher-clear-wrapper",
        "env CLAUDE_PROFILE=work<U+202E>",
        |launcher| &launcher.wrapper,
    );
    test_support::click(cx, "launcher-harness-Codex");
    assert_eq!(wrapper(cx), "with-endpoint --profile 'codex profile' --");
}

/// The Agent tab's other launch text is spelled out as the old launcher showed it (`view::shown_text`): a model a draft
/// carries in the closed model field, Custom flags in an editable escaped spelling, and the initial prompt read-only
/// behind Clear initial prompt. Why: such text comes from a source session or a template, and shown raw it could hide
/// or reverse what it says. Specified: a Codex draft with model `gpt<U+202E>x`, Custom flags `--a<U+200B>` and the
/// prompt `do<U+202E>it` shows those spellings while the draft keeps the raw bytes; the flags field takes typing after
/// the spelling, which the draft then holds as typed; the prompt says why it is read-only and refuses typing until
/// Clear initial prompt empties it.
#[gpui::test]
fn agent_launch_text_holding_invisible_characters_shows_escaped(cx: &mut TestAppContext) {
    let mut start = LaunchDraft::new(Some(1), "~".into(), None);
    let _ = start.choose_harness(LaunchHarness::Codex, None, None);
    start.agent.model = Some("gpt\u{202e}x".into());
    start.agent.permissions = Some(LaunchPermission::Custom);
    start.agent.permission_flags = "--a\u{200b}".into();
    start.prompt = "do\u{202e}it".into();
    let (_helm, cx) = launcher_on(cx, start);
    assert_eq!(model_text(cx), "gpt<U+202E>x");
    assert_eq!(
        draft(cx).agent.model.as_deref(),
        Some("gpt\u{202e}x"),
        "the draft keeps the raw id"
    );

    assert_eq!(
        draft(cx).agent.permission_flags,
        "--a\u{200b}",
        "untouched flags keep their raw bytes"
    );
    assert_eq!(draft(cx).prompt, "do\u{202e}it", "and so does the prompt");
    assert!(
        has_node(
            cx,
            "Invisible formatting is shown escaped. Clear the message to replace it."
        ),
        "the old line says why the prompt is read-only"
    );
    escaped_until_cleared(
        cx,
        "launcher-prompt-row",
        "launcher-clear-prompt",
        "do<U+202E>it",
        |launcher| &launcher.prompt,
    );

    let flags = |cx: &mut VisualTestContext| {
        read(cx, |launcher, _, cx| launcher.flags.text(cx).to_string())
    };
    assert_eq!(flags(cx), "--a<U+200B>");
    assert!(
        !has_node(
            cx,
            "Invisible formatting is shown escaped. Clear the message to replace it."
        ),
        "the line goes with the read-only prompt"
    );
    // After the prompt's Clear, which takes the prompt's help line and button away and so brings the flags row into the
    // dialog's view.
    test_support::click(cx, "launcher-flags-row");
    assert!(
        read(cx, |launcher, window, cx| launcher
            .flags
            .focus_handle(cx)
            .is_focused(window)),
        "the premise: the click focused the flags field"
    );
    keys(cx, "end x");
    assert_eq!(flags(cx), "--a<U+200B>x", "the spelling is editable");
    assert_eq!(
        draft(cx).agent.permission_flags,
        "--a<U+200B>x",
        "and an edit takes the field as it reads"
    );
}

/// Outside Restart with's stored values, a command line and resume command holding invisible characters are shown in
/// an editable escaped spelling, as the old launcher showed the ones Clone and Replace with copy, while an untouched
/// value still holds its raw bytes; text the user types is shown as typed, since it reaches Bash as written and a
/// spelling's `<` and `>` would be redirections there (SPEC.md, "Command launch"). Specified: a Command tab draft
/// opening with command `run<U+200B> {farhelm_args}` and resume command `run<U+202E> {conversation}` shows both spelled
/// out, with no Clear buttons; the draft keeps the raw text; typing at the command's end edits the spelling; a
/// zero-width space typed after that is shown and kept as typed.
#[gpui::test]
fn copied_command_text_holding_invisible_characters_shows_escaped_and_editable(
    cx: &mut TestAppContext,
) {
    let mut start = LaunchDraft::new(Some(1), "~".into(), None);
    start.choose_tab(Tab::Command);
    start.edit_command("run\u{200b} {farhelm_args}".into());
    start.choose_command_agent(Some(LaunchHarness::Codex));
    start.choose_command_resume(true);
    start.edit_command_resume("run\u{202e} {conversation}".into());
    let (_helm, cx) = launcher_on(cx, start);
    let text = |cx: &mut VisualTestContext| {
        read(cx, |launcher, _, cx| {
            (
                launcher.command.text(cx).to_string(),
                launcher.resume.text(cx).to_string(),
            )
        })
    };
    assert_eq!(
        text(cx),
        (
            "run<U+200B> {farhelm_args}".to_owned(),
            "run<U+202E> {conversation}".to_owned()
        )
    );
    assert!(
        !has_node(cx, "launcher-clear-command") && !has_node(cx, "launcher-clear-resume"),
        "editable, so no Clear"
    );
    assert_eq!(draft(cx).command.command, "run\u{200b} {farhelm_args}");
    test_support::click(cx, "launcher-command-row");
    assert!(
        read(cx, |launcher, window, cx| launcher
            .command
            .focus_handle(cx)
            .is_focused(window)),
        "the premise: the click focused the command field"
    );
    keys(cx, "end x");
    assert_eq!(draft(cx).command.command, "run<U+200B> {farhelm_args}x");
    cx.simulate_input("\u{200b}");
    settle(cx);
    assert_eq!(
        draft(cx).command.command,
        "run<U+200B> {farhelm_args}x\u{200b}"
    );
    assert_eq!(
        text(cx).0,
        "run<U+200B> {farhelm_args}x\u{200b}",
        "typed text is shown as typed"
    );
}

/// In Restart with, which holds the agent type, the list offers no other agents' models, and Enter in the field only
/// chooses, open or closed: Restart is a separate press. Why: the old launcher's "choosing a model must not submit the
/// launcher" (restart-with.spec's "restart with model Enter chooses before a separate restart action"), and a restart
/// stops a running agent, so a key that only meant "this model" must not do it. Specified: no show-all row; a typed
/// custom id and Enter choose it and show no Restart error; Enter again in the closed field still presses nothing; an
/// id another agent type owns, typed and entered, applies nothing and presses nothing; the separate press of Restart
/// then refuses, since that typed model was not applied.
#[gpui::test]
fn restart_with_model_enter_chooses_without_restarting(cx: &mut TestAppContext) {
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
    assert_eq!(shown_refusal(cx), None, "Enter chose and pressed nothing");
    assert!(
        !has_node(cx, "launcher-model-list"),
        "the premise: Enter closed the list"
    );
    keys(cx, "enter");
    assert_eq!(
        shown_refusal(cx),
        None,
        "nor does Enter in the closed field"
    );
    // A model that is not applied refuses the restart, so it cannot quietly keep the old model: `sonnet` belongs to
    // another agent type, so Enter applies nothing. The
    // field shows the picked model now, and typing would add to it, so focus leaves it (Tab) and comes back with a
    // click, which empties it.
    keys(cx, "tab");
    test_support::click(cx, "launcher-model-row");
    assert_eq!(
        model_text(cx),
        "",
        "the premise: the field is empty for typing"
    );
    keys(cx, "s o n n e t enter");
    assert_eq!(
        shown_refusal(cx),
        None,
        "the premise: Enter pressed nothing"
    );
    test_support::click(cx, "launcher-launch");
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Restart error. Choose a model before restarting"),
        "the separate press reached Restart"
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

/// A model picked before an agent chooses its owner, so model-first composition remains reviewable and needs no
/// separate agent press (sidebar.spec's arbitrary model-first case).
#[gpui::test]
fn a_catalog_model_picked_first_chooses_its_agent(cx: &mut TestAppContext) {
    let (mut helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    answer_catalog(&mut helm, cx, Some(model_catalog()));
    assert_eq!(
        draft(cx).agent.harness,
        None,
        "the premise: no agent chosen"
    );
    test_support::click(cx, "launcher-model-row");
    assert!(
        !has_node(cx, "launcher-model-show-all"),
        "all owners are already offered"
    );
    test_support::click(cx, "launcher-model-option-Codex-gpt-5");
    assert_eq!(draft(cx).agent.harness, Some(LaunchHarness::Codex));
    assert_eq!(draft(cx).agent.model.as_deref(), Some("gpt-5"));
    assert_eq!(model_text(cx), "gpt-5");
}

/// Prompt and named-start controls follow the selected agent's capabilities, including the capture warning needed to
/// explain why Restart cannot find some named starts later. Command mode exposes neither control (named-conversations
/// specs' capability and reporting cases).
#[gpui::test]
fn prompt_and_conversation_rows_follow_the_agents_capabilities(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    assert!(!drawn(cx, "launcher-prompt-row"));
    assert!(!drawn(cx, "launcher-conversation-row"));
    let warns = |cx: &mut VisualTestContext| {
        crate::session_list::tests::painted(&test_support::frame(cx))
            .iter()
            .any(|text| text.contains("Farhelm will not capture"))
    };
    // Cursor goes last so the Command switch has both fields and the warning to hide.
    for (button, agent, prompt, captures) in [
        ("launcher-harness-Claude", LaunchHarness::Claude, true, true),
        ("launcher-harness-Codex", LaunchHarness::Codex, true, true),
        ("launcher-harness-Grok", LaunchHarness::Grok, true, true),
        ("launcher-harness-Muse", LaunchHarness::Muse, true, true),
        ("launcher-harness-OMP", LaunchHarness::Omp, true, true),
        (
            "launcher-harness-OpenCode",
            LaunchHarness::OpenCode,
            true,
            false,
        ),
        ("launcher-harness-Goose", LaunchHarness::Goose, false, false),
        ("launcher-harness-Pi", LaunchHarness::Pi, false, true),
        (
            "launcher-harness-Cursor",
            LaunchHarness::Cursor,
            true,
            false,
        ),
    ] {
        test_support::click(cx, button);
        assert_eq!(draft(cx).agent.harness, Some(agent), "{button}");
        assert_eq!(drawn(cx, "launcher-prompt-row"), prompt, "{button}");
        assert!(drawn(cx, "launcher-conversation-row"), "{button}");
        assert_eq!(warns(cx), !captures, "{button}");
    }
    test_support::click(cx, "launcher-tabs-Command");
    assert_eq!(
        draft(cx).tab,
        Tab::Command,
        "the premise: Command is active"
    );
    assert!(!drawn(cx, "launcher-prompt-row"));
    assert!(!drawn(cx, "launcher-conversation-row"));
    assert!(!warns(cx));
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

// ---- The host picker -------------------------------------------------------------------------------------------------

/// The launcher on New on host1, with host1, host2, host3 and host11 registered, and folder `folder`.
fn host_picker_on<'a>(cx: &'a mut TestAppContext, folder: &str) -> &'a mut VisualTestContext {
    let (_helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::new(Some(1), folder.into(), None),
        fixture_hosts(&[0, 1, 2, 10]),
    );
    cx
}

/// The registry can lose a manually picked host while New is open. Keep the draft on that id and refuse Launch,
/// so a registry push cannot silently redirect the create to the local host.
#[gpui::test]
fn launch_refuses_after_the_chosen_host_disappears(cx: &mut TestAppContext) {
    let mut draft = super::launch_tests::command_draft();
    draft.edit_folder("/tmp".into());
    let (mut helm, cx) = launcher_with_hosts(cx, draft, fixture_hosts(&[0, 8]));
    test_support::click(cx, "launcher-host");
    test_support::click(cx, "launcher-host-option-9");
    assert_eq!(read(cx, |launcher, _, _| launcher.draft.host), Some(9));
    assert!(name_of(cx, "launcher-host").is_some_and(|name| name.contains("host9")));
    assert_eq!(
        launch_help(cx).as_deref(),
        Some("Launch: start the session")
    );
    assert!(super::launch_tests::creates(&mut helm, cx).is_empty());

    helm.push(farhelm_proto::ui_channel::CollectionChanges {
        collection: Collection::Hosts,
        meta: meta(&["1"]),
        upsert: Vec::new(),
        remove: vec!["9".into()],
    });
    settle(cx);
    assert_eq!(read(cx, |launcher, _, _| launcher.draft.host), Some(9));
    assert_eq!(name_of(cx, "launcher-host").as_deref(), Some("Host"));
    assert_eq!(
        name_of(cx, "launcher-summary-target").as_deref(),
        Some("Command · Unavailable host · /tmp"),
    );
    test_support::click(cx, "launcher-launch");
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Not launched. Choose a connected host before launching")
    );
    assert!(super::launch_tests::creates(&mut helm, cx).is_empty());
}

/// Whether the host button holds focus.
fn host_focused(cx: &mut VisualTestContext) -> bool {
    read(cx, |launcher, window, _| {
        launcher.host_focus.is_focused(window)
    })
}

/// The Host row names the chosen host, and its list shows every host in the registry's order but those a disposable
/// environment owns, each with its phase while not connected, so an unavailable host is seen as one; a click picks a
/// host and closes the list, re-seeding a folder the user has not typed (SPEC.md, "Working directory"). Why this
/// matters: the host is where the session runs, and a list that hid unavailable hosts or offered an environment's own
/// host would send the user somewhere they did not mean.
#[gpui::test]
fn the_host_list_offers_every_host_with_its_phase_and_a_click_picks_one(cx: &mut TestAppContext) {
    let cx = host_picker_on(cx, "/seeded");
    assert_eq!(name_of(cx, "launcher-host").as_deref(), Some("Host: host1"));
    test_support::click(cx, "launcher-host");
    assert!(
        has_node(cx, "launcher-host-list"),
        "the click opened the list"
    );
    assert_eq!(
        name_of(cx, "launcher-host-option-2").as_deref(),
        Some("host2 (Connecting)")
    );
    assert_eq!(
        name_of(cx, "launcher-host-option-3").as_deref(),
        Some("host3 (Unreachable, retrying)")
    );
    assert!(
        !has_node(cx, "launcher-host-option-11"),
        "an environment's host is not offered"
    );
    test_support::click(cx, "launcher-host-option-3");
    assert_eq!(draft(cx).host, Some(3));
    assert!(
        !has_node(cx, "launcher-host-list"),
        "picking closed the list"
    );
    assert_eq!(
        draft(cx).folder.value(),
        "~",
        "the untouched folder was re-seeded"
    );
    assert_eq!(
        name_of(cx, "launcher-host").as_deref(),
        Some("Host: host3 (Unreachable, retrying)")
    );
}

/// The host button is as wide as the chosen host's words (at least the old 12em), not the row, and 36 pixels high;
/// Browse, as high, sits on its own line right under the Host row, from the row's left edge; the host list is as wide
/// as the hosts it offers, not the button. Why: the old launcher drew a compact picker (`min-width: 12em; height:
/// 36px`) and its block layout put Browse under the row, and the port first stretched the button across the row with
/// Browse beside it, which the bridge drive caught beside the reference pictures; a list tied to the narrow button
/// would squeeze longer host names.
#[gpui::test]
fn the_host_button_is_as_wide_as_its_host_with_browse_under_it(cx: &mut TestAppContext) {
    let cx = host_picker_on(cx, "/seeded");
    let (button, browse, row) = (
        test_support::bounds(cx, "launcher-host"),
        test_support::bounds(cx, "launcher-browse"),
        test_support::bounds(cx, "launcher-host-row"),
    );
    assert!(
        button.size.width >= gpui::px(168.) && button.size.width < row.size.width / 2.,
        "compact, not the row's width: {button:?} in the row {row:?}"
    );
    assert_eq!(
        button.size.height,
        gpui::px(36.),
        "the host button's height"
    );
    assert_eq!(browse.size.height, gpui::px(36.), "Browse's height");
    assert_eq!(
        browse.top(),
        row.bottom(),
        "Browse sits right under the Host row"
    );
    assert_eq!(browse.left(), row.left(), "from the row's left edge");
    assert_eq!(
        name_of(cx, "launcher-host").as_deref(),
        Some("Host: host1"),
        "the premise: the button names the chosen host before the click"
    );
    test_support::click(cx, "launcher-host");
    // Read from the drawn frame, as `has_node` reads the list; `test_support::bounds` found no debug bounds for it.
    let list = test_support::frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("launcher-host-list"))
        .map(|node| node.bounds)
        .expect("the click opened the list");
    assert!(
        list.size.width > button.size.width,
        "the list widens for host3 (Unreachable, retrying): {list:?} against {button:?}"
    );
}

/// SPEC.md, "Launch templates": Save as template checks the host "only after a hand pick", and the old picker counted
/// re-picking the host already shown as one. A picker that skipped the shown host would leave New's prefilled host
/// unchecked however the user confirmed it. Specified: picking host1, already chosen, marks the host the user's own
/// choice and leaves the folder as it was.
#[gpui::test]
fn re_picking_the_shown_host_counts_as_a_hand_pick(cx: &mut TestAppContext) {
    let cx = host_picker_on(cx, "/seeded");
    assert!(
        !draft(cx).host_chosen,
        "the premise: New's host is a prefill"
    );
    test_support::click(cx, "launcher-folder-row");
    // Browse sits between the folder and the host button in Tab order, as the old app's did.
    keys(cx, "shift-tab shift-tab");
    assert!(host_focused(cx), "the premise: the host button has focus");
    // Down opens the list at the chosen host1, and Enter picks the highlighted row.
    keys(cx, "down");
    assert!(
        has_node(cx, "launcher-host-list"),
        "the premise: the list is open"
    );
    keys(cx, "enter");
    assert!(!has_node(cx, "launcher-host-list"), "Enter picked a host");
    assert_eq!(draft(cx).host, Some(1));
    assert!(draft(cx).host_chosen);
    assert_eq!(draft(cx).folder.value(), "/seeded");
}

/// The keyboard: with the button focused, Down opens the list at the chosen host and moves on, Enter picks the
/// highlighted host and closes the list; a typed folder survives the host change. Enter on the closed button presses
/// the main action, under the choice controls' Enter rule (SPEC.md, "Creation": "Enter on the closed pickers ...
/// launches with the shown values").
#[gpui::test]
fn the_host_picker_browses_with_arrows_and_enter(cx: &mut TestAppContext) {
    let cx = host_picker_on(cx, "~");
    test_support::click(cx, "launcher-folder-row");
    keys(cx, "x");
    // Browse sits between the folder and the host button in Tab order, as the old app's did.
    keys(cx, "shift-tab shift-tab");
    assert!(
        host_focused(cx),
        "the premise: Shift+Tab from the folder reached the host button"
    );
    keys(cx, "down");
    assert!(has_node(cx, "launcher-host-list"), "Down opened the list");
    keys(cx, "down enter");
    assert_eq!(
        draft(cx).host,
        Some(2),
        "Enter picked the highlighted host2"
    );
    assert!(!has_node(cx, "launcher-host-list"));
    assert_eq!(
        draft(cx).folder.value(),
        "~x",
        "a typed folder survives the host change"
    );
    assert_eq!(shown_refusal(cx), None, "the premise: nothing said yet");
    keys(cx, "enter");
    assert!(
        shown_refusal(cx).is_some(),
        "Enter on the closed button pressed the main action"
    );
}

/// Typeahead: letters typed within a second build a name prefix, and the highlight jumps to the first host it starts;
/// Escape closes the list without a choice and leaves the launcher open (the old picker's typeahead and cancel).
#[gpui::test]
fn typeahead_finds_a_host_and_escape_cancels(cx: &mut TestAppContext) {
    let cx = host_picker_on(cx, "~");
    test_support::click(cx, "launcher-folder-row");
    // Browse sits between the folder and the host button in Tab order, as the old app's did.
    keys(cx, "shift-tab shift-tab");
    assert!(host_focused(cx), "the premise: the host button has focus");
    keys(cx, "h o s t 3");
    assert!(has_node(cx, "launcher-host-list"), "typing opened the list");
    assert_eq!(
        selected_state(cx, "launcher-host-option-3"),
        Some(true),
        "host3 is highlighted"
    );
    keys(cx, "escape");
    assert!(
        !has_node(cx, "launcher-host-list"),
        "Escape closed the list"
    );
    assert!(is_open(cx), "but not the launcher");
    assert_eq!(draft(cx).host, Some(1), "and chose nothing");
}

/// Restart with keeps the source's host, so the row shows it as text and draws no picker (SPEC.md, "Restart with": the
/// held host stays visible).
#[gpui::test]
fn restart_with_shows_its_held_host_as_text(cx: &mut TestAppContext) {
    let mut source = super::draft_tests::session(codex(None, None, None));
    source.restart_offer = RestartOffer::Resume;
    let restart = LaunchDraft::restart_with("missing", &source, Some(1)).unwrap();
    let (_helm, cx) = launcher_on(cx, restart);
    assert!(!has_node(cx, "launcher-host"), "no picker");
    assert_eq!(name_of(cx, "launcher-host-name").as_deref(), Some("host1"));
}

/// The selected state the element under `id` reports in a fresh frame (`None` when selection does not apply to it).
pub(super) fn selected_state(cx: &mut VisualTestContext, id: &str) -> Option<bool> {
    test_support::frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .and_then(|node| node.selected)
}

/// Cancel and the host button are reached with Tab like every other control: the launcher owns their focus handles (it
/// starts Restart with on Cancel and watches the host button's focus), and a handle that was not a tab stop would leave
/// a keyboard user unable to reach them. Specified: Tab from the launch kind's tabs eventually reaches Cancel, and
/// Shift+Tab from the folder field reaches the host button, past Browse under it.
#[gpui::test]
fn cancel_and_the_host_button_are_tab_stops(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    test_support::click(cx, "launcher-folder-row");
    assert!(
        folder_focused(cx),
        "the premise: the click focused the folder"
    );
    // Browse sits between the folder and the host button in Tab order, as the old app's did.
    keys(cx, "shift-tab shift-tab");
    assert!(
        host_focused(cx),
        "Shift+Tab from the folder reached the host button"
    );
    let mut reached = false;
    for _ in 0..30 {
        keys(cx, "tab");
        if read(cx, |launcher, window, _| {
            launcher.cancel_focus.is_focused(window)
        }) {
            reached = true;
            break;
        }
    }
    assert!(reached, "Tab reached Cancel");
}

/// A typed prefix that matches no host leaves the highlight on the chosen host, so Enter then picks it again and
/// changes nothing (the old spec "host picker unmatched typeahead preserves a remote launch default": an unmatched
/// prefix must not revive an older highlight and redirect Enter to it).
#[gpui::test]
fn unmatched_typeahead_keeps_the_chosen_host(cx: &mut TestAppContext) {
    let cx = host_picker_on(cx, "~");
    test_support::click(cx, "launcher-host");
    keys(cx, "down down");
    keys(cx, "escape");
    assert_eq!(
        draft(cx).host,
        Some(1),
        "the premise: browsing chose nothing"
    );
    keys(cx, "z q");
    assert!(has_node(cx, "launcher-host-list"), "typing opened the list");
    assert_eq!(
        selected_state(cx, "launcher-host-option-1"),
        Some(true),
        "the chosen host stays highlighted"
    );
    keys(cx, "enter");
    assert_eq!(draft(cx).host, Some(1));
}

/// On macOS a click does not focus a button, but the host list is driven by the button's keys and closes when the
/// button loses focus, so opening it by a click moves focus to the button (the old spec "host picker opened without
/// click focus still owns the keyboard and closes outside"). Specified under the macOS click rule, from the folder
/// field: a click on the host button opens the list and focuses the button, Down and Enter then pick host2, and Tab
/// away from a reopened list closes it.
#[gpui::test]
fn a_click_opened_host_list_owns_the_keyboard_on_macos(cx: &mut TestAppContext) {
    let cx = host_picker_on(cx, "~");
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    test_support::click(cx, "launcher-folder-row");
    assert!(
        folder_focused(cx),
        "the premise: the folder field has focus"
    );
    test_support::click(cx, "launcher-host");
    assert!(
        has_node(cx, "launcher-host-list"),
        "the click opened the list"
    );
    assert!(
        host_focused(cx),
        "and moved focus to the button, though a Mac click focuses no button"
    );
    keys(cx, "down enter");
    assert_eq!(draft(cx).host, Some(2), "the button's keys drove the list");
    test_support::click(cx, "launcher-host");
    assert!(
        has_node(cx, "launcher-host-list"),
        "the premise: the list is open again"
    );
    keys(cx, "tab");
    assert!(
        !has_node(cx, "launcher-host-list"),
        "leaving the button closed the list"
    );
}

/// The same promise from where New starts: on macOS a click on the host button straight from the search box (New's
/// starting focus) opens the list and moves focus to the button. Why: a test Mac run of the shared tests on 2026-11-16
/// failed at this click from the search box while `a_click_opened_host_list_owns_the_keyboard_on_macos`, which focuses
/// the folder field first, passed (it did not reproduce on a later main), and the shared harness now runs those tests
/// under Linux's click rule, so only this test keeps the Mac path from the search box checked. Specified under the
/// macOS click rule, with focus on the search box: a click on the host button opens the list and focuses the button,
/// and Down and Enter then pick host2.
#[gpui::test]
fn a_mac_click_on_the_host_button_from_the_search_box_opens_the_list_and_focuses_the_button(
    cx: &mut TestAppContext,
) {
    let cx = host_picker_on(cx, "~");
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    assert!(
        read(cx, |launcher, window, cx| launcher
            .search
            .focus_handle(cx)
            .is_focused(window)),
        "the premise: New starts on the search box"
    );
    assert_eq!(
        name_of(cx, "launcher-host").as_deref(),
        Some("Host: host1"),
        "the premise: the button is drawn with the chosen host"
    );
    test_support::click(cx, "launcher-host");
    assert!(
        has_node(cx, "launcher-host-list"),
        "the click opened the list"
    );
    assert!(
        host_focused(cx),
        "and moved focus to the button, though a Mac click focuses no button"
    );
    keys(cx, "down enter");
    assert_eq!(draft(cx).host, Some(2), "the button's keys drove the list");
}

/// A press in the launcher's empty padding dismisses the host list. The padding keeps the current focus, so the test
/// isolates dismissal by an outside press from closing the list when its button loses focus.
#[gpui::test]
fn a_press_outside_closes_the_host_list_without_picking(cx: &mut TestAppContext) {
    let cx = host_picker_on(cx, "~");
    test_support::click(cx, "launcher-folder-row");
    // Browse is between the folder and host button in Tab order. End opens the list with another host highlighted.
    keys(cx, "shift-tab shift-tab");
    assert!(
        host_focused(cx),
        "the premise: the host button owns the keys"
    );
    keys(cx, "end");
    assert_eq!(selected_state(cx, "launcher-host-option-3"), Some(true));
    assert_eq!(
        draft(cx).host,
        Some(1),
        "the premise: browsing has picked nothing"
    );

    let list = test_support::frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("launcher-host-list"))
        .expect("the premise: the host list is open");
    let panel = test_support::bounds(cx, "launcher");
    let padding = gpui::point(panel.left() + gpui::px(4.), panel.top() + gpui::px(4.));
    assert!(
        !list.bounds.contains(&padding),
        "the press must miss the list"
    );
    assert!(
        !test_support::bounds(cx, "launcher-host").contains(&padding),
        "the press must miss the toggle"
    );
    cx.simulate_mouse_move(padding, None, gpui::Modifiers::none());
    cx.simulate_mouse_down(padding, gpui::MouseButton::Left, gpui::Modifiers::none());
    settle(cx);
    assert!(
        !has_node(cx, "launcher-host-list"),
        "the press closed the list"
    );
    assert!(host_focused(cx), "the press in empty space kept focus");
    assert_eq!(
        draft(cx).host,
        Some(1),
        "the highlighted host was not picked"
    );
    cx.simulate_mouse_up(padding, gpui::MouseButton::Left, gpui::Modifiers::none());
    settle(cx);
}

/// While the chosen host is not in the registry the helm pushed (before hosts arrive, or with none registered, which
/// the button cannot tell apart), the button shows nothing and is named just "Host" (SPEC.md: the host's value is
/// empty before hosts arrive), rather than naming a host that is not there. Specified with an empty hosts collection.
#[gpui::test]
fn the_host_button_is_empty_before_hosts_arrive(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_with_hosts(
        cx,
        LaunchDraft::new(Some(1), "~".into(), None),
        fixture_hosts(&[]),
    );
    assert_eq!(name_of(cx, "launcher-host").as_deref(), Some("Host"));
    assert!(
        !has_node(cx, "launcher-host-name"),
        "no host words are drawn"
    );
}

/// The open list's other keys, as the old picker's: End and Home jump to the last and first host, Up moves back and
/// stops at the first, and Space picks the highlighted host, as Enter does.
#[gpui::test]
fn home_end_up_and_space_in_the_host_list(cx: &mut TestAppContext) {
    let cx = host_picker_on(cx, "~");
    test_support::click(cx, "launcher-host");
    assert!(host_focused(cx), "the premise: the button has focus");
    keys(cx, "end");
    assert_eq!(
        selected_state(cx, "launcher-host-option-3"),
        Some(true),
        "End: the last host"
    );
    keys(cx, "home");
    assert_eq!(
        selected_state(cx, "launcher-host-option-1"),
        Some(true),
        "Home: the first host"
    );
    keys(cx, "up");
    assert_eq!(
        selected_state(cx, "launcher-host-option-1"),
        Some(true),
        "Up stops at the first host"
    );
    keys(cx, "down space");
    assert_eq!(
        draft(cx).host,
        Some(2),
        "Space picked the highlighted host2"
    );
    assert!(!has_node(cx, "launcher-host-list"));
}

// ---- The Command tab's declared agent type -----------------------------------------------------------------------------

/// The Command tab's agent type says which agent the command runs, so Farhelm can track and resume it (SPEC.md,
/// "Command launch": the optional declared agent type); declaring one offers the `{farhelm_args}` hint and Resume, and
/// None takes them away again. Why this matters: Resume is only meaningful for a declared agent, so without the choice
/// a command launch could never be resumed, and Resume offered without it would promise something Farhelm cannot do.
#[gpui::test]
fn declaring_the_commands_agent_type_offers_resume(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    test_support::click(cx, "launcher-tabs-Command");
    assert!(
        drawn(cx, "launcher-command-agent-row"),
        "the agent type is offered"
    );
    assert!(
        !drawn(cx, "launcher-resume"),
        "the premise: no Resume without an agent type"
    );
    test_support::click(cx, "launcher-command-agent");
    test_support::click(cx, "launcher-command-agent-option-Codex");
    assert_eq!(draft(cx).command.agent, Some(LaunchHarness::Codex));
    assert!(
        drawn(cx, "launcher-resume"),
        "Resume is offered for a declared agent"
    );
    assert!(
        painted(cx, "Put {farhelm_args}"),
        "and the farhelm_args hint"
    );
    test_support::click(cx, "launcher-command-agent");
    test_support::click(cx, "launcher-command-agent-option-none");
    assert_eq!(draft(cx).command.agent, None);
    assert!(!drawn(cx, "launcher-resume"), "and goes with None");
}

/// Enter on the closed agent type select or on the Resume checkbox presses Launch with the values shown, changing
/// neither (the old spec "launcher value-control Enter keeps the shown choices"; SPEC.md, "Creation": Enter on the
/// closed pickers and checkboxes "launches with the shown values"). Specified with no host chosen, so each press is
/// refused and says so: Enter on the select after choosing Codex shows the refusal and keeps Codex; after an edit takes
/// the refusal away, Enter on the focused Resume checkbox shows it again and leaves Resume as it was.
#[gpui::test]
fn enter_on_the_commands_agent_type_and_resume_presses_launch(cx: &mut TestAppContext) {
    let (_helm, cx) = launcher_on(cx, LaunchDraft::new(None, "~".into(), None));
    test_support::click(cx, "launcher-tabs-Command");
    test_support::click(cx, "launcher-command-agent");
    test_support::click(cx, "launcher-command-agent-option-Codex");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("launcher-command-agent"),
        "the premise: the closed select holds focus"
    );
    assert_eq!(shown_refusal(cx), None, "the premise: nothing said yet");
    keys(cx, "enter");
    assert!(
        shown_refusal(cx).is_some(),
        "Enter on the select pressed Launch"
    );
    assert_eq!(
        draft(cx).command.agent,
        Some(LaunchHarness::Codex),
        "and kept the shown agent"
    );
    // Two clicks leave Resume as it was but count as edits, which take the refusal away.
    test_support::click(cx, "launcher-resume");
    test_support::click(cx, "launcher-resume");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("launcher-resume"),
        "the premise: Resume holds focus"
    );
    assert_eq!(
        shown_refusal(cx),
        None,
        "the premise: the edits took the refusal away"
    );
    let resume = draft(cx).command.resume_on;
    keys(cx, "enter");
    assert!(
        shown_refusal(cx).is_some(),
        "Enter on Resume pressed Launch"
    );
    assert_eq!(
        draft(cx).command.resume_on,
        resume,
        "and left Resume as it was"
    );
}

/// Whether the quick-launch sequence's pill in its launcher form ("Apply sequence:") is drawn: the sign a sequence was
/// armed from the launcher.
fn sequence_armed(cx: &mut VisualTestContext) -> bool {
    crate::session_list::tests::painted(&test_support::frame(cx))
        .iter()
        .any(|line| line.contains("Apply sequence:"))
}

/// SPEC.md, "Quick-launch sequences": "In the launcher's own text fields it starts a sequence only when no text is
/// selected; with a selection it keeps Cut", while every other text field keeps the chord
/// (`templates::armed_tests::a_text_field_keeps_the_prefix`). The user composes a launch in these fields, so a prefix a
/// field swallowed would leave sequences out of reach there. Specified: with New open on `~`, `abc` typed in the search
/// box, where focus starts, and nothing selected there, the prefix arms the launcher's form of the sequence and leaves
/// the query `abc`; Escape ends it. With focus moved to the folder field, the prefix arms the sequence again and leaves
/// the folder `~`, focus still in the field; Escape ends it; Select All selects the whole folder, and then the prefix
/// arms nothing, and leaves `~` on Linux (GPUI Kit binds that chord to nothing) or cuts it on a Mac (Cmd+X is Cut
/// there).
#[gpui::test]
fn the_prefix_in_a_launcher_field_begins_a_sequence_only_with_nothing_selected(
    cx: &mut TestAppContext,
) {
    let (_helm, cx) = launcher_on(cx, LaunchDraft::new(Some(1), "~".into(), None));
    cx.simulate_input("abc");
    settle(cx);
    let query = |cx: &mut VisualTestContext| {
        read(cx, |launcher, _, cx| launcher.search.text(cx).to_string())
    };
    assert_eq!(
        query(cx),
        "abc",
        "the premise: the search box, where focus starts, holds the query"
    );
    assert!(
        read(cx, |launcher, _, cx| launcher.search.selection_empty(cx)),
        "the premise: nothing is selected in it"
    );
    keys(cx, crate::actions::SEQUENCE_PREFIX_CHORD);
    assert!(
        sequence_armed(cx),
        "the prefix in the search box armed a sequence"
    );
    assert_eq!(query(cx), "abc", "and left the query");
    keys(cx, "escape");
    assert!(!sequence_armed(cx), "the premise: Escape ended it");

    test_support::click(cx, "launcher-folder-row");
    assert!(
        focused(cx, Field::Folder),
        "the premise: the folder field holds focus"
    );
    keys(cx, crate::actions::SEQUENCE_PREFIX_CHORD);
    assert!(sequence_armed(cx), "the prefix armed a sequence");
    assert_eq!(
        draft(cx).folder.value(),
        "~",
        "the field's text is unchanged"
    );
    assert!(focused(cx, Field::Folder), "focus stayed in the field");
    keys(cx, "escape");
    assert!(!sequence_armed(cx), "the premise: Escape ended it");

    keys(cx, SELECT_ALL);
    assert!(
        read(cx, |launcher, _, cx| !launcher.folder.selection_empty(cx)),
        "the premise: Select All selected the folder"
    );
    keys(cx, crate::actions::SEQUENCE_PREFIX_CHORD);
    assert!(
        !sequence_armed(cx),
        "with a selection the prefix armed nothing"
    );
    let kept = if cfg!(target_os = "macos") { "" } else { "~" };
    assert_eq!(
        draft(cx).folder.value(),
        kept,
        "and the field kept its chord"
    );
}

// ---- Font roles, with the real text system ---------------------------------------------------------------------------

/// Run `test` on a test app whose text system is the real one, knowing only the bundled fonts, with the checks and the
/// teardown `#[gpui::test]` and `gpui::run_test` give the app they make: the entity leak check once the tasks are
/// drained, and the scheduler's end-of-test check. GPUI's test text system gives every face the same widths, so only a
/// real one can tell the interface face from the monospace one; a test app built over one, rather than the headless
/// app the sidebar's font tests draw in, keeps the shared harness's simulated typing and per-step checks.
fn with_real_text_system(test: impl FnOnce(&mut TestAppContext)) {
    let dispatcher = gpui::TestDispatcher::new(0);
    let scheduler = dispatcher.scheduler().clone();
    let text_system = std::sync::Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
        crate::assets::UI_FONT,
    ));
    let mut cx = TestAppContext::build_with_text_system(dispatcher.clone(), None, text_system);
    // Held until the tasks are drained, as the macro holds it: dropping it runs the leak detector.
    let entity_refcounts = cx.app.borrow().ref_counts_drop_handle();
    test(&mut cx);
    cx.run_until_parked();
    cx.update(|cx| {
        cx.background_executor().forbid_parking();
        cx.quit();
    });
    cx.run_until_parked();
    drop(cx);
    dispatcher.drain_tasks();
    drop(dispatcher);
    drop(entity_refcounts);
    scheduler.end_test();
}

/// `text`'s width at the launcher's 13-pixel text size, shaped by the window's text system in the monospace face when
/// `mono` is set and in the interface face otherwise.
fn face_width(cx: &mut VisualTestContext, text: &str, mono: bool) -> f32 {
    let family = if mono {
        crate::assets::TERMINAL_FONT
    } else {
        crate::assets::UI_FONT
    };
    let run = gpui::TextStyle {
        font_family: family.into(),
        ..gpui::TextStyle::default()
    }
    .to_run(text.len());
    cx.update(|window, _| {
        let line = window.text_system().shape_line(
            text.to_owned().into(),
            crate::theme::metrics::TEXT_MD,
            &[run],
            None,
        );
        f32::from(line.width)
    })
}

/// The old `interface-fonts.spec.ts` case "interface and technical values load their own bundled font roles", its
/// launcher half: the old launcher drew the Folder and Agent command fields in the monospace face, as technical
/// values, the search box in the interface face, and its "Use this path" and "Browse this path" rows with their words
/// in the interface face and the path in the monospace face. Only the real text system can tell the faces apart. The
/// draft is on the Command tab, which shows the Folder field and the search box as the Agent tab does, so one launcher
/// shows every field. Specified, with the real text system, on New's Command tab with the folder `/iiiiiiii` and the
/// agent command `iiiiiiii`, and `/iiiiiiii` typed into the search box, where focus starts: the Folder field, the Agent
/// command field, the search box and the two rows each paint one line holding their text (a row's sentence isolates
/// aside), as wide as its parts drawn in their faces at 13 pixels, within half a pixel: the fields' values in the
/// monospace face, the query in the interface face, and each row's words in the interface face followed by the path in
/// the monospace face. The premise, per case: every other choice of face for the line's parts draws it at least a pixel
/// wider or narrower.
#[test]
fn the_launchers_paths_and_command_are_monospace_beside_words_in_the_interface_face() {
    const PATH: &str = "/iiiiiiii";
    const COMMAND: &str = "iiiiiiii";
    with_real_text_system(|cx| {
        let mut start = LaunchDraft::new(Some(1), PATH.into(), None);
        start.choose_tab(Tab::Command);
        start.edit_command(COMMAND.into());
        let (_helm, cx) = launcher_on(cx, start);
        assert!(
            search_focused(cx),
            "the premise: focus starts in the search box"
        );
        cx.simulate_input(PATH);
        settle(cx);
        let nodes = test_support::frame(cx);
        // A search row is found by its accessible name, its words and the path, since its id is its place in the list.
        let row = |words: &str| -> String {
            let label = format!("{words}{PATH}");
            nodes
                .iter()
                .find(|node| node.label.as_deref() == Some(label.as_str()))
                .and_then(|node| node.element_id.as_deref())
                .unwrap_or_else(|| panic!("a search row named {label:?} was drawn"))
                .to_owned()
        };
        // Each case: what is measured, the id it is drawn under, and its parts, each with whether it is monospace.
        type Case = (&'static str, String, &'static [(&'static str, bool)]);
        let cases: [Case; 5] = [
            (
                "the Folder field",
                "launcher-folder".into(),
                &[(PATH, true)],
            ),
            (
                "the Agent command field",
                "launcher-command".into(),
                &[(COMMAND, true)],
            ),
            ("the search box", "launcher-search".into(), &[(PATH, false)]),
            (
                "the Use this path row",
                row("Use this path: "),
                &[("Use this path: ", false), (PATH, true)],
            ),
            (
                "the Browse this path row",
                row("Browse this path: "),
                &[("Browse this path: ", false), (PATH, true)],
            ),
        ];
        // A line's text as the user sees it: a sentence draws a host value inside Unicode isolates (`host_text`),
        // which draw as nothing.
        let shown = |line: &gpui::DriverText| -> String {
            line.text
                .chars()
                .filter(|c| !('\u{2066}'..='\u{2069}').contains(c))
                .collect()
        };
        for (case, id, parts) in cases {
            let text: String = parts.iter().map(|(part, _)| *part).collect();
            let lines = crate::session_list::host_rail::tests::texts_within(&nodes, &id);
            let matching: Vec<_> = lines.iter().filter(|&line| shown(line) == text).collect();
            let [line] = &matching[..] else {
                let painted: Vec<_> = lines.iter().map(|line| &line.text).collect();
                panic!("{case} paints {text:?} on one line; it painted {painted:?}");
            };
            // The line's width for every choice of face for its parts, the specified choice first: bit `n` of `flips`
            // puts part `n` in the other face.
            let mut choices = Vec::new();
            for flips in 0..1u32 << parts.len() {
                let mut total = 0.;
                for (n, (part, mono)) in parts.iter().enumerate() {
                    total += face_width(cx, part, *mono != ((flips >> n) & 1 == 1));
                }
                choices.push(total);
            }
            let (specified, others) = (choices[0], &choices[1..]);
            assert!(
                others.iter().all(|other| (other - specified).abs() >= 1.),
                "premise: other faces draw {case} apart from {specified}: {others:?}"
            );
            let drawn = f32::from(line.bounds.size.width);
            assert!(
                (drawn - specified).abs() < 0.5,
                "{case} is {drawn} wide; its specified faces draw it {specified}, the others {others:?}"
            );
        }
    });
}
