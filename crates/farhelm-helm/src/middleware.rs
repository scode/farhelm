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
//! all. `require_loopback_origin` is what actually stands in the way, and
//! `origin_is_allowed` is its decision as a pure function so the whole
//! matrix can be pinned by unit tests rather than by whichever branches
//! an integration test happens to reach.
//!
//! ## CORS is the desktop build's, and only the desktop build's
//!
//! The web build is served BY the helm, so nothing it does is
//! cross-origin. The desktop build's page comes from a custom webview
//! scheme, so its credential validation, token exchange, uploads, and
//! client-log reports are. `desktop_webview_cors` closes that gap on
//! exactly those four routes, for exactly the origins
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
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    if !origin_is_allowed(req.headers(), port) {
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
fn origin_is_allowed(headers: &axum::http::HeaderMap, port: u16) -> bool {
    let is_loopback_authority = |value: &str| -> bool {
        // Host carries no scheme; Origin does. Strip a known scheme and
        // refuse anything still containing '/': deriving the authority
        // by splitting on '/' would accept any value that merely ENDS
        // in a loopback authority ("evil.example/127.0.0.1:7433"). No
        // browser emits such a Host/Origin, but this check is the sole
        // gate in front of command execution, so it must not lean on
        // the client's URL parser for its own correctness.
        let authority = value
            .strip_prefix("http://")
            .or_else(|| value.strip_prefix("https://"))
            .unwrap_or(value);
        if authority.contains('/') {
            return false;
        }
        // Browsers omit the port from Host and Origin when it is the
        // scheme default, so on `--port 80` the explicit-`:80` forms
        // below never match and every request would 403 — a functional
        // lockout of a legal flag value. Accept the bare authorities for
        // exactly that port; everything else stays exact-match.
        if port == 80 && matches!(authority, "127.0.0.1" | "localhost" | "[::1]") {
            return true;
        }
        authority == format!("127.0.0.1:{port}")
            || authority == format!("localhost:{port}")
            || authority == format!("[::1]:{port}")
    };

    let host_ok = headers
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .is_some_and(&is_loopback_authority);

    // A missing Origin is fine — curl and other non-browser clients omit
    // it — but a present one must match. The desktop target is the
    // exception: its webview serves the app from dioxus's custom scheme,
    // so its WebSocket carries that origin. A web page cannot forge a
    // custom-scheme Origin, which is why this is safe to allow; `null`
    // (sandboxed iframes, data: documents) deliberately is not.
    let origin_ok = headers.get(axum::http::header::ORIGIN).is_none_or(|v| {
        v.to_str()
            .is_ok_and(|o| is_loopback_authority(o) || is_desktop_webview_origin(o))
    });

    host_ok && origin_ok
}

/// Whether an `Origin` is one of the desktop build's own webview schemes.
///
/// The single definition of "the desktop app is calling", shared by
/// [`origin_is_allowed`] (which decides whether the request is answered at
/// all) and [`desktop_webview_cors`] (which decides whether the ANSWER may be
/// read). Two lists would be a way for those to disagree, and disagreeing
/// means either the desktop build breaks or a web page gets CORS access it
/// was never meant to have.
///
/// Safe to allow because a web page cannot forge a custom-scheme `Origin`:
/// only a native webview serving the app from that scheme produces one.
fn is_desktop_webview_origin(origin: &str) -> bool {
    origin.starts_with("dioxus://") || origin.starts_with("wry://")
}

/// The CORS headers for the desktop webview's four cross-origin fetch edges.
///
/// The web build has no CORS problem: the helm serves the page, so its
/// uploads are same-origin. The desktop build does. Its page is served by
/// wry from a custom scheme while the helm answers on
/// `http://127.0.0.1:<port>`, so every JavaScript `fetch` from it is
/// cross-origin. Today those fetches are credential validation, the
/// bootstrap-token exchange, attachment upload, and client-log reporting.
/// The terminal and invalidation WebSockets are governed
/// by the origin guard and explicit subprotocol credential instead: WebSocket
/// upgrades are not CORS-gated.
///
/// Deliberately narrow in every direction:
///
/// - Only [`is_desktop_webview_origin`] origins get headers at all — the
///   same origins the loopback guard already lets through, echoed back
///   rather than answered with `*`, with `Vary: Origin` so nothing caches
///   one origin's answer for another.
/// - Only the credential-validation, token-exchange, attachment, and
///   client-log routes carry it (see `build_router`). The ordinary REST
///   client is native, so the other routes have no cross-origin caller and
///   get no cross-origin readability.
/// - Only the methods and headers those routes need: `GET` for credential
///   validation, `POST` for exchange/upload/client-log (plus the `OPTIONS`
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
    use super::origin_is_allowed;
    use axum::http::HeaderMap;
    const PORT: u16 = 7433;

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
    #[test]
    fn loopback_hosts_with_no_or_loopback_origin_are_allowed() {
        // curl and non-browser clients: Host only, no Origin.
        assert!(origin_is_allowed(
            &headers(Some("127.0.0.1:7433"), None),
            PORT
        ));
        assert!(origin_is_allowed(
            &headers(Some("localhost:7433"), None),
            PORT
        ));
        assert!(origin_is_allowed(&headers(Some("[::1]:7433"), None), PORT));
        // The browser's own same-origin requests carry both.
        assert!(origin_is_allowed(
            &headers(Some("127.0.0.1:7433"), Some("http://127.0.0.1:7433")),
            PORT
        ));
    }

    /// The desktop webview serves the app from a custom scheme a web page
    /// cannot forge; those origins are allowed. `null` (sandboxed iframe,
    /// data: document) is deliberately NOT — loosening `None => true`
    /// into "unparseable/null is fine" would reopen the rebinding hole.
    #[test]
    fn custom_scheme_origins_are_allowed_but_null_is_not() {
        let host = Some("127.0.0.1:7433");
        assert!(origin_is_allowed(
            &headers(host, Some("dioxus://index.html")),
            PORT
        ));
        assert!(origin_is_allowed(
            &headers(host, Some("wry://localhost")),
            PORT
        ));
        assert!(!origin_is_allowed(&headers(host, Some("null")), PORT));
    }

    /// A rebinding attack presents a foreign Host (the attacker's domain
    /// resolving to 127.0.0.1) or a foreign Origin; both directions must
    /// refuse, as must a missing Host and the wrong loopback port.
    #[test]
    fn foreign_or_missing_authorities_are_refused() {
        assert!(!origin_is_allowed(&headers(None, None), PORT));
        assert!(!origin_is_allowed(
            &headers(Some("attacker.example:7433"), None),
            PORT
        ));
        assert!(!origin_is_allowed(
            &headers(Some("127.0.0.1:9999"), None),
            PORT
        ));
        assert!(!origin_is_allowed(
            &headers(Some("127.0.0.1:7433"), Some("http://evil.example")),
            PORT
        ));
        assert!(!origin_is_allowed(
            &headers(Some("127.0.0.1:7433"), Some("http://127.0.0.1:9999")),
            PORT
        ));
    }

    /// Browsers omit the scheme-default port from Host/Origin, so on
    /// `--port 80` the bare loopback authorities must be accepted — the
    /// exact-`:80` forms never arrive, and requiring them locks every
    /// browser out of a legal flag value. The bare forms stay refused on
    /// any other port (fail-closed).
    #[test]
    fn default_port_80_accepts_portless_loopback_authorities() {
        assert!(origin_is_allowed(&headers(Some("127.0.0.1"), None), 80));
        assert!(origin_is_allowed(&headers(Some("localhost"), None), 80));
        assert!(origin_is_allowed(
            &headers(Some("127.0.0.1"), Some("http://127.0.0.1")),
            80
        ));
        // Explicit :80 still works (curl sends it).
        assert!(origin_is_allowed(&headers(Some("127.0.0.1:80"), None), 80));
        // Foreign authorities are still refused on port 80...
        assert!(!origin_is_allowed(&headers(Some("evil.example"), None), 80));
        // ...and portless loopback stays refused on non-default ports.
        assert!(!origin_is_allowed(&headers(Some("127.0.0.1"), None), PORT));
    }

    /// Authority derivation must not be a suffix match: a value that
    /// merely ENDS in a loopback authority ("evil.example/127.0.0.1:7433")
    /// has to be refused. No browser sends such a value — the point is
    /// that this gate must not depend on that.
    #[test]
    fn embedded_loopback_suffixes_are_refused() {
        assert!(!origin_is_allowed(
            &headers(Some("evil.example/127.0.0.1:7433"), None),
            PORT
        ));
        assert!(!origin_is_allowed(
            &headers(
                Some("127.0.0.1:7433"),
                Some("http://evil.example/127.0.0.1:7433")
            ),
            PORT
        ));
    }
}
