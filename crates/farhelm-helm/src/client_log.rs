//! `POST /api/client-log` — the receiving end of the desktop webview's
//! console shim (PLAN_desktop_web_bug_triage.md's helm-endpoint step).
//!
//! The desktop webview is the one layer of this stack that fails invisibly:
//! native Rust has `tracing`, and a browser tab has devtools, but a wry
//! webview today has neither — a thrown exception or a bridge death just
//! disappears. This endpoint is where a later PR's JS shim lands what it
//! saw, so it becomes an ordinary, greppable `tracing` event instead.
//!
//! Transport is deliberately loopback HTTP to this already-running helm,
//! never the eval IPC channel the desktop shell also uses to reach the
//! webview. That channel dying (the MT-5 class of bug) is exactly the
//! failure this pipeline exists to report, so evidence about it cannot be
//! made to travel over the thing that just broke.
//!
//! ## Caps are a chatty-failure backstop, not a UX feature
//!
//! The JS shim caps itself before sending, but a hostile or merely buggy
//! page cannot be trusted to keep that promise, so every cap here is
//! re-enforced server-side: the route's own body limit
//! ([`MAX_BODY_BYTES`], applied in `lib.rs` — the JSON is fully allocated
//! before any handler code runs, so a cap inside the handler alone would
//! bound the log but not the parse), at most [`MAX_ENTRIES_PER_REQUEST`]
//! entries per call, per-field byte bounds ([`MAX_MESSAGE_BYTES`],
//! [`MAX_SOURCE_BYTES`]), and at most [`MAX_ACCEPTED_PER_MINUTE`] accepted
//! entries per fixed window. Dropping is reported, but the report itself is
//! bounded too — at most one warn per window (see [`RateWindow`]) — because
//! a per-drop warn would hand the flooding loop the very amplifier the caps
//! exist to take away (PLAN_desktop_web_bug_triage.md's dropped-count
//! decision).
//!
//! ## What reaches `tracing`, exactly
//!
//! Two request fields and nothing else: `message` and the optional `source`
//! label, each bounded and control-escaped through
//! [`crate::manager::peer_text_capped`] — the same treatment every other
//! peer-supplied string gets before an operator's terminal, because a
//! newline or escape byte in either could forge log lines or repaint the
//! console reading them. Headers, the device session, and anything else
//! about the request never reach the log beyond what
//! [`crate::auth::require_device_session`] already established by admitting
//! the request at all.
//!
//! ## 204 for every authenticated, parsed request
//!
//! Once a request passes auth and JSON parsing, the answer is 204 whether
//! every entry was kept or every one was dropped by a cap. The whole point
//! of this pipe is capturing a failure the webview cannot otherwise report;
//! a client that has to handle its OWN logging endpoint erroring has one
//! more failure mode to lose evidence to. Auth failures (401) and malformed
//! bodies (4xx) still fail normally — they are the caller's bugs, not its
//! evidence.

use crate::AppState;
use axum::extract::State;
use axum::response::IntoResponse;
use serde::Deserialize;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Surplus entries beyond this many in one request are dropped, not queued
/// or split across calls — the JS shim already batches to stay under this,
/// so hitting it at all means the client-side cap was bypassed or is buggy.
pub(crate) const MAX_ENTRIES_PER_REQUEST: usize = 32;

/// Byte bound on one `message` before it reaches `tracing` — roomy enough
/// for a JS stack trace, far below anything that matters as log retention.
/// Enforced (with escaping and a visible `(truncated)` marker) by
/// [`crate::manager::peer_text_capped`].
pub(crate) const MAX_MESSAGE_BYTES: usize = 2048;

/// Byte bound on one `source` label. Much smaller than the message bound
/// on purpose: a source is a file path or a handler name
/// (`window.onerror`, `unhandledrejection`), and anything bigger is the
/// caller misusing the field as a second message — which, unbounded, would
/// let one entry smuggle the whole request body past the message cap.
pub(crate) const MAX_SOURCE_BYTES: usize = 256;

/// How many entries this whole helm accepts per fixed window, across every
/// request and every caller. One shared budget rather than one per device
/// session: the failure this guards against is a single bricked webview
/// retrying itself into a flood, and a per-session budget would not slow
/// that caller down at all.
pub(crate) const MAX_ACCEPTED_PER_MINUTE: usize = 60;

/// The route-level body limit `lib.rs` applies to `POST /api/client-log`,
/// sized from the caps above: 32 entries × (2 KiB message + 256 B source +
/// JSON overhead) fits comfortably, and anything bigger is a request whose
/// surplus this endpoint would drop anyway — better refused before the
/// parse allocates it than truncated after.
pub(crate) const MAX_BODY_BYTES: usize = 128 * 1024;

/// The fixed window [`RateWindow`] counts against. A window boundary
/// permits one boundary burst of up to twice the per-window budget (60
/// just before the line, 60 just after); that is the documented, accepted
/// cost of keeping the accounting a single compare under a shared lock —
/// this cap is a backstop against a runaway loop, not a fairness contract.
const RATE_WINDOW: Duration = Duration::from_secs(60);

/// One console-shim entry, matching what the browser's `console.error` /
/// `console.warn` and its `window.onerror` handler observe.
///
/// `deny_unknown_fields`: this is a small control message with a pinned
/// shape (this crate's convention for request bodies whose fields all
/// decide something — see `hosts::HostSpec`'s docs), and a typo silently
/// ignored here would mean an entry the shim thought it sent never reaches
/// the log at all.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClientLogEntry {
    level: ClientLogLevel,
    message: String,
    /// Free-text origin the shim attaches (a source file, `window.onerror`,
    /// `unhandledrejection`, …). Optional because not every capture site can
    /// name one.
    source: Option<String>,
}

/// The console method an entry came from. Only these two are forwarded —
/// PLAN_desktop_web_bug_triage.md limits the pipe to the failure signal;
/// `console.log`/`.info` are ordinary application chatter that would spend
/// the small shared budget on noise.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ClientLogLevel {
    Error,
    Warn,
}

/// The body of `POST /api/client-log`: an `entries`-wrapped batch, not a
/// bare top-level array. The wrapper is deliberate — it is what lets
/// `deny_unknown_fields` catch a misspelled control field at the top level
/// today and leaves room to version the envelope later without breaking
/// the array shape; the shim must send exactly
/// `{"entries": [{level, message, source?}, ...]}`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClientLogRequest {
    entries: Vec<ClientLogEntry>,
}

/// A fixed-window counter admitting at most [`MAX_ACCEPTED_PER_MINUTE`]
/// entries per [`RATE_WINDOW`], shared by every `/api/client-log` request
/// this helm serves — and the once-per-window bound on drop REPORTING,
/// which needs the same window state to be meaningful.
///
/// Fixed-window rather than sliding or a leaky bucket: the cap is a
/// backstop against a chatty failure loop, not a fairness guarantee — a
/// burst landing right at a window boundary costs the log at most one
/// extra window's worth of entries (see [`RATE_WINDOW`]), which is a fine
/// trade for keeping the accounting this cheap and this easy to reason
/// about under a shared lock.
pub(crate) struct RateWindow {
    window_start: Instant,
    accepted: usize,
    /// Whether this window has already produced its one dropped-entries
    /// warn. Without this, every over-budget request would emit its own
    /// warn line — handing a retry loop an unbounded log amplifier through
    /// the very mechanism meant to stop it.
    drop_warned: bool,
}

/// What one request's trip through [`RateWindow::account`] decided.
pub(crate) struct Admission {
    /// How many of the request's (already entry-capped) entries may be
    /// emitted. The rest are dropped.
    admitted: usize,
    /// `Some(total_dropped)` exactly when this request should emit the
    /// window's single drop warn — i.e. it is the first request in this
    /// window to drop anything. `None` on every later drop in the same
    /// window; the suppression is deliberate and documented in the warn
    /// text itself.
    warn_dropped: Option<usize>,
}

impl RateWindow {
    /// A fresh window starting now, with nothing yet admitted or warned.
    pub(crate) fn new(now: Instant) -> RateWindow {
        RateWindow {
            window_start: now,
            accepted: 0,
            drop_warned: false,
        }
    }

    /// Account one request: admit what the window's budget allows of
    /// `requested`, fold in `pre_dropped` (entries the per-request cap
    /// already discarded before the budget was consulted), and decide
    /// whether THIS request emits the window's one drop warn.
    ///
    /// Deterministic in `now` and the counter's own fields — no
    /// `Instant::now()` inside — so window-rollover, refill, and
    /// warn-once behavior are unit-testable by constructing successive
    /// `Instant`s via arithmetic rather than by actually waiting (this
    /// repo's rule against sleep-based tests). A `now` at or past the
    /// window's end resets the window: budget refilled, warn re-armed.
    fn account(&mut self, now: Instant, requested: usize, pre_dropped: usize) -> Admission {
        if now.duration_since(self.window_start) >= RATE_WINDOW {
            self.window_start = now;
            self.accepted = 0;
            self.drop_warned = false;
        }
        let available = MAX_ACCEPTED_PER_MINUTE.saturating_sub(self.accepted);
        let admitted = requested.min(available);
        self.accepted += admitted;
        let dropped = pre_dropped + (requested - admitted);
        let warn_dropped = (dropped > 0 && !self.drop_warned).then(|| {
            self.drop_warned = true;
            dropped
        });
        Admission {
            admitted,
            warn_dropped,
        }
    }
}

/// Keep at most `max` entries in order, reporting how many trailing entries
/// were dropped as surplus.
///
/// A pure function so the entry-count cap is unit-testable directly, rather
/// than only through the axum handler.
fn cap_entry_count(mut entries: Vec<ClientLogEntry>, max: usize) -> (Vec<ClientLogEntry>, usize) {
    let dropped = entries.len().saturating_sub(max);
    entries.truncate(max);
    (entries, dropped)
}

/// Turn one admitted entry into exactly one `tracing` event under the
/// load-bearing `webview_console` target — the string the triage doc tells
/// people to grep the native log for, so it must never change.
///
/// Both request strings pass through [`crate::manager::peer_text_capped`]
/// on their way out: bounded ([`MAX_MESSAGE_BYTES`] / [`MAX_SOURCE_BYTES`]),
/// control-escaped, truncation marked in-line with `(truncated)` — the same
/// never-trust-peer-text convention the connection manager applies before
/// anything reaches an operator's terminal.
///
/// The four call sites (level × source presence) exist because `tracing`'s
/// macros need their level and field set at compile time; there is no way
/// to parameterize "which macro" or "which field list" over a runtime value.
fn emit(entry: ClientLogEntry) {
    let message = crate::manager::peer_text_capped(&entry.message, MAX_MESSAGE_BYTES);
    let source = entry
        .source
        .map(|source| crate::manager::peer_text_capped(&source, MAX_SOURCE_BYTES));
    match (entry.level, source) {
        (ClientLogLevel::Error, Some(source)) => {
            tracing::error!(target: "webview_console", source = %source, "{message}")
        }
        (ClientLogLevel::Error, None) => {
            tracing::error!(target: "webview_console", "{message}")
        }
        (ClientLogLevel::Warn, Some(source)) => {
            tracing::warn!(target: "webview_console", source = %source, "{message}")
        }
        (ClientLogLevel::Warn, None) => {
            tracing::warn!(target: "webview_console", "{message}")
        }
    }
}

/// `POST /api/client-log` — accept a batch of webview console entries,
/// cap and escape them, and forward what survives to `tracing`.
///
/// Every request that passed auth and JSON parsing answers 204, including
/// when every entry was dropped: see the module docs for why a client that
/// logs its own failures must never be made to handle THIS endpoint
/// failing. Caps apply in order — the per-request entry-count cap first,
/// then the shared per-window budget against what remains — and dropping
/// is announced by at most one warn per window ([`RateWindow`]'s
/// accounting), so the drop report can never itself become the flood.
pub(crate) async fn post_client_log(
    State(state): State<Arc<AppState>>,
    axum::Json(request): axum::Json<ClientLogRequest>,
) -> impl IntoResponse {
    let (kept, count_capped) = cap_entry_count(request.entries, MAX_ENTRIES_PER_REQUEST);
    let admission = {
        // A short critical section: the lock covers only the counter
        // arithmetic, never the tracing emission below.
        let mut window = state
            .client_log_rate
            .lock()
            .expect("client-log rate window mutex poisoned");
        window.account(Instant::now(), kept.len(), count_capped)
    };

    for entry in kept.into_iter().take(admission.admitted) {
        emit(entry);
    }

    if let Some(dropped) = admission.warn_dropped {
        tracing::warn!(
            target: "webview_console",
            dropped,
            "client-log entries dropped by server-side caps; further drops this window \
             are counted but not logged individually"
        );
    }

    axum::http::StatusCode::NO_CONTENT
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(message: &str) -> ClientLogEntry {
        ClientLogEntry {
            level: ClientLogLevel::Warn,
            message: message.to_string(),
            source: None,
        }
    }

    /// Below and at the per-request cap nothing is dropped and order is
    /// preserved — the ordinary cases every real batch from the shim hits.
    /// Order is asserted on the retained messages, not just the length,
    /// because the drop contract is "the FIRST N are kept".
    #[test]
    fn cap_entry_count_keeps_everything_at_or_under_the_cap_in_order() {
        let below: Vec<_> = (0..3).map(|i| entry(&format!("m{i}"))).collect();
        let (kept, dropped) = cap_entry_count(below, super::MAX_ENTRIES_PER_REQUEST);
        assert_eq!(dropped, 0);
        assert_eq!(
            kept.iter().map(|e| e.message.as_str()).collect::<Vec<_>>(),
            ["m0", "m1", "m2"]
        );

        let at: Vec<_> = (0..super::MAX_ENTRIES_PER_REQUEST)
            .map(|i| entry(&format!("m{i}")))
            .collect();
        let (kept, dropped) = cap_entry_count(at, super::MAX_ENTRIES_PER_REQUEST);
        assert_eq!(kept.len(), super::MAX_ENTRIES_PER_REQUEST);
        assert_eq!(dropped, 0);
    }

    /// Surplus beyond the cap is dropped from the TAIL, and the count of
    /// what was dropped is exact — both halves of the contract the drop
    /// warn depends on.
    #[test]
    fn cap_entry_count_drops_the_surplus_and_counts_it() {
        let entries: Vec<_> = (0..super::MAX_ENTRIES_PER_REQUEST + 5)
            .map(|i| entry(&format!("m{i}")))
            .collect();
        let (kept, dropped) = cap_entry_count(entries, super::MAX_ENTRIES_PER_REQUEST);
        assert_eq!(kept.len(), super::MAX_ENTRIES_PER_REQUEST);
        assert_eq!(dropped, 5);
        assert_eq!(
            kept.last().unwrap().message,
            "m31",
            "the first N entries must be kept, in order"
        );
    }

    /// Requests within the per-window budget are admitted in full, the
    /// budget is charged, and a request that would exceed what remains is
    /// PARTIALLY admitted — an entry that fits must never be dropped just
    /// because it arrived beside entries that do not.
    #[test]
    fn rate_window_admits_within_budget_and_partially_at_the_edge() {
        let start = Instant::now();
        let mut window = RateWindow::new(start);
        assert_eq!(window.account(start, 10, 0).admitted, 10);
        let at_edge = window.account(start, super::MAX_ACCEPTED_PER_MINUTE, 0);
        assert_eq!(
            at_edge.admitted,
            super::MAX_ACCEPTED_PER_MINUTE - 10,
            "only the unspent remainder of the window's budget may be admitted"
        );
        assert_eq!(window.account(start, 1, 0).admitted, 0, "budget spent");
    }

    /// Once `RATE_WINDOW` has elapsed, the counter resets to a fresh
    /// window with the full budget restored — proven with `Instant`
    /// arithmetic rather than a real sleep, per this repo's rule against
    /// wall-clock-dependent tests.
    #[test]
    fn rate_window_refills_after_the_window_elapses() {
        let start = Instant::now();
        let mut window = RateWindow::new(start);
        assert_eq!(
            window
                .account(start, super::MAX_ACCEPTED_PER_MINUTE, 0)
                .admitted,
            super::MAX_ACCEPTED_PER_MINUTE
        );

        let just_before = start + super::RATE_WINDOW - Duration::from_millis(1);
        assert_eq!(
            window.account(just_before, 1, 0).admitted,
            0,
            "one millisecond short of the boundary must not refill early"
        );

        let after = start + super::RATE_WINDOW;
        assert_eq!(
            window
                .account(after, super::MAX_ACCEPTED_PER_MINUTE, 0)
                .admitted,
            super::MAX_ACCEPTED_PER_MINUTE,
            "a fresh window must restore the full budget"
        );
    }

    /// The drop warn fires exactly once per window — on the first request
    /// that drops anything, with the correct combined count — and is
    /// re-armed by a window rollover. This is the bound that keeps the
    /// drop REPORT from becoming the flood the caps exist to stop.
    #[test]
    fn rate_window_warns_about_drops_once_per_window() {
        let start = Instant::now();
        let mut window = RateWindow::new(start);

        // Nothing dropped: no warn.
        assert!(window.account(start, 10, 0).warn_dropped.is_none());

        // First dropping request: warn, with pre-dropped and rate-dropped
        // folded together (50 remaining budget, 55 requested, 7 already
        // cut by the entry cap → 5 + 7 = 12).
        let first_drop = window.account(start, 55, 7);
        assert_eq!(first_drop.admitted, 50);
        assert_eq!(first_drop.warn_dropped, Some(12));

        // Later drops in the same window: counted implicitly, never warned.
        assert!(window.account(start, 20, 3).warn_dropped.is_none());

        // A new window re-arms the warn.
        let next = start + super::RATE_WINDOW;
        assert!(
            window
                .account(next, super::MAX_ACCEPTED_PER_MINUTE + 1, 0)
                .warn_dropped
                .is_some(),
            "the rollover must re-arm the once-per-window warn"
        );
    }

    /// The axum layer's own tests: the auth boundary this route must never
    /// weaken, the CORS preflight the webview's fetch depends on, and the
    /// emission contract (target, level, escaping, capping) asserted
    /// through the crate's tracing capture. Unique message markers keep
    /// each test's events distinguishable in the shared capture buffer.
    mod router {
        use crate::rest_harness;
        use axum::http::StatusCode;
        use tower::ServiceExt;

        fn post(body: serde_json::Value) -> axum::http::Request<axum::body::Body> {
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/client-log")
                .header("host", "127.0.0.1:7433")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(body.to_string()))
                .unwrap()
        }

        /// Mandatory auth: an unauthenticated `POST /api/client-log` gets the
        /// same structured 401 every other protected route does, and — the
        /// part specific to this route's CORS layering — a desktop-webview
        /// origin can still READ that refusal rather than seeing an opaque
        /// fetch failure.
        #[tokio::test]
        async fn unauthenticated_post_is_a_structured_401_readable_by_the_desktop_webview() {
            let harness = rest_harness::idle_helm().await;
            let mut request = post(serde_json::json!({"entries": []}));
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
            let body = axum::body::to_bytes(response.into_body(), 1024)
                .await
                .unwrap();
            assert_eq!(
                body,
                r#"{"error":"unauthenticated","code":"device_auth_required"}"#.as_bytes(),
                "the same structured refusal body every protected route answers with"
            );
        }

        /// The browser sends `OPTIONS` before any cross-origin JSON POST; a
        /// preflight that 401s or 405s silently kills every console report.
        /// Unauthenticated on purpose — preflights carry no credentials —
        /// and mirrored from the attachment route's preflight contract.
        #[tokio::test]
        async fn preflight_answers_the_desktop_webview_origin_without_auth() {
            let harness = rest_harness::idle_helm().await;
            let request = axum::http::Request::builder()
                .method("OPTIONS")
                .uri("/api/client-log")
                .header("host", "127.0.0.1:7433")
                .header("origin", "dioxus://index.html")
                .header("access-control-request-method", "POST")
                .header(
                    "access-control-request-headers",
                    "authorization, content-type",
                )
                .body(axum::body::Body::empty())
                .unwrap();
            let response = harness
                .unauthenticated_router()
                .oneshot(request)
                .await
                .unwrap();
            assert!(
                response.status().is_success(),
                "the preflight must succeed without credentials: {}",
                response.status()
            );
            assert_eq!(
                response.headers()["access-control-allow-origin"],
                "dioxus://index.html"
            );
            let methods = response.headers()["access-control-allow-methods"]
                .to_str()
                .unwrap();
            assert!(methods.contains("POST"), "POST must be allowed: {methods}");
        }

        /// The endpoint's central promise: accepted entries become tracing
        /// events under the exact `webview_console` target, at the level
        /// the entry named, with the source label attached — asserted
        /// through the capture layer, because a 204 alone would stay green
        /// while the whole pipeline silently discarded everything.
        #[farhelm_testtrace::test]
        async fn accepted_entries_become_webview_console_tracing_events() {
            let captured = crate::test_capture::current();
            let harness = rest_harness::idle_helm().await;
            let marker = format!("emit-{}", uuid::Uuid::new_v4());
            let response = harness
                .router()
                .oneshot(post(serde_json::json!({
                    "entries": [
                        {"level": "error", "message": format!("{marker}-boom")},
                        {"level": "warn", "message": format!("{marker}-careful"), "source": "app.js"},
                    ]
                })))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NO_CONTENT);

            let events = crate::test_capture::matching(&captured, &marker);
            assert_eq!(events.len(), 2, "both entries must be emitted: {events:?}");
            let error = &events[0];
            assert_eq!(error.target, "webview_console");
            assert_eq!(error.level, "ERROR");
            assert!(error.message_contains("boom"));
            let warn = &events[1];
            assert_eq!(warn.target, "webview_console");
            assert_eq!(warn.level, "WARN");
            assert_eq!(
                warn.field("source"),
                Some("\"app.js\""),
                "the source label must ride along, peer-escaped"
            );
        }

        /// Control characters in request strings must never reach the log
        /// raw: a newline forges extra log lines, an escape byte can
        /// repaint the terminal reading the log. The peer-text escaping
        /// renders both inert.
        #[farhelm_testtrace::test]
        async fn control_characters_are_escaped_before_tracing() {
            let captured = crate::test_capture::current();
            let harness = rest_harness::idle_helm().await;
            let marker = format!("esc-{}", uuid::Uuid::new_v4());
            let response = harness
                .router()
                .oneshot(post(serde_json::json!({
                    "entries": [{
                        "level": "error",
                        "message": format!("{marker}\nFORGED LINE \u{1b}[2J"),
                        "source": "a\nb",
                    }]
                })))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NO_CONTENT);

            let events = crate::test_capture::matching(&captured, &marker);
            assert_eq!(events.len(), 1);
            let message = events[0].field("message").unwrap();
            assert!(
                !message.contains('\n') && !message.contains('\u{1b}'),
                "raw control bytes must not survive escaping: {message:?}"
            );
            assert!(
                message.contains("\\n"),
                "the newline must be visibly escaped, not deleted: {message:?}"
            );
            let source = events[0].field("source").unwrap();
            assert!(
                !source.contains('\n'),
                "source is peer text too: {source:?}"
            );
        }

        /// An oversized source label is truncated to its own small cap with
        /// the visible `(truncated)` marker — the field a caller could
        /// otherwise use to smuggle a body-sized payload past the message
        /// cap.
        #[farhelm_testtrace::test]
        async fn an_oversized_source_is_truncated_with_a_visible_marker() {
            let captured = crate::test_capture::current();
            let harness = rest_harness::idle_helm().await;
            let marker = format!("src-{}", uuid::Uuid::new_v4());
            let response = harness
                .router()
                .oneshot(post(serde_json::json!({
                    "entries": [{
                        "level": "warn",
                        "message": marker,
                        "source": "s".repeat(super::super::MAX_SOURCE_BYTES * 4),
                    }]
                })))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NO_CONTENT);

            let events = crate::test_capture::matching(&captured, &marker);
            assert_eq!(events.len(), 1);
            let source = events[0].field("source").unwrap();
            assert!(
                source.len() < super::super::MAX_SOURCE_BYTES * 2,
                "the emitted source must be bounded: {} bytes",
                source.len()
            );
            assert!(
                source.ends_with("(truncated)"),
                "truncation must be marked, not silent: {source:?}"
            );
        }

        /// Surplus entries beyond the per-request cap are dropped from the
        /// tail while the first 32 are still emitted, the response stays
        /// 204, and the drop is announced — the renamed, honest version of
        /// what an earlier draft mislabeled "capped down to nothing".
        #[farhelm_testtrace::test]
        async fn surplus_entries_are_dropped_and_announced_while_the_rest_emit() {
            let captured = crate::test_capture::current();
            let harness = rest_harness::idle_helm().await;
            let marker = format!("surplus-{}", uuid::Uuid::new_v4());
            let entries: Vec<_> = (0..super::super::MAX_ENTRIES_PER_REQUEST + 10)
                .map(|i| serde_json::json!({"level": "warn", "message": format!("{marker}-{i}")}))
                .collect();
            let response = harness
                .router()
                .oneshot(post(serde_json::json!({"entries": entries})))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NO_CONTENT);

            let events = crate::test_capture::matching(&captured, &marker);
            assert_eq!(
                events.len(),
                super::super::MAX_ENTRIES_PER_REQUEST,
                "exactly the first 32 entries must be emitted"
            );
            assert!(
                events[0].message_contains(&format!("{marker}-0")),
                "the kept entries are the FIRST ones"
            );
            let drops = crate::test_capture::matching(&captured, "dropped by server-side caps");
            assert!(
                drops.iter().any(|e| e.field("dropped") == Some("10")),
                "one warn must carry the exact dropped count: {drops:?}"
            );
        }

        /// Exhausting the shared per-window budget must still answer 204
        /// while emitting nothing — and must NOT add one warn per rejected
        /// request. This drives the same helm through three requests, so it
        /// also proves the budget is genuinely shared across requests.
        #[farhelm_testtrace::test]
        async fn a_fully_rate_capped_request_is_a_silent_204() {
            let captured = crate::test_capture::current();
            let harness = rest_harness::idle_helm().await;
            let marker = format!("exhaust-{}", uuid::Uuid::new_v4());
            let batch = |tag: &str| {
                let entries: Vec<_> = (0..super::super::MAX_ENTRIES_PER_REQUEST)
                    .map(|i| serde_json::json!({"level": "warn", "message": format!("{marker}-{tag}-{i}")}))
                    .collect();
                post(serde_json::json!({"entries": entries}))
            };
            // 32 + 32 = 64 requested against a budget of 60: the second
            // request drops 4 and emits the window's one warn.
            for tag in ["a", "b"] {
                let response = harness.router().oneshot(batch(tag)).await.unwrap();
                assert_eq!(response.status(), StatusCode::NO_CONTENT);
            }
            let warns_before =
                crate::test_capture::matching(&captured, "dropped by server-side caps").len();

            // The budget is now spent: a third request emits nothing new —
            // no entries AND no additional warn.
            let response = harness.router().oneshot(batch("c")).await.unwrap();
            assert_eq!(
                response.status(),
                StatusCode::NO_CONTENT,
                "an entirely dropped request is still never a client-facing error"
            );
            assert_eq!(
                crate::test_capture::matching(&captured, &format!("{marker}-c")).len(),
                0,
                "nothing from the exhausted request may be emitted"
            );
            assert_eq!(
                crate::test_capture::matching(&captured, "dropped by server-side caps").len(),
                warns_before,
                "later drops in the same window must not add warn lines"
            );
        }

        /// An unknown field is refused rather than silently ignored at BOTH
        /// levels of the request shape — `deny_unknown_fields` sits on the
        /// envelope and on each entry, and either attribute could be
        /// dropped independently without the other test noticing.
        #[tokio::test]
        async fn unknown_fields_are_refused_at_both_levels() {
            let harness = rest_harness::idle_helm().await;
            let nested = post(serde_json::json!({
                "entries": [{"level": "error", "message": "boom", "extra": "nope"}]
            }));
            let response = harness.router().oneshot(nested).await.unwrap();
            assert!(response.status().is_client_error(), "{}", response.status());

            let top_level = post(serde_json::json!({"entries": [], "extra": "nope"}));
            let response = harness.router().oneshot(top_level).await.unwrap();
            assert!(response.status().is_client_error(), "{}", response.status());
        }

        /// Levels outside the deliberate error/warn pair are refused: the
        /// pipe carries the failure signal, and silently accepting `info`
        /// would let ordinary chatter spend the small shared budget.
        #[tokio::test]
        async fn unsupported_console_levels_are_refused() {
            let harness = rest_harness::idle_helm().await;
            let response = harness
                .router()
                .oneshot(post(serde_json::json!({
                    "entries": [{"level": "info", "message": "chatter"}]
                })))
                .await
                .unwrap();
            assert!(
                response.status().is_client_error(),
                "an unsupported level must be refused: {}",
                response.status()
            );
        }

        /// A body past the route's own limit is refused before the JSON is
        /// parsed — the cap that bounds the endpoint's WORK, where the
        /// entry cap only bounds its output.
        #[tokio::test]
        async fn an_oversized_body_is_refused_outright() {
            let harness = rest_harness::idle_helm().await;
            let huge = "x".repeat(super::super::MAX_BODY_BYTES + 1024);
            let response = harness
                .router()
                .oneshot(post(serde_json::json!({
                    "entries": [{"level": "error", "message": huge}]
                })))
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::PAYLOAD_TOO_LARGE,
                "the route body limit must refuse before parsing"
            );
        }

        /// Two helm instances keep independent budgets: exhausting one
        /// must not silence the other. Pins `AppState::client_log_rate`
        /// being per-instance rather than process-global — an embedded
        /// second helm's diagnostics must not vanish because an unrelated
        /// window is flooding.
        #[farhelm_testtrace::test]
        async fn rate_budgets_are_per_helm_instance() {
            let captured = crate::test_capture::current();
            let first = rest_harness::idle_helm().await;
            let second = rest_harness::idle_helm().await;
            let marker = format!("iso-{}", uuid::Uuid::new_v4());

            // Exhaust the first helm's budget (32 + 32 > 60).
            for _ in 0..2 {
                let entries: Vec<_> = (0..super::super::MAX_ENTRIES_PER_REQUEST)
                    .map(|i| serde_json::json!({"level": "warn", "message": format!("{marker}-flood-{i}")}))
                    .collect();
                let response = first
                    .router()
                    .oneshot(post(serde_json::json!({"entries": entries})))
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::NO_CONTENT);
            }

            // The second helm still emits.
            let response = second
                .router()
                .oneshot(post(serde_json::json!({
                    "entries": [{"level": "error", "message": format!("{marker}-independent")}]
                })))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NO_CONTENT);
            assert_eq!(
                crate::test_capture::matching(&captured, &format!("{marker}-independent")).len(),
                1,
                "an unrelated helm's flood must not spend this helm's budget"
            );
        }
    }
}
