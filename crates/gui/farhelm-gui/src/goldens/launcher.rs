//! The real session launcher over a fake helm. Its drawing and focus come from the regular New entry point.

use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::launcher::{LaunchTemplate, TemplateFields};
use farhelm_proto::tensorlake::SshKeyStatus;
use farhelm_proto::ui_channel::{Collection, CollectionState, HelmMarker, KeyedEntry, Preferences};
use farhelm_proto::{LaunchEffort, LaunchHarness, LaunchPermission};
use gpui::{App, AppContext as _, Entity, Window, point, px};
use serde_json::json;

use super::{SceneState, fleet, session_list_scene_preferring};
use crate::launcher::entry;
use crate::launcher::environment_tests::{account, recipe_json};
use crate::session_list::SessionList;
use crate::session_list::tests::{preferences_state, session};
use crate::test_support::{click_before_redraw, meta};

/// Show the temporary capsule with the launcher hidden and the fake's Resume still outstanding.
pub(super) fn launcher_wake_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    use crate::launcher::view_tests::fixture_hosts;
    use crate::session_list::tests::{listing, preferences};
    window.set_driver_enabled(true);
    crate::ticker::set_reduced_motion(true, cx);
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = crate::session::start(handle, events, None, cx);
    let hosts = fixture_hosts(&[0, 11]);
    helm.connect(vec![listing(&[], &[]), preferences(&[], None), hosts]);
    state.lists.borrow_mut().push(cx.new(SessionList::new));
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    state.after.set(Some(open_wake_launcher));
    state
        .after_capture
        .set(Some(super::session_list::disconnect_pending_launch));
}

/// Use the same visible launcher press that hides the form and starts Resume.
fn open_wake_launcher(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    use farhelm_proto::ui_channel::{HostStateView, HostView};
    let Some(store) = crate::session::current_store(cx) else {
        state.after.set(Some(open_wake_launcher));
        return;
    };
    let host = store
        .read(cx)
        .collections()
        .get::<HostView>(&HostView::key(12))
        .expect("the fixture lists its suspended host");
    assert!(matches!(host.state, HostStateView::Suspended { .. }));
    let mut draft = crate::launcher::launch_tests::command_draft();
    draft.choose_host(12, "~".into());
    draft.edit_folder("/home/user/work".into());
    let mark = crate::focus::Mark::take(cx);
    assert!(crate::launcher::view::open(draft, &mark, window, cx).is_some());
    state.after.set(Some(press_wake_launcher));
}

/// The capture waits for the actual launch button and then for the wake it opens.
fn press_wake_launcher(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    if click_control(window, cx, "launcher-launch") {
        assert!(crate::launcher::launch::waking(cx));
    } else {
        state.after.set(Some(press_wake_launcher));
    }
}

/// Which launcher interaction must settle before capture.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LauncherScene {
    Form,
    HostMenu,
    Container,
    RememberedWrapper,
    ShellWrapper,
    ShellWrapperNote,
    RepositoryRefusal,
    Results,
    Restart,
    Replace,
    Narrow,
    Yolo,
    HookSetup,
}

/// Show New with a chosen agent, so the screenshot covers the model, effort and permission controls and the prompt.
pub(super) fn launcher_new_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, LauncherScene::Form, window, cx);
}

/// Open the host menu once both new-environment offers can be drawn from the fixture.
pub(super) fn launcher_host_menu_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, LauncherScene::HostMenu, window, cx);
}

/// Take New container through the host menu so the capture shows what replaces an editable folder.
pub(super) fn launcher_container_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, LauncherScene::Container, window, cx);
}

/// Show a wrapper brought in by the actual harness choice, without opening More options to see it.
pub(super) fn launcher_more_options_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, LauncherScene::RememberedWrapper, window, cx);
}

/// Open a Bash launch with the wrapper row expanded and its help visible.
pub(super) fn launcher_shell_wrapper_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, LauncherScene::ShellWrapper, window, cx);
}

/// The ordinary field label is readable before its tooltip opens above the input.
pub(super) fn launcher_shell_wrapper_note_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    setup(state, LauncherScene::ShellWrapperNote, window, cx);
}

/// An escaped host-authored failure beneath the command field that asked for a repository search.
pub(super) fn launcher_repository_refusal_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    setup(state, LauncherScene::RepositoryRefusal, window, cx);
}

/// Capture a typed query against the open host catalog and file history, with a source already applied to the form.
pub(super) fn launcher_results_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, LauncherScene::Results, window, cx);
}

/// Restart keeps the recorded agent and destination fixed. Its command pane shows why their rows cannot apply.
pub(super) fn launcher_restart_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, LauncherScene::Restart, window, cx);
}

/// Keep the source warning and held host visible beside the same form and command pane used by New.
pub(super) fn launcher_replace_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, LauncherScene::Replace, window, cx);
}

/// Open a template query at the minimum width so the screenshot proves the popover fallback is reachable.
pub(super) fn launcher_narrow_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, LauncherScene::Narrow, window, cx);
}

/// Ask the ordinary inline YOLO question, leaving the real launcher actions and both panes drawn around it.
pub(super) fn launcher_yolo_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, LauncherScene::Yolo, window, cx);
}

/// Show the first-pick consent question with an off switch and no suppressed hint.
pub(super) fn launcher_hook_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, LauncherScene::HookSetup, window, cx);
}

fn setup(state: &SceneState, scene: LauncherScene, window: &mut Window, cx: &mut App) {
    state.keep.borrow_mut().push(Box::new(scene));
    window.set_driver_enabled(true);
    let mut rows = fleet();
    if scene == LauncherScene::Restart {
        // The stored resume command makes Restart with available even before a conversation has been captured.
        rows[0] = session("a")
            .host(1)
            .host_name("Local (this machine)")
            .title("Review the launch workflow")
            .cwd("/home/user/src/api")
            .launch(json!({
                "kind": "agent", "selection": { "harness": "claude", "permissions": "guarded" },
                "start": ["claude", "{farhelm_args}"],
                "resume": ["claude", "--resume", "{conversation}", "{farhelm_args}"]
            }));
    }
    let mut preferences = Preferences {
        welcome_seen: Some(true),
        visit_history: vec!["a".into()],
        template_shortcuts: [
            ("1".into(), "Review a PR".into()),
            ("2".into(), "Bench run".into()),
        ]
        .into(),
        quick_launch_template: Some("Review a PR".into()),
        ..Preferences::default()
    };
    if scene == LauncherScene::RememberedWrapper {
        preferences.remembered_wrapper_prefixes.insert(
            LaunchHarness::Claude,
            ["with-endpoint", "--profile", "work", "--"]
                .map(str::to_owned)
                .to_vec(),
        );
    }
    let mut extra = vec![launcher_templates()];
    if matches!(scene, LauncherScene::HostMenu | LauncherScene::Container) {
        // The recipe catalog must already be drawn when the host menu picks New container.
        extra.push(CollectionState {
            collection: Collection::Recipes,
            meta: meta(&["Development"]),
            entries: vec![KeyedEntry {
                key: "Development".into(),
                value: recipe_json("Development", json!({ "kind": "tensorlake_ubuntu" })),
            }],
        });
    }
    session_list_scene_preferring(state, &rows, preferences_state(preferences), extra, cx);
    state.after.set(Some(open_new_with_claude));
}

/// The empty pane shows saved rows and their real marks and shortcut bindings before a query is entered.
fn launcher_templates() -> CollectionState {
    let names = ["Review a PR", "Bench run", "Scratch shell"];
    let fields = [
        TemplateFields {
            agent: Some(LaunchHarness::Claude),
            effort: Some(Some(LaunchEffort::High)),
            ..Default::default()
        },
        TemplateFields {
            agent: Some(LaunchHarness::Codex),
            permissions: Some(Some(LaunchPermission::Yolo)),
            ..Default::default()
        },
        TemplateFields {
            agent: Some(LaunchHarness::Bash),
            ..Default::default()
        },
    ];
    CollectionState {
        collection: Collection::Templates,
        meta: meta(&names),
        entries: names
            .into_iter()
            .zip(fields)
            .map(|(name, fields)| KeyedEntry {
                key: name.into(),
                value: serde_json::to_value(LaunchTemplate {
                    name: name.into(),
                    fields,
                })
                .expect("the launcher fixture serializes a stored template"),
            })
            .collect(),
    }
}

fn launcher_scene(state: &SceneState) -> LauncherScene {
    state
        .keep
        .borrow()
        .iter()
        .find_map(|item| item.downcast_ref::<LauncherScene>().copied())
        .expect("launcher scene setup named the interaction")
}

/// Open through the same entry points used by New, Restart with and Replace with, after the fleet snapshot arrives.
fn open_new_with_claude(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(mut draft) = entry::new_prefill(cx) else {
        state.after.set(Some(open_new_with_claude));
        return;
    };
    let scene = launcher_scene(state);
    if matches!(scene, LauncherScene::Restart | LauncherScene::Replace) {
        let open = if scene == LauncherScene::Restart {
            entry::open_restart_with
        } else {
            entry::open_replace_with
        };
        assert!(
            open("a", window, cx),
            "the source supports its launcher mode"
        );
        state.after.set(Some(answer_new_reads));
        return;
    }
    let remembered = scene == LauncherScene::RememberedWrapper;
    draft.edit_folder("/home/user/src/api".into());
    if matches!(
        scene,
        LauncherScene::ShellWrapper | LauncherScene::ShellWrapperNote
    ) {
        draft.choose_harness(LaunchHarness::Bash, None, None);
    } else if !remembered {
        draft.choose_harness(LaunchHarness::Claude, None, None);
    }
    assert!(
        entry::open_with(draft, None, window, cx),
        "New opens once the helm is connected"
    );
    state.after.set(Some(match scene {
        LauncherScene::RememberedWrapper => pick_claude,
        LauncherScene::ShellWrapper | LauncherScene::ShellWrapperNote => open_shell_wrapper,
        _ => answer_new_reads,
    }));
}

/// The wrapper's text and hover help belong to the rendered field, so open More options before resting the pointer.
fn open_shell_wrapper(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    if !click_control(window, cx, "launcher-more-options") {
        state.after.set(Some(open_shell_wrapper));
    } else if launcher_scene(state) == LauncherScene::ShellWrapper {
        state.hover.set(Some("launcher-wrapper"));
    }
}

/// Pick Claude through the same control a user would press, so the summary depends on the helm's saved preference.
fn pick_claude(_: &Entity<SessionList>, state: &SceneState, window: &mut Window, cx: &mut App) {
    if click_control(window, cx, "launcher-harness-Claude") {
        state.after.set(Some(answer_new_reads));
    } else {
        state.after.set(Some(pick_claude));
    }
}

/// Reply in send order and wait for the model count from that catalog in the drawn form. A stable empty frame or a
/// frame drawn before the catalog reply cannot stand in for the launcher this scene means to show.
fn answer_new_reads(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    let scene = launcher_scene(state);
    let environment = matches!(scene, LauncherScene::HostMenu | LauncherScene::Container);
    let mut keep = state.keep.borrow_mut();
    let helm = keep
        .iter_mut()
        .find_map(|kept| kept.downcast_mut::<FakeHelm>())
        .expect("the New scene keeps its fake helm");
    let mut repository_answered = false;
    for command in helm.commands() {
        if scene == LauncherScene::RepositoryRefusal && command.path == "/api/github-repositories" {
            helm.reply(
                command.id,
                504,
                Some(json!("The build host could not check acme\u{202e}/tools")),
            );
            repository_answered = true;
            continue;
        }
        let body = if command.path == "/api/launch-catalog" {
            Some(
                json!([{ "id": "sonnet", "harness": "claude", "efforts": ["low", "medium", "high"] }]),
            )
        } else if command.path == "/api/tensorlake" && environment {
            Some(json!(account(true, SshKeyStatus::NotCreated)))
        } else if command.path.starts_with("/api/launch-history") {
            Some(launcher_history())
        } else {
            None
        };
        helm.reply(command.id, 200, body);
    }
    let catalog_drawn = window.driver_nodes().iter().any(|node| {
        node.element_id.as_deref() == Some("launcher-model-hint")
            && node.label.as_deref() == Some("1 for Claude")
    });
    let catalog_template = window
        .driver_nodes()
        .iter()
        .any(|node| node.element_id.as_deref() == Some("launcher-template-Review a PR"));
    let nodes = window.driver_nodes();
    let recent = nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some("launcher-recent-0"));
    // Query scenes can return here with the results pane already replacing the template list.
    let shows_templates = !matches!(
        scene,
        LauncherScene::Narrow | LauncherScene::RepositoryRefusal
    );
    if !catalog_drawn || (shows_templates && !catalog_template) {
        state.after.set(Some(answer_new_reads));
    } else if environment {
        state.after.set(Some(open_host_menu));
    } else if scene == LauncherScene::RememberedWrapper {
        let summary = nodes
            .iter()
            .find(|node| node.element_id.as_deref() == Some("launcher-more-options-wrapper"))
            .expect("the folded row shows the remembered wrapper");
        assert_eq!(
            summary.label.as_deref(),
            Some("with-endpoint --profile work --")
        );
        assert!(
            !nodes
                .iter()
                .any(|node| node.element_id.as_deref() == Some("launcher-wrapper"))
        );
        // The chip was used to bring in the saved preference. The scene shows the editor ready for the next word.
        let launcher =
            entry::open_launcher(window, cx).expect("the wrapper is in the open launcher");
        // Through the focus coordinator, as every focus move the app makes is: the launcher is the open dialog.
        let field = launcher.read(cx).search_for_golden().focus_handle(cx);
        crate::focus::focus_in_layer(&field, &crate::focus::Mark::take(cx), window, cx);
    } else if scene == LauncherScene::Yolo {
        state.after.set(Some(choose_yolo));
    } else if scene == LauncherScene::HookSetup {
        state.after.set(Some(pick_grok));
    } else if matches!(
        scene,
        LauncherScene::Narrow | LauncherScene::RepositoryRefusal
    ) {
        if scene == LauncherScene::RepositoryRefusal && repository_answered {
            state.after.set(Some(wait_for_launcher_results));
        } else {
            state.after.set(Some(type_launcher_query));
        }
    } else {
        let Some(recent) = recent else {
            state.after.set(Some(answer_new_reads));
            return;
        };
        if scene == LauncherScene::Results {
            state.after.set(Some(apply_fixture_setup));
        } else if scene == LauncherScene::Restart {
            assert!(
                !recent.flags.click,
                "a held recent row stays in the pane and cannot apply"
            );
            assert!(
                nodes.iter().any(|node| node.element_id.as_deref()
                    == Some("launcher-held-harness")
                    && node.label.as_deref() == Some("Agent: Claude")),
                "Restart names the fixed harness without an editable harness control"
            );
        }
    }
}

/// Pick Grok after the catalog, then wait for the question from the setting's off reply.
fn pick_grok(_: &Entity<SessionList>, state: &SceneState, window: &mut Window, cx: &mut App) {
    state.after.set(Some(
        if click_control(window, cx, "launcher-harness-Grok") {
            answer_hook_setup
        } else {
            pick_grok
        },
    ));
}

fn answer_hook_setup(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    _: &mut App,
) {
    let mut keep = state.keep.borrow_mut();
    let helm = keep
        .iter_mut()
        .find_map(|kept| kept.downcast_mut::<FakeHelm>())
        .expect("the launcher scene keeps its fake helm");
    for command in helm.commands() {
        let answer =
            (command.path == "/api/agent-hook-setup/grok").then(|| json!({ "enabled": false }));
        helm.reply(command.id, 200, answer);
    }
    if !window
        .driver_nodes()
        .iter()
        .any(|node| node.element_id.as_deref() == Some("hook-setup-consent"))
    {
        state.after.set(Some(answer_hook_setup));
    }
}

/// Seed the actual launch-history response so the view has to match setups and folders against the real draft.
fn launcher_history() -> serde_json::Value {
    let entry = |cwd: &str, selection| {
        json!({
            "host": 1, "cwd": cwd, "canonical_cwd": cwd, "github_repo": null,
            "selection": selection, "created_at": 1, "creation_seq": null,
        })
    };
    json!({
        "launches": [
            entry("/home/user/src/api", json!({"harness": "claude", "permissions": "guarded"})),
            entry("/home/user/src/api", json!({"harness": "claude", "permissions": "yolo"})),
            entry("/home/user/src/api", json!({
                "harness": "claude", "permissions": "guarded", "effort": "high",
            })),
            entry("/home/user/src/build-tools", json!({
                "harness": "claude", "permissions": "guarded", "effort": "high", "model": "sonnet",
                "wrapper_prefix": ["with-endpoint", "--profile", "work", "--"]
            })),
        ],
        "folders": [{"host": 1, "display_cwd": "/home/user/src/build-tools", "canonical_cwd": "/home/user/src/build-tools",
                     "canonical_proven": true, "created_at": 1, "creation_seq": null}],
        "checkout_config_revision": 1,
    })
}

/// Apply a recent setup with the pointer so its source pill and the editor's restored focus are both real.
fn apply_fixture_setup(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    assert!(click_control(window, cx, "launcher-recent-0"));
    state.after.set(Some(type_launcher_query));
}

/// Let the real text input's change event open the results. No fixture draws a substitute list.
fn type_launcher_query(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    let launcher = entry::open_launcher(window, cx).expect("the launcher is open for typing");
    let field = launcher.read(cx).search_for_golden();
    assert!(
        field.focus_handle(cx).is_focused(window),
        "the command editor is ready for the query"
    );
    let (query, stage): (&str, super::AfterDrawn) = match launcher_scene(state) {
        LauncherScene::Narrow => ("tl:", wait_for_launcher_results),
        LauncherScene::RepositoryRefusal => ("gh:acme", answer_new_reads),
        _ => ("build", wait_for_launcher_results),
    };
    if field.text(cx).as_ref() != query {
        field.edit_for_golden(query, window, cx);
    } else if launcher_scene(state) == LauncherScene::RepositoryRefusal {
        // The real repository read is debounced on the renderer's simulated clock.
        state
            .advance_clock
            .set(Some(std::time::Duration::from_millis(160)));
    }
    state.after.set(Some(stage));
}

/// The frame must include the actual results and the pane only when there is room for both columns.
fn wait_for_launcher_results(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    _: &mut App,
) {
    if launcher_scene(state) == LauncherScene::RepositoryRefusal {
        if !window.driver_nodes().iter().any(|node| {
            node.element_id.as_deref() == Some("launcher-search-repository-note")
                && !node.visible_bounds.is_empty()
        }) {
            state.after.set(Some(wait_for_launcher_results));
        }
        return;
    }
    let shown = |id: &str| {
        window
            .driver_nodes()
            .iter()
            .any(|node| node.element_id.as_deref() == Some(id))
    };
    if !shown("launcher-search-option-0") {
        state.after.set(Some(wait_for_launcher_results));
    } else {
        let wide = launcher_scene(state) != LauncherScene::Narrow;
        assert_eq!(shown("launcher-command-pane"), wide);
        if wide {
            assert!(
                shown("launcher-provenance-label"),
                "the recent setup filled the form"
            );
            assert!(
                shown("launcher-search-group-Folders")
                    && shown("launcher-search-group-Recent setups"),
                "the typed scene includes both folder and recent history"
            );
        }
    }
}

/// Set YOLO as an ordinary form edit, then use the main action after the next frame knows the selection.
fn choose_yolo(_: &Entity<SessionList>, state: &SceneState, window: &mut Window, cx: &mut App) {
    assert!(click_control(window, cx, "launcher-permissions-YOLO"));
    state.after.set(Some(press_launch_for_yolo));
}

fn press_launch_for_yolo(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    assert!(click_control(window, cx, "launcher-launch"));
    state.after.set(Some(answer_yolo_launch));
}

/// The launcher hides while the create is pending. The fake must give the refusal the real helm sends to
/// bring it back with its confirmation, and to let the operation release its handle to the launcher.
fn answer_yolo_launch(_: &Entity<SessionList>, state: &SceneState, _: &mut Window, _: &mut App) {
    let mut keep = state.keep.borrow_mut();
    let helm = keep
        .iter_mut()
        .find_map(|kept| kept.downcast_mut::<FakeHelm>())
        .expect("the YOLO scene keeps its fake helm");
    let mut answered = false;
    for command in helm.commands() {
        if command.path == "/api/sessions" {
            let body = command.body.expect("the create sent its launch choices");
            assert_eq!(body["launch"]["permissions"], json!("yolo"));
            assert_eq!(body["confirm_yolo"], json!(false));
            helm.reply_marked(
                command.id,
                409,
                vec![HelmMarker::YoloConfirmation],
                Some(json!("YOLO launches on this host need confirmation")),
            );
            answered = true;
        } else {
            helm.reply(command.id, 200, None);
        }
    }
    state.after.set(Some(if answered {
        wait_for_yolo_question
    } else {
        answer_yolo_launch
    }));
}

/// Require the real answers in the pinned question before capturing the launcher.
fn wait_for_yolo_question(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    _: &mut App,
) {
    let nodes = window.driver_nodes();
    let shown = |id| {
        nodes
            .iter()
            .any(|node| node.element_id.as_deref() == Some(id))
    };
    if !shown("prompt-confirm") || !shown("launcher-command-pane") {
        state.after.set(Some(wait_for_yolo_question));
    }
}

/// Use the drawn Host control's bounds so the golden exercises the menu's regular anchor and focus.
fn open_host_menu(_: &Entity<SessionList>, state: &SceneState, window: &mut Window, cx: &mut App) {
    if click_control(window, cx, "launcher-host") {
        state.after.set(Some(wait_for_environment_rows));
    } else {
        state.after.set(Some(open_host_menu));
    }
}

/// Both environment options must actually be in the rendered menu before either scenario advances.
fn wait_for_environment_rows(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    let sandbox_shown = window
        .driver_nodes()
        .iter()
        .any(|node| node.element_id.as_deref() == Some("launcher-host-option-New sandbox"));
    let container_shown = window
        .driver_nodes()
        .iter()
        .any(|node| node.element_id.as_deref() == Some("launcher-host-option-New container"));
    if !sandbox_shown || !container_shown {
        state.after.set(Some(wait_for_environment_rows));
    } else if launcher_scene(state) == LauncherScene::Container {
        assert!(click_control(
            window,
            cx,
            "launcher-host-option-New container"
        ));
        state.after.set(Some(wait_for_container));
    }
}

/// A stable frame from before the popover closed would miss the fields the scene needs to show.
fn wait_for_container(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    _: &mut App,
) {
    if let Some(recipe) = window
        .driver_nodes()
        .iter()
        .find(|node| node.element_id.as_deref() == Some("launcher-recipe"))
    {
        assert_eq!(
            recipe.value.as_deref(),
            Some("Development"),
            "the fixture recipe was available when New container was picked"
        );
    } else {
        state.after.set(Some(wait_for_container));
    }
}

fn click_control(window: &mut Window, cx: &mut App, id: &str) -> bool {
    let Some(center) = window
        .driver_nodes()
        .iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .map(|node| node.bounds.center())
    else {
        return false;
    };
    click_before_redraw(center, window, cx);
    // Park the pointer away from the form so its delayed hover help does not cover the controls being captured.
    window.dispatch_event(
        gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
            position: point(px(1.), px(1.)),
            modifiers: gpui::Modifiers::default(),
            pressed_button: None,
        }),
        cx,
    );
    true
}
