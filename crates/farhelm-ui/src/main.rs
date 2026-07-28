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

    let builder = dioxus::LaunchBuilder::new().with_context(ApiBase(base));

    // Desktop windows need an explicit WindowBuilder, and not only for the
    // title: dioxus-desktop's `Config::new()` marks debug-build windows
    // always-on-top whenever the app is NOT launched through `dx`
    // (`dioxus_cli_config::always_on_top().unwrap_or(true)` — a
    // convenience for `dx serve` development that misfires for a real app
    // started via `cargo run`, leaving the window permanently above
    // everything). `Config::with_window` replaces the default builder
    // wholesale, which discards that always-on-top default along with the
    // "Dioxus App" placeholder title.
    #[cfg(feature = "desktop")]
    let builder = builder.with_cfg(
        dioxus::desktop::Config::new()
            .with_window(dioxus::desktop::WindowBuilder::new().with_title("farhelm")),
    );

    builder.launch(App);
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
