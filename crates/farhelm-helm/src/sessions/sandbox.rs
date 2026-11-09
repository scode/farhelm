//! Session controls adapt existing whole-machine operations for disposable
//! Tensorlake hosts. Explicit Delete owns the resource, while Replace's internal
//! source removal remains a supervisor session operation. No member count is
//! needed to decide whether the user authorized destroying the whole sandbox.

use super::*;

/// Recipe placement has its own endpoint so an older helm cannot silently
/// ignore it and create the session on its ordinary default host.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SandboxCreateRequest {
    recipe: String,
    #[serde(default)]
    confirmed: bool,
    name_kind: Option<farhelm_proto::launcher::TemplateNameKind>,
    request: CreateReq,
}

/// Start one billed resource only after the user has accepted its consequence.
/// Response headers name existing host progress immediately. The ordinary JSON
/// body arrives when the owned setup task finishes, including after successful
/// failure cleanup has removed that host's progress. No retained job registry is
/// needed, and dropping this response cannot cancel the provisioning task.
pub(crate) async fn create_sandbox_session(
    State(state): State<Arc<AppState>>,
    axum::Json(request): axum::Json<SandboxCreateRequest>,
) -> axum::response::Response {
    let result = crate::run_owned(async move {
        let settings = state.tensorlake.settings().await?;
        if !request.confirmed && !settings.create_without_asking {
            return Err(invalid_request(
                "confirm creation of a billed sandbox before launching".into(),
            ));
        }
        Arc::clone(&state.provisioning)
            .start_session_sandbox(state, request.recipe, request.request, request.name_kind)
            .await
    })
    .await;
    let (accepted, result) = match result {
        Ok(accepted) => accepted,
        Err(error) => return crate::provisioning::provisioning_error(error),
    };
    use futures_util::StreamExt as _;
    // A first whitespace frame flushes progress headers while leaving one
    // valid JSON value for the client's ordinary decoder to read at the end.
    let prefix = futures_util::stream::once(async { Ok::<_, std::io::Error>("\n".to_string()) });
    let body = futures_util::stream::once(async move {
        let reply = match result.await {
            Ok(Ok(session)) => serde_json::json!({ "session": session }),
            Ok(Err(error)) => serde_json::json!({ "error": error.to_string() }),
            Err(_) => {
                serde_json::json!({ "error": "The launch task ended without a result. Check Hosts for the named sandbox before launching again.", "uncertain": true })
            }
        };
        Ok::<_, std::io::Error>(reply.to_string())
    });
    axum::response::Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .header("x-farhelm-sandbox-host", accepted.host_id.to_string())
        .header("x-farhelm-sandbox-run", accepted.run_id)
        .body(axum::body::Body::from_stream(prefix.chain(body)))
        .expect("launch headers contain only a host number and a generated UUID")
}

/// Refuse session-only deletion of a resource whose files belong to its user.
/// The same refusal closes the agent helper and an older page's Delete path.
pub(super) fn user_only_delete() -> anyhow::Error {
    anyhow::Error::new(SupervisorError::helm(
        ErrorKind::Conflict,
        "Only the user can delete a sandbox session; confirm Delete session and sandbox in Farhelm"
            .to_string(),
    ))
}

/// Return None for an ordinary host; an owned host requires explicit consent
/// to end every process and discard every file, including unlisted leftovers.
/// Reuse host removal's lock, bounded vendor cleanup and cascading local cleanup.
pub(super) async fn delete_if_owned(
    state: &AppState,
    key: &store::SessionKey,
    confirmed: bool,
    guard: farhelm_proto::DeleteGuard,
) -> anyhow::Result<Option<farhelm_proto::SessionDeleteOutcome>> {
    let row = crate::hosts::registry_row(state, key.host).await?;
    if !row.is_session_owned() {
        if confirmed {
            return Err(anyhow::Error::new(SupervisorError::helm(
                ErrorKind::Conflict,
                "this session no longer belongs to a disposable sandbox".into(),
            )));
        }
        return Ok(None);
    }
    if !confirmed || guard != farhelm_proto::DeleteGuard::Unconditional {
        return Err(user_only_delete());
    }
    // Resolve the selected session's existing reference, without contacting a
    // suspended supervisor or interpreting cache absence as a membership proof.
    resolve_owner(state, key).await?;
    crate::hosts::remove_host_checked(state, key.host, |now| {
        if now.is_session_owned() {
            Ok(())
        } else {
            Err(user_only_delete())
        }
    })
    .await?;
    Ok(Some(farhelm_proto::SessionDeleteOutcome::default()))
}

/// Stop and suspend is one helm-owned user operation. A dropped browser must
/// not abandon the second half after it has already stopped the user's agent.
pub(crate) async fn stop_and_suspend(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
    axum::Json(request): axum::Json<StopRequest>,
) -> impl IntoResponse {
    let result = crate::run_owned(async move {
        let key = resolve_session_ref(&state, &id).await?;
        if !crate::hosts::registry_row(&state, key.host)
            .await?
            .is_session_owned()
        {
            return Err(invalid_request("Stop and suspend is available only for sandbox sessions".into()));
        }
        let outcome = do_stop_session(
            &state,
            &key,
            request.refuse_if_tabs_open,
            request.refuse_if_agent_running,
        )
        .await?;
        if let Some(notice) = outcome.notice {
            return Err(anyhow::Error::new(SupervisorError::helm(
                ErrorKind::Conflict, format!("The agent stopped, but cleanup was incomplete; the sandbox was not suspended: {notice}"),
            )));
        }
        crate::tensorlake::lifecycle::change(&state, key.host, false).await
            .context("The agent stopped, but the sandbox was not suspended")
    })
    .await;
    match result {
        Ok(()) => axum::Json(farhelm_proto::SessionStopOutcome::default()).into_response(),
        Err(error) => http_error(error),
    }
}
