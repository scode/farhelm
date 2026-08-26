//! UI entry points for both targets.
//!
//! Web (built by `dx build --platform web`, served by the helm): the API
//! base is the page's own origin. Desktop (wry webview): the webview's
//! origin is not the helm, so the whole shell — the loopback helm embedded in
//! this process, the managed local supervisor discovered or spawned beside
//! it, and the asset handler that feeds the window — lives in
//! [`farhelm_ui::desktop::run`], and this file is only the door to it.
//!
//! ## Why the desktop half is not written here
//!
//! D6 ships the desktop app as `crates/farhelm-desktop`, a bare binary that
//! must behave identically to what `dx build --platform desktop` produces
//! from THIS bin (the smoke script's subject, and the maintainer's dev loop).
//! Two `main`s cannot be kept identical by discipline, so there is one body
//! in the library and two callers of it.
//!
//! A renderer feature (`web` or `desktop`) selects the target. Plain
//! `cargo build`/`clippy` compile with neither so the workspace checks
//! stay one command; in that configuration this binary is inert and
//! says so rather than failing to compile.

/// Desktop: hand off immediately. Everything this used to do inline now
/// lives in `desktop::run`, which `crates/farhelm-desktop` calls too.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
fn main() -> anyhow::Result<()> {
    farhelm_ui::desktop::run()
}

/// Web (wasm32): launch straight into the component tree.
///
/// The cfg subtracts the desktop arm's condition rather than testing `web`
/// alone, so that whatever features are enabled, exactly one `main` is
/// compiled for a given target: with both features on, a native build takes
/// the desktop arm above and a wasm build takes this one, since the desktop
/// arm excludes wasm outright.
#[cfg(all(
    feature = "web",
    not(all(feature = "desktop", not(target_arch = "wasm32")))
))]
fn main() {
    use farhelm_ui::{ApiBase, App};

    // reqwest requires absolute URLs even on wasm, so the web build
    // derives its API base from the page's own origin rather than using
    // relative paths — same origin either way, since the helm serves
    // both the UI and the API.
    //
    // Deliberately defined only for wasm32, with no native arm: `web` is a
    // wasm-only renderer, and `--features web` on a native target should
    // keep failing to compile here (as it always has) rather than launch a
    // window pointed at an empty origin.
    #[cfg(target_arch = "wasm32")]
    let base = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default();

    dioxus::LaunchBuilder::new()
        .with_context(ApiBase(base))
        .launch(App);
}

/// No renderer: say how to get one, and be exact about it.
///
/// Every command below has a trap that cost real time to find, so none is the
/// obvious spelling. dx needs `--package` because the workspace has
/// `default-members` and dx resolves those relative paths against its own
/// working directory. Cargo needs `-p` because the root manifest is a virtual
/// workspace with several binaries, so `cargo run` alone stops at an
/// ambiguity error. And even spelled correctly, `cargo run -p farhelm-ui
/// --features desktop` opens a window with no assets in it — Cargo does not
/// perform dx's post-link rewrite of the `asset!()` names — which is why the
/// desktop line points at the two things that produce a working app rather
/// than at a command that merely starts.
#[cfg(not(any(feature = "web", feature = "desktop")))]
fn main() {
    eprintln!(
        "farhelm-ui was built without a renderer feature.\n\
         Web:     cd crates/farhelm-ui && dx build --package farhelm-ui --platform web\n\
         Desktop: scripts/desktop-smoke.sh runs one under Xvfb; see\n\
        \x20        crates/farhelm-desktop/README.md to build one by hand.\n\
         (cargo run -p farhelm-ui --features desktop builds, but opens a window with no assets.)"
    );
    std::process::exit(1);
}
