//! The helm-owned catalog of user-authored agent integrations. It supplies a launch's executable and capabilities
//! before dispatch. Sessions retain that compiled launch, so later catalog writes never rewrite supervisor state.

use std::sync::Arc;

use anyhow::Context as _;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use farhelm_proto::{
    AgentListingEntry, AgentListingKind, ErrorKind, LaunchHarness, custom_agents::CustomAgent,
};
use rusqlite::OptionalExtension as _;

use crate::{AppState, SupervisorError, http_error, store::HelmStore};

/// Shared by the fresh schema and the migration so both admit the same catalog rows.
pub(crate) const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS custom_agents (
    id TEXT PRIMARY KEY,
    spec_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;";

impl HelmStore {
    /// Read one ordered catalog snapshot without contacting any launch host. The UI channel can project it on any
    /// helm event. A corrupted entry fails the read rather than disappearing while its ID stays taken.
    pub(crate) async fn custom_agents(&self) -> anyhow::Result<Vec<CustomAgent>> {
        self.conn()
            .call("reading the custom agent catalog", |conn| {
                let mut stmt = conn.prepare("SELECT spec_json FROM custom_agents ORDER BY id")?;
                let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
                rows.map(|row| serde_json::from_str(&row?).context("decoding a saved custom agent"))
                    .collect()
            })
            .await
    }

    /// Take the capabilities and command for one launch from the same row, before the helm builds either argv.
    pub(crate) async fn custom_agent(&self, id: &str) -> anyhow::Result<Option<CustomAgent>> {
        let id = id.to_owned();
        self.conn()
            .call("reading a custom agent", move |conn| {
                let spec: Option<String> = conn
                    .query_row(
                        "SELECT spec_json FROM custom_agents WHERE id = ?1",
                        [&id],
                        |row| row.get(0),
                    )
                    .optional()?;
                spec.map(|json| {
                    serde_json::from_str(&json).context("decoding a saved custom agent")
                })
                .transpose()
            })
            .await
    }

    /// Apply a valid definition in one SQL statement. Create refuses an existing ID. Save refuses to recreate a
    /// deleted registration, so an open editor cannot put it back after another writer removes it.
    async fn save_custom_agent(&self, agent: &CustomAgent, create: bool) -> anyhow::Result<()> {
        agent
            .validate()
            .map_err(|reason| crate::sessions::invalid_request(reason.into()))?;
        let spec = serde_json::to_string(agent).context("encoding a custom agent")?;
        let id = agent.id.clone();
        let changed = self.conn()
            .call("storing a custom agent", move |conn| {
                let sql = if create {
                    "INSERT INTO custom_agents (id, spec_json, created_at, updated_at)
                     VALUES (?1, ?2, unixepoch(), unixepoch()) ON CONFLICT (id) DO NOTHING"
                } else {
                    "UPDATE custom_agents SET spec_json = ?2, updated_at = unixepoch() WHERE id = ?1"
                };
                Ok(conn.execute(sql, rusqlite::params![id, spec])?)
            })
            .await?;
        if changed != 0 {
            return Ok(());
        }
        Err(if create {
            SupervisorError::helm(
                ErrorKind::Conflict,
                "a custom agent already uses this ID".into(),
            )
            .into()
        } else {
            missing_agent()
        })
    }

    /// Drop only the registration. Templates and sessions can continue showing its stable ID.
    async fn delete_custom_agent(&self, id: String) -> anyhow::Result<()> {
        let changed = self
            .conn()
            .call("deleting a custom agent", move |conn| {
                Ok(conn.execute("DELETE FROM custom_agents WHERE id = ?1", [id])?)
            })
            .await?;
        if changed == 0 {
            Err(missing_agent())
        } else {
            Ok(())
        }
    }
}

/// Report a registration that disappeared before a catalog edit or deletion.
pub(crate) fn missing_agent() -> anyhow::Error {
    SupervisorError::helm(
        ErrorKind::NotFound,
        "the custom agent is no longer registered".into(),
    )
    .into()
}

/// Refuse an agent's `--agent` value that names neither a built-in type nor a custom agent registered here.
///
/// Use the same catalog snapshot that ruled out the ID so the caller gets the choices it could have used on
/// this request. A valid-looking custom ID might be a typo of a built-in type.
pub(crate) fn unknown_agent(id: &str, registered: &[CustomAgent]) -> anyhow::Error {
    let built_ins: Vec<String> = built_in_agents().map(|(_, id)| id).collect();
    let custom = if registered.is_empty() {
        "no custom agents are registered".to_owned()
    } else {
        let ids: Vec<&str> = registered.iter().map(|agent| agent.id.as_str()).collect();
        format!("registered custom agents are {}", ids.join(", "))
    };
    SupervisorError::helm(
        ErrorKind::NotFound,
        format!(
            "{id:?} is not a built-in agent type or a custom agent registered on this helm; \
             built-in types are {}. {custom}",
            built_ins.join(", ")
        ),
    )
    .into()
}

/// Use the serialized spelling the CLI parses, in the fixed harness declaration order.
fn built_in_agents() -> impl Iterator<Item = (LaunchHarness, String)> {
    LaunchHarness::BUILT_INS
        .iter()
        .copied()
        .filter_map(|harness| match serde_json::to_value(harness) {
            Ok(serde_json::Value::String(id)) => Some((harness, id)),
            _ => None,
        })
}

/// Agent discovery exposes launch choices from the helm catalog alone. It needs no host probe,
/// and its dedicated reply shape keeps commands out of both the human and JSON listings.
pub(crate) async fn agent_listing(store: &HelmStore) -> anyhow::Result<Vec<AgentListingEntry>> {
    let registered = store.custom_agents().await?;
    let custom = LaunchHarness::Custom;
    Ok(built_in_agents()
        .map(|(harness, id)| AgentListingEntry {
            id,
            kind: AgentListingKind::BuiltIn,
            display_name: harness.display_name().into(),
            supports_named_conversation: harness.offers_named_conversation(),
            supports_initial_prompt: harness.supports_initial_prompt(false),
            supports_model: harness.offers_model(),
            supports_effort: harness.offers_effort(),
        })
        .chain(registered.into_iter().map(|agent| AgentListingEntry {
            id: agent.id,
            kind: AgentListingKind::Custom,
            display_name: agent.display_name,
            supports_named_conversation: agent.supports_resumption,
            supports_initial_prompt: custom.supports_initial_prompt(false),
            supports_model: custom.offers_model(),
            supports_effort: custom.offers_effort(),
        }))
        .collect())
}

/// Used for the Settings view and the channel's keyed catalog projection.
pub(crate) async fn list(State(state): State<Arc<AppState>>) -> Response {
    match state.store.custom_agents().await {
        Ok(agents) => Json(serde_json::json!({"custom_agents": agents})).into_response(),
        Err(error) => http_error(error),
    }
}

/// Register an ID and return the definition this request saved.
pub(crate) async fn create(
    State(state): State<Arc<AppState>>,
    Json(agent): Json<CustomAgent>,
) -> Response {
    write(&state, agent, true).await
}

/// Replace the editable fields of this ID. The path and body must agree because existing references use the ID.
pub(crate) async fn save(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(agent): Json<CustomAgent>,
) -> Response {
    if agent.id != id {
        return http_error(crate::sessions::invalid_request(
            "a custom agent's ID cannot be changed".into(),
        ));
    }
    write(&state, agent, false).await
}

/// The router owns state-changing requests through commit and notification even if the client disconnects.
async fn write(state: &AppState, agent: CustomAgent, create: bool) -> Response {
    match state.store.save_custom_agent(&agent, create).await {
        Ok(()) => {
            state.manager.events().bump();
            let status = if create {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            };
            (status, Json(agent)).into_response()
        }
        Err(error) => http_error(error),
    }
}

/// Broadcast removal so the desktop selection can fall back to Add custom agent immediately.
pub(crate) async fn delete(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    match state.store.delete_custom_agent(id).await {
        Ok(()) => {
            state.manager.events().bump();
            StatusCode::NO_CONTENT.into_response()
        }
        Err(error) => http_error(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rest_harness;
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::{Value, json};
    use tower::ServiceExt as _;

    /// Drive the same route and request-ownership middleware used by the Settings editor.
    async fn request(
        h: &rest_harness::Harness,
        method: &str,
        path: &str,
        body: Value,
    ) -> (StatusCode, Value) {
        let request = Request::builder()
            .method(method)
            .uri(path)
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let response = h.router().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    /// Creation claims one ID. Renaming the display name preserves that ID and deleting it stops new lookups.
    /// A stale editor cannot use PUT to bring a deleted registration back into the catalog.
    #[farhelm_testtrace::test]
    async fn the_catalog_keeps_registration_ids_stable() {
        let h = rest_harness::idle_helm().await;
        let catalog = "/api/custom-agents";
        let item = "/api/custom-agents/acme-code";
        let original = json!({"id":"acme-code", "display_name":"Acme Code", "command":"~/bin/acme-code",
            "supports_reporting":true, "supports_resumption":false});
        assert_eq!(
            request(&h, "GET", catalog, Value::Null).await,
            (StatusCode::OK, json!({"custom_agents":[]}))
        );
        let mut reserved = original.clone();
        reserved["id"] = json!("codex");
        assert_eq!(
            request(&h, "POST", catalog, reserved).await.0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            request(&h, "POST", catalog, original.clone()).await,
            (StatusCode::CREATED, original.clone())
        );
        assert_eq!(
            request(&h, "POST", catalog, original.clone()).await.0,
            StatusCode::CONFLICT
        );
        let mut updated = original.clone();
        updated["id"] = json!("other-agent");
        assert_eq!(
            request(&h, "PUT", item, updated.clone()).await.0,
            StatusCode::BAD_REQUEST
        );
        updated["id"] = original["id"].clone();
        updated["display_name"] = json!("Acme Code Renamed");
        assert_eq!(
            request(&h, "PUT", item, updated.clone()).await,
            (StatusCode::OK, updated.clone())
        );
        assert_eq!(
            request(&h, "GET", catalog, Value::Null).await.1,
            json!({"custom_agents":[updated.clone()]})
        );
        assert_eq!(
            request(&h, "DELETE", item, Value::Null).await.0,
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            request(&h, "PUT", item, updated).await.0,
            StatusCode::NOT_FOUND
        );
        assert!(h.store.custom_agents().await.unwrap().is_empty());
    }

    /// Opening yesterday's database creates the catalog without rewriting its pre-existing tables.
    #[farhelm_testtrace::test]
    async fn schema_99_gains_an_empty_custom_agent_catalog() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("helm.db");
        let store = HelmStore::open_fixture(&path).await.unwrap();
        {
            let db = store.conn();
            let conn = db.lock();
            conn.execute_batch("DROP TABLE custom_agents; PRAGMA user_version = 99;")
                .unwrap();
        }
        drop(store);
        let reopened = HelmStore::open_fixture(&path).await.unwrap();
        assert!(reopened.custom_agents().await.unwrap().is_empty());
        assert_eq!(
            reopened.list_hosts().await.unwrap().len(),
            1,
            "the reserved local row survives"
        );
    }
}
