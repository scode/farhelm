//! The real session launcher over a fake helm. Its drawing and focus come from the regular New entry point.

use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::LaunchHarness;
use gpui::{App, Entity, Window};
use serde_json::json;

use super::{SceneState, session_list_then};
use crate::launcher::entry;
use crate::session_list::SessionList;

/// Show New with a chosen agent, so the screenshot covers the model, effort and permission controls and the prompt.
pub(super) fn launcher_new_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    window.set_driver_enabled(true);
    session_list_then(state, open_new_with_claude, cx);
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
    }
}
