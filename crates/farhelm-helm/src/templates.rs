//! The helm's launch templates API (SPEC.md, Launch templates):
//! `GET /api/templates`, `PUT /api/templates/{name}` and
//! `DELETE /api/templates/{name}`.
//!
//! Templates belong to the helm, one catalog for every host it manages. The
//! helm checks only a template's shape when storing it (a usable name,
//! fields of a bounded size, no unknown field); whether its fields apply is
//! decided when it is applied, against the launcher as it is then
//! (`farhelm_proto::launcher::apply_template`). Writes come only from these
//! routes: no agent verb writes a template, because a template can carry a
//! command line every host the helm manages may later run (SPEC.md defers
//! agent template writes to the CLI permission prompts).

use std::sync::Arc;

use axum::extract::{Path as AxPath, State};
use axum::response::IntoResponse;
use farhelm_proto::launcher::{LaunchTemplate, TemplateFields, check_template_shape};

use crate::{AppState, http_error};

/// What `GET /api/templates` answers with.
#[derive(Debug, serde::Serialize)]
pub(crate) struct TemplatesView {
    pub(crate) templates: Vec<LaunchTemplate>,
}

/// A refusal the caller can fix, as the API's typed 400.
fn invalid(message: String) -> axum::response::Response {
    http_error(anyhow::Error::new(crate::SupervisorError {
        origin: crate::client::ErrorOrigin::Helm,
        kind: farhelm_proto::ErrorKind::InvalidRequest,
        message,
    }))
}

/// `GET /api/templates` — every template, by name.
pub(crate) async fn list_templates(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.store.launch_templates().await {
        Ok(templates) => axum::Json(TemplatesView { templates }).into_response(),
        Err(error) => http_error(error),
    }
}

/// `PUT /api/templates/{name}` — create the template named in the URL, or
/// replace it wholesale.
///
/// The URL is the name's only authority, so a body cannot redirect a write
/// to another template; the body is the fields alone. Last write wins
/// (SPEC.md wants no concurrency check). The write and its change hint run
/// in a detached task, so a request dropped after the commit still bumps
/// the hint, like every other helm write. Clients read templates when their
/// launcher or Templates panel opens; nothing pushes a template change into
/// a launcher already open in another client.
pub(crate) async fn put_template(
    State(state): State<Arc<AppState>>,
    AxPath(name): AxPath<String>,
    axum::Json(fields): axum::Json<TemplateFields>,
) -> impl IntoResponse {
    let template = LaunchTemplate { name, fields };
    if let Err(message) = check_template_shape(&template) {
        return invalid(message);
    }
    let task_state = Arc::clone(&state);
    let stored = template.clone();
    let mutation = tokio::spawn(async move {
        task_state.store.put_launch_template(stored).await?;
        task_state.manager.events().bump();
        Ok::<_, anyhow::Error>(())
    });
    match mutation.await {
        Err(error) => http_error(anyhow::Error::new(error).context("template write task panicked")),
        Ok(Err(error)) => http_error(error),
        Ok(Ok(())) => axum::Json(template).into_response(),
    }
}

/// `DELETE /api/templates/{name}` — remove a template. No session records
/// which templates made it (SPEC.md), so nothing else changes.
pub(crate) async fn delete_template(
    State(state): State<Arc<AppState>>,
    AxPath(name): AxPath<String>,
) -> impl IntoResponse {
    let task_state = Arc::clone(&state);
    let mutation = tokio::spawn(async move {
        let deleted = task_state.store.delete_launch_template(name).await?;
        if deleted {
            task_state.manager.events().bump();
        }
        Ok::<_, anyhow::Error>(deleted)
    });
    match mutation.await {
        Err(error) => {
            http_error(anyhow::Error::new(error).context("template delete task panicked"))
        }
        Ok(Err(error)) => http_error(error),
        Ok(Ok(true)) => axum::Json(serde_json::json!({})).into_response(),
        Ok(Ok(false)) => http_error(anyhow::Error::new(crate::SupervisorError {
            origin: crate::client::ErrorOrigin::Helm,
            kind: farhelm_proto::ErrorKind::NotFound,
            message: "no template has that name".to_string(),
        })),
    }
}

#[cfg(test)]
mod tests {
    use crate::rest_harness;
    use tower::ServiceExt;

    /// One request against the real router: its status and JSON body.
    async fn request(
        harness: &rest_harness::Harness,
        method: &str,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> (axum::http::StatusCode, serde_json::Value) {
        let builder = axum::http::Request::builder()
            .method(method)
            .uri(uri)
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json");
        let request = builder
            .body(axum::body::Body::from(
                body.map(|body| body.to_string()).unwrap_or_default(),
            ))
            .unwrap();
        let response = harness.router().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value = serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(&bytes).into()));
        (status, value)
    }

    /// Spec: templates are created and replaced by name with PUT (last write
    /// wins), listed by name, and deleted by name, a missing one answering
    /// 404; the stored fields come back exactly, `null` included.
    ///
    /// Why: this is the whole storage contract SPEC.md gives templates
    /// (unique names, last-write-wins edits), and the absent-versus-`null`
    /// distinction is what lets a template reset a choice, so it must
    /// survive the round trip through the database.
    #[farhelm_testtrace::test]
    async fn templates_are_put_listed_and_deleted_by_name() {
        let harness = rest_harness::idle_helm().await;
        let fields = serde_json::json!({"kind": "agent", "agent": "codex", "model": null});
        let (status, body) = request(
            &harness,
            "PUT",
            "/api/templates/my-codex",
            Some(fields.clone()),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK, "{body}");
        assert_eq!(body["fields"], fields);
        let (status, _) = request(
            &harness,
            "PUT",
            "/api/templates/my-codex",
            Some(serde_json::json!({"effort": "high"})),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK);
        let (status, body) = request(&harness, "GET", "/api/templates", None).await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            body["templates"],
            serde_json::json!([{"name": "my-codex", "fields": {"effort": "high"}}]),
            "a second PUT replaces the template wholesale"
        );
        let (status, _) = request(&harness, "DELETE", "/api/templates/my-codex", None).await;
        assert_eq!(status, axum::http::StatusCode::OK);
        let (status, _) = request(&harness, "DELETE", "/api/templates/my-codex", None).await;
        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
        let (_, body) = request(&harness, "GET", "/api/templates", None).await;
        assert_eq!(body["templates"], serde_json::json!([]));
    }

    /// Spec: a write with an unusable name or an unknown field is refused and
    /// stores nothing.
    ///
    /// Why: names are matched exactly by `tl:` and `--template`, so one with
    /// surrounding spaces could never be typed back; and an unknown field (a
    /// misspelling) silently dropped would store a template that does less
    /// than its author wrote.
    #[farhelm_testtrace::test]
    async fn a_template_write_with_a_bad_name_or_field_is_refused() {
        let harness = rest_harness::idle_helm().await;
        let (status, body) = request(
            &harness,
            "PUT",
            "/api/templates/%20padded",
            Some(serde_json::json!({})),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::BAD_REQUEST, "{body}");
        let (status, _) = request(
            &harness,
            "PUT",
            "/api/templates/misspelled",
            Some(serde_json::json!({"modle": "x"})),
        )
        .await;
        assert!(status.is_client_error(), "{status}");
        let (_, body) = request(&harness, "GET", "/api/templates", None).await;
        assert_eq!(body["templates"], serde_json::json!([]));
    }
}
