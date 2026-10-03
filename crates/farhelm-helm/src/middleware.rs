//! The layers wrapped around the helm's routes: who is allowed to ask, who
//! is allowed to read the answer, and what every answer carries.
//!
//! Three concerns live here rather than inside the handlers, and each is
//! here for the same reason: within whatever scope it covers, it has to
//! hold for EVERY response, including the ones that fail. A guard a handler
//! forgot to call is a hole; a header a handler forgot to set on its error
//! path is a silent failure. Layers are the only construction that cannot
//! be forgotten response by response.
//!
//! The scopes differ, and deliberately. The origin guard and the build
//! stamp wrap the whole router, because both answer questions that have
//! nothing to do with which route was asked for. The CORS headers wrap the
//! four desktop-webview fetch edges, because widening them is widening what
//! a cross-origin page may read.
//!
//! ## The loopback guard is a real security boundary
//!
//! Binding to 127.0.0.1 is not by itself a defense against the user's OWN
//! browser: a hostile page can rebind DNS to loopback and reach this helm
//! as if it were same-origin, and WebSocket upgrades are not CORS-gated at
//! all. `require_loopback_origin` is what actually stands in the way. Its
//! `mode` selects the embedded helm's custom-scheme exemption, and
//! `origin_is_allowed` is its decision as a pure function so the whole
//! matrix can be pinned by unit tests rather than by whichever branches
//! an integration test happens to reach.
//!
//! ## CORS is the desktop build's, and only the desktop build's
//!
//! The web build is served BY the helm, so nothing it does is
//! cross-origin. The desktop build's page comes from a custom webview
//! scheme, so its credential validation, uploads, client-log reports, and
//! clipboard writes are. `desktop_webview_cors` closes that gap on exactly
//! those four webview fetch routes, for exactly the origins
//! `is_desktop_webview_origin` recognizes.
//!
//! ## The build stamp is how a stale tab finds out
//!
//! `stamp_build` rides every response, successes and failures alike,
//! because a browser tab left open across a helm upgrade learns about the
//! skew from whatever request it makes next — and a confused client is at
//! least as likely to be making a request that fails.

use crate::BUILD_STAMP_HEADER;
use axum::response::IntoResponse;

/// Reject requests whose `Host` — or, for browsers, `Origin` — is not
/// this helm's own loopback address.
///
/// Binding to 127.0.0.1 keeps other machines out, but not other origins
/// in the user's own browser: a page on attacker.example can rebind its
/// DNS to 127.0.0.1 and then read `/api/sessions` and open terminal
/// WebSockets as if same-origin — and typing into an agent's terminal is
/// code execution as the user. WebSocket upgrades bypass CORS entirely,
/// so this check remains defense in depth beside explicit device credentials:
/// authentication decides who holds authority, while this layer refuses a
/// browser request that arrived through the wrong origin in the first place.
pub(crate) async fn require_loopback_origin(
    port: u16,
    mode: crate::ServingMode,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    if !origin_is_allowed(req.headers(), port, mode) {
        if let Some(location) = legacy_loopback_redirect(req.method(), req.headers(), port) {
            return (
                axum::http::StatusCode::TEMPORARY_REDIRECT,
                [(axum::http::header::LOCATION, location)],
            )
                .into_response();
        }
        return (
            axum::http::StatusCode::FORBIDDEN,
            "request must originate from this helm's loopback address\n",
        )
            .into_response();
    }
    let mut resp = next.run(req).await;
    // Framing defense. The header check above cannot stop an
    // `<iframe src="http://127.0.0.1:PORT/">`: a GET navigation sends no
    // Origin, so it passes, and the framed page's own fetches are then
    // genuinely same-origin. The frame is unreadable cross-origin, but
    // it is a focused terminal wired to a live agent — clickjacking
    // would deliver keystrokes, which is command execution.
    let headers = resp.headers_mut();
    headers.insert(
        axum::http::header::X_FRAME_OPTIONS,
        axum::http::HeaderValue::from_static("DENY"),
    );
    headers.insert(
        axum::http::header::CONTENT_SECURITY_POLICY,
        axum::http::HeaderValue::from_static("frame-ancestors 'none'"),
    );
    resp
}

/// The header decision behind [`require_loopback_origin`], as a pure
/// function so its matrix is unit-testable (a browser cannot set `Host`,
/// so the integration test can only reach the Origin half).
///
/// The only loopback authority accepted is the IPv4 literal `127.0.0.1`,
/// never the name `localhost` or `[::1]`. The helm binds only
/// `127.0.0.1`, so another local account can bind `[::1]` on the same port
/// at any time, and a browser resolving `localhost` to `::1` first would
/// load that account's page at `http://localhost:<port>`. That page's
/// origin is the one the UI's stored device secret lives under, so serving
/// the UI there would hand the secret to whoever holds `[::1]`. Binding
/// `[::1]` as well does not close it: IPv6 loopback can appear after the
/// helm starts. Refusing the names means no credential is ever stored
/// under an origin another account can serve. SPEC_impl.md records the
/// rule and the residual it leaves (a squatter can still show a fake token
/// prompt at `localhost`).
fn origin_is_allowed(headers: &axum::http::HeaderMap, port: u16, mode: crate::ServingMode) -> bool {
    let is_loopback_authority = |authority: &str| -> bool {
        // Refuse anything containing '/': deriving the authority by
        // splitting on '/' would accept any value that merely ENDS in a
        // loopback authority ("evil.example/127.0.0.1:7433"). No browser
        // emits such a Host/Origin, but this check is the sole gate in
        // front of command execution, so it must not lean on the client's
        // URL parser for its own correctness.
        if authority.contains('/') {
            return false;
        }
        // Browsers omit the port from Host and Origin when it is the
        // scheme default, so on `--port 80` the explicit-`:80` forms
        // below never match and every request would 403 — a functional
        // lockout of a legal flag value. Accept the bare authorities for
        // exactly that port; everything else stays exact-match.
        if port == 80 && authority == "127.0.0.1" {
            return true;
        }
        authority == format!("127.0.0.1:{port}")
    };

    let host_ok = headers
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .is_some_and(is_loopback_authority);

    // A missing Origin is fine — curl and other non-browser clients omit
    // it — but a present one must match. Embedded mode is the one
    // exception: its webview serves the app from dioxus's custom scheme,
    // so its WebSocket carries that origin. A web page cannot forge a
    // custom-scheme Origin, which is why this is safe to allow there;
    // standalone mode refuses it. Both modes refuse `null` (sandboxed
    // iframes and data: documents), which has no trusted relationship with
    // this helm.
    //
    // Host carries no scheme; Origin does, and only `http://` can be this
    // helm's own page: it never serves TLS. The scheme also decides what a
    // portless authority means, so accepting `https://` would let
    // `https://127.0.0.1` (port 443, some other server) pass as the helm's
    // own origin on `--port 80`.
    let origin_ok = headers.get(axum::http::header::ORIGIN).is_none_or(|v| {
        v.to_str().is_ok_and(|o| {
            o.strip_prefix("http://").is_some_and(is_loopback_authority)
                || (mode == crate::ServingMode::Embedded && is_desktop_webview_origin(o))
        })
    });

    // The Origin check has one browser-shaped hole: a TOP-LEVEL cross-site
    // navigation (a hostile page assigning window.location to this helm's
    // predictable loopback URL) is a GET that carries no Origin at all, so
    // it sails through the arm above — and with the UI auto-selecting and
    // attaching a session on load, merely rendering the page displaces
    // whichever client held that terminal. `Sec-Fetch-Site` is the
    // browser-set (unforgeable from content) fetch-metadata header that
    // names the relationship: reject `cross-site` when no allowed Origin
    // vouched for the request. Everything legitimate stays open — address
    // bar and bookmark launches say `none`, reloads and the app's own
    // requests say `same-origin`, non-browser clients send nothing, and
    // the desktop webview's fetches (cross-site by construction, custom
    // scheme → loopback) carry their custom-scheme Origin, which the arm
    // above admits only on the embedded helm, and are gated by that arm
    // instead of this one.
    let fetch_site_ok = headers.get(axum::http::header::ORIGIN).is_some()
        || headers.get("sec-fetch-site").and_then(|v| v.to_str().ok()) != Some("cross-site");

    host_ok && origin_ok && fetch_site_ok
}

/// Where to send a plain page load that named this helm by a loopback name
/// it no longer answers to, or `None` when the request must simply be
/// refused.
///
/// Old bookmarks and habits say `http://localhost:<port>/`. When such a
/// request reaches the real helm (the browser fell back to IPv4 because
/// nothing listens on `[::1]`), a redirect to the IPv4 literal is friendlier
/// than a bare 403 and gives nothing away: it only ever comes from the
/// genuine helm, and its target is this fixed literal, never anything taken
/// from the request. The target is always `/` because the UI has no path
/// routing. Only `GET` and `HEAD` without an `Upgrade` header qualify; API
/// calls and WebSocket upgrades under those names stay refused, since no
/// page this helm serves can make them.
fn legacy_loopback_redirect(
    method: &axum::http::Method,
    headers: &axum::http::HeaderMap,
    port: u16,
) -> Option<axum::http::HeaderValue> {
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return None;
    }
    if headers.contains_key(axum::http::header::UPGRADE) {
        return None;
    }
    let host = headers
        .get(axum::http::header::HOST)
        .and_then(|value| value.to_str().ok())?;
    let named = host == format!("localhost:{port}")
        || host == format!("[::1]:{port}")
        || (port == 80 && matches!(host, "localhost" | "[::1]"));
    if !named {
        return None;
    }
    axum::http::HeaderValue::from_str(&format!("http://127.0.0.1:{port}/")).ok()
}

/// Whether an `Origin` is one of the desktop build's own webview schemes.
///
/// The single predicate for a desktop framework webview scheme, shared by
/// [`origin_is_allowed`] and [`desktop_webview_cors`]. The origin guard adds
/// the embedded-mode check before admitting it; the CORS layer only runs
/// after that guard. Keeping the scheme predicate in one place prevents the
/// two layers from disagreeing about which embedded requests the window may
/// read.
///
/// Safe to allow against the threat this guard exists for: a web page in a
/// browser cannot forge a custom-scheme `Origin`. The exemption is used only
/// by the embedded helm, whose page necessarily comes from one of these
/// schemes; the standalone helm has no such page and rejects them before the
/// CORS layers run. This does not identify Farhelm's own window: every Dioxus
/// desktop app serves its page as `dioxus://index.html/`, and any app built on
/// wry can use `wry://`. Content shown by another such app can therefore pass
/// the embedded browser-facing check, which still requires a device
/// credential and never establishes the native app as a client.
fn is_desktop_webview_origin(origin: &str) -> bool {
    origin.starts_with("dioxus://") || origin.starts_with("wry://")
}

/// The CORS headers for the desktop webview's four cross-origin fetch edges.
///
/// The web build has no CORS problem: the helm serves the page, so its
/// uploads are same-origin. The desktop build does. Its page is served by
/// wry from a custom scheme while the helm answers on
/// `http://127.0.0.1:<port>`, so every JavaScript `fetch` from it is
/// cross-origin. Today those fetches are credential validation, attachment
/// upload, client-log reporting, and native clipboard writes (`clipboard.rs`).
/// The terminal and invalidation WebSockets are governed
/// by the origin guard and explicit subprotocol credential instead: WebSocket
/// upgrades are not CORS-gated.
///
/// Deliberately narrow in every direction:
///
/// - Only [`is_desktop_webview_origin`] origins get headers at all — in
///   embedded mode these are the same origins the loopback guard lets
///   through, echoed back
///   rather than answered with `*`, with `Vary: Origin` so nothing caches
///   one origin's answer for another.
/// - The credential-validation, attachment, client-log, and clipboard routes
///   carry the four useful desktop answers (see `build_router`). The
///   standalone token-exchange route still carries the layer because this
///   change gates the origin check and nothing else; its custom-scheme
///   requests are refused by the outer guard before it can add headers, so it
///   is an inert fifth layer, not a desktop fetch edge.
///   The ordinary REST client is native, so the other routes have no
///   cross-origin caller and get no cross-origin readability.
/// - Only the methods and headers those routes need: `GET` for credential
///   validation, `POST` for upload/client-log/clipboard (plus the
///   `OPTIONS`
///   preflight itself), `authorization` for the explicit device secret, and
///   `content-type`, which `fetch(url, {body: file})` sets from the blob and
///   may itself make non-simple (the client-log body sends the same header
///   for its JSON body).
///
/// Applied as a middleware rather than inside the handler because the
/// headers have to be on EVERY answer, error ones included: a 500 the page
/// cannot read is a failure with no message, which is precisely the
/// silent-failure mode SPEC.md's "upload failures must be visible" rules
/// out. The one response it deliberately does not reach is the loopback
/// guard's own 403, which is outside this layer — an origin that was
/// refused must not be handed the means to read the refusal.
pub(crate) async fn desktop_webview_cors(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let origin = req
        .headers()
        .get(axum::http::header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .filter(|origin| is_desktop_webview_origin(origin))
        .map(|origin| origin.to_string());
    let mut response = next.run(req).await;
    let Some(origin) = origin else {
        return response;
    };
    // A header value that cannot be built from an origin this guard
    // already accepted would mean the origin contained control bytes; the
    // honest answer is then no CORS headers rather than a mangled one.
    let Ok(origin) = axum::http::HeaderValue::from_str(&origin) else {
        return response;
    };
    let headers = response.headers_mut();
    headers.insert(axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
    headers.insert(
        axum::http::header::VARY,
        axum::http::HeaderValue::from_static("Origin"),
    );
    headers.insert(
        axum::http::header::ACCESS_CONTROL_ALLOW_METHODS,
        axum::http::HeaderValue::from_static("GET, POST, OPTIONS"),
    );
    headers.insert(
        axum::http::header::ACCESS_CONTROL_ALLOW_HEADERS,
        axum::http::HeaderValue::from_static("authorization, content-type"),
    );
    // The build stamp is READABLE cross-origin, and only it. A
    // cross-origin response exposes none of its headers to script by
    // default. Nothing else is exposed: this list is the same deliberate
    // minimum as the methods and headers above.
    headers.insert(
        axum::http::header::ACCESS_CONTROL_EXPOSE_HEADERS,
        axum::http::HeaderValue::from_static(BUILD_STAMP_HEADER),
    );
    // Ten minutes: long enough that a burst of pastes does not preflight
    // every time, short enough that a helm restarted with different rules
    // is not shadowed by a stale permission for the rest of the day.
    headers.insert(
        axum::http::header::ACCESS_CONTROL_MAX_AGE,
        axum::http::HeaderValue::from_static("600"),
    );
    response
}

/// The desktop webview routes' CORS preflight.
///
/// The empty body is intentional: the permission is entirely in the headers
/// [`desktop_webview_cors`] attaches on the way out.
///
/// Present as a real route because a preflight is a real request: without
/// it, `OPTIONS /api/sessions/{id}/attachments` is a 405 the browser reads
/// as "not allowed", and the desktop build's upload never leaves the page.
pub(crate) async fn desktop_webview_preflight() -> axum::response::Response {
    axum::http::StatusCode::NO_CONTENT.into_response()
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
    use super::{legacy_loopback_redirect, origin_is_allowed};
    use axum::http::HeaderMap;
    const PORT: u16 = 7433;

    /// Pin a matrix case to the standalone helm, whose custom-scheme arm is
    /// deliberately the stricter baseline for the loopback guard.
    fn standalone_origin_is_allowed(headers: &HeaderMap, port: u16) -> bool {
        origin_is_allowed(headers, port, crate::ServingMode::Standalone)
    }

    /// Pin a matrix case to the embedded helm, the only mode that admits the
    /// desktop webview's custom-scheme Origin.
    fn embedded_origin_is_allowed(headers: &HeaderMap, port: u16) -> bool {
        origin_is_allowed(headers, port, crate::ServingMode::Embedded)
    }

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

    /// The full decision matrix for the DNS-rebinding defense. This
    /// check is the only thing between a hostile web page (or a spoofed
    /// Host) and keystroke-level control of a live agent, so every
    /// branch is pinned individually: each of these would pass with some
    /// plausible-but-wrong implementation (suffix matching, `null`
    /// treated as absent, Host ignored).
    #[farhelm_testtrace::test]
    fn loopback_hosts_with_no_or_loopback_origin_are_allowed() {
        // curl and non-browser clients: Host only, no Origin.
        assert!(standalone_origin_is_allowed(
            &headers(Some("127.0.0.1:7433"), None),
            PORT
        ));
        // The browser's own same-origin requests carry both.
        assert!(standalone_origin_is_allowed(
            &headers(Some("127.0.0.1:7433"), Some("http://127.0.0.1:7433")),
            PORT
        ));
    }

    /// `localhost` and `[::1]` name loopback too, but another local account
    /// can serve them: the helm binds only `127.0.0.1`, so `[::1]` on the
    /// same port is free for anyone, and a browser that resolves
    /// `localhost` to `::1` would load that account's page under the origin
    /// the UI keeps its device secret in. Neither name may be accepted as
    /// Host or Origin, or the UI could again be served (and its secret
    /// stored) under an origin the helm does not own.
    #[farhelm_testtrace::test]
    fn loopback_names_other_than_the_ipv4_literal_are_refused() {
        for host in ["localhost:7433", "[::1]:7433"] {
            assert!(!standalone_origin_is_allowed(
                &headers(Some(host), None),
                PORT
            ));
        }
        for origin in ["http://localhost:7433", "http://[::1]:7433"] {
            assert!(!standalone_origin_is_allowed(
                &headers(Some("127.0.0.1:7433"), Some(origin)),
                PORT
            ));
        }
    }

    fn with_method_and_upgrade(upgrade: bool, host: &str) -> (axum::http::Method, HeaderMap) {
        let mut h = headers(Some(host), None);
        if upgrade {
            h.insert(axum::http::header::UPGRADE, "websocket".parse().unwrap());
        }
        (axum::http::Method::GET, h)
    }

    /// A plain page load that still says `localhost` or `[::1]` is sent to
    /// the IPv4 literal instead of dead-ending in a 403.
    ///
    /// Old bookmarks say `http://localhost:<port>/`, and when the browser
    /// falls back to IPv4 the request reaches this helm. The target is the
    /// fixed literal and root path whatever the request carried, so the
    /// redirect cannot be steered anywhere.
    #[farhelm_testtrace::test]
    fn legacy_loopback_page_loads_redirect_to_the_ipv4_literal() {
        for host in ["localhost:7433", "[::1]:7433"] {
            let (method, h) = with_method_and_upgrade(false, host);
            assert_eq!(
                legacy_loopback_redirect(&method, &h, PORT).unwrap(),
                "http://127.0.0.1:7433/"
            );
            assert!(
                legacy_loopback_redirect(&axum::http::Method::HEAD, &h, PORT).is_some(),
                "HEAD is a page probe like GET"
            );
        }
        for host in ["localhost", "[::1]"] {
            let (method, h) = with_method_and_upgrade(false, host);
            assert_eq!(
                legacy_loopback_redirect(&method, &h, 80).unwrap(),
                "http://127.0.0.1:80/"
            );
        }
    }

    /// Only plain page loads under the legacy names are redirected.
    ///
    /// A WebSocket upgrade or an API call under `localhost` could only come
    /// from a page served at that origin, which the helm no longer serves,
    /// so those are refused outright. Foreign hosts, the wrong port, and a
    /// portless name on a non-default port get no redirect either: the
    /// redirect is a convenience for the one legacy spelling, not a second
    /// way past the guard.
    #[farhelm_testtrace::test]
    fn only_plain_legacy_page_loads_are_redirected() {
        let (method, h) = with_method_and_upgrade(true, "localhost:7433");
        assert!(legacy_loopback_redirect(&method, &h, PORT).is_none());
        let (_, h) = with_method_and_upgrade(false, "localhost:7433");
        assert!(legacy_loopback_redirect(&axum::http::Method::POST, &h, PORT).is_none());
        for host in [
            "attacker.example:7433",
            "localhost:9999",
            "localhost",
            "127.0.0.1:9999",
        ] {
            let (method, h) = with_method_and_upgrade(false, host);
            assert!(
                legacy_loopback_redirect(&method, &h, PORT).is_none(),
                "{host} must not redirect"
            );
        }
    }

    /// Custom-scheme origins are admitted only by the embedded helm. A
    /// standalone helm has no page that needs this cross-origin exemption,
    /// so accepting another framework app's scheme there would widen its
    /// browser boundary for no reason. `null` (sandboxed iframe, data:
    /// document) is refused in both modes.
    #[farhelm_testtrace::test]
    fn custom_scheme_origins_are_allowed_only_when_embedded() {
        let host = Some("127.0.0.1:7433");
        assert!(!standalone_origin_is_allowed(
            &headers(host, Some("dioxus://index.html")),
            PORT
        ));
        assert!(!standalone_origin_is_allowed(
            &headers(host, Some("wry://localhost")),
            PORT
        ));
        assert!(!standalone_origin_is_allowed(
            &headers(host, Some("null")),
            PORT
        ));
        assert!(embedded_origin_is_allowed(
            &headers(host, Some("dioxus://index.html")),
            PORT
        ));
        assert!(embedded_origin_is_allowed(
            &headers(host, Some("wry://localhost")),
            PORT
        ));
        assert!(!embedded_origin_is_allowed(
            &headers(host, Some("null")),
            PORT
        ));
        assert!(!embedded_origin_is_allowed(
            &headers(Some("attacker.example:7433"), Some("dioxus://index.html")),
            PORT
        ));
        assert!(!embedded_origin_is_allowed(
            &headers(None, Some("wry://localhost")),
            PORT
        ));
    }

    /// A rebinding attack presents a foreign Host (the attacker's domain
    /// resolving to 127.0.0.1) or a foreign Origin; both directions must
    /// refuse, as must a missing Host and the wrong loopback port.
    #[farhelm_testtrace::test]
    fn foreign_or_missing_authorities_are_refused() {
        assert!(!standalone_origin_is_allowed(&headers(None, None), PORT));
        assert!(!standalone_origin_is_allowed(
            &headers(Some("attacker.example:7433"), None),
            PORT
        ));
        assert!(!standalone_origin_is_allowed(
            &headers(Some("127.0.0.1:9999"), None),
            PORT
        ));
        assert!(!standalone_origin_is_allowed(
            &headers(Some("127.0.0.1:7433"), Some("http://evil.example")),
            PORT
        ));
        assert!(!standalone_origin_is_allowed(
            &headers(Some("127.0.0.1:7433"), Some("http://127.0.0.1:9999")),
            PORT
        ));
    }

    /// Browsers omit the scheme-default port from Host/Origin, so on
    /// `--port 80` the bare loopback authorities must be accepted — the
    /// exact-`:80` forms never arrive, and requiring them locks every
    /// browser out of a legal flag value. The bare forms stay refused on
    /// any other port (fail-closed).
    #[farhelm_testtrace::test]
    fn default_port_80_accepts_portless_loopback_authorities() {
        assert!(standalone_origin_is_allowed(
            &headers(Some("127.0.0.1"), None),
            80
        ));
        // The portless names are refused on port 80 like everywhere else.
        assert!(!standalone_origin_is_allowed(
            &headers(Some("localhost"), None),
            80
        ));
        assert!(standalone_origin_is_allowed(
            &headers(Some("127.0.0.1"), Some("http://127.0.0.1")),
            80
        ));
        // Explicit :80 still works (curl sends it).
        assert!(standalone_origin_is_allowed(
            &headers(Some("127.0.0.1:80"), None),
            80
        ));
        // Foreign authorities are still refused on port 80...
        assert!(!standalone_origin_is_allowed(
            &headers(Some("evil.example"), None),
            80
        ));
        // ...and portless loopback stays refused on non-default ports.
        assert!(!standalone_origin_is_allowed(
            &headers(Some("127.0.0.1"), None),
            PORT
        ));
    }

    /// The helm never serves TLS, so an `https://` Origin is never its own
    /// page. On `--port 80` this matters: the bare `https://127.0.0.1`
    /// means port 443, a different server, yet with the scheme discarded
    /// it matched the portless port-80 exception. Refused on every port,
    /// with and without an explicit port.
    #[farhelm_testtrace::test]
    fn https_origins_are_refused() {
        assert!(!standalone_origin_is_allowed(
            &headers(Some("127.0.0.1"), Some("https://127.0.0.1")),
            80
        ));
        assert!(!standalone_origin_is_allowed(
            &headers(Some("127.0.0.1:80"), Some("https://127.0.0.1:80")),
            80
        ));
        assert!(!standalone_origin_is_allowed(
            &headers(Some("127.0.0.1:7433"), Some("https://127.0.0.1:7433")),
            PORT
        ));
    }

    /// Authority derivation must not be a suffix match: a value that
    /// merely ENDS in a loopback authority ("evil.example/127.0.0.1:7433")
    /// has to be refused. No browser sends such a value — the point is
    /// that this gate must not depend on that.
    #[farhelm_testtrace::test]
    fn embedded_loopback_suffixes_are_refused() {
        assert!(!standalone_origin_is_allowed(
            &headers(Some("evil.example/127.0.0.1:7433"), None),
            PORT
        ));
        assert!(!standalone_origin_is_allowed(
            &headers(
                Some("127.0.0.1:7433"),
                Some("http://evil.example/127.0.0.1:7433")
            ),
            PORT
        ));
    }

    fn with_fetch_site(mut h: HeaderMap, value: &str) -> HeaderMap {
        h.insert(
            axum::http::HeaderName::from_static("sec-fetch-site"),
            value.parse().unwrap(),
        );
        h
    }

    /// The fetch-metadata arm: a top-level CROSS-SITE navigation carries no
    /// Origin (which the origin arm must keep permitting for curl and for
    /// direct launches) but does carry `Sec-Fetch-Site: cross-site` — and
    /// with the UI auto-attaching a session on load, rendering the page for
    /// a hostile navigator is a terminal takeover. Every legitimate shape
    /// stays permitted: `none` (address bar), `same-origin` (reloads, the
    /// app's own requests), and an absent header (non-browser clients). The
    /// embedded desktop webview's custom-scheme Origin is separately vouched
    /// for by the origin arm, while standalone mode refuses it.
    #[farhelm_testtrace::test]
    fn cross_site_navigations_are_refused_without_a_vouching_origin() {
        let base = || headers(Some("127.0.0.1:7433"), None);
        assert!(!standalone_origin_is_allowed(
            &with_fetch_site(base(), "cross-site"),
            PORT
        ));
        assert!(standalone_origin_is_allowed(
            &with_fetch_site(base(), "none"),
            PORT
        ));
        assert!(standalone_origin_is_allowed(
            &with_fetch_site(base(), "same-origin"),
            PORT
        ));
        assert!(standalone_origin_is_allowed(&base(), PORT));
        // The standalone helm refuses the custom scheme even when the
        // fetch-metadata header says cross-site.
        assert!(!standalone_origin_is_allowed(
            &with_fetch_site(
                headers(Some("127.0.0.1:7433"), Some("dioxus://index.html")),
                "cross-site"
            ),
            PORT
        ));
        // The embedded helm admits the same request because its own page
        // necessarily comes from that custom scheme.
        assert!(embedded_origin_is_allowed(
            &with_fetch_site(
                headers(Some("127.0.0.1:7433"), Some("dioxus://index.html")),
                "cross-site"
            ),
            PORT
        ));
        assert!(!embedded_origin_is_allowed(
            &with_fetch_site(base(), "cross-site"),
            PORT
        ));
    }
}
