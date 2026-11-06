//! Contract tests use private marker values and the real protected router.
//! Catalog replies are checked recursively by shape as well as by marker:
//! a new response field must not accidentally turn a write-only value readable.

use super::*;
use crate::rest_harness::{FleetBuilder, Harness};
use axum::{body::Body, http::Request};
use tower::ServiceExt as _;

const FIRST: &str = "private-credential-marker-first";
const SECOND: &str = "private-credential-marker-second";

/// Keep the database directory alive for the whole store contract test.
async fn store() -> (tempfile::TempDir, HelmStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
    (dir, store)
}

/// One globally named identity; callers vary only its consumer and value.
fn create(kind: CredentialKind, secret: &str) -> CreateCredential {
    CreateCredential {
        name: "work".into(),
        kind,
        secret: secret.into(),
    }
}

/// Observe persistence privately without adding any product secret-read API.
async fn raw_secret(store: &HelmStore) -> String {
    store
        .conn()
        .call("test credential contents", |conn| {
            Ok(conn.query_row(
                "SELECT secret FROM credentials WHERE name = 'work'",
                [],
                |row| row.get(0),
            )?)
        })
        .await
        .unwrap()
}

/// Creation identity stays private; metadata and recipes continue naming the
/// credential while delivery can distinguish a later reuse of the same name.
async fn incarnation(store: &HelmStore) -> String {
    store
        .conn()
        .call("test credential incarnation", |conn| {
            Ok(conn.query_row(
                "SELECT incarnation FROM credentials WHERE name = 'work'",
                [],
                |row| row.get(0),
            )?)
        })
        .await
        .unwrap()
}

/// A recreated name can have revision1 again, but must never masquerade as
/// the earlier credential a host remembers. Replacement keeps its incarnation.
#[farhelm_testtrace::test]
async fn recreation_changes_private_incarnation_while_replacement_keeps_it() {
    let (_dir, store) = store().await;
    store
        .create_credential(create(CredentialKind::GithubToken, FIRST))
        .await
        .unwrap();
    let first = incarnation(&store).await;
    uuid::Uuid::parse_str(&first).unwrap();
    store
        .save_credential(
            "work".into(),
            SaveCredential {
                secret: Some(SECOND.into()),
            },
        )
        .await
        .unwrap();
    assert_eq!(incarnation(&store).await, first);
    store.delete_credential("work".into()).await.unwrap();
    let recreated = store
        .create_credential(create(CredentialKind::GithubToken, SECOND))
        .await
        .unwrap();
    assert_eq!(recreated.revision, 1);
    let second = incarnation(&store).await;
    uuid::Uuid::parse_str(&second).unwrap();
    assert_ne!(first, second);
    let public = serde_json::to_string(&store.credentials().await.unwrap()).unwrap();
    assert!(!public.contains(&first) && !public.contains(&second));
}

/// Fixed identities survive replacement and blank saves; concurrent creates
/// cannot overwrite one another, and missing saves cannot resurrect deletions.
/// Revisions distinguish replacements within the same timestamp second.
#[farhelm_testtrace::test]
async fn replacement_keeps_identity_and_blank_keeps_revision() {
    let (_dir, store) = store().await;
    let row = store
        .create_credential(create(CredentialKind::GithubToken, FIRST))
        .await
        .unwrap();
    assert_eq!(row.revision, 1);
    assert!(matches!(
        store
            .create_credential(create(CredentialKind::EndpointToken, SECOND))
            .await,
        Err(Refusal::Exists)
    ));
    assert_eq!(raw_secret(&store).await, FIRST);
    for secret in [None, Some(" \n ".into())] {
        assert_eq!(
            store
                .save_credential("work".into(), SaveCredential { secret })
                .await
                .unwrap(),
            row
        );
    }
    let changed = store
        .save_credential(
            "work".into(),
            SaveCredential {
                secret: Some(format!(" {SECOND} ")),
            },
        )
        .await
        .unwrap();
    assert_eq!(changed.revision, 2);
    assert_eq!(changed.created_at, row.created_at);
    assert_eq!(changed.kind, CredentialKind::GithubToken);
    assert_eq!(raw_secret(&store).await, SECOND);
    assert!(
        store
            .save_credential(
                "work".into(),
                SaveCredential {
                    secret: Some("bad\0value".into())
                }
            )
            .await
            .is_err()
    );
    assert_eq!(store.credentials().await.unwrap(), vec![changed]);
    store.delete_credential("work".into()).await.unwrap();
    assert!(matches!(
        store
            .save_credential("work".into(), SaveCredential::default())
            .await,
        Err(Refusal::Missing)
    ));
    assert!(matches!(
        store.delete_credential("work".into()).await,
        Err(Refusal::Missing)
    ));
}

/// Every writer must acknowledge its own committed revision. A second metadata
/// read after releasing the write transaction could instead acknowledge a later
/// writer's value, defeating host-delivery revision comparisons.
#[farhelm_testtrace::test]
async fn concurrent_replacements_acknowledge_distinct_revisions() {
    let (_dir, store) = store().await;
    store
        .create_credential(create(CredentialKind::GithubToken, FIRST))
        .await
        .unwrap();
    let saves = (0..16).map(|index| {
        store.save_credential(
            "work".into(),
            SaveCredential {
                secret: Some(format!("fictional-parallel-token-{index}")),
            },
        )
    });
    let replies = futures_util::future::join_all(saves).await;
    let revisions: std::collections::BTreeSet<_> = replies
        .into_iter()
        .map(|reply| reply.unwrap().revision)
        .collect();
    assert_eq!(revisions, (2..=17).collect());
    assert_eq!(store.credentials().await.unwrap()[0].revision, 17);
}

/// Codex adds fields independently of Farhelm. Accept any bounded JSON object,
/// preserve its contents, and reject malformed/oversized replacements atomically.
#[farhelm_testtrace::test]
async fn codex_accepts_opaque_objects_and_revision_overflow_is_safe() {
    let (_dir, store) = store().await;
    let json = format!(r#"{{ "future": {{"secret":"{FIRST}"}}, "version":999 }}"#);
    store
        .create_credential(create(CredentialKind::CodexLogin, &json))
        .await
        .unwrap();
    assert_eq!(raw_secret(&store).await, json);
    for bad in [
        "[]".into(),
        format!(r#"{{"secret":"{SECOND}""#),
        format!(r#"{{"value":"{}"}}"#, "x".repeat(64 * 1024)),
    ] {
        assert!(matches!(
            store
                .save_credential("work".into(), SaveCredential { secret: Some(bad) })
                .await,
            Err(Refusal::Invalid(_))
        ));
        assert_eq!(raw_secret(&store).await, json);
    }
    store
        .conn()
        .call("exhaust revision", |conn| {
            conn.execute("UPDATE credentials SET revision = ?1", [i64::MAX])?;
            Ok(())
        })
        .await
        .unwrap();
    assert!(matches!(
        store
            .save_credential(
                "work".into(),
                SaveCredential {
                    secret: Some("{}".into())
                }
            )
            .await,
        Err(Refusal::Storage)
    ));
    assert_eq!(raw_secret(&store).await, json);
}

/// Upgrade a real schema-74-shaped database, then reopen the new catalog. This
/// also pins idempotence for the version-rewind migration fixtures elsewhere.
#[farhelm_testtrace::test]
async fn schema_74_upgrade_creates_a_durable_catalog() {
    let (dir, store) = store().await;
    store
        .conn()
        .call("rewind credential schema", |conn| {
            conn.execute_batch("DROP TABLE credentials; PRAGMA user_version = 74;")?;
            Ok(())
        })
        .await
        .unwrap();
    drop(store);
    let store = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
    assert!(store.credentials().await.unwrap().is_empty());
    store
        .create_credential(create(CredentialKind::ClaudeLogin, FIRST))
        .await
        .unwrap();
    drop(store);
    let reopened = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
    assert_eq!(reopened.credentials().await.unwrap()[0].revision, 1);
    assert_eq!(raw_secret(&reopened).await, FIRST);
}

/// Exercise the production request decoder and protected mutation routes. No
/// stand-in HTTP handler can establish that rejected JSON is content-free.
async fn call(harness: &Harness, method: &str, path: &str, body: &str) -> (StatusCode, String) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap();
    let response = harness.router().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(
        !text.contains(FIRST) && !text.contains(SECOND),
        "secret-free response"
    );
    (status, text)
}

/// Successful reads and writes return exactly metadata; collisions, malformed
/// bodies, kind changes and missing identities cannot disclose submitted text.
#[farhelm_testtrace::test]
async fn routes_never_return_secrets_and_refuse_identity_changes() {
    let harness = FleetBuilder::new().await.start().await;
    let body = serde_json::to_string(&create(CredentialKind::EndpointToken, FIRST)).unwrap();
    let (status, text) = call(&harness, "POST", "/api/credentials", &body).await;
    assert_eq!(status, StatusCode::CREATED);
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    let keys: std::collections::BTreeSet<_> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        [
            "created_at",
            "kind",
            "name",
            "revision",
            "updated_at",
            "used_by"
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        call(&harness, "POST", "/api/credentials", &body).await.0,
        StatusCode::CONFLICT
    );
    for bad in [
        format!(r#"{{"kind":"{FIRST}","secret":"{SECOND}"}}"#),
        format!(r#"{{"secret":"{FIRST}""#),
    ] {
        assert_eq!(
            call(&harness, "PUT", "/api/credentials/work", &bad).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    let (status, text) = call(&harness, "GET", "/api/credentials", "").await;
    assert_eq!(status, StatusCode::OK);
    let list: Vec<Credential> = serde_json::from_str(&text).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].revision, 1);
    let replacement = serde_json::to_string(&SaveCredential {
        secret: Some(SECOND.into()),
    })
    .unwrap();
    let (status, text) = call(&harness, "PUT", "/api/credentials/work", &replacement).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_str::<Credential>(&text).unwrap().revision,
        2
    );
    assert_eq!(
        call(&harness, "DELETE", "/api/credentials/work", "")
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&harness, "PUT", "/api/credentials/work", &replacement)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&harness, "DELETE", "/api/credentials/work", "")
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

/// Credentials must be behind the real authentication boundary even for reads.
/// An oversized authenticated paste must likewise receive fixed refusal text,
/// rather than a decoder diagnostic quoting a fragment of its contents.
#[farhelm_testtrace::test]
async fn authentication_and_body_limit_refuse_without_echoing_contents() {
    let harness = FleetBuilder::new().await.start().await;
    for (method, path) in [
        ("GET", "/api/credentials"),
        ("POST", "/api/credentials"),
        ("PUT", "/api/credentials/work"),
        ("DELETE", "/api/credentials/work"),
    ] {
        let request = Request::builder()
            .method(method)
            .uri(path)
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(Body::from(format!(r#"{{"secret":"{FIRST}"}}"#)))
            .unwrap();
        let response = harness
            .unauthenticated_router()
            .oneshot(request)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let bytes = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains(FIRST));
    }
    let oversized = format!(
        r#"{{"name":"work","kind":"github_token","secret":"{FIRST}{}"}}"#,
        "x".repeat(BODY_LIMIT)
    );
    let (status, body) = call(&harness, "POST", "/api/credentials", &oversized).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body, "could not read the credential request");
    let (_, rows) = call(&harness, "GET", "/api/credentials", "").await;
    assert_eq!(rows, "[]");
}
