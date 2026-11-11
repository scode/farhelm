//! Shared recipe storage behind the compatibility `/api/tensorlake/recipes` routes.
//!
//! The shared definition lives in `environment::recipes::Recipe`, with wire
//! shapes and validation in `farhelm_proto::recipes`. Tensorlake callers keep
//! their existing imports through re-exports. The historical table and routes
//! retain their names so saved recipes and existing clients keep working.
//! This module stores recipes in the
//! `tensorlake_recipes` table and serves them:
//!
//! - `GET /api/tensorlake/recipes` lists every recipe, without secrets, with
//!   its built/prepared snapshots and their independent progress (`build.rs`).
//! - `POST /api/tensorlake/recipes` creates one; an existing name is a 409,
//!   never an overwrite, because "New recipe" in the editor must not replace a
//!   recipe someone else created meanwhile.
//! - `PUT /api/tensorlake/recipes/{name}` replaces an existing recipe's
//!   definition; a missing name is a 404 (a PUT never creates). The URL is the
//!   name's only authority, so names cannot change.
//! - `DELETE /api/tensorlake/recipes/{name}` removes one, refused while a host
//!   uses it, after deleting what Tensorlake bills for it.
//! - `POST /api/tensorlake/recipes/{name}/build` starts or joins a snapshot
//!   build (Build, Rebuild and Retry).
//!
//! This is deliberately not launch templates' last-write-wins "PUT creates or
//! replaces": names are immutable and create versus update are distinct
//! operations, and the row carries helm-owned build state beside the user's
//! spec (see `TENSORLAKE_RECIPES_SCHEMA` in `store.rs`). A save writes only
//! `spec_json`. A base or package edit keeps the built snapshot until the next
//! build replaces it; `state_json` remains the build engine's.
//!
//! Endpoints store credential names, never values. Because an outdated client
//! can still submit an embedded token, a body that does not
//! decode gets a fixed refusal instead of serde's diagnostic, which can quote
//! the submitted text. Only transient delivery inputs carry secret values.
//! Subscription names obey the same transactional reference protection as
//! endpoint tokens; each agent selects exactly one authentication mode.
//!
//! Saving never builds: a build bills, so only the explicit build route starts
//! one. Apart from an explicit build, the routes call Tensorlake only for a
//! delete's cleanup, through the build engine.

use std::sync::Arc;

use anyhow::Context as _;
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use farhelm_proto::tensorlake::recipes::{
    CreateRecipe, RecipeBase, RecipeSave, RecipeView, RecipesView, check_recipe_name,
};
use rusqlite::{Connection, OptionalExtension as _};

use super::build::{CleanupRefusal, RecipeState, StartRefusal, decode_state};
use crate::{AppState, store::HelmStore};

/// The request body cap for recipe writes. 500 typical package names, a few
/// tunnels and two endpoints fit with room to spare. This also bounds literal
/// container image references and login users, whose syntax belongs to Podman
/// and image inspection. Other field caps are checked in `check_recipe`.
pub(crate) const BODY_LIMIT: usize = 64 * 1024;

// ---------------------------------------------------------------------------
// Stored shape
// ---------------------------------------------------------------------------

// Existing delivery and Tensorlake callers use the shared catalog types.
pub(crate) use crate::environment::recipes::{
    Recipe as StoredRecipe, ResolvedEndpoint, StoredEndpoint,
};

/// Name/kind admission belongs to the same transaction that saves the recipe;
/// a separate catalog preflight would race deletion on another connection.
fn check_credentials(conn: &Connection, spec: &StoredRecipe) -> anyhow::Result<Option<Refusal>> {
    if let Some(name) = &spec.claude_login_credential
        && !crate::credentials::has_kind(
            conn,
            name,
            farhelm_proto::credentials::CredentialKind::ClaudeLogin,
        )?
    {
        return Ok(Some(Refusal::Invalid(
            "select an existing Claude login in Settings → Credentials".into(),
        )));
    }
    if let Some(name) = &spec.codex_login_credential
        && !crate::credentials::has_kind(
            conn,
            name,
            farhelm_proto::credentials::CredentialKind::CodexLogin,
        )?
    {
        return Ok(Some(Refusal::Invalid(
            "select an existing Codex login in Settings → Credentials".into(),
        )));
    }
    if let Some(name) = &spec.github_token_credential
        && !crate::credentials::has_kind(
            conn,
            name,
            farhelm_proto::credentials::CredentialKind::GithubToken,
        )?
    {
        return Ok(Some(Refusal::Invalid(
            "select an existing GitHub token in Settings → Credentials".into(),
        )));
    }
    for endpoint in [&spec.claude, &spec.codex].into_iter().flatten() {
        if let Some(name) = &endpoint.token_credential
            && !crate::credentials::has_kind(
                conn,
                name,
                farhelm_proto::credentials::CredentialKind::EndpointToken,
            )?
        {
            return Ok(Some(Refusal::Invalid(
                "select an existing API endpoint token in Settings → Credentials".into(),
            )));
        }
    }
    Ok(None)
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
    /// Delete: the recipe's build state still records a builder or snapshot
    /// that has not been deleted on Tensorlake.
    Billed,
}

/// How many hosts use the recipe named `name`, read inside the delete's own
/// transaction so a host cannot start using it between the check and the
/// delete.
///
/// A host uses a recipe when its `tensorlake_recipe` column names it (schema
/// 71). That column is a plain name with no foreign key, so this count is
/// the whole rule.
fn hosts_using_recipe(conn: &Connection, name: &str) -> anyhow::Result<u32> {
    conn.query_row(
        "SELECT count(*) FROM hosts WHERE tensorlake_recipe = ?1",
        [name],
        |row| row.get(0),
    )
    .context("counting the hosts that use a Tensorlake recipe")
}

/// Refuse a base that is a snapshot Farhelm built for a recipe
/// (current or retired), inside the caller's transaction.
///
/// Farhelm deletes those snapshots on its own: when their recipe is
/// rebuilt or deleted. A recipe built on one would lose its base when the
/// owning recipe moves on. Until snapshots can have several owners, using
/// one as a base is refused, including a recipe's own snapshot.
fn refuse_owned_snapshot(conn: &Connection, base: &RecipeBase) -> anyhow::Result<Option<Refusal>> {
    let RecipeBase::TensorlakeSnapshot { snapshot_id } = base else {
        return Ok(None);
    };
    let mut stmt = conn
        .prepare("SELECT name, state_json FROM tensorlake_recipes ORDER BY name")
        .context("preparing the Tensorlake build state read")?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .context("reading Tensorlake build state")?
        .collect::<Result<Vec<_>, _>>()
        .context("reading Tensorlake build state")?;
    for (owner, json) in rows {
        let state = decode_state(&owner, &json)?;
        let current = state
            .snapshot
            .as_ref()
            .is_some_and(|built| &built.id == snapshot_id);
        let prepared = state
            .prepared
            .as_ref()
            .is_some_and(|prepared| &prepared.id == snapshot_id);
        if current || prepared || state.retired_snapshots.contains(snapshot_id) {
            return Ok(Some(Refusal::Invalid(format!(
                "snapshot {snapshot_id} is the one Farhelm built for recipe {owner}, and Farhelm deletes it when that recipe is rebuilt or deleted; use recipe {owner} for these hosts, or a snapshot of your own"
            ))));
        }
    }
    Ok(None)
}

/// Seconds since the epoch for recipe timestamps and durable create settle
/// deadlines. A clock before the epoch records 0 rather than failing the
/// write, as launch templates do.
pub(crate) fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

/// Decode a stored spec without letting serde's message through: on a
/// hand-edited row it can quote a value, and that value can be a token.
pub(super) fn decode_spec(name: &str, json: &str) -> anyhow::Result<StoredRecipe> {
    serde_json::from_str(json).map_err(|_| {
        anyhow::anyhow!("Tensorlake recipe {name:?} has a definition this build cannot read")
    })
}

impl HelmStore {
    /// Every stored recipe's definition, ordered by name, for tests that
    /// check what a save stored; the routes read definitions and build state
    /// together through `tensorlake_recipe_rows`.
    ///
    /// A row that does not decode is an error, not skipped: names are unique
    /// and immutable, so a recipe missing from the list would keep its name
    /// taken with no way to see why. Rows are written only through
    /// [`StoredRecipe::saved`], so this takes a hand edit or a downgrade.
    #[cfg(test)]
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
        let spec = match StoredRecipe::saved(save) {
            Ok(spec) => spec,
            Err(reason) => return Ok(Err(Refusal::Invalid(reason))),
        };
        let json = serde_json::to_string(&spec).context("encoding a Tensorlake recipe")?;
        let now = now_secs();
        self.conn()
            .call("Tensorlake recipe create task panicked", move |conn| {
                let tx =
                    conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                if let Some(refusal) = check_credentials(&tx, &spec)? {
                    return Ok(Err(refusal));
                }
                if let Some(refusal) = refuse_owned_snapshot(&tx, &spec.base)? {
                    return Ok(Err(refusal));
                }
                let inserted = tx
                    .execute(
                        "INSERT INTO tensorlake_recipes (name, spec_json, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?3)
                         ON CONFLICT (name) DO NOTHING",
                        rusqlite::params![name, json, now],
                    )
                    .context("storing a Tensorlake recipe")?;
                tx.commit()
                    .context("committing the Tensorlake recipe create")?;
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
    /// Credential references are admitted in the same immediate transaction as
    /// the write. Only `spec_json` and `updated_at` change; the build state in `state_json`
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
                decode_spec(&name, &stored)?;
                let spec = match StoredRecipe::saved(save) {
                    Ok(spec) => spec,
                    Err(reason) => return Ok(Err(Refusal::Invalid(reason))),
                };
                if let Some(refusal) = check_credentials(&tx, &spec)? {
                    return Ok(Err(refusal));
                }
                if let Some(refusal) = refuse_owned_snapshot(&tx, &spec.base)? {
                    return Ok(Err(refusal));
                }
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

    /// Resolve the current value for setup without ever serializing it into a
    /// recipe. A deleted or wrong-kind identity refuses setup with fixed text.
    pub(crate) async fn resolve_endpoint(
        &self,
        endpoint: StoredEndpoint,
    ) -> anyhow::Result<ResolvedEndpoint> {
        self.conn()
            .call("resolve endpoint credential", move |conn| {
                let value = match endpoint.token_credential {
                    Some(name) => Some(
                        crate::credentials::secret(
                            conn,
                            &name,
                            farhelm_proto::credentials::CredentialKind::EndpointToken,
                        )?
                        .ok_or_else(|| {
                            anyhow::anyhow!("the recipe's API endpoint credential no longer exists")
                        })?,
                    ),
                    None => None,
                };
                Ok(ResolvedEndpoint {
                    base_url: endpoint.base_url,
                    token: value.as_ref().map(|(secret, _, _)| secret.clone()),
                    revision: value.as_ref().map(|(_, revision, _)| *revision),
                    incarnation: value.map(|(_, _, incarnation)| incarnation),
                })
            })
            .await
    }

    /// How many hosts use the recipe `name`; see [`hosts_using_recipe`].
    pub(crate) async fn tensorlake_recipe_users(&self, name: String) -> anyhow::Result<u32> {
        self.conn()
            .call("Tensorlake recipe read task panicked", move |conn| {
                hosts_using_recipe(conn, &name)
            })
            .await
    }

    /// Delete the recipe `name`, refused while a host uses it
    /// ([`hosts_using_recipe`]), while its build state still records
    /// something Tensorlake may bill for, and when it does not exist.
    ///
    /// The billing refusal makes "no path forgets a billed resource" a
    /// property of the store, not of the route: the row is the only record
    /// of a builder or snapshot ID, so the route must have deleted them (and
    /// cleared their records) before this can succeed.
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
                let state: Option<String> = tx
                    .query_row(
                        "SELECT state_json FROM tensorlake_recipes WHERE name = ?1",
                        [&name],
                        |row| row.get(0),
                    )
                    .optional()
                    .context("reading Tensorlake build state")?;
                if let Some(state) = state
                    && decode_state(&name, &state)?.needs_cleanup()
                {
                    return Ok(Err(Refusal::Billed));
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
        Refusal::Billed => (
            StatusCode::CONFLICT,
            format!(
                "recipe {name} still has a snapshot or build sandbox on Tensorlake; try deleting it again"
            ),
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

/// Project both snapshot layers without changing the user's recipe definition.
fn full_view(state: &AppState, name: &str, spec: &StoredRecipe, build: &RecipeState) -> RecipeView {
    let (prepared, preparation) = state.tensorlake.builds().preparation_view(name, build);
    let (snapshot, build, cleanup_pending) = state.tensorlake.builds().view(name, build);
    RecipeView {
        snapshot,
        build,
        cleanup_pending,
        prepared,
        preparation,
        ..spec.view(name)
    }
}

/// The current full view of recipe `name`, answered as JSON, or the refusal
/// for a recipe that is gone.
async fn view_response(state: &AppState, name: &str, status: StatusCode) -> Response {
    match state.store.tensorlake_recipe_row(name).await {
        Ok(Some((spec, build))) => {
            (status, Json(full_view(state, name, &spec, &build))).into_response()
        }
        Ok(None) => refusal_response(name, Refusal::Missing),
        Err(error) => storage_failure("read the Tensorlake recipe", error),
    }
}

/// `GET /api/tensorlake/recipes`: every recipe, by name, without secrets,
/// with its snapshot and build status. Local only; never calls Tensorlake.
pub(crate) async fn list_recipes(State(state): State<Arc<AppState>>) -> Response {
    match state.store.tensorlake_recipe_rows().await {
        Ok(rows) => Json(RecipesView {
            recipes: rows
                .iter()
                .map(|(name, spec, build)| full_view(&state, name, spec, build))
                .collect(),
        })
        .into_response(),
        Err(error) => storage_failure("read Tensorlake recipes", error),
    }
}

/// `POST /api/tensorlake/recipes`: create a recipe. 409 when the name is
/// taken, 400 when it or the definition is invalid. Answers with the stored
/// recipe's view and bumps the invalidation feed so other clients re-read.
/// Saving never starts a snapshot build: a build bills, and only the
/// explicit Build action may start one.
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
///
/// Saves only change the definition. Current snapshots remain usable until
/// their replacement build succeeds, and already-retired snapshots wait for
/// the next build or recipe deletion to retry cleanup.
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
        Ok(Ok(_)) => {
            state.manager.events().bump();
            view_response(&state, &name, StatusCode::OK).await
        }
        Ok(Err(refusal)) => refusal_response(&name, refusal),
        Err(error) => storage_failure("save the Tensorlake recipe", error),
    }
}

/// The compatibility route deletes the one shared recipe across all places.
/// Admission excludes new builds and source users before either provider
/// cleans resources. Partial failure retains the row and remaining ownership;
/// an empty Tensorlake side needs no cloud credentials for Podman cleanup.
pub(crate) async fn delete_recipe(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Response {
    crate::run_owned(async move { delete_recipe_owned(state, name).await }).await
}

/// Keep deletion admission through job cancellation and every confirmed
/// cleanup even if the HTTP caller disappears during a remote command.
async fn delete_recipe_owned(state: Arc<AppState>, name: String) -> Response {
    // Refuse a recipe in use before deleting anything at any place: hosts
    // started from its snapshot keep working without it, but the user asked
    // for nothing to change while the refusal stands.
    match state.store.tensorlake_recipe_users(name.clone()).await {
        Ok(0) => {}
        Ok(users) => return refusal_response(&name, Refusal::InUse(users)),
        Err(error) => return storage_failure("delete the Tensorlake recipe", error),
    }
    let builds = Arc::clone(state.tensorlake.builds());
    let guard = match builds.begin_delete(&name) {
        Ok(guard) => guard,
        Err(refusal) => {
            state.manager.events().bump();
            return cleanup_refusal_response(&name, refusal);
        }
    };
    if let Err(error) = state.manager.providers().clean_recipe(&name, &guard).await {
        state.manager.events().bump();
        return crate::http_error(error);
    }
    let deleted = state.store.delete_tensorlake_recipe(name.clone()).await;
    drop(guard);
    state.manager.events().bump();
    match deleted {
        Ok(Ok(())) => Json(serde_json::json!({})).into_response(),
        Ok(Err(refusal)) => refusal_response(&name, refusal),
        Err(error) => storage_failure("delete the Tensorlake recipe", error),
    }
}

/// The HTTP answer for a delete whose Tensorlake cleanup did not finish.
fn cleanup_refusal_response(name: &str, refusal: CleanupRefusal) -> Response {
    match refusal {
        CleanupRefusal::Missing => refusal_response(name, Refusal::Missing),
        CleanupRefusal::Deleting => (
            StatusCode::CONFLICT,
            format!("recipe {name} is already being deleted"),
        )
            .into_response(),
        CleanupRefusal::NoApiKey => (
            StatusCode::BAD_REQUEST,
            format!(
                "recipe {name} has a snapshot on Tensorlake; save a Tensorlake API key in Settings so Farhelm can delete it"
            ),
        )
            .into_response(),
        CleanupRefusal::Tensorlake(reason) => (
            StatusCode::BAD_GATEWAY,
            format!("Farhelm could not delete recipe {name}'s snapshot or build sandbox: {reason}"),
        )
            .into_response(),
        CleanupRefusal::Storage(error) => storage_failure("delete the Tensorlake recipe", error),
    }
}

/// `POST /api/tensorlake/recipes/{name}/build`: start building the recipe's
/// snapshot, or join the build already running. Build, Rebuild and Retry
/// all land here. Answers 202 with the recipe's view, which shows the build
/// running; progress then arrives through the feed and this list's GET.
pub(crate) async fn build_recipe(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Response {
    let builds = Arc::clone(state.tensorlake.builds());
    match builds
        .start_or_join(&name, Arc::clone(state.manager.events()))
        .await
    {
        Ok(()) => {
            state.manager.events().bump();
            view_response(&state, &name, StatusCode::ACCEPTED).await
        }
        Err(StartRefusal::Missing) => refusal_response(&name, Refusal::Missing),
        Err(StartRefusal::UnsupportedBase) => (
            StatusCode::BAD_REQUEST,
            "this recipe's base cannot run on Tensorlake",
        )
            .into_response(),
        Err(StartRefusal::NoApiKey) => (
            StatusCode::BAD_REQUEST,
            "save a Tensorlake API key in Settings first",
        )
            .into_response(),
        Err(StartRefusal::NoSshKey(reason)) => (StatusCode::BAD_REQUEST, reason).into_response(),
        Err(StartRefusal::Deleting) => (
            StatusCode::CONFLICT,
            format!("recipe {name} is being deleted"),
        )
            .into_response(),
        Err(StartRefusal::Storage(error)) => storage_failure("start the snapshot build", error),
    }
}

/// Fixtures other modules' tests use to put a recipe in a store.
#[cfg(test)]
pub(crate) mod tests_support {
    use super::*;
    use farhelm_proto::recipes::RecipeSize;

    /// A small valid Ubuntu recipe, with packages to witness layer preservation.
    pub(crate) fn save() -> RecipeSave {
        RecipeSave {
            size: RecipeSize {
                cpus: 2,
                memory_gb: 4,
                disk_gb: 20,
            },
            base: RecipeBase::ManagedUbuntu,
            packages: vec!["git".into(), "ripgrep".into()],
            claude: None,
            codex: None,
            github_token_credential: None,
            claude_login_credential: None,
            codex_login_credential: None,
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
