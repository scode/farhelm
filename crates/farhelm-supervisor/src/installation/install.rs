//! The locked part of the Mac installer: everything `scripts/install.sh`
//! used to do between taking its lock folder and releasing it, now done by
//! the new release's own `farhelm` (`farhelm internal install`), because a
//! shell cannot hold folders open or take kernel locks.
//!
//! The script still resolves the version, downloads and checks the release,
//! and stages its files (both programs, the icon, and the forwarder and
//! `Info.plist` texts it writes) in a private folder; it then hands over,
//! and prints the closing messages after this returns. This module takes
//! the installation lock and the old installers' lock folder, opens
//! `~/Applications` and the app through held descriptors, and from then on
//! changes the app only through them (the design's sections 2 to 5 and 7).
//!
//! The order of an update, each step durable before the next depends on it:
//!
//! 1. the ownership record's next "installing" generation;
//! 2. repair of what a killed earlier run left (missing `MacOS/` or
//!    `Resources/`, stray `.farhelm-new.*` files, trash and staging folders
//!    beside the app);
//! 3. the new version's folder, staged beside the app and renamed into
//!    `Versions/`, then `Versions/` flushed (an existing complete folder for
//!    the same version is reused, never rewritten, because its users hold
//!    locks on that very file);
//! 4. the app program, the icon and the forwarder, then `MacOS/` flushed:
//!    an older app's updater judges success by the Installed record, so the
//!    program must be on disk before the record names the new version;
//! 5. the Installed record, then `Info.plist` last;
//! 6. the replaced names flushed, then the record's "complete" generation;
//! 7. pruning: a version goes only if today's rule lets it go (not the
//!    target, not the previously Installed version, not one a Running record
//!    names) and nothing holds its version lock, by renaming it into a trash
//!    folder beside the app under an exclusive lock and deleting the trash
//!    once the rename is on disk;
//! 8. the Terminal link, and its folder flushed.
//!
//! An app without `Versions/` (a fresh install, or an installation from
//! before the side-by-side layout) is instead built whole under a temporary
//! name beside its final place, flushed, and renamed into place.

use super::dirs::{self, EntryKind};
use super::{
    InstallState, OwnershipRecord, RECORD_FILE, RECORD_LIMIT, full_sync, lock_installation,
    take_compat_lock_folder,
};
use std::fs::File;
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The app's name in `~/Applications`.
pub const APP_NAME: &str = "Farhelm.app";

/// Prefixes of the folders this module creates beside the app. Only the
/// locked work creates them, so under the lock any that exist were left by
/// a run that died, and are removed.
const STAGE_PREFIX: &str = ".farhelm-stage.";
const TRASH_PREFIX: &str = ".farhelm-trash.";

/// What the script hands over.
#[derive(Debug, Clone)]
pub struct InstallRequest {
    /// The script's private folder holding `farhelm`, `farhelm-desktop`,
    /// `Farhelm.icns`, `forwarder` and `Info.plist`, all checked.
    pub staging: PathBuf,
    /// The normalized version being installed (`1.2.3`, no `v`).
    pub version: String,
    /// `~/Applications`.
    pub applications: PathBuf,
    /// `~/.local/bin`, where the Terminal link goes.
    pub bin_dir: PathBuf,
    /// The canonical path of the Terminal link, which the record names.
    pub link: PathBuf,
    /// Running records whose versions pruning keeps (the default state
    /// folder's and the one `XDG_STATE_HOME` names).
    pub running_records: Vec<PathBuf>,
    /// An update started by an older app's updater: refuses a missing app,
    /// an interrupted uninstall, and going back over a newer Installed
    /// version.
    pub update_only: bool,
    /// The inode of the app folder the script established, before taking
    /// the lock, is one an older installer built (without this layout's
    /// record), so that folder may be replaced whole. Bound to the inode
    /// because the script checks before the lock: an app put at the place
    /// after the check is a different folder, and is refused rather than
    /// replaced on another folder's say-so.
    pub replace_legacy_bundle: Option<u64>,
    /// How long to wait for a running install or uninstall.
    pub lock_wait: Duration,
}

/// How a run ended well.
#[derive(Debug, PartialEq, Eq)]
pub struct Installed {
    /// An existing installation was updated (the closing message then says
    /// to reopen Farhelm), as opposed to a fresh install.
    pub updated: bool,
    /// Notes for the closing message (a foreign file kept aside).
    pub notes: Vec<String>,
}

/// Why a run stopped: one line for the error marker, then lines of advice.
#[derive(Debug, PartialEq, Eq)]
pub struct Refusal {
    pub message: String,
    pub advice: Vec<String>,
}

impl Refusal {
    fn new(message: impl Into<String>, advice: &[&str]) -> Self {
        Self {
            message: message.into(),
            advice: advice.iter().map(|line| line.to_string()).collect(),
        }
    }
}

/// The named points of a run, for tests that stop it after each one and
/// check what is on disk (in production the hook does nothing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    RecordInstalling,
    Repaired,
    VersionPlaced,
    AppProgram,
    Icon,
    Forwarder,
    InstalledRecord,
    InfoPlist,
    RecordComplete,
    Pruned,
    /// A rebuild's whole app is staged and flushed beside its place.
    BundleBuilt,
    BundlePlaced,
    Link,
}

/// A hook called after each [`Step`]; an error stops the run there, as a
/// kill would.
pub type StepHook<'a> = &'a mut dyn FnMut(Step) -> io::Result<()>;

/// Run the locked install.
pub fn install(request: &InstallRequest, hook: StepHook<'_>) -> Result<Installed, Refusal> {
    let app_path = request.applications.join(APP_NAME);
    let fail = |what: &str, error: &dyn std::fmt::Display| {
        Refusal::new(
            format!(
                "installing {} failed at: {what}: {error}",
                app_path.display()
            ),
            &["Re-run the installer to retry."],
        )
    };
    let lock = lock_installation(&request.applications, request.lock_wait).map_err(|error| {
        match error {
            super::InstallLockError::Busy => Refusal::new(
                "another Farhelm install or uninstall is running right now; wait for it to finish \
                 and re-run",
                &[],
            ),
            super::InstallLockError::Unsupported(path) => unsupported_refusal(&path),
            other => fail("taking the installation lock", &other),
        }
    })?;
    let compat = take_compat_lock_folder(&lock).map_err(|error| match error {
        super::CompatLockError::HeldByOlderTool(path) => Refusal::new(
            format!(
                "another farhelm install is changing {} right now, or one was interrupted while \
                 doing so; wait a moment and re-run -- if this persists, remove {} by hand",
                app_path.display(),
                path.display()
            ),
            &[],
        ),
        other => fail("taking the lock folder", &other),
    })?;
    let applications = lock.applications();
    let ctx = Context {
        request,
        applications,
        app_path: &app_path,
    };
    let outcome = ctx.run(hook);
    // The folder goes whatever happened; the kernel lock goes with `lock`.
    let released = compat.release();
    let installed = outcome?;
    released.map_err(|error| fail("removing the lock folder", &error))?;
    Ok(installed)
}

/// The refusal where `~/Applications` cannot be locked (design, section 7:
/// such an installation stays usable on its current version).
fn unsupported_refusal(applications: &Path) -> Refusal {
    Refusal::new(
        format!(
            "{} is on a volume where files cannot be locked, which installing or updating Farhelm \
             safely needs; nothing was changed",
            applications.display()
        ),
        &[
            "The installed Farhelm, if any, keeps working on its current version. To remove it, quit",
            "Farhelm and delete ~/Applications/Farhelm.app and ~/.local/bin/farhelm.",
        ],
    )
}

/// One run's fixed inputs.
struct Context<'a> {
    request: &'a InstallRequest,
    applications: BorrowedFd<'a>,
    app_path: &'a Path,
}

/// The device and inode of the entry at the app's place when it was
/// inspected, so a rebuild can tell it is moving that entry and not one
/// Finder put there since.
type Identity = (u64, u64);

/// What the run found at the app's place.
enum Found {
    /// An installation of this layout: its record, and whether it has
    /// `Versions/` (else it is rebuilt whole, keeping the record's id).
    Ours {
        contents: OwnedFd,
        record: OwnershipRecord,
        versioned: bool,
        identity: Identity,
    },
    /// An app from before the side-by-side layout that the script vouched
    /// for (its identity), or no app at all (`None`).
    Replaceable { existing: Option<Identity> },
}

impl Context<'_> {
    fn fail(&self, what: &str, error: &dyn std::fmt::Display) -> Refusal {
        Refusal::new(
            format!(
                "installing {} failed at: {what}: {error}",
                self.app_path.display()
            ),
            &["Re-run the installer to retry."],
        )
    }

    fn run(&self, hook: StepHook<'_>) -> Result<Installed, Refusal> {
        // Before inspecting, so an app a killed rebuild left aside is back
        // at its place when the run decides what is there.
        self.remove_own_leftovers()?;
        let found = self.inspect()?;
        if self.request.update_only {
            self.check_update_only(&found)?;
        }
        let updated = match found {
            Found::Ours {
                contents,
                record,
                versioned: true,
                ..
            } => {
                self.update(contents.as_fd(), &record, hook)?;
                true
            }
            Found::Ours {
                record,
                versioned: false,
                identity,
                ..
            } => {
                self.rebuild(Some(&record), Some(identity), hook)?;
                true
            }
            Found::Replaceable { existing } => {
                self.rebuild(None, existing, hook)?;
                existing.is_some()
            }
        };
        let notes = self.place_link(hook)?;
        Ok(Installed { updated, notes })
    }

    /// Classify what is at the app's place, refusing anything this installer
    /// must not change.
    fn inspect(&self) -> Result<Found, Refusal> {
        let app_path = self.app_path;
        let kind = dirs::entry_kind(self.applications, APP_NAME)
            .map_err(|error| self.fail("inspecting the app", &error))?;
        let not_a_bundle = || {
            Refusal::new(
                format!(
                    "{} exists and is not an app bundle this installer built; refusing to replace it.",
                    app_path.display()
                ),
                &["Remove or rename that bundle and re-run."],
            )
        };
        let identity = dirs::identity_at(self.applications, APP_NAME)
            .map_err(|error| self.fail("inspecting the app", &error))?;
        match kind {
            EntryKind::Absent => return Ok(Found::Replaceable { existing: None }),
            EntryKind::Folder => {}
            // Never followed: refused with the advice older installers gave
            // for a linked app folder.
            EntryKind::Symlink => return Err(self.require_folder(kind, app_path).unwrap_err()),
            // No installer ever built a file at the app's place, and the
            // script's check never approves one.
            EntryKind::File | EntryKind::Other => return Err(not_a_bundle()),
        }
        let identity =
            identity.ok_or_else(|| self.fail("inspecting the app", &"it disappeared"))?;
        let app = dirs::open_folder(self.applications, APP_NAME)
            .map_err(|error| self.fail("opening the app", &error))?
            .ok_or_else(|| self.fail("opening the app", &"it disappeared"))?;
        let contents_kind = dirs::entry_kind(app.as_fd(), "Contents")
            .map_err(|error| self.fail("inspecting the app", &error))?;
        if contents_kind == EntryKind::Symlink {
            return Err(self
                .require_folder(contents_kind, &app_path.join("Contents"))
                .unwrap_err());
        }
        let record = match dirs::open_folder(app.as_fd(), "Contents") {
            Ok(Some(contents)) => dirs::read_small(contents.as_fd(), RECORD_FILE, RECORD_LIMIT)
                .ok()
                .flatten()
                .and_then(|bytes| OwnershipRecord::parse(&bytes))
                .map(|record| (contents, record)),
            _ => None,
        };
        let Some((contents, record)) = record else {
            return if self.request.replace_legacy_bundle == Some(identity.1) {
                Ok(Found::Replaceable {
                    existing: Some(identity),
                })
            } else {
                Err(not_a_bundle())
            };
        };
        // An installation of this layout is only updated through real
        // folders: a link or a file where a folder belongs is refused before
        // anything changes, as before.
        let version = self.request.version.as_str();
        let mut versioned = false;
        for name in ["MacOS", "Resources", "Versions"] {
            let kind = dirs::entry_kind(contents.as_fd(), name)
                .map_err(|error| self.fail("inspecting the app", &error))?;
            self.require_folder(kind, &app_path.join("Contents").join(name))?;
            if name == "Versions" {
                versioned = kind == EntryKind::Folder;
            }
        }
        if versioned {
            let versions = dirs::open_folder(contents.as_fd(), "Versions")
                .map_err(|error| self.fail("opening Versions", &error))?
                .ok_or_else(|| self.fail("opening Versions", &"it disappeared"))?;
            let kind = dirs::entry_kind(versions.as_fd(), version)
                .map_err(|error| self.fail("inspecting the app", &error))?;
            self.require_folder(kind, &app_path.join("Contents/Versions").join(version))?;
            for (parent, name, label) in [
                (
                    contents.as_fd(),
                    "MacOS/farhelm-desktop",
                    "Contents/MacOS/farhelm-desktop",
                ),
                (contents.as_fd(), "MacOS/farhelm", "Contents/MacOS/farhelm"),
                (
                    contents.as_fd(),
                    "Resources/Farhelm.icns",
                    "Contents/Resources/Farhelm.icns",
                ),
                (contents.as_fd(), "Info.plist", "Contents/Info.plist"),
                (
                    contents.as_fd(),
                    "Versions/installed",
                    "Contents/Versions/installed",
                ),
                (
                    contents.as_fd(),
                    RECORD_FILE,
                    "Contents/.farhelm-installation",
                ),
            ] {
                // Followed on purpose: a link to a folder at a file's place
                // is refused like a folder, as older installers did.
                let is_folder = match name.split_once('/') {
                    Some((folder, file)) => match dirs::open_folder(parent, folder) {
                        Ok(Some(folder)) => dirs::resolves_to_folder(folder.as_fd(), file),
                        _ => false,
                    },
                    None => dirs::resolves_to_folder(parent, name),
                };
                if is_folder {
                    return Err(Refusal::new(
                        format!(
                            "{} is a folder, and this installer only replaces files at this path; \
                             nothing was changed.",
                            app_path.join(label).display()
                        ),
                        &["Move it out of the way, or move Farhelm.app aside, and re-run."],
                    ));
                }
            }
        }
        drop(app);
        Ok(Found::Ours {
            contents,
            record,
            versioned,
            identity,
        })
    }

    /// Refuse a link or a non-folder where a folder belongs; absent is fine.
    fn require_folder(&self, kind: EntryKind, path: &Path) -> Result<(), Refusal> {
        match kind {
            EntryKind::Absent | EntryKind::Folder => Ok(()),
            EntryKind::Symlink => Err(Refusal::new(
                format!(
                    "{} is a symbolic link, and this installer only updates Farhelm.app through \
                     real folders; nothing was changed.",
                    path.display()
                ),
                &["Replace the link with a real folder, or move Farhelm.app aside, and re-run."],
            )),
            EntryKind::File | EntryKind::Other => Err(Refusal::new(
                format!(
                    "{} is not a folder, and this installer only updates Farhelm.app through real \
                     folders; nothing was changed.",
                    path.display()
                ),
                &["Move it out of the way, or move Farhelm.app aside, and re-run."],
            )),
        }
    }

    /// The refusals of a run started by an older app's updater (design,
    /// section 7): it never reinstalls a removed app, never finishes into a
    /// half-removed one, and never goes back over a newer Installed version
    /// (a Terminal install the user made meanwhile). An interrupted install
    /// is finished, since an older app cannot show the notice that would
    /// tell its user to run the installer.
    fn check_update_only(&self, found: &Found) -> Result<(), Refusal> {
        let (contents, record) = match found {
            Found::Ours {
                contents, record, ..
            } => (contents, record),
            Found::Replaceable { .. } => {
                return Err(Refusal::new(
                    format!(
                        "Farhelm is not installed at {}, and an update does not install it again",
                        self.app_path.display()
                    ),
                    &["Run the installer from Terminal to install Farhelm."],
                ));
            }
        };
        if record.state == InstallState::Removing {
            return Err(Refusal::new(
                format!(
                    "an uninstall of {} was interrupted, so it is not updated",
                    self.app_path.display()
                ),
                &[
                    "Run ~/.local/bin/farhelm uninstall again to finish it, or run the installer",
                    "from Terminal to keep Farhelm.",
                ],
            ));
        }
        let installed = dirs::open_folder(contents.as_fd(), "Versions")
            .ok()
            .flatten()
            .and_then(|versions| {
                dirs::read_small(versions.as_fd(), "installed", 4096)
                    .ok()
                    .flatten()
            })
            .map(|bytes| String::from_utf8_lossy(&bytes).trim_end().to_string());
        if let Some(installed) = installed
            && newer_than(&installed, &self.request.version)
        {
            return Err(Refusal::new(
                format!(
                    "{} already has Farhelm {installed}, newer than {}; an update does not go back",
                    self.app_path.display(),
                    self.request.version
                ),
                &[],
            ));
        }
        Ok(())
    }

    // -- the update path ---------------------------------------------------

    fn update(
        &self,
        contents: BorrowedFd<'_>,
        record: &OwnershipRecord,
        hook: StepHook<'_>,
    ) -> Result<(), Refusal> {
        let request = self.request;
        let version = request.version.as_str();
        let staged = |name: &str| request.staging.join(name);
        let mut record = record.advanced(InstallState::Installing);
        record.link = path_bytes(&request.link);
        self.write_record(contents, &record)?;
        step(hook, Step::RecordInstalling).map_err(|e| self.fail("stopping", &e))?;

        // Repair what a killed older run left inside the app, exactly as
        // older installers do (this module's own leftovers beside the app
        // went before inspection).
        let macos = dirs::open_or_create_folder(contents, "MacOS")
            .map_err(|e| self.fail("recreating the app's folders", &e))?;
        let resources = dirs::open_or_create_folder(contents, "Resources")
            .map_err(|e| self.fail("recreating the app's folders", &e))?;
        let versions = dirs::open_folder(contents, "Versions")
            .map_err(|e| self.fail("opening Versions", &e))?
            .ok_or_else(|| self.fail("opening Versions", &"it disappeared"))?;
        for folder in [contents, macos.as_fd(), resources.as_fd(), versions.as_fd()] {
            dirs::remove_temporaries(folder).map_err(|e| self.fail("removing leftovers", &e))?;
        }
        step(hook, Step::Repaired).map_err(|e| self.fail("stopping", &e))?;

        let previous_installed = dirs::read_small(versions.as_fd(), "installed", 4096)
            .ok()
            .flatten()
            .map(|bytes| String::from_utf8_lossy(&bytes).trim_end().to_string());

        // The new version's folder: reused when complete, else staged
        // beside the app and renamed in.
        let complete = dirs::open_folder(versions.as_fd(), version)
            .ok()
            .flatten()
            .is_some_and(|folder| {
                dirs::entry_kind(folder.as_fd(), "farhelm").ok() == Some(EntryKind::File)
            });
        if !complete {
            let stage = self.stage_folder()?;
            let folder = dirs::open_or_create_folder(stage.fd(), "version")
                .map_err(|e| self.fail(&format!("creating the {version} folder"), &e))?;
            dirs::replace_file_from(folder.as_fd(), "farhelm", &staged("farhelm"), 0o755)
                .map_err(|e| self.fail("copying farhelm", &e))?;
            full_sync(folder.as_fd()).map_err(|e| self.fail("flushing the new version", &e))?;
            // An incomplete folder for this version (a killed older run's)
            // goes to trash first; nothing can hold a lock on a program it
            // does not have.
            let mut replaced = None;
            if dirs::entry_kind(versions.as_fd(), version)
                .map_err(|e| self.fail("inspecting Versions", &e))?
                != EntryKind::Absent
            {
                let trash = self.trash_folder()?;
                rustix::fs::renameat(versions.as_fd(), version, trash.fd(), version).map_err(
                    |e| self.fail(&format!("moving the incomplete {version} folder aside"), &e),
                )?;
                replaced = Some(trash);
            }
            rustix::fs::renameat(stage.fd(), "version", versions.as_fd(), version)
                .map_err(|e| self.fail(&format!("moving the {version} folder into place"), &e))?;
            full_sync(versions.as_fd()).map_err(|e| self.fail("flushing Versions", &e))?;
            stage.remove();
            if let Some(trash) = replaced {
                full_sync(self.applications)
                    .map_err(|e| self.fail("flushing ~/Applications", &e))?;
                trash.remove();
            }
        }
        step(hook, Step::VersionPlaced).map_err(|e| self.fail("stopping", &e))?;

        dirs::replace_file_from(
            macos.as_fd(),
            "farhelm-desktop",
            &staged("farhelm-desktop"),
            0o755,
        )
        .map_err(|e| self.fail("replacing the app's main program", &e))?;
        step(hook, Step::AppProgram).map_err(|e| self.fail("stopping", &e))?;
        dirs::replace_file_from(
            resources.as_fd(),
            "Farhelm.icns",
            &staged("Farhelm.icns"),
            0o644,
        )
        .map_err(|e| self.fail("replacing the icon", &e))?;
        step(hook, Step::Icon).map_err(|e| self.fail("stopping", &e))?;
        if !dirs::same_contents(macos.as_fd(), "farhelm", &staged("forwarder")).unwrap_or(false) {
            dirs::replace_file_from(macos.as_fd(), "farhelm", &staged("forwarder"), 0o755)
                .map_err(|e| self.fail("replacing the forwarder", &e))?;
        }
        step(hook, Step::Forwarder).map_err(|e| self.fail("stopping", &e))?;
        // The barrier an older app's updater depends on: the program is on
        // disk before the Installed record names the new version.
        full_sync(macos.as_fd()).map_err(|e| self.fail("flushing MacOS", &e))?;

        dirs::replace_file(
            versions.as_fd(),
            "installed",
            &mut format!("{version}\n").as_bytes(),
            0o644,
        )
        .map_err(|e| self.fail("replacing the Installed record", &e))?;
        step(hook, Step::InstalledRecord).map_err(|e| self.fail("stopping", &e))?;
        dirs::replace_file_from(contents, "Info.plist", &staged("Info.plist"), 0o644)
            .map_err(|e| self.fail("replacing Info.plist", &e))?;
        step(hook, Step::InfoPlist).map_err(|e| self.fail("stopping", &e))?;

        for (folder, what) in [
            (versions.as_fd(), "flushing Versions"),
            (resources.as_fd(), "flushing Resources"),
            (contents, "flushing Contents"),
        ] {
            full_sync(folder).map_err(|e| self.fail(what, &e))?;
        }
        let record = record.advanced(InstallState::Complete);
        self.write_record(contents, &record)?;
        step(hook, Step::RecordComplete).map_err(|e| self.fail("stopping", &e))?;

        self.prune(versions.as_fd(), previous_installed.as_deref())?;
        step(hook, Step::Pruned).map_err(|e| self.fail("stopping", &e))?;
        Ok(())
    }

    /// Write `record` durably inside `contents`: flushed under a temporary
    /// name, renamed over the old one, and `Contents/` flushed.
    fn write_record(
        &self,
        contents: BorrowedFd<'_>,
        record: &OwnershipRecord,
    ) -> Result<(), Refusal> {
        dirs::replace_file(
            contents,
            RECORD_FILE,
            &mut record.render().as_slice(),
            0o600,
        )
        .and_then(|()| full_sync(contents))
        .map_err(|e| self.fail("writing installer ownership metadata", &e))
    }

    /// Remove every version that today's rule lets go and nothing holds.
    fn prune(
        &self,
        versions: BorrowedFd<'_>,
        previous_installed: Option<&str>,
    ) -> Result<(), Refusal> {
        let mut keep: Vec<String> = vec![self.request.version.clone()];
        keep.extend(previous_installed.map(str::to_string));
        for record in &self.request.running_records {
            if let Ok(text) = std::fs::read_to_string(record)
                && let Some(line) = text.lines().next()
            {
                keep.push(line.to_string());
            }
        }
        let names = dirs::list(versions).map_err(|e| self.fail("listing Versions", &e))?;
        let mut trash: Option<StagedFolder<'_>> = None;
        for name in names {
            if keep.contains(&name)
                || !is_version_folder_name(&name)
                || dirs::entry_kind(versions, &name).ok() != Some(EntryKind::Folder)
            {
                continue;
            }
            let Ok(Some(folder)) = dirs::open_folder(versions, &name) else {
                continue;
            };
            // Busy keeps the version; so does anything that cannot be
            // checked. A folder without its program has no users to ask.
            let held = match dirs::open_file(folder.as_fd(), "farhelm") {
                Ok(Some(program)) => match program.try_lock() {
                    Ok(()) => Some(program),
                    Err(_) => continue,
                },
                Ok(None) => None,
                Err(_) => continue,
            };
            if trash.is_none() {
                trash = Some(self.trash_folder()?);
            }
            let bin = trash.as_ref().expect("just created");
            if rustix::fs::renameat(versions, name.as_str(), bin.fd(), name.as_str()).is_err() {
                continue;
            }
            drop(held);
        }
        if let Some(bin) = trash {
            full_sync(versions).map_err(|e| self.fail("flushing Versions", &e))?;
            full_sync(bin.fd()).map_err(|e| self.fail("flushing the trash", &e))?;
            bin.remove();
        }
        Ok(())
    }

    // -- the rebuild path --------------------------------------------------

    /// Build the whole app beside its place and rename it in. `record` is
    /// the existing installation's (its id carries over); `exists` says an
    /// app is at the place now and moves aside first.
    fn rebuild(
        &self,
        record: Option<&OwnershipRecord>,
        existing: Option<Identity>,
        hook: StepHook<'_>,
    ) -> Result<(), Refusal> {
        let request = self.request;
        let version = request.version.as_str();
        let staged = |name: &str| request.staging.join(name);
        let stage = self.stage_folder()?;
        let build = || -> io::Result<OwnedFd> {
            let app = dirs::open_or_create_folder(stage.fd(), APP_NAME)?;
            let contents = dirs::open_or_create_folder(app.as_fd(), "Contents")?;
            let macos = dirs::open_or_create_folder(contents.as_fd(), "MacOS")?;
            let resources = dirs::open_or_create_folder(contents.as_fd(), "Resources")?;
            let versions = dirs::open_or_create_folder(contents.as_fd(), "Versions")?;
            let folder = dirs::open_or_create_folder(versions.as_fd(), version)?;
            dirs::replace_file_from(folder.as_fd(), "farhelm", &staged("farhelm"), 0o755)?;
            dirs::replace_file_from(
                macos.as_fd(),
                "farhelm-desktop",
                &staged("farhelm-desktop"),
                0o755,
            )?;
            dirs::replace_file_from(macos.as_fd(), "farhelm", &staged("forwarder"), 0o755)?;
            dirs::replace_file_from(
                resources.as_fd(),
                "Farhelm.icns",
                &staged("Farhelm.icns"),
                0o644,
            )?;
            dirs::replace_file(
                versions.as_fd(),
                "installed",
                &mut format!("{version}\n").as_bytes(),
                0o644,
            )?;
            dirs::replace_file_from(contents.as_fd(), "Info.plist", &staged("Info.plist"), 0o644)?;
            let mut next = match record {
                Some(record) => record.advanced(InstallState::Complete),
                None => OwnershipRecord::fresh(&request.link).advanced(InstallState::Complete),
            };
            next.link = path_bytes(&request.link);
            dirs::replace_file(
                contents.as_fd(),
                RECORD_FILE,
                &mut next.render().as_slice(),
                0o600,
            )?;
            dirs::sync_tree(app.as_fd())?;
            full_sync(stage.fd())?;
            Ok(app)
        };
        build().map_err(|e| self.fail("building the app", &e))?;
        step(hook, Step::BundleBuilt).map_err(|e| self.fail("stopping", &e))?;
        // The app being replaced moves aside first, and only if it is still
        // the one inspected: Finder can put another app at the place at any
        // moment, and that one must neither be deleted with the previous app
        // nor be replaced. Publishing never replaces anything either.
        let no_replace = rustix::fs::RenameFlags::NOREPLACE;
        if let Some(expected) = existing {
            rustix::fs::renameat(self.applications, APP_NAME, stage.fd(), "previous").map_err(
                |e| {
                    self.fail(
                        "moving the previous bundle aside (grant your terminal App Management in \
                         System Settings > Privacy & Security if this said 'Operation not \
                         permitted')",
                        &e,
                    )
                },
            )?;
            if dirs::identity_at(stage.fd(), "previous").ok().flatten() != Some(expected) {
                // Put back what was moved; if that fails it stays in the
                // staging folder, which the next run restores from.
                if rustix::fs::renameat_with(
                    stage.fd(),
                    "previous",
                    self.applications,
                    APP_NAME,
                    no_replace,
                )
                .is_ok()
                {
                    stage.remove();
                }
                return Err(Refusal::new(
                    format!(
                        "{} changed while it was being installed; nothing was replaced",
                        self.app_path.display()
                    ),
                    &["Re-run the installer."],
                ));
            }
        }
        if let Err(error) = rustix::fs::renameat_with(
            stage.fd(),
            APP_NAME,
            self.applications,
            APP_NAME,
            no_replace,
        ) {
            // Put the previous app back rather than leave none at its place;
            // if that fails too, the next run restores it from the staging
            // folder, which is therefore kept.
            if existing.is_some() {
                let _ = rustix::fs::renameat_with(
                    stage.fd(),
                    "previous",
                    self.applications,
                    APP_NAME,
                    no_replace,
                );
            }
            return Err(self.fail("moving the staged bundle into place", &error));
        }
        full_sync(self.applications).map_err(|e| self.fail("flushing ~/Applications", &e))?;
        step(hook, Step::BundlePlaced).map_err(|e| self.fail("stopping", &e))?;
        stage.remove();
        Ok(())
    }

    // -- beside the app ----------------------------------------------------

    /// Remove staging and trash folders a run that died left beside the
    /// app. The lock proves no run is using them. Only folders this module
    /// could have made are touched: the exact name it gives them (its prefix
    /// and 32 hex digits) and only the entries it puts in them (version
    /// folders in a trash folder; `version`, `Farhelm.app` or `previous` in
    /// a staging folder). Anything else with a similar name is someone
    /// else's and stays.
    ///
    /// A rebuild killed between moving the previous app aside and placing
    /// the new one left the previous app in its staging folder; with
    /// nothing at the app's place it is put back first, and if that fails
    /// the run refuses with the folder kept, rather than delete the only
    /// copy.
    fn remove_own_leftovers(&self) -> Result<(), Refusal> {
        let names =
            dirs::list(self.applications).map_err(|e| self.fail("listing ~/Applications", &e))?;
        for name in names {
            let Some(kind) = leftover_kind(&name) else {
                continue;
            };
            if dirs::entry_kind(self.applications, &name).ok() != Some(EntryKind::Folder) {
                continue;
            }
            let Ok(Some(folder)) = dirs::open_folder(self.applications, &name) else {
                continue;
            };
            let Ok(children) = dirs::list(folder.as_fd()) else {
                continue;
            };
            let ours = children.iter().all(|child| match kind {
                Leftover::Trash => is_version_folder_name(child),
                Leftover::Stage => matches!(child.as_str(), "version" | APP_NAME | "previous"),
            });
            if !ours {
                continue;
            }
            if children.iter().any(|child| child == "previous")
                && dirs::entry_kind(self.applications, APP_NAME).ok() == Some(EntryKind::Absent)
            {
                rustix::fs::renameat_with(
                    folder.as_fd(),
                    "previous",
                    self.applications,
                    APP_NAME,
                    rustix::fs::RenameFlags::NOREPLACE,
                )
                .map_err(|e| {
                    self.fail(
                        &format!(
                            "putting back the app an interrupted run left in {}",
                            self.request.applications.join(&name).display()
                        ),
                        &e,
                    )
                })?;
            }
            self.flush_moves_into(folder.as_fd())
                .map_err(|e| self.fail("flushing an earlier run's leftovers", &e))?;
            dirs::remove_tree(self.applications, &name)
                .map_err(|e| self.fail("removing an earlier run's leftovers", &e))?;
        }
        Ok(())
    }

    /// Make every move that can have put something in the leftover
    /// `folder` durable before it is deleted: a version moved out of the
    /// app's `Versions/` into trash, or a previous app moved out of (and
    /// perhaps just back into) `~/Applications`.
    ///
    /// The run that made the move may have been killed before it flushed
    /// both folders, and a leftover does not say how far it got. Deleting
    /// first would break the design's rule that nothing is deleted before
    /// its removal from view is on disk: after a power cut, the version's
    /// old name could come back inside `Versions/` with half its contents
    /// gone. Any failure stops the cleanup, and the run with it.
    fn flush_moves_into(&self, folder: BorrowedFd<'_>) -> std::io::Result<()> {
        // A place that holds no such folder chain (nothing, a link, a file:
        // the inspection that follows refuses those) has no Versions/ a
        // version could have left.
        let open = |dir: BorrowedFd<'_>, name: &str| dirs::open_folder(dir, name).ok().flatten();
        if let Some(app) = open(self.applications, APP_NAME)
            && let Some(contents) = open(app.as_fd(), "Contents")
            && let Some(versions) = open(contents.as_fd(), "Versions")
        {
            full_sync(versions.as_fd())?;
        }
        full_sync(self.applications)?;
        full_sync(folder)
    }

    fn stage_folder(&self) -> Result<StagedFolder<'_>, Refusal> {
        StagedFolder::create(self.applications, STAGE_PREFIX)
            .map_err(|e| self.fail("creating a private work directory in ~/Applications", &e))
    }

    fn trash_folder(&self) -> Result<StagedFolder<'_>, Refusal> {
        StagedFolder::create(self.applications, TRASH_PREFIX)
            .map_err(|e| self.fail("creating a trash folder in ~/Applications", &e))
    }

    // -- the Terminal link -------------------------------------------------

    /// Point `~/.local/bin/farhelm` at the forwarder, keeping today's checks
    /// and notes about files this installer did not put there, then flush
    /// the folder. Outside the app these are public names and keep their
    /// path-based checks (design, section 3).
    fn place_link(&self, hook: StepHook<'_>) -> Result<Vec<String>, Refusal> {
        let bin = &self.request.bin_dir;
        let forwarder = self.app_path.join("Contents/MacOS/farhelm");
        let mut notes = Vec::new();
        let flat = FlatRecord::read(&bin.join(".farhelm-installation"));
        let stamp = utc_stamp();
        for name in ["farhelm", "farhelm-desktop"] {
            let dest = bin.join(name);
            let meta = std::fs::symlink_metadata(&dest);
            match meta {
                Err(_) => continue,
                Ok(meta) if meta.file_type().is_symlink() => {
                    if name == "farhelm"
                        && std::fs::read_link(&dest).ok().as_deref() == Some(&forwarder)
                    {
                        continue;
                    }
                }
                Ok(meta) if meta.is_file() => {
                    if let Some(recorded) = flat.as_ref().and_then(|flat| flat.digest(name))
                        && sha256_of(&dest).as_deref() == Some(recorded.as_str())
                    {
                        if name == "farhelm-desktop" {
                            let _ = std::fs::remove_file(&dest);
                        }
                        continue;
                    }
                }
                Ok(_) => {}
            }
            if name != "farhelm" {
                continue;
            }
            if std::fs::symlink_metadata(&dest).is_ok_and(|meta| meta.is_dir()) {
                return Err(Refusal::new(
                    format!(
                        "{} is a directory; the app is installed, but the Terminal link was not \
                         created; move it aside and re-run",
                        dest.display()
                    ),
                    &[],
                ));
            }
            let mut kept = bin.join(format!("{name}.replaced-{stamp}"));
            if std::fs::symlink_metadata(&kept).is_ok() {
                kept = bin.join(format!("{name}.replaced-{stamp}-{}", std::process::id()));
            }
            std::fs::rename(&dest, &kept).map_err(|_| {
                Refusal::new(
                    format!(
                        "could not keep the existing {} as {}; move it aside and re-run the installer",
                        dest.display(),
                        kept.display()
                    ),
                    &[],
                )
            })?;
            let kept_name = kept.file_name().unwrap_or_default().to_string_lossy();
            notes.push(format!(
                "ℹ️  ~/.local/bin/{name} was not installed by this installer, so it was renamed\n   to \
                 ~/.local/bin/{kept_name} - farhelm installation still\n   proceeded."
            ));
        }
        let link = bin.join("farhelm");
        if std::fs::read_link(&link).ok().as_deref() != Some(&forwarder) {
            let stage = bin.join(format!(".farhelm-link.{}", std::process::id()));
            let _ = std::fs::remove_file(&stage);
            std::os::unix::fs::symlink(&forwarder, &stage)
                .map_err(|e| self.fail("creating the Terminal link", &e))?;
            if let Err(error) = std::fs::rename(&stage, &link) {
                let _ = std::fs::remove_file(&stage);
                return Err(self.fail(
                    &format!("installing the Terminal link {}", link.display()),
                    &error,
                ));
            }
        }
        let flat_record = bin.join(".farhelm-installation");
        if std::fs::symlink_metadata(&flat_record).is_ok_and(|meta| meta.is_file())
            && std::fs::remove_file(&flat_record).is_err()
        {
            notes.push(format!(
                "note: could not remove {}, the previous layout's record; it is safe to delete",
                flat_record.display()
            ));
        }
        File::open(bin)
            .and_then(|folder| full_sync(folder.as_fd()))
            .map_err(|e| self.fail("flushing ~/.local/bin", &e))?;
        step(hook, Step::Link).map_err(|e| self.fail("stopping", &e))?;
        Ok(notes)
    }
}

/// Which of this module's folders beside the app a name is, if it is one.
enum Leftover {
    Stage,
    Trash,
}

/// `name` is a folder this module made: its prefix and exactly the 32
/// lowercase hex digits [`StagedFolder::create`] appends.
fn leftover_kind(name: &str) -> Option<Leftover> {
    let (kind, suffix) = if let Some(suffix) = name.strip_prefix(STAGE_PREFIX) {
        (Leftover::Stage, suffix)
    } else {
        (Leftover::Trash, name.strip_prefix(TRASH_PREFIX)?)
    };
    (suffix.len() == 32
        && suffix
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
    .then_some(kind)
}

/// Call the hook for `at`.
fn step(hook: StepHook<'_>, at: Step) -> io::Result<()> {
    hook(at)
}

/// A folder this run created beside the app; removed by [`Self::remove`],
/// or left for the next run's cleanup if this run dies first.
struct StagedFolder<'a> {
    parent: BorrowedFd<'a>,
    name: String,
    fd: OwnedFd,
}

impl<'a> StagedFolder<'a> {
    fn create(parent: BorrowedFd<'a>, prefix: &str) -> io::Result<Self> {
        let name = format!("{prefix}{}", uuid::Uuid::new_v4().simple());
        rustix::fs::mkdirat(parent, name.as_str(), rustix::fs::Mode::RWXU)?;
        let fd = dirs::open_folder(parent, &name)?
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        Ok(Self { parent, name, fd })
    }

    fn fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }

    /// Delete the folder and what it holds. A failure leaves it for the
    /// next run, which removes leftovers under the lock.
    fn remove(self) {
        let _ = dirs::remove_tree(self.parent, &self.name);
    }
}

/// The record the flat layout before the app bundle wrote in
/// `~/.local/bin`: `farhelm-standalone`, a directory, then the CLI's and
/// the desktop program's SHA-256 digests, NUL-terminated. Only the digests
/// matter here: a file whose digest matches was this installer's own and is
/// replaced silently.
struct FlatRecord {
    fields: Vec<String>,
}

impl FlatRecord {
    fn read(path: &Path) -> Option<Self> {
        let meta = std::fs::symlink_metadata(path).ok()?;
        if !meta.is_file() {
            return None;
        }
        let bytes = std::fs::read(path).ok()?;
        let fields: Vec<String> = bytes
            .split(|byte| *byte == 0)
            .map(|field| String::from_utf8_lossy(field).into_owned())
            .collect();
        (fields.first().map(String::as_str) == Some("farhelm-standalone"))
            .then_some(Self { fields })
    }

    /// The digest recorded for `name`, read from the end (the directory
    /// field may contain anything): the desktop digest is the last field
    /// before the final terminator, the CLI's the one before it.
    fn digest(&self, name: &str) -> Option<String> {
        let mut fields = self.fields.clone();
        if fields.last().is_some_and(String::is_empty) {
            fields.pop();
        }
        let back = match name {
            "farhelm" => 1,
            "farhelm-desktop" => 0,
            _ => return None,
        };
        let index = fields.len().checked_sub(1 + back)?;
        let digest = fields.get(index)?.clone();
        (!digest.is_empty()).then_some(digest)
    }
}

fn sha256_of(path: &Path) -> Option<String> {
    use sha2::Digest;
    use std::io::Read;
    let mut hasher = sha2::Sha256::new();
    let mut file = File::open(path).ok()?;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Some(
        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
}

/// `YYYYmmddTHHMMSSZ` in UTC, the stamp the kept-aside name carries.
fn utc_stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let (days, rest) = (secs / 86_400, secs % 86_400);
    let (hour, minute, second) = (rest / 3600, rest % 3600 / 60, rest % 60);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}{month:02}{day:02}T{hour:02}{minute:02}{second:02}Z")
}

fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

/// Whether `name` is a version folder exactly as the installer names one: a
/// normalized release version, optionally `-rc.N` or `-dev.N`, or
/// `0.0.0-unreleased`. Only such folders are ever pruned.
pub fn is_version_folder_name(name: &str) -> bool {
    if name == "0.0.0-unreleased" {
        return true;
    }
    let number = |part: &str| {
        !part.is_empty()
            && part.bytes().all(|byte| byte.is_ascii_digit())
            && (part == "0" || !part.starts_with('0'))
    };
    let (core, pre) = match name.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (name, None),
    };
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() != 3 || !parts.iter().all(|part| number(part)) {
        return false;
    }
    match pre {
        None => true,
        Some(pre) => match pre.split_once('.') {
            Some(("rc" | "dev", n)) => number(n),
            _ => false,
        },
    }
}

/// Whether version `a` is newer than `b` by release precedence. A name that
/// is not a version compares as not newer, so it never blocks an update.
pub fn newer_than(a: &str, b: &str) -> bool {
    match (semver::Version::parse(a), semver::Version::parse(b)) {
        (Ok(a), Ok(b)) => a.cmp_precedence(&b) == std::cmp::Ordering::Greater,
        _ => false,
    }
}

#[cfg(test)]
mod tests;
