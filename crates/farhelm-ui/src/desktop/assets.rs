//! The desktop window's `/assets/*` handler: every bundled asset the webview
//! asks for is answered from the UI tree compiled into this binary, never from
//! the filesystem next to it.

/// First path segment the desktop asset handler claims.
///
/// dioxus-desktop dispatches on exactly this: `desktop_handler` in
/// dioxus-desktop 0.7.10's `protocol.rs` takes `uri.path().split('/').nth(1)`
/// and, if a handler is registered under that name, calls it INSTEAD of
/// `dioxus_asset_resolver::native::serve_asset`. So the name is not a label —
/// it is the route, and it has to equal the first segment manganis puts in
/// front of every bundled asset path (`/assets/<file>`; see
/// `manganis_core::Asset::resolve`).
const ASSET_ROUTE: &str = "assets";
/// Claim `/assets/*` in the desktop window and serve it from the UI tree
/// this build embedded (D6).
///
/// ## Why a handler at all
///
/// A bare binary has no bundle around it. Left to itself, dioxus resolves
/// `asset!()` paths off the FILESYSTEM relative to the executable
/// (`dioxus_asset_resolver::native::get_asset_root`: `<exe>/../Resources` on
/// macOS, `<exe>/../lib/<product>` on Linux), so `farhelm-desktop` sitting in
/// `~/.local/bin` would look for `~/.local/Resources/assets/...` and find
/// nothing — a window that renders chrome and loads no terminal, no fonts, no
/// scripts.
/// The embedded tree (`farhelm_helm::embedded_ui()`, D12) is already in the
/// binary for the helm's own sake; this hands the same bytes to the webview.
///
/// ## Why registration wins
///
/// dioxus-desktop 0.7.10 consults the handler registry BEFORE the filesystem
/// resolver — `protocol.rs`'s `desktop_handler` matches the first path
/// segment against registered names and returns early on a hit. Registering
/// [`ASSET_ROUTE`] therefore takes `/assets/*` away from the resolver
/// entirely, which is what makes the embedded tree authoritative rather than
/// merely a fallback. Note what that costs: with a handler registered there
/// is no filesystem fallback for `/assets/*` at all, so a file missing from
/// the embedded tree is a hard 404 even when a copy happens to sit next to
/// the binary.
///
/// (0.7.10 has no `Config::with_asset_handler`; `use_asset_handler` is the
/// only public way in, which is why this is a hook and must run from a
/// component rather than from [`run`](super::run).)
pub(crate) fn use_embedded_asset_handler() {
    dioxus::desktop::use_asset_handler(ASSET_ROUTE, |request, responder| {
        responder.respond(serve_asset(request.method(), request.uri().path()));
    });
}

/// What [`serve_asset_from`] is allowed to know about the embedded tree: a
/// path in, the bytes out, nothing else.
///
/// The narrow shape is the point. `farhelm_helm::embedded_ui()` answers from
/// a `static` that only a release-shaped build populates, so a test running
/// against the real thing would be testing whichever build it happened to be
/// compiled into. One function pointer's worth of indirection lets the
/// response rules be tested against a fixture instead, with no environment
/// mutation and no `include_dir!` fixture tree to maintain.
type AssetLookup<'a> = dyn Fn(&str) -> Option<&'a [u8]> + 'a;

/// Answer one `dioxus://` asset request out of the embedded UI tree.
///
/// Thin on purpose: it resolves whichever lookup this build has — including
/// none — and hands the rest to [`serve_asset_from`], which is where the
/// rules live and where the tests point.
fn serve_asset(
    method: &dioxus::desktop::wry::http::Method,
    path: &str,
) -> dioxus::desktop::wry::http::Response<Vec<u8>> {
    match farhelm_helm::embedded_ui() {
        Some(dir) => {
            let lookup = move |relative: &str| dir.get_file(relative).map(|file| file.contents());
            serve_asset_from(Some(&lookup), method, path)
        }
        None => serve_asset_from(None, method, path),
    }
}

/// The response rules for one asset request, over any lookup.
///
/// Deliberately mirrors `farhelm-helm`'s `serve_embedded` for the rules that
/// matter — the `GET`/`HEAD` method gate, percent-decoding before lookup, and
/// `mime_guess` content types — so an asset behaves identically whether a
/// browser fetched it from the helm or the native window fetched it from
/// here. It does NOT mirror the SPA `index.html` fallback: this route only
/// ever serves concrete files, and answering a miss with markup would hand
/// the webview HTML where it asked for JavaScript. Nor does anything strip a
/// `HEAD` response's body the way axum's router does for the helm — wry hands
/// the response straight back — which costs nothing here, since the webview
/// only ever issues `GET` for an asset.
///
/// Every outcome is logged at DEBUG with a stable prefix, because that log is
/// the only evidence anyone has that the handler ran at all:
/// `scripts/desktop-smoke.sh` asserts at least one `served` line and zero
/// `missing` lines, which is what turns "the window looked fine" into a real
/// gate. Do not reword these lines without updating that script.
///
/// `lookup` is `None` for a build that embedded no UI at all. That is not an
/// error: D12 makes it a supported developer arrangement (`cargo build -p
/// farhelm-desktop` with no `FARHELM_UI_DIST`), and it answers every request
/// with the same empty 404 a miss gets — a window with no UI, whose one
/// explanation is the log line below. Taking it as a parameter rather than
/// reading `embedded_ui()` here is what lets that branch be tested at all:
/// whether a tree is embedded is fixed when the test binary is compiled.
fn serve_asset_from(
    lookup: Option<&AssetLookup<'static>>,
    method: &dioxus::desktop::wry::http::Method,
    path: &str,
) -> dioxus::desktop::wry::http::Response<Vec<u8>> {
    use dioxus::desktop::wry::http::{Method, Response, StatusCode, header};

    if *method != Method::GET && *method != Method::HEAD {
        tracing::debug!("desktop asset handler: refused {method} {path} (405)");
        return Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .header(header::ALLOW, "GET,HEAD")
            .body(Vec::new())
            .expect("a status-and-header response is always well-formed");
    }
    let Some(lookup) = lookup else {
        tracing::debug!("desktop asset handler: no UI is embedded in this build, {path} (404)");
        return not_found();
    };
    // `include_dir!` keys every entry by its path relative to the embedded
    // root with no leading slash; every path wry hands a handler has one.
    let relative = path.trim_start_matches('/');
    // Decode once, before the lookup: an asset whose real name needs
    // escaping in a URL must resolve to the file `include_dir!` compiled in
    // under its actual name. Invalid UTF-8 cannot match any entry (they are
    // all Rust string literals), so it is a miss rather than a panic.
    let Ok(relative) = percent_encoding::percent_decode_str(relative).decode_utf8() else {
        tracing::debug!("desktop asset handler: missing {path} (404)");
        return not_found();
    };
    match lookup(relative.as_ref()) {
        Some(bytes) => {
            tracing::debug!("desktop asset handler: served {path} (200)");
            Response::builder()
                .header(
                    header::CONTENT_TYPE,
                    mime_guess::from_path(relative.as_ref())
                        .first_or_octet_stream()
                        .essence_str(),
                )
                // Matches what dioxus's own resolver stamps on every asset
                // it serves. The page and its assets share the
                // `dioxus://index.html` origin, so nothing here NEEDS it
                // today; diverging from the responses the rest of the
                // ecosystem produces is the larger risk.
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(bytes.to_vec())
                .unwrap_or_else(|_| not_found())
        }
        None => {
            tracing::debug!("desktop asset handler: missing {path} (404)");
            not_found()
        }
    }
}

/// The one 404 shape [`serve_asset`] returns, built here so every miss looks
/// the same to the webview regardless of which check rejected it.
fn not_found() -> dioxus::desktop::wry::http::Response<Vec<u8>> {
    dioxus::desktop::wry::http::Response::builder()
        .status(dioxus::desktop::wry::http::StatusCode::NOT_FOUND)
        .body(Vec::new())
        .expect("a status-only response is always well-formed")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- the desktop asset handler (D6) ----
    //
    // These drive `serve_asset_from` against a fixture lookup rather than the
    // real embedded tree, which only a release-shaped build populates. What
    // is under test is the RESPONSE CONTRACT: with the handler registered,
    // dioxus stops consulting its own filesystem resolver for `/assets/*`
    // (see `use_embedded_asset_handler`), so whatever this function returns
    // is the entire answer the webview gets — there is nothing behind it to
    // paper over a wrong status, a wrong content type, or a body that is
    // subtly not the file.

    /// A two-entry embedded tree: one JavaScript asset and one whose name
    /// needs percent-escaping in a URL.
    fn fixture_lookup(relative: &str) -> Option<&'static [u8]> {
        match relative {
            "assets/terminal-dxhabc.js" => Some(b"console.log('hi')\n"),
            "assets/a name with spaces.css" => Some(b"body{}"),
            _ => None,
        }
    }

    fn method(name: &str) -> dioxus::desktop::wry::http::Method {
        dioxus::desktop::wry::http::Method::from_bytes(name.as_bytes())
            .expect("test method is valid")
    }

    /// Serve against the fixture tree — the release-shaped configuration.
    fn serve(method_name: &str, path: &str) -> dioxus::desktop::wry::http::Response<Vec<u8>> {
        serve_asset_from(Some(&fixture_lookup), &method(method_name), path)
    }

    /// Serve against no embedded tree at all — the plain-`cargo build`
    /// configuration.
    fn serve_unembedded(
        method_name: &str,
        path: &str,
    ) -> dioxus::desktop::wry::http::Response<Vec<u8>> {
        serve_asset_from(None, &method(method_name), path)
    }

    /// A hit returns the file's exact bytes, a guessed content type, and the
    /// permissive CORS header dioxus's own resolver stamps on assets.
    ///
    /// The content type is the load-bearing part: a webview that receives
    /// `application/octet-stream` for a `<script src>` refuses to execute it,
    /// which looks exactly like an asset that never loaded.
    #[farhelm_testtrace::test]
    fn a_hit_returns_the_bytes_with_a_guessed_content_type() {
        let response = serve("GET", "/assets/terminal-dxhabc.js");
        assert_eq!(response.status(), 200);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/javascript"
        );
        assert_eq!(
            response
                .headers()
                .get("access-control-allow-origin")
                .unwrap(),
            "*"
        );
        assert_eq!(response.body().as_slice(), b"console.log('hi')\n");
    }

    /// A percent-escaped path resolves to the file whose real name contains
    /// those characters.
    ///
    /// Decoding has to happen before BOTH the lookup and the `mime_guess`
    /// call: an undecoded `%20` would miss the entry, and a path decoded
    /// after the type guess would classify by the wrong extension.
    #[farhelm_testtrace::test]
    fn a_percent_escaped_path_is_decoded_once_before_lookup_and_typing() {
        let response = serve("GET", "/assets/a%20name%20with%20spaces.css");
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers().get("content-type").unwrap(), "text/css");
        assert_eq!(response.body().as_slice(), b"body{}");
    }

    /// `HEAD` returns exactly what `GET` would, body included.
    ///
    /// wry hands the response back untouched — there is no router layer here
    /// to strip the body and recompute `Content-Length` the way axum does for
    /// the helm — so returning the same response is both the simplest and the
    /// only truthful option.
    #[farhelm_testtrace::test]
    fn head_is_answered_identically_to_get() {
        let head = serve("HEAD", "/assets/terminal-dxhabc.js");
        let get = serve("GET", "/assets/terminal-dxhabc.js");
        assert_eq!(head.status(), get.status());
        assert_eq!(head.headers(), get.headers());
        assert_eq!(head.body(), get.body());
    }

    /// Anything other than `GET`/`HEAD` is refused before any lookup, with
    /// the `Allow` header that makes the refusal actionable.
    ///
    /// Parity with `farhelm-helm`'s `serve_embedded`, so the same request
    /// gets the same answer from the native window and from the browser.
    #[farhelm_testtrace::test]
    fn a_write_method_is_refused_with_the_allowed_set() {
        let response = serve("POST", "/assets/terminal-dxhabc.js");
        assert_eq!(response.status(), 405);
        assert_eq!(response.headers().get("allow").unwrap(), "GET,HEAD");
        assert!(response.body().is_empty());
    }

    /// A path with no entry is a 404, not an `index.html` fallback.
    ///
    /// The helm's embedded source answers an extension-less miss with
    /// `index.html` so a single-page route can render. This route must NOT:
    /// its only clients are `<script>`, `<link>` and font requests, and
    /// handing one of those a page of HTML produces a parse error instead of
    /// a legible failure.
    #[farhelm_testtrace::test]
    fn a_miss_is_a_plain_404_with_no_spa_fallback() {
        for path in ["/assets/never-bundled.js", "/assets/looks-like-a-route"] {
            let response = serve("GET", path);
            assert_eq!(response.status(), 404, "{path}");
            assert!(response.body().is_empty(), "{path}");
        }
    }

    /// A percent escape that decodes to invalid UTF-8 is a miss, not a panic.
    ///
    /// Every embedded entry is keyed by a Rust string literal, so no such
    /// path could ever match one. The webview is not a trusted input source
    /// in the sense that matters here: this runs in the app's own process,
    /// and a panic in the handler takes the window with it.
    #[farhelm_testtrace::test]
    fn an_undecodable_path_is_a_miss_rather_than_a_panic() {
        let response = serve("GET", "/assets/%ff%fe.js");
        assert_eq!(response.status(), 404);
    }

    /// A build with no embedded UI answers every asset with an empty 404.
    ///
    /// D12 makes that a supported arrangement, not a broken build: `cargo
    /// build -p farhelm-desktop` without `FARHELM_UI_DIST` opens a window
    /// with no UI in it. What must NOT happen is a panic, a partial
    /// response, or a revived filesystem fallback — registering the handler
    /// took `/assets/*` away from dioxus's resolver, so anything this
    /// returns is the whole answer.
    ///
    /// Reachable as a test only because the lookup is a parameter: whether
    /// this binary embedded a tree was decided when it was compiled.
    #[farhelm_testtrace::test]
    fn a_build_with_no_embedded_ui_answers_every_asset_with_an_empty_404() {
        for path in ["/assets/terminal-dxhabc.js", "/assets/anything-at-all"] {
            let response = serve_unembedded("GET", path);
            assert_eq!(response.status(), 404, "{path}");
            assert!(response.body().is_empty(), "{path}");
        }
    }

    /// The method gate runs before the embedded-tree question.
    ///
    /// Both orderings answer honestly, but this one keeps the refusal
    /// identical across build configurations: a client asking the wrong way
    /// gets `405` and `Allow` whether or not this build has a UI, rather
    /// than a 404 that suggests the path was the problem.
    #[farhelm_testtrace::test]
    fn a_write_method_is_refused_even_with_no_embedded_ui() {
        let response = serve_unembedded("POST", "/assets/terminal-dxhabc.js");
        assert_eq!(response.status(), 405);
        assert_eq!(response.headers().get("allow").unwrap(), "GET,HEAD");
    }
}
