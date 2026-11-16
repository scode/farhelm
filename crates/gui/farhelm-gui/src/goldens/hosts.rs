//! Golden scenes for host management: the Hosts panel, a host's menu and settings, the Add host dialog, and the
//! questions, refusals and progress of host setup, update, removal and uninstall. The scene list itself, `SCENES` with
//! each scene's name and size, stays in `mod.rs`.

use crate::focus::Mark;
use crate::host_text::HostText;
use crate::hosts::HostsPanel;
use crate::layers;
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
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![crate::hosts::tests::hosts(
        &crate::hosts::tests::reference_hosts(),
    )]);
    // The dialog opens only over a live UI session, which the snapshot's connection makes after this setup returns:
    // open once the connection has one, and only once.
    let handle = window.window_handle();
    let opener = state.ring.clone();
    let opened = std::cell::Cell::new(false);
    let watch = cx.observe(&connection, move |connection, cx| {
        if opened.get() || connection.read(cx).session().is_none() {
            return;
        }
        opened.set(true);
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            crate::hosts::add::open(&opener, &mark, window, cx);
            let dialog = layers::last_opened::<crate::hosts::add::AddHostDialog>(cx);
            if let Some(dialog) = dialog {
                let input = dialog.read(cx).destination_input();
                input.set_text("dev@new-box", window, cx);
            }
        });
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(watch));
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
    let opened = std::cell::Cell::new(false);
    let watch = cx.observe(&connection, move |connection, cx| {
        if opened.get() || connection.read(cx).session().is_none() {
            return;
        }
        opened.set(true);
        let arming = arming.clone();
        let request = request.clone();
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            if let Some(agent) = crate::approvals::host_add::AgentRequest::of(&request, arming) {
                let _ = crate::hosts::add::open_for_agent(agent, &opener, &mark, window, cx);
            }
        });
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(watch));
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
    let opened = std::cell::Cell::new(false);
    let watch = cx.observe(&connection, move |connection, cx| {
        if opened.get() || connection.read(cx).session().is_none() {
            return;
        }
        opened.set(true);
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            let offer = crate::hosts::setup::tests::reference_offer(opener.clone(), mark);
            crate::hosts::setup::confirm(offer, window, cx);
        });
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(watch));
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
    let opened = std::cell::Cell::new(false);
    let watch = cx.observe(&connection, move |connection, cx| {
        if opened.get() || connection.read(cx).session().is_none() {
            return;
        }
        opened.set(true);
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            let _ = crate::hosts::remove::ask(2, &mark, window, cx);
        });
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(watch));
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
    let opened = std::cell::Cell::new(false);
    let watch = cx.observe(&connection, move |connection, cx| {
        if opened.get() || connection.read(cx).session().is_none() {
            return;
        }
        opened.set(true);
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            let view = crate::hosts::tests::reference_hosts().remove(1);
            crate::hosts::uninstall::confirm_for_golden(view, &mark, window, cx);
        });
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(watch));
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
    let shown = std::cell::Cell::new(false);
    let watch = cx.observe(&connection, move |connection, cx| {
        if shown.get() || connection.read(cx).session().is_none() {
            return;
        }
        shown.set(true);
        crate::hosts::uninstall::show_notice_for_golden(
            "build-box",
            "/home/dev/.local/state/farhelm",
            cx,
        );
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(panel));
    keep.push(Box::new(watch));
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
    let opened = Cell::new(false);
    let answering = helm.clone();
    let watch = cx.observe(&connection, move |connection, cx| {
        if opened.get() || connection.read(cx).session().is_none() {
            return;
        }
        opened.set(true);
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
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(watch));
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
