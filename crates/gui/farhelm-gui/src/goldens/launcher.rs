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
use crate::session_list::host_rail::tests::texts_within;
use crate::session_list::tests::{preferences_state, session};
use crate::test_support::{click_before_redraw, meta, withdraw_presented_choice};

/// Show the temporary capsule with the launcher hidden and the fake's Resume still outstanding.
pub(super) fn launcher_wake_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    suspended_launcher_setup(state, window, cx, open_wake_launcher);
    state
        .after_capture
        .set(Some(super::session_list::disconnect_pending_launch));
}

/// Show a complete managed checkout on a suspended host before the press, while its allocation is still promised.
pub(super) fn launcher_waking_checkout_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    suspended_launcher_setup(state, window, cx, open_waking_checkout);
}

/// The same pinned registry and drawing setup for the two launcher scenes that target the suspended sandbox.
fn suspended_launcher_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
    open: super::AfterDrawn,
) {
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
    state.after.set(Some(open));
}

/// Use the same visible launcher press that hides the form and starts Resume.
fn open_wake_launcher(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(mut draft) = suspended_launch_draft(cx) else {
        state.after.set(Some(open_wake_launcher));
        return;
    };
    draft.edit_folder("/home/user/work".into());
    let mark = crate::focus::Mark::take(cx);
    assert!(crate::launcher::view::open(draft, &mark, window, cx).is_some());
    state.after.set(Some(press_wake_launcher));
}

/// Wait for the registry before drawing the allocation notice. No preview is issued while this host is suspended.
fn open_waking_checkout(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(mut draft) = suspended_launch_draft(cx) else {
        state.after.set(Some(open_waking_checkout));
        return;
    };
    draft.choose_repository(farhelm_proto::GithubRepo {
        owner: "acme".into(),
        name: "bar".into(),
    });
    let mark = crate::focus::Mark::take(cx);
    assert!(crate::launcher::view::open(draft, &mark, window, cx).is_some());
}

/// A complete existing-host command draft, once the fake has published the chosen host's suspension.
fn suspended_launch_draft(cx: &App) -> Option<crate::launcher::draft::LaunchDraft> {
    use farhelm_proto::ui_channel::{HostStateView, HostView};
    let store = crate::session::current_store(cx)?;
    let host = store
        .read(cx)
        .collections()
        .get::<HostView>(&HostView::key(12))
        .expect("the fixture lists its suspended host");
    assert!(matches!(host.state, HostStateView::Suspended { .. }));
    let mut draft = crate::launcher::launch_tests::command_draft();
    draft.choose_host(12, "~".into());
    Some(draft)
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
    ManagedCheckout,
    HostMenu,
    Container,
    ContainerCheckoutRefusal,
    RememberedWrapper,
    CustomAgent,
    ShellWrapper,
    ShellWrapperNote,
    RepositoryRefusal,
    StaleChoice,
    Results,
    IncompleteTemplate,
    RecentFill,
    TemplateSaveAfterUndo,
    Restart,
    HeldHostHelp,
    ShellHelp,
    Replace,
    Narrow,
    Yolo,
    HookSetup,
}

/// The held address truncates this name. The tooltip needs to show its distinguishing suffix.
const HELD_HOST_NAME: &str =
    "Integration build host for the production migration and release checks";

/// Show New with a chosen agent, so the screenshot covers the model, effort and permission controls and the prompt.
pub(super) fn launcher_new_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, LauncherScene::Form, window, cx);
}

/// Use the same accepted destination in both widths so the capture differs only by the responsive layout.
pub(super) fn launcher_managed_checkout_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    setup(state, LauncherScene::ManagedCheckout, window, cx);
}

/// Open the host menu once both new-environment offers can be drawn from the fixture.
pub(super) fn launcher_host_menu_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, LauncherScene::HostMenu, window, cx);
}

/// Take New container through the host menu so the capture shows what replaces an editable folder.
pub(super) fn launcher_container_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, LauncherScene::Container, window, cx);
}

/// Keep the invalid name and selected container checkout available to correct after the helm's refusal.
pub(super) fn launcher_container_checkout_refusal_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    setup(state, LauncherScene::ContainerCheckoutRefusal, window, cx);
}

/// Show a wrapper brought in by the actual harness choice, without opening More options to see it.
pub(super) fn launcher_more_options_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, LauncherScene::RememberedWrapper, window, cx);
}

/// Draw the registered chips and literal model and effort inputs without requiring a vendor or a launch.
pub(super) fn launcher_custom_agent_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, LauncherScene::CustomAgent, window, cx);
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

/// A partial shortcut opens New and points back to the form above the banner.
pub(super) fn launcher_incomplete_template_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    setup(state, LauncherScene::IncompleteTemplate, window, cx);
}

/// The typed recent in Replace with fills the draft. Its permission stays visible on the selected row.
pub(super) fn launcher_recent_fill_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, LauncherScene::RecentFill, window, cx);
}

/// Restart keeps the recorded agent and destination fixed. Its command pane shows why their rows cannot apply.
pub(super) fn launcher_restart_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, LauncherScene::Restart, window, cx);
}

/// A held host's full name must stay legible in help even when its address segment clips it.
pub(super) fn launcher_held_host_help_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, LauncherScene::HeldHostHelp, window, cx);
}

/// The shell chip's help identifies it as a shell at the point where the user chooses it.
pub(super) fn launcher_shell_help_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, LauncherScene::ShellHelp, window, cx);
}

/// Show the restored source while Save as template holds the recovered draft.
pub(super) fn launcher_template_save_after_undo_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    setup(state, LauncherScene::TemplateSaveAfterUndo, window, cx);
}

/// Keep the source warning and held host visible beside the same form and command pane used by New.
pub(super) fn launcher_replace_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, LauncherScene::Replace, window, cx);
}

/// Open a template query at the minimum width so the screenshot proves the popover fallback is reachable.
pub(super) fn launcher_narrow_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, LauncherScene::Narrow, window, cx);
}

/// Show the warning at the narrow width, where the old results popover would otherwise cover it.
pub(super) fn launcher_stale_choice_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, LauncherScene::StaleChoice, window, cx);
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
    if matches!(scene, LauncherScene::Restart | LauncherScene::HeldHostHelp) {
        // The stored resume command makes Restart with available even before a conversation has been captured.
        let host_name = if scene == LauncherScene::HeldHostHelp {
            HELD_HOST_NAME
        } else {
            "Local (this machine)"
        };
        rows[0] = session("a")
            .host(1)
            .host_name(host_name)
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
    let mut extra = vec![launcher_templates(scene)];
    if scene == LauncherScene::CustomAgent {
        extra.push(super::custom_agent_state());
    }
    if matches!(
        scene,
        LauncherScene::HostMenu
            | LauncherScene::Container
            | LauncherScene::ContainerCheckoutRefusal
    ) {
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
fn launcher_templates(scene: LauncherScene) -> CollectionState {
    let undo = scene == LauncherScene::TemplateSaveAfterUndo;
    let names = ["Review a PR", "Bench run", "Scratch shell"];
    let mut fields = [
        TemplateFields {
            agent: Some(LaunchHarness::Claude),
            effort: Some(Some(LaunchEffort::High.into())),
            permissions: undo.then_some(Some(LaunchPermission::Guarded)),
            ..Default::default()
        },
        TemplateFields {
            agent: Some(if undo {
                LaunchHarness::OpenCode
            } else {
                LaunchHarness::Codex
            }),
            permissions: (!undo).then_some(Some(LaunchPermission::Yolo)),
            ..Default::default()
        },
        TemplateFields {
            agent: Some(LaunchHarness::Bash),
            ..Default::default()
        },
    ];
    if scene == LauncherScene::IncompleteTemplate {
        fields[0] = TemplateFields {
            name: Some("Review the API".into()),
            ..Default::default()
        };
    }
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
    if scene == LauncherScene::IncompleteTemplate {
        use crate::templates::dispatch::{Origin, Reference, dispatch};
        dispatch(Origin::NumberKey, &[Reference::Key("1".into())], window, cx);
        state.after.set(Some(answer_new_reads));
        return;
    }
    if matches!(
        scene,
        LauncherScene::Restart
            | LauncherScene::HeldHostHelp
            | LauncherScene::Replace
            | LauncherScene::RecentFill
    ) {
        let open = if matches!(scene, LauncherScene::Restart | LauncherScene::HeldHostHelp) {
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
    if scene == LauncherScene::CustomAgent {
        let fixture = super::custom_agent();
        draft.choose_custom_agent(&fixture, None, None);
        draft.agent.model = Some("fast-v2".into());
        draft.agent.custom_effort = "deep_work".into();
    } else if matches!(
        scene,
        LauncherScene::ShellWrapper | LauncherScene::ShellWrapperNote
    ) {
        draft.choose_harness(LaunchHarness::Bash, None, None);
    } else if !remembered {
        draft.choose_harness(LaunchHarness::Claude, None, None);
    }
    if scene == LauncherScene::ManagedCheckout {
        draft.choose_host(1, "~".into());
        draft.choose_repository(farhelm_proto::parse_github_repo("acme/widget").unwrap());
        draft.edit_name("widget-3".into());
    } else if scene == LauncherScene::ContainerCheckoutRefusal {
        draft.seed_target(farhelm_proto::LaunchTarget::Environment {
            recipe: "Development".into(),
            place: farhelm_proto::Place::PodmanHost(1),
        });
        draft.choose_repository(farhelm_proto::parse_github_repo("acme/widget").unwrap());
        draft.edit_name("!!!".into());
    }
    assert!(
        entry::open_with(draft, None, window, cx),
        "New opens once the helm is connected"
    );
    state.after.set(Some(match scene {
        LauncherScene::RememberedWrapper => pick_claude,
        LauncherScene::CustomAgent => open_shell_wrapper,
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
    } else if launcher_scene(state) == LauncherScene::CustomAgent {
        state.after.set(Some(reveal_custom_wrapper));
    }
}

/// The expanded options extend beyond the pinned footer. Scroll just enough to show the wrapper and the named start.
fn reveal_custom_wrapper(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    let nodes = window.driver_nodes();
    let bounds = |id| {
        nodes
            .iter()
            .find(|node| node.element_id.as_deref() == Some(id))
            .map(|node| node.bounds)
    };
    let (Some(body), Some(wrapper)) = (bounds("launcher-body"), bounds("launcher-wrapper-row"))
    else {
        state.after.set(Some(reveal_custom_wrapper));
        return;
    };
    let beyond = wrapper.bottom() + px(12.) - body.bottom();
    if beyond > px(1.) {
        window.dispatch_event(
            gpui::PlatformInput::Scroll(gpui::ScrollEvent {
                position: point(body.left() + px(8.), body.center().y),
                delta: gpui::ScrollDelta::Pixels(point(px(0.), -beyond)),
                ..Default::default()
            }),
            cx,
        );
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

/// The ordinary local host's preview for the exact typed name. Both window widths show this same checkout.
const CHECKOUT_PATH: &str = "/home/user/src/checkouts/widget-3";

/// Staging can take several frames. The preview's independent reply cannot stand in for the debounced discovery read.
struct CheckoutDiscoveryAnswered;

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
    let mut checkout_refused = false;
    for command in helm.commands() {
        if scene == LauncherScene::ContainerCheckoutRefusal
            && command.path == "/api/environment-sessions"
        {
            assert!(!checkout_refused, "one press sends one environment create");
            let body = command.body.expect("the launch names its checkout");
            assert_eq!(body["request"]["github_checkout"]["title"], json!("!!!"));
            let reason = farhelm_proto::github_checkout::NameError::EmptySlug.to_string();
            helm.reply(command.id, 400, Some(json!(reason)));
            checkout_refused = true;
            continue;
        }
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
            Some(launcher_history(scene))
        } else if scene == LauncherScene::ManagedCheckout
            && command.path == "/api/github-checkout-preview"
        {
            assert_eq!(
                command.body,
                Some(
                    json!({"host": 1, "expected_incarnation": 4, "repo": "acme/widget", "title": "widget-3"})
                )
            );
            Some(json!({
                "canonical_root": "/home/user/src/checkouts", "basename": "widget-3", "cwd": CHECKOUT_PATH,
                "config_revision": 3, "host": "1", "incarnation": 4, "installation_identity": "identity1"
            }))
        } else if scene == LauncherScene::ManagedCheckout
            && command.path == "/api/github-repositories"
        {
            assert_eq!(
                command.body,
                Some(json!({"host": 1, "expected_incarnation": 4, "query": "acme/widget"}))
            );
            repository_answered = true;
            Some(json!({
                "host": "1", "incarnation": 4, "installation_identity": "identity1",
                "repos": [{"owner": "acme", "name": "widget"}], "truncated": false, "needs_checkout_root": false,
            }))
        } else {
            None
        };
        helm.reply(command.id, 200, body);
    }
    if checkout_refused {
        // Finishing the drained batch keeps any later reads answerable.
        state.after.set(Some(wait_for_container_checkout_refusal));
        return;
    }
    if scene == LauncherScene::IncompleteTemplate {
        if !window.driver_nodes().iter().any(|node| {
            node.element_id.as_deref() == Some("launcher-banner") && !node.visible_bounds.is_empty()
        }) {
            state.after.set(Some(answer_new_reads));
        }
        return;
    }
    let catalog_drawn = window.driver_nodes().iter().any(|node| {
        node.element_id.as_deref() == Some("launcher-model-hint")
            && node.label.as_deref() == Some("1 for Claude")
    });
    if scene == LauncherScene::ManagedCheckout {
        if repository_answered {
            keep.push(Box::new(CheckoutDiscoveryAnswered));
            // Allow a subsequent frame to consume the discovery reply, even if the preview has already drawn.
            state.after.set(Some(answer_new_reads));
            return;
        }
        let discovery_answered = keep
            .iter()
            .any(|item| item.is::<CheckoutDiscoveryAnswered>());
        if !discovery_answered {
            state
                .advance_clock
                .set(Some(std::time::Duration::from_millis(160)));
        }
        let nodes = window.driver_nodes();
        let preview_drawn = nodes.iter().any(|node| {
            node.element_id.as_deref() == Some("launcher-preview-path")
                && node.label.as_deref() == Some(CHECKOUT_PATH)
                && !node.visible_bounds.is_empty()
        });
        if !catalog_drawn || !discovery_answered || !preview_drawn {
            state.after.set(Some(answer_new_reads));
        } else {
            // Kit only exposes the editable value when accessibility is enabled. Check the painted text underneath
            // each input's wrapper so the screenshot itself proves the exact values are visible.
            for (id, expected) in [
                ("launcher-name", "widget-3"),
                ("launcher-repository", "acme/widget"),
            ] {
                let lines = texts_within(nodes, id);
                assert!(
                    lines
                        .iter()
                        .any(|line| line.text.as_ref() == expected
                            && !line.visible_bounds.is_empty()),
                    "{id} shows {expected:?}; recorded lines: {lines:?}"
                );
            }
        }
        return;
    }
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
        LauncherScene::Narrow | LauncherScene::RepositoryRefusal | LauncherScene::StaleChoice
    );
    if !catalog_drawn || (shows_templates && !catalog_template) {
        state.after.set(Some(answer_new_reads));
    } else if scene == LauncherScene::ContainerCheckoutRefusal {
        assert!(
            !nodes
                .iter()
                .any(|node| node.element_id.as_deref() == Some("launcher-refusal"))
        );
        assert!(click_control(window, cx, "launcher-launch"));
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
    } else if scene == LauncherScene::TemplateSaveAfterUndo {
        assert!(click_control(window, cx, "launcher-template-Review a PR"));
        state.after.set(Some(escalate_template));
    } else if matches!(
        scene,
        LauncherScene::Narrow | LauncherScene::RepositoryRefusal | LauncherScene::StaleChoice
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
        } else if scene == LauncherScene::RecentFill {
            state.after.set(Some(type_launcher_query));
        } else if matches!(scene, LauncherScene::Restart | LauncherScene::HeldHostHelp) {
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
            if scene == LauncherScene::HeldHostHelp {
                assert!(nodes.iter().any(|node| {
                    node.element_id.as_deref() == Some("launcher-host-name")
                        && node.label.as_deref() == Some(HELD_HOST_NAME)
                }));
                hover_launcher_help(state, "launcher-host-held", window, cx);
            }
        } else if scene == LauncherScene::ShellHelp {
            hover_launcher_help(state, "launcher-shells-Bash", window, cx);
        }
    }
}

/// The restored checkout must keep the host and repository close enough to
/// edit together. A fixed width basis used as a height once pushed them apart.
fn wait_for_container_checkout_refusal(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    _: &mut App,
) {
    let nodes = window.driver_nodes();
    let find = |id| {
        nodes
            .iter()
            .find(|node| node.element_id.as_deref() == Some(id))
    };
    let Some(alert) = find("launcher-refusal").filter(|alert| !alert.visible_bounds.is_empty())
    else {
        state.after.set(Some(wait_for_container_checkout_refusal));
        return;
    };
    let reason = farhelm_proto::github_checkout::NameError::EmptySlug;
    assert_eq!(
        alert.label.as_deref(),
        Some(format!("Not launched. {reason}").as_str())
    );
    let host = find("launcher-container-host-row").expect("the container host is still shown");
    let repo = find("launcher-repository-row").expect("the managed checkout is still shown");
    assert!(!host.visible_bounds.is_empty() && !repo.visible_bounds.is_empty());
    let gap = repo.bounds.top() - host.bounds.bottom();
    assert!(
        gap >= px(0.) && gap <= px(72.),
        "the place note and row gap fit within 72 px, got {gap:?}"
    );
}

/// Start the permission-changing fill only after the previous template named itself on the form.
fn escalate_template(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    assert!(template_source_shown(
        window,
        "From a template: Review a PR"
    ));
    assert!(click_control(window, cx, "launcher-template-Bench run"));
    state.after.set(Some(undo_template));
}

/// Undo is offered by the actual banner, and the switched template must have taken over the source first.
fn undo_template(_: &Entity<SessionList>, state: &SceneState, window: &mut Window, cx: &mut App) {
    assert!(template_source_shown(window, "From a template: Bench run"));
    assert!(click_control(window, cx, "launcher-escalation-undo"));
    state.after.set(Some(save_restored_template));
}

/// Open Save as template on the restored form, then try to take a different template behind it.
fn save_restored_template(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    assert!(template_source_shown(
        window,
        "From a template: Review a PR"
    ));
    assert!(click_control(window, cx, "launcher-save-template"));
    state.after.set(Some(pick_behind_template_save));
}

fn pick_behind_template_save(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    assert!(window.driver_nodes().iter().any(|node| {
        node.element_id.as_deref() == Some("save-template-name") && !node.visible_bounds.is_empty()
    }));
    assert!(click_control(window, cx, "launcher-template-Bench run"));
    state.after.set(Some(|_, _, window, _| {
        assert!(template_source_shown(
            window,
            "From a template: Review a PR"
        ));
    }));
}

/// Check the rendered source itself so a capture of the form on the wrong template cannot pass.
fn template_source_shown(window: &Window, label: &str) -> bool {
    window.driver_nodes().iter().any(|node| {
        node.element_id.as_deref() == Some("launcher-provenance-label")
            && node.label.as_deref() == Some(label)
            && !node.visible_bounds.is_empty()
    })
}

/// Dispatch the hover before arming the next draw. Its deferred update must replace the warming tooltip first.
fn hover_launcher_help(state: &SceneState, id: &str, window: &mut Window, cx: &mut App) {
    let bounds = window
        .driver_nodes()
        .iter()
        .find(|node| node.element_id.as_deref() == Some(id) && !node.visible_bounds.is_empty())
        .unwrap_or_else(|| panic!("{id} is visible before the hover"))
        .visible_bounds;
    let at = point(bounds.right() - bounds.size.width / 8., bounds.center().y);
    super::rest_pointer_at(state, at, window, cx);
    state.after.set(Some(check_launcher_help));
    window.refresh();
}

/// Capture only after the addressed control has replaced the fixture's warming tooltip.
fn check_launcher_help(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    let expected = if launcher_scene(state) == LauncherScene::HeldHostHelp {
        HELD_HOST_NAME
    } else {
        "Use Bash as the session shell"
    };
    let shown = crate::layers::shown_tooltip(window, cx).expect("the launcher tooltip is showing");
    assert_eq!(shown.text.as_ref(), expected);
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
fn launcher_history(scene: LauncherScene) -> serde_json::Value {
    let yolo_folder = if scene == LauncherScene::RecentFill {
        "/home/user/src/permission-check"
    } else {
        "/home/user/src/api"
    };
    let entry = |cwd: &str, selection| {
        json!({
            "host": 1, "cwd": cwd, "canonical_cwd": cwd, "github_repo": null,
            "selection": selection, "created_at": 1, "creation_seq": null,
        })
    };
    json!({
        "launches": [
            entry("/home/user/src/api", json!({"harness": "claude", "permissions": "guarded"})),
            entry(yolo_folder, json!({"harness": "claude", "permissions": "yolo"})),
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
        LauncherScene::StaleChoice => ("tl:Bench", click_withdrawn_template),
        LauncherScene::RepositoryRefusal => ("gh:acme", answer_new_reads),
        LauncherScene::RecentFill => ("recent:permission-check", wait_for_recent_fill),
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

/// Dispatch the click against the template's last frame after its removal has reached the store.
fn click_withdrawn_template(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    let ready = window.driver_nodes().iter().any(|node| {
        node.element_id.as_deref() == Some("launcher-search-option-0")
            && node.label.as_deref() == Some("Template: Bench run")
            && !node.visible_bounds.is_empty()
    });
    if !ready {
        state.after.set(Some(click_withdrawn_template));
        return;
    }
    withdraw_presented_choice::<LaunchTemplate>("Bench run", cx);
    assert!(click_control(window, cx, "launcher-search-option-0"));
    state.after.set(Some(wait_for_stale_choice_warning));
}

/// Capture after both the refusal and the retained query have painted, with the form uncovered.
fn wait_for_stale_choice_warning(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    _: &mut App,
) {
    let nodes = window.driver_nodes();
    let visible_warning = nodes.iter().any(|node| {
        node.element_id.as_deref() == Some("launcher-search-refusal")
            && node.label.as_deref()
                == Some("That choice is no longer available; choose a current result")
            && !node.visible_bounds.is_empty()
    });
    if !visible_warning {
        state.after.set(Some(wait_for_stale_choice_warning));
        return;
    }
    assert!(!nodes.iter().any(|node| {
        matches!(
            node.element_id.as_deref(),
            Some("launcher-command-pane" | "launcher-search-results")
        )
    }));
    assert!(
        texts_within(nodes, "launcher-search-row")
            .iter()
            .any(|line| line.text.as_ref() == "tl:Bench" && !line.visible_bounds.is_empty()),
        "the original query remains visible"
    );
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

/// Only a selected YOLO setup with its fill action in view proves this scene's wording and contrast.
fn wait_for_recent_fill(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    _: &mut App,
) {
    let nodes = window.driver_nodes();
    if let Some(cap) = nodes.iter().find(|node| {
        node.element_id.as_deref() == Some("launcher-recent-enter")
            && !node.visible_bounds.is_empty()
    }) {
        assert_eq!(cap.label.as_deref(), Some("⏎ Fill"));
        assert!(
            texts_within(nodes, "launcher-search-option-0")
                .iter()
                .any(|line| { line.text.as_ref() == "YOLO" && !line.visible_bounds.is_empty() })
        );
    } else {
        state.after.set(Some(wait_for_recent_fill));
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
        assert!(window.driver_nodes().iter().any(|node| {
            node.element_id.as_deref() == Some("launcher-environment-notice")
                && node.label.as_deref()
                    == Some("Deleting the last session removes this container and its files.")
                && !node.visible_bounds.is_empty()
        }));
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
