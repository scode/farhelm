//! The layers wrapped around the helm's routes: who is allowed to ask, who
//! owns the work, and what every answer carries.
//!
//! These concerns live here rather than inside the handlers, and each is
//! here for the same reason: within whatever scope it covers, it has to
//! hold for EVERY response, including the ones that fail. A guard a handler
//! forgot to call is a hole; a header a handler forgot to set on its error
//! path is a silent failure. Layers are the only construction that cannot
//! be forgotten response by response.
//!
//! The scopes differ, and deliberately. The origin guard and the build
//! stamp wrap the whole router, because both answer questions that have
//! nothing to do with which route was asked for. Request ownership covers
//! the authenticated routes except for an explicitly cancellable sub-router:
//! an action must finish even if the client leaves, while reads and
//! cancellation-sensitive transfers must retain their connection's lifetime.
//!
//! ## The loopback guard is a real security boundary
//!
//! Binding to 127.0.0.1 is not by itself a defense against the user's OWN
//! browser: a hostile page can rebind DNS to loopback and reach this helm
//! as if it were a local client, and WebSocket upgrades are not CORS-gated
//! at all. `require_loopback_origin` is what actually stands in the way: no
//! browser page is a client of this helm, so it refuses every request a
//! browser page sends. `origin_is_allowed` is its decision as a pure
//! function so the whole matrix can be pinned by unit tests rather than by
//! whichever branches an integration test happens to reach.
//!
//! ## The build stamp is how a stale client finds out
//!
//! `stamp_build` rides every response, successes and failures alike,
//! because a client left running across a helm upgrade learns about the
//! skew from whatever request it makes next — and a confused client is at
//! least as likely to be making a request that fails.

use crate::BUILD_STAMP_HEADER;
use axum::response::IntoResponse;

/// Own authenticated state-changing requests before typed extraction begins.
///
/// Authentication must wrap this layer, so a refused caller never starts
/// owned work. Entry here is the acceptance boundary: the helm may finish
/// parsing and carry out a valid request after the caller disconnects.
/// Extraction rejections still produce their usual responses.
///
/// GET and HEAD run inline so reads, implicit HEAD handling for GET routes,
/// and WebSocket upgrades retain their usual cancellation behavior. Routes
/// with other methods that deliberately remain cancellable belong in the
/// separately merged exempt sub-router, without this layer. Do not add an
/// exception here when adding such a route; keep its reason beside the route.
///
/// `run_owned` preserves panic behavior by resuming a handler's panic on the
/// awaiting task rather than turning it into an HTTP error response.
pub(crate) async fn own_state_change(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    if request.method() == axum::http::Method::GET || request.method() == axum::http::Method::HEAD {
        next.run(request).await
    } else {
        crate::run_owned(next.run(request)).await
    }
}

/// Refuse every request that did not come from one of this machine's own non-browser clients: a `Host` other than
/// this helm's loopback literal, any `Origin`, or browser fetch metadata naming another site.
///
/// Binding to 127.0.0.1 keeps other machines out, but not web pages in the user's own browser: a page on
/// attacker.example can rebind its DNS to 127.0.0.1 and then read the API and open terminal WebSockets as if it were a
/// local client, and typing into an agent's terminal is code execution as the user. WebSocket upgrades bypass CORS
/// entirely. The helm serves no page of its own and none of its clients is a browser, so every request a browser page
/// sends is refused (SPEC.md, "Security", "Client to helm"). This stays defense in depth beside the device credentials:
/// authentication decides who holds authority, while this layer refuses a browser request outright.
pub(crate) async fn require_loopback_origin(
    port: u16,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    if !origin_is_allowed(req.headers(), port) {
        return (
            axum::http::StatusCode::FORBIDDEN,
            "request must come from a client on this helm's loopback address, not a web page\n",
        )
            .into_response();
    }
    next.run(req).await
}

/// The header decision behind [`require_loopback_origin`], as a pure function so its matrix is unit-testable.
///
/// The only loopback authority accepted in `Host` is the IPv4 literal `127.0.0.1`, never the name `localhost` or
/// `[::1]`: the helm binds only `127.0.0.1`, so another local account can bind `[::1]` on the same port at any time,
/// and a client resolving `localhost` to `::1` first would talk to that account's server instead. A present `Origin`
/// refuses the request whatever it says, because only browsers send one and no browser page is a client of this
/// helm. `Sec-Fetch-Site` catches the browser requests that carry no `Origin`, such as a top-level navigation from a
/// hostile page or from a page on another loopback port.
fn origin_is_allowed(headers: &axum::http::HeaderMap, port: u16) -> bool {
    let is_loopback_authority = |authority: &str| -> bool {
        // Refuse anything containing '/': deriving the authority by splitting on '/' would accept any value that
        // merely ENDS in a loopback authority ("evil.example/127.0.0.1:7433").
        if authority.contains('/') {
            return false;
        }
        // Clients omit the port from Host when it is the scheme default, so on `--port 80` the explicit-`:80` form
        // never matches; accept the bare authority for exactly that port.
        if port == 80 && authority == "127.0.0.1" {
            return true;
        }
        authority == format!("127.0.0.1:{port}")
    };
    let host_ok = headers
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .is_some_and(is_loopback_authority);
    let no_origin = headers.get(axum::http::header::ORIGIN).is_none();
    let fetch_site_ok = !matches!(
        headers.get("sec-fetch-site").and_then(|v| v.to_str().ok()),
        Some("cross-site" | "same-site")
    );
    host_ok && no_origin && fetch_site_ok
}

/// Stamp this helm's build version onto one response (PLAN_M6.md item 6's
/// client↔helm skew edge; SPEC_impl.md's version-and-skew section).
///
/// The UI compares it against the stamp compiled into its own bundle and
/// surfaces a reload prompt; nothing here refuses anything, unlike the
/// helm↔supervisor hello. The two edges differ deliberately: the supervisor
/// edge refuses because a mismatched FRAME contract cannot be spoken at
/// all, while a stale bundle still works well enough that taking the app
/// away from its user would be the bigger harm.
///
/// Universal by construction rather than by discipline. A browser tab left
/// open across a helm upgrade must learn about it from whatever request it
/// happens to make next, and that includes the requests that FAIL — a
/// mismatch is at least as likely to surface as an inexplicable refusal as
/// it is on a success, and a 403 from the origin guard is exactly the shape
/// a confused client produces.
pub(crate) async fn stamp_build(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mut resp = next.run(req).await;
    resp.headers_mut().insert(
        axum::http::HeaderName::from_static(BUILD_STAMP_HEADER),
        axum::http::HeaderValue::from_static(farhelm_proto::BUILD_VERSION),
    );
    resp
}

#[cfg(test)]
mod tests {
    use super::{origin_is_allowed, own_state_change};
    use axum::http::{HeaderMap, Method, StatusCode};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::sync::{mpsc, watch};
    use tower::ServiceExt;

    const PORT: u16 = 7433;

    // Request ownership: observe the handler's lifetime, not only the caller.

    /// Report when the entered handler future is dropped, independently of
    /// the router closure's lifetime. Caller cancellation alone cannot prove
    /// that a middleware layer also cancelled the work it was awaiting.
    struct HandlerDropWitness(mpsc::UnboundedSender<()>);

    impl Drop for HandlerDropWitness {
        fn drop(&mut self) {
            let _ = self.0.send(());
        }
    }

    /// GET and its implicit HEAD handler must die with the caller.
    ///
    /// A detached read can retain streams or other request-scoped resources.
    /// The gate prevents a normal return, so the drop witness proves that
    /// aborting the caller actually drops the entered handler future.
    #[farhelm_testtrace::test]
    async fn ownership_layer_keeps_get_and_implicit_head_cancellable() {
        for method in [Method::GET, Method::HEAD] {
            let (entered_tx, mut entered_rx) = mpsc::unbounded_channel();
            let (dropped_tx, mut dropped_rx) = mpsc::unbounded_channel();
            // Keeping the sender alive holds the handler at its entry gate.
            // Unwinding the test closes it too, so a broken ownership layer
            // cannot leave detached fixture work blocked indefinitely.
            let (_release_tx, release_rx) = watch::channel(());
            let router = axum::Router::new()
                .route(
                    "/",
                    axum::routing::get(move || {
                        let entered_tx = entered_tx.clone();
                        let dropped_tx = dropped_tx.clone();
                        let mut release_rx = release_rx.clone();
                        async move {
                            let _witness = HandlerDropWitness(dropped_tx);
                            entered_tx.send(()).expect("the test is awaiting entry");
                            let _ = release_rx.changed().await;
                            StatusCode::NO_CONTENT
                        }
                    }),
                )
                .route_layer(axum::middleware::from_fn(own_state_change));
            let caller = tokio::spawn(
                router.oneshot(
                    axum::http::Request::builder()
                        .method(method.clone())
                        .uri("/")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                ),
            );

            tokio::time::timeout(Duration::from_secs(5), entered_rx.recv())
                .await
                .expect("the read handler must enter before cancellation")
                .expect("the read handler must report entry");
            caller.abort();
            let cancelled = tokio::time::timeout(Duration::from_secs(5), caller)
                .await
                .expect("the read caller must stop after abort")
                .expect_err("the gated read caller must be cancelled");
            assert!(cancelled.is_cancelled(), "{method}: {cancelled}");
            tokio::time::timeout(Duration::from_secs(5), dropped_rx.recv())
                .await
                .expect("the read handler must be dropped with its caller")
                .expect("the entered handler must report its drop");
        }
    }

    /// An entered mutation must finish its second step after its caller dies.
    ///
    /// Waiting for the first write before aborting pins the half-completed
    /// action this layer exists to protect. The completion signal and second
    /// write distinguish owned work from a caller that merely stopped waiting.
    #[farhelm_testtrace::test]
    async fn ownership_layer_finishes_mutation_after_caller_cancellation() {
        let progress = Arc::new(AtomicUsize::new(0));
        let (first_step_tx, mut first_step_rx) = mpsc::unbounded_channel();
        let (completed_tx, mut completed_rx) = mpsc::unbounded_channel();
        // Closing this gate on failure also lets any detached work finish.
        let (release_tx, release_rx) = watch::channel(());
        let handler_progress = Arc::clone(&progress);
        let router = axum::Router::new()
            .route(
                "/",
                axum::routing::post(move || {
                    let progress = Arc::clone(&handler_progress);
                    let first_step_tx = first_step_tx.clone();
                    let completed_tx = completed_tx.clone();
                    let mut release_rx = release_rx.clone();
                    async move {
                        progress.store(1, Ordering::SeqCst);
                        first_step_tx
                            .send(())
                            .expect("the test is awaiting the first mutation step");
                        let _ = release_rx.changed().await;
                        progress.store(2, Ordering::SeqCst);
                        let _ = completed_tx.send(());
                        StatusCode::NO_CONTENT
                    }
                }),
            )
            .route_layer(axum::middleware::from_fn(own_state_change));
        let caller = tokio::spawn(
            router.oneshot(
                axum::http::Request::builder()
                    .method(Method::POST)
                    .uri("/")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            ),
        );

        tokio::time::timeout(Duration::from_secs(5), first_step_rx.recv())
            .await
            .expect("the mutation must reach its first step before cancellation")
            .expect("the mutation must report its first step");
        assert_eq!(progress.load(Ordering::SeqCst), 1);
        caller.abort();
        let cancelled = tokio::time::timeout(Duration::from_secs(5), caller)
            .await
            .expect("the mutation caller must stop after abort")
            .expect_err("the gated mutation caller must be cancelled");
        assert!(cancelled.is_cancelled(), "{cancelled}");
        release_tx
            .send(())
            .expect("the owned handler must still hold the release gate");
        tokio::time::timeout(Duration::from_secs(5), completed_rx.recv())
            .await
            .expect("owned work must finish after its caller is cancelled")
            .expect("owned work must report completion");
        assert_eq!(progress.load(Ordering::SeqCst), 2);
    }

    /// Handler panics must remain panics at the awaiting request task.
    ///
    /// Owning the handler introduces a task boundary, but it must not turn
    /// that failure into a fabricated HTTP error response or a normal return.
    #[farhelm_testtrace::test]
    async fn ownership_layer_resumes_handler_panics() {
        /// Give the panic fixture an explicit response type so the route has
        /// the same handler contract as a normally returning mutation.
        async fn panicking_handler() -> StatusCode {
            panic!("owned middleware panic fixture");
        }

        let router = axum::Router::new()
            .route("/", axum::routing::post(panicking_handler))
            .route_layer(axum::middleware::from_fn(own_state_change));
        let caller = tokio::spawn(
            router.oneshot(
                axum::http::Request::builder()
                    .method(Method::POST)
                    .uri("/")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            ),
        );
        let panic = tokio::time::timeout(Duration::from_secs(5), caller)
            .await
            .expect("the handler panic must reach its caller")
            .expect_err("a handler panic must not become an HTTP response");
        assert!(panic.is_panic(), "{panic}");
        assert_eq!(
            panic.into_panic().downcast_ref::<&str>(),
            Some(&"owned middleware panic fixture")
        );
    }

    // Origin admission: pin the request matrix of the loopback guard.

    fn headers(host: Option<&str>, origin: Option<&str>) -> HeaderMap {
        let mut h = HeaderMap::new();
        if let Some(host) = host {
            h.insert(axum::http::header::HOST, host.parse().unwrap());
        }
        if let Some(origin) = origin {
            h.insert(axum::http::header::ORIGIN, origin.parse().unwrap());
        }
        h
    }

    fn with_fetch_site(mut h: HeaderMap, value: &str) -> HeaderMap {
        h.insert("sec-fetch-site", value.parse().unwrap());
        h
    }

    /// Non-browser clients (the desktop app's own HTTP client, tests, curl) send only `Host`, and that is all the
    /// guard admits: the IPv4 loopback literal with this helm's port.
    #[farhelm_testtrace::test]
    fn a_loopback_host_without_browser_headers_is_allowed() {
        assert!(origin_is_allowed(
            &headers(Some("127.0.0.1:7433"), None),
            PORT
        ));
        assert!(origin_is_allowed(
            &with_fetch_site(headers(Some("127.0.0.1:7433"), None), "none"),
            PORT
        ));
        assert!(origin_is_allowed(
            &with_fetch_site(headers(Some("127.0.0.1:7433"), None), "same-origin"),
            PORT
        ));
    }

    /// The guard is the only thing between a web page and keystroke-level control of a live agent, and no browser
    /// page is a client of this helm, so any `Origin` refuses the request: the helm's own loopback origin (a rebound
    /// DNS name serves a page "from" it), the old desktop web view's custom schemes, `null` and HTTPS alike.
    #[farhelm_testtrace::test]
    fn any_origin_is_refused() {
        for origin in [
            "http://127.0.0.1:7433",
            "dioxus://index.html",
            "wry://localhost",
            "null",
            "https://127.0.0.1:7433",
            "http://attacker.example",
        ] {
            assert!(
                !origin_is_allowed(&headers(Some("127.0.0.1:7433"), Some(origin)), PORT),
                "{origin}"
            );
        }
    }

    /// Loopback names other than the IPv4 literal can be served by another local account, so they are refused, as
    /// are foreign, missing, suffix-shaped and wrong-port hosts.
    #[farhelm_testtrace::test]
    fn hosts_other_than_this_helms_ipv4_loopback_literal_are_refused() {
        for host in [
            Some("localhost:7433"),
            Some("[::1]:7433"),
            Some("attacker.example"),
            Some("evil.example/127.0.0.1:7433"),
            Some("127.0.0.1:7434"),
            None,
        ] {
            assert!(!origin_is_allowed(&headers(host, None), PORT), "{host:?}");
        }
    }

    /// Browsers omit the default port from `Host`, so on `--port 80` the bare literal must pass, and only there.
    #[farhelm_testtrace::test]
    fn port_80_accepts_the_portless_loopback_literal() {
        assert!(origin_is_allowed(&headers(Some("127.0.0.1"), None), 80));
        assert!(origin_is_allowed(&headers(Some("127.0.0.1:80"), None), 80));
        assert!(!origin_is_allowed(&headers(Some("127.0.0.1"), None), PORT));
    }

    /// A top-level navigation from another site, or from a page on another loopback port, carries no `Origin`; its
    /// fetch metadata is what refuses it.
    #[farhelm_testtrace::test]
    fn cross_site_and_same_site_fetches_are_refused() {
        for site in ["cross-site", "same-site"] {
            assert!(
                !origin_is_allowed(
                    &with_fetch_site(headers(Some("127.0.0.1:7433"), None), site),
                    PORT
                ),
                "{site}"
            );
        }
    }
}
