//! Helm-wide consent to set up conversation hooks on the machines where an agent starts.
//!
//! The helm attaches the current setting to each start. The supervisor owns the write and freezes consent for retries
//! of an already accepted create. Hint dismissals live with the other shared preferences.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse as _, Response};
use farhelm_proto::{AgentKind, PersistentHookOptIns};

use crate::{AppState, http_error, store::HelmStore};

/// The SQL column is selected from the agent kind. An unsupported kind has no dedicated hook file to authorize.
#[warn(clippy::wildcard_enum_match_arm)]
fn opt_in_column(kind: AgentKind) -> Option<&'static str> {
    match kind {
        AgentKind::Grok => Some("grok_hook_setup"),
        AgentKind::Antigravity => Some("antigravity_hook_setup"),
        AgentKind::Claude
        | AgentKind::Codex
        | AgentKind::Goose
        | AgentKind::Pi
        | AgentKind::Omp
        | AgentKind::Muse
        | AgentKind::Generic => None,
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentHookOptIn {
    enabled: bool,
}

/// Read one switch. Unsupported agent kinds are rejected before the settings store is consulted.
pub(crate) async fn get(
    State(state): State<Arc<AppState>>,
    Path(kind): Path<AgentKind>,
) -> Response {
    if !kind.offers_persistent_hooks() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    match state.store.agent_hook_setup_enabled(kind).await {
        Ok(enabled) => Json(AgentHookOptIn { enabled }).into_response(),
        Err(error) => http_error(error),
    }
}

/// Save consent independently of launch defaults and notify connected clients after the write.
pub(crate) async fn put(
    State(state): State<Arc<AppState>>,
    Path(kind): Path<AgentKind>,
    Json(opt_in): Json<AgentHookOptIn>,
) -> Response {
    if !kind.offers_persistent_hooks() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    match crate::run_owned(async move {
        state
            .store
            .set_agent_hook_setup(kind, opt_in.enabled)
            .await?;
        state.manager.events().bump();
        Ok::<_, anyhow::Error>(())
    })
    .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => http_error(error),
    }
}

impl HelmStore {
    /// Read consent for a launch whose resolved agent kind is known. Plain commands and unrelated agents need no read.
    pub(crate) async fn agent_hook_setup_enabled(&self, kind: AgentKind) -> anyhow::Result<bool> {
        let Some(column) = opt_in_column(kind) else {
            return Ok(false);
        };
        self.conn()
            .call("agent hook setup setting read task panicked", move |conn| {
                Ok(conn.query_row(
                    &format!("SELECT {column} FROM checkout_config WHERE singleton = 1"),
                    [],
                    |row| row.get(0),
                )?)
            })
            .await
    }

    /// Read consent for both agents in one snapshot. The destination supervisor matches it to the session's kind.
    pub(crate) async fn agent_hook_setup_opt_ins(&self) -> anyhow::Result<PersistentHookOptIns> {
        self.conn()
            .call("agent hook setup settings read task panicked", |conn| {
                Ok(conn.query_row(
                    "SELECT grok_hook_setup, antigravity_hook_setup FROM checkout_config WHERE singleton = 1",
                    [],
                    |row| Ok(PersistentHookOptIns { grok: row.get(0)?, antigravity: row.get(1)? }),
                )?)
            })
            .await
    }

    /// Save one agent's choice without invalidating checkout previews. Launch admission reads the persisted value.
    pub(crate) async fn set_agent_hook_setup(
        &self,
        kind: AgentKind,
        enabled: bool,
    ) -> anyhow::Result<()> {
        let column = opt_in_column(kind)
            .ok_or_else(|| anyhow::anyhow!("this agent has no automatic hook setup"))?;
        self.conn()
            .call_full_sync_if(
                true,
                "agent hook setup setting write task panicked",
                move |conn| {
                    conn.execute(
                        &format!("UPDATE checkout_config SET {column} = ?1 WHERE singleton = 1"),
                        [enabled],
                    )?;
                    Ok(())
                },
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rest_harness;
    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request};
    use serde_json::{Value, json};
    use tower::ServiceExt as _;

    /// Drives the real settings routes so unknown settings, sparse writes, and the event announcing them all use
    /// the same path as a connected desktop.
    async fn request(
        h: &rest_harness::Harness,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Response {
        h.router()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("host", "127.0.0.1:7433")
                    .header("content-type", "application/json")
                    .body(body.map_or_else(Body::empty, |value| Body::from(value.to_string())))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    /// The switches authorize separate vendor files. The shared preferences can suppress or restore each agent's
    /// prompt and hint independently, including when another desktop saves a separate preference.
    #[farhelm_testtrace::test]
    async fn hook_setup_routes_keep_each_agent_and_its_dismissal_independent() {
        let h = rest_harness::idle_helm().await;
        let grok = "/api/agent-hook-setup/grok";
        let antigravity = "/api/agent-hook-setup/antigravity";
        let toggle = |enabled| Some(json!({"enabled": enabled}));
        for path in [grok, antigravity] {
            let response = request(&h, Method::GET, path, None).await;
            assert_eq!(response.status(), StatusCode::OK);
            let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
            assert_eq!(
                serde_json::from_slice::<Value>(&bytes).unwrap(),
                json!({"enabled": false})
            );
        }
        assert_eq!(
            request(
                &h,
                Method::PUT,
                "/api/agent-hook-setup/claude",
                toggle(true)
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );
        let events = h.state.manager.events();
        let mut changes = events.subscribe();
        assert_eq!(
            request(&h, Method::PUT, grok, toggle(true)).await.status(),
            StatusCode::NO_CONTENT
        );
        assert!(
            changes.has_changed().unwrap(),
            "saving consent did not wake the desktop"
        );
        changes.borrow_and_update();
        assert!(
            h.store
                .agent_hook_setup_enabled(AgentKind::Grok)
                .await
                .unwrap()
        );
        assert!(
            !h.store
                .agent_hook_setup_enabled(AgentKind::Antigravity)
                .await
                .unwrap()
        );

        for body in [
            json!({"grok_hook_hint_dismissed": true}),
            json!({"antigravity_hook_hint_dismissed": true}),
            json!({"grok_hook_hint_dismissed": null}),
        ] {
            assert_eq!(
                request(&h, Method::PUT, "/api/preferences", Some(body))
                    .await
                    .status(),
                StatusCode::NO_CONTENT
            );
            assert!(
                changes.has_changed().unwrap(),
                "saving the dismissal did not wake the desktop"
            );
            changes.borrow_and_update();
        }
        let prefs = h.store.preferences().await.unwrap();
        assert_eq!(prefs.grok_hook_hint_dismissed, None);
        assert_eq!(prefs.antigravity_hook_hint_dismissed, Some(true));
        assert_eq!(
            request(&h, Method::PUT, grok, toggle(false)).await.status(),
            StatusCode::NO_CONTENT
        );
        assert!(
            !h.store
                .agent_hook_setup_enabled(AgentKind::Grok)
                .await
                .unwrap()
        );
    }

    /// An upgrade from a database without consent or dismissals must start both switches off and leave existing
    /// settings alone. A second open must preserve consent that was deliberately saved after the upgrade.
    #[farhelm_testtrace::test]
    async fn hook_setup_schema_95_requires_new_consent_and_persists_it() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("helm.db");
        let store = HelmStore::open_fixture(&path).await.unwrap();
        store.set_claude_trust(true).await.unwrap();
        drop(store);
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute_batch(
                "ALTER TABLE checkout_config DROP COLUMN grok_hook_setup;
                ALTER TABLE checkout_config DROP COLUMN antigravity_hook_setup;
                ALTER TABLE preferences DROP COLUMN grok_hook_hint_dismissed;
                ALTER TABLE preferences DROP COLUMN antigravity_hook_hint_dismissed;
                PRAGMA user_version = 94;",
            )
            .unwrap();
            assert!(
                conn.prepare("SELECT grok_hook_setup FROM checkout_config")
                    .is_err()
            );
            assert!(
                conn.prepare("SELECT antigravity_hook_hint_dismissed FROM preferences")
                    .is_err()
            );
        }
        let store = HelmStore::open_fixture(&path).await.unwrap();
        assert!(
            store
                .checkout_config_snapshot(None)
                .await
                .unwrap()
                .claude_trust
        );
        assert!(store.agent_hook_setup_opt_ins().await.unwrap().is_empty());
        let prefs = store.preferences().await.unwrap();
        assert_eq!(prefs.grok_hook_hint_dismissed, None);
        assert_eq!(prefs.antigravity_hook_hint_dismissed, None);
        store
            .set_agent_hook_setup(AgentKind::Antigravity, true)
            .await
            .unwrap();
        store
            .update_preferences(crate::store::PreferencePatch {
                antigravity_hook_hint_dismissed: Some(Some(true)),
                ..Default::default()
            })
            .await
            .unwrap();
        drop(store);
        let reopened = HelmStore::open_fixture(&path).await.unwrap();
        assert!(
            reopened
                .agent_hook_setup_enabled(AgentKind::Antigravity)
                .await
                .unwrap()
        );
        assert!(
            !reopened
                .agent_hook_setup_enabled(AgentKind::Grok)
                .await
                .unwrap()
        );
        assert_eq!(
            reopened
                .preferences()
                .await
                .unwrap()
                .antigravity_hook_hint_dismissed,
            Some(true)
        );
    }
}
