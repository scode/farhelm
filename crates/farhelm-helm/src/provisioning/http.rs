//! Discovery-first supervisor provisioning and its host-scoped REST state.

use super::backend::BackendFailure;
use super::plan::{PlanSteps, ProvisioningPlan};
use super::service::ProvisioningService;
use crate::store::HostId;
use crate::{AppState, http_error};
use axum::extract::{Path as AxPath, State};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Request-level refusals whose HTTP status must not depend on prose.
#[derive(Debug, thiserror::Error)]
pub(super) enum ProvisioningRequestError {
    #[error("{0}")]
    InvalidProbe(String),
    /// A valid request cannot proceed until the operator follows the
    /// supplied handoff or manual-install instruction.
    #[error("{0}")]
    Refused(String),
    /// Uninstall cannot (or need not) remove the installation, and removing
    /// the host from the list is the remedy: forgetting it, which leaves the
    /// machine untouched, or for a Tensorlake host deleting its sandbox.
    #[error("{0}")]
    RemoveInstead(String),
    #[error("the provisioning plan is unknown or has already been used; probe again")]
    UnknownPlan,
    #[error("host {0} already has a provisioning operation in flight")]
    Busy(HostId),
}

/// What the client wants discovery to inspect.
///
/// The tagged target keeps a remote host literally named `local` distinct
/// from the reserved direct-process transport.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProbeRequest {
    pub(super) target: ProbeDestination,
    #[serde(default)]
    pub(super) remote_farhelm: Option<String>,
    #[serde(default)]
    pub(super) remote_state_dir: Option<String>,
    /// Present only on an add from the add host dialog, which asks for a NEW
    /// host entry (see `crate::store::NewHostEntry`): an existing entry for
    /// the destination is then refused instead of set up again, and the new
    /// entry gets the alias. Registered setup and legacy destination-only discovery leave it out to converge.
    #[serde(default)]
    pub(super) new_entry: Option<NewEntryRequest>,
}

/// The add host dialog's new-entry marker and its optional alias, as sent.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NewEntryRequest {
    #[serde(default)]
    pub(super) alias: Option<String>,
    /// An approved agent Add may discover a host. Its approval does not
    /// authorize an ownership change.
    #[serde(default)]
    pub(super) from_agent: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum ProbeDestination {
    Local,
    Ssh {
        destination: String,
        /// A legacy request omits this field and inherits the stored mode.
        #[serde(default)]
        tailscale: Option<bool>,
    },
    /// Set up one registered SSH host. Keep the destination from the initiating UI action across a sign-in replay.
    SshHost {
        host: HostId,
        destination: String,
    },
    Tensorlake {
        name: String,
        recipe: String,
    },
    TensorlakeHost {
        host: HostId,
    },
}

/// The opaque reference returned with a confirmed provisioning plan.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProvisionRequest {
    pub(super) probe_id: String,
}

/// Discovery either registers an answering supervisor, offers one concrete
/// plan, or explains why this host remains a manual install. The shape is
/// `farhelm-proto`'s, which the GUI decodes; the plan in it is the helm's own.
pub(super) type ProbeResponse = farhelm_proto::provisioning::ProbeReply<ProvisioningPlan>;

pub(crate) use farhelm_proto::provisioning::RunAccepted;

/// A frozen UPDATE or UNINSTALL plan with the helm's step descriptions.
/// Posting its opaque id back to the same host route consumes it exactly once
/// and starts the run; the extra steps field does not change that authority.
#[derive(Debug, Serialize)]
pub(crate) struct HostPlanResponse {
    pub(super) probe_id: String,
    pub(super) plan: ProvisioningPlan,
    pub(super) confirmation: String,
    pub(super) steps: PlanSteps,
}

// The progress shapes live in `farhelm-proto`, because the UI channel pushes them to the GUI; the helm builds them
// through [`ProgressViews`].
pub(crate) use farhelm_proto::provisioning::{ProvisioningView, RunStatus, StepStatus, StepView};

/// `GET /api/provisioning`: every run this helm process retains, one per host. A host with no retained run is absent
/// rather than listed as idle, so the list is exactly the progress there is to show. Besides the registered hosts' runs
/// it holds the failed environment launches kept after their cleanup removed the host, under that removed host's id
/// (`ProvisioningMemory::ended_launches`).
///
/// Unlike the entries, this wrapper is not in `farhelm-proto`: the GUI decodes the [`ProvisioningView`] entries the UI
/// channel splits out of it, never the wrapper.
#[derive(Debug, Serialize)]
pub(crate) struct ProvisioningRuns {
    pub(super) runs: Vec<ProvisioningView>,
}

/// The helm's constructors for a host's progress view.
pub(super) trait ProgressViews: Sized {
    /// An explicit idle view for a valid host that has never been provisioned by this helm process.
    fn idle(host_id: HostId) -> Self;
    /// Start progress from the plan's own descriptions so status updates keep
    /// the names and paths the user saw before authorizing the run.
    fn for_plan(host_id: HostId, run_id: String, plan: &ProvisioningPlan) -> Self;
}

impl ProgressViews for ProvisioningView {
    fn idle(host_id: HostId) -> Self {
        Self {
            host_id,
            run_id: None,
            operation: None,
            status: RunStatus::Completed,
            steps: Vec::new(),
            message: Some("no provisioning run has been recorded for this host".to_string()),
            outcome_unknown: false,
        }
    }

    fn for_plan(host_id: HostId, run_id: String, plan: &ProvisioningPlan) -> Self {
        let steps = plan.steps();
        Self {
            host_id,
            run_id: Some(run_id),
            operation: Some(plan.operation),
            status: RunStatus::Running,
            steps: steps
                .steps
                .into_iter()
                .map(|step| StepView {
                    step: step.id,
                    name: step.name,
                    paths: step.paths,
                    explanation: step.explanation,
                    status: StepStatus::Pending,
                    message: None,
                })
                .collect(),
            message: None,
            outcome_unknown: false,
        }
    }
}

/// `POST /api/hosts/probe` — discover and register as-is, or return the
/// exact plan a later confirmation may consume.
///
/// Answers within the host reply bound (`crate::sessions::HOST_REPLY_BOUND`):
/// an SSH probe and the inspection after it can take far longer than the GUI's
/// channel lets one command wait (15 seconds per connection attempt, then
/// inspection commands of up to 30 seconds each, behind any other probes
/// holding the planning slots). The probe itself carries on after the timeout
/// reply, so a supervisor that answers late is still registered and appears in
/// the pushed hosts list; a plan found late is retained but never shown, and
/// the user probes again.
pub(crate) async fn probe_host(
    State(state): State<Arc<AppState>>,
    axum::Json(request): axum::Json<ProbeRequest>,
) -> Response {
    planning_reply(
        state,
        move |state| async move {
            let result = if matches!(
                request.target,
                ProbeDestination::Tensorlake { .. } | ProbeDestination::TensorlakeHost { .. }
            ) {
                state
                    .provisioning
                    .probe_tensorlake(request, state.tensorlake.clone())
                    .await
            } else {
                state.provisioning.probe(request).await
            };
            match result {
                Ok(response) => axum::Json(response).into_response(),
                Err(error) => provisioning_error(error),
            }
        },
        format!(
            "the check did not finish within {} seconds; it carries on, and if it finds a running \
             Farhelm supervisor the host is added to the hosts list, otherwise try again",
            crate::sessions::HOST_REPLY_BOUND.as_secs()
        ),
    )
    .await
}

/// `POST /api/hosts/provision` — consume one confirmed ADD plan, register
/// its host first, and return before background convergence finishes.
pub(crate) async fn provision_host(
    State(state): State<Arc<AppState>>,
    axum::Json(request): axum::Json<ProvisionRequest>,
) -> Response {
    match state.provisioning.start_add(request).await {
        Ok(run) => (StatusCode::ACCEPTED, axum::Json(run)).into_response(),
        Err(error) => provisioning_error(error),
    }
}

/// Confirm the ownership change offered by Add host. Discovery has
/// already proved the dial coordinates and this endpoint can only
/// consume its opaque approval id once.
pub(crate) async fn take_over_discovered_host(
    State(state): State<Arc<AppState>>,
    axum::Json(request): axum::Json<ProvisionRequest>,
) -> Response {
    match state.provisioning.confirm_takeover(request).await {
        Ok(response) => axum::Json(response).into_response(),
        Err(error) => provisioning_error(error),
    }
}

/// `POST /api/hosts/{id}/update` — plan without a body, then consume the
/// returned opaque plan when the same route receives its confirmation body.
///
/// Only planning answers within the host reply bound
/// (`crate::sessions::HOST_REPLY_BOUND`): it probes the host the way
/// [`probe_host`] does and can wait as long. A plan found after the timeout
/// reply is retained but never confirmed, and the user asks for Update again.
/// The confirmation consumes the plan, claims the host and starts the run on
/// its own task. It needs no bound, because it does not wait for the host's
/// provisioning lock: it refuses at once with a 409 while another operation
/// holds it (`ProvisioningService::start_update`; SPEC_impl.md, "The UI
/// channel").
pub(crate) async fn update_host(
    State(state): State<Arc<AppState>>,
    AxPath(host): AxPath<HostId>,
    request: Option<axum::Json<ProvisionRequest>>,
) -> Response {
    match request {
        Some(axum::Json(request)) => match state.provisioning.start_update(host, request).await {
            Ok(run) => (StatusCode::ACCEPTED, axum::Json(run)).into_response(),
            Err(error) => provisioning_error(error),
        },
        None => {
            planning_reply(
                state,
                move |state| async move {
                    match state.provisioning.plan_update_for_ui(host).await {
                        Ok(plan) => axum::Json(plan).into_response(),
                        Err(error) => provisioning_error(error),
                    }
                },
                format!(
                    "the host did not answer within {} seconds while Farhelm prepared the \
                     update; nothing was changed, so try again",
                    crate::sessions::HOST_REPLY_BOUND.as_secs()
                ),
            )
            .await
        }
    }
}

/// `POST /api/hosts/{id}/uninstall` — the update route's shape: plan
/// without a body (the response carries the confirmation the user must
/// accept), then consume the returned opaque plan when the same route
/// receives its confirmation body.
///
/// As for [`update_host`], only planning answers within the host reply
/// bound (`crate::sessions::HOST_REPLY_BOUND`): it reads the host's session
/// list, probes the destination and inspects the installation, each of which
/// can wait on the host longer than the GUI's channel lets one command wait.
/// A plan found after the timeout reply is retained but never confirmed, and
/// the user chooses uninstall again. The confirmation consumes the plan and
/// starts the run, which checks the host again on its own task.
pub(crate) async fn uninstall_host(
    State(state): State<Arc<AppState>>,
    AxPath(host): AxPath<HostId>,
    request: Option<axum::Json<ProvisionRequest>>,
) -> Response {
    match request {
        Some(axum::Json(request)) => {
            match state.provisioning.start_uninstall(host, request).await {
                Ok(run) => (StatusCode::ACCEPTED, axum::Json(run)).into_response(),
                Err(error) => provisioning_error(error),
            }
        }
        None => {
            planning_reply(
                state,
                move |state| async move {
                    match state.provisioning.plan_uninstall(host).await {
                        Ok(plan) => axum::Json(plan).into_response(),
                        Err(error) => provisioning_error(error),
                    }
                },
                format!(
                    "the host did not answer within {} seconds while Farhelm checked what \
                     uninstalling would remove; nothing was changed, so choose uninstall again",
                    crate::sessions::HOST_REPLY_BOUND.as_secs()
                ),
            )
            .await
        }
    }
}

/// `GET /api/hosts/{id}/provisioning` — re-read progress after a feed bump.
pub(crate) async fn provisioning_state(
    State(state): State<Arc<AppState>>,
    AxPath(host): AxPath<HostId>,
) -> Response {
    match state.provisioning.view(host).await {
        Ok(view) => axum::Json(view).into_response(),
        Err(error) => http_error(error),
    }
}

/// `GET /api/provisioning` — every host's retained run in one read, which the UI channel pushes as its
/// host-progress collection (the per-host route stays for callers that want one host), including the failed
/// environment launches kept for removed hosts ([`super::ProvisioningService::runs`]).
pub(crate) async fn provisioning_runs(State(state): State<Arc<AppState>>) -> Response {
    axum::Json(state.provisioning.runs().await).into_response()
}

/// Map provisioning's own typed refusals without parsing their diagnostics.
/// Only the Remove remedy carries a machine-readable header: the response
/// body can include remote text and is never authority for a UI action.
pub(crate) fn provisioning_error(error: anyhow::Error) -> Response {
    if let Some(start) = error.downcast_ref::<super::setup_sign_in::Pending>() {
        let mut reply = pending_sign_in(start.attempt.id.clone());
        reply.extensions_mut().insert(start.clone());
        return reply;
    }
    if error.is::<BackendFailure>() {
        return (StatusCode::BAD_GATEWAY, format!("{error:#}")).into_response();
    }
    if let Some(request) = error.downcast_ref::<ProvisioningRequestError>() {
        let status = match request {
            ProvisioningRequestError::InvalidProbe(_) => StatusCode::BAD_REQUEST,
            ProvisioningRequestError::Refused(_)
            | ProvisioningRequestError::RemoveInstead(_)
            | ProvisioningRequestError::UnknownPlan
            | ProvisioningRequestError::Busy(_) => StatusCode::CONFLICT,
        };
        let mut response = (status, format!("{error:#}")).into_response();
        if matches!(request, ProvisioningRequestError::RemoveInstead(_)) {
            response.headers_mut().insert(
                farhelm_proto::http::UNINSTALL_REMEDY_HEADER,
                HeaderValue::from_static(farhelm_proto::http::UNINSTALL_REMEDY_REMOVE),
            );
        }
        return response;
    }
    http_error(error)
}

/// A pending reply names the check that owns the question and the eventual completion.
fn pending_sign_in(attempt: String) -> Response {
    (
        StatusCode::ACCEPTED,
        axum::Json(farhelm_proto::ui_channel::PendingSshSignIn { attempt }),
    )
        .into_response()
}

/// Keep the existing deadline and owned batch work. An interactive preflight only starts when the pending reply
/// has won that deadline. A late 504's detached planning task can never strand a question for an unknown draft.
async fn planning_reply<F>(
    state: Arc<AppState>,
    work: impl FnOnce(Arc<AppState>) -> F,
    timed_out: String,
) -> Response
where
    F: std::future::Future<Output = Response> + Send + 'static,
{
    let reply =
        crate::sessions::within_host_reply_bound_saying(work(Arc::clone(&state)), timed_out).await;
    activate_pending_sign_in(&state.provisioning, reply)
}

/// The captured response won the command deadline. Start or join its check before serializing the final id.
pub(super) fn activate_pending_sign_in(
    service: &Arc<ProvisioningService>,
    mut reply: Response,
) -> Response {
    if let Some(start) = reply
        .extensions_mut()
        .remove::<super::setup_sign_in::Pending>()
        && let Some(setup) = &service.setup_sign_in
    {
        // Concurrent manual and automatic requests may have prepared different ids. The winning check's id
        // must reach the desktop so the explicit request can focus its question and replay on its completion.
        return pending_sign_in(setup.start_manual(start, service));
    }
    reply
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Remove action is offered only for the helm's typed remedy, so a
    /// remote diagnostic that happens to mention Remove cannot alter the UI.
    #[test]
    fn only_typed_uninstall_remedy_sets_the_remove_header() {
        let remove = provisioning_error(anyhow::Error::new(
            ProvisioningRequestError::RemoveInstead("remove this host".into()),
        ));
        assert_eq!(remove.status(), StatusCode::CONFLICT);
        assert_eq!(
            remove
                .headers()
                .get(farhelm_proto::http::UNINSTALL_REMEDY_HEADER),
            Some(&HeaderValue::from_static(
                farhelm_proto::http::UNINSTALL_REMEDY_REMOVE
            ))
        );
        let plain = provisioning_error(anyhow::Error::new(ProvisioningRequestError::Refused(
            "remove this host".into(),
        )));
        assert_eq!(plain.status(), StatusCode::CONFLICT);
        assert!(
            !plain
                .headers()
                .contains_key(farhelm_proto::http::UNINSTALL_REMEDY_HEADER)
        );
    }
}
