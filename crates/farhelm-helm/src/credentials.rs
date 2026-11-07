//! A write-only catalog: callers can replace secrets but can only read metadata.
//!
//! Secret-bearing rows never implement Debug or Serialize. Routes deliberately
//! discard database and JSON diagnostics, which can include submitted contents.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use farhelm_proto::credentials::{
    CreateCredential, Credential, CredentialKind, SaveCredential, check_secret,
};
use rusqlite::{OptionalExtension, params};

use crate::{AppState, store::HelmStore};

/// JSON encoding can expand a bounded pasted file; the value itself has a
/// separate smaller limit, shared with the client validation.
pub(crate) const BODY_LIMIT: usize = 512 * 1024;

/// Refusals are fixed text: neither a malformed paste nor a storage failure
/// has authority to put credential contents into a response or a log.
#[derive(Debug)]
pub(crate) enum Refusal {
    Invalid(&'static str),
    Exists,
    Missing,
    InUse,
    Storage,
}

impl Refusal {
    fn response(self) -> Response {
        let (status, reason) = match self {
            Self::Invalid(reason) => (StatusCode::BAD_REQUEST, reason),
            Self::Exists => (StatusCode::CONFLICT, "a credential already has that name"),
            Self::Missing => (StatusCode::NOT_FOUND, "the credential no longer exists"),
            Self::InUse => (
                StatusCode::CONFLICT,
                "a recipe uses this credential; change that recipe before deleting it",
            ),
            Self::Storage => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Farhelm could not access the credential catalog",
            ),
        };
        (status, reason).into_response()
    }
}

/// Decode only the non-secret columns selected by catalog reads.
fn metadata(row: &rusqlite::Row<'_>) -> rusqlite::Result<Credential> {
    let kind: String = row.get(1)?;
    let kind = serde_json::from_str(&kind).map_err(|_| rusqlite::Error::InvalidQuery)?;
    Ok(Credential {
        name: row.get(0)?,
        kind,
        created_at: row.get(2)?,
        updated_at: row.get(3)?,
        revision: u64::try_from(row.get::<_, i64>(4)?)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        used_by: Vec::new(),
    })
}

impl HelmStore {
    /// List metadata without even selecting the secret column.
    pub(crate) async fn credentials(&self) -> Result<Vec<Credential>, Refusal> {
        self.conn()
            .call("credential metadata read", |conn| {
                let mut statement = conn.prepare(
                    "SELECT name, kind, created_at, updated_at, revision
                     FROM credentials ORDER BY name",
                )?;
                let mut rows = statement
                    .query_map([], metadata)?
                    .collect::<Result<Vec<_>, _>>()?;
                for row in &mut rows {
                    row.used_by = users(conn, &row.name)?;
                }
                Ok(rows)
            })
            .await
            .map_err(|_| Refusal::Storage)
    }

    /// Create a fixed name and kind, refusing collisions instead of replacing
    /// whatever another Settings window saved under that name.
    pub(crate) async fn create_credential(
        &self,
        create: CreateCredential,
    ) -> Result<Credential, Refusal> {
        farhelm_proto::tensorlake::recipes::check_recipe_name(&create.name)
            .map_err(|_| Refusal::Invalid("credential names use lowercase letters, digits and hyphens, start with a letter, end with a letter or digit, and have at most 32 characters"))?;
        check_secret(create.kind, &create.secret).map_err(Refusal::Invalid)?;
        let name = create.name;
        let kind = serde_json::to_string(&create.kind).map_err(|_| Refusal::Storage)?;
        let secret = create.secret.trim().to_owned();
        // Names can be reused after deletion and revisions restart at one.
        // Delivery must distinguish that new credential from an old seed.
        let incarnation = uuid::Uuid::new_v4().to_string();
        let now = crate::tensorlake::recipes::now_secs();
        self.conn()
            .call("credential create", move |conn| {
                let tx =
                    conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                let inserted = tx.execute(
                    "INSERT INTO credentials (name, kind, secret, created_at, updated_at, revision, incarnation)
                     VALUES (?1, ?2, ?3, ?4, ?4, 1, ?5) ON CONFLICT (name) DO NOTHING",
                    params![name, kind, secret, now, incarnation],
                )?;
                if inserted == 0 {
                    return Ok(Err(Refusal::Exists));
                }
                // A second Settings window may replace or delete immediately
                // after commit; acknowledge precisely the row this save made.
                let row = read_metadata(&tx, &name)?;
                tx.commit()?;
                Ok(Ok(row))
            })
            .await
            .map_err(|_| Refusal::Storage)?
    }

    /// Resolve a blank replacement against the current row under the same
    /// immediate transaction as the write. Revisions never wrap or become REAL
    /// when SQLite's signed integer range has been exhausted.
    pub(crate) async fn save_credential(
        &self,
        name: String,
        save: SaveCredential,
    ) -> Result<Credential, Refusal> {
        self.conn()
            .call("credential replacement", move |conn| {
                let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                let previous: Option<(String, i64)> = tx
                    .query_row(
                        "SELECT kind, revision FROM credentials WHERE name = ?1",
                        [&name],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()?;
                let Some((kind, revision)) = previous else {
                    return Ok(Err(Refusal::Missing));
                };
                if let Some(secret) = save.secret.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                    let kind: CredentialKind = serde_json::from_str(&kind)?;
                    if let Err(reason) = check_secret(kind, secret) {
                        return Ok(Err(Refusal::Invalid(reason)));
                    }
                    let Some(next) = revision.checked_add(1) else {
                        return Ok(Err(Refusal::Storage));
                    };
                    tx.execute(
                        "UPDATE credentials SET secret = ?2, updated_at = ?3, revision = ?4 WHERE name = ?1",
                        params![name, secret, crate::tensorlake::recipes::now_secs(), next],
                    )?;
                }
                let row = read_metadata(&tx, &name)?;
                tx.commit()?;
                Ok(Ok(row))
            })
            .await
            .map_err(|_| Refusal::Storage)?
    }

    /// Refuse deletion while any recipe names the credential. The immediate
    /// transaction also excludes a recipe writer admitting a new reference.
    pub(crate) async fn delete_credential(&self, name: String) -> Result<(), Refusal> {
        let deleted = self
            .conn()
            .call("credential delete", move |conn| {
                let tx =
                    conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                if !users(&tx, &name)?.is_empty() {
                    return Ok(Err(Refusal::InUse));
                }
                let deleted = tx.execute("DELETE FROM credentials WHERE name = ?1", [name])?;
                tx.commit()?;
                Ok(Ok(deleted))
            })
            .await
            .map_err(|_| Refusal::Storage)??;
        if deleted == 0 {
            Err(Refusal::Missing)
        } else {
            Ok(())
        }
    }
}

/// Read the acknowledgement while its writer still excludes competing edits.
fn read_metadata(conn: &rusqlite::Connection, name: &str) -> rusqlite::Result<Credential> {
    let mut row = conn.query_row(
        "SELECT name, kind, created_at, updated_at, revision FROM credentials WHERE name = ?1",
        [name],
        metadata,
    )?;
    row.used_by = users(conn, name)?;
    Ok(row)
}

/// The same reference query feeds metadata and delete admission. Recipe writers
/// validate references under an immediate transaction too, so independent
/// connections cannot leave a saved recipe pointing at a deleted credential.
fn users(conn: &rusqlite::Connection, name: &str) -> rusqlite::Result<Vec<String>> {
    let mut statement = conn.prepare(
        "SELECT name FROM tensorlake_recipes
         WHERE json_extract(spec_json, '$.claude.token_credential') = ?1
            OR json_extract(spec_json, '$.codex.token_credential') = ?1
            OR json_extract(spec_json, '$.codex_login_credential') = ?1
            OR json_extract(spec_json, '$.claude_login_credential') = ?1
            OR json_extract(spec_json, '$.github_token_credential') = ?1 ORDER BY name",
    )?;
    statement.query_map([name], |row| row.get(0))?.collect()
}

/// Saving a recipe checks identity and kind without loading secret values.
pub(crate) fn has_kind(
    conn: &rusqlite::Connection,
    name: &str,
    kind: CredentialKind,
) -> rusqlite::Result<bool> {
    let kind = serde_json::to_string(&kind).map_err(|_| rusqlite::Error::InvalidQuery)?;
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM credentials WHERE name = ?1 AND kind = ?2)",
        params![name, kind],
        |row| row.get(0),
    )
}

/// Resolve a value, revision and creation identity together for host delivery. Callers
/// keep the result transient and non-Debug/non-Serialize; reading the revision
/// separately could record a newer value as delivered when only the old one was.
pub(crate) fn secret(
    conn: &rusqlite::Connection,
    name: &str,
    kind: CredentialKind,
) -> rusqlite::Result<Option<(String, u64, String)>> {
    let kind = serde_json::to_string(&kind).map_err(|_| rusqlite::Error::InvalidQuery)?;
    conn.query_row(
        "SELECT secret, revision, incarnation FROM credentials WHERE name = ?1 AND kind = ?2",
        params![name, kind],
        |row| {
            Ok((
                row.get(0)?,
                u64::try_from(row.get::<_, i64>(1)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
                row.get(2)?,
            ))
        },
    )
    .optional()
}

/// Catalog reads return metadata only, including after a failed replacement.
pub(crate) async fn list(State(state): State<Arc<AppState>>) -> Response {
    match state.store.credentials().await {
        Ok(rows) => Json(rows).into_response(),
        Err(reason) => reason.response(),
    }
}

/// Reject malformed bodies without serde's diagnostic, which may quote secrets.
pub(crate) async fn create(
    State(state): State<Arc<AppState>>,
    body: Result<Json<CreateCredential>, JsonRejection>,
) -> Response {
    let Ok(Json(create)) = body else {
        return Refusal::Invalid("could not read the credential request").response();
    };
    match state.store.create_credential(create).await {
        Ok(row) => {
            state.manager.events().bump();
            (StatusCode::CREATED, Json(row)).into_response()
        }
        Err(reason) => reason.response(),
    }
}

/// A replacement changes only its fixed identity's secret and revision.
pub(crate) async fn save(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    body: Result<Json<SaveCredential>, JsonRejection>,
) -> Response {
    let Ok(Json(save)) = body else {
        return Refusal::Invalid("could not read the credential request").response();
    };
    let replaced = save
        .secret
        .as_deref()
        .is_some_and(|secret| !secret.trim().is_empty());
    // A disconnected Settings client cannot leave a committed replacement
    // without its corresponding push. The helm owns both phases together.
    crate::run_owned(async move {
        match state.store.save_credential(name, save).await {
            Ok(row) => {
                state.manager.events().bump();
                let delivery_failures = if replaced {
                    state
                        .tensorlake
                        .credential_delivery()
                        .push(&row.name, state.manager.events())
                        .await
                } else {
                    Vec::new()
                };
                Json(farhelm_proto::credentials::SavedCredential {
                    credential: row,
                    delivery_failures,
                })
                .into_response()
            }
            Err(reason) => reason.response(),
        }
    })
    .await
}

/// Deletion never needs to select, deserialize or return the saved value.
pub(crate) async fn delete(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Response {
    match state.store.delete_credential(name).await {
        Ok(()) => {
            state.manager.events().bump();
            Json(serde_json::json!({})).into_response()
        }
        Err(reason) => reason.response(),
    }
}

#[cfg(test)]
#[path = "credentials_tests.rs"]
mod tests;
