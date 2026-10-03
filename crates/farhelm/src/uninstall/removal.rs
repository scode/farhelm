//! Fixed-path removal after ownership preflight and confirmation.
//!
//! The CLI remains the retry command until every required removal completes.
//! A bundle's receipt moves outside its final directory-removal boundary so
//! a failed rmdir cannot leave an unidentifiable partial bundle on retry.

use super::{
    ownership::{BundleInspection, OwnershipPlan},
    path_text,
};
use anyhow::{Context as _, Result};
use std::{
    fmt::Write as _,
    fs,
    io::{self, Write as _},
    path::Path,
};

/// The narrow mutation seam makes partial failures reproducible under root too.
trait RemovalFs {
    fn remove_file(&mut self, path: &Path) -> io::Result<()>;
    fn remove_dir(&mut self, path: &Path) -> io::Result<()>;
    fn publish_receipt(&mut self, path: &Path, contents: &[u8]) -> Result<()>;
}

struct NativeFs;

impl RemovalFs for NativeFs {
    fn remove_file(&mut self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }

    fn remove_dir(&mut self, path: &Path) -> io::Result<()> {
        fs::remove_dir(path)
    }

    fn publish_receipt(&mut self, path: &Path, contents: &[u8]) -> Result<()> {
        let mut staged =
            tempfile::NamedTempFile::new_in(path.parent().context("receipt has no parent")?)
                .with_context(|| format!("staging retry receipt {}", path_text(path)))?;
        staged
            .write_all(contents)
            .context("writing bundle retry receipt")?;
        staged
            .persist_noclobber(path)
            .map_err(|error| error.error)
            .with_context(|| {
                format!(
                    "publishing {} without replacing an existing file",
                    path_text(path)
                )
            })?;
        Ok(())
    }
}

/// Apply the verified file list; no recursive traversal invents extra targets.
pub(crate) fn remove(plan: &OwnershipPlan, report: &mut String) -> Result<()> {
    remove_with(plan, &mut NativeFs, report)
}

/// Stop on the first required failure, keeping receipts for the next inspection.
fn remove_with(
    plan: &OwnershipPlan,
    filesystem: &mut impl RemovalFs,
    report: &mut String,
) -> Result<()> {
    if let BundleInspection::Recognized(bundle) = &plan.bundle {
        for file in &bundle.files {
            remove_one(filesystem, file, false, report)?;
        }
        let contents = bundle.root.join("Contents");
        for directory in &bundle.directories {
            if directory != &contents && directory != &bundle.root {
                remove_one(filesystem, directory, true, report)?;
            }
        }
        // All payloads have gone, but the internal receipt cannot survive
        // removing Contents. Preserve exactly those validated bytes outside
        // the bundle before crossing that boundary. No rollback is needed.
        if !bundle.pending_present {
            filesystem.publish_receipt(&bundle.pending_metadata, &bundle.receipt)?;
        }
        remove_one(filesystem, &bundle.metadata, false, report)?;
        remove_one(filesystem, &contents, true, report)?;
        remove_one(filesystem, &bundle.root, true, report)?;
        remove_one(filesystem, &bundle.pending_metadata, false, report)?;
    }
    if let Some(desktop) = &plan.flat.desktop {
        remove_one(filesystem, desktop, false, report)?;
    }
    remove_one(filesystem, &plan.flat.cli, false, report)?;
    // Payload removal has completed. A leftover receipt is inert bookkeeping;
    // reporting it is honest, while asking to retry a removed CLI would not be.
    if let Err(error) = remove_one(filesystem, &plan.flat.metadata, false, report) {
        writeln!(report, "retained installer receipt: {error:#}")?;
    }
    if let BundleInspection::RetainedWithoutReceipt(root) = &plan.bundle {
        writeln!(
            report,
            "left {} in place: it has no Farhelm installer receipt; move it to the Trash yourself if it is no longer wanted",
            path_text(root)
        )?;
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
    use crate::uninstall::ownership::{self, PlatformArtifacts, tests::Fixture};
    use std::path::PathBuf;

    /// Fail one named mutation while all other operations use the real filesystem.
    /// This works under root and does not rely on permission bits being enforced.
    struct FailAt {
        path: PathBuf,
        publish: bool,
        fired: bool,
    }

    impl FailAt {
        fn check(&mut self, path: &Path, publish: bool) -> io::Result<()> {
            if self.path == path && self.publish == publish {
                self.fired = true;
                Err(io::Error::other("injected removal failure"))
            } else {
                Ok(())
            }
        }
    }

    impl RemovalFs for FailAt {
        fn remove_file(&mut self, path: &Path) -> io::Result<()> {
            self.check(path, false)?;
            fs::remove_file(path)
        }

        fn remove_dir(&mut self, path: &Path) -> io::Result<()> {
            self.check(path, false)?;
            fs::remove_dir(path)
        }

        fn publish_receipt(&mut self, path: &Path, contents: &[u8]) -> Result<()> {
            self.check(path, true)?;
            NativeFs.publish_receipt(path, contents)
        }
    }

    /// Every required deletion boundary must preserve the flat CLI and enough
    /// ownership evidence for a fresh inspection and successful second attempt.
    #[test]
    fn every_partial_bundle_removal_can_be_reinspected_and_retried() {
        for boundary in [
            "Contents/Info.plist",
            "Contents/MacOS/farhelm",
            "Contents/MacOS/farhelm-desktop",
            "Contents/Resources/Farhelm.icns",
            "Contents/MacOS",
            "Contents/Resources",
            "publish",
            "Contents/.farhelm-installation",
            "Contents",
            "app",
            "pending",
            "flat-desktop",
            "flat-cli",
        ] {
            let fixture = Fixture::new();
            fixture.flat(Some(b"desktop"));
            let app = fixture.bundle(false);
            let sentinel = fixture.install.join("unrelated");
            fs::write(&sentinel, b"retain me").unwrap();
            let cli = fixture.install.join("farhelm");
            let pending = fixture
                .home
                .join("Applications/.Farhelm.app.uninstall-receipt");
            let path = match boundary {
                "publish" | "pending" => pending.clone(),
                "app" => app.clone(),
                "flat-desktop" => fixture.install.join("farhelm-desktop"),
                "flat-cli" => cli.clone(),
                relative => app.join(relative),
            };
            let inputs = fixture.inputs(PlatformArtifacts::Macos);
            let plan = ownership::inspect(&inputs).unwrap();
            assert!(cli.is_file());
            assert!(app.join("Contents/MacOS/farhelm").is_file());
            let mut failing = FailAt {
                path,
                publish: boundary == "publish",
                fired: false,
            };
            let error = remove_with(&plan, &mut failing, &mut String::new()).unwrap_err();
            assert!(failing.fired, "did not reach {boundary}: {error:#}");
            assert!(cli.is_file(), "lost retry CLI at {boundary}");
            let retry_plan = ownership::inspect(&inputs)
                .unwrap_or_else(|error| panic!("cannot reinspect {boundary}: {error:#}"));
            remove(&retry_plan, &mut String::new()).unwrap();
            assert!(!cli.exists(), "retry at {boundary}");
            assert!(!app.exists(), "retry at {boundary}");
            assert!(!pending.exists(), "retry at {boundary}");
            assert_eq!(fs::read(&sentinel).unwrap(), b"retain me");
            assert!(fixture.install.is_dir());
        }
    }

    /// A `Farhelm.app` without an installer receipt is left in place while the
    /// verified flat installation is removed, and both the plan and the
    /// report say the app was kept.
    ///
    /// Why it matters: bundles from installers that predate receipts, and
    /// self-built or signed apps, used to refuse the entire uninstall, and the
    /// advised reinstall could replace a recognized legacy app or refuse a
    /// foreign one. Neither supplied a removal path that preserved it. Deleting
    /// an app Farhelm cannot prove it made stays excluded.
    /// Specified: with the receipt removed and signed-app entries the
    /// installer never writes added, inspection succeeds with the app retained,
    /// removal deletes the flat CLI, every file of the app survives, and the
    /// report names the kept app.
    #[test]
    fn a_receiptless_app_is_kept_while_the_rest_is_removed() {
        let fixture = Fixture::new();
        fixture.flat(Some(b"desktop"));
        let bundle = fixture.bundle(false);
        let contents = bundle.join("Contents");
        std::fs::remove_file(contents.join(".farhelm-installation")).unwrap();
        std::fs::write(contents.join("PkgInfo"), b"APPL????").unwrap();
        std::fs::create_dir(contents.join("_CodeSignature")).unwrap();
        std::fs::write(contents.join("_CodeSignature/CodeResources"), b"signature").unwrap();

        let plan = ownership::inspect(&fixture.inputs(PlatformArtifacts::Macos))
            .expect("a receipt-less app must not block uninstall");
        let ownership::BundleInspection::RetainedWithoutReceipt(retained) = &plan.bundle else {
            panic!("the app must be retained, got {:?}", plan.bundle);
        };
        assert_eq!(retained, &bundle);

        let mut report = String::new();
        remove(&plan, &mut report).unwrap();
        assert!(!plan.flat.cli.exists(), "the verified CLI is removed");
        for kept in [
            "MacOS/farhelm",
            "MacOS/farhelm-desktop",
            "Info.plist",
            "PkgInfo",
            "_CodeSignature/CodeResources",
        ] {
            assert!(contents.join(kept).is_file(), "{kept} must survive");
        }
        assert!(
            report.contains(&format!("left {} in place", path_text(&bundle))),
            "{report}"
        );
    }

    /// Receipt tidying after the CLI is gone must not promise an impossible
    /// command retry; the inert leftover is reported while removal succeeds.
    #[test]
    fn final_flat_receipt_failure_reports_retention_after_success() {
        let fixture = Fixture::new();
        fixture.flat(None);
        let plan = ownership::inspect(&fixture.inputs(PlatformArtifacts::Linux)).unwrap();
        let mut failing = FailAt {
            path: plan.flat.metadata.clone(),
            publish: false,
            fired: false,
        };
        let mut report = String::new();
        remove_with(&plan, &mut failing, &mut report).unwrap();
        assert!(failing.fired);
        assert!(!plan.flat.cli.exists());
        assert!(plan.flat.metadata.is_file());
        assert!(report.contains("retained installer receipt"), "{report}");
        assert!(report.contains("injected removal failure"), "{report}");
    }
}
