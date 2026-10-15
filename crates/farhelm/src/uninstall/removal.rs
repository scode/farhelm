//! Fixed-path removal after ownership preflight and confirmation.
//!
//! The plan's own order is the removal order; nothing here traverses a
//! directory or invents a target. A flat (Linux) installation removes its CLI
//! last so an ordinary failure leaves the retry command. The Mac app's plan is
//! ordered the same way for its CLI, which there is the Terminal link, the
//! forwarder, the Installed record and the running version (see
//! [`super::app::AppPlan`]).

use super::{
    app::{AppPlan, LinkPlan},
    ownership::{FlatPlan, OwnershipPlan},
    path_text,
};
use anyhow::{Context as _, Result};
use std::{
    fmt::Write as _,
    fs, io,
    path::{Path, PathBuf},
};

/// The narrow mutation seam makes partial failures reproducible under root too.
trait RemovalFs {
    fn remove_file(&mut self, path: &Path) -> io::Result<()>;
    fn remove_dir(&mut self, path: &Path) -> io::Result<()>;
}

struct NativeFs;

impl RemovalFs for NativeFs {
    fn remove_file(&mut self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }

    fn remove_dir(&mut self, path: &Path) -> io::Result<()> {
        fs::remove_dir(path)
    }
}

/// Removal through the app's folders, opened once and never by path inside
/// the app (design, section 6, "through the held folder"): a path under the
/// app's root is removed relative to its parent folder, reached from the
/// root through opened folders that never follow a link. Paths outside the
/// app (the Running records, the Terminal link, the app's own name in its
/// parent) are public names and keep today's path-based removal.
pub(crate) struct HeldFs {
    root: PathBuf,
    root_fd: std::os::fd::OwnedFd,
}

impl HeldFs {
    /// Hold the app folder at `root`, refusing a link there.
    pub(crate) fn open(root: &Path) -> io::Result<Self> {
        use std::os::unix::fs::OpenOptionsExt as _;
        let folder = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(root)?;
        Ok(Self {
            root: root.to_path_buf(),
            root_fd: folder.into(),
        })
    }

    /// Whether the folder held is the one at `path` now (same device and
    /// inode, the path not followed if it is a link): what makes the
    /// re-check, which reads by path, a check of the held folder.
    pub(crate) fn is_at(&self, path: &Path) -> bool {
        use std::os::unix::fs::MetadataExt as _;
        let Ok(fd) = self.root_fd.try_clone() else {
            return false;
        };
        match (fs::File::from(fd).metadata(), fs::symlink_metadata(path)) {
            (Ok(held), Ok(now)) => held.dev() == now.dev() && held.ino() == now.ino(),
            _ => false,
        }
    }

    /// Open the regular file at `path` (inside the app) through the held
    /// folders, never following a link. `None` when it is absent.
    pub(crate) fn open_file(&self, path: &Path) -> io::Result<Option<fs::File>> {
        use std::os::fd::AsFd as _;
        let Some((folder, name)) = self.locate(path)? else {
            return Err(io::Error::other(format!(
                "{} is not inside the app",
                path.display()
            )));
        };
        farhelm_supervisor::installation::dirs::open_file(folder.as_fd(), &name)
    }

    /// The held folder and name `path` is removed through, or `None` for a
    /// path outside the app (or the app's root itself).
    fn locate(&self, path: &Path) -> io::Result<Option<(std::os::fd::OwnedFd, String)>> {
        use farhelm_supervisor::installation::dirs;
        use std::os::fd::AsFd as _;
        let Ok(relative) = path.strip_prefix(&self.root) else {
            return Ok(None);
        };
        let parts: Vec<String> = relative
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect();
        let Some((name, folders)) = parts.split_last() else {
            return Ok(None);
        };
        let mut current = self.root_fd.try_clone()?;
        for folder in folders {
            current = dirs::open_folder(current.as_fd(), folder)?
                .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        }
        Ok(Some((current, name.clone())))
    }

    /// Flush the app's folders the early removals changed, and the folder
    /// beside the app where leftover trash went, so those removals are on
    /// disk before the retry command's own pieces go. A folder already gone
    /// needs nothing.
    pub(crate) fn flush_early(
        &self,
        applications: Option<std::os::fd::BorrowedFd<'_>>,
    ) -> io::Result<()> {
        use farhelm_supervisor::installation::{dirs, full_sync};
        use std::os::fd::AsFd as _;
        if let Some(contents) = dirs::open_folder(self.root_fd.as_fd(), "Contents")? {
            for name in ["Versions", "MacOS", "Resources"] {
                if let Some(folder) = dirs::open_folder(contents.as_fd(), name)? {
                    full_sync(folder.as_fd())?;
                }
            }
            full_sync(contents.as_fd())?;
        }
        if let Some(applications) = applications {
            full_sync(applications)?;
        }
        Ok(())
    }

    /// The held `Contents/` folder, if there is one.
    pub(crate) fn contents(&self) -> io::Result<Option<std::os::fd::OwnedFd>> {
        use std::os::fd::AsFd as _;
        farhelm_supervisor::installation::dirs::open_folder(self.root_fd.as_fd(), "Contents")
    }
}

impl RemovalFs for HeldFs {
    fn remove_file(&mut self, path: &Path) -> io::Result<()> {
        use std::os::fd::AsFd as _;
        match self.locate(path)? {
            Some((folder, name)) => {
                farhelm_supervisor::installation::dirs::remove_entry(folder.as_fd(), &name, false)
            }
            None => fs::remove_file(path),
        }
    }

    fn remove_dir(&mut self, path: &Path) -> io::Result<()> {
        use std::os::fd::AsFd as _;
        match self.locate(path)? {
            Some((folder, name)) => {
                farhelm_supervisor::installation::dirs::remove_entry(folder.as_fd(), &name, true)
            }
            None => fs::remove_dir(path),
        }
    }
}

/// Remove the app through its held folders: the Running records, the
/// leftover trash a killed install left beside the app, the early removals,
/// a flush of what they changed, then the rest, and the Terminal link last.
pub(crate) fn remove_app_held(
    plan: &AppPlan,
    held: &mut HeldFs,
    applications: Option<std::os::fd::BorrowedFd<'_>>,
    running_records: &[PathBuf],
    report: &mut String,
) -> Result<()> {
    if let Some(applications) = applications {
        farhelm_supervisor::installation::install::remove_leftovers(applications)
            .map_err(|error| anyhow::anyhow!("{}: {}", error.what, error.error))?;
    }
    remove_app_with_flush(plan, running_records, held, report, &mut |held: &HeldFs| {
        held.flush_early(applications)
            .context("flushing the app's folders before removing the rest")
    })
}

/// Apply the verified plan. `running_records` are the Running records
/// (`running-version`) of the state directories uninstall checked, removed
/// first on macOS, so that a retry reaches the Installed version, which the
/// plan keeps for last.
pub(crate) fn remove(
    plan: &OwnershipPlan,
    running_records: &[PathBuf],
    report: &mut String,
) -> Result<()> {
    remove_with(plan, running_records, &mut NativeFs, report)
}

/// Stop on the first required failure; what is left is what a retry plans.
fn remove_with(
    plan: &OwnershipPlan,
    running_records: &[PathBuf],
    filesystem: &mut impl RemovalFs,
    report: &mut String,
) -> Result<()> {
    match plan {
        OwnershipPlan::Flat(flat) => remove_flat(flat, filesystem, report),
        OwnershipPlan::App(app) => remove_app(app, running_records, filesystem, report),
    }
}

/// A standalone bin directory: the desktop program, then the CLI, then the
/// record, which is only bookkeeping once the CLI is gone.
fn remove_flat(
    plan: &FlatPlan,
    filesystem: &mut impl RemovalFs,
    report: &mut String,
) -> Result<()> {
    if let Some(desktop) = &plan.desktop {
        remove_one(filesystem, desktop, false, report)?;
    }
    remove_one(filesystem, &plan.cli, false, report)?;
    // Payload removal has completed. A leftover receipt is inert bookkeeping;
    // reporting it is honest, while asking to retry a removed CLI would not be.
    if let Err(error) = remove_one(filesystem, &plan.metadata, false, report) {
        writeln!(report, "retained installer receipt: {error:#}")?;
    }
    Ok(())
}

/// The Mac app, in the plan's order, then the Terminal link.
fn remove_app(
    plan: &AppPlan,
    running_records: &[PathBuf],
    filesystem: &mut impl RemovalFs,
    report: &mut String,
) -> Result<()> {
    remove_app_with_flush(plan, running_records, filesystem, report, &mut |_| Ok(()))
}

/// [`remove_app`], calling `after_early` between the early removals and the
/// retry command's own pieces.
fn remove_app_with_flush<F: RemovalFs>(
    plan: &AppPlan,
    running_records: &[PathBuf],
    filesystem: &mut F,
    report: &mut String,
    after_early: &mut dyn FnMut(&F) -> Result<()>,
) -> Result<()> {
    // First: with them gone, the forwarder sends a retry to the Installed
    // version from any context, including a session that outlived its
    // supervisor, and that version is kept for the late steps.
    for record in running_records {
        remove_one(filesystem, record, false, report)?;
    }
    for (index, removal) in plan.removals.iter().enumerate() {
        if index == plan.early {
            after_early(filesystem)?;
        }
        remove_one(filesystem, &removal.path, removal.directory, report)?;
    }
    match &plan.link {
        LinkPlan::Remove(link) => remove_one(filesystem, link, false, report)?,
        LinkPlan::Absent(link) => writeln!(report, "already absent {}", path_text(link))?,
        LinkPlan::Retain(link) => writeln!(
            report,
            "left {} in place: it is no longer a link to this installation's app",
            path_text(link)
        )?,
    }
    Ok(())
}

/// Missing entries are completed work, not a reason to abandon a partial retry.
fn remove_one(
    filesystem: &mut impl RemovalFs,
    path: &Path,
    directory: bool,
    report: &mut String,
) -> Result<()> {
    let result = if directory {
        filesystem.remove_dir(path)
    } else {
        filesystem.remove_file(path)
    };
    match result {
        Ok(()) => writeln!(report, "removed {}", path_text(path))?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            writeln!(report, "already absent {}", path_text(path))?;
        }
        Err(error) => return Err(error).with_context(|| format!("removing {}", path_text(path))),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::uninstall::{
        app::tests::AppFixture,
        ownership::{self, PlatformArtifacts, tests::Fixture},
    };

    /// Fail one named mutation while all other operations use the real
    /// filesystem. This works under root and does not rely on permission bits
    /// being enforced.
    struct FailAt {
        path: PathBuf,
        fired: bool,
    }

    impl FailAt {
        fn check(&mut self, path: &Path) -> io::Result<()> {
            if self.path == path {
                self.fired = true;
                Err(io::Error::other("injected removal failure"))
            } else {
                Ok(())
            }
        }
    }

    impl RemovalFs for FailAt {
        fn remove_file(&mut self, path: &Path) -> io::Result<()> {
            self.check(path)?;
            fs::remove_file(path)
        }

        fn remove_dir(&mut self, path: &Path) -> io::Result<()> {
            self.check(path)?;
            fs::remove_dir(path)
        }
    }

    /// A failure anywhere before the retry command's own files leaves
    /// `farhelm uninstall` working and a retry that finishes the job.
    ///
    /// Why: SPEC.md requires the CLI to stay available until the other
    /// required removal steps succeed, and in the app layout the CLI is the
    /// Terminal link, the forwarder, the Installed record and the running
    /// version. Spec: failing each earlier step in turn leaves all four in
    /// place, a fresh inspection still plans the rest, and the retry removes
    /// the app and the link while an unrelated file in the bin directory
    /// survives.
    #[test]
    fn every_failure_before_the_retry_command_is_retryable() {
        for boundary in [
            "Contents/Versions/1.2.3/farhelm",
            "Contents/Versions/1.2.3",
            "Contents/MacOS/farhelm-desktop",
            "Contents/Resources/Farhelm.icns",
            "Contents/Resources",
            "Contents/Info.plist",
        ] {
            let fixture = AppFixture::new();
            let running = fixture.home.join(".local/state/farhelm/running-version");
            fs::create_dir_all(running.parent().unwrap()).unwrap();
            fs::write(&running, "1.2.4\n").unwrap();
            let sentinel = fixture.link.with_file_name("unrelated");
            fs::write(&sentinel, b"retain me").unwrap();
            let path = fixture.app.join(boundary);
            let plan = OwnershipPlan::App(fixture.plan());
            let mut failing = FailAt { path, fired: false };
            let error = remove_with(
                &plan,
                std::slice::from_ref(&running),
                &mut failing,
                &mut String::new(),
            )
            .unwrap_err();
            assert!(failing.fired, "did not reach {boundary}: {error:#}");
            for kept in [
                "Contents/MacOS/farhelm",
                "Contents/Versions/installed",
                "Contents/Versions/1.2.4/farhelm",
            ] {
                assert!(
                    fixture.app.join(kept).is_file(),
                    "lost {kept} at {boundary}"
                );
            }
            assert!(
                fs::symlink_metadata(&fixture.link).is_ok(),
                "lost the link at {boundary}"
            );
            let retry = OwnershipPlan::App(fixture.plan());
            remove(&retry, std::slice::from_ref(&running), &mut String::new()).unwrap();
            assert!(!fixture.app.exists(), "retry at {boundary}");
            assert!(
                fs::symlink_metadata(&fixture.link).is_err(),
                "retry at {boundary}"
            );
            assert!(!running.exists(), "retry at {boundary}");
            assert_eq!(fs::read(&sentinel).unwrap(), b"retain me");
        }
    }

    /// A link that is no longer this installation's is left and reported,
    /// while the app itself is removed.
    #[test]
    fn a_retained_link_is_reported() {
        let fixture = AppFixture::new();
        fs::remove_file(&fixture.link).unwrap();
        fs::write(&fixture.link, b"my own farhelm").unwrap();
        let plan = OwnershipPlan::App(fixture.plan());
        let mut report = String::new();
        remove(&plan, &[], &mut report).unwrap();
        assert!(!fixture.app.exists());
        assert_eq!(fs::read(&fixture.link).unwrap(), b"my own farhelm");
        assert!(report.contains("left"), "{report}");
    }

    /// Receipt tidying after the CLI is gone must not promise an impossible
    /// command retry; the inert leftover is reported while removal succeeds.
    #[test]
    fn final_flat_receipt_failure_reports_retention_after_success() {
        let fixture = Fixture::new();
        fixture.flat(None);
        let ownership::OwnershipPlan::Flat(flat) =
            ownership::inspect(&fixture.inputs(PlatformArtifacts::Linux)).unwrap()
        else {
            panic!("a Linux plan is flat");
        };
        let metadata = flat.metadata.clone();
        let cli = flat.cli.clone();
        let plan = OwnershipPlan::Flat(flat);
        let mut failing = FailAt {
            path: metadata.clone(),
            fired: false,
        };
        let mut report = String::new();
        remove_with(&plan, &[], &mut failing, &mut report).unwrap();
        assert!(failing.fired);
        assert!(!cli.exists());
        assert!(metadata.is_file());
        assert!(report.contains("retained installer receipt"), "{report}");
        assert!(report.contains("injected removal failure"), "{report}");
    }
}
