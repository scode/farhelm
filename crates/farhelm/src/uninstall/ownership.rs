//! Verify installer receipts before uninstall can name any removal path.
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

pub(super) const RECORD_LIMIT: u64 = 64 * 1024;
pub(super) const FLAT_RECORD: &str = ".farhelm-installation";
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

/// Everything the remover may act on after service preflight and confirmation.
///
/// Which kind depends on the platform. On macOS the installation is the app
/// bundle with its side-by-side versions ([`super::app`]); on Linux it is a
/// standalone bin directory an earlier installer left (the installer now
/// refuses Linux, but those installations still need a removal path).
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum OwnershipPlan {
    Flat(FlatPlan),
    App(super::app::AppPlan),
}

/// Verified ownership information for the selected standalone directory.
///
/// Paths are derived from fixed names below a verified root. Metadata is kept
/// apart from payloads, and `cli` remains separately identified because it
/// must be the final artifact removed for an ordinary retry to work.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct FlatPlan {
    pub(crate) root: PathBuf,
    pub(crate) cli: PathBuf,
    pub(crate) desktop: Option<PathBuf>,
    pub(crate) retained_foreign_desktop: Option<PathBuf>,
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
pub(super) trait MetadataClassifier {
    fn uid(&self, metadata: &Metadata) -> u32;
}
pub(super) struct NativeClassifier;
impl MetadataClassifier for NativeClassifier {
    fn uid(&self, metadata: &Metadata) -> u32 {
        metadata.uid()
    }
}

/// Run the production inspection with a replaceable ownership classification.
///
/// The classifier changes no traversal or hashing rule; it only makes the
/// effective-UID boundary observable on hosts where a test cannot `chown`.
pub(super) fn inspect_with_classifier(
    inputs: &InspectionInputs,
    classifier: &impl MetadataClassifier,
) -> Result<OwnershipPlan> {
    match inputs.platform {
        PlatformArtifacts::Macos => {
            super::app::inspect_app(inputs, classifier).map(OwnershipPlan::App)
        }
        PlatformArtifacts::Linux => inspect_flat(inputs, classifier).map(OwnershipPlan::Flat),
    }
}

/// Verify the standalone bin directory that holds the running CLI, from the
/// record beside it.
fn inspect_flat(
    inputs: &InspectionInputs,
    classifier: &impl MetadataClassifier,
) -> Result<FlatPlan> {
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
    let root = executable
        .parent()
        .ok_or_else(|| anyhow!("{} has no installation directory", path_text(&executable)))?
        .to_path_buf();
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
    if !fields[3].is_empty() {
        return repair_refusal(
            &metadata,
            "its desktop digest does not match this platform's artifact set",
        );
    }
    let cli = root.join(CLI);
    verify_payload(&cli, &cli_hash, inputs.effective_uid, classifier)?;
    if cli != executable {
        bail!(
            "{} is not the resolved CLI {}",
            path_text(&inputs.current_exe),
            path_text(&cli)
        );
    }
    let desktop_path = root.join(DESKTOP);
    let retained_foreign_desktop = match fs::symlink_metadata(&desktop_path) {
        Ok(_) => Some(desktop_path),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "inspecting retained foreign artifact {}",
                    path_text(&desktop_path)
                )
            });
        }
    };
    Ok(FlatPlan {
        root,
        cli,
        desktop: None,
        retained_foreign_desktop,
        metadata,
    })
}

/// Turn a receipt failure into the repair direction old installs need.
pub(super) fn repair_refusal<T>(path: &Path, reason: &str) -> Result<T> {
    bail!(
        "cannot verify installer ownership record {}: {reason}; rerun the installer, or upgrade once to a release that writes uninstall metadata",
        path_text(path)
    )
}

/// Read a bounded NUL record without following its final directory entry.
pub(super) fn read_record(
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
pub(super) fn open_regular(
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
pub(super) fn field_path(bytes: &[u8], record: &Path, name: &str) -> Result<PathBuf> {
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
    // The advice matters as much as the refusal. The installer publishes the
    // record just after committing new binaries, so an install or update
    // interrupted in between leaves new files beside the old record, and a
    // bare "does not match" reads like tampering. Re-running the installer
    // republishes the record for what is installed.
    if &actual != expected {
        bail!(
            "{} does not match its recorded SHA-256 digest; if an install or update was interrupted, \
             rerun the installer to record the files it installed, then run uninstall again",
            path_text(path)
        );
    }
    Ok(())
}

/// Require a bundle component to be a user-owned directory, never a link.
pub(super) fn validate_dir(
    path: &Path,
    uid: u32,
    classifier: &impl MetadataClassifier,
) -> Result<()> {
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
pub(super) fn exists_dir(
    path: &Path,
    uid: u32,
    classifier: &impl MetadataClassifier,
) -> Result<bool> {
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
pub(super) fn reject_unexpected(path: &Path, allowed: &[&str]) -> Result<()> {
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

/// Quote raw Unix path bytes so a diagnostic cannot forge terminal output.
pub(crate) fn path_text(path: &Path) -> String {
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
pub(crate) mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt as _, symlink};
    /// Own every path inspected by a test; receipts use the same physical
    /// spelling as the installer, including on macOS's aliased temporary root.
    pub(crate) struct Fixture {
        root: tempfile::TempDir,
        pub(crate) install: PathBuf,
        pub(crate) home: PathBuf,
        uid: u32,
    }
    impl Fixture {
        /// Create a private installation and home without changing the test's environment.
        pub(crate) fn new() -> Self {
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
        pub(crate) fn flat(&self, desktop: Option<&[u8]>) {
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
        pub(crate) fn inputs(&self, platform: PlatformArtifacts) -> InspectionInputs {
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
    }
    /// A complete Linux receipt proves the verifier builds authority only from fixed names.
    #[test]
    fn successful_linux_plan_and_retained_desktop_are_distinct() {
        let fixture = Fixture::new();
        fixture.flat(None);
        Fixture::write(&fixture.install.join(DESKTOP), b"foreign");
        let OwnershipPlan::Flat(plan) =
            inspect(&fixture.inputs(PlatformArtifacts::Linux)).expect("valid plan")
        else {
            panic!("a Linux installation is a flat plan");
        };
        assert_eq!(plan.cli, fixture.install.join(CLI));
        assert_eq!(plan.desktop, None);
        assert_eq!(
            plan.retained_foreign_desktop,
            Some(fixture.install.join(DESKTOP))
        );
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
    /// A syntactically valid digest is still not authority over changed bytes,
    /// and the refusal says how an interrupted install is repaired.
    ///
    /// The advice is the point for the common cause: an install or update
    /// interrupted between committing its binaries and publishing their record
    /// leaves exactly this mismatch, and a refusal without it reads like
    /// tampering (SPEC.md "Operator prerequisites and failure behavior": a
    /// refusal says how to handle it).
    #[test]
    fn mismatched_flat_digest_refuses() {
        let fixture = Fixture::new();
        fixture.flat(None);
        Fixture::write(&fixture.install.join(CLI), b"changed");
        let error = inspect(&fixture.inputs(PlatformArtifacts::Linux))
            .unwrap_err()
            .to_string();
        assert!(error.contains("digest"), "{error}");
        assert!(error.contains("rerun the installer"), "{error}");
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
        fixture.flat(None);
        fs::remove_file(fixture.install.join(CLI)).expect("cli");
        symlink(&outside, fixture.install.join(CLI)).expect("payload link");
        assert!(inspect(&fixture.inputs(PlatformArtifacts::Linux)).is_err());
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
    }

    /// FIFO and dangling-link metadata must refuse on type, before reading bytes;
    /// otherwise a dry-run could hang waiting for an unrelated writer.
    #[test]
    fn nonregular_record_and_payload_refuse_without_reading() {
        for name in [FLAT_RECORD] {
            for kind in ["fifo", "directory", "dangling"] {
                let fixture = Fixture::new();
                fixture.flat(None);
                assert!(inspect(&fixture.inputs(PlatformArtifacts::Linux)).is_ok());
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
                let error = inspect(&fixture.inputs(PlatformArtifacts::Linux))
                    .unwrap_err()
                    .to_string();
                assert!(error.contains("regular file"), "{name}/{kind}: {error}");
            }
        }
    }

    /// Raw bytes must remain distinguishable in errors and no inspection mutates sentinels.
    #[test]
    fn raw_path_bytes_are_escaped_and_inspection_is_read_only() {
        let fixture = Fixture::new();
        let raw_name = b"space ' quote\\ newline\n\xff";
        let escaped = path_text(Path::new(&OsString::from_vec(raw_name.to_vec())));
        assert!(escaped.contains("\\xff"));
        // APFS refuses invalid UTF-8 names at creation. Exercise raw-byte
        // rendering above on every platform, and keep real non-UTF-8 paths
        // on Linux while macOS still verifies quotes, slashes and newlines.
        let disk_name = if cfg!(target_os = "macos") {
            b"space ' quote\\ newline\n".as_slice()
        } else {
            raw_name.as_slice()
        };
        let odd = fixture
            .install
            .parent()
            .unwrap()
            .join(OsString::from_vec(disk_name.to_vec()));
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
        // The receipt verifier refuses group- and world-writable records, and
        // a bare write inherits the umask. Under a 0o002 umask that leaves the
        // record group-writable and inspect() would refuse it, so give the
        // record the producer's private mode like `flat` does.
        fs::set_permissions(odd.join(FLAT_RECORD), fs::Permissions::from_mode(0o600))
            .expect("private record");
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
        if !cfg!(target_os = "macos") {
            assert!(path_text(&odd).contains("\\xff"));
        }
    }
}
