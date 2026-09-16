//! Verify installer receipts before a future uninstall can name any path.
//!
//! This module only reads metadata and returns a concrete plan. It does not
//! remove files, probe processes, read process environment variables, or try
//! to coordinate an install racing with it. The installer receipts establish
//! the small fixed vocabulary below; they never contribute arbitrary paths.

use anyhow::{Context as _, Result, anyhow, bail};
use sha2::{Digest as _, Sha256};
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, Metadata};
use std::io::{ErrorKind, Read as _};
use std::os::unix::{
    ffi::{OsStrExt as _, OsStringExt as _},
    fs::{MetadataExt as _, OpenOptionsExt as _},
};
use std::path::{Path, PathBuf};

const RECORD_LIMIT: u64 = 64 * 1024;
const FLAT_RECORD: &str = ".farhelm-installation";
const CLI: &str = "farhelm";
const DESKTOP: &str = "farhelm-desktop";

/// Which installer artifact vocabulary the caller is inspecting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PlatformArtifacts {
    Linux,
    Macos,
}

/// Facts captured by the CLI before ownership inspection begins.
///
/// `home` is optional because a Finder-like macOS invocation may not provide
/// it. The verifier reports that omission in the plan instead of silently
/// claiming the bundle location was inspected.
pub(crate) struct InspectionInputs {
    pub(crate) current_exe: PathBuf,
    pub(crate) home: Option<PathBuf>,
    pub(crate) effective_uid: u32,
    pub(crate) platform: PlatformArtifacts,
}

/// Everything a later remover may act on after its own runtime checks pass.
///
/// Paths are derived from fixed names below a verified root. Metadata is kept
/// apart from payloads, and `cli` remains separately identified because it
/// must be the final flat artifact removed for an ordinary retry to work.
#[derive(Debug)]
pub(crate) struct OwnershipPlan {
    pub(crate) flat: FlatPlan,
    pub(crate) bundle: BundleInspection,
}

/// Verified ownership information for the selected standalone directory.
#[derive(Debug)]
pub(crate) struct FlatPlan {
    pub(crate) root: PathBuf,
    pub(crate) cli: PathBuf,
    pub(crate) desktop: Option<PathBuf>,
    pub(crate) retained_foreign_desktop: Option<PathBuf>,
    pub(crate) metadata: PathBuf,
}

/// macOS bundle inspection result, including the fact that it was impossible.
#[derive(Debug)]
pub(crate) enum BundleInspection {
    NotApplicable,
    UninspectedWithoutHome,
    Absent,
    Recognized(BundlePlan),
}

/// Fixed, verified paths inside an installer-created macOS application.
///
/// `files` never includes the receipt. `directories` is deepest-first and
/// ends with the app root, so a remover can delete exactly these entries
/// without recursively inventing authority over an unexpected child.
#[derive(Debug)]
pub(crate) struct BundlePlan {
    pub(crate) root: PathBuf,
    pub(crate) files: Vec<PathBuf>,
    pub(crate) directories: Vec<PathBuf>,
    pub(crate) metadata: PathBuf,
}

/// Inspect the installation that supplied the running CLI and return no
/// deletion side effects.
pub(crate) fn inspect(inputs: &InspectionInputs) -> Result<OwnershipPlan> {
    inspect_with_classifier(inputs, &NativeClassifier)
}

/// The narrow ownership seam keeps permission-constrained tests honest.
///
/// Production reads `uid` from the same metadata that supplies file type and
/// inode checks. Tests may substitute only that classification; they still
/// exercise the real no-follow and descriptor verification path.
trait MetadataClassifier {
    fn uid(&self, metadata: &Metadata) -> u32;
}
struct NativeClassifier;
impl MetadataClassifier for NativeClassifier {
    fn uid(&self, metadata: &Metadata) -> u32 {
        metadata.uid()
    }
}

/// Run the production inspection with a replaceable ownership classification.
///
/// The classifier changes no traversal or hashing rule; it only makes the
/// effective-UID boundary observable on hosts where a test cannot `chown`.
fn inspect_with_classifier(
    inputs: &InspectionInputs,
    classifier: &impl MetadataClassifier,
) -> Result<OwnershipPlan> {
    let executable = fs::canonicalize(&inputs.current_exe).with_context(|| {
        format!(
            "resolving running executable {}",
            path_text(&inputs.current_exe)
        )
    })?;
    if executable.file_name() != Some(OsStr::new(CLI)) {
        bail!(
            "{} resolves to {}, whose basename is not {CLI}",
            path_text(&inputs.current_exe),
            path_text(&executable)
        );
    }
    let candidate_root = executable
        .parent()
        .ok_or_else(|| anyhow!("{} has no installation directory", path_text(&executable)))?
        .to_path_buf();
    // An app copy has no flat receipt beside it. A custom standalone directory
    // may happen to have the same suffix as an app's MacOS directory, so its
    // adjacent receipt takes precedence over shape-based app discovery. An
    // existing but invalid receipt still refuses in the normal verifier below.
    let adjacent_record = candidate_root.join(FLAT_RECORD);
    let has_flat_record = match fs::symlink_metadata(&adjacent_record) {
        Ok(_) => true,
        Err(error) if error.kind() == ErrorKind::NotFound => false,
        Err(error) => {
            return Err(error)
                .with_context(|| format!("inspecting {}", path_text(&adjacent_record)));
        }
    };
    let invoked_bundle = if inputs.platform == PlatformArtifacts::Macos && !has_flat_record {
        bundle_invocation(&executable, inputs.effective_uid, classifier)?
    } else {
        None
    };
    let root = match &invoked_bundle {
        Some((_, origin)) => origin.clone(),
        None => candidate_root,
    };
    let metadata = root.join(FLAT_RECORD);
    let fields = match read_record(&metadata, 4, inputs.effective_uid, true, classifier) {
        Ok(fields) => fields,
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|io| io.kind() == ErrorKind::NotFound) =>
        {
            return repair_refusal(&metadata, "it is missing");
        }
        Err(error) => return Err(error),
    };
    if fields[0] != b"farhelm-standalone" {
        return repair_refusal(&metadata, "its magic field is not farhelm-standalone");
    }
    let recorded_root = field_path(&fields[1], &metadata, "flat directory")?;
    if recorded_root.as_os_str().as_bytes() != root.as_os_str().as_bytes() {
        return repair_refusal(
            &metadata,
            "its flat directory does not match the running CLI",
        );
    }
    let cli_hash = digest_field(&fields[2], &metadata, "CLI digest")?;
    let desktop_hash = if fields[3].is_empty() {
        None
    } else {
        Some(digest_field(&fields[3], &metadata, "desktop digest")?)
    };
    match (inputs.platform, desktop_hash.is_some()) {
        (PlatformArtifacts::Linux, true) | (PlatformArtifacts::Macos, false) => {
            return repair_refusal(
                &metadata,
                "its desktop digest does not match this platform's artifact set",
            );
        }
        _ => {}
    }
    let cli = root.join(CLI);
    verify_payload(&cli, &cli_hash, inputs.effective_uid, classifier)?;
    if invoked_bundle.is_none() && cli != executable {
        bail!(
            "{} is not the resolved CLI {}",
            path_text(&inputs.current_exe),
            path_text(&cli)
        );
    }
    let desktop_path = root.join(DESKTOP);
    let (desktop, retained_foreign_desktop) = match desktop_hash {
        Some(hash) => {
            match verify_optional_payload(&desktop_path, &hash, inputs.effective_uid, classifier)? {
                Some(()) => (Some(desktop_path), None),
                None => (None, None),
            }
        }
        None => match fs::symlink_metadata(&desktop_path) {
            Ok(_) => (None, Some(desktop_path)),
            Err(error) if error.kind() == ErrorKind::NotFound => (None, None),
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "inspecting retained foreign artifact {}",
                        path_text(&desktop_path)
                    )
                });
            }
        },
    };
    let flat = FlatPlan {
        root: root.clone(),
        cli: cli.clone(),
        desktop,
        retained_foreign_desktop,
        metadata,
    };
    let bundle = match (inputs.platform, invoked_bundle) {
        (PlatformArtifacts::Linux, _) => BundleInspection::NotApplicable,
        (PlatformArtifacts::Macos, Some((bundle_root, _))) => {
            let _plan = inspect_bundle_at(&bundle_root, &root, inputs.effective_uid, classifier)?;
            bail!(
                "{} is the verified app-bundle CLI; rerun uninstall from the originating flat CLI {}",
                path_text(&executable),
                path_text(&cli)
            );
        }
        (PlatformArtifacts::Macos, None) => match inputs.home.as_deref() {
            Some(home) => inspect_bundle_at(
                &home.join("Applications/Farhelm.app"),
                &root,
                inputs.effective_uid,
                classifier,
            )?,
            None => BundleInspection::UninspectedWithoutHome,
        },
    };
    Ok(OwnershipPlan { flat, bundle })
}

/// Turn a receipt failure into the repair direction old installs need.
fn repair_refusal<T>(path: &Path, reason: &str) -> Result<T> {
    bail!(
        "cannot verify installer ownership record {}: {reason}; rerun the installer, or upgrade once to a release that writes uninstall metadata",
        path_text(path)
    )
}

/// Read a bounded NUL record without following its final directory entry.
fn read_record(
    path: &Path,
    count: usize,
    uid: u32,
    private: bool,
    classifier: &impl MetadataClassifier,
) -> Result<Vec<Vec<u8>>> {
    let (mut file, metadata) = open_regular(path, uid, classifier)?;
    if private && metadata.mode() & 0o022 != 0 {
        bail!(
            "ownership record {} is group or world writable",
            path_text(path)
        );
    }
    if metadata.len() > RECORD_LIMIT {
        return repair_refusal(path, "it exceeds the 64 KiB record limit");
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize + 1);
    file.by_ref()
        .take(RECORD_LIMIT + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("reading ownership record {}", path_text(path)))?;
    if bytes.len() as u64 > RECORD_LIMIT {
        return repair_refusal(path, "it exceeds the 64 KiB record limit");
    }
    let fields: Vec<Vec<u8>> = bytes
        .split_inclusive(|byte| *byte == 0)
        .map(|field| field[..field.len() - 1].to_vec())
        .collect();
    if !bytes.ends_with(&[0]) || fields.len() != count {
        return repair_refusal(path, "it is not the expected NUL-terminated field layout");
    }
    Ok(fields)
}

/// Open one already-type-checked regular file without blocking on a raced FIFO.
fn open_regular(
    path: &Path,
    uid: u32,
    classifier: &impl MetadataClassifier,
) -> Result<(File, Metadata)> {
    let before =
        fs::symlink_metadata(path).with_context(|| format!("inspecting {}", path_text(path)))?;
    if !before.file_type().is_file() {
        bail!(
            "{} must be a regular file and not a symlink",
            path_text(path)
        );
    }
    if classifier.uid(&before) != uid {
        bail!("{} is not owned by the effective user", path_text(path));
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .with_context(|| format!("opening {} without following links", path_text(path)))?;
    let after = file
        .metadata()
        .with_context(|| format!("rechecking opened {}", path_text(path)))?;
    if !after.file_type().is_file()
        || before.dev() != after.dev()
        || before.ino() != after.ino()
        || classifier.uid(&after) != uid
    {
        bail!("{} changed while it was being inspected", path_text(path));
    }
    Ok((file, after))
}

/// Decode the deliberately narrow lowercase digest grammar the installer writes.
fn digest_field(bytes: &[u8], record: &Path, name: &str) -> Result<[u8; 32]> {
    if bytes.len() != 64
        || !bytes
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return repair_refusal(record, &format!("its {name} is not lowercase SHA-256 hex"));
    }
    let mut digest = [0; 32];
    for (index, pair) in bytes.as_chunks::<2>().0.iter().enumerate() {
        digest[index] = u8::from_str_radix(std::str::from_utf8(pair).expect("hex is ASCII"), 16)
            .expect("validated hex");
    }
    Ok(digest)
}
/// Require the physical path spelling the installer recorded, preserving Unix bytes.
///
/// Executable aliases are resolved at the invocation boundary. Receipts have a
/// stricter contract: accepting an alias here would make their meaning depend
/// on a link the installer never recorded. Path equality alone normalizes some
/// components, so compare the canonical spelling as bytes.
fn field_path(bytes: &[u8], record: &Path, name: &str) -> Result<PathBuf> {
    if bytes.is_empty() || bytes.contains(&0) {
        return repair_refusal(record, &format!("its {name} is empty or contains NUL"));
    }
    let path = PathBuf::from(OsString::from_vec(bytes.to_vec()));
    if !path.is_absolute() {
        return repair_refusal(record, &format!("its {name} is not absolute"));
    }
    let canonical = fs::canonicalize(&path).with_context(|| {
        format!(
            "resolving {name} {} from ownership record {}; rerun the installer to repair metadata",
            path_text(&path),
            path_text(record)
        )
    })?;
    if canonical.as_os_str().as_bytes() != bytes {
        return repair_refusal(
            record,
            &format!("its {name} is not the physical canonical path"),
        );
    }
    Ok(path)
}
/// Accept an absent retry artifact, but validate every artifact still present.
fn verify_optional_payload(
    path: &Path,
    digest: &[u8; 32],
    uid: u32,
    classifier: &impl MetadataClassifier,
) -> Result<Option<()>> {
    match fs::symlink_metadata(path) {
        Ok(_) => verify_payload(path, digest, uid, classifier).map(Some),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("inspecting {}", path_text(path))),
    }
}

/// Check one claimed file against its receipt digest while streaming its bytes.
fn verify_payload(
    path: &Path,
    expected: &[u8; 32],
    uid: u32,
    classifier: &impl MetadataClassifier,
) -> Result<()> {
    let (mut file, _) = open_regular(path, uid, classifier)?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("reading claimed artifact {}", path_text(path)))?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    let actual: [u8; 32] = hash.finalize().into();
    if &actual != expected {
        bail!(
            "{} does not match its recorded SHA-256 digest",
            path_text(path)
        );
    }
    Ok(())
}

/// Find an app invocation by exact fixed components, then read only its
/// receipt-origin field. Full payload validation waits until that origin's
/// flat receipt has independently verified the installation.
fn bundle_invocation(
    executable: &Path,
    uid: u32,
    classifier: &impl MetadataClassifier,
) -> Result<Option<(PathBuf, PathBuf)>> {
    let Some(macos) = executable.parent() else {
        return Ok(None);
    };
    let Some(contents) = macos.parent() else {
        return Ok(None);
    };
    let Some(root) = contents.parent() else {
        return Ok(None);
    };
    if macos.file_name() != Some(OsStr::new("MacOS"))
        || contents.file_name() != Some(OsStr::new("Contents"))
        || root.file_name() != Some(OsStr::new("Farhelm.app"))
    {
        return Ok(None);
    }
    validate_dir(root, uid, classifier)?;
    validate_dir(contents, uid, classifier)?;
    let metadata = contents.join(FLAT_RECORD);
    let fields = read_record(&metadata, 6, uid, true, classifier)?;
    if fields[0] != b"farhelm-app" {
        return repair_refusal(&metadata, "its magic field is not farhelm-app");
    }
    let origin = field_path(&fields[1], &metadata, "origin directory")?;
    Ok(Some((root.to_path_buf(), origin)))
}

/// Validate one known app location against an already-verified flat root.
///
/// The receipt's four hashes apply to app-local copies. They intentionally do
/// not compare against current flat bytes because a later opt-out update may
/// leave an older, still installer-owned app behind.
fn inspect_bundle_at(
    root: &Path,
    flat_root: &Path,
    uid: u32,
    classifier: &impl MetadataClassifier,
) -> Result<BundleInspection> {
    match fs::symlink_metadata(root) {
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(BundleInspection::Absent),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("inspecting app bundle {}", path_text(root)));
        }
        Ok(_) => validate_dir(root, uid, classifier)?,
    }
    reject_unexpected(root, &["Contents"])?;
    let contents = root.join("Contents");
    validate_dir(&contents, uid, classifier)?;
    reject_unexpected(
        &contents,
        &[".farhelm-installation", "Info.plist", "MacOS", "Resources"],
    )?;
    let metadata = contents.join(FLAT_RECORD);
    let fields = read_record(&metadata, 6, uid, true, classifier)?;
    if fields[0] != b"farhelm-app" {
        return repair_refusal(&metadata, "its magic field is not farhelm-app");
    }
    let origin = field_path(&fields[1], &metadata, "origin directory")?;
    if origin.as_os_str().as_bytes() != flat_root.as_os_str().as_bytes() {
        return repair_refusal(
            &metadata,
            "its origin does not match the selected flat installation",
        );
    }
    let hashes = [
        digest_field(&fields[2], &metadata, "CLI digest")?,
        digest_field(&fields[3], &metadata, "desktop digest")?,
        digest_field(&fields[4], &metadata, "Info.plist digest")?,
        digest_field(&fields[5], &metadata, "icon digest")?,
    ];
    let macos = contents.join("MacOS");
    let resources = contents.join("Resources");
    let mut files = Vec::new();
    let mut directories = Vec::new();
    verify_bundle_file(
        &contents.join("Info.plist"),
        &hashes[2],
        uid,
        classifier,
        &mut files,
    )?;
    if exists_dir(&macos, uid, classifier)? {
        reject_unexpected(&macos, &[CLI, DESKTOP])?;
        verify_bundle_file(&macos.join(CLI), &hashes[0], uid, classifier, &mut files)?;
        verify_bundle_file(
            &macos.join(DESKTOP),
            &hashes[1],
            uid,
            classifier,
            &mut files,
        )?;
        directories.push(macos.clone());
    }
    if exists_dir(&resources, uid, classifier)? {
        reject_unexpected(&resources, &["Farhelm.icns"])?;
        verify_bundle_file(
            &resources.join("Farhelm.icns"),
            &hashes[3],
            uid,
            classifier,
            &mut files,
        )?;
        directories.push(resources.clone());
    }
    directories.push(contents.clone());
    directories.push(root.to_path_buf());
    Ok(BundleInspection::Recognized(BundlePlan {
        root: root.to_path_buf(),
        files,
        directories,
        metadata,
    }))
}
/// Require a bundle component to be a user-owned directory, never a link.
fn validate_dir(path: &Path, uid: u32, classifier: &impl MetadataClassifier) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("inspecting bundle directory {}", path_text(path)))?;
    if !metadata.file_type().is_dir() {
        bail!(
            "bundle path {} must be a real directory and not a link",
            path_text(path)
        );
    }
    if classifier.uid(&metadata) != uid {
        bail!(
            "bundle directory {} is not owned by the effective user",
            path_text(path)
        );
    }
    Ok(())
}
/// Distinguish an absent retry directory from an unsafe existing one.
fn exists_dir(path: &Path, uid: u32, classifier: &impl MetadataClassifier) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => {
            validate_dir(path, uid, classifier)?;
            Ok(true)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => {
            Err(error).with_context(|| format!("inspecting bundle directory {}", path_text(path)))
        }
    }
}
/// Prove a fixed bundle directory contains no recursive-removal surprises.
fn reject_unexpected(path: &Path, allowed: &[&str]) -> Result<()> {
    for entry in fs::read_dir(path)
        .with_context(|| format!("enumerating bundle directory {}", path_text(path)))?
    {
        let entry =
            entry.with_context(|| format!("enumerating bundle directory {}", path_text(path)))?;
        if !allowed
            .iter()
            .any(|name| entry.file_name().as_bytes() == name.as_bytes())
        {
            bail!(
                "bundle directory {} contains unexpected entry {}",
                path_text(path),
                path_text(&entry.path())
            );
        }
    }
    Ok(())
}
/// Add a surviving fixed bundle payload only after its own receipt hash holds.
fn verify_bundle_file(
    path: &Path,
    digest: &[u8; 32],
    uid: u32,
    classifier: &impl MetadataClassifier,
    files: &mut Vec<PathBuf>,
) -> Result<()> {
    if verify_optional_payload(path, digest, uid, classifier)?.is_some() {
        files.push(path.to_path_buf());
    }
    Ok(())
}

/// Quote raw Unix path bytes so a diagnostic cannot forge terminal output.
fn path_text(path: &Path) -> String {
    let mut result = String::from("\"");
    for byte in path.as_os_str().as_bytes() {
        match byte {
            b'\\' => result.push_str("\\\\"),
            b'\"' => result.push_str("\\\""),
            0x20..=0x7e => result.push(*byte as char),
            _ => result.push_str(&format!("\\x{byte:02x}")),
        }
    }
    result.push('\"');
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt as _, symlink};
    /// Own every path inspected by a test; receipts use the same physical
    /// spelling as the installer, including on macOS's aliased temporary root.
    struct Fixture {
        root: tempfile::TempDir,
        install: PathBuf,
        home: PathBuf,
        uid: u32,
    }
    impl Fixture {
        /// Create a private installation and home without changing the test's environment.
        fn new() -> Self {
            let root = tempfile::tempdir().expect("fixture root");
            let physical = fs::canonicalize(root.path()).expect("physical fixture root");
            let install = physical.join("installed");
            fs::create_dir(&install).expect("install");
            let home = physical.join("home");
            fs::create_dir(&home).expect("home");
            Self {
                root,
                install,
                home,
                uid: unsafe { libc::geteuid() },
            }
        }
        fn write(path: &Path, bytes: &[u8]) {
            fs::write(path, bytes).expect("fixture bytes");
        }
        fn digest(bytes: &[u8]) -> String {
            format!("{:x}", Sha256::digest(bytes))
        }
        /// Publish synthetic installed bytes and matching metadata with the producer's mode.
        fn flat(&self, desktop: Option<&[u8]>) {
            Self::write(&self.install.join(CLI), b"cli");
            let cli_hash = Self::digest(b"cli");
            let desktop_hash = desktop
                .map(|b| {
                    Self::write(&self.install.join(DESKTOP), b);
                    Self::digest(b)
                })
                .unwrap_or_default();
            let parts = [
                b"farhelm-standalone".as_slice(),
                self.install.as_os_str().as_bytes(),
                cli_hash.as_bytes(),
                desktop_hash.as_bytes(),
            ];
            let mut record = parts.join(&0);
            record.push(0);
            Self::write(&self.install.join(FLAT_RECORD), &record);
            fs::set_permissions(
                self.install.join(FLAT_RECORD),
                fs::Permissions::from_mode(0o600),
            )
            .expect("private record");
        }
        fn inputs(&self, platform: PlatformArtifacts) -> InspectionInputs {
            InspectionInputs {
                current_exe: self.install.join(CLI),
                home: Some(self.home.clone()),
                effective_uid: self.uid,
                platform,
            }
        }
        /// Change exactly one receipt field so negative cases retain all other valid evidence.
        fn replace_field(path: &Path, index: usize, value: &[u8]) {
            let bytes = fs::read(path).expect("existing receipt");
            assert_eq!(bytes.last(), Some(&0));
            let mut fields: Vec<_> = bytes[..bytes.len() - 1]
                .split(|byte| *byte == 0)
                .map(<[u8]>::to_vec)
                .collect();
            fields[index] = value.to_vec();
            let mut changed = fields.join(&0);
            changed.push(0);
            Self::write(path, &changed);
        }
        /// Build the fixed app tree, optionally retaining an older independent payload generation.
        fn bundle(&self, older: bool) -> PathBuf {
            let contents = self.home.join("Applications/Farhelm.app/Contents");
            fs::create_dir_all(contents.join("MacOS")).expect("macos");
            fs::create_dir_all(contents.join("Resources")).expect("resources");
            let cli = if older {
                b"old cli".as_slice()
            } else {
                b"cli".as_slice()
            };
            let desktop = if older {
                b"old desktop".as_slice()
            } else {
                b"desktop".as_slice()
            };
            Self::write(&contents.join("MacOS/farhelm"), cli);
            Self::write(&contents.join("MacOS/farhelm-desktop"), desktop);
            Self::write(&contents.join("Info.plist"), b"plist");
            Self::write(&contents.join("Resources/Farhelm.icns"), b"icon");
            let cli_hash = Self::digest(cli);
            let desktop_hash = Self::digest(desktop);
            let plist_hash = Self::digest(b"plist");
            let icon_hash = Self::digest(b"icon");
            let parts = [
                b"farhelm-app".as_slice(),
                self.install.as_os_str().as_bytes(),
                cli_hash.as_bytes(),
                desktop_hash.as_bytes(),
                plist_hash.as_bytes(),
                icon_hash.as_bytes(),
            ];
            let mut record = parts.join(&0);
            record.push(0);
            Self::write(&contents.join(FLAT_RECORD), &record);
            fs::set_permissions(
                contents.join(FLAT_RECORD),
                fs::Permissions::from_mode(0o600),
            )
            .expect("private app record");
            contents.parent().unwrap().to_path_buf()
        }
    }
    /// A complete Linux receipt proves the verifier builds authority only from fixed names.
    #[test]
    fn successful_linux_plan_and_retained_desktop_are_distinct() {
        let fixture = Fixture::new();
        fixture.flat(None);
        Fixture::write(&fixture.install.join(DESKTOP), b"foreign");
        let plan = inspect(&fixture.inputs(PlatformArtifacts::Linux)).expect("valid plan");
        assert_eq!(plan.flat.cli, fixture.install.join(CLI));
        assert_eq!(plan.flat.desktop, None);
        assert_eq!(
            plan.flat.retained_foreign_desktop,
            Some(fixture.install.join(DESKTOP))
        );
        assert!(matches!(plan.bundle, BundleInspection::NotApplicable));
    }
    /// A valid app may carry older independent bytes after a bundle-opt-out update.
    #[test]
    fn successful_macos_plan_accepts_an_older_independent_bundle() {
        let fixture = Fixture::new();
        fixture.flat(Some(b"desktop"));
        let root = fixture.bundle(true);
        let plan = inspect(&fixture.inputs(PlatformArtifacts::Macos)).expect("valid mac plan");
        let BundleInspection::Recognized(bundle) = plan.bundle else {
            panic!("recognized bundle")
        };
        assert_eq!(bundle.root, root);
        assert_eq!(bundle.directories.last(), Some(&root));
        assert_eq!(bundle.files.len(), 4);
    }
    /// Aliases are allowed only because their resolved target proves the selected installation.
    #[test]
    fn executable_alias_selects_the_real_flat_installation() {
        let fixture = Fixture::new();
        fixture.flat(None);
        let alias = fixture.root.path().join("alias");
        symlink(fixture.install.join(CLI), &alias).expect("alias");
        let mut inputs = fixture.inputs(PlatformArtifacts::Linux);
        inputs.current_exe = alias;
        assert!(inspect(&inputs).is_ok());
    }

    /// A custom install's directory suffix is not app ownership evidence. Its
    /// adjacent flat receipt must win even when the path resembles an app copy.
    #[test]
    fn bundle_shaped_custom_flat_directory_uses_its_adjacent_receipt() {
        let mut fixture = Fixture::new();
        fixture.install = fixture.install.join("Farhelm.app/Contents/MacOS");
        fs::create_dir_all(&fixture.install).unwrap();
        fixture.flat(Some(b"desktop"));
        let plan = inspect(&fixture.inputs(PlatformArtifacts::Macos)).expect("custom flat install");
        assert_eq!(plan.flat.root, fixture.install);
        assert!(matches!(plan.bundle, BundleInspection::Absent));
    }
    /// The app copy is a running retry command, so it must direct the user to verified flat CLI.
    #[test]
    fn bundle_cli_invocation_refuses_after_validating_origin() {
        let fixture = Fixture::new();
        fixture.flat(Some(b"desktop"));
        let bundle = fixture.bundle(false);
        let mut inputs = fixture.inputs(PlatformArtifacts::Macos);
        inputs.current_exe = bundle.join("Contents/MacOS/farhelm");
        let error = inspect(&inputs).expect_err("bundle invocation").to_string();
        assert!(error.contains("originating flat CLI"), "{error}");
    }
    /// Receipts are syntax boundaries, not best-effort hints.
    #[test]
    fn malformed_truncated_and_oversized_records_refuse() {
        let fixture = Fixture::new();
        fixture.flat(None);
        let record = fixture.install.join(FLAT_RECORD);
        Fixture::write(&record, b"farhelm-standalone\0");
        assert!(inspect(&fixture.inputs(PlatformArtifacts::Linux)).is_err());
        Fixture::write(&record, &vec![b'x'; RECORD_LIMIT as usize + 1]);
        assert!(inspect(&fixture.inputs(PlatformArtifacts::Linux)).is_err());
    }
    /// A missing receipt is an upgrade-repair problem, not a permission to infer ownership.
    #[test]
    fn missing_flat_record_names_the_installer_repair_path() {
        let fixture = Fixture::new();
        Fixture::write(&fixture.install.join(CLI), b"cli");
        let error = inspect(&fixture.inputs(PlatformArtifacts::Linux))
            .expect_err("missing receipt")
            .to_string();
        assert!(error.contains("rerun the installer"), "{error}");
    }
    /// A syntactically valid digest is still not authority over changed bytes.
    #[test]
    fn mismatched_flat_digest_refuses() {
        let fixture = Fixture::new();
        fixture.flat(None);
        Fixture::write(&fixture.install.join(CLI), b"changed");
        assert!(
            inspect(&fixture.inputs(PlatformArtifacts::Linux))
                .unwrap_err()
                .to_string()
                .contains("digest")
        );
    }
    /// Group-writable receipts could be rewritten by another account and therefore grant no authority.
    #[test]
    fn writable_record_refuses_before_hashing() {
        let fixture = Fixture::new();
        fixture.flat(None);
        fs::set_permissions(
            fixture.install.join(FLAT_RECORD),
            fs::Permissions::from_mode(0o666),
        )
        .expect("mode");
        assert!(
            inspect(&fixture.inputs(PlatformArtifacts::Linux))
                .unwrap_err()
                .to_string()
                .contains("writable")
        );
    }
    /// Neither a receipt nor a payload symlink may redirect a no-follow inspection.
    #[test]
    fn symlink_and_nonregular_paths_refuse() {
        let fixture = Fixture::new();
        fixture.flat(None);
        let outside = fixture.root.path().join("outside");
        Fixture::write(&outside, b"outside");
        fs::remove_file(fixture.install.join(FLAT_RECORD)).expect("remove record");
        symlink(&outside, fixture.install.join(FLAT_RECORD)).expect("record link");
        assert!(
            inspect(&fixture.inputs(PlatformArtifacts::Linux))
                .unwrap_err()
                .to_string()
                .contains("regular file")
        );
        fs::remove_file(fixture.install.join(FLAT_RECORD)).expect("link");
        fixture.flat(Some(b"desktop"));
        fs::remove_file(fixture.install.join(DESKTOP)).expect("desktop");
        symlink(&outside, fixture.install.join(DESKTOP)).expect("payload link");
        assert!(
            inspect(&fixture.inputs(PlatformArtifacts::Macos))
                .unwrap_err()
                .to_string()
                .contains("regular file")
        );
    }
    /// The injected classifier exercises production's ownership decision without chown.
    #[test]
    fn foreign_uid_classifier_refuses_a_real_record() {
        struct Foreign;
        impl MetadataClassifier for Foreign {
            fn uid(&self, metadata: &Metadata) -> u32 {
                metadata.uid() + 1
            }
        }
        let fixture = Fixture::new();
        fixture.flat(None);
        assert!(
            inspect_with_classifier(&fixture.inputs(PlatformArtifacts::Linux), &Foreign)
                .unwrap_err()
                .to_string()
                .contains("effective user")
        );
    }
    /// Partial removal remains retryable only while the receipt and required parents survive.
    #[test]
    fn partial_bundle_leftovers_keep_a_bounded_plan() {
        let fixture = Fixture::new();
        fixture.flat(Some(b"desktop"));
        let bundle = fixture.bundle(false);
        fs::remove_file(bundle.join("Contents/MacOS/farhelm-desktop")).expect("desktop");
        fs::remove_dir_all(bundle.join("Contents/Resources")).expect("resources");
        let plan = inspect(&fixture.inputs(PlatformArtifacts::Macos)).expect("partial plan");
        let BundleInspection::Recognized(bundle) = plan.bundle else {
            panic!("bundle")
        };
        assert_eq!(bundle.files.len(), 2);
        assert!(
            !bundle
                .directories
                .iter()
                .any(|path| path.ends_with("Resources"))
        );
    }
    /// A removed flat desktop is retryable when its receipt still identifies the CLI.
    #[test]
    fn missing_claimed_flat_desktop_is_retryable() {
        let fixture = Fixture::new();
        fixture.flat(Some(b"desktop"));
        fs::remove_file(fixture.install.join(DESKTOP)).expect("desktop");
        let plan = inspect(&fixture.inputs(PlatformArtifacts::Macos)).expect("partial flat plan");
        assert_eq!(plan.flat.desktop, None);
    }
    /// macOS must say it skipped bundle inspection when HOME was unavailable.
    #[test]
    fn absent_home_is_not_reported_as_an_absent_bundle() {
        let fixture = Fixture::new();
        fixture.flat(Some(b"desktop"));
        let mut inputs = fixture.inputs(PlatformArtifacts::Macos);
        inputs.home = None;
        assert!(matches!(
            inspect(&inputs).expect("plan").bundle,
            BundleInspection::UninspectedWithoutHome
        ));
    }
    /// A surviving app without its own receipt is uncertainty, never recursive deletion authority.
    #[test]
    fn missing_or_extra_bundle_metadata_refuses() {
        let fixture = Fixture::new();
        fixture.flat(Some(b"desktop"));
        let bundle = fixture.bundle(false);
        fs::remove_file(bundle.join("Contents/.farhelm-installation")).expect("record");
        assert!(inspect(&fixture.inputs(PlatformArtifacts::Macos)).is_err());
        fixture.bundle(false);
        Fixture::write(&bundle.join("Contents/extra"), b"x");
        assert!(
            inspect(&fixture.inputs(PlatformArtifacts::Macos))
                .unwrap_err()
                .to_string()
                .contains("unexpected")
        );
    }
    /// The bundle receipt must point back to this flat install, not merely any valid one.
    #[test]
    fn mismatched_bundle_origin_refuses() {
        let fixture = Fixture::new();
        fixture.flat(Some(b"desktop"));
        let bundle = fixture.bundle(false);
        let contents = bundle.join("Contents");
        let elsewhere = fixture.root.path().join("elsewhere");
        fs::create_dir(&elsewhere).expect("elsewhere");
        let cli_hash = Fixture::digest(b"cli");
        let desktop_hash = Fixture::digest(b"desktop");
        let plist_hash = Fixture::digest(b"plist");
        let icon_hash = Fixture::digest(b"icon");
        let parts = [
            b"farhelm-app".as_slice(),
            elsewhere.as_os_str().as_bytes(),
            cli_hash.as_bytes(),
            desktop_hash.as_bytes(),
            plist_hash.as_bytes(),
            icon_hash.as_bytes(),
        ];
        let mut record = parts.join(&0);
        record.push(0);
        Fixture::write(&contents.join(FLAT_RECORD), &record);
        assert!(
            inspect(&fixture.inputs(PlatformArtifacts::Macos))
                .unwrap_err()
                .to_string()
                .contains("origin")
        );
    }

    /// Receipt origins cannot acquire new meanings through aliases or normalized
    /// components, even when those spellings currently resolve to the right root.
    #[test]
    fn receipt_paths_require_exact_physical_spelling() {
        let fixture = Fixture::new();
        fixture.flat(None);
        let alias = fixture.install.parent().unwrap().join("install-alias");
        symlink(&fixture.install, &alias).expect("alias");
        let mut dotted = fixture.install.as_os_str().as_bytes().to_vec();
        dotted.extend_from_slice(b"/.");
        let record = fixture.install.join(FLAT_RECORD);
        for path in [alias.as_os_str().as_bytes(), dotted.as_slice(), b"relative"] {
            fixture.flat(None);
            assert!(inspect(&fixture.inputs(PlatformArtifacts::Linux)).is_ok());
            Fixture::replace_field(&record, 1, path);
            let error = inspect(&fixture.inputs(PlatformArtifacts::Linux))
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("canonical") || error.contains("absolute"),
                "{error}"
            );
            assert!(error.contains("rerun the installer"), "{error}");
        }
        fixture.flat(Some(b"desktop"));
        let bundle = fixture.bundle(false);
        Fixture::replace_field(
            &bundle.join("Contents").join(FLAT_RECORD),
            1,
            alias.as_os_str().as_bytes(),
        );
        let error = inspect(&fixture.inputs(PlatformArtifacts::Macos))
            .unwrap_err()
            .to_string();
        assert!(error.contains("canonical"), "{error}");
    }

    /// Validate every fixed app boundary, including directories: following a
    /// linked parent would bypass a no-follow check on its child files.
    #[test]
    fn bundle_links_and_unexpected_entries_refuse_at_each_boundary() {
        for component in [
            "",
            "Contents",
            "Contents/MacOS",
            "Contents/Resources",
            "Contents/Info.plist",
            "Contents/MacOS/farhelm",
            "Contents/MacOS/farhelm-desktop",
            "Contents/Resources/Farhelm.icns",
            "Contents/.farhelm-installation",
        ] {
            let fixture = Fixture::new();
            fixture.flat(Some(b"desktop"));
            let bundle = fixture.bundle(false);
            assert!(inspect(&fixture.inputs(PlatformArtifacts::Macos)).is_ok());
            let path = if component.is_empty() {
                bundle.clone()
            } else {
                bundle.join(component)
            };
            let parked = fixture.install.parent().unwrap().join("parked");
            fs::rename(&path, &parked).expect("park fixture artifact");
            symlink(&parked, &path).expect("replace with link");
            let error = inspect(&fixture.inputs(PlatformArtifacts::Macos))
                .unwrap_err()
                .to_string();
            assert!(error.contains("link"), "{component}: {error}");
        }
        for component in ["", "Contents", "Contents/MacOS", "Contents/Resources"] {
            let fixture = Fixture::new();
            fixture.flat(Some(b"desktop"));
            let bundle = fixture.bundle(false);
            assert!(inspect(&fixture.inputs(PlatformArtifacts::Macos)).is_ok());
            let extra = bundle.join(component).join("unowned");
            Fixture::write(&extra, b"keep");
            let error = inspect(&fixture.inputs(PlatformArtifacts::Macos))
                .unwrap_err()
                .to_string();
            assert!(error.contains("unexpected"), "{component}: {error}");
            assert_eq!(fs::read(extra).unwrap(), b"keep");
        }
    }

    /// FIFO and dangling-link metadata must refuse on type, before reading bytes;
    /// otherwise a dry-run could hang waiting for an unrelated writer.
    #[test]
    fn nonregular_record_and_payload_refuse_without_reading() {
        for name in [FLAT_RECORD, DESKTOP] {
            for kind in ["fifo", "directory", "dangling"] {
                let fixture = Fixture::new();
                fixture.flat(Some(b"desktop"));
                assert!(inspect(&fixture.inputs(PlatformArtifacts::Macos)).is_ok());
                let path = fixture.install.join(name);
                fs::remove_file(&path).unwrap();
                match kind {
                    "fifo" => {
                        let cpath = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
                        // The owned pathname is NUL-terminated and remains alive for this call.
                        assert_eq!(unsafe { libc::mkfifo(cpath.as_ptr(), 0o600) }, 0);
                    }
                    "directory" => fs::create_dir(&path).unwrap(),
                    _ => symlink(fixture.install.join("missing-target"), &path).unwrap(),
                }
                assert!(!fs::symlink_metadata(&path).unwrap().file_type().is_file());
                let error = inspect(&fixture.inputs(PlatformArtifacts::Macos))
                    .unwrap_err()
                    .to_string();
                assert!(error.contains("regular file"), "{name}/{kind}: {error}");
            }
        }
    }

    /// Every app digest binds its own payload; swapped or ignored hash fields
    /// must not let an altered app pass under a still-valid flat receipt.
    #[test]
    fn each_bundle_payload_is_bound_to_its_own_digest() {
        for component in [
            "MacOS/farhelm",
            "MacOS/farhelm-desktop",
            "Info.plist",
            "Resources/Farhelm.icns",
        ] {
            let fixture = Fixture::new();
            fixture.flat(Some(b"desktop"));
            let bundle = fixture.bundle(true);
            assert!(inspect(&fixture.inputs(PlatformArtifacts::Macos)).is_ok());
            let path = bundle.join("Contents").join(component);
            Fixture::write(&path, b"altered");
            let error = inspect(&fixture.inputs(PlatformArtifacts::Macos))
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("digest") && error.contains(&path_text(&path)),
                "{error}"
            );
        }
    }
    /// Raw bytes must remain distinguishable in errors and no inspection mutates sentinels.
    #[test]
    fn raw_path_bytes_are_escaped_and_inspection_is_read_only() {
        let fixture = Fixture::new();
        let odd = fixture.install.parent().unwrap().join(OsString::from_vec(
            b"space ' quote\\ newline\n\xff".to_vec(),
        ));
        fs::create_dir(&odd).expect("odd");
        let cli = odd.join(CLI);
        Fixture::write(&cli, b"cli");
        let parts = [
            b"farhelm-standalone".as_slice(),
            odd.as_os_str().as_bytes(),
            Fixture::digest(b"cli").as_bytes(),
            b"",
        ]
        .join(&0);
        let mut record = parts;
        record.push(0);
        Fixture::write(&odd.join(FLAT_RECORD), &record);
        let sentinel = fixture.root.path().join("sentinel");
        Fixture::write(&sentinel, b"unchanged");
        let inputs = InspectionInputs {
            current_exe: cli,
            home: None,
            effective_uid: fixture.uid,
            platform: PlatformArtifacts::Linux,
        };
        assert!(inspect(&inputs).is_ok());
        assert_eq!(fs::read(&sentinel).expect("sentinel"), b"unchanged");
        assert!(path_text(&odd).contains("\\\\"));
        assert!(path_text(&odd).contains("\\x0a"));
        assert!(path_text(&odd).contains("\\xff"));
    }
}
