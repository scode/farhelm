//! Incremental, fixed-layout trace storage owned by one capture session.
//!
//! This is deliberately synchronous. The collector calls it while holding its admission mutex, so
//! the assigned sequence and on-disk order agree. That means a filesystem write can block a test;
//! the bounded layout limits retained bytes, not kernel I/O latency or power-loss exposure.

use std::ffi::{CStr, CString};
use std::fs::File;
use std::io::{self, Seek as _, Write as _};
use std::os::fd::{AsRawFd as _, FromRawFd as _};
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::MetadataExt as _;
use std::path::PathBuf;
use std::sync::Arc;

use crate::{
    CaptureConfig, CaptureConfigError, CaptureIdentity, LossCounters, MAX_METADATA_RECORD_BYTES,
    MAX_PERSISTENT_SLOTS, ObservedOutcome, TestMetadata,
};

pub(crate) const FILE_NAMES: [&str; 5] = [
    "metadata.json",
    "head.jsonl",
    "tail-0.jsonl",
    "tail-1.jsonl",
    "tail-2.jsonl",
];
const EVENT_FILE_BYTES: usize = 256 * 1024;

/// Private limits and fault seams keep pressure contracts cheap without widening the public API.
#[derive(Clone, Copy)]
struct PersistenceOptions {
    event_file_bytes: usize,
    setup_failure: SetupFailure,
    fail_event_write: Option<u64>,
    partial_event_bytes: Option<usize>,
    fail_tail_truncate: bool,
    fail_tail_rewind: bool,
    fail_final_metadata: bool,
    fail_metadata_after_truncate: bool,
    fail_cleanup_before_file: Option<usize>,
}

impl Default for PersistenceOptions {
    fn default() -> Self {
        Self {
            event_file_bytes: EVENT_FILE_BYTES,
            setup_failure: SetupFailure::None,
            fail_event_write: None,
            partial_event_bytes: None,
            fail_tail_truncate: false,
            fail_tail_rewind: false,
            fail_final_metadata: false,
            fail_metadata_after_truncate: false,
            fail_cleanup_before_file: None,
        }
    }
}

/// One-purpose setup failures exercise cleanup boundaries without filesystem timing tricks.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
enum SetupFailure {
    #[default]
    None,
    BeforeSlotOpen,
    BeforeFile(usize),
}

/// A failure while establishing a persistence root or obtaining a slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PersistenceSetupError {
    UnusableRoot(String),
    Exhausted,
}

/// A validated explicit root. Its shared descriptor anchors all later relative operations.
#[derive(Clone)]
pub(crate) struct PersistenceConfig {
    root: Arc<File>,
}

impl PersistenceConfig {
    /// Opens the caller-created private directory without following its final component.
    pub(crate) fn new(path: PathBuf) -> Result<Self, PersistenceSetupError> {
        if !path.is_absolute() {
            return Err(PersistenceSetupError::UnusableRoot(
                "path must be absolute".to_owned(),
            ));
        }
        // A trailing slash (or redundant dot) turns the preceding component into
        // prefix traversal, where O_NOFOLLOW permits links. Strip that spelling
        // without resolving links or parent components before the no-follow open.
        let normalized: PathBuf = path.components().collect();
        let path = cstring(normalized.as_os_str().as_bytes())
            .map_err(|_| PersistenceSetupError::UnusableRoot("path contains NUL".to_owned()))?;
        // SAFETY: `path` is a NUL-terminated path owned for this call. O_NOFOLLOW and O_DIRECTORY
        // make the final component a real directory before we retain its descriptor.
        let fd = unsafe {
            libc::open(
                path.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(PersistenceSetupError::UnusableRoot(
                io::Error::last_os_error().to_string(),
            ));
        }
        // SAFETY: `fd` is a newly owned descriptor above and is transferred exactly once.
        let root = unsafe { File::from_raw_fd(fd) };
        let metadata = root
            .metadata()
            .map_err(|error| PersistenceSetupError::UnusableRoot(error.to_string()))?;
        if !metadata.is_dir() {
            return Err(PersistenceSetupError::UnusableRoot(
                "not a directory".to_owned(),
            ));
        }
        if metadata.uid() != unsafe { libc::geteuid() } {
            return Err(PersistenceSetupError::UnusableRoot(
                "directory is not owned by the effective user".to_owned(),
            ));
        }
        if metadata.mode() & 0o077 != 0 {
            return Err(PersistenceSetupError::UnusableRoot(
                "directory grants group or world permissions".to_owned(),
            ));
        }
        Ok(Self {
            root: Arc::new(root),
        })
    }
}

/// The one fixed slot and five files created exclusively by a capture session.
pub(crate) struct Persistence {
    root: Arc<File>,
    slot: File,
    slot_name: CString,
    slot_identity: FileIdentity,
    files: [OwnedFile; 5],
    head_bytes: usize,
    tail_bytes: [usize; 3],
    tail_records: [u64; 3],
    next_tail: usize,
    active_tail: Option<usize>,
    head_closed: bool,
    event_writes_failed: bool,
    metadata_failed: bool,
    event_write_attempts: u64,
    options: PersistenceOptions,
}

impl Persistence {
    /// Reserves an unused slot and initializes the complete allowlisted layout before use.
    pub(crate) fn reserve(
        config: PersistenceConfig,
        metadata: &TestMetadata<'_>,
        identity: &CaptureIdentity,
        loss: &LossCounters,
    ) -> Result<Self, PersistenceSetupError> {
        Self::reserve_with_options(
            config,
            metadata,
            identity,
            loss,
            PersistenceOptions::default(),
        )
    }

    /// Shares production reservation logic with private bounds and deterministic fault contracts.
    fn reserve_with_options(
        config: PersistenceConfig,
        metadata: &TestMetadata<'_>,
        identity: &CaptureIdentity,
        loss: &LossCounters,
        options: PersistenceOptions,
    ) -> Result<Self, PersistenceSetupError> {
        let root = config.root;
        for number in 0..MAX_PERSISTENT_SLOTS {
            let name =
                cstring(format!("slot-{number:03}")).expect("fixed ASCII slot name has no NUL");
            // SAFETY: root remains open, name is a fixed NUL-terminated component, and mode is
            // private. EEXIST means any existing entry is occupied evidence, regardless of type.
            let created = unsafe { libc::mkdirat(root.as_raw_fd(), name.as_ptr(), 0o700) };
            if created != 0 {
                if io::Error::last_os_error().raw_os_error() == Some(libc::EEXIST) {
                    continue;
                }
                return Err(PersistenceSetupError::UnusableRoot(
                    io::Error::last_os_error().to_string(),
                ));
            }
            if options.setup_failure == SetupFailure::BeforeSlotOpen {
                return Err(PersistenceSetupError::UnusableRoot(
                    "injected slot open failure".to_owned(),
                ));
            }
            let slot = match open_directory_at(&root, &name) {
                Ok(slot) => slot,
                Err(error) => {
                    return Err(PersistenceSetupError::UnusableRoot(error.to_string()));
                }
            };
            let slot_identity = match identity_of(&slot) {
                Ok(identity) => identity,
                Err(error) => {
                    return Err(PersistenceSetupError::UnusableRoot(format!(
                        "inspect reserved slot: {error}"
                    )));
                }
            };
            let files = match create_files(&slot, options.setup_failure) {
                Ok(files) => files,
                Err(error) => {
                    let _ = cleanup_partial(&root, &slot, &name, slot_identity, &error.created);
                    return Err(PersistenceSetupError::UnusableRoot(error.error.to_string()));
                }
            };
            let mut persistence = Self {
                root,
                slot,
                slot_name: name,
                slot_identity,
                files,
                head_bytes: 0,
                tail_bytes: [0; 3],
                tail_records: [0; 3],
                next_tail: 0,
                active_tail: None,
                head_closed: false,
                event_writes_failed: false,
                metadata_failed: false,
                event_write_attempts: 0,
                options,
            };
            if persistence
                .write_metadata(metadata, identity, None, loss, true)
                .is_err()
            {
                let _ = persistence.cleanup();
                return Err(PersistenceSetupError::UnusableRoot(
                    "write initial metadata".to_owned(),
                ));
            }
            return Ok(persistence);
        }
        Err(PersistenceSetupError::Exhausted)
    }

    /// Arms one deterministic event-write failure after ordinary session setup has completed.
    ///
    /// Tests call this while holding the collector state lock and before emitting events. The
    /// attempt number counts append calls, so a later event proves whether failure was latched.
    #[cfg(test)]
    pub(super) fn fail_event_write_on_attempt(&mut self, attempt: u64) {
        self.options.fail_event_write = Some(attempt);
    }

    /// Exercises a real written prefix before the selected append reports failure.
    #[cfg(test)]
    pub(super) fn fail_partial_event_write_on_attempt(&mut self, attempt: u64, bytes: usize) {
        self.options.fail_event_write = Some(attempt);
        self.options.partial_event_bytes = Some(bytes);
    }

    /// Appends one complete encoded event or records exactly why no persistent copy was retained.
    pub(crate) fn append(&mut self, record: &[u8], loss: &mut LossCounters) {
        if self.event_writes_failed {
            return;
        }
        let required = record.len().saturating_add(1);
        if required > self.options.event_file_bytes {
            loss.persistent_omitted_events = loss.persistent_omitted_events.saturating_add(1);
            return;
        }
        let index = if !self.head_closed
            && self.head_bytes.saturating_add(required) <= self.options.event_file_bytes
        {
            1
        } else {
            self.head_closed = true;
            let tail = match self.active_tail {
                Some(tail)
                    if self.tail_bytes[tail].saturating_add(required)
                        <= self.options.event_file_bytes =>
                {
                    tail
                }
                _ => {
                    let tail = self.next_tail;
                    self.next_tail = (self.next_tail + 1) % 3;
                    self.active_tail = Some(tail);
                    if self.tail_bytes[tail] > 0 {
                        if self.options.fail_tail_truncate
                            || self.files[tail + 2].file.set_len(0).is_err()
                        {
                            self.fail_event_writes(loss);
                            return;
                        }
                        // Truncation displaces the old records even if rewind
                        // fails next; a failed truncation displaces none.
                        loss.rotated_persistent_events = loss
                            .rotated_persistent_events
                            .saturating_add(self.tail_records[tail]);
                        self.tail_bytes[tail] = 0;
                        self.tail_records[tail] = 0;
                        if self.options.fail_tail_rewind
                            || self.files[tail + 2].file.rewind().is_err()
                        {
                            self.fail_event_writes(loss);
                            return;
                        }
                    }
                    tail
                }
            };
            tail + 2
        };
        self.event_write_attempts = self.event_write_attempts.saturating_add(1);
        if self.options.fail_event_write == Some(self.event_write_attempts) {
            if let Some(bytes) = self.options.partial_event_bytes {
                // A write error may follow a successful prefix, including a
                // complete JSON value whose newline was never written.
                let _ = self.files[index]
                    .file
                    .write_all(&record[..bytes.min(record.len())]);
            }
            self.fail_event_writes(loss);
            return;
        }
        let write = self.files[index]
            .file
            .write_all(record)
            .and_then(|()| self.files[index].file.write_all(b"\n"));
        if write.is_err() {
            self.fail_event_writes(loss);
            return;
        }
        if index == 1 {
            self.head_bytes = self.head_bytes.saturating_add(required);
        } else {
            let tail = index - 2;
            self.tail_bytes[tail] = self.tail_bytes[tail].saturating_add(required);
            self.tail_records[tail] = self.tail_records[tail].saturating_add(1);
        }
    }

    /// Finalizes metadata, then removes only a provably unchanged successful session layout.
    pub(crate) fn finish(
        mut self,
        metadata: &TestMetadata<'_>,
        identity: &CaptureIdentity,
        outcome: ObservedOutcome,
        loss: &mut LossCounters,
        retain: bool,
        incomplete: bool,
    ) {
        if !self.metadata_failed
            && (self.options.fail_final_metadata
                || self
                    .write_metadata(metadata, identity, Some(outcome), loss, incomplete)
                    .is_err())
        {
            self.metadata_failed = true;
            loss.persistence_failures = loss.persistence_failures.saturating_add(1);
        }
        if !retain
            && !self.metadata_failed
            && !self.event_writes_failed
            && loss.is_complete()
            && self.cleanup().is_err()
        {
            loss.persistence_failures = loss.persistence_failures.saturating_add(1);
            if self
                .write_metadata(metadata, identity, Some(outcome), loss, true)
                .is_err()
            {
                self.metadata_failed = true;
                loss.persistence_failures = loss.persistence_failures.saturating_add(1);
            }
        }
    }

    /// Latches the first event-output failure so later events cannot retry any event handle.
    fn fail_event_writes(&mut self, loss: &mut LossCounters) {
        if !self.event_writes_failed {
            self.event_writes_failed = true;
            loss.persistence_failures = loss.persistence_failures.saturating_add(1);
        }
    }

    /// Replaces metadata through its retained handle without reopening the visible filename.
    fn write_metadata(
        &mut self,
        metadata: &TestMetadata<'_>,
        identity: &CaptureIdentity,
        outcome: Option<ObservedOutcome>,
        loss: &LossCounters,
        incomplete: bool,
    ) -> io::Result<()> {
        let record = PersistentMetadata {
            kind: "farhelm-testtrace",
            test: metadata,
            identity,
            outcome,
            loss,
            incomplete: incomplete || !loss.is_complete(),
        };
        let bytes = crate::encode_bounded(&record, MAX_METADATA_RECORD_BYTES)
            .map_err(|_| io::Error::other("persistent metadata exceeded its fixed budget"))?;
        let file = &mut self.files[0].file;
        file.set_len(0)?;
        file.rewind()?;
        if outcome.is_some() && self.options.fail_metadata_after_truncate {
            return Err(io::Error::other(
                "injected metadata failure after truncation",
            ));
        }
        file.write_all(&bytes)
    }

    /// Deletes only held, identity-checked files and then the now-empty, identity-checked slot.
    fn cleanup(&self) -> io::Result<()> {
        if identity_at(&self.root, &self.slot_name)? != self.slot_identity {
            return Err(io::Error::other("reserved persistence slot was replaced"));
        }
        verify_owned_directory(&self.slot, &self.files)?;
        for (index, owned) in self.files.iter().enumerate().rev() {
            if self.options.fail_cleanup_before_file == Some(index) {
                return Err(io::Error::other("injected cleanup failure before unlink"));
            }
            unlink_at(&self.slot, &owned.name, 0)?;
        }
        if identity_at(&self.root, &self.slot_name)? != self.slot_identity {
            return Err(io::Error::other("reserved persistence slot was replaced"));
        }
        unlink_at(&self.root, &self.slot_name, libc::AT_REMOVEDIR)
    }
}

/// Reader-visible session state written initially and once the outcome is known.
#[derive(serde::Serialize)]
struct PersistentMetadata<'a> {
    kind: &'static str,
    test: &'a TestMetadata<'a>,
    identity: &'a CaptureIdentity,
    outcome: Option<ObservedOutcome>,
    loss: &'a LossCounters,
    incomplete: bool,
}

/// Validates the actual persistent metadata shapes before the caller copies borrowed metadata.
pub(crate) fn validate_metadata(
    metadata: &TestMetadata<'_>,
    identity: &CaptureIdentity,
    config: CaptureConfig,
) -> Result<(), CaptureConfigError> {
    let maximum_loss = LossCounters {
        evicted_events: u64::MAX,
        dropped_events: u64::MAX,
        truncated_fields: u64::MAX,
        saturated_spans: u64::MAX,
        persistence_failures: u64::MAX,
        persistent_omitted_events: u64::MAX,
        rotated_persistent_events: u64::MAX,
        diagnostic_failures: u64::MAX,
        omitted_dump_events: u64::MAX,
    };
    for (outcome, incomplete) in [
        (None, true),
        (Some(ObservedOutcome::ReturnedSuccess), true),
        (Some(ObservedOutcome::ReturnedFailure), false),
        (Some(ObservedOutcome::Unwind), true),
        (Some(ObservedOutcome::ObservationFailed), true),
    ] {
        let record = PersistentMetadata {
            kind: "farhelm-testtrace",
            test: metadata,
            identity,
            outcome,
            loss: &maximum_loss,
            incomplete,
        };
        if crate::encode_bounded(&record, config.max_metadata_record_bytes).is_err() {
            return Err(CaptureConfigError::MetadataDoesNotFit {
                encoded_bytes: crate::encoded_size(&record),
                budget: config.max_metadata_record_bytes,
            });
        }
    }
    Ok(())
}

/// A retained descriptor paired with the exact directory entry this session created.
struct OwnedFile {
    name: CString,
    file: File,
    identity: FileIdentity,
}

/// Carries enough successful setup state to perform identity-proven partial cleanup.
struct CreateFilesError {
    error: io::Error,
    created: Vec<OwnedFile>,
}

/// Creates the fixed allowlist exclusively and returns every descriptor as one complete set.
fn create_files(
    slot: &File,
    setup_failure: SetupFailure,
) -> Result<[OwnedFile; 5], CreateFilesError> {
    let mut created = Vec::with_capacity(FILE_NAMES.len());
    for (index, file_name) in FILE_NAMES.into_iter().enumerate() {
        if setup_failure == SetupFailure::BeforeFile(index) {
            return Err(CreateFilesError {
                error: io::Error::other("injected persistence file creation failure"),
                created,
            });
        }
        let name = cstring(file_name).expect("fixed ASCII filename has no NUL");
        match create_file_at(slot, &name) {
            Ok(file) => match identity_of(&file) {
                Ok(identity) => created.push(OwnedFile {
                    name,
                    file,
                    identity,
                }),
                Err(error) => return Err(CreateFilesError { error, created }),
            },
            Err(error) => return Err(CreateFilesError { error, created }),
        }
    }
    match created.try_into() {
        Ok(files) => Ok(files),
        Err(_) => unreachable!("the fixed creation loop returned before producing five files"),
    }
}

/// Removes a partial setup only after proving every visible object belongs to this attempt.
fn cleanup_partial(
    root: &File,
    slot: &File,
    slot_name: &CString,
    slot_identity: FileIdentity,
    files: &[OwnedFile],
) -> io::Result<()> {
    if identity_at(root, slot_name)? != slot_identity {
        return Err(io::Error::other("partially initialized slot was replaced"));
    }
    verify_owned_directory(slot, files)?;
    for owned in files.iter().rev() {
        unlink_at(slot, &owned.name, 0)?;
    }
    if identity_at(root, slot_name)? != slot_identity {
        return Err(io::Error::other("partially initialized slot was replaced"));
    }
    unlink_at(root, slot_name, libc::AT_REMOVEDIR)
}

/// Proves a directory contains exactly the retained owned files before cleanup starts.
fn verify_owned_directory(slot: &File, files: &[OwnedFile]) -> io::Result<()> {
    verify_owned_entries(slot, files)
}

/// Bounded inspection refuses on the first entry outside the retained allowlist.
fn verify_owned_entries(slot: &File, files: &[OwnedFile]) -> io::Result<()> {
    for owned in files {
        if identity_at(slot, &owned.name)? != owned.identity {
            return Err(io::Error::other("owned persistence file was replaced"));
        }
    }
    let descriptor = unsafe { libc::fcntl(slot.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
    if descriptor < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fdopendir takes ownership of this fresh duplicate on success.
    let directory = unsafe { libc::fdopendir(descriptor) };
    if directory.is_null() {
        let error = io::Error::last_os_error();
        // SAFETY: fdopendir did not take ownership on failure.
        unsafe { libc::close(descriptor) };
        return Err(error);
    }
    let mut seen = 0_usize;
    loop {
        set_errno(0);
        // SAFETY: directory remains live until closed below; readdir owns its internal buffer.
        let entry = unsafe { libc::readdir(directory) };
        if entry.is_null() {
            let error = errno();
            // SAFETY: directory was returned by fdopendir and is closed exactly once.
            unsafe { libc::closedir(directory) };
            if error != 0 {
                return Err(io::Error::from_raw_os_error(error));
            }
            break;
        }
        // SAFETY: POSIX guarantees d_name is NUL-terminated for the returned live entry.
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        seen = seen.saturating_add(1);
        if seen > files.len() || !files.iter().any(|owned| owned.name.as_bytes() == name) {
            // SAFETY: directory was returned by fdopendir and is closed exactly once.
            unsafe { libc::closedir(directory) };
            return Err(io::Error::other(
                "persistence slot contains an unexpected entry",
            ));
        }
    }
    if seen != files.len() {
        return Err(io::Error::other(
            "persistence slot is missing an owned file",
        ));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
/// Returns this thread's errno cell on libc targets with the GNU accessor.
fn errno_pointer() -> *mut libc::c_int {
    // SAFETY: libc exposes the calling thread's errno storage for these targets.
    unsafe { libc::__errno_location() }
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
/// Returns this thread's errno cell on the supported BSD-derived libc targets.
fn errno_pointer() -> *mut libc::c_int {
    // SAFETY: the supported non-Linux Unix libc targets expose errno through __error.
    unsafe { libc::__error() }
}

/// Clears stale errno before `readdir` so EOF can be distinguished from failure.
fn set_errno(value: libc::c_int) {
    // SAFETY: errno_pointer returns writable thread-local errno storage.
    unsafe { *errno_pointer() = value };
}

/// Reads the errno value set by the immediately preceding directory operation.
fn errno() -> libc::c_int {
    // SAFETY: errno_pointer returns readable thread-local errno storage.
    unsafe { *errno_pointer() }
}

/// Stable descriptor identity used to refuse missing, replaced, or type-changed entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileIdentity {
    device: u64,
    inode: u64,
    file_type: u32,
}

/// Reads identity from a descriptor that already owns the intended object.
fn identity_of(file: &File) -> io::Result<FileIdentity> {
    let metadata = file.metadata()?;
    Ok(FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
        file_type: metadata.mode() & file_type_mask(),
    })
}

/// Inspects one relative name without following a symlink at its final component.
#[allow(clippy::unnecessary_fallible_conversions, clippy::useless_conversion)]
fn identity_at(directory: &File, name: &CString) -> io::Result<FileIdentity> {
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: held directory descriptor, fixed relative name, and writable output structure.
    if unsafe {
        libc::fstatat(
            directory.as_raw_fd(),
            name.as_ptr(),
            &mut stat,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(FileIdentity {
        device: u64::try_from(stat.st_dev).unwrap_or(u64::MAX),
        inode: u64::try_from(stat.st_ino).unwrap_or(u64::MAX),
        file_type: u32::try_from(stat.st_mode).unwrap_or(u32::MAX) & file_type_mask(),
    })
}

/// Normalizes libc's target-specific mode type before comparing directory-entry kinds.
#[allow(clippy::unnecessary_fallible_conversions, clippy::useless_conversion)]
fn file_type_mask() -> u32 {
    u32::try_from(libc::S_IFMT).expect("POSIX file-type mask fits u32")
}

/// Opens one relative directory without rediscovering its parent or following its final name.
fn open_directory_at(directory: &File, name: &CString) -> io::Result<File> {
    // SAFETY: held directory descriptor and fixed relative component; ownership transfers below.
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        // SAFETY: the descriptor was returned by openat and is owned by this function.
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

/// Exclusively creates one private regular file below a retained directory descriptor.
fn create_file_at(directory: &File, name: &CString) -> io::Result<File> {
    // SAFETY: held directory descriptor and fixed relative component; O_EXCL refuses replacement.
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        // SAFETY: the descriptor was returned by openat and is owned by this function.
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

/// Removes one previously checked relative name without resolving its parent path again.
fn unlink_at(directory: &File, name: &CString, flags: libc::c_int) -> io::Result<()> {
    // SAFETY: held directory descriptor and a fixed relative component selected after identity check.
    if unsafe { libc::unlinkat(directory.as_raw_fd(), name.as_ptr(), flags) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Converts a pathname component only when libc can receive it without truncation.
fn cstring(bytes: impl AsRef<[u8]>) -> Result<CString, std::ffi::NulError> {
    CString::new(bytes.as_ref())
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::fs;
    use std::os::fd::AsRawFd as _;
    use std::path::Path;

    use super::*;
    use crate::tests::private_tempdir;
    use crate::{ExpectedPanic, MAX_METADATA_RECORD_BYTES, RuntimeConfig};

    /// Supplies stable bounded metadata to tests that focus on filesystem behavior.
    fn metadata() -> TestMetadata<'static> {
        TestMetadata {
            name: Cow::Borrowed("persistence-contract"),
            expected_panic: ExpectedPanic::None,
            runtime: None::<RuntimeConfig>,
        }
    }

    /// Opens a private temporary root through the same validation as public callers.
    fn config(root: &Path) -> PersistenceConfig {
        PersistenceConfig::new(root.to_path_buf()).unwrap()
    }

    /// Reads all complete event records and sorts their explicit sequence fields.
    fn sequences(root: &Path) -> Vec<u64> {
        let slot = root.join("slot-000");
        let mut sequences = FILE_NAMES[1..]
            .iter()
            .flat_map(|name| {
                fs::read(slot.join(name))
                    .unwrap()
                    .split(|byte| *byte == b'\n')
                    .filter(|line| !line.is_empty())
                    .map(|line| {
                        serde_json::from_slice::<serde_json::Value>(line).unwrap()["sequence"]
                            .as_u64()
                            .unwrap()
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        sequences.sort_unstable();
        sequences
    }

    /// A slot whose descriptor could not be obtained remains occupied evidence.
    #[test]
    fn failed_slot_open_never_unlinks_an_unverified_name() {
        let root = private_tempdir();
        let error = Persistence::reserve_with_options(
            config(root.path()),
            &metadata(),
            &CaptureIdentity::now(),
            &LossCounters::default(),
            PersistenceOptions {
                setup_failure: SetupFailure::BeforeSlotOpen,
                ..PersistenceOptions::default()
            },
        )
        .err()
        .unwrap();
        assert!(matches!(error, PersistenceSetupError::UnusableRoot(_)));
        assert!(root.path().join("slot-000").is_dir());
    }

    /// A proven partial layout can be removed without consuming a slot permanently.
    #[test]
    fn file_creation_failure_cleans_only_identity_proven_output() {
        let root = private_tempdir();
        let error = Persistence::reserve_with_options(
            config(root.path()),
            &metadata(),
            &CaptureIdentity::now(),
            &LossCounters::default(),
            PersistenceOptions {
                setup_failure: SetupFailure::BeforeFile(3),
                ..PersistenceOptions::default()
            },
        )
        .err()
        .unwrap();
        assert!(matches!(error, PersistenceSetupError::UnusableRoot(_)));
        assert_eq!(root.path().read_dir().unwrap().count(), 0);
    }

    /// An unexpected partial-slot entry prevents deletion of every verified owned file.
    #[test]
    fn partial_cleanup_preserves_unexpected_entries_and_owned_evidence() {
        let root = private_tempdir();
        let root_handle = config(root.path()).root;
        let slot_name = cstring("slot-000").unwrap();
        // SAFETY: the descriptor is this fixture's live private root and the name is fixed.
        assert_eq!(
            unsafe { libc::mkdirat(root_handle.as_raw_fd(), slot_name.as_ptr(), 0o700) },
            0
        );
        let slot = open_directory_at(&root_handle, &slot_name).unwrap();
        let slot_identity = identity_of(&slot).unwrap();
        let owned_name = cstring("metadata.json").unwrap();
        let owned_file = create_file_at(&slot, &owned_name).unwrap();
        let owned = OwnedFile {
            identity: identity_of(&owned_file).unwrap(),
            name: owned_name,
            file: owned_file,
        };
        fs::write(root.path().join("slot-000/foreign"), "foreign").unwrap();
        assert!(cleanup_partial(&root_handle, &slot, &slot_name, slot_identity, &[owned]).is_err());
        assert!(root.path().join("slot-000/metadata.json").exists());
        assert_eq!(
            fs::read_to_string(root.path().join("slot-000/foreign")).unwrap(),
            "foreign"
        );
    }

    /// Failure injected before any metadata write preserves the initial snapshot without a retry.
    #[test]
    fn final_metadata_failure_preserves_initial_snapshot() {
        let root = private_tempdir();
        let identity = CaptureIdentity::now();
        let metadata = metadata();
        let mut loss = LossCounters::default();
        let persistence = Persistence::reserve_with_options(
            config(root.path()),
            &metadata,
            &identity,
            &loss,
            PersistenceOptions {
                fail_final_metadata: true,
                ..PersistenceOptions::default()
            },
        )
        .unwrap();
        persistence.finish(
            &metadata,
            &identity,
            ObservedOutcome::ReturnedFailure,
            &mut loss,
            true,
            false,
        );
        assert_eq!(loss.persistence_failures, 1);
        let persisted: serde_json::Value =
            serde_json::from_slice(&fs::read(root.path().join("slot-000/metadata.json")).unwrap())
                .unwrap();
        assert!(persisted["outcome"].is_null());
        assert_eq!(persisted["incomplete"], true);
    }

    /// Failure after truncation can erase metadata; it cannot trigger cleanup or another write.
    #[test]
    fn metadata_failure_after_truncation_retains_empty_metadata_and_events() {
        let root = private_tempdir();
        let identity = CaptureIdentity::now();
        let metadata = metadata();
        let mut loss = LossCounters::default();
        let mut persistence = Persistence::reserve_with_options(
            config(root.path()),
            &metadata,
            &identity,
            &loss,
            PersistenceOptions {
                fail_metadata_after_truncate: true,
                ..PersistenceOptions::default()
            },
        )
        .unwrap();
        persistence.append(br#"{"sequence":0}"#, &mut loss);
        persistence.finish(
            &metadata,
            &identity,
            ObservedOutcome::ReturnedSuccess,
            &mut loss,
            false,
            false,
        );
        let slot = root.path().join("slot-000");
        assert_eq!(loss.persistence_failures, 1);
        assert!(fs::read(slot.join("metadata.json")).unwrap().is_empty());
        assert_eq!(
            fs::read(slot.join("head.jsonl")).unwrap(),
            b"{\"sequence\":0}\n"
        );
        assert_eq!(slot.read_dir().unwrap().count(), FILE_NAMES.len());
    }

    /// A mid-cleanup failure preserves healthy metadata after the first event-file unlink.
    #[test]
    fn cleanup_failure_after_an_unlink_discloses_the_partial_layout() {
        let root = private_tempdir();
        let identity = CaptureIdentity::now();
        let metadata = metadata();
        let mut loss = LossCounters::default();
        let persistence = Persistence::reserve_with_options(
            config(root.path()),
            &metadata,
            &identity,
            &loss,
            PersistenceOptions {
                fail_cleanup_before_file: Some(3),
                ..PersistenceOptions::default()
            },
        )
        .unwrap();
        persistence.finish(
            &metadata,
            &identity,
            ObservedOutcome::ReturnedSuccess,
            &mut loss,
            false,
            false,
        );
        let slot = root.path().join("slot-000");
        assert!(!slot.join("tail-2.jsonl").exists());
        for name in &FILE_NAMES[..4] {
            assert!(slot.join(name).is_file());
        }
        assert_eq!(loss.persistence_failures, 1);
        let bytes = fs::read(slot.join("metadata.json")).unwrap();
        assert!(bytes.len() <= MAX_METADATA_RECORD_BYTES);
        let state: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(state["outcome"], "returned_success");
        assert_eq!(state["incomplete"], true);
        assert_eq!(state["loss"]["persistence_failures"], 1);
    }

    /// The head remains a contiguous prefix after a smaller later record could fit its slack.
    #[test]
    fn head_transition_is_latched_across_different_record_sizes() {
        let root = private_tempdir();
        let identity = CaptureIdentity::now();
        let metadata = metadata();
        let loss = LossCounters::default();
        let mut persistence = Persistence::reserve_with_options(
            config(root.path()),
            &metadata,
            &identity,
            &loss,
            PersistenceOptions {
                event_file_bytes: 41,
                ..PersistenceOptions::default()
            },
        )
        .unwrap();
        let mut loss = loss;
        persistence.append(br#"{"sequence":0,"pad":"xx"}"#, &mut loss);
        persistence.append(br#"{"sequence":1,"pad":"xxxxxxxx"}"#, &mut loss);
        persistence.append(br#"{"sequence":2}"#, &mut loss);
        drop(persistence);
        let head = fs::read_to_string(root.path().join("slot-000/head.jsonl")).unwrap();
        assert!(head.contains(r#""sequence":0"#));
        assert!(!head.contains(r#""sequence":2"#));
        assert_eq!(sequences(root.path()), vec![0, 1, 2]);
    }

    /// Three rotating tails retain their newest chunks while sequence numbers recover order.
    #[test]
    fn tail_rotation_counts_displaced_records_and_holds_every_file_bound() {
        assert_eq!(PersistenceOptions::default().event_file_bytes, 256 * 1024);
        let root = private_tempdir();
        let identity = CaptureIdentity::now();
        let metadata = metadata();
        let mut loss = LossCounters::default();
        let mut persistence = Persistence::reserve_with_options(
            config(root.path()),
            &metadata,
            &identity,
            &loss,
            PersistenceOptions {
                event_file_bytes: 16,
                ..PersistenceOptions::default()
            },
        )
        .unwrap();
        for sequence in 0..8 {
            persistence.append(
                format!(r#"{{"sequence":{sequence}}}"#).as_bytes(),
                &mut loss,
            );
        }
        drop(persistence);
        assert_eq!(loss.rotated_persistent_events, 4);
        assert_eq!(sequences(root.path()), vec![0, 5, 6, 7]);
        let slot = root.path().join("slot-000");
        assert_eq!(slot.read_dir().unwrap().count(), FILE_NAMES.len());
        assert!(fs::metadata(slot.join("metadata.json")).unwrap().len() <= 4 * 1024);
        for name in &FILE_NAMES[1..] {
            assert!(fs::metadata(slot.join(name)).unwrap().len() <= 16);
        }
    }

    /// Rotation loss follows successful truncation, even when the subsequent rewind fails.
    ///
    /// Both faults stop further event I/O. Inspect bytes and final metadata so
    /// conservative incompleteness cannot hide an inaccurate displaced count.
    #[test]
    fn tail_rotation_faults_count_only_displaced_records() {
        for fail_truncate in [true, false] {
            let root = private_tempdir();
            let identity = CaptureIdentity::now();
            let metadata = metadata();
            let mut loss = LossCounters::default();
            let mut persistence = Persistence::reserve_with_options(
                config(root.path()),
                &metadata,
                &identity,
                &loss,
                PersistenceOptions {
                    event_file_bytes: 16,
                    fail_tail_truncate: fail_truncate,
                    fail_tail_rewind: !fail_truncate,
                    ..PersistenceOptions::default()
                },
            )
            .unwrap();
            for sequence in 0..4 {
                persistence.append(
                    format!(r#"{{"sequence":{sequence}}}"#).as_bytes(),
                    &mut loss,
                );
            }
            let slot = root.path().join("slot-000");
            let original = fs::read(slot.join("tail-0.jsonl")).unwrap();
            assert_eq!(original, b"{\"sequence\":1}\n");
            persistence.append(br#"{"sequence":4}"#, &mut loss);
            let displaced = u64::from(!fail_truncate);
            assert_eq!(loss.rotated_persistent_events, displaced);
            assert_eq!(loss.persistence_failures, 1);
            assert!(persistence.event_writes_failed);
            let after_failure = fs::read(slot.join("tail-0.jsonl")).unwrap();
            assert_eq!(after_failure, if fail_truncate { original } else { vec![] });
            persistence.append(br#"{"sequence":5}"#, &mut loss);
            assert_eq!(persistence.event_write_attempts, 4);
            assert_eq!(loss.persistence_failures, 1);
            assert_eq!(fs::read(slot.join("tail-0.jsonl")).unwrap(), after_failure);
            persistence.finish(
                &metadata,
                &identity,
                ObservedOutcome::ReturnedFailure,
                &mut loss,
                true,
                true,
            );
            let state: serde_json::Value =
                serde_json::from_slice(&fs::read(slot.join("metadata.json")).unwrap()).unwrap();
            assert_eq!(state["incomplete"], true);
            assert_eq!(state["loss"]["persistence_failures"], 1);
            if fail_truncate {
                assert!(state["loss"].get("rotated_persistent_events").is_none());
                assert_eq!(sequences(root.path()), vec![0, 1, 2, 3]);
            } else {
                assert_eq!(state["loss"]["rotated_persistent_events"], 1);
                assert_eq!(sequences(root.path()), vec![0, 2, 3]);
            }
        }
    }

    /// A whole record larger than an empty chunk is omitted without writing a prefix.
    #[test]
    fn oversized_persistent_record_is_counted_without_partial_json() {
        let root = private_tempdir();
        let identity = CaptureIdentity::now();
        let metadata = metadata();
        let mut loss = LossCounters::default();
        let mut persistence = Persistence::reserve_with_options(
            config(root.path()),
            &metadata,
            &identity,
            &loss,
            PersistenceOptions {
                event_file_bytes: 8,
                ..PersistenceOptions::default()
            },
        )
        .unwrap();
        persistence.append(br#"{"sequence":0}"#, &mut loss);
        drop(persistence);
        assert_eq!(loss.persistent_omitted_events, 1);
        for name in &FILE_NAMES[1..] {
            assert_eq!(
                fs::metadata(root.path().join("slot-000").join(name))
                    .unwrap()
                    .len(),
                0
            );
        }
    }

    /// Metadata validation covers every final outcome with maximum-width counters.
    #[test]
    fn persistent_metadata_validation_covers_the_longest_final_variants() {
        let identity = CaptureIdentity::now();
        let metadata = metadata();
        let minimum = (1..=MAX_METADATA_RECORD_BYTES)
            .find(|budget| {
                validate_metadata(
                    &metadata,
                    &identity,
                    CaptureConfig {
                        max_metadata_record_bytes: *budget,
                        ..CaptureConfig::default()
                    },
                )
                .is_ok()
            })
            .expect("the production metadata budget must fit every final variant");
        assert!(minimum > 1);
        assert!(
            validate_metadata(
                &metadata,
                &identity,
                CaptureConfig {
                    max_metadata_record_bytes: minimum - 1,
                    ..CaptureConfig::default()
                },
            )
            .is_err()
        );
    }
}
