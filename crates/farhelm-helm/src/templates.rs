//! The helm's launch templates API (SPEC.md, Launch templates):
//! `GET /api/templates`, `PUT /api/templates/{name}` and
//! `DELETE /api/templates/{name}`.
//!
//! Templates belong to the helm, one catalog for every host it manages. The
//! helm checks only a template's shape when storing it (a usable name,
//! fields of a bounded size, editable commands, no unknown field); whether its fields apply is
//! decided when it is applied, against the launcher as it is then
//! (`farhelm_proto::launcher::apply_template`). Writes come from these routes
//! and from an agent's `farhelm agent template` verbs, which share
//! [`store_template`] and [`remove_template`] with them; an agent's write
//! waits for the user's approval first, because a template can carry a
//! command line every host the helm manages may later run (SPEC.md,
//! Agent-spawned sessions).

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
/// (SPEC.md wants no concurrency check for the GUI's writes; only an
/// agent's write carries a [`Precondition`]). Clients read templates when their
/// launcher or Templates panel opens; nothing pushes a template change into
/// a launcher already open in another client.
pub(crate) async fn put_template(
    State(state): State<Arc<AppState>>,
    AxPath(name): AxPath<String>,
    axum::Json(fields): axum::Json<TemplateFields>,
) -> impl IntoResponse {
    let template = LaunchTemplate { name, fields };
    match store_template(&state, template.clone(), Precondition::None).await {
        Ok(_) => axum::Json(template).into_response(),
        Err(error) => http_error(error),
    }
}

/// What a template write or delete requires of the template it replaces.
pub(crate) enum Precondition {
    /// Nothing: last write wins. The GUI's editor and its delete button,
    /// which SPEC.md wants without a concurrency check.
    None,
    /// The template of that name must still be exactly this (`None`: no
    /// template has the name). An agent's write verbs, which the user
    /// approved against what the card showed; checked in the same store
    /// transaction as the write, so another writer cannot land in between.
    Unchanged(Option<TemplateFields>),
}

/// Store `template`, creating or replacing it, and announce the change: the
/// one write path for templates, shared by the GUI's editor (`PUT`) and an
/// agent's `farhelm agent template create`/`edit` (SPEC.md: "These writes go
/// through the same path as the GUI's template editor"). Reports whether it
/// wrote, which is `false` only when `precondition` no longer held.
///
/// The shape is checked first, and a bad one is the API's typed 400. The write
/// and its change hint run in a detached task, so a request dropped after the
/// commit still bumps the hint, like every other helm write.
pub(crate) async fn store_template(
    state: &Arc<AppState>,
    template: LaunchTemplate,
    precondition: Precondition,
) -> anyhow::Result<bool> {
    check_template_shape(&template).map_err(|message| {
        anyhow::Error::new(crate::SupervisorError {
            origin: crate::client::ErrorOrigin::Helm,
            kind: farhelm_proto::ErrorKind::InvalidRequest,
            message,
        })
    })?;
    let task_state = Arc::clone(state);
    tokio::spawn(async move {
        let wrote = match precondition {
            Precondition::None => {
                task_state.store.put_launch_template(template).await?;
                true
            }
            Precondition::Unchanged(expected) => {
                task_state
                    .store
                    .put_launch_template_if(template, expected)
                    .await?
            }
        };
        if wrote {
            task_state.manager.events().bump();
        }
        Ok::<_, anyhow::Error>(wrote)
    })
    .await
    .map_err(|error| anyhow::Error::new(error).context("template write task panicked"))?
}

/// `DELETE /api/templates/{name}` — remove a template. No session records
/// which templates made it (SPEC.md), so nothing else changes. A template
/// that does not exist is the helm's not-found refusal.
pub(crate) async fn delete_template(
    State(state): State<Arc<AppState>>,
    AxPath(name): AxPath<String>,
) -> impl IntoResponse {
    match remove_template(&state, name, Precondition::None).await {
        Ok(true) => axum::Json(serde_json::json!({})).into_response(),
        Ok(false) => http_error(anyhow::Error::new(crate::SupervisorError {
            origin: crate::client::ErrorOrigin::Helm,
            kind: farhelm_proto::ErrorKind::NotFound,
            message: "no template has that name".to_string(),
        })),
        Err(error) => http_error(error),
    }
}

/// Remove the template named `name` and announce it; the shared delete path
/// for the GUI and `farhelm agent template delete`. Reports whether it
/// deleted, which is `false` when no template has the name or (under
/// [`Precondition::Unchanged`]) it is no longer the one expected; each
/// caller words its own refusal.
pub(crate) async fn remove_template(
    state: &Arc<AppState>,
    name: String,
    precondition: Precondition,
) -> anyhow::Result<bool> {
    let task_state = Arc::clone(state);
    tokio::spawn(async move {
        let deleted = match precondition {
            Precondition::None => task_state.store.delete_launch_template(name).await?,
            // An expectation of "absent" has nothing to delete.
            Precondition::Unchanged(None) => false,
            Precondition::Unchanged(Some(expected)) => {
                task_state
                    .store
                    .delete_launch_template_if(name, expected)
                    .await?
            }
        };
        if deleted {
            task_state.manager.events().bump();
        }
        Ok::<_, anyhow::Error>(deleted)
    })
    .await
    .map_err(|error| anyhow::Error::new(error).context("template delete task panicked"))?
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

    /// The shared save boundary must refuse unsafe command bytes before any
    /// persistence. Both fields are checked through the real HTTP route, and
    /// refusal leaves the catalog empty rather than storing a partial edit.
    #[farhelm_testtrace::test]
    async fn template_writes_refuse_hidden_command_characters_without_storing() {
        let harness = rest_harness::idle_helm().await;
        let (_, before) = request(&harness, "GET", "/api/templates", None).await;
        assert_eq!(before["templates"], serde_json::json!([]));
        for (key, label) in [
            ("command", "launch command"),
            ("resume_command", "resume command"),
        ] {
            for command in ["echo 'first\nsecond'", "tool \u{200B}hidden"] {
                let (status, body) = request(
                    &harness,
                    "PUT",
                    "/api/templates/unsafe",
                    Some(serde_json::json!({key: command})),
                )
                .await;
                assert_eq!(status, axum::http::StatusCode::BAD_REQUEST, "{body}");
                assert!(body.as_str().unwrap().contains(label), "{body}");
            }
        }
        let (_, after) = request(&harness, "GET", "/api/templates", None).await;
        assert_eq!(after["templates"], before["templates"]);
    }
}
