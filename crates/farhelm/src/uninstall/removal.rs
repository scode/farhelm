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
    // First: with them gone, the forwarder sends a retry to the Installed
    // version from any context, including a session that outlived its
    // supervisor, and that version is kept for the late steps.
    for record in running_records {
        remove_one(filesystem, record, false, report)?;
    }
    for removal in &plan.removals {
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
