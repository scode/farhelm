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

/// A fresh store in its own temporary directory. The directory is returned so
/// it outlives the store.
async fn store() -> (tempfile::TempDir, HelmStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = HelmStore::open(&dir.path().join("helm.db"))
        .await
        .expect("open a fresh store");
    (dir, store)
}

/// `save()` with a Claude Code endpoint at `base_url` and the given token
/// field.
fn with_claude(base_url: &str, token: Option<&str>) -> RecipeSave {
    let mut recipe = save();
    recipe.claude = Some(EndpointSave {
        base_url: base_url.into(),
        token: token.map(str::to_string),
    });
    recipe
}

/// The stored Claude Code token of recipe `name`, read through the store.
async fn claude_token(store: &HelmStore, name: &str) -> Option<String> {
    store
        .tensorlake_recipes()
        .await
        .unwrap()
        .into_iter()
        .find(|(stored, _)| stored == name)
        .expect("the recipe exists")
        .1
        .claude
        .and_then(|endpoint| endpoint.token)
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
/// replace the other recipe's tokens.
#[farhelm_testtrace::test]
async fn create_refuses_an_existing_name_and_invalid_input() {
    let (_dir, store) = store().await;
    store
        .create_tensorlake_recipe(
            "rust-dev".into(),
            with_claude("http://127.0.0.1:8080", Some(TOKEN)),
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

/// Spec: an omitted token survives an update only while the endpoint's
/// origin (scheme, host, effective port) is unchanged; a path change keeps
/// it, an origin change drops it, an empty token clears it, a new token
/// replaces it, and removing the endpoint drops its token.
///
/// Why: the form never receives the saved token, so "leave blank to keep" is
/// the only way to edit a recipe without retyping it. Resolving that against
/// the stored row inside the save is what stops a stale form from moving a
/// token to a server the user did not intend to send it to (ntfy's rule).
#[farhelm_testtrace::test]
async fn endpoint_tokens_follow_the_origin_rule() {
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
            with_claude("http://127.0.0.1:8080", Some(TOKEN)),
        )
        .await
        .unwrap()
        .unwrap_or_else(|_| panic!("create succeeds"));
    assert_eq!(claude_token(&store, name).await.as_deref(), Some(TOKEN));

    update(with_claude("http://127.0.0.1:8080/v1", None)).await;
    assert_eq!(
        claude_token(&store, name).await.as_deref(),
        Some(TOKEN),
        "same origin, other path: kept"
    );
    update(with_claude("HTTP://127.0.0.1:8080", None)).await;
    assert_eq!(
        claude_token(&store, name).await.as_deref(),
        Some(TOKEN),
        "same origin spelled differently: kept"
    );
    update(with_claude("http://127.0.0.1:9090", None)).await;
    assert_eq!(
        claude_token(&store, name).await,
        None,
        "origin change: dropped"
    );

    update(with_claude("http://127.0.0.1:8080", Some(TOKEN))).await;
    update(with_claude("http://127.0.0.1:8080", Some(OTHER_TOKEN))).await;
    assert_eq!(
        claude_token(&store, name).await.as_deref(),
        Some(OTHER_TOKEN),
        "replaced"
    );
    update(with_claude("http://127.0.0.1:8080", Some(""))).await;
    assert_eq!(claude_token(&store, name).await, None, "cleared");

    update(with_claude("http://127.0.0.1:8080", Some(TOKEN))).await;
    update(save()).await;
    update(with_claude("http://127.0.0.1:8080", None)).await;
    assert_eq!(
        claude_token(&store, name).await,
        None,
        "removing the endpoint dropped its token; re-adding it does not restore it"
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
    FleetBuilder::new().await.start().await
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
/// body) contains a saved or submitted token; reads report `token_set`.
///
/// Why: tokens are write-only, like the ntfy token and the Tensorlake API
/// key. Every client of the helm can read these routes, and nothing needs a
/// token back. A malformed body is the subtle case: serde's own diagnostic
/// quotes the offending value.
#[farhelm_testtrace::test]
async fn tokens_never_appear_in_replies() {
    let harness = helm().await;
    let recipe = with_claude("http://127.0.0.1:8080", Some(TOKEN));
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
            token_set: true
        })
    );
    assert_eq!(view.codex, None);

    let (_, body) = call(&harness, "GET", "/api/tensorlake/recipes", None).await;
    assert!(!body.contains(TOKEN), "{body}");
    assert!(body.contains(r#""token_set":true"#), "{body}");

    // Keep the token across an unrelated edit, then read it back only as a flag.
    let mut edited = with_claude("http://127.0.0.1:8080", None);
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
    assert!(body.contains(r#""token_set":true"#), "{body}");
    assert_eq!(
        claude_token(&harness.store, "rust-dev").await.as_deref(),
        Some(TOKEN),
        "premise: the token is stored, so its absence above is the redaction"
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
    for malformed in [
        format!(r#"{{"name":"x","recipe":{{"size":"{OTHER_TOKEN}"}}}}"#),
        format!(r#"{{"name":"x","recipe":{{}},"{OTHER_TOKEN}":1}}"#),
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
/// Why: no host can use a recipe until Tensorlake hosts exist, so the store's
/// usage check returns zero today and this refusal cannot be reached through
/// the route yet; the mapping is pinned here so the hosts change only has to
/// fill in `hosts_using_recipe`.
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
