//! The UI half of provisioning: inspect, confirm, submit, then follow the
//! host-scoped run through the fleet feed.
//!
//! The helm is the authority for every meaningful decision here. It decides
//! whether a probe found a supervisor, positively established absence, or
//! reached a manual-only target; it renders the exact confirmation from the
//! plan the executor will consume; and it excludes overlapping runs across
//! every browser. This module keeps those answers intact rather than
//! reconstructing them from paths or action tags the UI happens to know.
//!
//! ## Why `OpLock` ends at acceptance
//!
//! A provision or update can take a minute. Holding the page token for that
//! minute would disable unrelated creates and host changes without excluding
//! another browser, while the helm already owns the real host-scoped lock.
//! Confirmation therefore claims [`OpLock`] only around the POST and releases
//! it as soon as the submission request completes: accepted identity,
//! accepted-but-unreadable body, or refusal. Progress is a read after that
//! point, driven by the existing fleet feed and its bounded fallback.
//!
//! ## Local setup is the same operation over another transport
//!
//! The reserved local row probes with `{kind: "local"}`. There is no
//! SSH-to-self fallback. A provisionable reply replaces the old manual-start
//! hint as the primary remedy while keeping that command beneath it; a
//! manual-only reply (including no usable systemd user manager) leaves the
//! command as the whole remedy.

use dioxus::prelude::*;

use crate::api::{
    ProbeResponse, ProvisioningOperation, ProvisioningStatus, ProvisioningSubmission,
    ProvisioningView, fetch_provisioning, plan_host_update, probe_local_host, probe_ssh_host,
    provision_host, update_host,
};
use crate::feed::{fallback_polls_now, fallback_sleep, use_feed_reader};
use crate::ops::OpLock;
use crate::peer::{DetailPart, PeerBlock, PeerLine};
use crate::reader::{SurfaceReader, Trigger, request_read};
use crate::{ApiBase, Host, HostId, HostKind};

/// Registry facts that make a displayed plan belong to this exact row.
///
/// The helm revalidates the same boundary before mutation. Keeping the UI
/// binding as well prevents a stale confirmation from remaining actionable
/// under a retargeted or newly adopted row while that refusal round trip is
/// still avoidable.
#[derive(Debug, Clone, PartialEq, Eq)]
struct HostBinding {
    id: HostId,
    kind: HostKind,
    destination: Option<String>,
    identity: Option<String>,
    remote_farhelm: Option<String>,
    remote_state_dir: Option<String>,
    incarnation: u64,
}

impl From<&Host> for HostBinding {
    fn from(host: &Host) -> Self {
        Self {
            id: host.id,
            kind: host.kind,
            destination: host.destination.clone(),
            identity: host.identity.clone(),
            remote_farhelm: host.remote_farhelm.clone(),
            remote_state_dir: host.remote_state_dir.clone(),
            incarnation: host.incarnation,
        }
    }
}

/// The opaque id and server-rendered text that must stay paired until POST.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingPlan {
    operation: ProvisioningOperation,
    binding: HostBinding,
    probe_id: String,
    confirmation: String,
}

/// What preparing a run established without yet mutating the target.
enum Preparation {
    Plan(PendingPlan),
    /// ADD found a supervisor and registered it as-is, so there is no run to
    /// confirm or follow.
    Discovered,
    /// The transport worked, but this target stays on the manual path.
    Manual(String),
    /// The probe may have registered a supervisor, but its successful reply
    /// did not say which outcome this client should render.
    Unvalidated(String),
}

/// Render the exact confirmation text returned by the helm.
///
/// This component deliberately does not walk `ProvisioningPlan.actions`.
/// The server's string was rendered from the same value execution consumes,
/// including paths, unit names, and the conditional linger promise. A second
/// renderer here would be a second list of promises that could drift.
#[component]
pub(crate) fn PlanConfirmation(
    confirmation: String,
    busy: bool,
    confirm_label: &'static str,
    on_confirm: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "provisioning-confirmation",
            PeerBlock { class: "provisioning-plan", text: confirmation }
            div { class: "provisioning-confirm-actions",
                button {
                    r#type: "button",
                    class: "btn provisioning-confirm",
                    disabled: busy,
                    onclick: move |_| on_confirm.call(()),
                    "{confirm_label}"
                }
                button {
                    r#type: "button",
                    class: "btn provisioning-cancel",
                    disabled: busy,
                    onclick: move |_| on_cancel.call(()),
                    "cancel"
                }
            }
        }
    }
}

/// Prepare a fresh plan for the requested idempotent operation.
async fn prepare(
    base: &str,
    host: &Host,
    operation: ProvisioningOperation,
) -> Result<Preparation, String> {
    let binding = HostBinding::from(host);
    match operation {
        ProvisioningOperation::Update => {
            let planned = plan_host_update(base, host.id).await?;
            Ok(Preparation::Plan(PendingPlan {
                operation,
                binding,
                probe_id: planned.probe_id,
                confirmation: planned.confirmation,
            }))
        }
        ProvisioningOperation::Add => {
            let probed = match host.kind {
                HostKind::Local => probe_local_host(base).await?,
                HostKind::Ssh => {
                    let destination = host
                        .destination
                        .as_deref()
                        .ok_or_else(|| "this ssh host has no destination to probe".to_string())?;
                    probe_ssh_host(
                        base,
                        destination,
                        host.remote_farhelm.as_deref().unwrap_or_default(),
                        host.remote_state_dir.as_deref().unwrap_or_default(),
                    )
                    .await?
                }
                HostKind::Unrecognized => {
                    return Err(
                        "this build does not recognize the host kind, so it cannot provision it"
                            .to_string(),
                    );
                }
            };
            Ok(match probed {
                ProbeResponse::Discovered => Preparation::Discovered,
                ProbeResponse::Provisionable {
                    probe_id,
                    confirmation,
                } => Preparation::Plan(PendingPlan {
                    operation,
                    binding,
                    probe_id,
                    confirmation,
                }),
                ProbeResponse::Manual { reason } => Preparation::Manual(reason),
                ProbeResponse::Unvalidated(problem) => Preparation::Unvalidated(problem),
            })
        }
    }
}

/// One registered host's provisioning actions and latest retained run.
///
/// `manual_remedy` is passed for every phase because the host row derives it
/// once. This component consumes and renders it only for the reserved local
/// setup state, where the automatic offer replaces it as the primary
/// affordance without removing the manual escape hatch.
#[component]
pub(crate) fn ProvisioningPanel(
    host: Host,
    mut ops: OpLock,
    /// Whether this row's manual local remedy should be replaced by an
    /// automatic ADD probe and offer.
    local_setup: bool,
    /// The exact manual fallback derived from the host phase.
    manual_remedy: Option<Vec<DetailPart>>,
    /// Paint-only busy bookkeeping for this row in the parent. It never
    /// excludes a run.
    on_running: EventHandler<bool>,
    /// Ask the authoritative registry to refresh after registration or an
    /// accepted run.
    on_changed: EventHandler<()>,
) -> Element {
    let base = use_context::<ApiBase>().0;
    let host_id = host.id;
    let binding = HostBinding::from(&host);
    let current_binding = use_memo(use_reactive((&binding,), |(binding,)| binding));
    let mut progress = use_signal(|| None::<ProvisioningView>);
    let mut progress_error = use_signal(|| None::<String>);
    let progress_surface = use_signal(SurfaceReader::default);
    let mut pending = use_signal(|| None::<PendingPlan>);
    let mut planning = use_signal(|| false);
    let mut action_error = use_signal(|| None::<String>);
    let mut action_warning = use_signal(|| None::<String>);
    // Acceptance is authoritative before the first follow-up GET. Retain
    // that short-lived fact so the local manual command cannot become the
    // primary remedy while automatic setup is already running.
    let mut accepted_run_waiting = use_signal(|| false);
    // Once automatic local setup proved possible (or failed before it could
    // decide), cancel and retry return to an explicit offer. A Manual reply
    // turns this off because repeating an unsupported probe is not a remedy.
    let mut local_auto_retry = use_signal(|| false);

    // The parent owns the provisioning-busy set. Removing this row must
    // remove only this component's contribution to it; mutation-busy lives
    // in a different set and is unaffected.
    use_drop(move || on_running.call(false));

    // One reader per row. A feed bump says only that something changed, so
    // every mounted row re-reads its own host-scoped view; the reader
    // coalesces bursts and keeps retry demand alive after a failed request.
    let read_base = base.clone();
    let read_progress = move || {
        let base = read_base.clone();
        async move {
            match fetch_provisioning(&base, host_id).await {
                Ok(view) => {
                    let running =
                        view.run_id.is_some() && view.status == ProvisioningStatus::Running;
                    accepted_run_waiting.set(false);
                    on_running.call(running);
                    if running {
                        // A run observed from another browser supersedes any
                        // plan this component was still displaying.
                        pending.set(None);
                    }
                    progress.set(Some(view));
                    progress_error.set(None);
                    true
                }
                Err(error) => {
                    progress_error.set(Some(error));
                    false
                }
            }
        }
    };
    let request_progress =
        move |trigger: Trigger| request_read(progress_surface, trigger, read_progress.clone());

    let mount_progress = request_progress.clone();
    use_hook(move || mount_progress(Trigger::Explicit));
    let feed_progress = request_progress.clone();
    use_feed_reader(move || feed_progress(Trigger::Notice));
    let fallback_progress = request_progress.clone();
    use_future(move || {
        let request = fallback_progress.clone();
        async move {
            loop {
                fallback_sleep().await;
                if fallback_polls_now() {
                    request(Trigger::Scheduled);
                }
            }
        }
    });

    // UPDATE planning does not mutate. ADD preparation is discovery-first
    // and may register an answering supervisor, but neither path starts a
    // provisioning run or holds OpLock across transport inspection. The
    // component's synchronous flag excludes duplicate plans from one row.
    let plan_base = base.clone();
    let plan_host = host.clone();
    let plan_progress = request_progress.clone();
    let begin_plan = move |operation: ProvisioningOperation| {
        if *planning.peek() || ops.busy_now() {
            return;
        }
        planning.set(true);
        pending.set(None);
        action_error.set(None);
        action_warning.set(None);
        let base = plan_base.clone();
        let host = plan_host.clone();
        let requested_binding = HostBinding::from(&host);
        let is_local_add = host.kind == HostKind::Local && operation == ProvisioningOperation::Add;
        let reread = plan_progress.clone();
        spawn(async move {
            let prepared = prepare(&base, &host, operation).await;
            if *current_binding.peek() != requested_binding {
                planning.set(false);
                return;
            }
            match prepared {
                Ok(Preparation::Plan(plan)) => {
                    if is_local_add {
                        local_auto_retry.set(true);
                    }
                    pending.set(Some(plan));
                }
                Ok(Preparation::Discovered) => {
                    // Discovery can register a host and can resolve a
                    // retained failed ADD run, so both authorities move.
                    if is_local_add {
                        local_auto_retry.set(false);
                    }
                    reread(Trigger::Explicit);
                    on_changed.call(());
                }
                Ok(Preparation::Manual(reason)) => {
                    if is_local_add {
                        local_auto_retry.set(false);
                    }
                    action_error.set(Some(reason));
                }
                Ok(Preparation::Unvalidated(problem)) => {
                    if is_local_add {
                        local_auto_retry.set(true);
                    }
                    action_error.set(Some(problem));
                    on_changed.call(());
                }
                Err(error) => {
                    if is_local_add {
                        local_auto_retry.set(true);
                    }
                    action_error.set(Some(error));
                }
            }
            planning.set(false);
        });
    };

    // A local down-state probes once after the progress authority has said
    // IDLE. Waiting for that first view prevents a reload during a live run
    // from displaying a second setup plan beside it. Both the prop and the
    // row binding are explicit dependencies; tracked signal reads cover the
    // idle/running transition and the page token becoming available.
    let mut local_probe_started = use_signal(|| false);
    // Setup-specific diagnostics belong to a state transition, not to the
    // lifetime of the row. Keep the previous prop explicitly so leaving the
    // state clears them even when the path was a failed-run rerun rather
    // than the initial automatic probe.
    let mut was_local_setup = use_signal(|| local_setup);
    let mut auto_plan = begin_plan.clone();
    use_effect(use_reactive(
        (&local_setup, &binding),
        move |(local_setup, binding)| {
            let authoritative = progress.read().clone();
            let page_busy = ops.busy();
            if pending
                .peek()
                .as_ref()
                .is_some_and(|plan| plan.binding != binding)
            {
                pending.set(None);
            }
            let running = authoritative.as_ref().is_some_and(|view| {
                view.run_id.is_some() && view.status == ProvisioningStatus::Running
            });
            if running {
                pending.set(None);
            }
            if local_setup
                && !page_busy
                && authoritative
                    .as_ref()
                    .is_some_and(|view| view.run_id.is_none())
                && !*local_probe_started.peek()
            {
                local_probe_started.set(true);
                auto_plan(ProvisioningOperation::Add);
            }
            if !local_setup && *was_local_setup.peek() {
                local_probe_started.set(false);
                local_auto_retry.set(false);
                pending.set(None);
                action_error.set(None);
            }
            if *was_local_setup.peek() != local_setup {
                was_local_setup.set(local_setup);
            }
        },
    ));

    // Confirmation is the only path here that starts a provisioning run.
    // ADD preparation may already have registered an answering supervisor,
    // but it never claims this token. The owned guard releases submission
    // exclusion even if this row disappears while the POST is pending.
    let submit_base = base.clone();
    let submit_progress = request_progress.clone();
    let confirm = move |_| {
        let Some(plan) = pending.peek().clone() else {
            return;
        };
        if plan.binding != *current_binding.peek() {
            pending.set(None);
            return;
        }
        let Some(claim) = ops.claim_guard() else {
            return;
        };
        // Every attempt consumes what was displayed from the client's point
        // of view too. A refusal or transport ambiguity cannot prove the
        // helm left this one-use id unconsumed.
        pending.set(None);
        action_error.set(None);
        action_warning.set(None);
        if plan.binding.kind == HostKind::Local && plan.operation == ProvisioningOperation::Add {
            local_auto_retry.set(false);
        }
        let base = submit_base.clone();
        let reread = submit_progress.clone();
        spawn(async move {
            let result = match plan.operation {
                ProvisioningOperation::Add => provision_host(&base, &plan.probe_id).await,
                ProvisioningOperation::Update => update_host(&base, host_id, &plan.probe_id).await,
            };
            // Progress reads and registry reconciliation are ordinary reads;
            // release the page token as soon as submission has an outcome.
            drop(claim);
            match result {
                Ok(ProvisioningSubmission::Accepted(accepted)) => {
                    // Paint the returned row busy before its follow-up GET
                    // can land. ADD registration can legitimately return an
                    // existing row, so the accepted id is the route, not the
                    // panel that happened to submit it.
                    on_changed.call(());
                    if accepted.host_id == host_id {
                        accepted_run_waiting.set(true);
                        on_running.call(true);
                        reread(Trigger::Explicit);
                    } else {
                        // The returned row owns its paint-only busy bit; its
                        // feed-driven reader will publish that state. This
                        // immediate read still catches a run that cannot be
                        // reconciled after acceptance.
                        if let Err(error) = fetch_provisioning(&base, accepted.host_id).await {
                            action_warning.set(Some(format!(
                                "the helm accepted run {} for host {}, but its progress could not yet be read: {}",
                                accepted.run_id, accepted.host_id, error
                            )));
                        }
                    }
                }
                Ok(ProvisioningSubmission::Unvalidated(warning)) => {
                    accepted_run_waiting.set(true);
                    on_running.call(true);
                    action_warning.set(Some(warning));
                    reread(Trigger::Explicit);
                    on_changed.call(());
                }
                Err(error) => {
                    if plan.binding.kind == HostKind::Local
                        && plan.operation == ProvisioningOperation::Add
                    {
                        local_auto_retry.set(true);
                    }
                    action_error.set(Some(error));
                    reread(Trigger::Explicit);
                    on_changed.call(());
                }
            }
        });
    };

    let snapshot = progress.read().clone();
    let offer = pending.read().clone();
    let current_error = action_error.read().clone();
    let current_warning = action_warning.read().clone();
    let current_progress_error = progress_error.read().clone();
    let is_planning = *planning.read();
    let is_accepted_run_waiting = *accepted_run_waiting.read();
    let can_retry_local_setup = *local_auto_retry.read();
    let page_busy = ops.busy();
    let update_allowed = host.kind != HostKind::Unrecognized;
    let run_active = snapshot
        .as_ref()
        .is_some_and(|view| view.run_id.is_some() && view.status == ProvisioningStatus::Running);
    let failed_operation = snapshot.as_ref().and_then(|view| {
        (view.run_id.is_some() && view.status == ProvisioningStatus::Failed)
            .then_some(view.operation)
            .flatten()
    });
    let automatic_local_remedy = local_setup
        && (offer.is_some()
            || is_planning
            || run_active
            || is_accepted_run_waiting
            || failed_operation.is_some()
            || can_retry_local_setup);

    rsx! {
        div {
            class: "provisioning-panel",
            "data-provisioning-host": "{host_id}",
            if let Some(view) = snapshot && view.run_id.is_some() {
                div {
                        class: "provisioning-run",
                        "data-provisioning-status": "{status_label(view.status)}",
                        "data-provisioning-operation": "{operation_label(view.operation)}",
                        div { class: "provisioning-run-header",
                            span { class: "provisioning-title",
                                "{operation_label(view.operation)} provisioning"
                            }
                            span { class: "provisioning-status", "{status_label(view.status)}" }
                        }
                        ol { class: "provisioning-steps",
                            for step in view.steps {
                                li {
                                    class: "provisioning-step",
                                    "data-step": "{step.step}",
                                    "data-status": "{step.status}",
                                    span { class: "provisioning-step-name", "{step.step}" }
                                    span { class: "provisioning-step-status", "{step.status}" }
                                    if let Some(message) = step.message {
                                        PeerLine {
                                            class: "provisioning-step-message",
                                            parts: vec![DetailPart::Peer(message)],
                                        }
                                    }
                                }
                            }
                        }
                        if let Some(message) = view.message {
                            PeerLine {
                                class: "provisioning-run-message",
                                parts: vec![DetailPart::Peer(message)],
                            }
                        }
                }
            }

            if let Some(plan) = offer {
                PlanConfirmation {
                    confirmation: plan.confirmation,
                    busy: page_busy,
                    confirm_label: match plan.operation {
                        ProvisioningOperation::Add => "confirm setup",
                        ProvisioningOperation::Update => "confirm update",
                    },
                    on_confirm: confirm,
                    on_cancel: move |_| {
                        if !ops.busy_now() {
                            pending.set(None);
                            action_error.set(None);
                            action_warning.set(None);
                            if local_setup {
                                local_auto_retry.set(true);
                            }
                        }
                    },
                }
            } else if !run_active && update_allowed {
                div { class: "provisioning-actions",
                    if let Some(operation) = failed_operation {
                        button {
                            r#type: "button",
                            class: "btn provisioning-rerun",
                            disabled: page_busy || is_planning,
                            onclick: {
                                let mut begin_plan = begin_plan.clone();
                                move |_| begin_plan(operation)
                            },
                            if is_planning { "planning…" } else { "re-run" }
                        }
                    }
                    if local_setup {
                        if is_planning {
                            div { class: "provisioning-planning", "checking automatic setup…" }
                        } else if failed_operation.is_none() && can_retry_local_setup {
                            button {
                                r#type: "button",
                                class: "btn provisioning-auto-setup",
                                disabled: page_busy,
                                onclick: {
                                    let mut begin_plan = begin_plan.clone();
                                    move |_| begin_plan(ProvisioningOperation::Add)
                                },
                                "set up automatically"
                            }
                        }
                    } else {
                        button {
                            r#type: "button",
                            class: "btn provisioning-update",
                            disabled: page_busy || is_planning,
                            onclick: {
                                let mut begin_plan = begin_plan.clone();
                                move |_| begin_plan(ProvisioningOperation::Update)
                            },
                            if is_planning { "planning…" } else { "update" }
                        }
                    }
                }
            } else if local_setup && is_planning {
                div { class: "provisioning-planning", "checking automatic setup…" }
            }

            if let Some(error) = current_error {
                PeerLine {
                    class: "action-error provisioning-error",
                    parts: vec![DetailPart::Peer(error)],
                }
            }
            if let Some(warning) = current_warning {
                PeerLine {
                    class: "host-warning provisioning-warning",
                    parts: vec![DetailPart::Peer(warning)],
                }
            }
            if let Some(error) = current_progress_error {
                PeerLine {
                    class: "action-error provisioning-read-error",
                    parts: vec![DetailPart::Peer(error)],
                }
            }
            if local_setup && let Some(remedy) = manual_remedy {
                PeerLine {
                    class: if automatic_local_remedy {
                        "host-remedy provisioning-manual secondary"
                    } else {
                        "host-remedy provisioning-manual"
                    },
                    parts: remedy,
                }
            }
        }
    }
}

/// Stable words for aggregate run state, also used as browser handles.
fn status_label(status: ProvisioningStatus) -> &'static str {
    match status {
        ProvisioningStatus::Running => "running",
        ProvisioningStatus::Completed => "completed",
        ProvisioningStatus::Failed => "failed",
    }
}

/// The operation label for a retained run; `None` occurs only in the idle
/// view, which the panel does not render as a run.
fn operation_label(operation: Option<ProvisioningOperation>) -> &'static str {
    match operation {
        Some(ProvisioningOperation::Add) => "setup",
        Some(ProvisioningOperation::Update) => "update",
        None => "idle",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The words painted in a run header must cover every rendered state and
    /// operation. Both are also exposed as stable data attributes, so a
    /// rename here is a contract change rather than copy editing. The valid
    /// idle view has no header and therefore does not belong in this test.
    #[test]
    fn run_labels_cover_the_wire_vocabulary() {
        assert_eq!(status_label(ProvisioningStatus::Running), "running");
        assert_eq!(status_label(ProvisioningStatus::Completed), "completed");
        assert_eq!(status_label(ProvisioningStatus::Failed), "failed");
        assert_eq!(operation_label(Some(ProvisioningOperation::Add)), "setup");
        assert_eq!(
            operation_label(Some(ProvisioningOperation::Update)),
            "update"
        );
    }
}
