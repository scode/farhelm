//! `POST /api/clipboard` — the desktop webview's only working route to the
//! system clipboard.
//!
//! ## Why a helm endpoint for something browsers do natively
//!
//! SPEC.md's terminal-experience section promises that selecting text, and a
//! program's OSC 52 WRITE, land on the system clipboard. In the desktop app
//! they never can through the web platform: WKWebView does not treat a
//! custom-scheme page (`dioxus://index.html`) as a secure context, so
//! `navigator.clipboard` is `undefined` there — not denied, absent — and
//! every write the UI attempted disappeared into its own deliberate
//! `?.`-chains (diagnosed 2026-09-01/02: select-copy and agent OSC 52 both
//! dead on macOS, while paste, a native path, worked; wry registers the
//! scheme as secure on webkit2gtk but has no way to on WKWebView, so Linux
//! never showed it). So the write goes AROUND the web platform: the page
//! POSTs the text here over the same authenticated loopback HTTP the
//! client-log shim uses, and the embedding desktop shell — which registered
//! a [`crate::ClipboardSink`] at helm construction — writes the real
//! pasteboard natively.
//!
//! ## Only a desktop-embedded helm answers
//!
//! Without a registered sink the endpoint answers 404 (see
//! [`post_clipboard`]). That is a capability statement, not a stub: on a
//! server helm the requester is a browser somewhere else entirely, and
//! "write THIS machine's clipboard" is not a thing a remote page should be
//! able to mean. There is deliberately no flag that enables the sink on
//! `farhelm helm run` — only [`crate::run_embedded`] can provide one.
//!
//! ## Caps, and the silent-success contract
//!
//! One bounded text field per request ([`MAX_TEXT_BYTES`], with
//! [`MAX_BODY_BYTES`] sized above it for JSON overhead and enforced in
//! `lib.rs` before the handler allocates anything). Oversized text is the
//! caller's bug and 413s. An authenticated, parsed, in-bounds request is a
//! 204 whether the native write succeeded or not: SPEC.md makes clipboard
//! operations best-effort and silent on failure by contract, so a sink
//! failure is a `tracing` warn for the operator, never an error for a page
//! that has nothing useful to do with one. No accept-rate window, unlike
//! client-log: each write costs one bounded pasteboard update and emits no
//! per-request log line, so there is no amplification for a budget to take
//! away — a page spamming this endpoint churns the clipboard exactly as
//! fast as it could churn any clipboard API it held.

use crate::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use std::sync::Arc;

/// Most text one request may place on the clipboard. Sized comfortably above
/// the biggest OSC 52 payload the vendored xterm.js addon will assemble
/// (base64 in the escape decodes to well under 100 KiB) while still bounding
/// what an arbitrary page script can make the native side allocate.
pub(crate) const MAX_TEXT_BYTES: usize = 256 * 1024;

/// The route's whole-body cap (enforced as the axum body limit in
/// `lib.rs`): [`MAX_TEXT_BYTES`] of payload plus generous headroom for JSON
/// string escaping — every clipboard byte can cost up to six (`\u00XX`) on
/// the wire — and the envelope.
pub(crate) const MAX_BODY_BYTES: usize = 6 * MAX_TEXT_BYTES + 1024;

/// One clipboard write. `deny_unknown_fields` for the same reason
/// client-log's request has it: a misspelled field silently ignored is a
/// write that "succeeds" while carrying nothing the caller meant.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClipboardRequest {
    text: String,
}

/// Land one webview clipboard write on the native pasteboard, if this helm
/// has one to land it on.
///
/// Answers, in order of the checks that produce them: 404 when no
/// [`crate::ClipboardSink`] is registered (not a desktop-embedded helm),
/// 413 for text over [`MAX_TEXT_BYTES`], and otherwise 204 — including when
/// the native write itself failed, which is logged and deliberately not
/// reported (module docs: SPEC.md's best-effort clipboard contract).
/// Authentication happened before this ran (`require_device_session` is
/// layered in `lib.rs`).
pub(crate) async fn post_clipboard(
    State(state): State<Arc<AppState>>,
    axum::Json(request): axum::Json<ClipboardRequest>,
) -> impl IntoResponse {
    let Some(sink) = state.clipboard_sink.as_ref() else {
        return StatusCode::NOT_FOUND;
    };
    if request.text.len() > MAX_TEXT_BYTES {
        return StatusCode::PAYLOAD_TOO_LARGE;
    }
    // On the blocking pool, never on an async worker: the native writer
    // takes a process-wide mutex and talks to the display server, and a
    // burst of copies (or a program spamming OSC 52) against a slow
    // clipboard backend would otherwise park the workers that serve every
    // terminal and the session list (SPEC.md "Waiting between operations on
    // one host").
    let sink = Arc::clone(sink);
    let written = tokio::task::spawn_blocking(move || sink(&request.text))
        .await
        .unwrap_or_else(|join| Err(format!("the clipboard write task failed: {join}")));
    if let Err(why) = written {
        // The reason string comes from the native clipboard library, not a
        // peer, but it still crosses onto an operator's terminal — same
        // escaping discipline as every other logged string.
        tracing::warn!(
            reason = %crate::manager::peer_text_capped(&why, 512),
            "native clipboard write failed; dropped per the best-effort contract"
        );
    }
    StatusCode::NO_CONTENT
}

#[cfg(test)]
mod tests {
    use crate::rest_harness;
    use axum::http::StatusCode;
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;

    fn post(body: serde_json::Value) -> axum::http::Request<axum::body::Body> {
        axum::http::Request::builder()
            .method("POST")
            .uri("/api/clipboard")
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(body.to_string()))
            .unwrap()
    }

    /// The auth boundary must be exactly client-log's: no device session, no
    /// clipboard write — and the desktop webview can READ the structured
    /// refusal thanks to the CORS layering order.
    #[farhelm_testtrace::test]
    async fn unauthenticated_post_is_a_structured_401_readable_by_the_desktop_webview() {
        let harness = rest_harness::idle_helm().await;
        let mut request = post(serde_json::json!({"text": "hello"}));
        request
            .headers_mut()
            .insert("origin", "dioxus://index.html".parse().unwrap());
        let response = harness
            .unauthenticated_router()
            .oneshot(request)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers()["access-control-allow-origin"],
            "dioxus://index.html",
            "CORS must wrap authentication so the webview can read its 401"
        );
    }

    /// A helm with no registered sink — every server helm — must answer 404
    /// even to an authenticated caller: the endpoint's existence IS the
    /// desktop capability, and a remote page must never be able to write
    /// the helm machine's clipboard.
    #[farhelm_testtrace::test]
    async fn without_a_sink_an_authenticated_write_is_404() {
        let harness = rest_harness::idle_helm().await;
        let response = harness
            .router()
            .oneshot(post(serde_json::json!({"text": "hello"})))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    /// The happy path end to end: an authenticated write on a sink-bearing
    /// helm reaches the sink byte-for-byte and answers 204. Asserted through
    /// an injected sink because a 204 alone stays green while the pipeline
    /// discards everything.
    #[farhelm_testtrace::test]
    async fn an_authenticated_write_reaches_the_sink_and_answers_204() {
        let written: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_written = Arc::clone(&written);
        let harness = rest_harness::idle_helm_with_clipboard_sink(Arc::new(move |text: &str| {
            sink_written.lock().unwrap().push(text.to_string());
            Ok(())
        }))
        .await;
        let response = harness
            .router()
            .oneshot(post(serde_json::json!({"text": "copied 19 chars"})))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            *written.lock().unwrap(),
            vec!["copied 19 chars".to_string()]
        );
    }

    /// A slow native clipboard write never occupies an async worker: other
    /// requests are answered while the write is still blocked.
    ///
    /// Why it matters: the desktop sink blocks on a process-wide mutex and
    /// the display server, and running it on the async worker let a burst of
    /// copies freeze terminals and the session list (SPEC.md "Waiting between
    /// operations on one host"). Specified, on the test's single-threaded
    /// runtime: while one write is blocked inside the sink, a second request
    /// completes before that write returns.
    #[farhelm_testtrace::test]
    async fn a_blocked_native_write_does_not_stall_other_requests() {
        let (entered_tx, entered_rx) = std::sync::mpsc::channel::<()>();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let entered_tx = Mutex::new(entered_tx);
        let release_rx = Mutex::new(release_rx);
        let returned = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let sink_returned = Arc::clone(&returned);
        let harness = rest_harness::idle_helm_with_clipboard_sink(Arc::new(move |_: &str| {
            let _ = entered_tx.lock().unwrap().send(());
            // Bounded so a regression that blocks the runtime cannot hang
            // the test forever; the assertion below still fails it.
            let _ = release_rx
                .lock()
                .unwrap()
                .recv_timeout(std::time::Duration::from_secs(10));
            sink_returned.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }))
        .await;
        let router = harness.router();
        let first = tokio::spawn(
            router
                .clone()
                .oneshot(post(serde_json::json!({"text": "slow"}))),
        );
        tokio::task::spawn_blocking(move || {
            entered_rx
                .recv_timeout(std::time::Duration::from_secs(10))
                .expect("the first write reaches the sink")
        })
        .await
        .expect("wait for the sink");

        let oversized = "x".repeat(super::MAX_TEXT_BYTES + 1);
        let second = router
            .oneshot(post(serde_json::json!({"text": oversized})))
            .await
            .unwrap();
        assert_eq!(second.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert!(
            !returned.load(std::sync::atomic::Ordering::SeqCst),
            "the second request must complete while the first write is still blocked"
        );

        release_tx.send(()).expect("release the blocked write");
        let first = first.await.expect("first request task").unwrap();
        assert_eq!(first.status(), StatusCode::NO_CONTENT);
    }

    /// A failing native write is still a 204 — SPEC.md's best-effort
    /// clipboard contract — and must not panic, error, or leak the reason to
    /// the caller.
    #[farhelm_testtrace::test]
    async fn a_failing_sink_is_still_a_silent_204() {
        let harness = rest_harness::idle_helm_with_clipboard_sink(Arc::new(|_: &str| {
            Err("pasteboard said no".to_string())
        }))
        .await;
        let response = harness
            .router()
            .oneshot(post(serde_json::json!({"text": "hello"})))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    /// Text over the cap is the caller's bug and gets told so; the sink must
    /// never see it.
    #[farhelm_testtrace::test]
    async fn oversized_text_is_413_and_never_reaches_the_sink() {
        let written: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_written = Arc::clone(&written);
        let harness = rest_harness::idle_helm_with_clipboard_sink(Arc::new(move |text: &str| {
            sink_written.lock().unwrap().push(text.to_string());
            Ok(())
        }))
        .await;
        let oversized = "x".repeat(super::MAX_TEXT_BYTES + 1);
        let response = harness
            .router()
            .oneshot(post(serde_json::json!({"text": oversized})))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert!(written.lock().unwrap().is_empty());
    }

    /// An unknown field is refused rather than ignored — the
    /// `deny_unknown_fields` contract the request struct documents.
    #[farhelm_testtrace::test]
    async fn unknown_fields_are_refused() {
        let harness = rest_harness::idle_helm_with_clipboard_sink(Arc::new(|_: &str| Ok(()))).await;
        let response = harness
            .router()
            .oneshot(post(serde_json::json!({"text": "hello", "selection": "c"})))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
}
