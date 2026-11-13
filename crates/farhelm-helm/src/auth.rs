//! Device authentication at the helm's HTTP boundary.
//!
//! Every protected API method, both WebSocket handshakes included, needs a
//! device credential. The helm mints those credentials in memory for one
//! process lifetime and hands them only to callers inside that process: the
//! desktop app that embeds the helm (one per launch, see
//! [`crate::EmbeddedReady`]) and, for tests, a standalone helm started with
//! the hidden `--test-credential-file` flag. Only each secret's SHA-256 digest
//! is kept, and nothing is written to `helm.db`, so a credential dies with the
//! process and no HTTP request can obtain one.
//!
//! NOTE: There is no browser sign-in. The browser UI, its bootstrap token and
//! the device rows a token exchange created were removed when the GUI moved to
//! a native desktop app. Older helms left a `web_token` row and
//! `device_sessions` rows in `helm.db`; this module never reads them, so they
//! authenticate nothing. The tables stay only because dropping them would be a
//! schema migration.
//!
//! REST carries the credential in `Authorization: Bearer`, while WebSockets
//! offer it as a subprotocol, because a WebSocket client in a web view cannot
//! set an `Authorization` header on the upgrade.

use anyhow::Context;
use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use base64::Engine;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use subtle::ConstantTimeEq;

use crate::AppState;

/// Application protocol every authenticated WebSocket offers.
///
/// The device secret is a second offered protocol. Selecting this stable
/// value satisfies the WebSocket negotiation contract without reflecting a
/// credential in the upgrade response.
pub(crate) const WS_PROTOCOL: &str = "farhelm";

/// Prefix separating the application protocol from the credential-bearing
/// protocol token.
const WS_DEVICE_PREFIX: &str = "farhelm-device-";

/// Structured marker emitted only by the device-authentication boundary.
/// Shared with the UI, which branches on it, through `farhelm-proto`.
pub(crate) const AUTH_REQUIRED_CODE: &str = farhelm_proto::http::AUTH_REQUIRED_CODE;

/// The credentials this helm process minted, as digests.
///
/// They live exactly as long as the process, so nothing about them needs
/// persisting, revoking or evicting. A credential from another helm process,
/// including an earlier launch on the same state directory, is refused.
#[derive(Clone, Default)]
pub(crate) struct AuthState {
    devices: Arc<std::sync::RwLock<Vec<[u8; 32]>>>,
}

impl AuthState {
    pub(crate) fn new() -> AuthState {
        AuthState::default()
    }

    /// Mint a device credential for a caller inside this process.
    ///
    /// The callers are [`crate::run_embedded`] (for the desktop app that owns
    /// the helm) and a standalone helm given `--test-credential-file`, both
    /// before the helm announces readiness. There is deliberately no HTTP route
    /// to this, and any new caller needs the same scrutiny: no test can catch a
    /// route that leaks a credential.
    pub(crate) fn mint_device(&self) -> anyhow::Result<String> {
        let secret = mint_secret()?;
        self.devices
            .write()
            .expect("device list poisoned")
            .push(digest(secret.as_bytes()));
        Ok(secret)
    }

    /// Whether `secret` is one this process minted.
    ///
    /// Constant-time per entry and visits every entry, so timing says nothing
    /// about which credential (if any) a guess came close to.
    fn accepts_device(&self, secret: &str) -> bool {
        let supplied = digest(secret.as_bytes());
        let found = self
            .devices
            .read()
            .expect("device list poisoned")
            .iter()
            .fold(subtle::Choice::from(0), |found, known| {
                found | known.ct_eq(&supplied)
            });
        bool::from(found)
    }
}

/// Require an explicit device secret before any protected REST handler or
/// WebSocket upgrade is reached.
pub(crate) async fn require_device_session(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let websocket_request = request
        .headers()
        .get(header::UPGRADE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("websocket"));
    let accepted = if websocket_request {
        websocket_device_secret(request.headers())
            .is_some_and(|secret| state.auth.accepts_device(&secret))
    } else {
        authorization_device_secret(request.headers())
            .is_some_and(|secret| state.auth.accepts_device(secret))
    };
    if accepted {
        next.run(request).await
    } else {
        unauthenticated()
    }
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
pub(crate) fn websocket_device_secret(headers: &axum::http::HeaderMap) -> Option<String> {
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

/// Fixed-size digest of a credential, the only form this process keeps.
fn digest(value: &[u8]) -> [u8; 32] {
    Sha256::digest(value).into()
}

/// Mint a URL-safe 128-bit credential without padding or punctuation, which
/// HTTP and WebSocket protocol tokens can carry without escaping.
fn mint_secret() -> anyhow::Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).context("reading operating-system randomness")?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

fn unauthenticated() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::CONTENT_TYPE, "application/json")],
        format!(r#"{{"error":"unauthenticated","code":"{AUTH_REQUIRED_CODE}"}}"#),
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
    #[farhelm_testtrace::test]
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
    /// decorates that refusal so a client can tell version skew from a
    /// missing credential.
    #[farhelm_testtrace::test]
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
    /// opening and then emitting a close code the client has to reinterpret.
    #[farhelm_testtrace::test]
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

    /// Only credentials minted by this process authenticate. One minted by
    /// another helm process, an earlier launch on the same state directory
    /// included, must be refused: that is what lets the desktop app treat its
    /// credentials as worthless once it exits.
    #[farhelm_testtrace::test]
    fn credentials_are_scoped_to_the_minting_process() {
        let this_process = AuthState::new();
        let other_process = AuthState::new();
        let ours = this_process.mint_device().unwrap();
        let theirs = other_process.mint_device().unwrap();
        assert!(this_process.accepts_device(&ours));
        assert!(!this_process.accepts_device(&theirs));
        assert!(!this_process.accepts_device(""));
    }

    /// A browser credential an older helm stored in `helm.db` no longer
    /// authenticates anything.
    ///
    /// Why it matters: state directories that ran an older helm still hold
    /// `device_sessions` rows from browser sign-in, and the table stays
    /// because dropping it would be a schema migration. If anything still read
    /// those rows, a leaked browser credential would outlive the feature.
    /// Spec: a request carrying a secret whose digest is a stored row is
    /// refused with 401, while the identical request carrying a credential
    /// this process minted succeeds. The second half is the control: without
    /// it, a 401 could come from the request's shape (its host, route or
    /// header form) rather than from the ignored row.
    #[farhelm_testtrace::test]
    async fn a_device_row_left_by_an_older_helm_does_not_authenticate() {
        let harness = rest_harness::idle_helm().await;
        let stored = "AAAAAAAAAAAAAAAAAAAAAA";
        harness
            .store
            .conn()
            .lock()
            .execute(
                "INSERT INTO device_sessions (cookie_hash, created_at) VALUES (?1, ?2)",
                rusqlite::params![digest(stored.as_bytes()).to_vec(), 1],
            )
            .expect("fixture premise: the old table still accepts a row");
        // One request shape for both credentials, so only the secret differs.
        let list_sessions = |secret: &str| {
            Request::builder()
                .uri("/api/sessions")
                .header(header::HOST, "127.0.0.1:7433")
                .header(header::AUTHORIZATION, format!("Bearer {secret}"))
                .body(Body::empty())
                .unwrap()
        };
        let response = harness
            .unauthenticated_router()
            .oneshot(list_sessions(stored))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let minted = harness.state.auth.mint_device().unwrap();
        let response = harness
            .unauthenticated_router()
            .oneshot(list_sessions(&minted))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "control: the same request with a credential this helm minted must authenticate"
        );
    }

    /// Device credentials carry the promised 128 bits, rather than merely
    /// looking random as encoded strings.
    #[farhelm_testtrace::test]
    fn minted_credentials_decode_to_exactly_128_bits() {
        for _ in 0..32 {
            let secret = mint_secret().unwrap();
            let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(secret)
                .unwrap();
            assert_eq!(decoded.len(), 16);
        }
    }
}
