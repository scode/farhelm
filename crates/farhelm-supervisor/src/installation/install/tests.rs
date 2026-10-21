//! The locked install core, in process, against a temporary home.
//!
//! The shell harness (`scripts/test-install-sh.sh`) runs the real script and
//! the real binary end to end; what only an in-process test can do is stop a
//! run after each step of the write order (no kill switch exists in the
//! shipped program) and move the app away in the middle of a run, which is
//! what these tests add.

use super::*;
use crate::installation::{COMPAT_LOCK_FOLDER, InstallState, OwnershipRecord, RECORD_FILE};
use std::path::{Path, PathBuf};

/// The inode of `path`, as `install.sh` hands it over to vouch for an
/// older app (`InstallRequest::replace_legacy_bundle`).
fn inode(path: &Path) -> u64 {
    use std::os::unix::fs::MetadataExt as _;
    std::fs::symlink_metadata(path).expect("inode").ino()
}

/// A temporary home with `Applications/` and `.local/bin/`, and a staging
/// folder per version holding what the script would stage.
struct Home {
    root: tempfile::TempDir,
}

impl Home {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("temp home");
        std::fs::create_dir_all(root.path().join(".local/bin")).unwrap();
        Self { root }
    }

    fn path(&self) -> &Path {
        self.root.path()
    }

    fn app(&self) -> PathBuf {
        self.path().join("Applications").join(APP_NAME)
    }

    /// Stage version `v`'s files; each one's contents names the version, so
    /// a test can tell which release a file came from.
    fn stage(&self, v: &str) -> PathBuf {
        let staging = self.path().join(format!("staging-{v}"));
        std::fs::create_dir_all(&staging).unwrap();
        for (name, text) in [
            ("farhelm", format!("farhelm {v}\n")),
            ("farhelm-desktop", format!("farhelm-desktop {v}\n")),
            ("Farhelm.icns", format!("icns {v}\n")),
            ("forwarder", "#!/bin/sh\n# forwarder\n".to_string()),
            ("Info.plist", format!("plist {v}\n")),
        ] {
            std::fs::write(staging.join(name), text).unwrap();
        }
        staging
    }

    fn request(&self, v: &str) -> InstallRequest {
        InstallRequest {
            staging: self.stage(v),
            version: v.to_string(),
            applications: self.path().join("Applications"),
            bin_dir: self.path().join(".local/bin"),
            link: self.path().join(".local/bin/farhelm"),
            running_records: vec![self.path().join("running-version")],
            update_only: false,
            replace_legacy_bundle: None,
            lock_wait: Duration::ZERO,
        }
    }

    fn install(&self, v: &str) -> Result<Installed, Refusal> {
        install(&self.request(v), &mut |_| Ok(()))
    }

    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.app().join(rel)).unwrap_or_default()
    }

    fn record(&self) -> OwnershipRecord {
        OwnershipRecord::parse(
            &std::fs::read(self.app().join("Contents").join(RECORD_FILE)).unwrap(),
        )
        .expect("a valid record")
    }

    fn versions(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.app().join("Contents/Versions"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name != "installed")
            .collect();
        names.sort();
        names
    }

    /// Whether the app would start and its Terminal commands would run: the
    /// app program's own version (which it starts its supervisor from; the
    /// fixture program's text names it) has its `farhelm`, and so does the
    /// version the Installed record names (which the forwarder runs). During
    /// an update the two can differ, so both are checked.
    fn launchable(&self) -> bool {
        let desktop = self.read("Contents/MacOS/farhelm-desktop");
        let Some(own) = desktop.trim().strip_prefix("farhelm-desktop ") else {
            return false;
        };
        let installed = self.read("Contents/Versions/installed");
        [own, installed.trim()].iter().all(|version| {
            self.app()
                .join("Contents/Versions")
                .join(version)
                .join("farhelm")
                .is_file()
        })
    }

    /// Nothing this module stages or trashes is left beside the app, and
    /// the old installers' lock folder is gone.
    fn no_leftovers_beside_the_app(&self) -> bool {
        self.no_leftovers_beside_the_app_but("")
    }

    /// [`Self::no_leftovers_beside_the_app`], allowing one more entry a
    /// test put there itself.
    fn no_leftovers_beside_the_app_but(&self, allowed: &str) -> bool {
        std::fs::read_dir(self.path().join("Applications"))
            .unwrap()
            .all(|entry| {
                let name = entry.unwrap().file_name();
                name == APP_NAME || name == allowed
            })
    }
}

/// A fresh install builds the whole app, gives the installation a new id
/// with a complete generation, points the Terminal link at the forwarder,
/// and leaves nothing beside the app.
#[farhelm_testtrace::test]
fn a_fresh_install_builds_the_app_with_a_new_installation_id() {
    let home = Home::new();
    let done = home.install("1.2.3").expect("fresh install");
    assert_eq!(
        done,
        Installed {
            updated: false,
            notes: vec![]
        }
    );
    assert!(home.launchable());
    assert_eq!(home.read("Contents/Versions/installed"), "1.2.3\n");
    let record = home.record();
    assert!(record.id.is_some());
    assert_eq!(
        (record.generation, record.state),
        (2, InstallState::Complete)
    );
    assert_eq!(record.link_path(), home.path().join(".local/bin/farhelm"));
    assert_eq!(
        std::fs::read_link(home.path().join(".local/bin/farhelm")).unwrap(),
        home.app().join("Contents/MacOS/farhelm")
    );
    assert!(home.no_leftovers_beside_the_app());
}

/// Updates keep the installation's id and move the generation on; pruning
/// keeps the target and the previously Installed version, and removes the
/// rest once nothing holds them.
#[farhelm_testtrace::test]
fn updates_keep_the_id_and_prune_by_todays_rule() {
    let home = Home::new();
    home.install("1.2.3").unwrap();
    let id = home.record().id;
    let done = home.install("1.2.4").expect("update");
    assert!(done.updated);
    assert_eq!(
        home.versions(),
        ["1.2.3", "1.2.4"],
        "the previous Installed version stays"
    );
    assert_eq!(home.record().id, id);
    assert_eq!(home.record().generation, 4);
    home.install("1.2.5").unwrap();
    assert_eq!(
        home.versions(),
        ["1.2.4", "1.2.5"],
        "1.2.3 is no longer kept"
    );
    assert!(home.no_leftovers_beside_the_app());
}

/// A version something still runs from is kept even when today's rule
/// would remove it (the union rule), and goes on a later run once it is
/// free; a version a Running record names is kept as before.
#[farhelm_testtrace::test]
fn a_busy_or_running_version_is_kept() {
    let home = Home::new();
    home.install("1.2.3").unwrap();
    home.install("1.2.4").unwrap();
    let user = std::fs::File::open(home.app().join("Contents/Versions/1.2.3/farhelm")).unwrap();
    user.lock_shared()
        .expect("a running 1.2.3 holds its version");
    home.install("1.2.5").unwrap();
    assert_eq!(
        home.versions(),
        ["1.2.3", "1.2.4", "1.2.5"],
        "busy 1.2.3 is kept"
    );
    drop(user);
    std::fs::write(home.path().join("running-version"), "1.2.4\n").unwrap();
    home.install("1.2.6").unwrap();
    assert_eq!(
        home.versions(),
        ["1.2.4", "1.2.5", "1.2.6"],
        "the free 1.2.3 goes; the running 1.2.4 stays"
    );
}

/// An update stopped after any step leaves an app that starts, and a rerun
/// finishes it. The write order's one ordering promise to older apps is
/// checked at every stop: once the Installed record names the new version,
/// the app program is already the new one; and the record says "installing"
/// until everything published is on disk.
#[farhelm_testtrace::test]
fn an_update_stopped_after_each_step_starts_and_a_rerun_finishes_it() {
    for stop in [
        Step::RecordInstalling,
        Step::Repaired,
        Step::VersionPlaced,
        Step::AppProgram,
        Step::Icon,
        Step::Forwarder,
        Step::InstalledRecord,
        Step::InfoPlist,
        Step::RecordComplete,
        Step::Pruned,
        Step::Link,
    ] {
        let home = Home::new();
        home.install("1.2.3").unwrap();
        let mut reached = false;
        let mut hook = |at: Step| {
            if at == stop {
                reached = true;
                Err(io::Error::other("stopped"))
            } else {
                Ok(())
            }
        };
        assert!(
            install(&home.request("1.2.4"), &mut hook).is_err(),
            "{stop:?}: stopped"
        );
        assert!(
            reached,
            "{stop:?}: the run reached the step it was stopped at"
        );
        assert!(home.launchable(), "{stop:?}: the app still starts");
        if home.read("Contents/Versions/installed") == "1.2.4\n" {
            assert_eq!(
                home.read("Contents/MacOS/farhelm-desktop"),
                "farhelm-desktop 1.2.4\n",
                "{stop:?}: Installed names 1.2.4 only after the app program is 1.2.4's"
            );
        }
        let state = home.record().state;
        let finished = matches!(stop, Step::RecordComplete | Step::Pruned | Step::Link);
        assert_eq!(
            state == InstallState::Complete,
            finished,
            "{stop:?}: the record says installing until everything is on disk"
        );
        assert!(
            !home
                .path()
                .join("Applications")
                .join(COMPAT_LOCK_FOLDER)
                .exists(),
            "{stop:?}: no lock folder is left for older installers to trip on"
        );
        home.install("1.2.4").expect("a rerun finishes the update");
        assert_eq!(home.read("Contents/Versions/installed"), "1.2.4\n");
        assert_eq!(home.read("Contents/Info.plist"), "plist 1.2.4\n");
        assert_eq!(home.record().state, InstallState::Complete);
        assert!(
            home.no_leftovers_beside_the_app(),
            "{stop:?}: the rerun cleans up"
        );
    }
}

/// The work follows the app it checked: if the app is moved away mid-run
/// and something else appears at its place, the update lands in the moved
/// app and the newcomer is untouched (design, section 3).
#[farhelm_testtrace::test]
fn the_update_follows_the_app_it_checked_when_finder_moves_it() {
    let home = Home::new();
    home.install("1.2.3").unwrap();
    let moved = home.path().join("Applications/Moved.app");
    let app = home.app();
    let mut hook = |at: Step| {
        if at == Step::RecordInstalling {
            std::fs::rename(&app, &moved)?;
            std::fs::create_dir(&app)?;
            std::fs::write(app.join("newcomer"), b"not Farhelm")?;
        }
        Ok(())
    };
    install(&home.request("1.2.4"), &mut hook).expect("the update completes");
    assert_eq!(
        std::fs::read_to_string(moved.join("Contents/Versions/installed")).unwrap(),
        "1.2.4\n"
    );
    assert!(moved.join("Contents/Versions/1.2.4/farhelm").is_file());
    let newcomer: Vec<_> = std::fs::read_dir(&app).unwrap().collect();
    assert_eq!(
        newcomer.len(),
        1,
        "the app now at the old place is untouched"
    );
}

/// A rebuild moves the app it inspected aside, not whatever is at the
/// place by then: if Finder swaps in another app between inspection and the
/// move, the rebuild refuses and that app is left exactly where it is,
/// neither deleted with the previous app nor replaced, and the inspected
/// app is untouched too.
#[farhelm_testtrace::test]
fn a_rebuild_never_moves_or_replaces_an_app_swapped_in_meanwhile() {
    let home = Home::new();
    std::fs::create_dir_all(home.app().join("Contents/MacOS")).unwrap();
    std::fs::write(home.app().join("Contents/MacOS/old"), b"older layout").unwrap();
    let mut request = home.request("1.2.3");
    request.replace_legacy_bundle = Some(inode(&home.app()));
    let app = home.app();
    let aside = home.path().join("Applications/Aside.app");
    let mut hook = |at: Step| {
        if at == Step::BundleBuilt {
            std::fs::rename(&app, &aside)?;
            std::fs::create_dir(&app)?;
            std::fs::write(app.join("newcomer"), b"not ours")?;
        }
        Ok(())
    };
    let refusal = install(&request, &mut hook).expect_err("the app at the place changed");
    assert!(
        refusal
            .message
            .contains("changed while it was being installed"),
        "{refusal:?}"
    );
    assert_eq!(std::fs::read(app.join("newcomer")).unwrap(), b"not ours");
    assert!(
        aside.join("Contents/MacOS/old").exists(),
        "the inspected app is untouched"
    );
    assert!(home.no_leftovers_beside_the_app_but("Aside.app"));
}

/// Cleanup beside the app removes only folders this module made, by their
/// exact names and contents, and puts back an app a killed rebuild left
/// aside before removing its staging folder: a similar-looking folder of
/// someone else's is never deleted, and the only copy of an app is never
/// deleted with the staging folder that holds it.
#[farhelm_testtrace::test]
fn cleanup_touches_only_this_modules_own_leftovers() {
    let home = Home::new();
    let applications = home.path().join("Applications");
    let hex = "0123456789abcdef0123456789abcdef";
    std::fs::create_dir_all(applications.join(".farhelm-trash.notes/keep")).unwrap();
    std::fs::create_dir_all(applications.join(format!(".farhelm-trash.{hex}/mine"))).unwrap();
    let previous = applications.join(format!(".farhelm-stage.{hex}/previous/Contents"));
    std::fs::create_dir_all(&previous).unwrap();
    std::fs::write(
        previous.join("marker"),
        b"the app a killed rebuild moved aside",
    )
    .unwrap();
    // The restored app is not this layout's, so the run then refuses it;
    // what matters is what cleanup did first.
    home.install("1.2.3")
        .expect_err("the restored app is not this layout's");
    assert!(
        applications.join(".farhelm-trash.notes/keep").is_dir(),
        "not our name"
    );
    assert!(
        applications
            .join(format!(".farhelm-trash.{hex}/mine"))
            .is_dir(),
        "our name, but not our contents"
    );
    assert!(!applications.join(format!(".farhelm-stage.{hex}")).exists());
    assert_eq!(
        std::fs::read(home.app().join("Contents/marker")).unwrap(),
        b"the app a killed rebuild moved aside",
        "the moved-aside app is back at its place"
    );
}

/// A run started by an older app's updater never reinstalls a removed app,
/// never finishes into an interrupted uninstall, and never goes back over a
/// newer Installed version; it does finish an interrupted install.
#[farhelm_testtrace::test]
fn update_only_runs_refuse_what_an_old_updater_must_not_do() {
    let home = Home::new();
    let mut request = home.request("1.2.4");
    request.update_only = true;
    let refusal = install(&request, &mut |_| Ok(())).expect_err("no app");
    assert!(refusal.message.contains("not installed"), "{refusal:?}");
    assert!(!home.app().exists(), "nothing was created");

    home.install("1.2.5").unwrap();
    let refusal = install(&request, &mut |_| Ok(())).expect_err("Installed is newer");
    assert!(refusal.message.contains("newer than 1.2.4"), "{refusal:?}");

    let record_path = home.app().join("Contents").join(RECORD_FILE);
    let removing = home.record().advanced(InstallState::Removing);
    std::fs::write(&record_path, removing.render()).unwrap();
    let mut later = home.request("1.2.6");
    later.update_only = true;
    let refusal = install(&later, &mut |_| Ok(())).expect_err("an uninstall was interrupted");
    assert!(refusal.message.contains("uninstall"), "{refusal:?}");

    let installing = home.record().advanced(InstallState::Installing);
    std::fs::write(&record_path, installing.render()).unwrap();
    install(&later, &mut |_| Ok(())).expect("an interrupted install is finished");
    assert_eq!(home.record().state, InstallState::Complete);
}

/// The record today's installers write (two fields, no id) is accepted, and
/// the update gives the installation its id; a "moved here" record naming
/// another link is accepted and rewritten to this home's link.
#[farhelm_testtrace::test]
fn an_older_installers_record_is_accepted_and_extended() {
    let home = Home::new();
    home.install("1.2.3").unwrap();
    let record_path = home.app().join("Contents").join(RECORD_FILE);
    std::fs::write(
        &record_path,
        b"farhelm-app-v2\0/old/home/.local/bin/farhelm\0",
    )
    .unwrap();
    home.install("1.2.4")
        .expect("update over a two-field record");
    let record = home.record();
    assert!(record.id.is_some());
    assert_eq!(record.state, InstallState::Complete);
    assert_eq!(record.link_path(), home.path().join(".local/bin/farhelm"));
}

/// Another install or uninstall holding the installation lock, or an older
/// tool's lock folder, refuses before anything changes; a lock folder a
/// newer tool left behind when it died is cleared.
#[farhelm_testtrace::test]
fn locks_held_by_others_refuse_before_anything_changes() {
    let home = Home::new();
    home.install("1.2.3").unwrap();
    let held =
        crate::installation::lock_installation(&home.path().join("Applications"), Duration::ZERO)
            .unwrap();
    let refusal = home.install("1.2.4").expect_err("busy");
    assert!(
        refusal
            .message
            .contains("another Farhelm install or uninstall"),
        "{refusal:?}"
    );
    drop(held);
    assert_eq!(home.read("Contents/Versions/installed"), "1.2.3\n");

    let folder = home.path().join("Applications").join(COMPAT_LOCK_FOLDER);
    std::fs::create_dir(&folder).unwrap();
    let refusal = home.install("1.2.4").expect_err("an older tool's folder");
    assert!(refusal.message.contains("remove"), "{refusal:?}");
    assert_eq!(home.read("Contents/Versions/installed"), "1.2.3\n");
}

/// An app at the usual place that is not this layout's is refused unless
/// the script vouched for it as an older layout this installer built, in
/// which case it is replaced whole with a fresh id.
#[farhelm_testtrace::test]
fn a_foreign_app_is_refused_and_a_vouched_older_one_is_rebuilt() {
    let home = Home::new();
    std::fs::create_dir_all(home.app().join("Contents/MacOS")).unwrap();
    std::fs::write(home.app().join("Contents/MacOS/other"), b"someone else's").unwrap();
    let refusal = home.install("1.2.3").expect_err("not ours");
    assert!(
        refusal
            .message
            .contains("not an app bundle this installer built")
    );
    assert!(home.app().join("Contents/MacOS/other").exists());

    let mut request = home.request("1.2.3");
    request.replace_legacy_bundle = Some(inode(&home.app()));
    let done = install(&request, &mut |_| Ok(())).expect("rebuilt");
    assert!(done.updated);
    assert!(home.launchable());
    assert!(!home.app().join("Contents/MacOS/other").exists());
    assert!(home.no_leftovers_beside_the_app());
}

/// The script's approval to replace an older app holds only for the folder
/// it checked: an app put at the place after the check (a different
/// folder, so a different inode) is refused and left as it is, and a file
/// at the place is never replaced, whatever the approval says.
///
/// Why: the script checks before the installation lock is taken, so
/// without this binding Finder could swap anything in between the check
/// and the locked work, and the rebuild would move it aside and delete it
/// (design, section 3: the locked work checks it is the expected
/// installation).
#[farhelm_testtrace::test]
fn an_approval_for_one_older_app_does_not_cover_another() {
    let home = Home::new();
    std::fs::create_dir_all(home.app().join("Contents/MacOS")).unwrap();
    let checked = inode(&home.app());
    let aside = home.path().join("Applications/Aside.app");
    std::fs::rename(home.app(), &aside).unwrap();
    std::fs::create_dir_all(home.app().join("Contents/MacOS")).unwrap();
    std::fs::write(home.app().join("Contents/MacOS/other"), b"someone else's").unwrap();
    assert_ne!(
        inode(&home.app()),
        checked,
        "fixture: the newcomer is another folder"
    );
    let mut request = home.request("1.2.3");
    request.replace_legacy_bundle = Some(checked);
    let refusal = install(&request, &mut |_| Ok(())).expect_err("not the checked folder");
    assert!(
        refusal
            .message
            .contains("not an app bundle this installer built"),
        "{refusal:?}"
    );
    assert!(
        home.app().join("Contents/MacOS/other").exists(),
        "left as it is"
    );
    assert!(aside.join("Contents/MacOS").exists());

    std::fs::remove_dir_all(home.app()).unwrap();
    std::fs::write(home.app(), b"a file").unwrap();
    let mut request = home.request("1.2.3");
    request.replace_legacy_bundle = Some(inode(&home.app()));
    install(&request, &mut |_| Ok(())).expect_err("a file is never replaced");
    assert_eq!(std::fs::read(home.app()).unwrap(), b"a file");
}

/// What a killed older installer leaves is repaired as before: missing
/// `Resources/`, stray `.farhelm-new.*` files, and a version folder without
/// its program, which is completed rather than refused.
#[farhelm_testtrace::test]
fn an_older_installers_leftovers_are_repaired() {
    let home = Home::new();
    home.install("1.2.3").unwrap();
    std::fs::remove_dir_all(home.app().join("Contents/Resources")).unwrap();
    std::fs::write(
        home.app()
            .join("Contents/MacOS/.farhelm-new.4242.farhelm-desktop"),
        b"x",
    )
    .unwrap();
    std::fs::create_dir(home.app().join("Contents/Versions/1.2.4")).unwrap();
    home.install("1.2.4").expect("repairing update");
    assert!(home.launchable());
    assert!(home.app().join("Contents/Resources/Farhelm.icns").is_file());
    assert!(
        !home
            .app()
            .join("Contents/MacOS/.farhelm-new.4242.farhelm-desktop")
            .exists()
    );
    assert_eq!(
        home.read("Contents/Versions/1.2.4/farhelm"),
        "farhelm 1.2.4\n"
    );
}

/// Reinstalling the version already installed reuses its folder rather
/// than rewriting the program its users hold locks on.
#[farhelm_testtrace::test]
fn reinstalling_the_same_version_reuses_its_folder() {
    let home = Home::new();
    home.install("1.2.3").unwrap();
    home.install("1.2.4").unwrap();
    let program = home.app().join("Contents/Versions/1.2.4/farhelm");
    let before = std::fs::metadata(&program).unwrap();
    home.install("1.2.4").unwrap();
    use std::os::unix::fs::MetadataExt;
    assert_eq!(
        std::fs::metadata(&program).unwrap().ino(),
        before.ino(),
        "same file"
    );
}

/// The version-name rule pruning relies on, and release precedence for the
/// update-only "newer" refusal.
#[farhelm_testtrace::test]
fn version_names_and_precedence() {
    for good in ["1.2.3", "0.29.0-dev.0", "0.29.0-rc.12", "0.0.0-unreleased"] {
        assert!(is_version_folder_name(good), "{good}");
    }
    for bad in [
        "v1.2.3",
        "1.2",
        "01.2.3",
        "1.2.3-beta.1",
        "installed",
        "1.2.3-rc.01",
    ] {
        assert!(!is_version_folder_name(bad), "{bad}");
    }
    assert!(newer_than("0.29.0", "0.29.0-rc.1"));
    assert!(newer_than("0.29.0-rc.1", "0.29.0-dev.3"));
    assert!(!newer_than("0.28.2", "0.29.0-dev.0"));
    assert!(!newer_than("not-a-version", "1.0.0"));
}
