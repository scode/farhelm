//! Discovery-first supervisor provisioning and its host-scoped REST state.

use super::backend::BackendFailure;
use super::plan::{PlanStepPath, PlanSteps, ProvisioningOperation, ProvisioningPlan};
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
    /// Uninstall cannot safely remove the installation, but forgetting the
    /// host leaves it untouched and is an available remedy.
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
    /// entry gets the alias. Every other caller of this request (a host
    /// row's "set up again", update confirmation, a browser tab from before
    /// this field existed) leaves it out and keeps the converging behavior.
    #[serde(default)]
    pub(super) new_entry: Option<NewEntryRequest>,
}

/// The add host dialog's new-entry marker and its optional alias, as sent.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NewEntryRequest {
    #[serde(default)]
    pub(super) alias: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum ProbeDestination {
    Local,
    Ssh { destination: String },
}

/// The opaque reference returned with a confirmed provisioning plan.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProvisionRequest {
    pub(super) probe_id: String,
}

/// Discovery either registers an answering supervisor, offers one concrete
/// plan, or explains why this host remains a manual install.
#[derive(Debug, Serialize)]
#[serde(tag = "result", rename_all = "kebab-case")]
pub(super) enum ProbeResponse {
    Discovered {
        host_id: HostId,
        build_version: String,
        host_identity: Option<String>,
    },
    Provisionable {
        probe_id: String,
        plan: ProvisioningPlan,
        confirmation: String,
        /// Indirection keeps discovery's small outcomes small in memory;
        /// JSON still contains the same additive step list beside the plan.
        steps: Box<PlanSteps>,
    },
    Manual {
        reason: String,
    },
}

/// Identity returned before a long provision or update continues in the
/// background.
#[derive(Debug, Serialize)]
pub(crate) struct RunAccepted {
    pub(super) host_id: HostId,
    pub(super) run_id: String,
}

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

/// One action's observable progress. Completed runs remain readable through
/// the host-scoped GET until a later run replaces them.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(super) enum StepStatus {
    Pending,
    Running,
    Completed,
    Skipped,
    Degraded,
    Failed,
}

/// One action's retained status and bounded explanatory message. The helm
/// carries its plan wording into progress; clients never name steps from ids.
#[derive(Debug, Clone, Serialize)]
pub(super) struct StepView {
    pub(super) step: String,
    pub(super) name: String,
    pub(super) paths: Vec<PlanStepPath>,
    pub(super) explanation: Option<String>,
    pub(super) status: StepStatus,
    pub(super) message: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(super) enum RunStatus {
    Running,
    Completed,
    Failed,
}

/// What `GET /api/hosts/{id}/provisioning` returns.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProvisioningView {
    pub(super) host_id: HostId,
    pub(super) run_id: Option<String>,
    pub(super) operation: Option<ProvisioningOperation>,
    pub(super) status: RunStatus,
    pub(super) steps: Vec<StepView>,
    pub(super) message: Option<String>,
}

impl ProvisioningView {
    pub(super) fn idle(host_id: HostId) -> Self {
        Self {
            host_id,
            run_id: None,
            operation: None,
            status: RunStatus::Completed,
            steps: Vec::new(),
            message: Some("no provisioning run has been recorded for this host".to_string()),
        }
    }

    /// Start progress from the plan's own descriptions so status updates keep
    /// the names and paths the user saw before authorizing the run.
    pub(super) fn for_plan(host_id: HostId, run_id: String, plan: &ProvisioningPlan) -> Self {
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
        }
    }
}

/// `POST /api/hosts/probe` — discover and register as-is, or return the
/// exact plan a later confirmation may consume.
pub(crate) async fn probe_host(
    State(state): State<Arc<AppState>>,
    axum::Json(request): axum::Json<ProbeRequest>,
) -> Response {
    match state.provisioning.probe(request).await {
        Ok(response) => axum::Json(response).into_response(),
        Err(error) => provisioning_error(error),
    }
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

/// `POST /api/hosts/{id}/update` — plan without a body, then consume the
/// returned opaque plan when the same route receives its confirmation body.
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
        None => match state.provisioning.plan_update(host).await {
            Ok(plan) => axum::Json(plan).into_response(),
            Err(error) => provisioning_error(error),
        },
    }
}

/// `POST /api/hosts/{id}/uninstall` — the update route's shape: plan
/// without a body (the response carries the confirmation the user must
/// accept), then consume the returned opaque plan when the same route
/// receives its confirmation body.
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
        None => match state.provisioning.plan_uninstall(host).await {
            Ok(plan) => axum::Json(plan).into_response(),
            Err(error) => provisioning_error(error),
        },
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

/// Map provisioning's own typed refusals without parsing their diagnostics.
/// Only the Remove remedy carries a machine-readable header: the response
/// body can include remote text and is never authority for a UI action.
fn provisioning_error(error: anyhow::Error) -> Response {
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
