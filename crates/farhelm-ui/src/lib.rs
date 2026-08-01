//! The Farhelm UI: one Dioxus component tree, two targets.
//!
//! The same components render as the web app (wasm32, real DOM, served
//! by the helm at loopback) and the desktop app (wry webview). The
//! terminal itself is an xterm.js island (assets/terminal.js) whose byte
//! path bypasses Dioxus entirely — Dioxus owns the chrome around the
//! terminal, never its content (SPEC_impl.md, "Terminal widget").
//!
//! Data fetching uses reqwest, which works on both native (desktop) and
//! wasm (browser fetch) — one code path, no per-target HTTP client.

use dioxus::prelude::*;
use serde::Deserialize;

/// Absolute origin of the helm's HTTP/WS API, e.g.
/// `http://127.0.0.1:7433`.
///
/// Both targets carry a real origin rather than a relative path: reqwest
/// requires absolute URLs even on wasm, so the web build reads the
/// page's own origin (it is served by the helm) and the desktop build
/// takes `FARHELM_URL`, since a wry webview's origin is not the helm.
#[derive(Clone, PartialEq)]
pub struct ApiBase(pub String);

/// Mirror of the helm's session JSON (farhelm-proto SessionInfo). Kept
/// as a local type so the UI depends on the HTTP contract, not on proto
/// internals — the browser speaks JSON, not frames.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub cwd: String,
    pub invocation: String,
}

const VENDOR_XTERM_CSS: Asset = asset!("/assets/vendor/xterm.css");
const VENDOR_XTERM_JS: Asset = asset!("/assets/vendor/xterm.js");
const VENDOR_FIT_JS: Asset = asset!("/assets/vendor/addon-fit.js");
const TERMINAL_JS: Asset = asset!("/assets/terminal.js");
const APP_CSS: Asset = asset!("/assets/app.css");

/// Root component. M1 shows the (single) session full-window; the
/// session list UI proper is M2 — but the data flow already goes
/// through GET /api/sessions so M2 grows out of this rather than
/// replacing it.
#[component]
pub fn App() -> Element {
    let base = use_context::<ApiBase>().0;
    let sessions = use_resource(move || {
        let base = base.clone();
        async move { fetch_sessions(&base).await }
    });

    rsx! {
        document::Link { rel: "stylesheet", href: VENDOR_XTERM_CSS }
        document::Link { rel: "stylesheet", href: APP_CSS }
        document::Script { src: VENDOR_XTERM_JS }
        document::Script { src: VENDOR_FIT_JS }
        document::Script { src: TERMINAL_JS }
        match &*sessions.read_unchecked() {
            None => rsx! { div { class: "status", "loading sessions…" } },
            Some(Err(e)) => rsx! {
                div { class: "status error", "failed to load sessions: {e}" }
            },
            Some(Ok(list)) => match list.first() {
                None => rsx! { div { class: "status", "no sessions" } },
                Some(session) => rsx! { SessionView { session: session.clone() } },
            },
        }
    }
}

/// Mirror of the helm's `GET /api/sessions` response body (farhelm-helm's
/// `SessionListing`, PLAN_M2.md step 6): `{"sessions": [...], "total": N,
/// "truncated": bool}`. A local type for the same reason `Session` is one
/// — the UI depends on the HTTP contract, not on `farhelm-helm` internals
/// — and, like `Session`, only `sessions` is read today; the list UI that
/// displays "showing N of M" from `total`/`truncated` is the next PR.
#[derive(Deserialize)]
struct SessionListing {
    sessions: Vec<Session>,
}

/// Fetch the session list, flattening every failure into a displayable
/// string.
///
/// The string is not laziness: this value is what `use_resource` stores
/// and the component matches on, so it has to be `Clone + PartialEq` for
/// Dioxus to diff it — and `reqwest::Error` is neither. The message is
/// rendered to the user directly (SPEC.md wants concrete errors), which is
/// why the URL and status go into it rather than being logged and dropped.
async fn fetch_sessions(base: &str) -> Result<Vec<Session>, String> {
    let url = format!("{base}/api/sessions");
    let resp = reqwest::get(&url).await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp
            .text()
            .await
            .map_err(|error| format!("GET {url}: {status}: reading error response: {error}"))?;
        let detail = detail.trim();
        return Err(if detail.is_empty() {
            format!("GET {url}: {status}")
        } else {
            format!("GET {url}: {status}: {detail}")
        });
    }
    // The response is the full listing object now (PLAN_M2.md step 6), not
    // a bare array — only `.sessions` is used here; `total`/`truncated`
    // wait for the list-UI PR that actually displays truncation.
    let listing = resp
        .json::<SessionListing>()
        .await
        .map_err(|e| e.to_string())?;
    Ok(sort_sessions(listing.sessions))
}

/// Stabilize M1's single-session choice across the protocol's undefined
/// list order. This helper becomes unnecessary when M2 renders the list,
/// but until then reloading must not pick an arbitrary different session.
fn sort_sessions(mut sessions: Vec<Session>) -> Vec<Session> {
    // The wire order is a HashMap's ("no defined order" per the proto
    // docs), and M1 shows `first()` full-window — without a sort, WHICH
    // session the browser shows could change on every reload once a
    // supervisor holds more than one (re-running the README's helm
    // command against a live supervisor does exactly that).
    sessions.sort_by(|a, b| a.id.cmp(&b.id));
    sessions
}

/// One session, terminal filling the window. The terminal div is handed
/// to the JS island on mount; Dioxus never touches its children again —
/// that boundary is the whole design.
#[component]
fn SessionView(session: Session) -> Element {
    let base = use_context::<ApiBase>().0;
    let session_id = session.id.clone();

    use_effect(move || {
        // Values are JSON-encoded, never interpolated raw: the session
        // id comes from a supervisor, which with --ssh is a different
        // machine. A hostile or compromised host returning an id
        // containing a quote would otherwise get arbitrary JavaScript
        // running on the helm's origin — turning a remote-host
        // compromise into control of the local helm API.
        let path = serde_json::to_string(&format!("/api/sessions/{session_id}/term"))
            .expect("string is serializable");
        let base_js = serde_json::to_string(&base).expect("string is serializable");
        // The vendor scripts are injected asynchronously by the
        // document::Script components above, so mounting must wait for
        // the globals to exist — firing on first render would race the
        // script loads. The island guards against double-mount itself.
        let js = format!(
            r#"(function tryMount() {{
                if (window.farhelmTerm && window.Terminal && window.FitAddon
                    && document.getElementById('terminal')) {{
                    farhelmTerm.mount('terminal', {path}, {base_js});
                }} else {{
                    setTimeout(tryMount, 50);
                }}
            }})();"#
        );
        document::eval(&js);
    });

    rsx! {
        div { class: "layout",
            header { class: "titlebar",
                span { class: "title", "{session.title}" }
                span { class: "meta", "{session.cwd} — {session.invocation}" }
            }
            div { id: "term-banner", class: "banner" }
            div { id: "terminal", class: "terminal" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// M1 renders only the first session, so the selection must be stable
    /// even though the supervisor's HashMap-backed wire order is not.
    /// Reversing the response must not change which session wins.
    #[test]
    fn session_selection_is_stable_across_wire_orders() {
        let a = Session {
            id: "a".into(),
            title: "A".into(),
            cwd: "/a".into(),
            invocation: "agent-a".into(),
        };
        let b = Session {
            id: "b".into(),
            title: "B".into(),
            cwd: "/b".into(),
            invocation: "agent-b".into(),
        };

        let forward = sort_sessions(vec![a.clone(), b.clone()]);
        let reverse = sort_sessions(vec![b, a]);
        assert_eq!(
            forward.first().map(|session| session.id.as_str()),
            Some("a")
        );
        assert_eq!(
            reverse.first().map(|session| session.id.as_str()),
            Some("a")
        );
    }
}
