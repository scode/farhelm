//! Golden scenes for the agent approval card. The scene list itself, `SCENES` with each scene's name and size, stays in
//! `mod.rs`.

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};
use std::rc::Rc;

use super::SceneState;

/// An agent's approval card as the old app's reference screen showed it: an inherited spawn's command launch from this
/// machine, with the narrow and fleet-wide grants offered, at the card's full width. Pins the badge, the From line,
/// the facts with the Permissions chip, a box and the answer bar with Deny apart.
pub(super) fn approval_card(_: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    approval_scene(approval_spawn(), 660., cx)
}

/// The action beside a YOLO chip matters. Plain Restart uses the session's saved command, while a prompt goes to an
/// already running agent. The prompt fixture deliberately shows a bypass observed on the agent's status line.
pub(super) fn approval_yolo_actions(_: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    use farhelm_proto::approvals::ApprovalAction;
    use serde_json::json;

    let mut restart = approval_spawn();
    let mut prompt = restart.clone();
    let mut target = restart.session.clone();
    target.id = "7f8c2b1e-6d0e-4c51-9a8e-1e2f3a4b5c6d:review-notes".to_owned();
    target.title = Some("Review notes".to_owned());
    let launch = serde_json::from_value(json!({
        "kind": "agent",
        "selection": {"harness": "codex", "permissions": "yolo"},
        "start": ["codex", "--yolo", "{farhelm_args}"],
        "resume": ["codex", "--yolo", "resume", "{conversation}", "{farhelm_args}"],
    }))
    .expect("saved YOLO launch fixture");
    restart.action = ApprovalAction::Restart {
        target: target.clone(),
        stop_if_running: true,
        launch: Some(launch),
    };
    let ApprovalAction::Launch { launch, .. } = prompt.action else {
        unreachable!("the approval scenes start from a launch");
    };
    prompt.id = "golden-prompt".to_owned();
    prompt.own_host_session_action = false;
    target.id = "7f8c2b1e-6d0e-4c51-9a8e-1e2f3a4b5c6d:draft-update".to_owned();
    target.title = Some("Draft update".to_owned());
    prompt.action = ApprovalAction::Prompt {
        target,
        launch,
        screen_yolo: true,
        attached: false,
        prompt: "Summarize the draft update.".to_owned(),
    };
    div()
        .flex()
        .child(approval_scene(restart, 520., cx))
        .child(approval_scene(prompt, 520., cx))
        .into_any_element()
}

/// A proposed shell template shows the incompatible mode and flag words that the write would store.
pub(super) fn shell_template_approval(_: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    let mut approval = approval_spawn();
    approval.action = farhelm_proto::approvals::ApprovalAction::TemplateWrite {
        name: "shell-profile".into(),
        replaces_existing: false,
        fields: super::templates::shell_template_fields(),
        host_name: None,
    };
    approval_scene(approval, 660., cx)
}

/// The card shows the stored permission even when the proposed agent cannot launch with that mode.
pub(super) fn unavailable_template_permission(
    _: &SceneState,
    _: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let mut approval = approval_spawn();
    approval.action = farhelm_proto::approvals::ApprovalAction::TemplateWrite {
        name: "review".into(),
        replaces_existing: false,
        fields: farhelm_proto::launcher::TemplateFields {
            agent: Some(farhelm_proto::LaunchHarness::Claude),
            permissions: Some(Some(farhelm_proto::LaunchPermission::Approve)),
            ..Default::default()
        },
        host_name: None,
    };
    approval_scene(approval, 660., cx)
}

/// The same spawn at the old minimum window's card width, from a host with a long name: labels stacked over their
/// values, the From line wrapping with its ID and Copy ID under the title, and the answer buttons wrapping onto lines of
/// their own, each "Always allow" button shrinking and wrapping its host name inside the card rather than running past
/// it (the old stylesheet: "the button itself must shrink and wrap so the narrow pane never needs sideways
/// scrolling").
pub(super) fn approval_card_narrow(_: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    let host = "build-workstation-west-02.example";
    let mut approval = approval_spawn();
    approval.host_name = host.to_owned();
    approval.session.host_name = Some(host.to_owned());
    let farhelm_proto::approvals::ApprovalAction::Launch { host_name, .. } = &mut approval.action
    else {
        unreachable!("the approval scenes start from a launch");
    };
    *host_name = host.to_owned();
    approval_scene(approval, 284., cx)
}

/// The inherited spawn the approval card scenes start from.
fn approval_spawn() -> farhelm_proto::approvals::PendingApproval {
    use farhelm_proto::approvals::{ApprovalAction, ApprovalSession, LaunchVerb, PendingApproval};
    PendingApproval {
        id: "golden".to_owned(),
        host_id: 1,
        host_name: "this machine".to_owned(),
        session: ApprovalSession {
            id: "7f8c2b1e-6d0e-4c51-9a8e-1e2f3a4b5c6d:a75bff30-1c2d-4e5f-8a9b-0c1d2e3f27a4"
                .to_owned(),
            title: Some("Review agent request".to_owned()),
            host_name: Some("this machine".to_owned()),
            launch_kind: None,
        },
        action: ApprovalAction::Launch {
            verb: LaunchVerb::SpawnInherited,
            host_name: "this machine".to_owned(),
            cwd: "/tmp".to_owned(),
            repo: None,
            title: None,
            launch: farhelm_proto::SessionLaunch::Command(farhelm_proto::CommandLaunch {
                custom_agent: None,
                command: "claude {farhelm_args}".to_owned(),
                yolo: false,
                agent: Some(farhelm_proto::LaunchHarness::Claude),
                resume: None,
            }),
            source: None,
        },
        own_host_session_action: true,
        expires_at_ms: 1,
    }
}

/// `approval` drawn as an expanded card `width` pixels wide, one request waiting, nothing in flight.
fn approval_scene(
    approval: farhelm_proto::approvals::PendingApproval,
    width: f32,
    cx: &mut App,
) -> AnyElement {
    use crate::approvals::card::{CardState, card};
    use crate::approvals::model::{Card, TunnelOffer};
    let model = Card::of(&approval, TunnelOffer::NotOffered);
    let state = CardState {
        width: px(width),
        waiting: 1,
        busy: false,
        refusal: None,
        measure: None,
        body_scroll: None,
    };
    div()
        .p(px(12.))
        .child(card(&model, &state, Rc::new(|_, _, _| {}), cx))
        .into_any_element()
}
