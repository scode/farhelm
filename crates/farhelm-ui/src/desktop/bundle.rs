//! Where the desktop app finds what it needs outside itself: its state
//! directory, the `farhelm` CLI installed beside it, and a developer's UI
//! tree for the embedded helm.

use super::*;

pub(super) fn desktop_state_dir() -> anyhow::Result<PathBuf> {
    match std::env::var_os("FARHELM_DESKTOP_STATE_DIR") {
        Some(path) => Ok(PathBuf::from(path)),
        None => farhelm_supervisor::default_state_dir(),
    }
}

/// Locate the `farhelm` CLI this app spawns its supervisor from.
///
/// D6 ships two bare binaries that install side by side (`~/.local/bin`), so
/// "next to me" is the whole contract — there is no bundle to look INSIDE.
/// The installer-assembled `Farhelm.app` (SPEC_impl.md, "Native app
/// packaging") satisfies the same contract from the other direction: it
/// places a `farhelm` copy next to the executable in `Contents/MacOS/`,
/// which is why this code needs no bundle awareness to run from either
/// location. `FARHELM_DESKTOP_FARHELM` overrides it for developers and for
/// `scripts/desktop-smoke.sh`, which runs a `dx` build tree where the sibling
/// does not exist.
///
/// The failure text names the exact path that was tried and both ways out,
/// because the person hitting it is looking at a GUI app that refused to
/// start with no other diagnostic.
pub(super) fn bundled_farhelm() -> anyhow::Result<PathBuf> {
    let current = std::env::current_exe().context("locating desktop executable")?;
    resolve_sibling_farhelm(
        &current,
        std::env::var_os("FARHELM_DESKTOP_FARHELM").as_deref(),
    )
}

/// Decide where `farhelm` is, given this executable's path and the override.
///
/// Split from [`bundled_farhelm`] so the decision can be tested: the release
/// contract is "the two binaries are installed side by side", and nothing
/// else in this repository's automation exercises it — the smoke always sets
/// `FARHELM_DESKTOP_FARHELM`, and the asset check exits through
/// `--print-assets` before bootstrap runs. Reading `current_exe` and the
/// environment stays in the caller so the rule itself needs neither.
///
/// The override wins unconditionally, INCLUDING over a sibling that exists
/// and including when it names something that does not: a developer pointing
/// at a specific build wants that build or a clear failure from it, not a
/// silent fall back to whatever happens to be next to the running binary.
/// The only filesystem question asked here is whether the sibling is a file.
fn resolve_sibling_farhelm(
    current_exe: &Path,
    override_path: Option<&std::ffi::OsStr>,
) -> anyhow::Result<PathBuf> {
    if let Some(path) = override_path {
        return Ok(PathBuf::from(path));
    }
    let sibling = current_exe.with_file_name("farhelm");
    if sibling.is_file() {
        return Ok(sibling);
    }
    bail!(
        "farhelm-desktop needs the farhelm binary next to it ({}) and did not find one; \
         run the install script or set FARHELM_DESKTOP_FARHELM",
        sibling.display()
    )
}

/// The UI tree the EMBEDDED HELM serves over loopback, if a developer named
/// one.
///
/// `None` is the normal answer, and it is not a failure: a release build
/// carries the tree compiled in (D12) and the helm falls back to that, while
/// a plain `cargo build -p farhelm-desktop` genuinely has no UI to serve and
/// says so in its own log (`farhelm-helm`'s `warn_if_no_ui`).
///
/// Note the scope: this only decides what the loopback HELM answers with. The
/// native window never loads that page — it renders the component tree in the
/// webview and pulls its `/assets/*` from `assets::serve_asset` instead — so an
/// override here does NOT change what the window shows. The
/// `Contents/Resources/web` lookup this used to perform is gone with the
/// dx-produced `.app` bundle it belonged to (D6); the installer-assembled
/// `Farhelm.app` carries no web tree either, so nothing brings it back.
pub(super) fn bundled_web_ui() -> Option<PathBuf> {
    std::env::var_os("FARHELM_DESKTOP_UI_DIST").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- the release contract: two binaries, side by side (D6) ----

    /// The override names the CLI outright, even when a sibling exists.
    ///
    /// `scripts/desktop-smoke.sh` depends on exactly this: it runs the dx
    /// build tree, where no `farhelm` sibling exists at all, and names the
    /// one it built.
    #[farhelm_testtrace::test]
    fn the_farhelm_override_wins_over_a_present_sibling() {
        let dir = tempfile::tempdir().expect("temp dir");
        let sibling = dir.path().join("farhelm");
        std::fs::write(&sibling, b"#!/bin/sh\n").expect("writing the sibling");
        let chosen = resolve_sibling_farhelm(
            &dir.path().join("farhelm-desktop"),
            Some(std::ffi::OsStr::new("/elsewhere/farhelm")),
        )
        .expect("an override is taken as given");
        assert_eq!(chosen, PathBuf::from("/elsewhere/farhelm"));
    }

    /// With no override, the CLI is the file named `farhelm` in this
    /// executable's own directory.
    ///
    /// This IS the installed shape (`~/.local/bin/farhelm` beside
    /// `~/.local/bin/farhelm-desktop`) and nothing else in the repository's
    /// automation exercises it — the smoke and the asset check both bypass
    /// it — so a regression here would first be noticed by a user.
    #[farhelm_testtrace::test]
    fn the_sibling_beside_this_executable_is_found_without_an_override() {
        let dir = tempfile::tempdir().expect("temp dir");
        let sibling = dir.path().join("farhelm");
        std::fs::write(&sibling, b"#!/bin/sh\n").expect("writing the sibling");
        let chosen = resolve_sibling_farhelm(&dir.path().join("farhelm-desktop"), None)
            .expect("a sibling next to the executable is found");
        assert_eq!(chosen, sibling);
    }

    /// A missing sibling fails with the exact text the distribution plan
    /// specifies, naming the path that was tried.
    ///
    /// Asserted verbatim because this string is the entire diagnostic a user
    /// gets: a GUI binary that refuses to start has no window to explain
    /// itself in, and the two ways out (the install script, the override)
    /// have to be in the message or they are nowhere.
    #[farhelm_testtrace::test]
    fn a_missing_sibling_names_the_path_and_both_ways_out() {
        let dir = tempfile::tempdir().expect("temp dir");
        let exe = dir.path().join("farhelm-desktop");
        let error = resolve_sibling_farhelm(&exe, None)
            .expect_err("no sibling was created, so this must fail");
        assert_eq!(
            format!("{error}"),
            format!(
                "farhelm-desktop needs the farhelm binary next to it ({}) and did not find one; \
                 run the install script or set FARHELM_DESKTOP_FARHELM",
                dir.path().join("farhelm").display()
            )
        );
    }

    /// A DIRECTORY named `farhelm` next to the executable is not the CLI.
    ///
    /// The check is `is_file` rather than "exists" for this case. Spawning a
    /// directory fails later and much less clearly than refusing here does.
    #[farhelm_testtrace::test]
    fn a_directory_named_farhelm_does_not_count_as_the_sibling() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir(dir.path().join("farhelm")).expect("creating the decoy directory");
        resolve_sibling_farhelm(&dir.path().join("farhelm-desktop"), None)
            .expect_err("a directory is not the CLI");
    }
}
