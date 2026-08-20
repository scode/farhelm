//! Discovery-first supervisor provisioning and its host-scoped REST state.

use super::backend::BackendFailure;
use super::plan::{ProvisioningOperation, ProvisioningPlan};
use crate::store::HostId;
use crate::{AppState, http_error};
use axum::extract::{Path as AxPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Request-level refusals whose HTTP status must not depend on prose.
#[derive(Debug, thiserror::Error)]
pub(super) enum ProvisioningRequestError {
    #[error("{0}")]
    InvalidProbe(String),
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

/// A frozen UPDATE plan. Posting its opaque id back to the same host route
/// consumes it exactly once and starts the run.
#[derive(Debug, Serialize)]
pub(crate) struct UpdatePlanResponse {
    pub(super) probe_id: String,
    pub(super) plan: ProvisioningPlan,
    pub(super) confirmation: String,
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

/// One action's retained status and bounded explanatory message.
#[derive(Debug, Clone, Serialize)]
pub(super) struct StepView {
    pub(super) step: String,
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

    pub(super) fn for_plan(host_id: HostId, run_id: String, plan: &ProvisioningPlan) -> Self {
        Self {
            host_id,
            run_id: Some(run_id),
            operation: Some(plan.operation),
            status: RunStatus::Running,
            steps: plan
                .actions
                .iter()
                .map(|action| StepView {
                    step: action.label().to_string(),
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
fn provisioning_error(error: anyhow::Error) -> Response {
    if error.is::<BackendFailure>() {
        return (StatusCode::BAD_GATEWAY, format!("{error:#}")).into_response();
    }
    if let Some(request) = error.downcast_ref::<ProvisioningRequestError>() {
        let status = match request {
            ProvisioningRequestError::InvalidProbe(_) => StatusCode::BAD_REQUEST,
            ProvisioningRequestError::UnknownPlan | ProvisioningRequestError::Busy(_) => {
                StatusCode::CONFLICT
            }
        };
        return (status, format!("{error:#}")).into_response();
    }
    http_error(error)
}
