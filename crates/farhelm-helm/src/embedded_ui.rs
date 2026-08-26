//! The web UI tree a release build bakes into this binary (D12/D13).
//!
//! `build.rs` decides, once per build, whether `FARHELM_UI_DIST` is set to a
//! directory whose `index.html` is a regular file — the only check it
//! performs; it does not otherwise validate the tree, so a directory that
//! passes can still be missing assets `index.html` itself references. When
//! it passes, `build.rs` turns on the `farhelm_embedded_ui` cfg, exports the
//! directory's canonical path as the `FARHELM_EMBEDDED_UI_DIR` compile-time
//! environment variable (via `cargo:rustc-env`, not a Rust string literal —
//! see `build.rs`'s own comment on why that distinction matters for a path
//! that might contain `\`, `"`, or `$`), and writes `$OUT_DIR/embedded_ui.rs`
//! holding one FIXED line: `include_dir::include_dir!("$FARHELM_EMBEDDED_UI_DIR")`.
//! That literal never changes between builds — only the environment variable
//! it substitutes does — so nothing here actually requires generating source
//! at build time; a `#[cfg(farhelm_embedded_ui)]`-gated `static` with that
//! same literal, hand-written in this file, would compile just as well. The
//! generated file is kept anyway so the whole embedding decision — whether a
//! UI is embedded at all, and from where — lives entirely in `build.rs`,
//! with this file staying a thin, cfg-oblivious accessor over whatever
//! `build.rs` decided.
include!(concat!(env!("OUT_DIR"), "/embedded_ui.rs"));

/// This build's compiled-in UI tree, or `None` for an ordinary developer
/// build that left `FARHELM_UI_DIST` unset.
///
/// [`crate::select_ui_source`] combines this with the runtime `--ui-dist`
/// flag to pick what [`crate::build_router`] actually serves — see that
/// function for the precedence. `farhelm-desktop` links `farhelm-helm` and
/// will call this too once its shell is wired up (a later step in the
/// distribution plan this crate is being built toward), so it carries the
/// same embedded tree rather than shipping its own copy; it does not call it
/// yet.
pub fn embedded_ui() -> Option<&'static include_dir::Dir<'static>> {
    #[cfg(farhelm_embedded_ui)]
    {
        Some(&UI)
    }
    #[cfg(not(farhelm_embedded_ui))]
    {
        None
    }
}
