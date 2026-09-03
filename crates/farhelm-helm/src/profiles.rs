//! Agent profile APIs for the helm-owned catalog.
//!
//! ## The helm owns the catalog
//!
//! The helm now stores one catalog shared by every host and every client.
//! `GET` and CRUD under `/api/profiles` read and mutate that catalog. There is
//! no host-scoped profile surface: hosts consume resolved launch bundles, not
//! catalog storage. The remembered default is one raw id per helm, including a
//! dangling id after deletion, so the client can ask instead of guessing.
//!
//! ## The default is one bare profile id
//!
//! The remembered default is one plain profile id in helm.db. It is not
//! reconciled against the catalog on read: a deleted profile remains a
//! useful signal to the client that it must ask instead of guessing.
//!
//! ## One read, both halves
//!
//! [`list_catalog_profiles`] answers with the catalog AND the remembered default id
//! in one shape, and that pairing is the point rather than a convenience.
//! SPEC.md's creation rule is that the dialog defaults to the last-used
//! profile and ASKS when that profile is gone — which is a question about
//! two facts at once. Served separately, a client would have to reconcile a
//! catalog and a default read at different moments, and the moment that
//! matters is exactly the one where a profile was just deleted. The
//! remembered id is served RAW, never filtered against the catalog beside
//! it: a default naming a profile that no longer exists is precisely the
//! state the ask-don't-guess fallback exists for, and quietly dropping it
//! would turn "your last profile is gone, pick another" into a silent
//! nothing.
//!
//! ## Every mutation that changed something invalidates
//!
//! Profiles are one of the surfaces the goal promises arrives without
//! polling: an edit in one client must reach another client's open profile
//! surface and its create dialog. Each mutation that actually changed the
//! catalog therefore bumps the fleet's revision
//! (`crate::manager::FleetEvents`). A plain read does not. An edit that
//! submits exactly what is already stored is accepted and wakes the fleet
//! like any other last-write-wins edit.

use crate::{AppState, http_error};
use axum::extract::{Path as AxPath, State};
use axum::response::IntoResponse;
use farhelm_proto::{AgentKind, Profile};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Resolve one exact profile name from a single helm catalog snapshot.
///
/// Agent creates and the supervisor's upward named-spawn relay use this
/// rule. Browser creation remains id-based, preserving the stable identity
/// chosen by its catalog picker. Keeping name resolution here prevents the
/// two name-taking entry points from disagreeing about ambiguity, and the
/// candidate list gives a caller enough information to retry without
/// weakening exact matching.
pub(crate) fn resolve_profile_name(profiles: &[Profile], name: &str) -> anyhow::Result<Profile> {
    let matches = profiles
        .iter()
        .filter(|profile| profile.name == name)
        .collect::<Vec<_>>();
    let [profile] = matches.as_slice() else {
        let candidates = if matches.is_empty() {
            profiles.iter().collect::<Vec<_>>()
        } else {
            matches
        }
        .iter()
        .map(|profile| format!("{} ({})", profile.name, profile.id))
        .collect::<Vec<_>>()
        .join(", ");
        return Err(anyhow::Error::new(crate::SupervisorError {
            kind: farhelm_proto::ErrorKind::InvalidRequest,
            message: format!(
                "profile name {name:?} did not resolve uniquely; candidates: {}",
                if candidates.is_empty() {
                    "none"
                } else {
                    &candidates
                }
            ),
        }));
    };
    Ok((*profile).clone())
}

/// What the profile-list route answers with: the helm catalog and its
/// remembered default.
///
/// See the module docs for why the two travel in one shape. The field names
/// are frozen by PLAN_M6_75.md item 6's consumer.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct ProfilesView {
    /// The catalog ordered by profile id ascending, stable across renames.
    /// Not re-sorted here: a client
    /// wanting the user's alphabet sorts locally, where it knows the locale.
    pub(crate) profiles: Vec<Profile>,
    /// The helm-wide id last used by a profile-backed session, or `None`.
    ///
    /// May name a profile ABSENT from `profiles` above — a deleted one — and
    /// that combination is meaningful rather than a bug: it is what a client
    /// keys SPEC.md's ask-don't-guess fallback off.
    ///
    /// Both values live in helm.db. They are read separately because a
    /// mismatch has one safe interpretation: ask rather than guess.
    pub(crate) default_profile: Option<String>,
}

/// The body of a profile create or update — everything but the id.
///
/// A client has no id to know in advance, and letting it propose one would
/// invite collisions. On update, the URL is the sole resource authority.
#[derive(Deserialize)]
pub(crate) struct ProfileSpec {
    name: String,
    invocation: String,
    /// Which integrated agent this profile IS. Required, unlike a create's
    /// `agent_kind` override: `Generic` is the explicit spelling of "no
    /// kind", and an absent field would be a second way to say the same
    /// thing about a value that decides whether capture and status
    /// sharpening run at all.
    agent_kind: AgentKind,
    /// The resume invocation as an argv vector, or absent. See
    /// `farhelm_proto::Profile::resume_template` for what absence means per
    /// kind — it is not uniformly "no resume".
    resume_template: Option<Vec<String>>,
}

/// Render a catalog field refusal as a typed 400 response at the helm API.
fn catalog_validation_error(message: String) -> axum::response::Response {
    http_error(anyhow::Error::new(crate::SupervisorError {
        kind: farhelm_proto::ErrorKind::InvalidRequest,
        message,
    }))
}

/// `GET /api/profiles` — read the helm-owned catalog and its raw remembered id.
///
/// The shared response deliberately exposes a dangling default: deletion
/// changes future choices, not the historical suggestion clients need to
/// recognize and ask the user to replace.
pub(crate) async fn list_catalog_profiles(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let profiles = match state.store.profiles().await {
        Ok(profiles) => profiles,
        Err(error) => return http_error(error),
    };
    let default_profile = match state.store.remembered_profile().await {
        Ok(default_profile) => default_profile,
        Err(error) => return http_error(error),
    };
    axum::Json(ProfilesView {
        profiles,
        default_profile,
    })
    .into_response()
}

/// `POST /api/profiles` — create a profile in the helm-wide catalog.
///
/// SQLite serializes catalog writes and enforces the bound in the same
/// transaction as insertion, so an application mutex would only impose an
/// unneeded request-arrival order. The detached task couples a successful
/// durable mutation to its revision bump even after axum drops this request.
pub(crate) async fn create_catalog_profile(
    State(state): State<Arc<AppState>>,
    axum::Json(spec): axum::Json<ProfileSpec>,
) -> impl IntoResponse {
    if let Err(message) = farhelm_proto::validate_profile_fields(
        &spec.name,
        &spec.invocation,
        spec.agent_kind,
        spec.resume_template.as_deref(),
    ) {
        return catalog_validation_error(message);
    }
    let task_state = Arc::clone(&state);
    let mutation = tokio::spawn(async move {
        let outcome = task_state
            .store
            .create_profile(
                spec.name,
                spec.invocation,
                spec.agent_kind,
                spec.resume_template,
            )
            .await?;
        if matches!(outcome, crate::store::ProfileCreation::Created(_)) {
            task_state.manager.events().bump();
        }
        Ok::<_, anyhow::Error>(outcome)
    });
    match mutation.await {
        Err(error) => http_error(anyhow::Error::new(error).context("catalog create task panicked")),
        Ok(Err(error)) => http_error(error),
        Ok(Ok(crate::store::ProfileCreation::Created(profile))) => {
            (axum::http::StatusCode::CREATED, axum::Json(profile)).into_response()
        }
        Ok(Ok(crate::store::ProfileCreation::CatalogFull)) => {
            http_error(anyhow::Error::new(crate::SupervisorError {
                kind: farhelm_proto::ErrorKind::InvalidRequest,
                message: format!(
                    "this helm already holds the maximum of {} profiles; delete one before creating another",
                    farhelm_proto::MAX_PROFILES
                ),
            }))
        }
    }
}

/// `POST /api/profiles/{id}` — replace a helm-owned profile wholesale.
///
/// The URL is the resource authority; the id-free body cannot accidentally
/// redirect an update. Concurrent accepted updates are last-write-wins in
/// SQLite commit order. As with create, a detached task makes the revision
/// bump cancellation-safe and refusals leave that revision unchanged.
pub(crate) async fn update_catalog_profile(
    State(state): State<Arc<AppState>>,
    AxPath(profile_id): AxPath<String>,
    axum::Json(spec): axum::Json<ProfileSpec>,
) -> impl IntoResponse {
    if let Err(message) = farhelm_proto::validate_profile_fields(
        &spec.name,
        &spec.invocation,
        spec.agent_kind,
        spec.resume_template.as_deref(),
    ) {
        return catalog_validation_error(message);
    }
    let profile = Profile {
        id: profile_id,
        name: spec.name,
        invocation: spec.invocation,
        agent_kind: spec.agent_kind,
        resume_template: spec.resume_template,
    };
    let task_state = Arc::clone(&state);
    let mutation = tokio::spawn(async move {
        let outcome = task_state.store.update_profile(profile).await?;
        if outcome.is_some() {
            task_state.manager.events().bump();
        }
        Ok::<_, anyhow::Error>(outcome)
    });
    match mutation.await {
        Err(error) => http_error(anyhow::Error::new(error).context("catalog update task panicked")),
        Ok(Err(error)) => http_error(error),
        Ok(Ok(Some(profile))) => axum::Json(profile).into_response(),
        Ok(Ok(None)) => http_error(anyhow::Error::new(crate::SupervisorError {
            kind: farhelm_proto::ErrorKind::NotFound,
            message: "profile not found".to_string(),
        })),
    }
}

/// `DELETE /api/profiles/{id}` — remove a helm-owned profile.
///
/// Deletion intentionally leaves a dangling remembered id, which tells a
/// later picker to ask rather than silently substitute another profile. Its
/// detached mutation task follows the same commit-and-invalidate contract as
/// create and update.
pub(crate) async fn delete_catalog_profile(
    State(state): State<Arc<AppState>>,
    AxPath(profile_id): AxPath<String>,
) -> impl IntoResponse {
    let task_state = Arc::clone(&state);
    let mutation = tokio::spawn(async move {
        let deleted = task_state.store.delete_profile(&profile_id).await?;
        if deleted {
            task_state.manager.events().bump();
        }
        Ok::<_, anyhow::Error>(deleted)
    });
    match mutation.await {
        Err(error) => http_error(anyhow::Error::new(error).context("catalog delete task panicked")),
        Ok(Err(error)) => http_error(error),
        Ok(Ok(true)) => axum::Json(serde_json::json!({})).into_response(),
        Ok(Ok(false)) => http_error(anyhow::Error::new(crate::SupervisorError {
            kind: farhelm_proto::ErrorKind::NotFound,
            message: "profile not found".to_string(),
        })),
    }
}

#[cfg(test)]
mod tests {
    use crate::rest_harness;
    use farhelm_proto::AgentKind;
    use tower::ServiceExt;

    /// Issue one request against the real router and return its status and
    /// body.
    ///
    /// `method`/`body` rather than separate helpers because these tests
    /// exercise all three verbs against the same two paths, and the
    /// difference between them is the only thing worth seeing at the call
    /// site.
    async fn request(
        harness: &rest_harness::Harness,
        method: &str,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> (axum::http::StatusCode, serde_json::Value) {
        let builder = axum::http::Request::builder()
            .method(method)
            .uri(uri)
            .header("host", "127.0.0.1:7433");
        let request = match &body {
            Some(json) => builder
                .header("content-type", "application/json")
                .body(axum::body::Body::from(json.to_string()))
                .unwrap(),
            None => builder.body(axum::body::Body::empty()).unwrap(),
        };
        let response = harness.router().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value = serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(&bytes).into()));
        (status, value)
    }

    /// The helm-owned routes expose the seeded catalog, validate writes, and
    /// keep the catalog bound observable at the HTTP boundary. This test
    /// matters because store-only coverage cannot catch a wrong route, status
    /// code, request shape, or fleet-revision invalidation.
    #[tokio::test]
    async fn helm_profile_catalog_routes_cover_crud_errors_and_bound() {
        let harness = rest_harness::idle_helm().await;
        let before = harness.manager.events().revision();

        let (status, value) = request(&harness, "GET", "/api/profiles", None).await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(value["profiles"].as_array().unwrap().len(), 4);
        assert_eq!(value["default_profile"], serde_json::Value::Null);

        let (status, value) = request(
            &harness,
            "POST",
            "/api/profiles",
            Some(serde_json::json!({
                "name": "wrapper",
                "invocation": "wrapper --agent",
                "agent_kind": "generic",
                "resume_template": null,
            })),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::CREATED);
        let id = value["id"].as_str().unwrap().to_string();
        assert!(harness.manager.events().revision() > before);
        let after_create = harness.manager.events().revision();

        let (status, value) = request(
            &harness,
            "POST",
            &format!("/api/profiles/{id}"),
            Some(serde_json::json!({
                "name": "renamed",
                "invocation": "wrapper --renamed",
                "agent_kind": "generic",
                "resume_template": null,
            })),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(value["id"], id);
        assert_eq!(value["name"], "renamed");
        assert!(harness.manager.events().revision() > after_create);
        let after_update = harness.manager.events().revision();

        let (status, value) = request(
            &harness,
            "POST",
            &format!("/api/profiles/{id}"),
            Some(serde_json::json!({
                "name": " ",
                "invocation": "wrapper",
                "agent_kind": "generic",
                "resume_template": null,
            })),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
        assert!(value.as_str().unwrap().contains("must not be empty"));
        assert_eq!(harness.manager.events().revision(), after_update);

        let (status, value) = request(
            &harness,
            "POST",
            "/api/profiles/unknown",
            Some(serde_json::json!({
                "name": "missing",
                "invocation": "wrapper",
                "agent_kind": "generic",
                "resume_template": null,
            })),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
        assert!(value.as_str().unwrap().contains("profile not found"));
        assert_eq!(harness.manager.events().revision(), after_update);

        let (status, value) =
            request(&harness, "DELETE", &format!("/api/profiles/{id}"), None).await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(value, serde_json::json!({}));
        assert!(harness.manager.events().revision() > after_update);
        let after_delete = harness.manager.events().revision();

        let (status, value) = request(&harness, "DELETE", "/api/profiles/unknown", None).await;
        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
        assert!(value.as_str().unwrap().contains("profile not found"));
        assert_eq!(harness.manager.events().revision(), after_delete);

        let starting_len = harness.store.profiles().await.unwrap().len();
        for _ in starting_len..farhelm_proto::MAX_PROFILES {
            assert!(matches!(
                harness
                    .store
                    .create_profile(
                        "wrapper".to_string(),
                        "wrapper".to_string(),
                        AgentKind::Generic,
                        None,
                    )
                    .await
                    .unwrap(),
                crate::store::ProfileCreation::Created(_)
            ));
        }
        let (status, value) = request(
            &harness,
            "POST",
            "/api/profiles",
            Some(serde_json::json!({
                "name": "too-many",
                "invocation": "wrapper",
                "agent_kind": "generic",
                "resume_template": null,
            })),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
        assert!(value.as_str().unwrap().contains("maximum"));
        assert_eq!(harness.manager.events().revision(), after_delete);
        assert_eq!(
            harness.store.profiles().await.unwrap().len(),
            farhelm_proto::MAX_PROFILES
        );

        harness
            .store
            .remember_profile_default("deleted-profile")
            .await
            .unwrap();
        let (status, value) = request(&harness, "GET", "/api/profiles", None).await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(value["default_profile"], "deleted-profile");
        assert_eq!(
            value["profiles"].as_array().unwrap().len(),
            farhelm_proto::MAX_PROFILES
        );
    }

    /// Host-shaped profile paths are gone rather than retained as a second
    /// spelling of helm state. This matters because accepting them would keep
    /// teaching clients that a host selects a catalog even though no such
    /// distinction exists.
    #[tokio::test]
    async fn host_scoped_profile_routes_are_not_found() {
        let harness = rest_harness::idle_helm().await;
        for (method, path) in [
            ("GET", "/api/hosts/999999/profiles"),
            ("POST", "/api/hosts/999999/profiles"),
            ("POST", "/api/hosts/999999/profiles/profile-1"),
            ("DELETE", "/api/hosts/999999/profiles/profile-1"),
        ] {
            let (status, _) = request(&harness, method, path, None).await;
            assert_eq!(status, axum::http::StatusCode::NOT_FOUND, "{method} {path}");
        }
    }

    /// The two creation modes are exclusive at the REST edge too, and a
    /// body that gets it wrong reaches no supervisor at all.
    ///
    /// Refused HERE rather than forwarded because the refusal is about the
    /// request's shape, and a helm that passed an ambiguous create along
    /// would turn a client bug into a round trip whose failure mode depends
    /// on which supervisor answered it. `silent_supervisor` is what proves
    /// nothing was forwarded.
    #[tokio::test]
    async fn a_create_naming_both_modes_or_neither_is_refused_locally() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(rest_harness::silent_supervisor(peer_side));
        let harness = rest_harness::spliced_helm(client_side).await;

        for (body, expected) in [
            (
                serde_json::json!({"cwd": "/work", "invocation": "claude", "profile_id": "p-1"}),
                "never both",
            ),
            (serde_json::json!({"cwd": "/work"}), "names neither"),
        ] {
            let (status, text) = request(&harness, "POST", "/api/sessions", Some(body)).await;
            assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
            assert!(
                text.as_str().unwrap_or_default().contains(expected),
                "the refusal must say which shape was wrong: {text:?}"
            );
        }
        peer.await.unwrap();
    }

    /// A profile-mode create carrying a snapshot override is REFUSED, not
    /// quietly stripped.
    ///
    /// The overrides are raw-mode only — a profile already states its kind
    /// and its resume template, and the wire refuses a request naming both —
    /// so forwarding a profile create while dropping the fields would launch
    /// a session under settings the caller believes it chose. Both fields
    /// are staged, because either alone is enough to make the request
    /// ambiguous, and neither reaches a supervisor.
    #[tokio::test]
    async fn a_profile_create_carrying_a_snapshot_override_is_refused_locally() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(rest_harness::silent_supervisor(peer_side));
        let harness = rest_harness::spliced_helm(client_side).await;

        for body in [
            serde_json::json!({
                "cwd": "/work",
                "profile_id": "p-1",
                "agent_kind": "codex",
            }),
            serde_json::json!({
                "cwd": "/work",
                "profile_id": "p-1",
                "resume_template": ["claude", "--resume", "{conversation}"],
            }),
        ] {
            let (status, text) = request(&harness, "POST", "/api/sessions", Some(body)).await;
            assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
            let text = text.as_str().unwrap_or_default().to_string();
            assert!(
                text.contains("agent_kind") && text.contains("resume_template"),
                "the refusal must name the fields to remove: {text:?}"
            );
        }
        peer.await.unwrap();
    }
}
