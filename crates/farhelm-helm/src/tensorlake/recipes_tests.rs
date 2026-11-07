//! Storage and route proofs for Tensorlake recipes.
//!
//! Storage tests use a fresh `HelmStore` in a temporary directory and reach
//! the table directly only to plant or read `state_json`, the build engine's
//! column (its own proofs are in `build_tests.rs`). Route tests drive the real protected router through
//! the REST harness (offline host actors, no network), with secret-looking
//! marker strings so a leak anywhere in a reply is a plain substring match.

use super::tests_support::save;
use super::*;
use crate::rest_harness::{FleetBuilder, Harness};
use axum::body::Body;
use axum::http::Request;
use farhelm_proto::tensorlake::recipes::EndpointSave;
use tower::ServiceExt as _;

const TOKEN: &str = "private-endpoint-token-marker";
const OTHER_TOKEN: &str = "private-replacement-token-marker";
const ENDPOINT: &str = "work-endpoint";
const OTHER_ENDPOINT: &str = "other-endpoint";

/// Saved values live outside recipes. Distinct names and marker values let
/// route assertions distinguish a valid reference from accidental disclosure.
async fn seed_credentials(store: &HelmStore) {
    for (name, secret) in [(ENDPOINT, TOKEN), (OTHER_ENDPOINT, OTHER_TOKEN)] {
        store
            .create_credential(farhelm_proto::credentials::CreateCredential {
                name: name.into(),
                kind: farhelm_proto::credentials::CredentialKind::EndpointToken,
                secret: secret.into(),
            })
            .await
            .unwrap();
    }
}

/// A fresh store in its own temporary directory. The directory is returned so
/// it outlives the store.
async fn store() -> (tempfile::TempDir, HelmStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = HelmStore::open(&dir.path().join("helm.db"))
        .await
        .expect("open a fresh store");
    seed_credentials(&store).await;
    (dir, store)
}

/// A Claude endpoint selecting a credential name, with no secret in its request.
fn with_claude(base_url: &str, credential: Option<&str>) -> RecipeSave {
    let mut recipe = save();
    recipe.claude = Some(EndpointSave {
        base_url: base_url.into(),
        token_credential: credential.map(str::to_string),
    });
    recipe
}

/// Observe setup's resolved value privately; recipe storage contains only names.
async fn claude_token(store: &HelmStore, name: &str) -> Option<String> {
    let endpoint = store
        .tensorlake_recipes()
        .await
        .unwrap()
        .into_iter()
        .find(|(stored, _)| stored == name)
        .expect("the recipe exists")
        .1
        .claude?;
    store.resolve_endpoint(endpoint).await.unwrap().token
}

/// Read `(spec_json, state_json, created_at)` of a row straight from the table.
async fn raw_row(store: &HelmStore, name: &str) -> (String, String, i64) {
    let name = name.to_string();
    store
        .conn()
        .call("test read", move |conn| {
            Ok(conn.query_row(
                "SELECT spec_json, state_json, created_at FROM tensorlake_recipes WHERE name = ?1",
                [&name],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?)
        })
        .await
        .unwrap()
}

/// Spec: creating a recipe whose name exists is refused and leaves the stored
/// recipe exactly as it was; an invalid name or definition is refused before
/// anything is stored.
///
/// Why: "New recipe" must never overwrite a recipe (another client may have
/// created the same name meanwhile), and an overwrite would also silently
/// replace the other recipe's endpoint choices.
#[farhelm_testtrace::test]
async fn create_refuses_an_existing_name_and_invalid_input() {
    let (_dir, store) = store().await;
    store
        .create_tensorlake_recipe(
            "rust-dev".into(),
            with_claude("http://127.0.0.1:8080", Some(ENDPOINT)),
        )
        .await
        .unwrap()
        .unwrap_or_else(|_| panic!("first create succeeds"));
    let before = raw_row(&store, "rust-dev").await;
    let mut other = save();
    other.size.cpus = 4;
    assert_eq!(
        store
            .create_tensorlake_recipe("rust-dev".into(), other)
            .await
            .unwrap()
            .err(),
        Some(Refusal::Exists)
    );
    assert_eq!(raw_row(&store, "rust-dev").await, before);
    assert!(matches!(
        store
            .create_tensorlake_recipe("Rust Dev".into(), save())
            .await
            .unwrap(),
        Err(Refusal::Invalid(_))
    ));
    let mut bad = save();
    bad.size.cpus = 0;
    assert!(matches!(
        store
            .create_tensorlake_recipe("py-data".into(), bad)
            .await
            .unwrap(),
        Err(Refusal::Invalid(_))
    ));
    assert_eq!(store.tensorlake_recipes().await.unwrap().len(), 1);
}

/// Spec: an update that keeps the recipe a package recipe writes only
/// `spec_json` (and `updated_at`); the row's `state_json` and `created_at`
/// survive byte for byte. An update or delete
/// of a missing name is refused as missing and creates nothing.
///
/// Why: `state_json` holds the only record of billed Tensorlake
/// resources (a snapshot ID, a builder ID). A user's save racing a build must
/// never drop it, which is why the two live in separate columns with separate
/// writers; this pins the user-save side of that split.
#[farhelm_testtrace::test]
async fn updates_write_the_spec_and_never_the_build_state() {
    let (_dir, store) = store().await;
    super::tests_support::create(&store, "rust-dev").await;
    // What a build records, planted directly.
    let state = r#"{"snapshot":{"id":"snap123"},"builder_sandbox_id":"sbx456"}"#;
    {
        let state = state.to_string();
        store
            .conn()
            .call("plant state", move |conn| {
                conn.execute(
                    "UPDATE tensorlake_recipes SET state_json = ?1 WHERE name = 'rust-dev'",
                    [state],
                )?;
                Ok(())
            })
            .await
            .unwrap();
    }
    let (spec_before, state_before, created_before) = raw_row(&store, "rust-dev").await;
    assert_eq!(state_before, state, "premise: the build state is planted");
    // Still a package recipe: a switch to an image is the one save that
    // retires the snapshot (pinned in `build_tests.rs`).
    let mut edited = save();
    edited.size.disk_gb = 77;
    store
        .update_tensorlake_recipe("rust-dev".into(), edited)
        .await
        .unwrap()
        .unwrap_or_else(|_| panic!("update succeeds"));
    let (spec_after, state_after, created_after) = raw_row(&store, "rust-dev").await;
    assert_ne!(spec_after, spec_before, "the spec changed");
    assert!(spec_after.contains("77"));
    assert_eq!(state_after, state, "the build state is untouched");
    assert_eq!(created_after, created_before);

    assert_eq!(
        store
            .update_tensorlake_recipe("missing".into(), save())
            .await
            .unwrap()
            .err(),
        Some(Refusal::Missing)
    );
    assert_eq!(
        store
            .delete_tensorlake_recipe("missing".into())
            .await
            .unwrap(),
        Err(Refusal::Missing)
    );
    assert_eq!(store.tensorlake_recipes().await.unwrap().len(), 1);
    // The planted state records billed resources, so the store keeps the row
    // until they are gone (pinned in `build_tests.rs`).
    assert_eq!(
        store
            .delete_tensorlake_recipe("rust-dev".into())
            .await
            .unwrap(),
        Err(Refusal::Billed)
    );
    store
        .conn()
        .call("clear state", |conn| {
            conn.execute(
                "UPDATE tensorlake_recipes SET state_json = '{}' WHERE name = 'rust-dev'",
                [],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(
        store
            .delete_tensorlake_recipe("rust-dev".into())
            .await
            .unwrap(),
        Ok(())
    );
    assert!(store.tensorlake_recipes().await.unwrap().is_empty());
}

/// A named choice survives a server change, and replacing that catalog entry
/// changes setup's resolved value without editing any recipe. A used entry is
/// undeletable until the recipe explicitly stops referring to it.
#[farhelm_testtrace::test]
async fn endpoint_references_survive_url_changes_and_track_replacements() {
    let (_dir, store) = store().await;
    let name = "rust-dev";
    let update = |recipe: RecipeSave| {
        let store = store.clone();
        async move {
            store
                .update_tensorlake_recipe(name.into(), recipe)
                .await
                .unwrap()
                .unwrap_or_else(|_| panic!("update succeeds"));
        }
    };
    store
        .create_tensorlake_recipe(
            name.into(),
            with_claude("http://127.0.0.1:8080", Some(ENDPOINT)),
        )
        .await
        .unwrap()
        .unwrap_or_else(|_| panic!("create succeeds"));
    assert_eq!(claude_token(&store, name).await.as_deref(), Some(TOKEN));

    update(with_claude("https://another.example/v1", Some(ENDPOINT))).await;
    let before = raw_row(&store, name).await;
    assert!(!before.0.contains(TOKEN));
    assert!(before.0.contains(ENDPOINT));
    let metadata = store
        .credentials()
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.name == ENDPOINT)
        .unwrap();
    assert_eq!(metadata.used_by, vec![name]);
    assert!(matches!(
        store.delete_credential(ENDPOINT.into()).await,
        Err(crate::credentials::Refusal::InUse)
    ));
    store
        .save_credential(
            ENDPOINT.into(),
            farhelm_proto::credentials::SaveCredential {
                secret: Some(OTHER_TOKEN.into()),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        raw_row(&store, name).await,
        before,
        "replacement does not edit the recipe"
    );
    assert_eq!(
        claude_token(&store, name).await.as_deref(),
        Some(OTHER_TOKEN)
    );
    update(with_claude("http://127.0.0.1:8080", None)).await;
    assert_eq!(claude_token(&store, name).await, None);
    store.delete_credential(ENDPOINT.into()).await.unwrap();
}

/// Missing and wrong-kind names cannot enter either a new or an existing
/// recipe. Checking only the picker would let stale or external clients save
/// an unusable definition, while resolving secret bytes is unnecessary here.
#[farhelm_testtrace::test]
async fn missing_and_wrong_kind_references_are_refused_atomically() {
    let (_dir, store) = store().await;
    store
        .create_credential(farhelm_proto::credentials::CreateCredential {
            name: "github-only".into(),
            kind: farhelm_proto::credentials::CredentialKind::GithubToken,
            secret: TOKEN.into(),
        })
        .await
        .unwrap();
    super::tests_support::create(&store, "existing").await;
    let before = raw_row(&store, "existing").await;
    for name in ["missing", "github-only"] {
        let recipe = with_claude("https://proxy.example/v1", Some(name));
        assert!(matches!(
            store
                .create_tensorlake_recipe("new".into(), recipe.clone())
                .await
                .unwrap(),
            Err(Refusal::Invalid(_))
        ));
        assert!(matches!(
            store
                .update_tensorlake_recipe("existing".into(), recipe)
                .await
                .unwrap(),
            Err(Refusal::Invalid(_))
        ));
        assert_eq!(raw_row(&store, "existing").await, before);
    }
    assert_eq!(store.tensorlake_recipes().await.unwrap().len(), 1);
}

/// A GitHub selection is a catalog identity, never a token copy. Admission and
/// deletion use the same transaction as their writes, so stale pickers cannot
/// create dangling references or delete a credential another recipe just used.
#[farhelm_testtrace::test]
async fn github_references_require_the_right_kind_and_guard_deletion() {
    use crate::credentials::Refusal as CredentialRefusal;
    use farhelm_proto::credentials::{CreateCredential, CredentialKind, SaveCredential};
    let (_dir, store) = store().await;
    store
        .create_credential(CreateCredential {
            name: "git-work".into(),
            kind: CredentialKind::GithubToken,
            secret: TOKEN.into(),
        })
        .await
        .unwrap();
    super::tests_support::create(&store, "existing").await;
    let before = raw_row(&store, "existing").await;
    for bad in ["missing", ENDPOINT] {
        let mut recipe = save();
        recipe.github_token_credential = Some(bad.into());
        assert!(matches!(
            store
                .create_tensorlake_recipe("new".into(), recipe.clone())
                .await
                .unwrap(),
            Err(Refusal::Invalid(_))
        ));
        assert!(matches!(
            store
                .update_tensorlake_recipe("existing".into(), recipe)
                .await
                .unwrap(),
            Err(Refusal::Invalid(_))
        ));
        assert_eq!(raw_row(&store, "existing").await, before);
    }
    let mut recipe = save();
    recipe.github_token_credential = Some("git-work".into());
    store
        .update_tensorlake_recipe("existing".into(), recipe)
        .await
        .unwrap()
        .unwrap();
    let selected = raw_row(&store, "existing").await;
    assert!(selected.0.contains("git-work") && !selected.0.contains(TOKEN));
    assert_eq!(
        store
            .credentials()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.name == "git-work")
            .unwrap()
            .used_by,
        vec!["existing"]
    );
    assert!(matches!(
        store.delete_credential("git-work".into()).await,
        Err(CredentialRefusal::InUse)
    ));
    store
        .save_credential(
            "git-work".into(),
            SaveCredential {
                secret: Some(OTHER_TOKEN.into()),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        raw_row(&store, "existing").await,
        selected,
        "value replacement leaves the reference unchanged"
    );
    store
        .update_tensorlake_recipe("existing".into(), save())
        .await
        .unwrap()
        .unwrap();
    store.delete_credential("git-work".into()).await.unwrap();
}

/// Independent SQLite connections race admission against deletion. Exactly
/// one operation can succeed: no committed recipe may reference the deleted
/// identity. Exercise create and update, which historically had different
/// transaction boundaries, without timing sleeps or process-global state.
#[farhelm_testtrace::test]
async fn recipe_reference_admission_and_credential_delete_have_one_winner() {
    for creating in [true, false] {
        let (dir, writer) = store().await;
        if !creating {
            super::tests_support::create(&writer, "racing").await;
        }
        let deleter = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
        assert!(
            writer
                .credentials()
                .await
                .unwrap()
                .iter()
                .any(|row| row.name == ENDPOINT)
        );
        let write = async {
            let recipe = with_claude("https://proxy.example/v1", Some(ENDPOINT));
            if creating {
                writer
                    .create_tensorlake_recipe("racing".into(), recipe)
                    .await
            } else {
                writer
                    .update_tensorlake_recipe("racing".into(), recipe)
                    .await
            }
        };
        let (written, deleted) = tokio::join!(write, deleter.delete_credential(ENDPOINT.into()));
        let written = written.unwrap();
        assert_ne!(
            written.is_ok(),
            deleted.is_ok(),
            "exactly one competing write succeeds"
        );
        let catalog = writer.credentials().await.unwrap();
        if written.is_ok() {
            assert!(matches!(deleted, Err(crate::credentials::Refusal::InUse)));
            assert_eq!(
                catalog
                    .iter()
                    .find(|row| row.name == ENDPOINT)
                    .unwrap()
                    .used_by,
                vec!["racing"]
            );
        } else {
            assert!(matches!(written, Err(Refusal::Invalid(_))));
            assert!(!catalog.iter().any(|row| row.name == ENDPOINT));
            for (_, recipe) in writer.tensorlake_recipes().await.unwrap() {
                assert!(
                    recipe
                        .claude
                        .as_ref()
                        .and_then(|endpoint| endpoint.token_credential.as_deref())
                        != Some(ENDPOINT)
                );
            }
        }
    }
}

/// Upgrade drops embedded tokens instead of inventing catalog identities. All
/// other definition fields and the build's billed-resource record survive;
/// a database version change must not silently lose ownership of a sandbox.
#[farhelm_testtrace::test]
async fn schema_75_drops_embedded_tokens_and_preserves_recipe_and_build_state() {
    let (dir, store) = store().await;
    let catalog_before = store.credentials().await.unwrap();
    let mut spec = serde_json::to_value(save()).unwrap();
    spec["claude"] = serde_json::json!({"base_url":"https://claude.example/v1", "token":"legacy-claude-private-value"});
    spec["codex"] = serde_json::json!({"base_url":"https://codex.example/v1", "token":"legacy-codex-private-value"});
    spec["future_setting"] = serde_json::json!({"keep":true});
    let json = serde_json::to_string(&spec).unwrap();
    let state = "{ \"builder_sandbox_id\": \"sbx456\" }";
    store.conn().call("plant schema75 recipe", move |conn| {
        conn.execute("INSERT INTO tensorlake_recipes (name, spec_json, state_json, created_at, updated_at) VALUES ('legacy', ?1, ?2, 7, 8)", rusqlite::params![json, state])?;
        conn.execute_batch("PRAGMA user_version = 75;")?;
        Ok(())
    }).await.unwrap();
    drop(store);
    let store = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
    let (migrated, build_state, created) = raw_row(&store, "legacy").await;
    assert!(!migrated.contains("legacy-claude-private-value"));
    assert!(!migrated.contains("legacy-codex-private-value"));
    spec["claude"].as_object_mut().unwrap().remove("token");
    spec["codex"].as_object_mut().unwrap().remove("token");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&migrated).unwrap(),
        spec
    );
    assert_eq!(build_state, state);
    assert_eq!(created, 7);
    assert_eq!(store.credentials().await.unwrap(), catalog_before);
    let recipes = store.tensorlake_recipes().await.unwrap();
    assert!(
        recipes[0]
            .1
            .claude
            .as_ref()
            .unwrap()
            .token_credential
            .is_none()
    );
}

/// Spec: a stored row that does not decode makes the listing an error naming
/// the recipe, and the error does not quote the stored text.
///
/// Why: skipping the row would leave its name taken and invisible; quoting
/// serde's message could print a token from a hand-edited row into a log or
/// an HTTP reply.
#[farhelm_testtrace::test]
async fn an_unreadable_row_fails_the_listing_without_quoting_it() {
    let (_dir, store) = store().await;
    let planted = format!(r#"{{"size":"{TOKEN}"}}"#);
    store
        .conn()
        .call("plant row", move |conn| {
            conn.execute(
                "INSERT INTO tensorlake_recipes (name, spec_json, created_at, updated_at)
                 VALUES ('broken', ?1, 0, 0)",
                [planted],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    let error = format!(
        "{:#}",
        store.tensorlake_recipes().await.err().expect("refused")
    );
    assert!(error.contains("broken"), "{error}");
    assert!(!error.contains(TOKEN), "{error}");
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

/// A helm with offline host actors; recipes need nothing else.
async fn helm() -> Harness {
    let harness = FleetBuilder::new().await.start().await;
    seed_credentials(&harness.store).await;
    harness
}

/// One request through the real protected router.
async fn call(
    harness: &Harness,
    method: &str,
    path: &str,
    body: Option<String>,
) -> (StatusCode, String) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(body.map_or_else(Body::empty, Body::from))
        .unwrap();
    let response = harness.router().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

/// The JSON body of a create.
fn create_body(name: &str, recipe: RecipeSave) -> String {
    serde_json::to_string(&CreateRecipe {
        name: name.into(),
        recipe,
    })
    .unwrap()
}

/// Spec: POST creates (200 with the view) and refuses an existing name with
/// 409; PUT updates (200) and answers 404 for a missing name without creating
/// it; DELETE answers `{}` and then 404; GET lists by name.
///
/// Why: these are the editor's contract. "New recipe" relies on the 409 to
/// never overwrite, and Save on an existing recipe relies on the 404 to never
/// resurrect one another client deleted.
#[farhelm_testtrace::test]
async fn routes_create_update_and_delete_with_distinct_refusals() {
    let harness = helm().await;
    let (status, body) = call(
        &harness,
        "POST",
        "/api/tensorlake/recipes",
        Some(create_body("rust-dev", save())),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let view: RecipeView = serde_json::from_str(&body).unwrap();
    assert_eq!(view.name, "rust-dev");
    assert_eq!(
        view.source,
        RecipeSource::Packages {
            packages: vec!["git".into(), "ripgrep".into()]
        }
    );

    let (status, body) = call(
        &harness,
        "POST",
        "/api/tensorlake/recipes",
        Some(create_body("rust-dev", save())),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body, "a recipe named rust-dev already exists");

    let mut edited = save();
    edited.size.disk_gb = 50;
    let edited = serde_json::to_string(&edited).unwrap();
    let (status, body) = call(
        &harness,
        "PUT",
        "/api/tensorlake/recipes/rust-dev",
        Some(edited.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        serde_json::from_str::<RecipeView>(&body)
            .unwrap()
            .size
            .disk_gb,
        50
    );

    let (status, body) = call(
        &harness,
        "PUT",
        "/api/tensorlake/recipes/py-data",
        Some(edited),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body, "no recipe has that name");

    let (status, body) = call(&harness, "GET", "/api/tensorlake/recipes", None).await;
    assert_eq!(status, StatusCode::OK);
    let listed: RecipesView = serde_json::from_str(&body).unwrap();
    assert_eq!(
        listed
            .recipes
            .iter()
            .map(|r| r.name.as_str())
            .collect::<Vec<_>>(),
        ["rust-dev"],
        "the refused PUT created nothing"
    );

    let (status, body) = call(&harness, "DELETE", "/api/tensorlake/recipes/rust-dev", None).await;
    assert_eq!((status, body.as_str()), (StatusCode::OK, "{}"));
    let (status, _) = call(&harness, "DELETE", "/api/tensorlake/recipes/rust-dev", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// Spec: no reply (create, update, list, a validation refusal, a malformed
/// body) contains a saved or submitted token; reads report only its name.
///
/// Why: tokens are write-only, like the ntfy token and the Tensorlake API
/// key. Every client of the helm can read these routes, and nothing needs a
/// token back. A malformed body is the subtle case: serde's own diagnostic
/// quotes the offending value.
#[farhelm_testtrace::test]
async fn tokens_never_appear_in_replies() {
    let harness = helm().await;
    let recipe = with_claude("http://127.0.0.1:8080", Some(ENDPOINT));
    let (status, body) = call(
        &harness,
        "POST",
        "/api/tensorlake/recipes",
        Some(create_body("rust-dev", recipe)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.contains(TOKEN), "{body}");
    let view: RecipeView = serde_json::from_str(&body).unwrap();
    assert_eq!(
        view.claude,
        Some(EndpointView {
            base_url: "http://127.0.0.1:8080".into(),
            token_credential: Some(ENDPOINT.into())
        })
    );
    assert_eq!(view.codex, None);

    let (_, body) = call(&harness, "GET", "/api/tensorlake/recipes", None).await;
    assert!(!body.contains(TOKEN), "{body}");
    assert!(body.contains(ENDPOINT), "{body}");

    // An unrelated edit carries the same name and never a secret value.
    let mut edited = with_claude("http://127.0.0.1:8080", Some(ENDPOINT));
    edited.size.cpus = 4;
    let (status, body) = call(
        &harness,
        "PUT",
        "/api/tensorlake/recipes/rust-dev",
        Some(serde_json::to_string(&edited).unwrap()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.contains(TOKEN));
    assert!(body.contains(ENDPOINT), "{body}");
    assert_eq!(
        claude_token(&harness.store, "rust-dev").await.as_deref(),
        Some(TOKEN),
        "premise: setup can resolve the saved value, while API replies cannot"
    );

    // A token that fails validation, and bodies that do not decode.
    let bad = with_claude(
        "http://127.0.0.1:8080",
        Some(&format!("{OTHER_TOKEN} with space")),
    );
    let (status, body) = call(
        &harness,
        "PUT",
        "/api/tensorlake/recipes/rust-dev",
        Some(serde_json::to_string(&bad).unwrap()),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(!body.contains(OTHER_TOKEN), "{body}");
    let mut legacy = serde_json::to_value(CreateRecipe {
        name: "legacy".into(),
        recipe: with_claude("http://127.0.0.1:8080", None),
    })
    .unwrap();
    legacy["recipe"]["claude"]["token"] = serde_json::json!(OTHER_TOKEN);
    for malformed in [
        format!(r#"{{"name":"x","recipe":{{"size":"{OTHER_TOKEN}"}}}}"#),
        format!(r#"{{"name":"x","recipe":{{}},"{OTHER_TOKEN}":1}}"#),
        serde_json::to_string(&legacy).unwrap(),
    ] {
        let (status, body) =
            call(&harness, "POST", "/api/tensorlake/recipes", Some(malformed)).await;
        assert_eq!(
            (status, body.as_str()),
            (StatusCode::BAD_REQUEST, "invalid recipe")
        );
    }
}

/// Spec: a delete refused because hosts use the recipe answers 409 with a
/// message naming the recipe and the count, and the other refusals map to
/// their statuses.
///
/// Why: the route answers what the store refuses, and the in-use refusal
/// must name how many hosts block the delete (the store's own count is
/// pinned in `tensorlake::hosts::tests::a_recipe_in_use_cannot_be_deleted`).
#[farhelm_testtrace::test]
async fn refusals_map_to_statuses() {
    for (refusal, status) in [
        (Refusal::Exists, StatusCode::CONFLICT),
        (Refusal::Missing, StatusCode::NOT_FOUND),
        (Refusal::Invalid("x".into()), StatusCode::BAD_REQUEST),
        (Refusal::InUse(1), StatusCode::CONFLICT),
        (Refusal::InUse(3), StatusCode::CONFLICT),
    ] {
        assert_eq!(refusal_response("rust-dev", refusal).status(), status);
    }
    for (hosts, expected) in [
        (1, "a host uses recipe rust-dev; delete that host first"),
        (3, "3 hosts use recipe rust-dev; delete those hosts first"),
    ] {
        let response = refusal_response("rust-dev", Refusal::InUse(hosts));
        let bytes = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        assert_eq!(String::from_utf8(bytes.to_vec()).unwrap(), expected);
    }
}
