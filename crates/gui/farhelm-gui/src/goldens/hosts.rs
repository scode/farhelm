//! Golden scenes for host management: the Hosts panel, a host's menu and settings, the Add host dialog, and the
//! questions, refusals and progress of host setup, update, removal and uninstall. The scene list itself, `SCENES` with
//! each scene's name and size, stays in `mod.rs`.

use crate::focus::Mark;
use crate::host_text::HostText;
use crate::hosts::HostsPanel;
use crate::layers;
use crate::operations::Outcome;
use crate::theme;
use crate::{session, ticker};
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::HostView;
use gpui::{
    AnyElement, App, AppContext as _, Entity, IntoElement, ParentElement, Styled, Window, div,
    point, px,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::{Pending, SceneState, session_list_then, take_pending};

/// Host management shown over the fleet with `buildbox` chosen (the old `hosts-panel` reference with a host chosen, as
/// in `host-rail-host-selected`): the Hosts panel in place of the list's column, Hosts pressed at the rail's foot, and
/// the chosen avatar in the quieter look the rail's choice wears while the panel shows. The fake answers the host query
/// and the list's other commands in order before the panel is shown, so the rail draws the accepted choice.
pub(super) fn hosts_shown_with_a_host_chosen_setup(
    state: &SceneState,
    _: &mut Window,
    cx: &mut App,
) {
    session_list_then(
        state,
        |list, state, _, cx| {
            list.update(cx, |list, cx| list.choose_host(Some(2), cx));
            {
                let mut keep = state.keep.borrow_mut();
                if let Some(helm) = keep
                    .iter_mut()
                    .find_map(|kept| kept.downcast_mut::<FakeHelm>())
                {
                    for command in helm.commands() {
                        helm.reply(command.id, 204, None);
                    }
                }
            }
            list.update(cx, |list, cx| list.show_hosts(cx));
        },
        cx,
    );
}

/// The Add host dialog over the fake helm's reference hosts, as the host rail's Add host opens it (reference
/// `host-add-dialog`), with `dev@new-box` in the destination field, which holds focus. The opener is the scene's ring
/// handle. The fake and the connection are kept for the scene's life, like Settings'.
pub(super) fn host_add_dialog_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    host_add(state, window, cx, AddScene::Form);
}

/// Add host owns the setup question inline, with room for the destination, explanation and masked answer.
pub(super) fn host_add_sign_in_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    use farhelm_proto::ui_channel::{SshConnectionPurpose, SshQuestion, SshQuestionTarget};
    host_add(
        state,
        window,
        cx,
        AddScene::SignIn(SshQuestion {
            id: "golden-add-question".into(),
            attempt: "golden-add-check".into(),
            target: SshQuestionTarget::Draft {
                draft: "golden-add-check".into(),
            },
            purpose: SshConnectionPurpose::Setup,
            startup: false,
            prompt: "Verification code:".into(),
            notice: false,
            expires_at: i64::MAX,
        }),
    );
}

/// Which part of a probe belongs in an open Add form, with no network calls from the scene.
enum AddScene {
    Form,
    SignIn(farhelm_proto::ui_channel::SshQuestion),
    ProbeReply(&'static str, bool, Outcome),
}

fn host_add(state: &SceneState, window: &mut Window, cx: &mut App, scene: AddScene) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    let mut snapshot = vec![crate::hosts::tests::hosts(
        &crate::hosts::tests::reference_hosts(),
    )];
    if let AddScene::SignIn(question) = &scene {
        snapshot.push(crate::sign_in::tests::questions(std::slice::from_ref(
            question,
        )));
    }
    helm.connect(snapshot);
    // The dialog opens only over a live UI session, which the snapshot's connection makes after this setup returns:
    // open once the connection has one, and only once.
    let handle = window.window_handle();
    let opener = state.ring.clone();
    super::once_ui_session(state, helm, connection, cx, move |cx| {
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            crate::hosts::add::open(&opener, &mark, window, cx);
            let dialog = layers::last_opened::<crate::hosts::add::AddHostDialog>(cx);
            if let Some(dialog) = dialog {
                let destination = match &scene {
                    AddScene::ProbeReply(destination, ..) => *destination,
                    AddScene::Form | AddScene::SignIn(_) => "dev@new-box",
                };
                dialog
                    .read(cx)
                    .destination_input()
                    .set_text(destination, window, cx);
                dialog.update(cx, |dialog, cx| match &scene {
                    AddScene::Form => (),
                    AddScene::SignIn(question) => {
                        dialog.wait_for_setup_for_golden(&question.attempt, destination, window, cx)
                    }
                    AddScene::ProbeReply(_, tailscale, outcome) => {
                        dialog.show_probe_reply_for_golden(*tailscale, outcome.clone(), window, cx)
                    }
                });
            }
        });
    });
}

/// A missing Mac capability is a completed probe with a manual reason, which Add shows on the form.
pub(super) fn host_add_mac_clock(state: &SceneState, window: &mut Window, cx: &mut App) {
    let reply = farhelm_proto::provisioning::ProbeReply::<()>::Manual {
        reason: MAC_MISSING_DATE.into(),
    };
    let body = serde_json::to_value(reply).expect("the manual reply encodes");
    host_add(
        state,
        window,
        cx,
        AddScene::ProbeReply("dev@mac-box", false, Outcome::Done(Some(body))),
    );
}

/// Tailscale uses its own socket. The probe's connection error stays visible on the Tailscale form.
pub(super) fn host_add_tailscale_error(state: &SceneState, window: &mut Window, cx: &mut App) {
    let outcome = probe_refused(
        r#"the supervisor probe closed before hello completion with exit status 255: host stderr "Permission denied""#,
    );
    host_add(
        state,
        window,
        cx,
        AddScene::ProbeReply("dev@mesh-box", true, outcome),
    );
}

/// A locally interrupted probe cannot authorize setup, even when the remote wrapper reported an exit code.
pub(super) fn host_add_stopped_probe(state: &SceneState, window: &mut Window, cx: &mut App) {
    let outcome = probe_refused(
        r#"the supervisor probe closed before hello completion with signal: 15 (SIGTERM): host stderr "farhelm-probe-command-started-v1\n""#,
    );
    host_add(
        state,
        window,
        cx,
        AddScene::ProbeReply("dev@new-box", false, outcome),
    );
}

/// Both failed probes return gateway errors with the helm's diagnostic as the body.
fn probe_refused(words: &str) -> Outcome {
    Outcome::Refused(crate::operations::Refusal {
        words: HostText::from_host(words),
        status: 502,
        markers: Vec::new(),
    })
}

/// The Add host dialog as an approval card opens it for an agent's host add (no reference screen: the old app's dialog
/// for an agent was the same dialog with two unstyled description lists): who asked, what Add approves, and the fields
/// prefilled with the request's destination and alias. Pins the agent's lines above the ordinary fields.
pub(super) fn host_add_dialog_agent_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    use farhelm_proto::approvals::{ApprovalAction, ApprovalSession, PendingApproval};
    use farhelm_proto::ui_channel::{Collection, CollectionState, KeyedEntry};
    let request = PendingApproval {
        id: "golden-add".to_owned(),
        host_id: 2,
        host_name: "build-box".to_owned(),
        session: ApprovalSession {
            id: "7f8c2b1e-6d0e-4c51-9a8e-1e2f3a4b5c6d:a75bff30-1c2d-4e5f-8a9b-0c1d2e3f27a4"
                .to_owned(),
            title: Some("Provision the test fleet".to_owned()),
            host_name: Some("build-box".to_owned()),
            launch_kind: None,
        },
        action: ApprovalAction::HostAdd {
            destination: "ops@new-box.example".to_owned(),
            alias: Some("new-box".to_owned()),
        },
        own_host_session_action: false,
        expires_at_ms: 1,
    };
    let approvals = CollectionState {
        collection: Collection::Approvals,
        meta: crate::test_support::meta(&["golden-add"]),
        entries: vec![KeyedEntry {
            key: "golden-add".to_owned(),
            value: serde_json::to_value(&request).expect("an approval encodes"),
        }],
    };
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![
        crate::hosts::tests::hosts(&crate::hosts::tests::reference_hosts()),
        approvals,
    ]);
    let arming = cx.new(|_| crate::approvals::answers::Arming::armed_for_scene());
    // Opened once the connection has its UI session, as Add host's own scene opens.
    let handle = window.window_handle();
    let opener = state.ring.clone();
    super::once_ui_session(state, helm, connection, cx, move |cx| {
        let arming = arming.clone();
        let request = request.clone();
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            if let Some(agent) = crate::approvals::host_add::AgentRequest::of(&request, arming) {
                let _ = crate::hosts::add::open_for_agent(agent, &opener, &mark, window, cx);
            }
        });
    });
}

/// The setup confirmation for the reference screens' plan (reference `host-add-setup-plan`), as a probe's plan opens it
/// in place of Add host: the plan, Copy plan, the footer note and the three answers, focus on Set up. The opener is the
/// scene's ring handle. The fake and the connection are kept for the scene's life, like Add host's.
pub(super) fn host_add_setup_plan_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![crate::hosts::tests::hosts(
        &crate::hosts::tests::reference_hosts(),
    )]);
    let handle = window.window_handle();
    let opener = state.ring.clone();
    super::once_ui_session(state, helm, connection, cx, move |cx| {
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            let offer = crate::hosts::setup::tests::reference_offer(opener.clone(), mark);
            crate::hosts::setup::confirm(offer, window, cx);
        });
    });
}

/// Add host found a supervisor whose owner last connected three days ago.
/// Use the production question with a frozen age for the screenshot.
pub(super) fn host_add_takeover_setup(_: &SceneState, window: &mut Window, cx: &mut App) {
    let now = ticker::wall_now(cx)
        .duration_since(std::time::UNIX_EPOCH)
        .expect("golden clock follows Unix epoch")
        .as_secs() as i64;
    let owner = farhelm_proto::HelmOwnerRefusal {
        tag: "ab12".into(),
        owner_revision: "owner-1".into(),
        connected: false,
        last_connected_at: now - 3 * 24 * 60 * 60,
    };
    crate::hosts::add::ask_takeover("devbox", &owner, &Mark::take(cx), |_, _| {}, window, cx);
}

/// Show the actual host-wide Suspend question for a connected sandbox.
pub(super) fn host_suspend_confirmation_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    let (mut helm, client, events) = FakeHelm::new();
    let connection = session::start(client, events, None, cx);
    let mut host = crate::hosts::tests::host(4, "tensorlake", crate::hosts::tests::connected());
    host.name = "Build sandbox".into();
    helm.connect(vec![crate::hosts::tests::hosts(&[host])]);
    let window = window.window_handle();
    super::once_ui_session(state, helm, connection, cx, move |cx| {
        window
            .update(cx, |_, window, cx| {
                crate::hosts::show_suspend_question_for_golden(4, window, cx);
            })
            .unwrap();
    });
}

/// The removal confirmation for the reference screens' `build-box` (reference `host-remove-confirmation`), as its menu's
/// Remove opens it: the host's mark, name, phase and build, its destination and what removal keeps, the footer note and
/// the three answers, focus on Remove. The question opens once the connection has a UI session, whose store the
/// snapshot made, as the setup confirmation's scene does; the fake, the connection and the watch are kept for the
/// scene's life.
pub(super) fn host_remove_confirmation_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![crate::hosts::tests::hosts(
        &crate::hosts::tests::reference_hosts(),
    )]);
    let handle = window.window_handle();
    super::once_ui_session(state, helm, connection, cx, move |cx| {
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            let _ = crate::hosts::remove::ask(2, &mark, window, cx);
        });
    });
}

/// Make the Hosts panel over a fake helm holding `listed` hosts, with Details on when `details` is; the scene draws it
/// through [`hosts_panels`]. The fake, the connection and the panel are kept for the scene's life.
fn hosts_panel_scene(state: &SceneState, listed: &[HostView], details: bool, cx: &mut App) {
    hosts_panel_scene_with(state, vec![crate::hosts::tests::hosts(listed)], details, cx);
}

/// [`hosts_panel_scene`] over a fake helm connected with `snapshot`, for a scene that needs more than the hosts.
fn hosts_panel_scene_with(
    state: &SceneState,
    snapshot: Vec<farhelm_proto::ui_channel::CollectionState>,
    details: bool,
    cx: &mut App,
) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(snapshot);
    let panel = cx.new(HostsPanel::new);
    if details {
        panel.update(cx, |panel, cx| panel.show_details(true, cx));
    }
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(panel));
}

/// The reference screens' three connected hosts, Details off (reference `hosts-panel`): the heading with its count,
/// switch and Update all, and each row's mark, name and phase.
pub(super) fn hosts_panel_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    hosts_panel_scene(state, &crate::hosts::tests::reference_hosts(), false, cx);
}

/// The same hosts with Details on (reference `hosts-panel-details`): each row's destination behind its alias and its
/// version, identity and session count.
pub(super) fn hosts_panel_details_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    hosts_panel_scene(state, &crate::hosts::tests::reference_hosts(), true, cx);
}

/// Details on with `build-box` failing to connect (reference `hosts-panel-host-unreachable`): its grey
/// "Connecting", and its attempt and the long SSH error wrapping under it.
pub(super) fn hosts_panel_host_unreachable_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    use crate::hosts::tests::{aliased, connecting, host, reference_hosts};
    let mut listed = reference_hosts();
    listed[1] = aliased(
        host(
            2,
            "ssh",
            connecting(
                2,
                Some(
                    "SSH could not connect or log in to the host; SSH stderr: \"ssh: Could not resolve hostname \
                     reference-capture.invalid: Name or service not known\"; cause: connection closed before hello",
                ),
            ),
        ),
        "build-box",
        "reference-capture.invalid",
    );
    hosts_panel_scene(state, &listed, true, cx);
}

/// With Details off, the frozen row shows its owner. Open its action menu
/// to capture Take over next to Retry and Remove.
pub(super) fn hosts_panel_owned_elsewhere_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    use crate::hosts::tests::{aliased, host, reference_hosts};
    use farhelm_proto::ui_channel::HostStateView;

    let mut listed = reference_hosts();
    listed[1] = aliased(
        host(
            2,
            "ssh",
            HostStateView::OwnedElsewhere {
                host_identity: "install-2".into(),
                owner: farhelm_proto::HelmOwnerRefusal {
                    tag: "cd34".into(),
                    owner_revision: "owner-2".into(),
                    connected: true,
                    last_connected_at: 42,
                },
            },
        ),
        "build-box",
        "dev@buildbox.example",
    );
    hosts_panel_scene(state, &listed, false, cx);
    state
        .keep
        .borrow_mut()
        .push(Box::new(Pending(Cell::new(true))));
}

/// Details on with a Tensorlake sandbox, `gpu-sandbox`, whose credentials the helm could not deliver, below the
/// reference hosts (against reference `hosts-panel-details`, whose stack had no sandbox): the sandbox's row in the
/// advisory amber though it is connected, and its details naming the sandbox, its recipe and size, then "Credentials
/// not delivered:" and the helm's reason. Drawn in the tall column, since four rows' details overflow the short one.
pub(super) fn hosts_panel_credentials_not_delivered_setup(
    state: &SceneState,
    _: &mut Window,
    cx: &mut App,
) {
    use crate::hosts::tests::{aliased, connected, host, reference_hosts};
    let mut sandbox = aliased(
        host(4, "tensorlake", connected()),
        "gpu-sandbox",
        "farhelm-host-fixture-gpubox",
    );
    sandbox.tensorlake = Some(farhelm_proto::ui_channel::TensorlakeHostView {
        sandbox: "farhelm-host-fixture-gpubox".into(),
        recipe: "rust-dev".into(),
        size: Some(farhelm_proto::recipes::RecipeSize {
            cpus: 2,
            memory_gb: 4,
            disk_gb: 20,
        }),
        tunnels: None,
        credential_warning: Some(
            "the sandbox refused the Codex login file: permission denied".into(),
        ),
    });
    let mut listed = reference_hosts();
    listed.push(sandbox);
    hosts_panel_scene(state, &listed, true, cx);
}

/// The Tensorlake account as a sandbox's Add host form has read it with a key saved, for the sandbox scenes below: no
/// SSH key made and creation asking first, as the helm's defaults have it.
fn saved_key_account() -> farhelm_proto::tensorlake::Settings {
    farhelm_proto::tensorlake::Settings {
        api_key_set: true,
        create_without_asking: false,
        helm_tag: "fixture".into(),
        ssh_key: farhelm_proto::tensorlake::SshKeyStatus::NotCreated,
    }
}

/// The pushed recipe catalog with the one recipe the sandbox scenes choose, `rust-dev`: 2 CPUs, 4 GB memory and 20 GB
/// disk on an image, with two packages and no snapshot built yet.
fn rust_dev_catalog() -> farhelm_proto::ui_channel::CollectionState {
    use farhelm_proto::ui_channel::{Collection, CollectionState, KeyedEntry};
    let recipe = serde_json::json!({
        "name": "rust-dev",
        "size": { "cpus": 2, "memory_gb": 4, "disk_gb": 20 },
        "base": { "kind": "image", "image": "team/devbox-base" },
        "packages": ["jq", "ripgrep"],
        "claude": null, "codex": null,
        "claude_login_credential": null, "codex_login_credential": null, "github_token_credential": null,
        "tunnels": [],
    });
    CollectionState {
        collection: Collection::Recipes,
        meta: crate::test_support::meta(&["rust-dev"]),
        entries: vec![KeyedEntry {
            key: "rust-dev".to_owned(),
            value: recipe,
        }],
    }
}

/// Run `open` once the fake helm, connected with the reference hosts and [`rust_dev_catalog`], has made its UI session,
/// as the Add host scenes open their dialog; the fake, the connection and the watch are kept for the scene's life.
/// Right after `open`, the recipe editor's credential catalog read is answered with no credentials, as Settings' recipe
/// editor scene answers it; the scenes that open no editor send no such read, and nothing else they send is answered.
fn with_sandbox_catalog(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
    open: impl Fn(&gpui::FocusHandle, &mut Window, &mut App) + 'static,
) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![
        crate::hosts::tests::hosts(&crate::hosts::tests::reference_hosts()),
        rust_dev_catalog(),
    ]);
    let handle = window.window_handle();
    let opener = state.ring.clone();
    let helm = Rc::new(RefCell::new(helm));
    let answering = helm.clone();
    super::once_ui_session(state, helm, connection, cx, move |cx| {
        let _ = handle.update(cx, |_, window, cx| open(&opener, window, cx));
        let mut helm = answering.borrow_mut();
        for read in helm.commands() {
            if read.path == "/api/credentials" {
                helm.reply(read.id, 200, Some(serde_json::json!([])));
            }
        }
    });
}

/// The Add host dialog on the Tensorlake sandbox kind with a key saved (against reference `host-add-dialog`, whose
/// stack had no Tensorlake account): the kind choice, Name holding `gpubox` and focus, Recipe with `rust-dev` chosen
/// and its summary, and Cancel and Continue. It opens as a sandbox's creation question's Back reopens it
/// (`hosts::add::reopen_sandbox`), from the account as read, so no read is answered and no Tensorlake account reached.
pub(super) fn host_add_tensorlake_form_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    with_sandbox_catalog(state, window, cx, reopen_gpubox);
}

/// Reopen Add host on the sandbox kind as [`host_add_tensorlake_form_setup`] pictures it: `gpubox` and `rust-dev`
/// from [`saved_key_account`], with the scene's ring handle as the opener.
fn reopen_gpubox(opener: &gpui::FocusHandle, window: &mut Window, cx: &mut App) {
    let mark = Mark::take(cx);
    let seed = crate::hosts::add::SandboxSeed {
        name: "gpubox".to_owned(),
        recipe: "rust-dev".to_owned(),
        account: saved_key_account(),
        problem: None,
    };
    let _ = crate::hosts::add::reopen_sandbox(opener, &mark, seed, window, cx);
}

/// The recipe editor inside Add host (against reference `host-add-dialog`, whose stack had no Tensorlake account), as
/// "New recipe…" in [`host_add_tensorlake_form_setup`]'s form opens it: the editor in place of the fields, focus in its
/// Name, and its Cancel and Save in Add host's footer, with no credentials in the catalog. The editor opens through
/// `hosts::add::open_new_recipe_for_opened` rather than by pressing Recipe and "New recipe…"; the form's own tests pin
/// that click path.
pub(super) fn host_add_tensorlake_recipe_editor_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    with_sandbox_catalog(state, window, cx, |opener, window, cx| {
        reopen_gpubox(opener, window, cx);
        crate::hosts::add::open_new_recipe_for_opened(window, cx);
    });
}

/// The creation question Continue opens for `gpubox` (against reference `host-add-setup-plan`, the SSH setup's
/// confirmation, since the reference stack had no Tensorlake account): its title and size, the helm's plan for a
/// sandbox from `rust-dev` in the helm's words with Copy plan, the billing callout with its pricing link, and Cancel,
/// Back and Create sandbox, focus on Create sandbox. The plan is the scene's own, so no Tensorlake account is reached.
pub(super) fn host_add_tensorlake_creation_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    use farhelm_proto::provisioning::{PlanStep, PlanSteps};
    with_sandbox_catalog(state, window, cx, |opener, window, cx| {
        let step = |id: &str, name: &str| PlanStep {
            id: id.into(),
            name: name.into(),
            paths: Vec::new(),
            explanation: None,
        };
        let steps = PlanSteps {
            lead: "Create a Tensorlake sandbox and set it up as a host".into(),
            kept: None,
            steps: vec![
                step(
                    "build-snapshot",
                    "Build the snapshot for recipe rust-dev (packages, Claude Code and Codex; takes a few minutes)",
                ),
                step(
                    "create-sandbox",
                    "Create sandbox farhelm-host-fixture-gpubox with 2 CPUs, 4 GB memory and 20 GB disk",
                ),
                step(
                    "prepare-sandbox",
                    "Start the sandbox's login manager and keep the user manager running",
                ),
                step(
                    "configure-agents",
                    "Configure host credentials and agent endpoints, preserving other settings",
                ),
                step(
                    "set-up-farhelm",
                    "Install Farhelm's supervisor, as on any host",
                ),
            ],
        };
        let confirmation = std::iter::once(steps.lead.clone())
            .chain(
                steps
                    .steps
                    .iter()
                    .enumerate()
                    .map(|(n, step)| format!("{}. {}", n + 1, step.name)),
            )
            .map(|line| line + "\n")
            .collect();
        let offer = crate::hosts::setup::Offer {
            key: crate::operations::OpKey::new_host_add(),
            probe_id: "golden-sandbox-plan".to_owned(),
            confirmation,
            steps,
            target: crate::hosts::setup::Target::Sandbox(crate::hosts::setup::Sandbox {
                opener: opener.clone(),
                name: "gpubox".to_owned(),
                recipe: "rust-dev".to_owned(),
                account: saved_key_account(),
                resources: farhelm_proto::recipes::RecipeSize {
                    cpus: 2,
                    memory_gb: 4,
                    disk_gb: 20,
                },
            }),
            mark: Mark::take(cx),
        };
        crate::hosts::setup::confirm(offer, window, cx);
    });
}

/// The reference screens' setup of `dev@new-box` (`host-setup-running-hosts-panel` and `host-setup-failed-hosts-panel`)
/// with Details on: the three reference hosts and `dev@new-box` failing to connect at `attempt`, its setup's nine steps
/// in `statuses`, the run in `status`, and `reason` as the failed step's and the run's message. Motion is reduced, so
/// the running step's ring and the status's dot hold still; the test clock does not move, so the elapsed time reads
/// "0:00" where the reference caught "0:06".
fn host_setup_scene(
    state: &SceneState,
    attempt: u32,
    status: farhelm_proto::provisioning::RunStatus,
    statuses: [farhelm_proto::provisioning::StepStatus; 9],
    reason: Option<&str>,
    cx: &mut App,
) {
    use crate::hosts::tests::{connecting, host, hosts, progress, reference_hosts, run};
    use farhelm_proto::provisioning::{ProvisioningOperation, StepStatus};
    ticker::set_reduced_motion(true, cx);
    let mut new_box = host(
        4,
        "ssh",
        connecting(
            attempt,
            Some(
                "SSH could not connect or log in to the host; SSH stderr: \"ssh: Could not resolve hostname new-box: \
                 Temporary failure in name resolution\"; cause: connection closed before hello",
            ),
        ),
    );
    new_box.name = "dev@new-box".into();
    new_box.destination = Some("dev@new-box".into());
    let mut listed = reference_hosts();
    listed.push(new_box);
    let names = [
        "Create directories",
        "Upload the farhelm binary",
        "Install the farhelm binary",
        "Write the systemd user unit",
        "Reload systemd user units",
        "Enable and start the supervisor",
        "Enable lingering, to start at boot",
        "Remove the bundled tmux",
        "Connect to the supervisor",
    ];
    let mut setup = run(4, ProvisioningOperation::Add, status);
    setup.steps = names
        .into_iter()
        .zip(statuses)
        .map(|(name, status)| {
            let failed = status == StepStatus::Failed;
            let mut step = crate::hosts::progress::tests::step(name, status);
            step.message = reason.filter(|_| failed).map(str::to_owned);
            step
        })
        .collect();
    setup.message = reason.map(str::to_owned);
    hosts_panel_scene_with(state, vec![hosts(&listed), progress(&[setup])], true, cx);
}

/// `dev@new-box`'s setup at its first step (reference `host-setup-running-hosts-panel`): the inline status on its own
/// line under the name, the step list with the running step's amber ring and the rest pending.
pub(super) fn host_setup_running_hosts_panel_setup(
    state: &SceneState,
    _: &mut Window,
    cx: &mut App,
) {
    use farhelm_proto::provisioning::{RunStatus, StepStatus};
    let statuses = std::array::from_fn(|n| {
        if n == 0 {
            StepStatus::Running
        } else {
            StepStatus::Pending
        }
    });
    host_setup_scene(state, 3, RunStatus::Running, statuses, None, cx);
}

/// `dev@new-box`'s setup failed at its last step (reference `host-setup-failed-hosts-panel`): the phase words back,
/// eight steps checked green, the last crossed red with the helm's reason under it, and the run's message after them.
pub(super) fn host_setup_failed_hosts_panel_setup(
    state: &SceneState,
    _: &mut Window,
    cx: &mut App,
) {
    use farhelm_proto::provisioning::{RunStatus, StepStatus};
    let statuses = std::array::from_fn(|n| {
        if n == 8 {
            StepStatus::Failed
        } else {
            StepStatus::Completed
        }
    });
    host_setup_scene(
        state,
        4,
        RunStatus::Failed,
        statuses,
        Some(
            "step 9 (attach-supervisor) failed: injected attach-supervisor failure: host stderr \"Setup could not \
             finish\"; rerun provisioning to continue",
        ),
        cx,
    );
}

/// The Hosts panel with an Update on offer in two rows, Details off (no reference: the old reference screens show no
/// outdated host): `build-box` on an older build, "Old version" with the amber outlined `↑ Update`, and `gpu-1` refused
/// at the handshake on an older protocol, "Needs update" with the red one; the heading's Update all available.
pub(super) fn hosts_panel_updates_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    use crate::hosts::tests::{reference_hosts, skew};
    let mut listed = reference_hosts();
    listed[1].state = farhelm_proto::ui_channel::HostStateView::Connected {
        identity: Some("c1182bd3-a604-4d1e-855d-ccbf6c992b0b".into()),
        build_version: "0.15.0".into(),
        old_version: true,
        newer_version: false,
        refresh: farhelm_proto::ui_channel::RefreshView::Ok { sessions: 1 },
    };
    listed[2].state = skew(1, 2);
    hosts_panel_scene(state, &listed, false, cx);
}

/// `build-box`'s Update running at its second of three steps, Details off (no reference): the row stays folded and its
/// inline status takes the phase words' place, the count, the running step and the clock without the visible verb.
/// Motion is reduced, so the dot holds still, and the test clock does not move, so the elapsed time reads "0:00".
pub(super) fn host_update_running_hosts_panel_setup(
    state: &SceneState,
    _: &mut Window,
    cx: &mut App,
) {
    use crate::hosts::tests::{hosts, progress, reference_hosts, run};
    use farhelm_proto::provisioning::{ProvisioningOperation, RunStatus, StepStatus};
    ticker::set_reduced_motion(true, cx);
    let mut update = run(2, ProvisioningOperation::Update, RunStatus::Running);
    update.steps = [
        ("Upload the farhelm binary", StepStatus::Completed),
        ("Install the farhelm binary", StepStatus::Running),
        ("Connect to the supervisor", StepStatus::Pending),
    ]
    .into_iter()
    .map(|(name, status)| crate::hosts::progress::tests::step(name, status))
    .collect();
    hosts_panel_scene_with(
        state,
        vec![hosts(&reference_hosts()), progress(&[update])],
        false,
        cx,
    );
}

/// `dev@new-box`'s setup at its third of nine steps, for the progress card scenes: the reference screens' new host and
/// steps (`host_setup_scene`'s), the first two completed, the third running with an explanation, the host connecting at
/// attempt 3 after `error`, and `message` as the run's own message. Motion is reduced, so the running step's ring holds
/// still, and the test clock does not move, so the elapsed time reads "0:00".
fn progress_card_scene(state: &SceneState, error: &str, message: Option<&str>, cx: &mut App) {
    use crate::hosts::tests::{connecting, host, hosts, progress, reference_hosts, run};
    use farhelm_proto::provisioning::{ProvisioningOperation, RunStatus, StepStatus};
    ticker::set_reduced_motion(true, cx);
    let mut new_box = host(4, "ssh", connecting(3, Some(error)));
    new_box.name = "dev@new-box".into();
    new_box.destination = Some("dev@new-box".into());
    let mut listed = reference_hosts();
    listed.push(new_box);
    let names = [
        "Create directories",
        "Upload the farhelm binary",
        "Install the farhelm binary",
        "Write the systemd user unit",
        "Reload systemd user units",
        "Enable and start the supervisor",
        "Enable lingering, to start at boot",
        "Remove the bundled tmux",
        "Connect to the supervisor",
    ];
    let mut setup = run(4, ProvisioningOperation::Add, RunStatus::Running);
    setup.steps = names
        .into_iter()
        .enumerate()
        .map(|(n, name)| {
            let status = match n {
                0 | 1 => StepStatus::Completed,
                2 => StepStatus::Running,
                _ => StepStatus::Pending,
            };
            let mut step = crate::hosts::progress::tests::step(name, status);
            if n == 2 {
                step.explanation = Some(
                    "Replaces the farhelm binary in ~/.local/bin and keeps the old one.".into(),
                );
            }
            step
        })
        .collect();
    setup.message = message.map(str::to_owned);
    hosts_panel_scene_with(state, vec![hosts(&listed), progress(&[setup])], false, cx);
}

/// The progress card of `dev@new-box`'s running setup (no reference picture; compared with the old stylesheet's
/// `.host-progress-*` rules): the host's mark and short name, "Setting up: 2 of 9 steps done" and the time, every step
/// with the running one ringed amber and its name set semibold over its explanation, and the connection detail.
pub(super) fn host_progress_card_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    progress_card_scene(
        state,
        "SSH could not connect or log in to the host; SSH stderr: \"ssh: Could not resolve hostname new-box: \
         Temporary failure in name resolution\"; cause: connection closed before hello",
        None,
        cx,
    );
}

/// The same card in a 300-pixel window with a long connection error and a long run message (no reference picture):
/// the list kept down to the running step, each diagnosis cut to its last whole lines, and "Full details in Hosts".
pub(super) fn host_progress_card_preview_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    let long = "SSH failed while reconnecting. ".repeat(12) + "Final diagnosis: permission denied.";
    progress_card_scene(state, &long, Some(&long), cx);
}

/// The progress card of host 4 inside a stand-in of the chrome the layer manager's card form of hover help draws
/// around it (the tooltip's raised ground, border, radius and menu shadow, at the width its hover help gives it), which
/// the card fills. Made once the scene's snapshot has reached the store, which happens after the setup runs.
pub(super) fn progress_cards(state: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    let palette = theme::palette(cx);
    let kept = state.keep.borrow().iter().find_map(|kept| {
        kept.downcast_ref::<Entity<crate::hosts::ProgressCard>>()
            .cloned()
    });
    let card = kept.or_else(|| {
        let card = crate::hosts::progress_card(4, cx)?;
        state.keep.borrow_mut().push(Box::new(card.clone()));
        Some(card)
    });
    div()
        .w(crate::hosts::progress::card::WIDTH)
        .bg(theme::color(palette.bg_2))
        .border_1()
        .border_color(theme::color(palette.border))
        .rounded(theme::metrics::RADIUS_MD)
        .shadow(crate::layers::menu_shadow(palette))
        .children(card)
        .into_any_element()
}

/// The host refusal dialog for a refused Update of `build-box` below the upgrade floor (no reference: the old dialog's
/// look is reference `host-uninstall-refusal`, the same dialog for Uninstall): "Update host · build-box", the red
/// summary, the destination and the helm's manual remedy naming the executable, and Close with focus.
pub(super) fn host_update_refusal_setup(_: &SceneState, window: &mut Window, cx: &mut App) {
    use crate::hosts::refusal::{self, Refusal};
    let view = crate::hosts::tests::reference_hosts().remove(1);
    let words = HostText::from_host(
        "the host runs Farhelm 0.12.0, older than this helm's upgrade floor of 0.14.0; it is too old to \
         update in place. On that host, stop its Farhelm supervisor service. Its Farhelm executable is \
         /home/dev/.local/lib/farhelm/farhelm. Remove its installation directory only if that directory is \
         dedicated to Farhelm (normally ~/.local/lib/farhelm). Then remove the host from Farhelm and set it \
         up again",
    );
    let reason = crate::operations::Refusal {
        words,
        status: 409,
        markers: Vec::new(),
    };
    let mark = Mark::take(cx);
    let _ = refusal::open(
        Refusal::of("Update host", &view, &reason),
        &mark,
        window,
        cx,
    );
}

/// Add, Update and Uninstall use the same Mac capability check and get the same reason from the helm.
const MAC_MISSING_DATE: &str = "remote Mac provisioning requires the command date";

/// An Update can be refused before any step runs when its Mac is missing the service-stop clock.
pub(super) fn host_update_mac_clock(_: &SceneState, window: &mut Window, cx: &mut App) {
    mac_missing_date_refusal("Update host", window, cx);
}

/// Uninstall likewise cannot start without the clock that bounds its service-stop step.
pub(super) fn host_uninstall_mac_clock(_: &SceneState, window: &mut Window, cx: &mut App) {
    mac_missing_date_refusal(crate::hosts::uninstall::ACTION, window, cx);
}

/// The two existing-host actions show the capability requirement with the host's name and destination.
fn mac_missing_date_refusal(action: &'static str, window: &mut Window, cx: &mut App) {
    use crate::hosts::refusal::{self, Refusal};
    use crate::hosts::tests::{aliased, reference_hosts};
    let view = aliased(reference_hosts().remove(1), "mac-box", "dev@mac-box");
    let reason = crate::operations::Refusal {
        words: HostText::from_host(MAC_MISSING_DATE),
        status: 409,
        markers: Vec::new(),
    };
    let mark = Mark::take(cx);
    let _ = refusal::open(Refusal::of(action, &view, &reason), &mark, window, cx);
}

/// The host refusal dialog for an uninstall of `build-box` refused while a session still runs there (reference
/// `host-uninstall-refusal`, which the helm's real refusal drew): "Uninstall Farhelm · build-box", the red summary, the
/// destination and the helm's words, and Close with focus. No Remove: the helm marks no remedy on this refusal.
pub(super) fn host_uninstall_refusal_setup(_: &SceneState, window: &mut Window, cx: &mut App) {
    use crate::hosts::refusal::{self, Refusal};
    let view = crate::hosts::tests::reference_hosts().remove(1);
    let reason = crate::operations::Refusal {
        words: HostText::from_host(
            "uninstall is refused while sessions on this host have not ended or still have terminal tabs open: \
             \"migrate auth tokens\". Stop those sessions and close their tabs, then choose uninstall again",
        ),
        status: 409,
        markers: Vec::new(),
    };
    let mark = Mark::take(cx);
    let _ = refusal::open(
        Refusal::of(crate::hosts::uninstall::ACTION, &view, &reason),
        &mark,
        window,
        cx,
    );
}

/// The uninstall confirmation for the reference screens' `build-box` (no reference: the old dialog was not captured),
/// as its menu's Uninstall opens it once the helm's plan arrives: the host's mark, name, phase and build, the numbered
/// plan with its paths and Copy plan, the kept data in its amber note, and Cancel and Uninstall, focus on Uninstall. The
/// question opens once the connection has a UI session, as the removal confirmation's scene does; the fake, the
/// connection and the watch are kept for the scene's life.
pub(super) fn host_uninstall_confirmation_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![crate::hosts::tests::hosts(
        &crate::hosts::tests::reference_hosts(),
    )]);
    let handle = window.window_handle();
    super::once_ui_session(state, helm, connection, cx, move |cx| {
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            let view = crate::hosts::tests::reference_hosts().remove(1);
            crate::hosts::uninstall::confirm_for_golden(view, &mark, window, cx);
        });
    });
}

/// `build-box`'s uninstall running at its second of six steps, Details off (no reference): the running row opens by
/// itself, its inline status reading "Uninstalling", the count, the running step and the clock, and the run's steps
/// under it with Collapse. Motion is reduced, so the dot holds still, and the test clock does not move, so the elapsed
/// time reads "0:00".
pub(super) fn host_uninstall_running_hosts_panel_setup(
    state: &SceneState,
    _: &mut Window,
    cx: &mut App,
) {
    use crate::hosts::tests::{hosts, progress, reference_hosts, run};
    use farhelm_proto::provisioning::{ProvisioningOperation, RunStatus, StepStatus};
    ticker::set_reduced_motion(true, cx);
    let mut uninstall = run(2, ProvisioningOperation::Uninstall, RunStatus::Running);
    uninstall.steps = crate::hosts::uninstall::tests::reference_steps()
        .steps
        .iter()
        .enumerate()
        .map(|(n, planned)| {
            let status = match n {
                0 => StepStatus::Completed,
                1 => StepStatus::Running,
                _ => StepStatus::Pending,
            };
            crate::hosts::progress::tests::step(&planned.name, status)
        })
        .collect();
    hosts_panel_scene_with(
        state,
        vec![hosts(&reference_hosts()), progress(&[uninstall])],
        false,
        cx,
    );
}

/// The Hosts panel after this window uninstalled `build-box` (no reference): the two hosts left and, above them, the
/// one-line removal notice, "Removed from build-box · data kept", with Copy and ×. The notice is shown once the
/// connection has a UI session, since it belongs to that session's operation store.
pub(super) fn hosts_panel_uninstall_notice_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    let mut listed = crate::hosts::tests::reference_hosts();
    listed.remove(1);
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![crate::hosts::tests::hosts(&listed)]);
    let panel = cx.new(HostsPanel::new);
    super::once_ui_session(state, helm, connection, cx, |cx| {
        crate::hosts::uninstall::show_notice_for_golden(
            "build-box",
            "/home/dev/.local/state/farhelm",
            cx,
        );
    });
    state.keep.borrow_mut().push(Box::new(panel));
}

/// The Hosts panel a scene's setup made, as [`hosts_panels`] draws it, in a column tall enough for a setup's nine
/// steps under its row and a failure's messages after them, as the reference screens' sidebar was.
pub(super) fn tall_hosts_panels(state: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    let palette = theme::palette(cx);
    let panel = state
        .keep
        .borrow()
        .iter()
        .find_map(|kept| kept.downcast_ref::<Entity<HostsPanel>>().cloned());
    div()
        .w(theme::metrics::SIDEBAR_WIDTH - px(56.))
        .h(px(900.))
        .bg(theme::color(palette.bg_1))
        .children(panel)
        .into_any_element()
}

/// The Hosts panel a scene's setup made, in the sidebar's column beside the rail (the sidebar's width less the rail's
/// 56 pixels) on the sidebar's surface.
pub(super) fn hosts_panels(state: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    let palette = theme::palette(cx);
    let panel = state
        .keep
        .borrow()
        .iter()
        .find_map(|kept| kept.downcast_ref::<Entity<HostsPanel>>().cloned());
    div()
        .w(theme::metrics::SIDEBAR_WIDTH - px(56.))
        .h(px(394.))
        .bg(theme::color(palette.bg_1))
        .children(panel)
        .into_any_element()
}

/// The reference screens' hosts in the Hosts panel, then `build-box`'s menu opened from its toggle (reference
/// `host-menu`): the header naming it, its destination and build, then Retry, highlighted, Update, Settings, and
/// Uninstall and Remove behind a separator.
/// The menu opens through [`host_menu_scene`] once the toggle has been drawn.
pub(super) fn host_menu_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    hosts_panel_scene(state, &crate::hosts::tests::reference_hosts(), false, cx);
    state
        .keep
        .borrow_mut()
        .push(Box::new(Pending(Cell::new(true))));
}

/// A stopped container waiting for its container host has no lifecycle action in the real Hosts menu.
pub(super) fn host_menu_waiting_container_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    use crate::hosts::tests::reference_hosts;
    use crate::session_list::environment_tests::{Provider, environment_host};
    use farhelm_proto::ui_channel::HostStateView;

    let mut listed = reference_hosts();
    listed[1] = environment_host(
        Provider::Container,
        HostStateView::Stopped {
            start_failed: None,
            waiting_for_place: true,
        },
    );
    listed[1].name = "build-container".into();
    listed[1].destination = None;
    hosts_panel_scene(state, &listed, false, cx);
    state
        .keep
        .borrow_mut()
        .push(Box::new(Pending(Cell::new(true))));
}

/// The Hosts panel at the scene's left, so its menu has room beside it, and once its toggles have been drawn, one press
/// of `build-box`'s toggle after that frame.
pub(super) fn host_menu_scene(state: &SceneState, window: &mut Window, cx: &mut App) -> AnyElement {
    let panel = state
        .keep
        .borrow()
        .iter()
        .find_map(|kept| kept.downcast_ref::<Entity<HostsPanel>>().cloned());
    if let Some(panel) = panel
        && panel.read(cx).toggle_drawn(2)
        && take_pending(state)
    {
        window.defer(cx, move |window, cx| {
            panel.update(cx, |panel, cx| panel.toggle_menu(2, window, cx))
        });
    }
    div()
        .size_full()
        .child(hosts_panels(state, window, cx))
        .into_any_element()
}

/// The reference screens' hosts, with `build-box`'s settings open as its menu opens them (reference
/// `host-settings-dialog`): the header with its mark, name, phase and build, then Connection, Appearance and the top of
/// Checkouts, focus on the destination's Edit. The dialog opens once the connection has a UI session, whose store the
/// snapshot made, as [`settings_scene`] opens Settings, and the checkout folder's read, sent as it opens, is answered
/// at once as the reference stack had it: no folder of the host's own and no default, so the field shows "No default
/// set". The fake, the connection and the watch are kept for the scene's life.
pub(super) fn host_settings_dialog_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let (helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    let helm = Rc::new(RefCell::new(helm));
    helm.borrow_mut().connect(vec![crate::hosts::tests::hosts(
        &crate::hosts::tests::reference_hosts(),
    )]);
    let handle = window.window_handle();
    // The scene's ring handle stands in for the menu's toggle, as it stands in for the gear in the Settings scenes.
    let opener = state.ring.clone();
    let answering = helm.clone();
    super::once_ui_session(state, helm, connection, cx, move |cx| {
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            crate::hosts::open_settings(2, &opener, &mark, window, cx);
        });
        let mut helm = answering.borrow_mut();
        for read in helm.commands() {
            if read.path == "/api/hosts/2/checkout-root" {
                let answer = serde_json::json!({ "root": null, "default": null });
                helm.reply(read.id, 200, Some(answer));
            }
        }
    });
}

/// The host settings dialog, then (reference `host-settings-dialog-end`) its body scrolled to its end.
pub(super) fn host_settings_dialog_end_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    host_settings_dialog_setup(state, window, cx);
    state
        .keep
        .borrow_mut()
        .push(Box::new(Pending(Cell::new(true))));
}

/// Once the dialog is open, one turn of the wheel over its body, after that frame, far enough to reach its end.
pub(super) fn host_settings_dialog_end(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    if layers::any_open(window, cx) && take_pending(state) {
        window.defer(cx, |window, cx| {
            window.dispatch_event(
                gpui::PlatformInput::Scroll(gpui::ScrollEvent {
                    // The middle of the scene, which the dialog's body covers whatever its header's height.
                    position: point(px(280.), px(500.)),
                    delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-4000.))),
                    ..Default::default()
                }),
                cx,
            );
        });
    }
    div().into_any_element()
}
