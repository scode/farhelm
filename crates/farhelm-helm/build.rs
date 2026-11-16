//! Mark a release-shaped build (D13) from the environment the release build sets.
//!
//! `FARHELM_RELEASE_BUILD`, set and not empty, marks a release-shaped build and does nothing else: it sets
//! `farhelm_release_build`, on which the provisioning payload download default depends (`cfg!(farhelm_release_build)`
//! in `src/lib.rs`). Without it a helm, the desktop app's embedded one included, defaults to no host payloads, so
//! adding or updating a remote host is refused unless a payload directory is given. The release workflow exports it for
//! every build job (`.github/dist-build-setup.yml`), and `scripts/build-desktop-binary.sh` refuses to run without it.
//!
//! The mark used to be the presence of `FARHELM_UI_DIST`, which named the web UI's built files the helm embedded
//! (D12/D13), when a release build was exactly a build that embedded a UI. The web UI is gone (GPUI migration), so the
//! fact has a name of its own.
//!
//! Provisioning payloads (the `farhelm` binaries hosts are sent) are not embedded either (D2): they are downloaded or
//! staged from a directory at runtime — see `src/provisioning/payloads.rs`.

/// The variable whose presence marks a release-shaped build. See the module doc.
const RELEASE_ENV: &str = "FARHELM_RELEASE_BUILD";

fn main() {
    println!("cargo:rerun-if-env-changed={RELEASE_ENV}");
    // Declared unconditionally so `#[cfg(farhelm_release_build)]` raises no unexpected-cfg warning on ordinary
    // development builds, which leave the variable unset.
    println!("cargo:rustc-check-cfg=cfg(farhelm_release_build)");
    if std::env::var_os(RELEASE_ENV).is_some_and(|value| !value.is_empty()) {
        println!("cargo:rustc-cfg=farhelm_release_build");
    }
}
