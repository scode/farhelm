//! Browser authentication at the helm's HTTP boundary.
//!
//! The web token is the recoverable bootstrap secret the user can print with
//! `farhelm helm token show`. A browser exchanges it once for a random device
//! secret; only that secret's SHA-256 digest is stored. REST carries it
//! in `Authorization`, while browser WebSockets offer it as a subprotocol so
//! the credential remains scoped to the helm's complete origin, port included.
//! The middleware in this module covers every protected API method, including
//! both WebSocket handshakes. The static bundle, bootstrap exchange, and the
//! attachment OPTIONS preflight remain reachable before a device has
//! authenticated; the preflight carries no credentials by browser contract.

use crate::{AppState, store::HelmStore};
use anyhow::Context;
use axum::extract::{Json, Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use subtle::ConstantTimeEq;

/// Application protocol every authenticated browser WebSocket offers.
///
/// The device secret is a second offered protocol. Selecting this stable
/// value satisfies the WebSocket negotiation contract without reflecting a
/// credential in the upgrade response.
pub(crate) const WS_PROTOCOL: &str = "farhelm";

/// Prefix separating the application protocol from the credential-bearing
/// protocol token.
const WS_DEVICE_PREFIX: &str = "farhelm-device-";

/// URL-safe base64 without padding encodes a 128-bit token in 22 characters.
const SECRET_LENGTH: usize = 22;

/// Structured marker emitted only by the device-authentication boundary.
pub(crate) const AUTH_REQUIRED_CODE: &str = "device_auth_required";

#[derive(Debug, thiserror::Error)]
enum ExchangeError {
    #[error("operating-system randomness is unavailable")]
    Randomness(#[source] anyhow::Error),
    #[error("the system clock is unavailable")]
    Clock(#[source] anyhow::Error),
    #[error("authentication storage is unavailable")]
    Storage(#[source] anyhow::Error),
    #[error("the serving helm has not loaded its bootstrap token")]
    TokenNotLoaded,
}

/// Safe categories the private control protocol may return to a local CLI.
#[derive(Debug, thiserror::Error)]
pub(crate) enum RotationError {
    #[error("operating-system randomness is unavailable")]
    Randomness(#[source] anyhow::Error),
    #[error("the system clock is unavailable")]
    Clock(#[source] anyhow::Error),
    #[error("authentication storage is unavailable; inspect the helm logs")]
    Storage(#[source] anyhow::Error),
}

/// Authentication state shared by the HTTP boundary and the local token
/// control socket.
///
/// The broadcast is process-local on purpose. Rotation is executed inside
/// the serving helm whenever one is running, so the same operation that
/// commits the database transaction can close every admitted socket without
/// polling or a durable auth epoch.
///
/// The bootstrap token is loaded before HTTP serving and cached here so an
/// invalid public exchange never contends for SQLite. The database still
/// validates every cache-approved exchange in the same immediate transaction
/// that inserts its device row; that transaction is the authority when a
/// rotation and exchange overlap.
#[derive(Clone)]
pub(crate) struct AuthState {
    store: HelmStore,
    token: Arc<tokio::sync::RwLock<Option<String>>>,
    rotation: Arc<tokio::sync::Mutex<()>>,
    revocations: tokio::sync::broadcast::Sender<()>,
}

impl AuthState {
    /// Build the one authentication authority for a helm process.
    pub(crate) fn new(store: HelmStore) -> AuthState {
        let (revocations, _) = tokio::sync::broadcast::channel(16);
        AuthState {
            store,
            token: Arc::new(tokio::sync::RwLock::new(None)),
            rotation: Arc::new(tokio::sync::Mutex::new(())),
            revocations,
        }
    }

    /// Return the stable bootstrap token and retain it for lock-free exchange
    /// rejection. Production calls this before HTTP serving begins.
    pub(crate) async fn token(&self) -> anyhow::Result<String> {
        if let Some(token) = self.token.read().await.as_ref() {
            return Ok(token.clone());
        }
        let token = show_token(&self.store).await?;
        let mut cached = self.token.write().await;
        Ok(cached.get_or_insert(token).clone())
    }

    /// Rotate the bootstrap token, delete every device row in the same
    /// transaction, then revoke every live authenticated socket.
    ///
    /// Revocation follows the commit. Broadcasting first would close clients
    /// even when SQLite rejected the rotation and left their credentials valid.
    pub(crate) async fn rotate(&self) -> Result<String, RotationError> {
        let _serialized = self.rotation.lock().await;
        let replacement = mint_secret().map_err(RotationError::Randomness)?;
        let created_at = unix_timestamp().map_err(RotationError::Clock)?;
        let token = self
            .store
            .rotate_web_token(replacement, created_at)
            .await
            .map_err(RotationError::Storage)?;
        *self.token.write().await = Some(token.clone());
        let _ = self.revocations.send(());
        Ok(token)
    }

    /// Mint and persist one browser device credential for test setup.
    ///
    /// Production exchange uses [`Self::exchange`] so token validation and
    /// insertion remain one transaction. This helper still goes through that
    /// boundary rather than writing an otherwise-impossible device row.
    #[cfg(test)]
    pub(crate) async fn mint_device(&self) -> anyhow::Result<String> {
        let token = self.token().await?;
        self.exchange(token)
            .await?
            .context("the stored web token must authenticate its own device exchange")
    }

    /// Exchange one valid bootstrap token for a new device secret.
    async fn exchange(&self, supplied_token: String) -> Result<Option<String>, ExchangeError> {
        let expected = self
            .token
            .read()
            .await
            .clone()
            .ok_or(ExchangeError::TokenNotLoaded)?;
        let accepted: bool = digest(expected.as_bytes())
            .ct_eq(&digest(supplied_token.as_bytes()))
            .into();
        if !accepted {
            return Ok(None);
        }

        let secret = mint_secret().map_err(ExchangeError::Randomness)?;
        let created_at = unix_timestamp().map_err(ExchangeError::Clock)?;
        let accepted = self
            .store
            .exchange_device_session(supplied_token, digest(secret.as_bytes()), created_at)
            .await
            .map_err(ExchangeError::Storage)?;
        Ok(accepted.then_some(secret))
    }

    /// Validate a browser device secret through the digest primary key.
    async fn accepts_device(&self, secret: &str) -> anyhow::Result<bool> {
        self.store
            .has_device_session(digest(secret.as_bytes()))
            .await
    }

    /// Subscribe before authenticating a WebSocket handshake so rotation
    /// cannot land in the gap between admission and subscription.
    pub(crate) fn socket_session(&self) -> AuthenticatedSocket {
        AuthenticatedSocket {
            revocations: Arc::new(tokio::sync::Mutex::new(self.revocations.subscribe())),
        }
    }
}

/// The revocation subscription carried from authenticated middleware into a
/// WebSocket serving task.
///
/// Wrapped so axum can clone request extensions. A request owns one receiver;
/// the clone moved into `on_upgrade` is the only consumer that waits on it.
#[derive(Clone)]
pub(crate) struct AuthenticatedSocket {
    revocations: Arc<tokio::sync::Mutex<tokio::sync::broadcast::Receiver<()>>>,
}

impl AuthenticatedSocket {
    /// Resolve when token rotation invalidates this socket's device session.
    pub(crate) async fn revoked(&self) {
        let _ = self.revocations.lock().await.recv().await;
    }

    /// Observe a revocation that already landed without waiting for a future
    /// rotation. A lag or sender shutdown is equally terminal: either means
    /// this socket can no longer prove it remained continuously authorized.
    pub(crate) async fn is_revoked(&self) -> bool {
        use tokio::sync::broadcast::error::TryRecvError;

        match self.revocations.lock().await.try_recv() {
            Ok(()) | Err(TryRecvError::Lagged(_) | TryRecvError::Closed) => true,
            Err(TryRecvError::Empty) => false,
        }
    }
}

/// The deliberately small public bootstrap request body.
#[derive(Deserialize)]
pub(crate) struct TokenExchange {
    token: String,
}

/// Successful bootstrap exchange response. The browser persists this value in
/// origin-scoped localStorage and supplies it explicitly on every API edge.
#[derive(Serialize)]
pub(crate) struct DeviceExchange {
    device_secret: String,
}

/// `POST /api/auth/token`: exchange the recoverable bootstrap token for a
/// persistent, origin-scoped device secret.
pub(crate) async fn exchange_token(
    State(state): State<Arc<AppState>>,
    Json(exchange): Json<TokenExchange>,
) -> Response {
    if !valid_secret_shape(&exchange.token) {
        return unauthenticated();
    }
    match state.auth.exchange(exchange.token).await {
        Ok(Some(device_secret)) => {
            (StatusCode::OK, Json(DeviceExchange { device_secret })).into_response()
        }
        Ok(None) => unauthenticated(),
        Err(error) => exchange_internal_error(error),
    }
}

/// `GET /api/auth/device`: prove that one persisted desktop credential is
/// still valid without minting a replacement on transport failures.
///
/// Authentication middleware runs before this handler. Reaching it is the
/// entire success condition; the empty response carries no credential or
/// account data across the webview's custom origin.
pub(crate) async fn validate_device() -> StatusCode {
    StatusCode::NO_CONTENT
}

/// Require an explicit device secret before any protected REST handler or
/// WebSocket upgrade is reached.
pub(crate) async fn require_device_session(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Response {
    let websocket_request = request
        .headers()
        .get(header::UPGRADE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("websocket"));
    let websocket = websocket_device_secret(request.headers());
    let socket_session = websocket_request.then(|| state.auth.socket_session());
    let supplied = if websocket_request {
        websocket.as_deref()
    } else {
        authorization_device_secret(request.headers())
    };
    match supplied {
        Some(secret) => match state.auth.accepts_device(secret).await {
            Ok(true) => {
                if let Some(socket_session) = socket_session {
                    request.extensions_mut().insert(socket_session);
                }
                next.run(request).await
            }
            Ok(false) => unauthenticated(),
            Err(error) => internal_error(error),
        },
        None => unauthenticated(),
    }
}

/// Return the stored token, minting the first one without assuming this
/// caller wins a concurrent first-use race.
pub(crate) async fn show_token(store: &HelmStore) -> anyhow::Result<String> {
    if let Some(token) = store.web_token().await? {
        return Ok(token);
    }
    store
        .web_token_or_insert(mint_secret()?, unix_timestamp()?)
        .await
}

/// Rotate directly in storage. Used only when no serving helm control socket
/// exists, so there can be no live WebSocket in that process to revoke.
pub(crate) async fn rotate_offline(store: &HelmStore) -> anyhow::Result<String> {
    let replacement = mint_secret()?;
    store.rotate_web_token(replacement, unix_timestamp()?).await
}

/// Extract the one exact REST authentication scheme this API accepts.
fn authorization_device_secret(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .filter(|secret| !secret.is_empty())
}

/// Decode the credential-bearing protocol only when the stable application
/// protocol is offered beside exactly one non-empty secret.
fn websocket_device_secret(headers: &axum::http::HeaderMap) -> Option<String> {
    let mut application = false;
    let mut secret = None;
    for protocol in headers
        .get_all(header::SEC_WEBSOCKET_PROTOCOL)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
    {
        if protocol == WS_PROTOCOL {
            application = true;
        } else if let Some(candidate) = protocol.strip_prefix(WS_DEVICE_PREFIX)
            && (candidate.is_empty() || secret.replace(candidate.to_string()).is_some())
        {
            return None;
        }
    }
    application.then_some(secret).flatten()
}

/// Fixed-size digest used as the device-session primary key.
fn digest(value: &[u8]) -> [u8; 32] {
    Sha256::digest(value).into()
}

/// Reject malformed bootstrap candidates before doing storage or hashing
/// work. Valid tokens are exactly one unpadded base64url encoding of 128 bits.
fn valid_secret_shape(secret: &str) -> bool {
    secret.len() == SECRET_LENGTH
        && secret
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

/// Mint a URL-safe 128-bit credential without padding or punctuation that a
/// HTTP and WebSocket protocol tokens can carry without escaping.
fn mint_secret() -> anyhow::Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).context("reading operating-system randomness")?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

fn unix_timestamp() -> anyhow::Result<i64> {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_secs();
    i64::try_from(seconds).context("system clock does not fit in SQLite INTEGER")
}

fn unauthenticated() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::CONTENT_TYPE, "application/json")],
        format!(r#"{{"error":"unauthenticated","code":"{AUTH_REQUIRED_CODE}"}}"#),
    )
        .into_response()
}

/// Return only fixed, actionable failure categories while logging the full
/// causal chain for the operator who can act on it.
fn exchange_internal_error(error: ExchangeError) -> Response {
    tracing::error!(error = %error, source = ?std::error::Error::source(&error), "device exchange failed internally");
    let (code, detail) = match error {
        ExchangeError::Randomness(_) => (
            "randomness_unavailable",
            "operating-system randomness is unavailable; retry after checking the helm host",
        ),
        ExchangeError::Clock(_) => (
            "clock_unavailable",
            "the helm host clock is unavailable; correct it and retry",
        ),
        ExchangeError::Storage(_) => (
            "authentication_storage_unavailable",
            "authentication storage is unavailable; inspect the helm logs and retry",
        ),
        ExchangeError::TokenNotLoaded => (
            "authentication_not_ready",
            "device authentication is not ready; retry after the helm finishes starting",
        ),
    };
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        [(header::CONTENT_TYPE, "application/json")],
        serde_json::json!({ "error": code, "detail": detail }).to_string(),
    )
        .into_response()
}

fn internal_error(error: anyhow::Error) -> Response {
    tracing::error!(error = %error, "browser authentication failed internally");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        [(header::CONTENT_TYPE, "application/json")],
        r#"{"error":"internal"}"#,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BUILD_STAMP_HEADER, rest_harness};
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, header};
    use tower::ServiceExt;

    /// Both explicit transports require their complete scheme/protocol names;
    /// lookalike prefixes must not turn attacker-controlled headers into a
    /// device credential.
    #[test]
    fn device_headers_require_the_complete_transport_shape() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "Bearer secret".parse().unwrap());
        assert_eq!(authorization_device_secret(&headers), Some("secret"));
        headers.insert(header::AUTHORIZATION, "NotBearer secret".parse().unwrap());
        assert_eq!(authorization_device_secret(&headers), None);

        headers.insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            "farhelm, farhelm-device-secret".parse().unwrap(),
        );
        assert_eq!(websocket_device_secret(&headers).as_deref(), Some("secret"));
        headers.insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            "not-farhelm, farhelm-device-secret".parse().unwrap(),
        );
        assert_eq!(websocket_device_secret(&headers), None);
    }

    /// The middleware boundary is structural: an ordinary REST route is
    /// refused before its handler, and the outer build-stamp layer still
    /// decorates that refusal so skew wins over login in the browser.
    #[tokio::test]
    async fn unauthenticated_rest_is_structured_401_with_a_build_stamp() {
        let harness = rest_harness::idle_helm().await;
        let request = Request::builder()
            .uri("/api/sessions")
            .header(header::HOST, "127.0.0.1:7433")
            .body(Body::empty())
            .unwrap();
        let response = harness
            .unauthenticated_router()
            .oneshot(request)
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers().get(BUILD_STAMP_HEADER).unwrap(),
            env!("CARGO_PKG_VERSION")
        );
        assert_eq!(
            to_bytes(response.into_body(), 1024).await.unwrap(),
            r#"{"error":"unauthenticated","code":"device_auth_required"}"#
        );
    }

    /// WebSockets are HTTP until the upgrade completes, so every protected
    /// socket route must answer a missing device secret with 401 rather than
    /// opening and then emitting a close code the browser has to reinterpret.
    #[tokio::test]
    async fn unauthenticated_websocket_is_refused_before_upgrade() {
        let mut harness = rest_harness::idle_helm().await;
        let addr = harness.serve_unauthenticated().await;
        for route in [
            "/api/events",
            "/api/sessions/missing/term",
            "/api/sessions/missing/term/unowned",
        ] {
            assert!(
                matches!(
                    rest_harness::WsTestClient::try_connect(addr, route).await,
                    Err(401)
                ),
                "{route} must be refused before upgrade"
            );
        }
    }

    /// The bootstrap path is public, returns a device secret in its body, and
    /// that secret alone authenticates later reads through Authorization.
    #[tokio::test]
    async fn token_exchange_mints_an_explicit_device_session() {
        let harness = rest_harness::idle_helm().await;
        let token = harness.state.auth.rotate().await.unwrap();
        let exchange = Request::builder()
            .method("POST")
            .uri("/api/auth/token")
            .header(header::HOST, "127.0.0.1:7433")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::json!({"token": token}).to_string()))
            .unwrap();
        let response = harness
            .unauthenticated_router()
            .oneshot(exchange)
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().get(header::SET_COOKIE).is_none());
        let body: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 1024).await.unwrap()).unwrap();
        let secret = body["device_secret"].as_str().unwrap();
        let expected: [u8; 32] = Sha256::digest(secret.as_bytes()).into();
        assert_eq!(
            harness.store.device_session_hashes().await.unwrap(),
            vec![expected],
            "the independent SHA-256 digest must be the sole stored credential form"
        );

        let read = Request::builder()
            .uri("/api/sessions")
            .header(header::HOST, "127.0.0.1:7433")
            .header(header::AUTHORIZATION, format!("Bearer {secret}"))
            .body(Body::empty())
            .unwrap();
        let response = harness
            .unauthenticated_router()
            .oneshot(read)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let validate = Request::builder()
            .uri("/api/auth/device")
            .header(header::HOST, "127.0.0.1:7433")
            .header(header::ORIGIN, "dioxus://index.html")
            .header(header::AUTHORIZATION, format!("Bearer {secret}"))
            .body(Body::empty())
            .unwrap();
        let response = harness
            .unauthenticated_router()
            .oneshot(validate)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap(),
            "dioxus://index.html"
        );

        let rejected = Request::builder()
            .uri("/api/auth/device")
            .header(header::HOST, "127.0.0.1:7433")
            .header(header::ORIGIN, "dioxus://index.html")
            .header(header::AUTHORIZATION, "Bearer rejected")
            .body(Body::empty())
            .unwrap();
        let response = harness
            .unauthenticated_router()
            .oneshot(rejected)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap(),
            "dioxus://index.html",
            "the webview must be allowed to distinguish auth rejection from transport failure"
        );
        assert_eq!(
            harness.store.device_session_count().await.unwrap(),
            1,
            "validation must never mint a replacement device row"
        );
    }

    /// A browser must be able to load the application before it has a credential;
    /// otherwise there is nowhere to present the in-page token form.
    #[tokio::test]
    async fn static_bundle_is_public() {
        let harness = rest_harness::idle_helm().await;
        let dist = tempfile::tempdir().unwrap();
        std::fs::write(dist.path().join("index.html"), "token prompt shell").unwrap();
        let request = Request::builder()
            .uri("/")
            .header(header::HOST, "127.0.0.1:7433")
            .body(Body::empty())
            .unwrap();
        let response = crate::build_router(Arc::clone(&harness.state), Some(dist.path()), 7433)
            .oneshot(request)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            to_bytes(response.into_body(), 1024).await.unwrap(),
            "token prompt shell"
        );
    }

    /// The recoverable token is durable rather than process-local: `show`
    /// after a helm restart must print the same value until rotation.
    #[tokio::test]
    async fn token_show_survives_a_helm_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("helm.db");
        let store = HelmStore::open(&path).await.unwrap();
        let before = show_token(&store).await.unwrap();
        drop(store);
        let reopened = HelmStore::open(&path).await.unwrap();
        let after = show_token(&reopened).await.unwrap();
        assert_eq!(after, before);
    }

    /// The exchange accepts the token only in its JSON body. A wrong body
    /// remains wrong even if the correct token is smuggled into the query,
    /// and neither refusal may leave a device row behind.
    #[tokio::test]
    async fn refused_exchanges_never_mint_a_device_session() {
        let harness = rest_harness::idle_helm().await;
        let token = harness.state.auth.token().await.unwrap();
        let before = harness.store.device_session_count().await.unwrap();
        let wrong = if token == "AAAAAAAAAAAAAAAAAAAAAA" {
            "BBBBBBBBBBBBBBBBBBBBBB"
        } else {
            "AAAAAAAAAAAAAAAAAAAAAA"
        };
        for uri in [
            "/api/auth/token".to_string(),
            format!("/api/auth/token?token={token}"),
        ] {
            let request = Request::builder()
                .method("POST")
                .uri(uri)
                .header(header::HOST, "127.0.0.1:7433")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::json!({"token": wrong}).to_string()))
                .unwrap();
            let response = harness
                .unauthenticated_router()
                .oneshot(request)
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
        assert_eq!(harness.store.device_session_count().await.unwrap(), before);
    }

    /// Invalid, well-shaped tokens are rejected from the in-memory authority
    /// even while another connection holds SQLite's write lock. Reaching the
    /// exchange transaction would make this call wait behind that lock.
    #[tokio::test]
    async fn invalid_exchange_never_touches_sqlite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("helm.db");
        let store = HelmStore::open(&path).await.unwrap();
        let auth = AuthState::new(store);
        let token = auth.token().await.unwrap();
        let wrong = if token == "AAAAAAAAAAAAAAAAAAAAAA" {
            "BBBBBBBBBBBBBBBBBBBBBB"
        } else {
            "AAAAAAAAAAAAAAAAAAAAAA"
        };
        let mut blocker = rusqlite::Connection::open(&path).unwrap();
        let _transaction = blocker
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .unwrap();

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            auth.exchange(wrong.to_string()),
        )
        .await
        .expect("invalid validation must not wait for SQLite")
        .unwrap();
        assert!(result.is_none());
    }

    /// Storage failures return a fixed actionable category and never expose
    /// SQLite trigger text supplied by the database.
    #[tokio::test]
    async fn exchange_storage_failure_is_actionable_and_sanitized() {
        let harness = rest_harness::idle_helm().await;
        let token = harness.state.auth.token().await.unwrap();
        harness.store.refuse_device_inserts_for_test().await;
        let request = Request::builder()
            .method("POST")
            .uri("/api/auth/token")
            .header(header::HOST, "127.0.0.1:7433")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::json!({"token": token}).to_string()))
            .unwrap();
        let response = harness
            .unauthenticated_router()
            .oneshot(request)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = String::from_utf8(to_bytes(response.into_body(), 1024).await.unwrap().to_vec())
            .unwrap();
        assert!(body.contains("authentication_storage_unavailable"));
        assert!(body.contains("inspect the helm logs"));
        assert!(!body.contains("secret database detail"));
    }

    /// A failed rotation preserves all three old-authority facts: token,
    /// device admission, and the absence of a live-socket revocation event.
    #[tokio::test]
    async fn failed_rotation_keeps_old_credentials_and_sockets() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("helm.db");
        let store = HelmStore::open(&path).await.unwrap();
        let auth = AuthState::new(store);
        let old_token = auth.token().await.unwrap();
        let old_device = auth.mint_device().await.unwrap();
        let socket = auth.socket_session();
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TRIGGER refuse_auth_delete BEFORE DELETE ON device_sessions \
             BEGIN SELECT RAISE(ABORT, 'scripted rotation failure'); END;",
        )
        .unwrap();

        assert!(matches!(
            auth.rotate().await,
            Err(RotationError::Storage(_))
        ));
        assert_eq!(auth.token().await.unwrap(), old_token);
        assert!(auth.accepts_device(&old_device).await.unwrap());
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), socket.revoked())
                .await
                .is_err(),
            "a transaction that did not commit must not revoke live sockets"
        );
    }

    /// Reusing one valid bootstrap token yields independent device authority,
    /// never a deterministic derivative of the token or prior exchange.
    #[tokio::test]
    async fn repeated_exchanges_mint_distinct_independently_usable_devices() {
        let harness = rest_harness::idle_helm().await;
        let token = harness.state.auth.token().await.unwrap();
        let first = exchange_through_router(&harness, &token).await;
        let second = exchange_through_router(&harness, &token).await;
        assert_ne!(first, second);
        for secret in [&first, &second] {
            assert_eq!(
                base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .decode(secret)
                    .unwrap()
                    .len(),
                16
            );
        }
        for secret in [first, second] {
            let request = Request::builder()
                .uri("/api/sessions")
                .header(header::HOST, "127.0.0.1:7433")
                .header(header::AUTHORIZATION, format!("Bearer {secret}"))
                .body(Body::empty())
                .unwrap();
            let response = harness
                .unauthenticated_router()
                .oneshot(request)
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }
    }

    /// Device sessions have no server-side age cutoff: rotation, not a wall
    /// clock comparison, is the sole expiry mechanism specified for v1.
    #[tokio::test]
    async fn arbitrarily_old_device_session_authenticates_until_rotation() {
        let dir = tempfile::tempdir().unwrap();
        let store = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
        let auth = AuthState::new(store.clone());
        let secret = "AAAAAAAAAAAAAAAAAAAAAA";
        store
            .insert_device_session(digest(secret.as_bytes()), i64::MIN)
            .await
            .unwrap();
        assert!(auth.accepts_device(secret).await.unwrap());
        auth.token().await.unwrap();
        auth.rotate().await.unwrap();
        assert!(!auth.accepts_device(secret).await.unwrap());
    }

    /// Drive the public exchange route and decode its explicit credential.
    async fn exchange_through_router(harness: &rest_harness::Harness, token: &str) -> String {
        let request = Request::builder()
            .method("POST")
            .uri("/api/auth/token")
            .header(header::HOST, "127.0.0.1:7433")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::json!({"token": token}).to_string()))
            .unwrap();
        let response = harness
            .unauthenticated_router()
            .oneshot(request)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 1024).await.unwrap()).unwrap();
        body["device_secret"].as_str().unwrap().to_string()
    }

    /// Both bootstrap and device credentials carry the promised 128 bits,
    /// rather than merely looking random as encoded strings.
    #[test]
    fn minted_credentials_decode_to_exactly_128_bits() {
        for _ in 0..32 {
            let secret = mint_secret().unwrap();
            let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(secret)
                .unwrap();
            assert_eq!(decoded.len(), 16);
        }
    }

    /// Two first-use readers on separate SQLite connections converge on the
    /// same committed singleton instead of each reporting its own candidate.
    #[tokio::test]
    async fn concurrent_first_mint_returns_one_committed_token() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("helm.db");
        let first = HelmStore::open(&path).await.unwrap();
        let second = HelmStore::open(&path).await.unwrap();
        let (first, second) = tokio::join!(show_token(&first), show_token(&second));
        assert_eq!(first.unwrap(), second.unwrap());
    }

    /// Rotation and exchange serialize at one immediate transaction. If
    /// validation pauses while holding that transaction, the later rotation
    /// deletes the admitted row rather than letting an old token mint after it.
    #[tokio::test]
    async fn rotation_cannot_be_overtaken_by_a_validated_exchange() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("helm.db");
        let exchanger = HelmStore::open(&path).await.unwrap();
        let rotator = HelmStore::open(&path).await.unwrap();
        let old = "AAAAAAAAAAAAAAAAAAAAAA".to_string();
        exchanger.web_token_or_insert(old.clone(), 1).await.unwrap();

        let (validated_tx, validated_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let exchange = tokio::spawn(async move {
            exchanger
                .exchange_device_session_with_pause(
                    old,
                    [9; 32],
                    2,
                    Box::new(move || {
                        let _ = validated_tx.send(());
                        release_rx.recv().unwrap();
                    }),
                )
                .await
        });
        validated_rx.await.unwrap();

        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let rotation = tokio::spawn(async move {
            let _ = started_tx.send(());
            rotator
                .rotate_web_token("BBBBBBBBBBBBBBBBBBBBBB".to_string(), 3)
                .await
        });
        started_rx.await.unwrap();
        release_tx.send(()).unwrap();
        assert!(exchange.await.unwrap().unwrap());
        rotation.await.unwrap().unwrap();

        let store = HelmStore::open(&path).await.unwrap();
        assert!(!store.has_device_session([9; 32]).await.unwrap());
    }

    /// Device rows stop at the documented cap and evict the oldest exact row
    /// when the next profile authenticates.
    #[tokio::test]
    async fn device_session_cap_is_exact_and_evicts_the_oldest() {
        let dir = tempfile::tempdir().unwrap();
        let store = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
        let token = "AAAAAAAAAAAAAAAAAAAAAA".to_string();
        store.web_token_or_insert(token.clone(), 1).await.unwrap();
        for index in 0..=crate::store::MAX_DEVICE_SESSIONS {
            assert!(
                store
                    .exchange_device_session(
                        token.clone(),
                        [u8::try_from(index).unwrap(); 32],
                        i64::try_from(index).unwrap(),
                    )
                    .await
                    .unwrap()
            );
        }
        assert_eq!(
            store.device_session_count().await.unwrap(),
            crate::store::MAX_DEVICE_SESSIONS
        );
        assert!(!store.has_device_session([0; 32]).await.unwrap());
        assert!(
            store
                .has_device_session([u8::try_from(crate::store::MAX_DEVICE_SESSIONS).unwrap(); 32])
                .await
                .unwrap()
        );
    }
}
