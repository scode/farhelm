//! UI entry points for both targets.
//!
//! Web (built by `dx build --platform web`, served by the helm): the API
//! base is the page's own origin. Desktop (wry webview): the webview's
//! origin is not the helm, so the base comes from FARHELM_URL (default
//! http://127.0.0.1:7433) and the window is chrome around the same
//! components. M1's desktop is deliberately a thin client — see
//! lore/2026-07-26-m1-desktop-is-a-thin-client.md.
//!
//! A renderer feature (`web` or `desktop`) selects the target. Plain
//! `cargo build`/`clippy` compile with neither so the workspace checks
//! stay one command; in that configuration this binary is inert and
//! says so rather than failing to compile.

#[cfg(any(feature = "web", feature = "desktop"))]
fn main() {
    use farhelm_ui::{ApiBase, App};

    // reqwest requires absolute URLs even on wasm, so the web build
    // derives its API base from the page's own origin rather than using
    // relative paths — same origin either way, since the helm serves
    // both the UI and the API.
    #[cfg(target_arch = "wasm32")]
    let base = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default();
    #[cfg(not(target_arch = "wasm32"))]
    let base = std::env::var("FARHELM_URL").unwrap_or_else(|_| "http://127.0.0.1:7433".to_string());

    // The helm prints its URL with a trailing slash, and pasting that
    // into FARHELM_URL is the obvious move — an untrimmed base would
    // yield "//api/..." paths that miss the routes and break the
    // terminal WebSocket. Origins from the browser come without one,
    // so this is a no-op on the web path.
    let base = base.trim_end_matches('/').to_string();

    dioxus::LaunchBuilder::new()
        .with_context(ApiBase(base))
        .launch(App);
}

#[cfg(not(any(feature = "web", feature = "desktop")))]
fn main() {
    eprintln!(
        "farhelm-ui was built without a renderer feature.\n\
         Web:     cd crates/farhelm-ui && dx build --platform web\n\
         Desktop: cargo run -p farhelm-ui --features desktop"
    );
    std::process::exit(1);
}
