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

/// The catalog as schema 100 created it, which the step to schema 100 still creates so that the later
/// rebuild ([`migrate_registration_seq`]) runs on the shape every older database has.
pub(crate) const SCHEMA_V100: &str = "CREATE TABLE IF NOT EXISTS custom_agents (
    id TEXT PRIMARY KEY,
    spec_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;";

/// The current catalog, for the fresh schema and the rebuild in [`migrate_registration_seq`].
///
/// `registration_seq` tells registrations apart when an ID is deleted and registered again. It is
/// `AUTOINCREMENT` because only that keeps SQLite from handing a deleted row's number to the next one
/// (a plain rowid is reused once the largest row is gone), and `created_at` has one-second
/// resolution, so neither the ID nor a timestamp can prove a launch came from the row that is there
/// now. `remembered_wrapper_prefix` is the JSON word list of this registration's last explicitly
/// launched wrapper, or NULL for none; keeping it on the row means deleting the registration drops
/// it in the same statement.
pub(crate) const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS custom_agents (
    registration_seq INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL UNIQUE,
    spec_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    remembered_wrapper_prefix TEXT
) STRICT;";

/// Which catalog row a launch was compiled from: the row's `registration_seq`.
///
/// A launch can take minutes between reading its registration and reporting success, and in
/// between the user may delete that agent or delete it and register the same ID again. Memory the
/// launch writes on success is fenced on this value, so it lands only on the registration the
/// launch actually used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Registration(pub(crate) i64);

/// A catalog entry together with the registration it belongs to.
pub(crate) struct RegisteredAgent {
    pub(crate) agent: CustomAgent,
    pub(crate) registration: Registration,
}

/// Rebuild schema 102's catalog into [`SCHEMA`]'s shape, keeping every row's ID, definition and
/// timestamps. Existing rows are numbered in ID order and start with no remembered wrapper: the
/// shared custom-agent wrapper they had before cannot be attributed to any one of them.
pub(crate) fn migrate_registration_seq(tx: &rusqlite::Transaction<'_>) -> anyhow::Result<()> {
    tx.execute_batch(&format!(
        "ALTER TABLE custom_agents RENAME TO custom_agents_v102;
         {SCHEMA}
         INSERT INTO custom_agents (id, spec_json, created_at, updated_at)
             SELECT id, spec_json, created_at, updated_at FROM custom_agents_v102 ORDER BY id;
         DROP TABLE custom_agents_v102;"
    ))
    .context("adding custom agent registration numbers")
}

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

    /// Take the capabilities and command for one launch from the same row, before the helm builds either argv,
    /// with the registration that row is, so that what the launch later remembers can be fenced on it.
    pub(crate) async fn custom_agent(&self, id: &str) -> anyhow::Result<Option<RegisteredAgent>> {
        let id = id.to_owned();
        self.conn()
            .call("reading a custom agent", move |conn| {
                let row: Option<(String, i64)> = conn
                    .query_row(
                        "SELECT spec_json, registration_seq FROM custom_agents WHERE id = ?1",
                        [&id],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()?;
                row.map(|(json, seq)| {
                    Ok(RegisteredAgent {
                        agent: serde_json::from_str(&json)
                            .context("decoding a saved custom agent")?,
                        registration: Registration(seq),
                    })
                })
                .transpose()
            })
            .await
    }

    /// Apply a valid definition in one SQL statement. Create refuses an existing ID. Save refuses to recreate a
    /// deleted registration, so an open editor cannot put it back after another writer removes it.
    pub(crate) async fn save_custom_agent(
        &self,
        agent: &CustomAgent,
        create: bool,
    ) -> anyhow::Result<()> {
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

    /// Drop only the registration, and with it the wrapper prefix it remembers, since both live on its row.
    /// Templates and sessions can continue showing its stable ID.
    pub(crate) async fn delete_custom_agent(&self, id: String) -> anyhow::Result<()> {
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

/// Every registration's remembered wrapper prefix, keyed by ID, for the shared preferences projection.
/// Registrations without one, or whose stored prefix does not decode, are left out, which the GUI reads as "start
/// without a wrapper".
pub(crate) fn remembered_wrapper_prefixes(
    conn: &rusqlite::Connection,
) -> anyhow::Result<std::collections::BTreeMap<String, Vec<String>>> {
    let mut stmt = conn.prepare(
        "SELECT id, remembered_wrapper_prefix FROM custom_agents
         WHERE remembered_wrapper_prefix IS NOT NULL ORDER BY id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut prefixes = std::collections::BTreeMap::new();
    for row in rows {
        let (id, json) = row?;
        // Only the helm writes this column, but it is a convenience: one entry that does not decode reads as "no
        // remembered prefix" rather than failing the whole preferences read, which tunnels, updates and approvals
        // also depend on.
        if let Ok(prefix) = serde_json::from_str(&json) {
            prefixes.insert(id, prefix);
        }
    }
    Ok(prefixes)
}

/// Remember `prefix` (or, for `None`, no wrapper) as the last explicitly launched wrapper of the registration
/// a successful launch was compiled from, inside the caller's history transaction.
///
/// The update is fenced on both the ID and `registration`, so a launch whose agent was deleted while it ran
/// writes nothing, and one whose agent was deleted and registered again under the same ID cannot hand its
/// prefix to the new registration. Neither is an error: the memory is a convenience for the next New.
pub(crate) fn remember_wrapper_prefix(
    tx: &rusqlite::Transaction<'_>,
    id: &str,
    registration: Registration,
    prefix: Option<&[String]>,
) -> anyhow::Result<()> {
    let json = prefix.map(serde_json::to_string).transpose()?;
    tx.execute(
        "UPDATE custom_agents SET remembered_wrapper_prefix = ?1
         WHERE id = ?2 AND registration_seq = ?3",
        rusqlite::params![json, id, registration.0],
    )
    .context("remembering a custom agent's wrapper prefix")?;
    Ok(())
}

/// Report a registration that disappeared before a catalog edit or deletion, or between a launch's
/// selection and its compilation.
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

    /// Spec: opening a schema 102 database keeps every registered agent's ID, definition and timestamps, numbers
    /// the registrations in ID order with nothing remembered yet, and never hands a deleted registration's number to
    /// a later one, even when the deleted one had the largest.
    ///
    /// Why: the number is what fences a launch's remembered wrapper to the registration it ran from, so a reused
    /// number would let a late launch of a deleted agent write onto a new agent registered under the same ID.
    #[farhelm_testtrace::test]
    async fn schema_102_numbers_registrations_that_are_never_reused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("helm.db");
        let store = HelmStore::open_fixture(&path).await.unwrap();
        {
            let db = store.conn();
            let conn = db.lock();
            conn.execute_batch(&format!(
                "DROP TABLE custom_agents;
                 {SCHEMA_V100}
                 INSERT INTO custom_agents (id, spec_json, created_at, updated_at) VALUES
                     ('zeta', '{{\"id\":\"zeta\",\"display_name\":\"Zeta\",\"command\":\"zeta\",\"supports_reporting\":false,\"supports_resumption\":false}}', 5, 6),
                     ('acme', '{{\"id\":\"acme\",\"display_name\":\"Acme\",\"command\":\"acme\",\"supports_reporting\":true,\"supports_resumption\":false}}', 7, 8);
                 PRAGMA user_version = 102;"
            ))
            .unwrap();
            let (version, numbered): (i64, bool) = conn
                .query_row(
                    "SELECT (SELECT user_version FROM pragma_user_version),
                            EXISTS (SELECT 1 FROM pragma_table_info('custom_agents')
                                    WHERE name = 'registration_seq')",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(
                (version, numbered),
                (102, false),
                "premise: a schema 102 catalog"
            );
        }
        drop(store);
        let reopened = HelmStore::open_fixture(&path).await.unwrap();
        let ids: Vec<String> = reopened
            .custom_agents()
            .await
            .unwrap()
            .into_iter()
            .map(|agent| agent.id)
            .collect();
        assert_eq!(ids, ["acme", "zeta"], "every registration survives");
        let rows: Vec<(i64, String, i64, i64)> = {
            let db = reopened.conn();
            let conn = db.lock();
            let mut stmt = conn
                .prepare(
                    "SELECT registration_seq, id, created_at, updated_at FROM custom_agents
                     WHERE remembered_wrapper_prefix IS NULL ORDER BY registration_seq",
                )
                .unwrap();
            stmt.query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
        };
        assert_eq!(
            rows,
            [(1, "acme".to_owned(), 7, 8), (2, "zeta".to_owned(), 5, 6)],
            "numbered in ID order, timestamps kept, nothing remembered"
        );

        reopened.delete_custom_agent("zeta".into()).await.unwrap();
        let agent: CustomAgent =
            serde_json::from_value(json!({"id":"zeta", "display_name":"Zeta again",
            "command":"zeta", "supports_reporting":false, "supports_resumption":false}))
            .unwrap();
        reopened.save_custom_agent(&agent, true).await.unwrap();
        let again = reopened
            .custom_agent("zeta")
            .await
            .unwrap()
            .unwrap()
            .registration;
        assert_eq!(
            again,
            Registration(3),
            "the deleted largest number is not handed out again"
        );
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
