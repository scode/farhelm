//! The real session launcher over a fake helm. Its drawing and focus come from the regular New entry point.

use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::LaunchHarness;
use farhelm_proto::tensorlake::SshKeyStatus;
use farhelm_proto::ui_channel::{Collection, CollectionState, KeyedEntry};
use gpui::{App, Entity, Window, point, px};
use serde_json::json;

use super::{SceneState, fleet, session_list_scene_holding, session_list_then};
use crate::launcher::entry;
use crate::launcher::environment_tests::{account, recipe_json};
use crate::session_list::SessionList;
use crate::test_support::{click_before_redraw, meta};

/// How far a destination scene takes the user through the real launcher.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DestinationScene {
    Form,
    HostMenu,
    Container,
}

/// Show New with a chosen agent, so the screenshot covers the model, effort and permission controls and the prompt.
pub(super) fn launcher_new_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, DestinationScene::Form, window, cx);
}

/// Open the host menu once both new-environment offers can be drawn from the fixture.
pub(super) fn launcher_host_menu_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, DestinationScene::HostMenu, window, cx);
}

/// Take New container through the host menu so the capture shows what replaces an editable folder.
pub(super) fn launcher_container_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, DestinationScene::Container, window, cx);
}

fn setup(state: &SceneState, scene: DestinationScene, window: &mut Window, cx: &mut App) {
    state.keep.borrow_mut().push(Box::new(scene));
    window.set_driver_enabled(true);
    if scene == DestinationScene::Form {
        session_list_then(state, open_new_with_claude, cx);
    } else {
        // Placement picks use the recipe catalog already displayed at the time of the click. Seed it in the first
        // snapshot so a separate asynchronous push cannot arrive after that pick and leave the recipe unset.
        let rows = fleet();
        session_list_scene_holding(
            state,
            &rows,
            rows.first().map(|row| row.key),
            vec![CollectionState {
                collection: Collection::Recipes,
                meta: meta(&["Development"]),
                entries: vec![KeyedEntry {
                    key: "Development".into(),
                    value: recipe_json("Development", json!({ "kind": "tensorlake_ubuntu" })),
                }],
            }],
            cx,
        );
        state.after.set(Some(open_new_with_claude));
    }
}

fn destination_scene(state: &SceneState) -> DestinationScene {
    state
        .keep
        .borrow()
        .iter()
        .find_map(|item| item.downcast_ref::<DestinationScene>().copied())
        .expect("launcher scene setup named the interaction")
}

/// Use the live session list's prefill once the fake helm's snapshot has arrived.
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
    draft.choose_harness(LaunchHarness::Claude, None, None);
    assert!(
        entry::open_with(draft, None, window, cx),
        "New opens once the helm is connected"
    );
    state.after.set(Some(answer_new_reads));
}

/// Reply in send order and wait for the model count from that catalog in the drawn form. A stable empty frame or a
/// frame drawn before the catalog reply cannot stand in for the launcher this scene means to show.
fn answer_new_reads(_: &Entity<SessionList>, state: &SceneState, window: &mut Window, _: &mut App) {
    let scene = destination_scene(state);
    let mut keep = state.keep.borrow_mut();
    let helm = keep
        .iter_mut()
        .find_map(|kept| kept.downcast_mut::<FakeHelm>())
        .expect("the New scene keeps its fake helm");
    for command in helm.commands() {
        let body = if command.path == "/api/launch-catalog" {
            Some(
                json!([{ "id": "sonnet", "harness": "claude", "efforts": ["low", "medium", "high"] }]),
            )
        } else if command.path == "/api/tensorlake" && scene != DestinationScene::Form {
            Some(json!(account(true, SshKeyStatus::NotCreated)))
        } else if command.path.starts_with("/api/launch-history") {
            Some(json!({ "launches": [], "folders": [], "checkout_config_revision": 1 }))
        } else {
            None
        };
        helm.reply(command.id, 200, body);
    }
    let catalog_drawn = window.driver_nodes().iter().any(|node| {
        node.element_id.as_deref() == Some("launcher-model-hint")
            && node.label.as_deref() == Some("1 for Claude")
    });
    if !catalog_drawn {
        state.after.set(Some(answer_new_reads));
    } else if scene != DestinationScene::Form {
        state.after.set(Some(open_host_menu));
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
    } else if destination_scene(state) == DestinationScene::Container {
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
