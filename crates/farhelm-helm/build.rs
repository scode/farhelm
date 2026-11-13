//! Mark a release-shaped build (D13) from the environment the release workflow sets.
//!
//! The helm once embedded the web UI's built files when `FARHELM_UI_DIST` named them (D12/D13), and a release build
//! was exactly a build that embedded a UI. The web UI is gone (GPUI migration), so nothing is embedded any more, but the
//! release workflow (`.github/dist-build-setup.yml`, release machinery that is left untouched until the cutover) still
//! exports the variable for every release build, and later steps still need to know that a build is release-shaped:
//! the provisioning payload download default depends on it (`cfg!(farhelm_release_build)` in `src/lib.rs`). So the
//! variable's presence keeps setting `farhelm_release_build` and does nothing else; the cutover gives that fact a name
//! of its own.
//!
//! Provisioning payloads (the `farhelm` binaries hosts are sent) are not embedded either (D2): they are downloaded or
//! staged from a directory at runtime — see `src/provisioning/payloads.rs`.

/// The variable whose presence marks a release-shaped build. See the module doc.
const RELEASE_ENV: &str = "FARHELM_UI_DIST";

fn main() {
    println!("cargo:rerun-if-env-changed={RELEASE_ENV}");
    // Declared unconditionally so `#[cfg(farhelm_release_build)]` raises no unexpected-cfg warning on ordinary
    // development builds, which leave the variable unset.
    println!("cargo:rustc-check-cfg=cfg(farhelm_release_build)");
    if std::env::var_os(RELEASE_ENV).is_some_and(|value| !value.is_empty()) {
        println!("cargo:rustc-cfg=farhelm_release_build");
    }
}
