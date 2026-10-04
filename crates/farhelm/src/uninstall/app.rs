//! Verify the Mac app bundle, which is the whole installation, before
//! uninstall can name any removal path.
//!
//! The installer lays the app out side by side (SPEC_impl.md, "Side-by-side
//! versions inside Farhelm.app"): each kept version's `farhelm` in
//! `Contents/Versions/<version>/`, the Installed record
//! `Contents/Versions/installed`, the forwarder `Contents/MacOS/farhelm`, the
//! app's main program `Contents/MacOS/farhelm-desktop`, `Info.plist`, the icon,
//! and an ownership record, `Contents/.farhelm-installation`, that names the
//! Terminal link (`~/.local/bin/farhelm`) this installation owns. The record
//! carries no checksums, because every other file changes on every update;
//! ownership rests on the record, the fixed layout (anything the installer
//! never writes refuses), and every entry being a real file or directory owned
//! by the user.
//!
//! The running CLI is how the installation is found: uninstall is reached
//! through the link and the forwarder, so it runs as
//! `Contents/Versions/<version>/farhelm` inside the bundle it removes. This
//! module only reads metadata and returns a plan; it removes nothing.

use super::ownership::{
    FLAT_RECORD, InspectionInputs, MetadataClassifier, exists_dir, open_regular, path_text,
    read_record, reject_unexpected, repair_refusal, validate_dir,
};
use anyhow::{Context as _, Result, bail};
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
use std::path::{Path, PathBuf};

/// The ownership record's identifier for this layout.
const RECORD_MAGIC: &[u8] = b"farhelm-app-v2";

/// One path the remover deletes, in plan order.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Removal {
    pub(crate) path: PathBuf,
    pub(crate) directory: bool,
}

/// What happens to the Terminal link the record names.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LinkPlan {
    /// A symlink to this bundle's forwarder: removed last.
    Remove(PathBuf),
    /// Already gone (a retry, or the user removed it).
    Absent(PathBuf),
    /// Something else is there now; it is not this installation's to remove.
    Retain(PathBuf),
}

/// Everything uninstall may remove from the app, in order.
///
/// The order keeps `farhelm uninstall` runnable for as long as it can: other
/// versions, the app's main program, the icon and Info.plist go first, so an
/// ordinary failure there leaves the link, the forwarder, the Installed record
/// and the running and Installed versions in place for a retry (the Running
/// records go first, so the forwarder sends that retry to the Installed
/// version from any context). Only after them go those versions' programs,
/// the Installed record, the forwarder, the record and the directories, and
/// the link last. A failure in that final stretch
/// can leave no runnable CLI; what remains is reported, and is only empty
/// folders and records by then.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct AppPlan {
    pub(crate) root: PathBuf,
    pub(crate) removals: Vec<Removal>,
    pub(crate) link: LinkPlan,
}

/// Inspect the app bundle that holds the running CLI.
pub(super) fn inspect_app(
    inputs: &InspectionInputs,
    classifier: &impl MetadataClassifier,
) -> Result<AppPlan> {
    let uid = inputs.effective_uid;
    let executable = fs::canonicalize(&inputs.current_exe).with_context(|| {
        format!(
            "resolving running executable {}",
            path_text(&inputs.current_exe)
        )
    })?;
    let Some(program) = farhelm_supervisor::app_bundle::versioned_program(&executable) else {
        bail!(
            "{} is not a farhelm installed in Farhelm.app (it is not in \
             Farhelm.app/Contents/Versions/<version>/); run the installed one with \
             `~/.local/bin/farhelm uninstall`",
            path_text(&executable)
        );
    };
    let version_dir = executable
        .parent()
        .expect("a versioned program has a version folder");
    let versions = version_dir.parent().expect("and a Versions folder");
    let contents = versions.parent().expect("and a Contents folder");
    let root = contents.parent().expect("and an app folder");
    validate_dir(root, uid, classifier)?;
    validate_dir(contents, uid, classifier)?;
    validate_dir(versions, uid, classifier)?;

    let record = contents.join(FLAT_RECORD);
    let fields = match read_record(&record, 2, uid, true, classifier) {
        Ok(fields) => fields,
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|io| io.kind() == ErrorKind::NotFound) =>
        {
            return repair_refusal(&record, "it is missing");
        }
        Err(error) => return Err(error),
    };
    if fields[0] != RECORD_MAGIC {
        return repair_refusal(&record, "its magic field is not farhelm-app-v2");
    }
    let link = link_field(&fields[1], &record)?;

    reject_unexpected(root, &["Contents"])?;
    reject_unexpected(
        contents,
        &[FLAT_RECORD, "Info.plist", "MacOS", "Resources", "Versions"],
    )?;
    let macos = contents.join("MacOS");
    let resources = contents.join("Resources");

    // Every version folder holds exactly its farhelm; the running one is
    // removed late, the others early.
    let mut early = Vec::new();
    let mut versions_present = Vec::new();
    for entry in fs::read_dir(versions)
        .with_context(|| format!("enumerating bundle directory {}", path_text(versions)))?
    {
        let entry = entry
            .with_context(|| format!("enumerating bundle directory {}", path_text(versions)))?;
        let name = entry.file_name();
        if name == "installed" {
            continue;
        }
        if !is_version_name(&name) {
            bail!(
                "bundle directory {} contains unexpected entry {}",
                path_text(versions),
                path_text(&entry.path())
            );
        }
        versions_present.push(name.to_os_string());
    }
    versions_present.sort();
    // The running version and the Installed one are both what
    // `farhelm uninstall` may run as on a retry: the forwarder picks the
    // running one from inside a surviving session, the Installed one
    // anywhere else. Both stay until the late steps.
    let installed = read_installed(&versions.join("installed"), uid, classifier)?;
    let mut late = Vec::new();
    for name in &versions_present {
        let folder = versions.join(name);
        validate_dir(&folder, uid, classifier)?;
        reject_unexpected(&folder, &["farhelm"])?;
        let keep_late = name.as_bytes() == program.version.as_bytes()
            || installed
                .as_deref()
                .is_some_and(|installed| name.as_bytes() == installed.as_bytes());
        let target = if keep_late { &mut late } else { &mut early };
        push_file(target, &folder.join("farhelm"), uid, classifier)?;
        target.push(Removal {
            path: folder,
            directory: true,
        });
    }
    let has_macos = exists_dir(&macos, uid, classifier)?;
    if has_macos {
        reject_unexpected(&macos, &["farhelm", "farhelm-desktop"])?;
        push_file(&mut early, &macos.join("farhelm-desktop"), uid, classifier)?;
    }
    if exists_dir(&resources, uid, classifier)? {
        reject_unexpected(&resources, &["Farhelm.icns"])?;
        push_file(&mut early, &resources.join("Farhelm.icns"), uid, classifier)?;
        early.push(Removal {
            path: resources,
            directory: true,
        });
    }
    push_file(&mut early, &contents.join("Info.plist"), uid, classifier)?;

    push_file(&mut late, &versions.join("installed"), uid, classifier)?;
    late.push(Removal {
        path: versions.to_path_buf(),
        directory: true,
    });
    if has_macos {
        push_file(&mut late, &macos.join("farhelm"), uid, classifier)?;
        late.push(Removal {
            path: macos.clone(),
            directory: true,
        });
    }
    late.push(Removal {
        path: record,
        directory: false,
    });
    late.push(Removal {
        path: contents.to_path_buf(),
        directory: true,
    });
    late.push(Removal {
        path: root.to_path_buf(),
        directory: true,
    });

    let link = inspect_link(link, &macos.join("farhelm"))?;
    early.extend(late);
    Ok(AppPlan {
        root: root.to_path_buf(),
        removals: early,
        link,
    })
}

/// Parse the record's Terminal link field: an absolute path named
/// `farhelm`. The link is only ever removed while it is a symlink to this
/// app's forwarder (see [`inspect_link`]), so the path itself needs no
/// further proof; in particular its folder need not still be spelled the
/// way it was when the installer recorded it (`~/.local/bin` may have
/// become a symlink since, or the home folder been renamed).
fn link_field(bytes: &[u8], record: &Path) -> Result<PathBuf> {
    if bytes.is_empty() || bytes.contains(&0) {
        return repair_refusal(record, "its Terminal link is empty or contains NUL");
    }
    let link = PathBuf::from(OsString::from_vec(bytes.to_vec()));
    if !link.is_absolute() || link.file_name() != Some(OsStr::new("farhelm")) {
        return repair_refusal(
            record,
            "its Terminal link is not an absolute path to farhelm",
        );
    }
    Ok(link)
}

/// Read the Installed record, a one-line version, through the same
/// no-follow, nonblocking open every payload gets: a FIFO or a link in its
/// place refuses instead of hanging or redirecting the read. Absent means
/// an earlier attempt already removed it; anything but a version name is
/// ignored here (the plan only uses it to keep that folder for last).
fn read_installed(
    path: &Path,
    uid: u32,
    classifier: &impl MetadataClassifier,
) -> Result<Option<String>> {
    use std::io::Read as _;
    match fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("inspecting {}", path_text(path)));
        }
    }
    let (file, _) = open_regular(path, uid, classifier)?;
    let mut bytes = Vec::new();
    file.take(256)
        .read_to_end(&mut bytes)
        .with_context(|| format!("reading {}", path_text(path)))?;
    Ok(String::from_utf8(bytes)
        .ok()
        .map(|text| text.trim_end_matches('\n').to_string()))
}

/// Add `path` to the plan if it exists, after proving it is a real file the
/// user owns; a missing one is completed work from an earlier attempt.
fn push_file(
    plan: &mut Vec<Removal>,
    path: &Path,
    uid: u32,
    classifier: &impl MetadataClassifier,
) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => {
            open_regular(path, uid, classifier)?;
            plan.push(Removal {
                path: path.to_path_buf(),
                directory: false,
            });
            Ok(())
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("inspecting {}", path_text(path))),
    }
}

/// Whether a `Versions/` entry is named like a version the installer
/// writes: a release version `X.Y.Z` (no leading zeros), optionally with
/// `-rc.N` or `-dev.N`, or `0.0.0-unreleased`, the version of builds from
/// main that the acceptance tests install. With no checksums in this
/// layout, this grammar is part of the ownership boundary: a folder named
/// anything else, `backup` say, is not the installer's and refuses the
/// uninstall rather than being removed.
fn is_version_name(name: &OsStr) -> bool {
    fn number(part: &str) -> bool {
        !part.is_empty()
            && part.bytes().all(|byte| byte.is_ascii_digit())
            && (part == "0" || !part.starts_with('0'))
    }
    let Some(name) = name.to_str() else {
        return false;
    };
    if name == "0.0.0-unreleased" {
        return true;
    }
    let (release, prerelease) = match name.split_once('-') {
        Some((release, prerelease)) => (release, Some(prerelease)),
        None => (name, None),
    };
    let parts: Vec<&str> = release.split('.').collect();
    if parts.len() != 3 || !parts.iter().all(|part| number(part)) {
        return false;
    }
    match prerelease {
        None => true,
        Some(prerelease) => match prerelease.split_once('.') {
            Some(("rc" | "dev", n)) => number(n),
            _ => false,
        },
    }
}

/// Decide what to do with the recorded Terminal link: remove it only when it
/// is a symlink to this bundle's forwarder. The installer writes the target
/// with `$HOME` as the shell saw it, which need not be the canonical spelling
/// the bundle was found by, so the target's folder is resolved before the
/// comparison.
fn inspect_link(link: PathBuf, forwarder: &Path) -> Result<LinkPlan> {
    let metadata = match fs::symlink_metadata(&link) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(LinkPlan::Absent(link)),
        Err(error) => {
            return Err(error).with_context(|| format!("inspecting {}", path_text(&link)));
        }
    };
    if !metadata.file_type().is_symlink() {
        return Ok(LinkPlan::Retain(link));
    }
    let target = fs::read_link(&link).with_context(|| format!("reading {}", path_text(&link)))?;
    let points_here = target.is_absolute()
        && target.file_name() == forwarder.file_name()
        && target
            .parent()
            .and_then(|folder| fs::canonicalize(folder).ok())
            .is_some_and(|folder| Some(folder.as_path()) == forwarder.parent());
    Ok(if points_here {
        LinkPlan::Remove(link)
    } else {
        LinkPlan::Retain(link)
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::uninstall::ownership::{PlatformArtifacts, inspect};
    use std::os::unix::fs::{PermissionsExt as _, symlink};

    /// An installed app in a private home, laid out the way the installer
    /// lays it out, with the running CLI at version `1.2.4` and an older
    /// `1.2.3` kept beside it.
    pub(crate) struct AppFixture {
        _root: tempfile::TempDir,
        pub(crate) home: PathBuf,
        pub(crate) app: PathBuf,
        pub(crate) link: PathBuf,
        pub(crate) uid: u32,
    }

    impl AppFixture {
        pub(crate) fn new() -> Self {
            let root = tempfile::tempdir().expect("fixture root");
            let home = fs::canonicalize(root.path())
                .expect("physical root")
                .join("home");
            let app = home.join("Applications/Farhelm.app");
            let contents = app.join("Contents");
            for dir in ["MacOS", "Resources", "Versions/1.2.3", "Versions/1.2.4"] {
                fs::create_dir_all(contents.join(dir)).expect("layout");
            }
            for (path, bytes) in [
                ("MacOS/farhelm", b"#!/bin/sh\n".as_slice()),
                ("MacOS/farhelm-desktop", b"desktop"),
                ("Resources/Farhelm.icns", b"icon"),
                ("Info.plist", b"plist"),
                ("Versions/1.2.3/farhelm", b"old cli"),
                ("Versions/1.2.4/farhelm", b"cli"),
                ("Versions/installed", b"1.2.4\n"),
            ] {
                fs::write(contents.join(path), bytes).expect("payload");
            }
            let bin = home.join(".local/bin");
            fs::create_dir_all(&bin).expect("bin");
            let link = bin.join("farhelm");
            symlink(contents.join("MacOS/farhelm"), &link).expect("link");
            let mut record = b"farhelm-app-v2\0".to_vec();
            record.extend_from_slice(link.as_os_str().as_bytes());
            record.push(0);
            fs::write(contents.join(FLAT_RECORD), record).expect("record");
            fs::set_permissions(
                contents.join(FLAT_RECORD),
                fs::Permissions::from_mode(0o600),
            )
            .expect("private record");
            Self {
                _root: root,
                home,
                app,
                link,
                uid: unsafe { libc::geteuid() },
            }
        }

        /// The inputs of `~/.local/bin/farhelm uninstall` on this app: the
        /// forwarder has exec'd the running version.
        pub(crate) fn inputs(&self) -> InspectionInputs {
            InspectionInputs {
                current_exe: self.app.join("Contents/Versions/1.2.4/farhelm"),
                home: Some(self.home.clone()),
                effective_uid: self.uid,
                platform: PlatformArtifacts::Macos,
            }
        }

        pub(crate) fn plan(&self) -> AppPlan {
            match inspect(&self.inputs()).expect("valid app plan") {
                crate::uninstall::ownership::OwnershipPlan::App(plan) => plan,
                other => panic!("expected an app plan, got {other:?}"),
            }
        }
    }

    fn relative(plan: &AppPlan) -> Vec<String> {
        plan.removals
            .iter()
            .map(|removal| {
                removal
                    .path
                    .strip_prefix(&plan.root)
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|_| "<app>".to_string())
            })
            .map(|path| {
                if path.is_empty() {
                    "<app>".to_string()
                } else {
                    path
                }
            })
            .collect()
    }

    /// The whole installed app is planned, in the order that keeps the
    /// retry command alive longest, and the Terminal link goes last.
    ///
    /// Why: SPEC.md has the CLI stay available until the other removal
    /// steps succeed, and in this layout the CLI is the link, the
    /// forwarder, the Installed record and the running version's folder.
    #[test]
    fn the_installed_app_is_planned_with_the_retry_command_last() {
        let fixture = AppFixture::new();
        let plan = fixture.plan();
        assert_eq!(plan.root, fixture.app);
        assert_eq!(
            relative(&plan),
            [
                "Contents/Versions/1.2.3/farhelm",
                "Contents/Versions/1.2.3",
                "Contents/MacOS/farhelm-desktop",
                "Contents/Resources/Farhelm.icns",
                "Contents/Resources",
                "Contents/Info.plist",
                "Contents/Versions/1.2.4/farhelm",
                "Contents/Versions/1.2.4",
                "Contents/Versions/installed",
                "Contents/Versions",
                "Contents/MacOS/farhelm",
                "Contents/MacOS",
                "Contents/.farhelm-installation",
                "Contents",
                "<app>",
            ]
        );
        assert_eq!(plan.link, LinkPlan::Remove(fixture.link.clone()));
    }

    /// A partly removed app still plans the rest, so a retry finishes it.
    #[test]
    fn missing_payloads_from_an_earlier_attempt_are_completed_work() {
        let fixture = AppFixture::new();
        let contents = fixture.app.join("Contents");
        fs::remove_file(contents.join("MacOS/farhelm-desktop")).unwrap();
        fs::remove_file(contents.join("Info.plist")).unwrap();
        fs::remove_file(contents.join("Versions/1.2.3/farhelm")).unwrap();
        fs::remove_dir(contents.join("Versions/1.2.3")).unwrap();
        let plan = fixture.plan();
        assert!(!relative(&plan).iter().any(|path| path.contains("1.2.3")));
        assert!(!relative(&plan).contains(&"Contents/Info.plist".to_string()));
    }

    /// Uninstall run from anything but an installed versioned farhelm is
    /// refused with the command to use instead.
    #[test]
    fn a_cli_outside_the_app_layout_refuses() {
        let fixture = AppFixture::new();
        let mut inputs = fixture.inputs();
        inputs.current_exe = fixture.app.join("Contents/MacOS/farhelm-desktop");
        let error = inspect(&inputs).unwrap_err().to_string();
        assert!(error.contains("~/.local/bin/farhelm uninstall"), "{error}");
    }

    /// The record is the ownership evidence: missing, foreign-magic,
    /// writable or malformed, it refuses before anything is planned.
    #[test]
    fn the_record_must_be_this_layouts_and_private() {
        let fixture = AppFixture::new();
        let record = fixture.app.join("Contents").join(FLAT_RECORD);
        fs::write(&record, b"farhelm-app\0/x\0").unwrap();
        assert!(
            inspect(&fixture.inputs())
                .unwrap_err()
                .to_string()
                .contains("magic")
        );
        fs::write(&record, b"farhelm-app-v2\0relative\0").unwrap();
        assert!(inspect(&fixture.inputs()).is_err());
        let mut bytes = b"farhelm-app-v2\0".to_vec();
        bytes.extend_from_slice(fixture.link.as_os_str().as_bytes());
        bytes.push(0);
        fs::write(&record, &bytes).unwrap();
        fs::set_permissions(&record, fs::Permissions::from_mode(0o666)).unwrap();
        assert!(
            inspect(&fixture.inputs())
                .unwrap_err()
                .to_string()
                .contains("writable")
        );
        fs::remove_file(&record).unwrap();
        assert!(
            inspect(&fixture.inputs())
                .unwrap_err()
                .to_string()
                .contains("rerun the installer")
        );
    }

    /// Anything the installer never writes refuses, at every level, so a
    /// removal never takes something of the user's with it.
    #[test]
    fn unexpected_entries_refuse_at_each_level() {
        for extra in [
            "Contents/PkgInfo",
            "Contents/MacOS/helper",
            "Contents/Resources/other.icns",
            "Contents/Versions/notes",
            "Contents/Versions/.hidden",
            "Contents/Versions/1.2.3/extra",
            "Contents/Versions/backup/farhelm",
            "Contents/Versions/1.2.5",
            "Extra",
        ] {
            let fixture = AppFixture::new();
            fs::create_dir_all(fixture.app.join(extra).parent().unwrap()).unwrap();
            fs::write(fixture.app.join(extra), b"user data").unwrap();
            let error = inspect(&fixture.inputs()).unwrap_err().to_string();
            // A version-named entry that is not a folder refuses as a
            // malformed version folder rather than as an unknown name.
            assert!(
                error.contains("unexpected entry") || error.contains("real directory"),
                "{extra}: {error}"
            );
        }
    }

    /// A FIFO in place of the Installed record refuses instead of hanging
    /// the inspection on a blocking open.
    #[test]
    fn a_fifo_installed_record_refuses_without_blocking() {
        let fixture = AppFixture::new();
        let installed = fixture.app.join("Contents/Versions/installed");
        fs::remove_file(&installed).unwrap();
        let path = std::ffi::CString::new(installed.as_os_str().as_bytes()).unwrap();
        // The owned pathname is NUL-terminated and remains alive for this call.
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
        let error = inspect(&fixture.inputs()).unwrap_err().to_string();
        assert!(error.contains("regular file"), "{error}");
    }

    /// The version grammar is the installer's, exactly.
    #[test]
    fn version_folder_names_follow_the_installers_grammar() {
        for good in [
            "1.2.3",
            "0.0.1",
            "10.20.30",
            "1.2.3-rc.1",
            "1.2.3-dev.12",
            "0.0.0-unreleased",
        ] {
            assert!(is_version_name(OsStr::new(good)), "{good}");
        }
        for bad in [
            "backup",
            "1.2",
            "1.2.3.4",
            "01.2.3",
            "1.2.3-beta.1",
            "1.2.3-rc",
            "1.2.3-rc.01",
            ".1.2.3",
            "v1.2.3",
        ] {
            assert!(!is_version_name(OsStr::new(bad)), "{bad}");
        }
    }

    /// A version folder that is a symlink refuses: removal must not follow
    /// a link out of the bundle, and a link is not a folder the installer
    /// wrote.
    #[test]
    fn a_symlinked_version_folder_refuses() {
        let fixture = AppFixture::new();
        let folder = fixture.app.join("Contents/Versions/1.2.3");
        fs::remove_file(folder.join("farhelm")).unwrap();
        fs::remove_dir(&folder).unwrap();
        fs::create_dir_all(fixture.home.join("elsewhere")).unwrap();
        symlink(fixture.home.join("elsewhere"), &folder).unwrap();
        assert!(
            inspect(&fixture.inputs())
                .unwrap_err()
                .to_string()
                .contains("real directory")
        );
    }

    /// Files and folders owned by another account give no authority; the
    /// injected classifier exercises the production decision without chown.
    #[test]
    fn a_foreign_owner_refuses() {
        use std::os::unix::fs::MetadataExt as _;
        /// Marks exactly one inode foreign, so the refusal shows that this
        /// particular path is ownership-checked, not just the app folder.
        struct ForeignAt(u64);
        impl MetadataClassifier for ForeignAt {
            fn uid(&self, metadata: &fs::Metadata) -> u32 {
                if metadata.ino() == self.0 {
                    metadata.uid() + 1
                } else {
                    metadata.uid()
                }
            }
        }
        let fixture = AppFixture::new();
        let target = fixture.app.join("Contents/Versions/1.2.3/farhelm");
        let classifier = ForeignAt(fs::metadata(&target).unwrap().ino());
        let error = inspect_app(&fixture.inputs(), &classifier)
            .unwrap_err()
            .to_string();
        assert!(error.contains("not owned by the effective user"), "{error}");
        assert!(error.contains(&path_text(&target)), "{error}");
    }

    /// When the CLI runs as a version other than the Installed one (from
    /// inside a session that outlived an update), the Installed version is
    /// kept for the late steps too, since a retry from a fresh terminal
    /// runs it.
    #[test]
    fn the_installed_version_is_removed_late_too() {
        let fixture = AppFixture::new();
        let mut inputs = fixture.inputs();
        inputs.current_exe = fixture.app.join("Contents/Versions/1.2.3/farhelm");
        let crate::uninstall::ownership::OwnershipPlan::App(plan) = inspect(&inputs).unwrap()
        else {
            panic!("an app plan");
        };
        let order = relative(&plan);
        let position = |name: &str| order.iter().position(|path| path == name).unwrap();
        assert!(
            position("Contents/Versions/1.2.4/farhelm") > position("Contents/Info.plist"),
            "{order:?}"
        );
        assert!(
            position("Contents/Versions/1.2.3/farhelm") > position("Contents/Info.plist"),
            "{order:?}"
        );
    }

    /// A symlink or other non-regular file in place of a payload refuses:
    /// removal must not follow a link out of the bundle.
    #[test]
    fn a_symlinked_payload_refuses() {
        let fixture = AppFixture::new();
        let desktop = fixture.app.join("Contents/MacOS/farhelm-desktop");
        fs::remove_file(&desktop).unwrap();
        symlink(fixture.home.join("elsewhere"), &desktop).unwrap();
        assert!(
            inspect(&fixture.inputs())
                .unwrap_err()
                .to_string()
                .contains("regular file")
        );
    }

    /// A Terminal link that no longer points at this bundle's forwarder is
    /// the user's now, and is kept; a missing one is completed work.
    #[test]
    fn the_link_is_removed_only_while_it_points_at_the_forwarder() {
        let fixture = AppFixture::new();
        fs::remove_file(&fixture.link).unwrap();
        symlink("/usr/bin/true", &fixture.link).unwrap();
        assert_eq!(fixture.plan().link, LinkPlan::Retain(fixture.link.clone()));
        fs::remove_file(&fixture.link).unwrap();
        fs::write(&fixture.link, b"my own farhelm").unwrap();
        assert_eq!(fixture.plan().link, LinkPlan::Retain(fixture.link.clone()));
        fs::remove_file(&fixture.link).unwrap();
        assert_eq!(fixture.plan().link, LinkPlan::Absent(fixture.link.clone()));
    }

    /// The installer spells the link target with `$HOME` as its shell saw
    /// it; a non-canonical spelling of the same forwarder still counts.
    #[test]
    fn a_link_through_an_aliased_home_still_counts() {
        let fixture = AppFixture::new();
        let alias = fixture.home.parent().unwrap().join("home-alias");
        symlink(&fixture.home, &alias).unwrap();
        fs::remove_file(&fixture.link).unwrap();
        symlink(
            alias.join("Applications/Farhelm.app/Contents/MacOS/farhelm"),
            &fixture.link,
        )
        .unwrap();
        assert_eq!(fixture.plan().link, LinkPlan::Remove(fixture.link.clone()));
    }
}
