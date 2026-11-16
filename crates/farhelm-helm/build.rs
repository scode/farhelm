//! Mark a release-shaped build (D13) from the environment the release build sets.
//!
//! `FARHELM_RELEASE_BUILD`, set and not empty, marks a release-shaped build and does nothing else: it sets
//! `farhelm_release_build`, on which the provisioning payload download default depends (`cfg!(farhelm_release_build)`
//! in `src/lib.rs`). Without it a helm, the desktop app's embedded one included, defaults to no host payloads, so
//! adding or updating a remote host is refused unless a payload directory is given. `scripts/build-desktop-binary.sh`
//! refuses to run without it for that reason.
//!
//! The mark used to be the presence of `FARHELM_UI_DIST`, which named the web UI's built files the helm embedded
//! (D12/D13), when a release build was exactly a build that embedded a UI. The web UI is gone (GPUI migration), so the
//! fact has a name of its own. During the transition either name, set and not empty, marks the build, so that the
//! order in which this change and the release workflow's switch to the new name land cannot leave a release build
//! unmarked.
//!
//! NOTE: `FARHELM_UI_DIST` is accepted only until the release workflow (`.github/dist-build-setup.yml`) exports
//! `FARHELM_RELEASE_BUILD`; remove it then. `scripts/build-desktop-binary.sh` already requires the new name.
//!
//! Provisioning payloads (the `farhelm` binaries hosts are sent) are not embedded either (D2): they are downloaded or
//! staged from a directory at runtime — see `src/provisioning/payloads.rs`.

/// The variables whose presence marks a release-shaped build: the name, then the old name kept for the transition.
/// See the module doc.
const RELEASE_ENVS: [&str; 2] = ["FARHELM_RELEASE_BUILD", "FARHELM_UI_DIST"];

fn main() {
    for name in RELEASE_ENVS {
        println!("cargo:rerun-if-env-changed={name}");
    }
    // Declared unconditionally so `#[cfg(farhelm_release_build)]` raises no unexpected-cfg warning on ordinary
    // development builds, which leave the variables unset.
    println!("cargo:rustc-check-cfg=cfg(farhelm_release_build)");
    if RELEASE_ENVS
        .iter()
        .any(|name| std::env::var_os(name).is_some_and(|value| !value.is_empty()))
    {
        println!("cargo:rustc-cfg=farhelm_release_build");
    }
}
