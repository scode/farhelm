//! UI entry points for both targets.
//!
//! Web (built by `dx build --platform web`, served by the helm): the API
//! base is the page's own origin. Desktop (wry webview): the webview's
//! origin is not the helm. The desktop bootstrap owns an embedded loopback
//! helm and bundled local supervisor, then hands their stable loopback origin
//! to the same component tree the browser uses.
//!
//! A renderer feature (`web` or `desktop`) selects the target. Plain
//! `cargo build`/`clippy` compile with neither so the workspace checks
//! stay one command; in that configuration this binary is inert and
//! says so rather than failing to compile.

#[cfg(any(feature = "web", feature = "desktop"))]
fn main() {
    use farhelm_ui::{ApiBase, App};

    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    let desktop = farhelm_ui::desktop::DesktopBootstrap::start()
        .unwrap_or_else(|error| panic!("desktop bootstrap failed: {error:#}"));

    // reqwest requires absolute URLs even on wasm, so the web build
    // derives its API base from the page's own origin rather than using
    // relative paths — same origin either way, since the helm serves
    // both the UI and the API.
    #[cfg(target_arch = "wasm32")]
    let base = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default();
    #[cfg(all(not(target_arch = "wasm32"), feature = "desktop"))]
    let base = desktop.api_base().to_string();

    let builder = dioxus::LaunchBuilder::new().with_context(ApiBase(base));

    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    let builder = builder.with_context(desktop.webview_bootstrap());

    // Desktop windows need an explicit WindowBuilder, and not only for the
    // title: dioxus-desktop's `Config::new()` marks debug-build windows
    // always-on-top whenever the app is NOT launched through `dx`
    // (`dioxus_cli_config::always_on_top().unwrap_or(true)` — a
    // convenience for `dx serve` development that misfires for a real app
    // started via `cargo run`, leaving the window permanently above
    // everything). `Config::with_window` replaces the default builder
    // wholesale, which discards that always-on-top default along with the
    // "Dioxus App" placeholder title.
    // `with_disable_drag_drop_handler(true)` is the attachments feature's
    // half of this (PLAN_M4.md item 7, SPEC_impl.md's "one concrete thing
    // to check early rather than debug late: wry's own file-drop handling
    // swallows DOM drop events unless configured not to"). Dropping a file
    // into a terminal is intercepted in the PAGE — assets/terminal.js —
    // so the DOM `drop` event has to reach it, and anything that consumes
    // the drag first breaks the headline feature on the desktop build
    // alone, where nothing in CI would notice.
    //
    // The audit trail behind this call, against dioxus-desktop 0.7.9 and
    // wry 0.53.5, since "configured not to" means different things per
    // platform:
    //
    // - Without this, dioxus installs its own `wry` drag-drop handler
    //   (`webview.rs`, gated on `cfg.disable_file_drop_handler`) to feed
    //   its native file-drop support. That handler returns `false`, which
    //   wry reads as "not handled" and answers by invoking the OS default
    //   — so on macOS (`wkwebview/drag_drop.rs` calling `super`) and on
    //   GTK the DOM events do still fire. On Windows they do not: dioxus's
    //   own comment says the WebView2 host blocks HTML-native drag events
    //   whenever a drop handler is present, and its config doc says the
    //   handler must be disabled for the HTML drag and drop APIs to work.
    // - So the setting is not load-bearing on the two platforms Farhelm
    //   targets today, and it is set anyway: it is the difference between
    //   "the DOM path works because a handler we do not want happens to
    //   decline every event" and "nothing is competing for the drag". The
    //   cost is dioxus's native file-drop support, which this UI does not
    //   use — no `ondrop` handler exists anywhere in the component tree,
    //   and the attachment path deliberately reads `File` objects in JS
    //   rather than paths in Rust (see src/attachments.rs).
    //
    // Verifying the CAPABILITY rather than the configuration is the
    // manual desktop pass PLAN_M4.md acceptance 9 records; this call is
    // what that pass is checking the effect of. The checklist that pass
    // has to work through — including the one risk it is most likely to
    // trip over — is written out in `attachments`' module header.
    #[cfg(feature = "desktop")]
    let builder = builder.with_cfg(
        dioxus::desktop::Config::new()
            .with_window(dioxus::desktop::WindowBuilder::new().with_title("farhelm"))
            .with_disable_drag_drop_handler(true),
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
