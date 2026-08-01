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
//!
//! ## Navigation (M2, PLAN_M2.md step 7)
//!
//! `App` holds `Signal<Option<Session>>` rather than pulling in a router
//! crate: `None` renders [`ListView`], `Some(session)` renders
//! [`SessionView`] plus a back control that clears the signal.
//! PLAN_M2.md names a premature router as a risk this milestone
//! deliberately avoids — two states and one signal cover everything M2
//! needs, and a router can still be introduced later if M4's terminal
//! tabs (or something else) actually demands one.

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

/// Mirror of the helm's session status JSON (farhelm-proto
/// `SessionStatus`). Kept local for the same reason `Session` is — the UI
/// depends on the HTTP contract, not on proto internals.
///
/// `#[serde(default)]` on every `Session::status` field (below) is what
/// makes an old-shaped reply — one with no `status` at all — decode as
/// `Unknown` rather than fail; this mirrors `SessionStatus`'s own
/// wire-tolerance contract in farhelm-proto. `#[default]` on the
/// `Unknown` variant is what backs that: a reply that predates this
/// field must decode as "not yet known", never as a fabricated liveness
/// claim in either direction.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SessionStatus {
    /// Not yet known one way or the other — never a guess. See
    /// farhelm-proto's `SessionStatus::Unknown` for the full rationale;
    /// this mirror exists only to give the UI something to match on.
    #[default]
    Unknown,
    /// The agent's process is running.
    Alive,
    /// The agent's process has ended. `exit_code` is `None` when tmux
    /// could not reduce the death to a plain code (a signal, or no live
    /// pane to ask at all).
    Exited { exit_code: Option<i32> },
}

/// Mirror of the helm's session JSON (farhelm-proto `SessionInfo`). Kept
/// as a local type so the UI depends on the HTTP contract, not on proto
/// internals — the browser speaks JSON, not frames.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub cwd: String,
    pub invocation: String,
    #[serde(default)]
    pub status: SessionStatus,
}

const VENDOR_XTERM_CSS: Asset = asset!("/assets/vendor/xterm.css");
const VENDOR_XTERM_JS: Asset = asset!("/assets/vendor/xterm.js");
const VENDOR_FIT_JS: Asset = asset!("/assets/vendor/addon-fit.js");
const TERMINAL_JS: Asset = asset!("/assets/terminal.js");
const APP_CSS: Asset = asset!("/assets/app.css");

/// Root component: switches between the session list and one open
/// terminal. No router crate (see the module docs) — just a signal.
#[component]
pub fn App() -> Element {
    let mut current = use_signal(|| None::<Session>);

    rsx! {
        document::Link { rel: "stylesheet", href: VENDOR_XTERM_CSS }
        document::Link { rel: "stylesheet", href: APP_CSS }
        document::Script { src: VENDOR_XTERM_JS }
        document::Script { src: VENDOR_FIT_JS }
        document::Script { src: TERMINAL_JS }
        match &*current.read() {
            None => rsx! {
                ListView { on_open: move |session| current.set(Some(session)) }
            },
            Some(session) => rsx! {
                SessionView {
                    session: session.clone(),
                    on_back: move |_| current.set(None),
                }
            },
        }
    }
}

/// Mirror of the helm's `GET /api/sessions` response body (farhelm-helm's
/// `SessionListing`, PLAN_M2.md step 6): `{"sessions": [...], "total": N,
/// "truncated": bool}`. `total`/`truncated` are `#[serde(default)]` for
/// the same old-peer tolerance as `Session::status` above; a decoder that
/// only ever talks to this build's own helm will never hit that path, but
/// nothing here should assume that. `Deserialize` is the only derive this
/// needs: it is read once by reference out of the signal that holds it
/// (see `fetch_sessions`'s docs), never cloned or compared.
#[derive(Deserialize)]
struct SessionListing {
    sessions: Vec<Session>,
    #[serde(default)]
    total: u64,
    #[serde(default)]
    truncated: bool,
}

/// Fetch the session listing, flattening every failure into a displayable
/// string.
///
/// The message is a `String`, not `reqwest::Error`, because it is
/// rendered to the user directly (SPEC.md wants concrete errors) — the
/// URL and status are folded into the message here rather than logged
/// and dropped. That is the whole reason, not a Dioxus constraint:
/// `Signal<T>` only requires `T: 'static` to hold a value, nothing about
/// `Clone` or `PartialEq`.
async fn fetch_sessions(base: &str) -> Result<SessionListing, String> {
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
    let mut listing = resp
        .json::<SessionListing>()
        .await
        .map_err(|e| e.to_string())?;
    listing.sessions = sort_sessions(listing.sessions);
    Ok(listing)
}

/// Stabilize the list's display order across the protocol's undefined
/// wire order.
///
/// The wire order is a HashMap's ("no defined order" per the proto
/// docs), so without a sort the list would visibly reshuffle rows on
/// every poll even when nothing changed — distracting at best, and
/// actively confusing for a "did status flip?" glance. `id` sort was M1's
/// stopgap for picking a stable `first()`; it survives into M2 as the
/// list's actual display order (its doc previously called it
/// unnecessary once M2 landed a real list — that turned out to be wrong,
/// since a list still needs SOME deterministic order) and stays until
/// something better — most likely creation time — is wanted.
fn sort_sessions(mut sessions: Vec<Session>) -> Vec<Session> {
    sessions.sort_by(|a, b| a.id.cmp(&b.id));
    sessions
}

/// How often the list view refetches (PLAN_M2.md: "Polling for list
/// freshness" is the M2 mechanism; live push is out of scope until M5).
const POLL_INTERVAL_MS: u64 = 3_000;

/// The flat session list: title, cwd, invocation, and a truthful status
/// badge per row, refetched on a timer.
///
/// The poll loop lives in a `use_future` scoped to this component, so it
/// is cancelled for free when `App` switches to `SessionView` and this
/// component unmounts — "polling stops while a terminal is open"
/// (PLAN_M2.md) falls out of Dioxus's own task lifecycle rather than
/// needing an explicit stop signal.
#[component]
fn ListView(on_open: EventHandler<Session>) -> Element {
    let base = use_context::<ApiBase>().0;
    let mut listing = use_signal(|| None::<Result<SessionListing, String>>);

    use_future(move || {
        let base = base.clone();
        async move {
            loop {
                listing.set(Some(fetch_sessions(&base).await));
                // Inlined rather than a shared `sleep_ms` helper: this is
                // the only call site, and `tokio::time::sleep` is
                // unavailable on wasm32 (no reactor in the browser) while
                // `gloo-timers`' `TimeoutFuture` only works on wasm32 (a
                // `wasm-bindgen` binding to `setTimeout`) — each target
                // gets the idiom that already fits it. The desktop build
                // runs inside the tokio multi-thread runtime
                // `dioxus-desktop` itself constructs (see its
                // `launch.rs`), so `tokio::time::sleep` needs no extra
                // setup there.
                #[cfg(target_arch = "wasm32")]
                gloo_timers::future::TimeoutFuture::new(POLL_INTERVAL_MS as u32).await;
                #[cfg(not(target_arch = "wasm32"))]
                tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await;
            }
        }
    });

    rsx! {
        match &*listing.read() {
            None => rsx! { div { class: "status", "loading sessions…" } },
            Some(Err(e)) => rsx! {
                div { class: "status error", "failed to load sessions: {e}" }
            },
            Some(Ok(listing)) => rsx! {
                if listing.sessions.is_empty() && listing.total == 0 {
                    div { class: "status", "no sessions" }
                } else {
                    if listing.truncated {
                        // PLAN_M2.md acceptance 5: the cap and truncated
                        // flag exist to be shown, not just plumbed —
                        // silently presenting a partial list would look
                        // like a complete one.
                        div { class: "banner truncation-banner",
                            "showing {listing.sessions.len()} of {listing.total} sessions"
                        }
                    }
                    div { class: "session-list",
                        for session in listing.sessions.iter() {
                            SessionRow {
                                key: "{session.id}",
                                session: session.clone(),
                                on_open,
                            }
                        }
                    }
                }
            },
        }
    }
}

/// One row: a native `<button>`, not a `div` with `role`/`tabindex`/a
/// hand-rolled `onkeydown` — a real button gets Enter- and
/// Space-activation, focus styling, and screen-reader semantics for
/// free, and does not need this component to reimplement any of it
/// (the div version also had a latent bug: Space on a focused element
/// scrolls the page unless the handler prevents default, which a
/// button's native activation never triggers in the first place).
/// `data-session-id` stays for Playwright to key off of.
#[component]
fn SessionRow(session: Session, on_open: EventHandler<Session>) -> Element {
    let (badge_class, badge_text) = status_badge(&session.status);
    let click_session = session.clone();

    rsx! {
        button {
            r#type: "button",
            class: "session-row",
            "data-session-id": "{session.id}",
            onclick: move |_| on_open.call(click_session.clone()),
            span { class: "session-title", "{session.title}" }
            span { class: "session-cwd", "{session.cwd}" }
            span { class: "session-invocation", "{session.invocation}" }
            span { class: "status-badge {badge_class}", "{badge_text}" }
        }
    }
}

/// Map a status to its badge's CSS modifier class and display text.
/// Kept as one function so the four cases (`Exited` has two — a known
/// code vs. an unrepresentable one) stay next to each other instead of
/// drifting apart across separate match arms in the render tree.
fn status_badge(status: &SessionStatus) -> (&'static str, String) {
    match status {
        SessionStatus::Alive => ("alive", "alive".to_string()),
        SessionStatus::Exited {
            exit_code: Some(code),
        } => ("exited", format!("exited (code {code})")),
        SessionStatus::Exited { exit_code: None } => ("exited", "exited".to_string()),
        SessionStatus::Unknown => ("unknown", "unknown".to_string()),
    }
}

/// One session, terminal filling the window, with a back control above
/// it. The terminal div is handed to the JS island on mount; Dioxus
/// never touches its children again — that boundary is the whole
/// design.
///
/// ## Mount/unmount lifecycle (PLAN_M2.md step 7)
///
/// M1 never unmounted this component (it was the only view), so
/// `terminal.js` only ever needed a mount-time guard against a
/// re-render calling `mount()` twice. Now that `App` can navigate away
/// and back, this component must clean up on drop — otherwise that
/// guard (terminal.js's `active`, now the ONLY mount guard — see its
/// own docs) would permanently wedge shut, and reopening ANY session
/// after the first would silently no-op `mount()` instead of attaching.
/// `use_drop` fires `farhelmTerm.unmount()` for exactly that reason: it
/// is the regression this lifecycle work exists to prevent, not a
/// hypothetical.
///
/// ## The mount-generation token
///
/// `window.__farhelmMountGeneration` guards only the OUTER wait here —
/// for `window.farhelmTerm` to exist at all, since terminal.js's
/// `document::Script` is injected asynchronously and this component can
/// render before it has executed (a real, if rare, race). Bumping the
/// counter on every mount attempt AND on drop means backing out before
/// that wait resolves reliably cancels it. Once `window.farhelmTerm`
/// exists, `mountWhenReady`'s OWN wait for xterm's globals is a separate
/// concern guarded entirely inside terminal.js (its `pending`
/// replacement-and-clear scheme — see that function's docs); this token
/// does not reach that far.
#[component]
fn SessionView(session: Session, on_back: EventHandler<()>) -> Element {
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
        let js = format!(
            r#"(function() {{
                var gen = (window.__farhelmMountGeneration || 0) + 1;
                window.__farhelmMountGeneration = gen;
                (function waitForIsland() {{
                    if (window.__farhelmMountGeneration !== gen) return;
                    if (window.farhelmTerm) {{
                        farhelmTerm.mountWhenReady('terminal', {path}, {base_js});
                    }} else {{
                        setTimeout(waitForIsland, 50);
                    }}
                }})();
            }})();"#
        );
        document::eval(&js);
    });

    use_drop(|| {
        // Bump the generation FIRST so an in-flight outer wait (see the
        // module docs above) aborts before `unmount()` below can race
        // it; `unmount()` itself is what cancels `mountWhenReady`'s own
        // wait, once the outer one has already handed off to it.
        // Fire-and-forget: this runs on the way out (navigating back to
        // the list, or the whole app tearing down), so there is no
        // reactive state left to update with a result, and the JS side
        // is already written to be a no-op if nothing is mounted.
        document::eval(
            "window.__farhelmMountGeneration = (window.__farhelmMountGeneration || 0) + 1; \
             if (window.farhelmTerm) { farhelmTerm.unmount(); }",
        );
    });

    rsx! {
        div { class: "layout",
            header { class: "titlebar",
                button {
                    class: "back-button",
                    onclick: move |_| on_back.call(()),
                    "← back",
                }
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

    fn session(id: &str, title: &str) -> Session {
        Session {
            id: id.into(),
            title: title.into(),
            cwd: format!("/{id}"),
            invocation: format!("agent-{id}"),
            status: SessionStatus::Unknown,
        }
    }

    /// The list's display order must be stable even though the
    /// supervisor's HashMap-backed wire order is not (M1 regression
    /// this helper originally existed to prevent for `first()`; it
    /// carries the same requirement for the whole list now). Reversing
    /// the response must not change the rendered order.
    #[test]
    fn session_order_is_stable_across_wire_orders() {
        let a = session("a", "A");
        let b = session("b", "B");

        let forward = sort_sessions(vec![a.clone(), b.clone()]);
        let reverse = sort_sessions(vec![b, a]);
        assert_eq!(
            forward.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            vec!["a", "b"]
        );
        assert_eq!(
            reverse.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }

    /// Pins BOTH the badge's display text and its CSS modifier class per
    /// status — not just the text — since a class regression (e.g. an
    /// `Exited` row silently keeping the `alive` class) would only
    /// otherwise surface as a wrong-COLORED row in the browser, which no
    /// text-only assertion here would ever catch.
    #[test]
    fn status_badge_matches_text_and_class_for_each_status() {
        assert_eq!(
            status_badge(&SessionStatus::Alive),
            ("alive", "alive".to_string())
        );
        assert_eq!(
            status_badge(&SessionStatus::Exited { exit_code: Some(7) }),
            ("exited", "exited (code 7)".to_string())
        );
        assert_eq!(
            status_badge(&SessionStatus::Exited { exit_code: None }),
            ("exited", "exited".to_string())
        );
        assert_eq!(
            status_badge(&SessionStatus::Unknown),
            ("unknown", "unknown".to_string())
        );
    }

    /// An old-shaped `Session` JSON (no `status` field at all — exactly
    /// what a pre-M2 peer would send) must decode as `Unknown`, mirroring
    /// farhelm-proto's own decode-tolerance contract for
    /// `SessionInfo::status`. A silent default of, say, `Alive` would be
    /// a fabricated liveness claim.
    #[test]
    fn session_without_status_field_decodes_as_unknown() {
        let json = serde_json::json!({
            "id": "s1",
            "title": "demo",
            "cwd": "/tmp",
            "invocation": "agent",
        });
        let decoded: Session = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.status, SessionStatus::Unknown);
    }

    /// The other half of `SessionListing`'s missing-field tolerance
    /// (mirrors `session_without_status_field_decodes_as_unknown` above,
    /// and farhelm-proto's own
    /// `old_shape_session_list_json_decodes_with_defaulted_new_fields`):
    /// a reply with no `total`/`truncated` keys at all must decode with
    /// both defaulted to their empty-safe values (0 and `false`) rather
    /// than failing.
    ///
    /// NOT a claim that this covers talking to an actual pre-M2 helm:
    /// the real M1-to-M2 change to `GET /api/sessions` replaced a bare
    /// JSON array with this object entirely (farhelm-helm's own docs
    /// call it "a breaking shape change from M1's bare array"), which
    /// `SessionListing` cannot decode either way — there is no array
    /// fallback here. What this pins is forward tolerance WITHIN the
    /// object shape: a later field added to `SessionListing` must still
    /// let a build one step behind decode today's object, the same
    /// tolerance farhelm-proto's own wire types carry.
    #[test]
    fn session_listing_without_total_or_truncated_defaults_both() {
        let json = serde_json::json!({ "sessions": [] });
        let decoded: SessionListing = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.total, 0);
        assert!(!decoded.truncated);
    }
}
