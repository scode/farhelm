//! Helm-local session group writes. The ordinary viewer-session preamble
//! resolves paired keys and admits stale hosts while refusing retired rows.

use axum::Json;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use farhelm_proto::ErrorKind;
use farhelm_proto::session_requests::{
    NewSessionGroup, SessionGroupMembership, UpdateSessionGroup,
};
use std::sync::Arc;

use super::{MovePinReq, PinPlacement, order_move_reply, viewer_session};
use crate::store::session_groups::GroupError;
use crate::{AppState, SupervisorError, http_error};

/// Moving a row is a viewer write, so the host need never be contacted.
/// The Delete gate prevents a late move from recreating its membership.
pub(crate) async fn set_group(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<SessionGroupMembership>,
) -> Response {
    let _session_mutation = state.tunnels.mutation.lock().await;
    let (key, _) = match viewer_session(&state, &id).await {
        Ok(session) => session,
        Err(error) => return http_error(error),
    };
    let result = state.store.set_session_group(key, request).await;
    match result {
        Err(error) if error.downcast_ref::<GroupError>().is_some() => group_error(error),
        result => order_move_reply(&state, result),
    }
}

/// Resolve all targets before writing the single group. A refused target
/// must not leave behind a group that holds only part of the marked set.
pub(crate) async fn create_group(
    State(state): State<Arc<AppState>>,
    Json(request): Json<NewSessionGroup>,
) -> Response {
    if request.sessions.len() > farhelm_proto::LIST_SESSIONS_CAP {
        return http_error(
            SupervisorError::helm(
                ErrorKind::InvalidRequest,
                "too many sessions selected for one group".into(),
            )
            .into(),
        );
    }
    let _session_mutation = state.tunnels.mutation.lock().await;
    let mut keys = Vec::with_capacity(request.sessions.len());
    for id in &request.sessions {
        match viewer_session(&state, id).await {
            Ok((key, _)) => keys.push(key),
            Err(error) => return http_error(error),
        }
    }
    match state.store.create_session_group(request.name, keys).await {
        Ok(id) => {
            state.manager.events().bump();
            Json(serde_json::json!({ "id": id })).into_response()
        }
        Err(error) => group_error(error),
    }
}

/// Name and collapse writes are sparse so a window never sends back another
/// client's earlier choice along with its own.
pub(crate) async fn update_group(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(patch): Json<UpdateSessionGroup>,
) -> Response {
    write_reply(&state, state.store.update_session_group(id, patch).await)
}

/// Ungroup is a single store transaction, including the pins it appends.
pub(crate) async fn ungroup(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Response {
    write_reply(&state, state.store.ungroup_sessions(id).await)
}

/// Group neighbors are group IDs. The relative move still uses the same
/// placement semantics and response as the pin-order route.
pub(crate) async fn move_group(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(request): Json<MovePinReq<i64>>,
) -> Response {
    if request.areas.is_some() {
        return http_error(
            SupervisorError::helm(
                ErrorKind::InvalidRequest,
                "group order does not take pin areas".into(),
            )
            .into(),
        );
    }
    let result = state
        .store
        .move_session_group(
            id,
            request.neighbor,
            matches!(request.placement, PinPlacement::Before),
        )
        .await;
    order_move_reply(&state, result)
}

/// Repeat writes that changed nothing leave the fleet revision alone.
fn write_reply(state: &AppState, result: anyhow::Result<bool>) -> Response {
    match result {
        Ok(changed) => {
            if changed {
                state.manager.events().bump();
            }
            Json(serde_json::json!({})).into_response()
        }
        Err(error) => group_error(error),
    }
}

/// Product errors travel through the existing REST refusal shape.
fn group_error(error: anyhow::Error) -> Response {
    let Some(kind) = error
        .downcast_ref::<GroupError>()
        .map(|reason| match reason {
            GroupError::Missing => ErrorKind::Conflict,
            GroupError::DuplicateName
            | GroupError::EmptyName
            | GroupError::LongName
            | GroupError::RosterFull
            | GroupError::EmptySelection => ErrorKind::InvalidRequest,
        })
    else {
        return http_error(error);
    };
    http_error(SupervisorError::helm(kind, error.to_string()).into())
}
