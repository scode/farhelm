//! Authenticated, read-only help for manually opening the helm's SSH masters.
//!
//! The response deliberately contains ready-to-paste commands, not a state
//! directory clients would have to interpret. No probe, authentication or
//! connection starts here; even unreachable registered destinations are useful.

use axum::{extract::State, response::IntoResponse};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    AppState,
    aggregate::host_display_name,
    ssh::{SharedConnection, manual_master_command},
    store::HostKind,
};

/// An optional unregistered destination from the still-open add-host form.
/// Duplicate/alias registration checks do not apply to this read-only request.
#[derive(Deserialize)]
pub(crate) struct Request {
    destination: Option<String>,
}

/// One destination's commands, with unavailable connection kinds omitted.
#[derive(Serialize)]
struct HostCommands {
    name: String,
    destination: String,
    sessions: Option<String>,
    setup: Option<String>,
}

/// All inability-to-share explanations belong to the helm path, not each host.
/// Keeping them once at the top also handles a helm with no SSH hosts yet.
#[derive(Serialize)]
struct Help {
    unavailable: Vec<String>,
    hosts: Vec<HostCommands>,
    extra: Option<HostCommands>,
}

/// Compose both purposes from one source, including for a destination not yet added.
fn for_host(name: String, destination: String, path: &std::path::Path) -> HostCommands {
    HostCommands {
        sessions: manual_master_command(&destination, path, SharedConnection::Supervisor).ok(),
        setup: manual_master_command(&destination, path, SharedConnection::Provisioning).ok(),
        name,
        destination,
    }
}

/// Return an authenticated snapshot without changing hosts or starting SSH.
/// An invalid extra destination refuses the whole request before composition;
/// an empty one is an intentional add-host placeholder, not a registration.
pub(crate) async fn commands(
    State(state): State<Arc<AppState>>,
    axum::Json(request): axum::Json<Request>,
) -> axum::response::Response {
    if let Some(destination) = &request.destination
        && !destination.is_empty()
        && !crate::store::destination_is_usable(destination)
    {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({
                "error": "enter an ssh destination without a leading dash or NUL"
            })),
        )
            .into_response();
    }
    let path = state.provisioning.local_state_dir();
    let unavailable = [
        ("sessions connection", SharedConnection::Supervisor),
        (
            "setup and updates connection",
            SharedConnection::Provisioning,
        ),
    ]
    .into_iter()
    .filter_map(|(label, kind)| {
        manual_master_command("", path, kind)
            .err()
            .map(|reason| format!("{label}: {reason}"))
    })
    .collect();
    let rows = match state.store.list_hosts().await {
        Ok(rows) => rows,
        Err(error) => return crate::http_error(error),
    };
    let hosts = rows
        .iter()
        .filter(|row| row.kind == HostKind::Ssh)
        .map(|row| {
            for_host(
                host_display_name(row.kind, row.destination.as_deref(), row.alias.as_deref()),
                row.destination.clone().expect("SSH row has a destination"),
                path,
            )
        })
        .collect();
    let extra = request
        .destination
        .filter(|dest| !dest.is_empty())
        .map(|dest| for_host("the host you're adding".into(), dest, path));
    axum::Json(Help {
        unavailable,
        hosts,
        extra,
    })
    .into_response()
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    /// Reach the protected production router with the idle harness's accepted
    /// Host header, so an auth assertion observes device admission, not the
    /// earlier DNS-rebinding refusal. Composition is never called directly.
    fn request(destination: Option<&str>) -> Request<Body> {
        Request::post("/api/ssh-connections")
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({"destination": destination}).to_string(),
            ))
            .unwrap()
    }

    /// Help lists registered SSH destinations, excludes local, and accepts an
    /// unregistered extra without adding it. Refusals traverse authentication
    /// and the same destination grammar as registration.
    #[farhelm_testtrace::test]
    async fn ssh_help_lists_and_validates_without_mutation() {
        let helm = crate::rest_harness::idle_helm().await;
        helm.store
            .add_ssh_host("example", None, None)
            .await
            .unwrap();
        let rows = helm.store.list_hosts().await.unwrap();
        assert_eq!(rows.len(), 2, "premise: local plus one registered SSH host");
        let denied = helm
            .unauthenticated_router()
            .oneshot(request(None))
            .await
            .unwrap();
        assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
        let response = helm
            .router()
            .oneshot(request(Some("new'destination")))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 64 * 1024).await.unwrap())
                .unwrap();
        assert_eq!(body["hosts"].as_array().unwrap().len(), 1);
        assert_eq!(body["hosts"][0]["destination"], "example");
        assert_eq!(body["extra"]["destination"], "new'destination");
        for kind in ["sessions", "setup"] {
            let argv = shell_words::split(body["extra"][kind].as_str().unwrap()).unwrap();
            assert_eq!(argv.last().unwrap(), "new'destination");
            assert_eq!(argv[argv.len() - 2], "--");
        }
        for invalid in ["-oProxyCommand=bad", "bad\0destination"] {
            assert_eq!(
                helm.router()
                    .oneshot(request(Some(invalid)))
                    .await
                    .unwrap()
                    .status(),
                StatusCode::BAD_REQUEST
            );
        }
        let blank = helm.router().oneshot(request(Some(""))).await.unwrap();
        assert_eq!(blank.status(), StatusCode::OK);
        assert_eq!(
            helm.store.list_hosts().await.unwrap(),
            rows,
            "help never registers the extra host"
        );
    }
}
