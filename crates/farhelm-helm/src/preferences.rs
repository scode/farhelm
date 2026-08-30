//! `GET`/`PUT /api/preferences` — the one client preference the helm
//! remembers for every client (SPEC.md, Session list): the chosen list order
//! and the last user-selected session.
//!
//! The helm holds this rather than each client, and that is the whole
//! design: no client keeps its own copy, so a browser tab and the desktop
//! app open in the same order and on the same session, and the value
//! survives reloads and relaunches because it lives in `helm.db` instead of
//! in whichever client last wrote it. Every client fetches the row once
//! after authenticating and writes a sparse patch on change; a field a
//! patch leaves out is untouched, and an explicit `null` clears one.
//!
//! ## What the handlers check, and what they leave alone
//!
//! `list_sort` is validated against the same vocabulary `GET
//! /api/sessions?sort=` accepts ([`store::parse_sort_key`]), for the same
//! reason that route refuses an unknown word: a preference the list read
//! would then 400 on is a list that fails to load until somebody finds
//! the stored word, and refusing it at the write is the cheap place.
//! `last_selected` is NOT checked against the fleet — a session can be
//! deleted after the write, so the reader has to tolerate a stale id
//! regardless (the UI's auto-select falls back to the newest session), and
//! a check here would only turn a race into a spurious refusal — but it IS
//! capped at [`farhelm_proto::MAX_SESSION_ID_BYTES`], the same bound a
//! session id honors on the supervisor wire: no real session can exceed
//! it, and without the cap one authenticated request could park a
//! megabyte-scale string in the row for every client to download and
//! re-resolve on every load until someone cleared it by hand.
//!
//! ## Why no CORS layer
//!
//! The desktop webview's JavaScript reaches exactly four helm routes across
//! origins (the device validation, the token exchange, attachment uploads,
//! and the console log), and this is deliberately not a fifth: the desktop
//! UI's preference read and write go through the same native reqwest
//! funnel every other REST call does (`farhelm-ui`'s `api::send`), so the
//! route sits inside the ordinary protected group with nothing special
//! about it.

use crate::store::{self, PreferencePatch};
use crate::{AppState, SupervisorError, http_error};
use axum::extract::State;
use axum::response::IntoResponse;
use farhelm_proto::{ErrorKind, MAX_SESSION_ID_BYTES};
use std::sync::Arc;

/// `GET /api/preferences` — the row as stored, with unset fields omitted.
///
/// A helm that has nothing remembered answers `{}` rather than an error:
/// "nothing chosen yet" is the ordinary state of a fresh install, and the
/// client's defaults are the right answer to it.
pub(crate) async fn get_preferences(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.store.preferences().await {
        Ok(preferences) => axum::Json(preferences).into_response(),
        Err(error) => http_error(error),
    }
}

/// `PUT /api/preferences` — merge a sparse patch into the row.
///
/// Sparse, so a client only ever sends the field the user changed and
/// never overwrites the other with its own possibly stale copy (see
/// [`store::HelmStore::update_preferences`] for why that matters with the
/// row shared). Three states per field, as [`PreferencePatch`] spells out:
/// absent leaves it alone, an explicit `null` clears it, a value replaces
/// it. An unknown field is refused through serde's `deny_unknown_fields`
/// (axum's `Json` extractor answers a data error with 422, like every other
/// JSON route here), which is what keeps a typo in a client from being an
/// accepted no-op; a sort word this helm does not serve, or a
/// `last_selected` longer than any session id can be
/// ([`MAX_SESSION_ID_BYTES`]), is a 400. 204 on success: there is nothing
/// to say back that the client did not just send.
pub(crate) async fn put_preferences(
    State(state): State<Arc<AppState>>,
    axum::Json(patch): axum::Json<PreferencePatch>,
) -> impl IntoResponse {
    if let Some(Some(sort)) = &patch.list_sort
        && store::parse_sort_key(sort).is_none()
    {
        return http_error(anyhow::Error::new(SupervisorError {
            kind: ErrorKind::InvalidRequest,
            message: format!(
                "{sort:?} is not a session list order; this helm serves created, activity, and \
                 title"
            ),
        }));
    }
    if let Some(Some(selected)) = &patch.last_selected
        && selected.len() > MAX_SESSION_ID_BYTES
    {
        return http_error(anyhow::Error::new(SupervisorError {
            kind: ErrorKind::InvalidRequest,
            message: format!(
                "last_selected exceeds {MAX_SESSION_ID_BYTES} bytes; no session id is that long"
            ),
        }));
    }
    match state.store.update_preferences(patch).await {
        Ok(()) => axum::http::StatusCode::NO_CONTENT.into_response(),
        Err(error) => http_error(error),
    }
}

#[cfg(test)]
mod tests {
    use crate::rest_harness;
    use crate::store::Preferences;
    use axum::http::StatusCode;
    use tower::ServiceExt;

    fn get() -> axum::http::Request<axum::body::Body> {
        axum::http::Request::builder()
            .method("GET")
            .uri("/api/preferences")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap()
    }

    fn put(body: serde_json::Value) -> axum::http::Request<axum::body::Body> {
        axum::http::Request::builder()
            .method("PUT")
            .uri("/api/preferences")
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(body.to_string()))
            .unwrap()
    }

    async fn read_raw(harness: &rest_harness::Harness) -> Vec<u8> {
        let response = harness.router().oneshot(get()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap()
            .to_vec()
    }

    async fn read(harness: &rest_harness::Harness) -> Preferences {
        serde_json::from_slice(&read_raw(harness).await)
            .expect("the preference reply decodes as the shared type")
    }

    /// The route pair is the whole contract every client is written
    /// against: a fresh helm answers an empty object, a sparse `PUT` lands
    /// only the field it names, a later `GET` reads the merged row back, a
    /// sort word the list would refuse is refused here with a 400 and
    /// leaves the row untouched, an explicit `null` clears one field, and
    /// the route is protected like every other read — an unauthenticated
    /// request gets the structured 401.
    ///
    /// One test rather than five because the assertions build on one
    /// another's state, and the sequence IS the specification: it is what a
    /// browser tab and the desktop app do to the same row in turn.
    #[tokio::test]
    async fn the_preference_routes_read_and_merge_one_shared_row() {
        let harness = rest_harness::idle_helm().await;
        // The raw bytes, not just the decoded value: `{"list_sort":null}`
        // decodes identically, but the documented wire contract — and what
        // `skip_serializing_if` exists to keep — is that unset fields are
        // OMITTED, and only a byte assertion notices that changing.
        assert_eq!(
            read_raw(&harness).await,
            b"{}",
            "a fresh helm has nothing remembered and says so with an empty object"
        );

        let response = harness
            .router()
            .oneshot(put(serde_json::json!({ "list_sort": "title" })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let response = harness
            .router()
            .oneshot(put(serde_json::json!({ "last_selected": "session-7" })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            read(&harness).await,
            Preferences {
                list_sort: Some("title".to_string()),
                last_selected: Some("session-7".to_string()),
            },
            "each sparse patch lands its own field and keeps the other's"
        );

        let response = harness
            .router()
            .oneshot(put(serde_json::json!({ "list_sort": "most-recent" })))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "a word the listing route would refuse is refused at the write"
        );
        let response = harness
            .router()
            .oneshot(put(serde_json::json!({ "colour": "blue" })))
            .await
            .unwrap();
        assert!(
            response.status().is_client_error(),
            "an unknown field is a client mistake, not an accepted no-op: {}",
            response.status()
        );
        assert_eq!(
            read(&harness).await.list_sort.as_deref(),
            Some("title"),
            "a refused patch leaves the row as it was"
        );

        let response = harness
            .router()
            .oneshot(put(serde_json::json!({ "last_selected": null })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            read(&harness).await,
            Preferences {
                list_sort: Some("title".to_string()),
                last_selected: None,
            },
            "an explicit null clears the field it names and only that one"
        );

        // The session-id size cap: a value at the wire's own limit is a
        // legal (if strange) id and is stored; one byte past it cannot name
        // any real session and is refused, leaving the row untouched.
        let at_limit = "s".repeat(farhelm_proto::MAX_SESSION_ID_BYTES);
        let response = harness
            .router()
            .oneshot(put(serde_json::json!({ "last_selected": at_limit })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let over_limit = "s".repeat(farhelm_proto::MAX_SESSION_ID_BYTES + 1);
        let response = harness
            .router()
            .oneshot(put(serde_json::json!({ "last_selected": over_limit })))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "a value no session id can reach must not be parked in the shared row"
        );
        assert_eq!(
            read(&harness).await.last_selected.as_deref(),
            Some(at_limit.as_str()),
            "the refused oversize patch left the stored value alone"
        );

        let response = harness
            .unauthenticated_router()
            .oneshot(get())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(
            body,
            r#"{"error":"unauthenticated","code":"device_auth_required"}"#.as_bytes(),
            "the same structured refusal body every protected route answers with"
        );
    }
}
