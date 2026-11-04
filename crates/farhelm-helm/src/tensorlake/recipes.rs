//! The helm's Tensorlake recipes: storage and the `/api/tensorlake/recipes`
//! routes.
//!
//! A recipe is a named description of the sandboxes Farhelm creates for
//! Tensorlake hosts (size, software source, agent endpoints, outbound
//! tunnels); the wire shapes and their validation live in
//! `farhelm_proto::tensorlake::recipes`. This module stores recipes in the
//! `tensorlake_recipes` table and serves them:
//!
//! - `GET /api/tensorlake/recipes` lists every recipe, without secrets.
//! - `POST /api/tensorlake/recipes` creates one; an existing name is a 409,
//!   never an overwrite, because "New recipe" in the editor must not replace a
//!   recipe someone else created meanwhile.
//! - `PUT /api/tensorlake/recipes/{name}` replaces an existing recipe's
//!   definition; a missing name is a 404 (a PUT never creates). The URL is the
//!   name's only authority, so names cannot change.
//! - `DELETE /api/tensorlake/recipes/{name}` removes one, refused while a host
//!   uses it.
//!
//! This is deliberately not launch templates' last-write-wins "PUT creates or
//! replaces": names are immutable and create versus update are distinct
//! operations, and the row carries helm-owned build state beside the user's
//! spec (see `TENSORLAKE_RECIPES_SCHEMA` in `store.rs`). Every write here
//! touches only `spec_json`; nothing in this change writes `state_json`.
//!
//! Endpoint tokens are stored unencrypted with the rest of the spec, like the
//! ntfy token, and never returned: the read shape says only whether one is
//! saved. Because a request body can carry a token, a body that does not
//! decode gets a fixed refusal instead of serde's diagnostic, which can quote
//! the submitted text, and no stored-type here implements `Debug`.
//!
//! Nothing here calls Tensorlake. Snapshot builds (and the cleanup a delete
//! must then do first) arrive with the build feature.

use std::sync::Arc;

use anyhow::Context as _;
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use farhelm_proto::tensorlake::recipes::{
    CreateRecipe, EndpointSave, EndpointView, RecipeSave, RecipeSize, RecipeSource, RecipeView,
    RecipesView, Tunnel, check_recipe, check_recipe_name,
};
use rusqlite::{Connection, OptionalExtension as _};
use serde::{Deserialize, Serialize};

use crate::{AppState, store::HelmStore};

/// The request body cap for recipe writes. 500 typical package names, a few
/// tunnels and two endpoints fit with room to spare; the per-field caps in
/// `check_recipe` bound what a body within this cap can store.
pub(crate) const BODY_LIMIT: usize = 64 * 1024;

// ---------------------------------------------------------------------------
// Stored shape
// ---------------------------------------------------------------------------

/// An agent endpoint as `spec_json` stores it: the token in the clear.
///
/// No `Debug`: this carries a secret.
#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct StoredEndpoint {
    pub(crate) base_url: String,
    pub(crate) token: Option<String>,
}

/// A recipe's `spec_json`: the user's definition, secrets included.
///
/// Never crosses the HTTP boundary; [`Self::view`] is the only way out.
/// Rows are written only through [`Self::saved`], so a stored recipe has
/// passed `check_recipe` with this build's or an older build's rules.
///
/// No `Debug`: the endpoints carry secrets.
#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct StoredRecipe {
    pub(crate) size: RecipeSize,
    pub(crate) source: RecipeSource,
    pub(crate) claude: Option<StoredEndpoint>,
    pub(crate) codex: Option<StoredEndpoint>,
    pub(crate) tunnels: Vec<Tunnel>,
}

impl StoredRecipe {
    /// Validate `save` and resolve its tokens against `previous`, the stored
    /// definition it replaces (`None` for a new recipe).
    ///
    /// Run this inside the write's own transaction for an update: the token
    /// rule compares against what is stored at that moment, so a stale form
    /// cannot carry a token saved after it loaded to another origin. Packages
    /// are stored without duplicates, in their given order.
    fn saved(previous: Option<&StoredRecipe>, mut save: RecipeSave) -> Result<Self, String> {
        check_recipe(&save)?;
        if let RecipeSource::Packages { packages } = &mut save.source {
            let mut seen = std::collections::HashSet::new();
            packages.retain(|package| seen.insert(package.clone()));
        }
        let claude = resolve_endpoint(previous.and_then(|p| p.claude.as_ref()), save.claude)
            .map_err(|reason| format!("Claude Code: {reason}"))?;
        let codex = resolve_endpoint(previous.and_then(|p| p.codex.as_ref()), save.codex)
            .map_err(|reason| format!("Codex: {reason}"))?;
        Ok(Self {
            size: save.size,
            source: save.source,
            claude,
            codex,
            tunnels: save.tunnels,
        })
    }

    /// The secret-free read shape of this recipe.
    pub(crate) fn view(&self, name: &str) -> RecipeView {
        let endpoint = |endpoint: &Option<StoredEndpoint>| {
            endpoint.as_ref().map(|endpoint| EndpointView {
                base_url: endpoint.base_url.clone(),
                token_set: endpoint.token.is_some(),
            })
        };
        RecipeView {
            name: name.to_string(),
            size: self.size,
            source: self.source.clone(),
            claude: endpoint(&self.claude),
            codex: endpoint(&self.codex),
            tunnels: self.tunnels.clone(),
        }
    }
}

/// Apply one endpoint save to the stored endpoint it replaces, under the
/// ntfy token rule (`EndpointSave`'s docs): an omitted token survives only
/// when the base URL keeps its origin, an empty one clears, any other
/// replaces. An endpoint saved as `None` is removed, token and all.
///
/// The URL has already passed the shared structural check; parsing it here
/// with a real URL parser is a second gate and gives the origin (scheme,
/// host, effective port with defaults normalized) the comparison needs.
fn resolve_endpoint(
    previous: Option<&StoredEndpoint>,
    save: Option<EndpointSave>,
) -> Result<Option<StoredEndpoint>, &'static str> {
    let Some(save) = save else {
        return Ok(None);
    };
    let url = reqwest::Url::parse(&save.base_url)
        .map_err(|_| "use an http or https base URL without credentials, query or fragment")?;
    let same_origin = previous.is_some_and(|previous| {
        reqwest::Url::parse(&previous.base_url)
            .is_ok_and(|previous| previous.origin() == url.origin())
    });
    let token = match save.token {
        None if same_origin => previous.and_then(|previous| previous.token.clone()),
        None => None,
        Some(token) if token.is_empty() => None,
        Some(token) => Some(token),
    };
    Ok(Some(StoredEndpoint {
        base_url: save.base_url,
        token,
    }))
}

/// Why a recipe write did not happen. Each maps to one HTTP status and a
/// user-facing message in [`refusal_response`].
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// Create: a recipe of that name exists already.
    Exists,
    /// Update or delete: no recipe has that name.
    Missing,
    /// The name or definition fails validation; the message says why.
    Invalid(String),
    /// Delete: this many hosts use the recipe.
    InUse(u32),
}

/// How many hosts use the recipe named `name`, read inside the delete's own
/// transaction so a host cannot start using it between the check and the
/// delete.
///
/// Always zero for now: no host can use a recipe until Tensorlake hosts
/// exist. The hosts change wires this to its link from hosts to recipes (the
/// design suggests a nullable `hosts.tensorlake_recipe` column referencing
/// `tensorlake_recipes(name)`, which also makes the refusal a schema
/// invariant). Keeping the check and its refusal in place now means that
/// change only has to fill in this function.
fn hosts_using_recipe(_conn: &Connection, _name: &str) -> anyhow::Result<u32> {
    Ok(0)
}

/// Seconds since the epoch for `created_at`/`updated_at`. Informational only
/// (nothing orders by them); a clock before the epoch records 0 rather than
/// failing the write, as launch templates do.
fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

/// Decode a stored spec without letting serde's message through: on a
/// hand-edited row it can quote a value, and that value can be a token.
fn decode_spec(name: &str, json: &str) -> anyhow::Result<StoredRecipe> {
    serde_json::from_str(json).map_err(|_| {
        anyhow::anyhow!("Tensorlake recipe {name:?} has a definition this build cannot read")
    })
}

impl HelmStore {
    /// Every stored recipe, ordered by name.
    ///
    /// A row that does not decode is an error, not skipped: names are unique
    /// and immutable, so a recipe missing from the list would keep its name
    /// taken with no way to see why. Rows are written only through
    /// [`StoredRecipe::saved`], so this takes a hand edit or a downgrade.
    pub(crate) async fn tensorlake_recipes(&self) -> anyhow::Result<Vec<(String, StoredRecipe)>> {
        self.conn()
            .call("Tensorlake recipe read task panicked", |conn| {
                let mut stmt = conn
                    .prepare("SELECT name, spec_json FROM tensorlake_recipes ORDER BY name")
                    .context("preparing the Tensorlake recipe read")?;
                let rows = stmt
                    .query_map([], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    })
                    .context("reading Tensorlake recipes")?
                    .collect::<Result<Vec<_>, _>>()
                    .context("reading Tensorlake recipes")?;
                rows.into_iter()
                    .map(|(name, json)| {
                        let spec = decode_spec(&name, &json)?;
                        Ok((name, spec))
                    })
                    .collect()
            })
            .await
    }

    /// Store a new recipe named `name`, refusing a name that is invalid or
    /// already taken. Writes `spec_json` only; `state_json` takes its empty
    /// default.
    pub(crate) async fn create_tensorlake_recipe(
        &self,
        name: String,
        save: RecipeSave,
    ) -> anyhow::Result<Result<StoredRecipe, Refusal>> {
        if let Err(reason) = check_recipe_name(&name) {
            return Ok(Err(Refusal::Invalid(reason.to_string())));
        }
        let spec = match StoredRecipe::saved(None, save) {
            Ok(spec) => spec,
            Err(reason) => return Ok(Err(Refusal::Invalid(reason))),
        };
        let json = serde_json::to_string(&spec).context("encoding a Tensorlake recipe")?;
        let now = now_secs();
        self.conn()
            .call("Tensorlake recipe create task panicked", move |conn| {
                let inserted = conn
                    .execute(
                        "INSERT INTO tensorlake_recipes (name, spec_json, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?3)
                         ON CONFLICT (name) DO NOTHING",
                        rusqlite::params![name, json, now],
                    )
                    .context("storing a Tensorlake recipe")?;
                Ok(if inserted == 0 {
                    Err(Refusal::Exists)
                } else {
                    Ok(spec)
                })
            })
            .await
    }

    /// Replace the definition of the existing recipe `name` with `save`.
    ///
    /// The stored spec is read and the tokens resolved against it in the same
    /// immediate transaction as the write (see [`StoredRecipe::saved`]). Only
    /// `spec_json` and `updated_at` change; the build state in `state_json`
    /// is never part of this statement, so an edit cannot clobber it.
    pub(crate) async fn update_tensorlake_recipe(
        &self,
        name: String,
        save: RecipeSave,
    ) -> anyhow::Result<Result<StoredRecipe, Refusal>> {
        let now = now_secs();
        self.conn()
            .call("Tensorlake recipe update task panicked", move |conn| {
                let tx = conn
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                    .context("starting the Tensorlake recipe update")?;
                let stored: Option<String> = tx
                    .query_row(
                        "SELECT spec_json FROM tensorlake_recipes WHERE name = ?1",
                        [&name],
                        |row| row.get(0),
                    )
                    .optional()
                    .context("reading a Tensorlake recipe")?;
                let Some(stored) = stored else {
                    return Ok(Err(Refusal::Missing));
                };
                let previous = decode_spec(&name, &stored)?;
                let spec = match StoredRecipe::saved(Some(&previous), save) {
                    Ok(spec) => spec,
                    Err(reason) => return Ok(Err(Refusal::Invalid(reason))),
                };
                let json = serde_json::to_string(&spec).context("encoding a Tensorlake recipe")?;
                tx.execute(
                    "UPDATE tensorlake_recipes SET spec_json = ?2, updated_at = ?3 WHERE name = ?1",
                    rusqlite::params![name, json, now],
                )
                .context("storing a Tensorlake recipe")?;
                tx.commit()
                    .context("committing the Tensorlake recipe update")?;
                Ok(Ok(spec))
            })
            .await
    }

    /// Delete the recipe `name`, refused while a host uses it
    /// ([`hosts_using_recipe`]) and when it does not exist.
    pub(crate) async fn delete_tensorlake_recipe(
        &self,
        name: String,
    ) -> anyhow::Result<Result<(), Refusal>> {
        self.conn()
            .call("Tensorlake recipe delete task panicked", move |conn| {
                let tx = conn
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                    .context("starting the Tensorlake recipe delete")?;
                let users = hosts_using_recipe(&tx, &name)?;
                if users > 0 {
                    return Ok(Err(Refusal::InUse(users)));
                }
                let deleted = tx
                    .execute("DELETE FROM tensorlake_recipes WHERE name = ?1", [&name])
                    .context("deleting a Tensorlake recipe")?;
                if deleted == 0 {
                    return Ok(Err(Refusal::Missing));
                }
                tx.commit()
                    .context("committing the Tensorlake recipe delete")?;
                Ok(Ok(()))
            })
            .await
    }
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

/// The HTTP answer for a refused write. The messages are shown to the user
/// as they are, and none quotes a token.
fn refusal_response(name: &str, refusal: Refusal) -> Response {
    match refusal {
        Refusal::Exists => (
            StatusCode::CONFLICT,
            format!("a recipe named {name} already exists"),
        )
            .into_response(),
        Refusal::Missing => (StatusCode::NOT_FOUND, "no recipe has that name").into_response(),
        Refusal::Invalid(reason) => (StatusCode::BAD_REQUEST, reason).into_response(),
        Refusal::InUse(hosts) => (
            StatusCode::CONFLICT,
            if hosts == 1 {
                format!("a host uses recipe {name}; delete that host first")
            } else {
                format!("{hosts} hosts use recipe {name}; delete those hosts first")
            },
        )
            .into_response(),
    }
}

/// A storage failure, logged in full and answered with a fixed message. The
/// errors built in this module carry no secret, but the answer stays fixed
/// anyway: the user can act on "could not" and nothing more specific.
fn storage_failure(action: &'static str, error: anyhow::Error) -> Response {
    tracing::warn!("Tensorlake recipes: could not {action}: {error:#}");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("could not {action}"),
    )
        .into_response()
}

/// The fixed refusal for a body that does not decode. serde's own message can
/// quote submitted text, which may be a token.
fn malformed_body() -> Response {
    (StatusCode::BAD_REQUEST, "invalid recipe").into_response()
}

/// `GET /api/tensorlake/recipes`: every recipe, by name, without secrets.
/// Local only; never calls Tensorlake.
pub(crate) async fn list_recipes(State(state): State<Arc<AppState>>) -> Response {
    match state.store.tensorlake_recipes().await {
        Ok(rows) => Json(RecipesView {
            recipes: rows.iter().map(|(name, spec)| spec.view(name)).collect(),
        })
        .into_response(),
        Err(error) => storage_failure("read Tensorlake recipes", error),
    }
}

/// `POST /api/tensorlake/recipes`: create a recipe. 409 when the name is
/// taken, 400 when it or the definition is invalid. Answers with the stored
/// recipe's view and bumps the invalidation feed so other clients re-read.
pub(crate) async fn create_recipe(
    State(state): State<Arc<AppState>>,
    body: Result<Json<CreateRecipe>, JsonRejection>,
) -> Response {
    let Ok(Json(CreateRecipe { name, recipe })) = body else {
        return malformed_body();
    };
    match state
        .store
        .create_tensorlake_recipe(name.clone(), recipe)
        .await
    {
        Ok(Ok(spec)) => {
            state.manager.events().bump();
            Json(spec.view(&name)).into_response()
        }
        Ok(Err(refusal)) => refusal_response(&name, refusal),
        Err(error) => storage_failure("save the Tensorlake recipe", error),
    }
}

/// `PUT /api/tensorlake/recipes/{name}`: replace an existing recipe's
/// definition. 404 when no recipe has the name; a PUT never creates.
pub(crate) async fn update_recipe(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    body: Result<Json<RecipeSave>, JsonRejection>,
) -> Response {
    let Ok(Json(save)) = body else {
        return malformed_body();
    };
    match state
        .store
        .update_tensorlake_recipe(name.clone(), save)
        .await
    {
        Ok(Ok(spec)) => {
            state.manager.events().bump();
            Json(spec.view(&name)).into_response()
        }
        Ok(Err(refusal)) => refusal_response(&name, refusal),
        Err(error) => storage_failure("save the Tensorlake recipe", error),
    }
}

/// `DELETE /api/tensorlake/recipes/{name}`: remove a recipe. 404 when it does
/// not exist, 409 while a host uses it.
pub(crate) async fn delete_recipe(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Response {
    match state.store.delete_tensorlake_recipe(name.clone()).await {
        Ok(Ok(())) => {
            state.manager.events().bump();
            Json(serde_json::json!({})).into_response()
        }
        Ok(Err(refusal)) => refusal_response(&name, refusal),
        Err(error) => storage_failure("delete the Tensorlake recipe", error),
    }
}

/// Fixtures other modules' tests use to put a recipe in a store.
#[cfg(test)]
pub(crate) mod tests_support {
    use super::*;

    /// A small valid managed recipe, as a save.
    pub(crate) fn save() -> RecipeSave {
        RecipeSave {
            size: RecipeSize {
                cpus: 2,
                memory_gb: 4,
                disk_gb: 20,
            },
            source: RecipeSource::Packages {
                packages: vec!["git".into(), "ripgrep".into()],
            },
            claude: None,
            codex: None,
            tunnels: vec![],
        }
    }

    /// Create the recipe `name` from [`save`], failing the test otherwise.
    pub(crate) async fn create(store: &HelmStore, name: &str) {
        assert!(
            store
                .create_tensorlake_recipe(name.to_string(), save())
                .await
                .expect("store the recipe")
                .is_ok(),
            "creating {name} must succeed"
        );
    }
}

#[cfg(test)]
#[path = "recipes_tests.rs"]
mod tests;
