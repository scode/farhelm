//! `GET`/`PUT /api/preferences` — the one client preference the helm
//! remembers for every client (SPEC.md, Session list): the chosen list order,
//! last user-selected session, compact-row choice, and the remembered
//! permissions and workspace-trust choices of successful structured launches,
//! host confirmation choices, terminal web-link ownership, quick-launch template,
//! number-key bindings, and contact reused after successful feedback.
//!
//! The helm holds this rather than each client, and that is the whole
//! design: no client persists its own copy, so a browser tab and the desktop
//! app open in the same order and on the same session, and the value
//! survives reloads and relaunches because it lives in `helm.db` instead of
//! in whichever client last wrote it. Every client fetches the row once
//! after authenticating and writes a sparse patch on change; a field a
//! patch leaves out is untouched, and an explicit `null` clears one.
//!
//! The remembered launch choices are exceptions to "every client writes its own
//! choice": no shipped client ever PUTs them. They are written only by the helm
//! itself, as a side effect of a successful user-initiated structured
//! launch (`store::HelmStore::record_create_history_with_destination`), which is
//! what makes them facts every client and the spawn path agree on rather
//! than choices any one client claims happened. The route still accepts
//! them on `PUT` for wire uniformity with the client-declared fields (and so a
//! test fixture can plant a value directly). The permissions word is validated.
//!
//! ## What the handlers check, and what they leave alone
//!
//! `list_sort` is validated against the same vocabulary `GET
//! /api/sessions?sort=` accepts ([`store::parse_sort_key`]), for the same
//! reason that route refuses an unknown word: a preference the list read
//! would then 400 on is a list that fails to load until somebody finds
//! the stored word, and refusing it at the write is the cheap place.
//! `remembered_permissions` gets the identical treatment against its own
//! smaller vocabulary ([`store::is_known_remembered_permissions_word`]).
//! Terminal link ownership similarly has three known words; unknown writes
//! refuse the patch, while null restores default interception.
//! `last_selected` is NOT checked against the fleet — a session can be
//! deleted after the write, so the reader has to tolerate a stale id
//! regardless (the UI's auto-select falls back to the newest session), and
//! a check here would only turn a race into a spurious refusal — but it IS
//! capped at [`farhelm_proto::MAX_SESSION_ID_BYTES`], the same bound a
//! session id honors on the supervisor wire: no real session can exceed
//! it, and without the cap one authenticated request could park a
//! megabyte-scale string in the row for every client to download and
//! re-resolve on every load until someone cleared it by hand.
//! Feedback contact uses the feedback submission's code-point cap. Refusals
//! must never echo it, since clients log preference-write errors.
//!
//! ## Why no CORS layer
//!
//! The desktop webview's JavaScript reaches exactly four helm routes across
//! origins (device validation, attachment uploads, the console log, and the
//! clipboard), and this is deliberately not a fifth: the desktop
//! UI's preference read and write go through the same native reqwest
//! funnel every other REST call does (`farhelm-ui`'s `api::send`), so the
//! route sits inside the ordinary protected group with nothing special
//! about it.

use crate::store::{self, PreferencePatch};
use crate::{AppState, SupervisorError, http_error};
use axum::extract::State;
use axum::response::IntoResponse;
use farhelm_proto::feedback::FEEDBACK_CONTACT_MAX_CHARS;
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
/// ([`MAX_SESSION_ID_BYTES`]), or a feedback contact beyond its code-point
/// cap is a 400. 204 on success: there is nothing
/// to say back that the client did not just send.
pub(crate) async fn put_preferences(
    State(state): State<Arc<AppState>>,
    axum::Json(patch): axum::Json<PreferencePatch>,
) -> impl IntoResponse {
    if let Some(Some(bindings)) = &patch.template_shortcuts {
        let mut names = std::collections::BTreeSet::new();
        // A whole-map replacement has ten possible entries, each naming a
        // template. Existence is deliberately independent of this shape check:
        // another client may delete or rename the name before the next press.
        let valid = bindings.iter().all(|(key, name)| {
            key.len() == 1
                && key.as_bytes()[0].is_ascii_digit()
                && names.insert(name)
                && farhelm_proto::launcher::check_template_shape(
                    &farhelm_proto::launcher::LaunchTemplate {
                        name: name.clone(),
                        fields: Default::default(),
                    },
                )
                .is_ok()
        });
        if !valid {
            return http_error(anyhow::Error::new(SupervisorError {
                origin: crate::client::ErrorOrigin::Helm,
                kind: ErrorKind::InvalidRequest,
                message: "template_shortcuts needs digit keys and distinct valid template names"
                    .into(),
            }));
        }
    }
    // Like last_selected, a template name can become stale after a write. The
    // reader handles that race; cap its size without inventing an existence or
    // eligibility gate that would disagree with the template editor.
    if let Some(Some(name)) = &patch.quick_launch_template
        && name.len() > farhelm_proto::launcher::TEMPLATE_NAME_CAP
    {
        return http_error(anyhow::Error::new(SupervisorError {
            origin: crate::client::ErrorOrigin::Helm,
            kind: ErrorKind::InvalidRequest,
            message: "quick_launch_template exceeds the template name limit".into(),
        }));
    }
    if let Some(Some(sort)) = &patch.list_sort
        && store::parse_sort_key(sort).is_none()
    {
        return http_error(anyhow::Error::new(SupervisorError {
            origin: crate::client::ErrorOrigin::Helm,
            kind: ErrorKind::InvalidRequest,
            message: format!(
                "{sort:?} is not a session list order; this helm serves created, activity, and \
                 title"
            ),
        }));
    }
    if let Some(Some(word)) = &patch.remembered_permissions
        && !store::is_known_remembered_permissions_word(word)
    {
        return http_error(anyhow::Error::new(SupervisorError {
            origin: crate::client::ErrorOrigin::Helm,
            kind: ErrorKind::InvalidRequest,
            message: format!(
                "{word:?} is not a remembered structured-launch permissions mode; this helm \
                 serves {}",
                farhelm_proto::LaunchPermission::ALL
                    .into_iter()
                    .filter(|permission| *permission != farhelm_proto::LaunchPermission::Custom)
                    .map(farhelm_proto::LaunchPermission::wire_word)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }));
    }
    if let Some(Some(selected)) = &patch.last_selected
        && selected.len() > MAX_SESSION_ID_BYTES
    {
        return http_error(anyhow::Error::new(SupervisorError {
            origin: crate::client::ErrorOrigin::Helm,
            kind: ErrorKind::InvalidRequest,
            message: format!(
                "last_selected exceeds {MAX_SESSION_ID_BYTES} bytes; no session id is that long"
            ),
        }));
    }
    if let Some(Some(contact)) = &patch.feedback_contact
        && contact.chars().count() > FEEDBACK_CONTACT_MAX_CHARS
    {
        return http_error(anyhow::Error::new(SupervisorError {
            origin: crate::client::ErrorOrigin::Helm,
            kind: ErrorKind::InvalidRequest,
            // The UI logs preference refusals. Never put the contact into one.
            message: format!("feedback_contact exceeds {FEEDBACK_CONTACT_MAX_CHARS} characters"),
        }));
    }
    if let Some(Some(mode)) = &patch.terminal_link_click
        && !store::is_known_terminal_link_click_word(mode)
    {
        return http_error(anyhow::Error::new(SupervisorError {
            origin: crate::client::ErrorOrigin::Helm,
            kind: ErrorKind::InvalidRequest,
            message: "terminal_link_click must be intercept, both, or program".into(),
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

    /// A protected preference read through the same router clients use.
    fn get() -> axum::http::Request<axum::body::Body> {
        axum::http::Request::builder()
            .method("GET")
            .uri("/api/preferences")
            .header("host", "127.0.0.1:7433")
            .body(axum::body::Body::empty())
            .unwrap()
    }

    /// Serialize a sparse patch so extraction exercises null versus absence.
    fn put(body: serde_json::Value) -> axum::http::Request<axum::body::Body> {
        axum::http::Request::builder()
            .method("PUT")
            .uri("/api/preferences")
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(body.to_string()))
            .unwrap()
    }

    /// Read bounded reply bytes to distinguish omitted fields from JSON null.
    async fn read_raw(harness: &rest_harness::Harness) -> Vec<u8> {
        let response = harness.router().oneshot(get()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap()
            .to_vec()
    }

    /// Decode the shared row without bypassing the HTTP read contract.
    async fn read(harness: &rest_harness::Harness) -> Preferences {
        serde_json::from_slice(&read_raw(harness).await)
            .expect("the preference reply decodes as the shared type")
    }

    /// Number bindings replace one shared map and never merge per key. Invalid
    /// maps refuse the whole sparse patch; missing templates remain valid names
    /// because deletion and cross-window rename can make any binding stale.
    #[farhelm_testtrace::test]
    async fn template_shortcuts_validate_replace_and_clear() {
        let harness = rest_harness::idle_helm().await;
        assert_eq!(read_raw(&harness).await, b"{}");
        let bindings = serde_json::json!({ "1": "missing", "0": "other" });
        let response = harness
            .router()
            .oneshot(put(serde_json::json!({
                "template_shortcuts": bindings, "quick_launch_template": "missing"
            })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let saved = read(&harness).await;
        assert_eq!(
            serde_json::to_value(&saved.template_shortcuts).unwrap(),
            bindings
        );
        assert_eq!(saved.quick_launch_template.as_deref(), Some("missing"));
        for invalid in [
            serde_json::json!({ "10": "a" }),
            serde_json::json!({ "x": "a" }),
            serde_json::json!({ "": "a" }),
            serde_json::json!({ "１": "a" }),
            serde_json::json!({ "1": "a", "2": "a" }),
            serde_json::json!({ "1": "" }),
            serde_json::json!({ "1": "." }),
            serde_json::json!({ "1": ".." }),
            serde_json::json!({ "1": " a" }),
            serde_json::json!({ "1": "a\n" }),
            serde_json::json!({ "1": "a".repeat(farhelm_proto::launcher::TEMPLATE_NAME_CAP + 1) }),
        ] {
            let response = harness
                .router()
                .oneshot(put(serde_json::json!({
                    "template_shortcuts": invalid, "compact": true
                })))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{invalid}");
            assert_eq!(read(&harness).await, saved, "refusal preserves every field");
        }
        let response = harness
            .router()
            .oneshot(put(serde_json::json!({
                "template_shortcuts": { "2": "replacement" }
            })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            read(&harness).await.template_shortcuts,
            std::collections::BTreeMap::from([("2".into(), "replacement".into())])
        );
        let response = harness
            .router()
            .oneshot(put(serde_json::json!({ "compact": true })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            read(&harness).await.template_shortcuts.len(),
            1,
            "omission preserves bindings"
        );
        for clear in [serde_json::json!(null), serde_json::json!({})] {
            let response = harness
                .router()
                .oneshot(put(serde_json::json!({
                    "template_shortcuts": clear
                })))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NO_CONTENT);
            let saved = read(&harness).await;
            assert!(saved.template_shortcuts.is_empty());
            assert_eq!(saved.compact, Some(true));
            assert_eq!(saved.quick_launch_template.as_deref(), Some("missing"));
        }
    }

    /// All link choices travel through the protected sparse route. Invalid
    /// words must refuse the entire patch, while null restores the default
    /// without clearing a separately chosen compact-list preference.
    #[farhelm_testtrace::test]
    async fn terminal_link_click_validates_and_clears_independently() {
        let harness = rest_harness::idle_helm().await;
        assert_eq!(read_raw(&harness).await, b"{}");
        for mode in ["intercept", "both", "program"] {
            let response = harness
                .router()
                .oneshot(put(serde_json::json!({
                    "terminal_link_click": mode, "compact": true
                })))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NO_CONTENT);
            let stored = read(&harness).await;
            assert_eq!(stored.terminal_link_click.as_deref(), Some(mode));
            assert_eq!(stored.compact, Some(true));
        }
        let response = harness
            .router()
            .oneshot(put(serde_json::json!({
                "terminal_link_click": "unknown", "compact": false
            })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let stored = read(&harness).await;
        assert_eq!(stored.terminal_link_click.as_deref(), Some("program"));
        assert_eq!(stored.compact, Some(true));
        let response = harness
            .router()
            .oneshot(put(serde_json::json!({
                "terminal_link_click": null
            })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            read(&harness).await,
            Preferences {
                compact: Some(true),
                ..Preferences::default()
            }
        );
    }

    /// Contact memory uses the feedback cap in code points, never echoes a
    /// refused value, and clears independently of other shared preferences.
    /// Exercise the public routes so null and absence pass through extraction.
    #[farhelm_testtrace::test]
    async fn feedback_contact_is_bounded_private_and_clearable() {
        let harness = rest_harness::idle_helm().await;
        assert_eq!(read_raw(&harness).await, b"{}");
        let at_limit = "界".repeat(farhelm_proto::feedback::FEEDBACK_CONTACT_MAX_CHARS);
        let response = harness
            .router()
            .oneshot(put(serde_json::json!({
                "feedback_contact": at_limit, "compact": true
            })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            read(&harness).await.feedback_contact.as_deref(),
            Some(at_limit.as_str())
        );

        let secret = format!("private-contact-marker{}", "界".repeat(179));
        assert!(secret.chars().count() > farhelm_proto::feedback::FEEDBACK_CONTACT_MAX_CHARS);
        let response = harness
            .router()
            .oneshot(put(serde_json::json!({
                "feedback_contact": secret, "compact": false
            })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("feedback_contact exceeds 200 characters"));
        assert!(!body.contains("private-contact-marker"));
        assert_eq!(
            read(&harness).await.feedback_contact.as_deref(),
            Some(at_limit.as_str())
        );
        assert_eq!(
            read(&harness).await.compact,
            Some(true),
            "refusal is atomic"
        );

        let response = harness
            .router()
            .oneshot(put(serde_json::json!({"list_sort": "title"})))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            read(&harness).await.feedback_contact.as_deref(),
            Some(at_limit.as_str())
        );
        let response = harness
            .router()
            .oneshot(put(serde_json::json!({"feedback_contact": null})))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let remembered = read(&harness).await;
        assert_eq!(remembered.feedback_contact, None);
        assert_eq!(remembered.list_sort.as_deref(), Some("title"));
        assert_eq!(remembered.compact, Some(true));
    }

    /// Only a real boolean opts out of session questions. The route must keep
    /// host authority independent and distinguish explicit false, null and omission.
    #[farhelm_testtrace::test]
    async fn session_confirmations_are_a_sparse_boolean_preference() {
        let harness = rest_harness::idle_helm().await;
        assert_eq!(read_raw(&harness).await, b"{}");
        for value in [
            serde_json::json!(true),
            serde_json::json!(false),
            serde_json::Value::Null,
        ] {
            let reply = harness
                .router()
                .oneshot(put(serde_json::json!({
                    "skip_session_confirmations": value,
                })))
                .await
                .unwrap();
            assert_eq!(reply.status(), StatusCode::NO_CONTENT);
            let reply = harness
                .router()
                .oneshot(put(serde_json::json!({
                    "skip_host_setup_confirmation": true,
                })))
                .await
                .unwrap();
            assert_eq!(reply.status(), StatusCode::NO_CONTENT);
            let snapshot = read(&harness).await;
            assert_eq!(snapshot.skip_session_confirmations, value.as_bool());
            assert_eq!(snapshot.skip_host_setup_confirmation, Some(true));
            assert_eq!(snapshot.skip_host_remove_confirmation, None);
            let wire: serde_json::Value =
                serde_json::from_slice(&read_raw(&harness).await).unwrap();
            if value.is_null() {
                assert!(wire.get("skip_session_confirmations").is_none());
            } else {
                assert_eq!(wire["skip_session_confirmations"], value);
            }
        }
        let before = read(&harness).await;
        for invalid in [serde_json::json!("true"), serde_json::json!(1)] {
            let reply = harness
                .router()
                .oneshot(put(serde_json::json!({
                    "skip_session_confirmations": invalid,
                })))
                .await
                .unwrap();
            assert!(reply.status().is_client_error());
            assert_eq!(read(&harness).await, before);
        }
    }

    /// Compact may be the first preference a fresh client changes. Exercise
    /// insertion into an absent singleton row, not only the update path after
    /// sort or selection created it, and leave those unrelated choices unset.
    #[farhelm_testtrace::test]
    async fn compact_alone_creates_the_shared_preference_row() {
        let harness = rest_harness::idle_helm().await;
        assert_eq!(read_raw(&harness).await, b"{}");
        let response = harness
            .router()
            .oneshot(put(serde_json::json!({ "compact": true })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            read(&harness).await,
            Preferences {
                compact: Some(true),
                ..Preferences::default()
            }
        );
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
    #[farhelm_testtrace::test]
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
        let response = harness
            .router()
            .oneshot(put(serde_json::json!({ "compact": true })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let response = harness
            .router()
            .oneshot(put(
                serde_json::json!({ "skip_host_remove_confirmation": true }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let response = harness
            .router()
            .oneshot(put(
                serde_json::json!({ "skip_host_setup_confirmation": true }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            read(&harness).await,
            Preferences {
                list_sort: Some("title".to_string()),
                last_selected: Some("session-7".to_string()),
                compact: Some(true),
                remembered_permissions: None,
                remembered_workspace_trust: None,
                skip_host_remove_confirmation: Some(true),
                skip_host_setup_confirmation: Some(true),
                feedback_contact: None,
                terminal_link_click: None,
                skip_session_confirmations: None,
                quick_launch_template: None,
                template_shortcuts: Default::default(),
            },
            "each sparse patch lands its own field and keeps the others"
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
            .oneshot(put(serde_json::json!({ "compact": "yes" })))
            .await
            .unwrap();
        assert!(
            response.status().is_client_error(),
            "a compact preference is a typed boolean, not a truthy string"
        );
        assert_eq!(read(&harness).await.compact, Some(true));

        // `remembered_permissions` is validated the same way `list_sort` is,
        // against its own smaller vocabulary: a word this helm does not
        // serve is a 400 that leaves the row untouched, and every released
        // word is accepted like any other field.
        let response = harness
            .router()
            .oneshot(put(
                serde_json::json!({ "remembered_permissions": "always" }),
            ))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "a permissions word this helm does not serve is refused at the write"
        );
        // The refusal names every word it would have accepted; it used to
        // name only `yolo` after the vocabulary had grown beyond it.
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(
            body.contains("this helm serves guarded, yolo, approve, smart_approve, chat"),
            "{body}"
        );
        assert_eq!(
            read(&harness).await.remembered_permissions,
            None,
            "a refused permissions patch leaves the row unchanged"
        );
        for word in ["guarded", "approve", "smart_approve", "chat", "yolo"] {
            let response = harness
                .router()
                .oneshot(put(serde_json::json!({ "remembered_permissions": word })))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NO_CONTENT, "{word}");
            assert_eq!(
                read(&harness).await.remembered_permissions.as_deref(),
                Some(word),
                "every released permission word is accepted"
            );
        }

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
                compact: Some(true),
                remembered_permissions: Some("yolo".to_string()),
                remembered_workspace_trust: None,
                skip_host_remove_confirmation: Some(true),
                skip_host_setup_confirmation: Some(true),
                feedback_contact: None,
                terminal_link_click: None,
                skip_session_confirmations: None,
                quick_launch_template: None,
                template_shortcuts: Default::default(),
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
            .router()
            .oneshot(put(serde_json::json!({ "remembered_permissions": null })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            read(&harness).await.remembered_permissions,
            None,
            "null clears the remembered permissions mode like any other field"
        );
        assert_eq!(
            read(&harness).await.last_selected.as_deref(),
            Some(at_limit.as_str()),
            "clearing remembered_permissions must not disturb an unrelated field"
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
