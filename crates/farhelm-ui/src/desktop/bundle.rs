//! Where the desktop app finds what it needs outside itself: its state
//! directory and the `farhelm` CLI installed beside it.

use super::*;

pub(super) fn desktop_state_dir() -> anyhow::Result<PathBuf> {
    match std::env::var_os("FARHELM_DESKTOP_STATE_DIR") {
        Some(path) => Ok(PathBuf::from(path)),
        None => farhelm_supervisor::default_state_dir(),
    }
}

/// Locate the `farhelm` CLI this app spawns its supervisor from.
///
/// D6 ships two bare binaries that install side by side, so "next to me" is
/// the base contract. The Mac app bundle with side-by-side versions
/// (SPEC_impl.md, "Side-by-side versions inside Farhelm.app") is the one
/// place this code looks inside a bundle: there the sibling
/// `Contents/MacOS/farhelm` is the forwarder, and the app starts its
/// supervisor from its own version's folder instead (see
/// [`resolve_supervisor_farhelm`]). `FARHELM_DESKTOP_FARHELM` overrides both
/// for developers and for `scripts/desktop-smoke.sh`, which runs a `dx`
/// build tree where no sibling exists.
///
/// The failure text names the exact path that was tried and both ways out,
/// because the person hitting it is looking at a GUI app that refused to
/// start with no other diagnostic.
pub(super) fn bundled_farhelm() -> anyhow::Result<PathBuf> {
    let current = std::env::current_exe().context("locating desktop executable")?;
    resolve_supervisor_farhelm(
        &current,
        std::env::var_os("FARHELM_DESKTOP_FARHELM").as_deref(),
        env!("CARGO_PKG_VERSION"),
    )
}

/// Decide which `farhelm` to start the managed supervisor from, given this
/// executable's path, the override and this build's version.
///
/// The override wins, as in [`resolve_sibling_farhelm`]. Otherwise, when
/// this executable is the main program of an app bundle
/// (`<x>.app/Contents/MacOS/`, recognized exactly as the supervisor
/// recognizes the layout) and that bundle has a `Contents/Versions/` folder, the answer is
/// `Contents/Versions/<version>/farhelm`, where `<version>` is the version
/// compiled into this app: the app and the supervisor it manages are one
/// release, and the installer names each version's folder after it. A
/// missing folder refuses with its path rather than falling back to the
/// sibling, because in that layout the sibling is the forwarder, which may
/// run a different version than this app (the Installed one, mid-update).
/// Without a `Versions/` folder the sibling rule applies unchanged.
fn resolve_supervisor_farhelm(
    current_exe: &Path,
    override_path: Option<&std::ffi::OsStr>,
    version: &str,
) -> anyhow::Result<PathBuf> {
    if override_path.is_none()
        && let Some(contents) =
            farhelm_supervisor::app_bundle::bundle_contents_of_main_program(current_exe)
        && contents.join("Versions").is_dir()
    {
        let versioned = contents.join("Versions").join(version).join("farhelm");
        if versioned.is_file() {
            return Ok(versioned);
        }
        bail!(
            "farhelm-desktop {version} needs its own version of the farhelm binary at {} \
             and did not find one; reinstall Farhelm or set FARHELM_DESKTOP_FARHELM",
            versioned.display()
        );
    }
    resolve_sibling_farhelm(current_exe, override_path)
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

    // ---- the side-by-side version layout inside the Mac app ----

    /// Build `Farhelm.app/Contents/{MacOS,Versions}` under `root`, with the
    /// sibling forwarder present, and return `Contents`.
    fn versioned_bundle(root: &Path) -> PathBuf {
        let contents = root.join("Farhelm.app").join("Contents");
        std::fs::create_dir_all(contents.join("MacOS")).expect("MacOS");
        std::fs::create_dir_all(contents.join("Versions")).expect("Versions");
        std::fs::write(contents.join("MacOS").join("farhelm"), b"#!/bin/sh\n")
            .expect("writing the forwarder");
        contents
    }

    /// In the versioned layout the app starts its supervisor from its own
    /// version's folder, never from the sibling: the sibling is the
    /// forwarder, which mid-update runs the newly Installed version, not
    /// this app's.
    #[farhelm_testtrace::test]
    fn the_versioned_layout_starts_this_apps_own_version() {
        let dir = tempfile::tempdir().expect("temp dir");
        let contents = versioned_bundle(dir.path());
        let own = contents.join("Versions").join("1.2.3").join("farhelm");
        let other = contents.join("Versions").join("1.2.4").join("farhelm");
        for program in [&own, &other] {
            std::fs::create_dir_all(program.parent().unwrap()).expect("version folder");
            std::fs::write(program, b"#!/bin/sh\n").expect("writing a versioned program");
        }
        let chosen = resolve_supervisor_farhelm(
            &contents.join("MacOS").join("farhelm-desktop"),
            None,
            "1.2.3",
        )
        .expect("this app's version folder is found");
        assert_eq!(chosen, own);
    }

    /// A missing version folder refuses and names the path, rather than
    /// silently starting the forwarder's choice of version.
    #[farhelm_testtrace::test]
    fn a_missing_version_folder_refuses_instead_of_using_the_forwarder() {
        let dir = tempfile::tempdir().expect("temp dir");
        let contents = versioned_bundle(dir.path());
        let error = resolve_supervisor_farhelm(
            &contents.join("MacOS").join("farhelm-desktop"),
            None,
            "1.2.3",
        )
        .expect_err("the folder for this version does not exist");
        assert!(
            format!("{error}").contains(
                &contents
                    .join("Versions")
                    .join("1.2.3")
                    .display()
                    .to_string()
            ),
            "{error}"
        );
    }

    /// The override still wins in the versioned layout; the desktop smoke
    /// depends on it wherever it runs.
    #[farhelm_testtrace::test]
    fn the_override_wins_in_the_versioned_layout_too() {
        let dir = tempfile::tempdir().expect("temp dir");
        let contents = versioned_bundle(dir.path());
        let chosen = resolve_supervisor_farhelm(
            &contents.join("MacOS").join("farhelm-desktop"),
            Some(std::ffi::OsStr::new("/elsewhere/farhelm")),
            "1.2.3",
        )
        .expect("an override is taken as given");
        assert_eq!(chosen, PathBuf::from("/elsewhere/farhelm"));
    }

    /// A bundle without `Versions/` (the layout before this one) keeps the
    /// sibling rule, so an app installed the old way still starts.
    #[farhelm_testtrace::test]
    fn a_bundle_without_versions_still_uses_the_sibling() {
        let dir = tempfile::tempdir().expect("temp dir");
        let macos = dir
            .path()
            .join("Farhelm.app")
            .join("Contents")
            .join("MacOS");
        std::fs::create_dir_all(&macos).expect("MacOS");
        std::fs::write(macos.join("farhelm"), b"#!/bin/sh\n").expect("sibling");
        let chosen = resolve_supervisor_farhelm(&macos.join("farhelm-desktop"), None, "1.2.3")
            .expect("the sibling");
        assert_eq!(chosen, macos.join("farhelm"));
    }

    /// The installer refuses releases from before the side-by-side layout
    /// by looking for a fixed piece of this refusal's text in the desktop
    /// program it downloaded (scripts/install.sh, step 6). Changing the text
    /// without the installer would make it refuse every new release, so the
    /// two are pinned together here: the refusal still contains the marker,
    /// and the installer still checks for exactly it.
    #[farhelm_testtrace::test]
    fn the_installers_release_marker_is_in_this_refusal() {
        const MARKER: &str = "needs its own version of the farhelm binary at";
        let dir = tempfile::tempdir().expect("temp dir");
        let contents = versioned_bundle(dir.path());
        let error = resolve_supervisor_farhelm(
            &contents.join("MacOS").join("farhelm-desktop"),
            None,
            "1.2.3",
        )
        .expect_err("no version folder");
        assert!(format!("{error}").contains(MARKER), "{error}");
        let installer = include_str!("../../../../scripts/install.sh");
        assert!(
            installer.contains(&format!("grep -qF '{MARKER}'")),
            "scripts/install.sh no longer checks for the marker"
        );
    }

    /// A `MacOS` folder outside an app bundle is not the layout, even with a
    /// `Versions` folder beside it: the sibling still wins there, so an
    /// installation that merely looks similar is not changed.
    #[farhelm_testtrace::test]
    fn a_macos_folder_outside_an_app_bundle_keeps_the_sibling() {
        let dir = tempfile::tempdir().expect("temp dir");
        let package = dir.path().join("package");
        std::fs::create_dir_all(package.join("MacOS")).expect("MacOS");
        std::fs::create_dir_all(package.join("Versions")).expect("Versions");
        std::fs::write(package.join("MacOS").join("farhelm"), b"#!/bin/sh\n").expect("sibling");
        let chosen = resolve_supervisor_farhelm(
            &package.join("MacOS").join("farhelm-desktop"),
            None,
            "1.2.3",
        )
        .expect("the sibling");
        assert_eq!(chosen, package.join("MacOS").join("farhelm"));
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
