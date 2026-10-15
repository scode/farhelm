//! Where the desktop app finds what it needs outside itself: its state
//! directory and the `farhelm` CLI installed beside it.

use super::*;

pub(super) fn desktop_state_dir() -> anyhow::Result<PathBuf> {
    match std::env::var_os("FARHELM_DESKTOP_STATE_DIR") {
        Some(path) => Ok(PathBuf::from(path)),
        None => farhelm_supervisor::default_state_dir(),
    }
}

/// The operator-staged provisioning payload directory for the embedded
/// helm, read from the same `FARHELM_HELM_PAYLOAD_DIR` the standalone
/// `farhelm helm run` takes.
///
/// This is what lets an app built from a working copy update remote hosts
/// at all: such a build reports the development version, has no release to
/// download payloads from, and otherwise refuses Update. With this set it
/// pushes whatever Linux binaries `scripts/devbuild-mac.sh` staged, with no
/// signature check, exactly as `--payload-dir` does for the standalone helm
/// on any build (SPEC_impl.md, the provisioning payload paragraphs under
/// D2/D3). An installed app launched from Finder or the Dock never sees the
/// variable, its updater strips every `FARHELM_*` variable before running
/// the installer, and the managed supervisor is spawned without it, so it
/// is opt-in per launch and stays inside this process.
///
/// An empty value counts as unset rather than selecting the app's working
/// directory. The standalone helm differs here: its argument parser rejects
/// an empty value outright.
pub(super) fn helm_payload_dir() -> Option<PathBuf> {
    std::env::var_os("FARHELM_HELM_PAYLOAD_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Locate the `farhelm` CLI this app spawns its supervisor from, holding its
/// version when it is this app's own (see [`supervisor_farhelm`]).
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
    supervisor_farhelm(
        &current,
        std::env::var_os("FARHELM_DESKTOP_FARHELM").as_deref(),
        env!("CARGO_PKG_VERSION"),
    )
}

/// Resolve the supervisor's `farhelm` and, when it is this app's own version
/// folder, hold that version for the rest of the app's life.
///
/// One function rather than two calls at the start of the bootstrap so that
/// the hold cannot be forgotten, and cannot be applied to a path the app did
/// not pick itself. Only the program [`resolve_supervisor_farhelm`] chose
/// from this app's `Contents/Versions/<version>/` is held: an override
/// selects a program the developer controls, which the app held nothing on
/// before and still does not, even when it happens to point into an
/// installed bundle. The caller runs this before discovery probes anything or
/// a supervisor is spawned, which is what makes the hold cover the whole
/// window in which the app still needs its version.
fn supervisor_farhelm(
    current_exe: &Path,
    override_path: Option<&std::ffi::OsStr>,
    version: &str,
) -> anyhow::Result<PathBuf> {
    let chosen = resolve_supervisor_farhelm(current_exe, override_path, version)?;
    if chosen.own_version {
        hold_supervisor_version(&chosen.path, version)?;
    }
    Ok(chosen.path)
}

/// The `farhelm` the bootstrap starts its supervisor from, and whether the
/// app picked it from its own version folder (and must therefore hold it).
#[derive(Debug)]
struct SupervisorFarhelm {
    path: PathBuf,
    own_version: bool,
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
) -> anyhow::Result<SupervisorFarhelm> {
    if override_path.is_none()
        && let Some(contents) =
            farhelm_supervisor::app_bundle::bundle_contents_of_main_program(current_exe)
        && contents.join("Versions").is_dir()
    {
        let versioned = contents.join("Versions").join(version).join("farhelm");
        if versioned.is_file() {
            return Ok(SupervisorFarhelm {
                path: versioned,
                own_version: true,
            });
        }
        return Err(own_version_missing(version, &versioned));
    }
    Ok(SupervisorFarhelm {
        path: resolve_sibling_farhelm(current_exe, override_path)?,
        own_version: false,
    })
}

/// The refusal for an app whose own version's `farhelm` is gone, whether
/// resolution found it missing or it vanished before the hold took it.
///
/// Both moments have the same cause in practice (an update or uninstall
/// removed the version while this app was not yet holding it) and the same
/// way out: opening Farhelm again starts whatever is installed now. Keeping
/// one text also gives the later bounded reopen a single refusal to act on.
/// The text keeps the fixed marker the installer looks for in a downloaded
/// desktop program to recognize a release with the side-by-side layout
/// (`scripts/install.sh`, pinned by
/// `the_installers_release_marker_is_in_this_refusal`), so the marker must
/// stay one unbroken literal with no placeholder inside it.
fn own_version_missing(version: &str, program: &Path) -> anyhow::Error {
    anyhow::anyhow!(
        "farhelm-desktop {version} needs its own version of the farhelm binary at {} \
         and did not find one; an update or uninstall probably removed it, so open Farhelm \
         again (if that does not help, reinstall Farhelm or set FARHELM_DESKTOP_FARHELM)",
        program.display()
    )
}

/// Hold this app's own version for the app's whole life, before anything is
/// started from it (see `farhelm_supervisor::version_lock`).
///
/// The app itself is `Contents/MacOS/farhelm-desktop`, not a program in a
/// version folder, so nothing else would tell an install or an uninstall
/// that its version is in use: between the app starting and its supervisor
/// running, and again whenever it restarts its supervisor, the version
/// folder is something it still needs. Holding it closes that gap; the
/// supervisor then takes its own lock as well. The lock's descriptor is
/// close-on-exec, so no child of the app (its supervisor, the updater's
/// installer) inherits it.
///
/// Nothing on a filesystem that cannot lock is held, and that is not an
/// error. A version that is already gone refuses with
/// [`own_version_missing`]; one that an update or uninstall still holds
/// exclusively after a few seconds refuses with its own sentence. Either
/// way, reopening Farhelm starts whatever is installed then.
fn hold_supervisor_version(farhelm: &Path, version: &str) -> anyhow::Result<()> {
    use farhelm_supervisor::version_lock::{self, HoldError};
    match version_lock::hold_for_process(farhelm, None) {
        Ok(_) => Ok(()),
        Err(HoldError::Gone { program }) => Err(own_version_missing(version, &program)),
        Err(HoldError::Busy { .. }) => {
            bail!("Farhelm is being updated or uninstalled; open it again in a moment")
        }
        Err(HoldError::Io { program, error }) => {
            Err(error).with_context(|| format!("locking {} for this app", program.display()))
        }
    }
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
        assert_eq!(chosen.path, own);
    }

    /// Write an executable stand-in for one version's `farhelm` inside the
    /// bundle whose `Contents` is `contents`, and return its path.
    fn versioned_program(contents: &Path, version: &str) -> PathBuf {
        let program = contents.join("Versions").join(version).join("farhelm");
        std::fs::create_dir_all(program.parent().unwrap()).expect("version folder");
        std::fs::write(&program, b"#!/bin/sh\n").expect("writing the versioned program");
        program
    }

    /// Whether something already holds `program`'s version lock: an
    /// exclusive no-wait attempt, which is what deleting a version takes,
    /// fails exactly when it does.
    fn version_is_held(program: &Path) -> bool {
        let deleter = std::fs::File::open(program).expect("open the versioned program");
        match deleter.try_lock() {
            Ok(()) => {
                deleter.unlock().expect("release the probe's lock");
                false
            }
            Err(std::fs::TryLockError::WouldBlock) => true,
            Err(std::fs::TryLockError::Error(error)) => panic!("probing the lock: {error}"),
        }
    }

    /// The app holds the version it picked from its own version folder, so
    /// an install or uninstall cannot delete it while the app runs. This is
    /// what protects the app before its supervisor exists and while it
    /// restarts one.
    #[farhelm_testtrace::test]
    fn the_app_holds_its_supervisors_version() {
        let dir = tempfile::tempdir().expect("temp dir");
        let contents = versioned_bundle(dir.path());
        let own = versioned_program(&contents, "1.2.3");
        assert!(
            !version_is_held(&own),
            "control: nothing holds the version before the app does"
        );

        let chosen = supervisor_farhelm(
            &contents.join("MacOS").join("farhelm-desktop"),
            None,
            "1.2.3",
        )
        .expect("the app finds and holds its version");
        assert_eq!(chosen, own);
        assert!(
            version_is_held(&own),
            "deleting the version is refused while the app holds it"
        );
    }

    /// An override that points into an installed bundle's version folder is
    /// taken as given and NOT held: the override is a developer's choice the
    /// app never locked before, and holding it would keep that version from
    /// being pruned or uninstalled for as long as the app runs. A missing
    /// override target is still returned rather than refused, as before.
    #[farhelm_testtrace::test]
    fn an_override_into_a_version_folder_is_not_held() {
        let dir = tempfile::tempdir().expect("temp dir");
        let contents = versioned_bundle(dir.path());
        let installed = versioned_program(&contents, "1.2.3");
        let exe = contents.join("MacOS").join("farhelm-desktop");

        let chosen = supervisor_farhelm(&exe, Some(installed.as_os_str()), "1.2.3")
            .expect("an override is taken as given");
        assert_eq!(chosen, installed);
        assert!(
            !version_is_held(&installed),
            "the app holds nothing on an overridden program"
        );

        let missing = contents.join("Versions").join("9.9.9").join("farhelm");
        let chosen = supervisor_farhelm(&exe, Some(missing.as_os_str()), "1.2.3")
            .expect("a missing override target is still taken as given");
        assert_eq!(chosen, missing);
    }

    /// The app's own version gone, whether resolution already finds it
    /// missing or it disappears between resolution and the hold, refuses with
    /// one sentence that says to open Farhelm again and keeps the installer's
    /// release marker. Two texts for one cause would leave the later bounded
    /// reopen two refusals to recognize, and a user two different answers.
    #[farhelm_testtrace::test]
    fn a_missing_own_version_refuses_the_same_way_before_and_after_resolution() {
        let dir = tempfile::tempdir().expect("temp dir");
        let contents = versioned_bundle(dir.path());
        let exe = contents.join("MacOS").join("farhelm-desktop");
        let own = contents.join("Versions").join("1.2.3").join("farhelm");
        let expected = own_version_missing("1.2.3", &own).to_string();
        assert!(expected.contains("open Farhelm again"), "{expected}");

        let already_gone =
            supervisor_farhelm(&exe, None, "1.2.3").expect_err("no version folder exists yet");
        assert_eq!(already_gone.to_string(), expected);

        versioned_program(&contents, "1.2.3");
        let chosen = resolve_supervisor_farhelm(&exe, None, "1.2.3").expect("found");
        assert!(chosen.own_version);
        std::fs::remove_file(&chosen.path).expect("an update removes the version");
        let gone_before_hold = hold_supervisor_version(&chosen.path, "1.2.3")
            .expect_err("the version vanished before the hold");
        assert_eq!(gone_before_hold.to_string(), expected);
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
        assert_eq!(chosen.path, PathBuf::from("/elsewhere/farhelm"));
        assert!(
            !chosen.own_version,
            "an override is never this app's own pick"
        );
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
        assert_eq!(chosen.path, macos.join("farhelm"));
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
        assert_eq!(chosen.path, package.join("MacOS").join("farhelm"));
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
