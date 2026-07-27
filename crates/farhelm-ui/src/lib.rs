//! The Farhelm UI: one Dioxus component tree, two targets.
//!
//! Per SPEC_impl.md, the same components render as the web app (wasm32,
//! real DOM, served by the helm) and the desktop app (wry webview). The
//! terminal itself is an xterm.js island whose byte path bypasses this
//! crate's reactivity entirely.
//!
//! NOTE: deliberately dependency-free in M0 — pulling Dioxus into the
//! stub would slow every CI run before any UI exists. The dependency
//! arrives with the first component in M1.

/// Placeholder for the M0 CI pipeline; replaced by real modules in M1.
pub fn crate_name() -> &'static str {
    "farhelm-ui"
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exists so `cargo test` compiles and exercises this crate from the
    /// first CI run; replaced by real tests as M1 lands functionality.
    #[test]
    fn stub_compiles_and_runs() {
        assert_eq!(crate_name(), "farhelm-ui");
    }
}
