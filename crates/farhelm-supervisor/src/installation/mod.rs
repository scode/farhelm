//! The Mac installation as installs, uninstall and the app's updater share
//! it: the installation lock, the old installers' lock folder, the extended
//! ownership record, and the flushes their write order rests on
//! (SPEC_impl.md, "Side-by-side versions inside Farhelm.app", and the
//! race-free installer design it implements).
//!
//! # The installation lock
//!
//! One per user, on the `~/Applications` folder itself: an exclusive `flock`
//! held by the `farhelm` doing the locked work for exactly as long as it
//! works, and dropped by the kernel when that process exits or is killed, so
//! it can never be left behind and nobody is ever told to delete it. The
//! lock is on the folder, not on anything derived from where the app is, so
//! an install, an uninstall of an app that was moved elsewhere, and the
//! updater all meet on the same lock for the one Terminal link they change.
//! Where a folder cannot be locked, a fixed lock file in `~/Applications`
//! that Farhelm never deletes stands in ([`FALLBACK_LOCK_FILE`]).
//!
//! # The old installers' lock folder
//!
//! Installers older than this design only know an empty folder beside the
//! app, `.farhelm-app.lock`, made with `mkdir`, and their advice for a stuck
//! one is to remove it by hand. Install and uninstall therefore take that
//! folder too, after the kernel lock, so old and new tools exclude each
//! other ([`CompatLockFolder`]). A new tool marks its folder with an
//! extended attribute rather than a file inside, so the folder stays empty
//! and that advice still works; it creates the folder under a temporary
//! name, marks it, and renames it into place only if nothing is there
//! (`RENAME_EXCL`), and removes it by renaming it away before deleting it.
//! A marked folder found while holding the kernel lock was left by a new
//! tool that died (the kernel lock proves no new tool is inside), so it is
//! removed; an unmarked one is an older tool's and refuses, as before. The
//! updater's short read takes the kernel lock alone, so it never makes an
//! older installer refuse.
//!
//! # The ownership record
//!
//! `Contents/.farhelm-installation` keeps the format name and first two
//! NUL-terminated fields older tools read (`farhelm-app-v2`, then the
//! Terminal link) and appends an installation id, a generation and a state
//! ([`OwnershipRecord`]). Older installers recognize it by its first two
//! fields; older uninstallers, which require exactly two, refuse it with
//! their own "rerun the installer" advice (the design's "Old sessions and
//! uninstall" says when and how to get out of it). A two-field record means
//! "complete, no id".

pub mod dirs;
pub mod install;

use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The format name every ownership record of the side-by-side layout starts
/// with, older or newer.
pub const RECORD_MAGIC: &str = "farhelm-app-v2";

/// The ownership record's file name inside `Contents/`.
pub const RECORD_FILE: &str = ".farhelm-installation";

/// The old installers' lock folder, beside the app in `~/Applications`.
pub const COMPAT_LOCK_FOLDER: &str = ".farhelm-app.lock";

/// The lock file used in place of a lock on `~/Applications` itself where a
/// folder cannot be locked. Never deleted, because a lock only excludes
/// holders of the same file.
pub const FALLBACK_LOCK_FILE: &str = ".farhelm-install.lock";

/// The extended attribute that marks [`COMPAT_LOCK_FOLDER`] as made by a
/// tool that also holds the kernel lock. Linux only allows user attributes
/// in the `user.` namespace; it exists there for tests.
#[cfg(target_os = "macos")]
const COMPAT_MARK: &str = "org.farhelm.install-lock";
#[cfg(not(target_os = "macos"))]
const COMPAT_MARK: &str = "user.farhelm.install-lock";

/// How long a run started from Terminal waits for a busy installation lock
/// before saying another install or uninstall is running. The updater's own
/// locked read takes well under a second; this only covers that and a run
/// finishing.
pub const TERMINAL_LOCK_WAIT: Duration = Duration::from_secs(5);

const LOCK_RETRY: Duration = Duration::from_millis(50);

// ---------------------------------------------------------------------------
// Flushing
// ---------------------------------------------------------------------------

/// Make a file's contents, or a folder's entries, durable before anything
/// that depends on them.
///
/// On a Mac this is `F_FULLFSYNC`, which drains the disk's own queue; plain
/// `fsync` there does not reach the disk and, on a folder, appears to do
/// nothing. Where `F_FULLFSYNC` is not supported (not APFS or HFS+, and
/// every other platform) plain `fsync` is used and the stronger promise is
/// not made. A failure stops the caller's run before its next step.
pub fn full_sync(fd: BorrowedFd<'_>) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    match rustix::fs::fcntl_fullfsync(fd) {
        Ok(()) => return Ok(()),
        Err(error)
            if error == rustix::io::Errno::NOTSUP
                || error == rustix::io::Errno::INVAL
                || error == rustix::io::Errno::NOTTY => {}
        Err(error) => return Err(error.into()),
    }
    rustix::fs::fsync(fd).map_err(io::Error::from)
}

// ---------------------------------------------------------------------------
// The installation lock
// ---------------------------------------------------------------------------

/// Why the installation lock could not be taken.
#[derive(Debug, thiserror::Error)]
pub enum InstallLockError {
    /// Another install or uninstall holds it (the lock cannot be stale).
    #[error("another Farhelm install or uninstall is running; wait for it to finish and try again")]
    Busy,
    /// Neither `~/Applications` nor a lock file in it can be locked.
    #[error("{} cannot be locked (locking is not supported on that volume)", .0.display())]
    Unsupported(PathBuf),
    /// Anything else: creating or opening the folder failed.
    #[error("locking {}: {error}", path.display())]
    Io { path: PathBuf, error: io::Error },
}

/// The installation lock, held until dropped (the kernel releases it with
/// the descriptor). Also holds `~/Applications` open, which every step
/// inside it reaches the app through.
#[derive(Debug)]
pub struct InstallationLock {
    applications: OwnedFd,
    /// The fallback lock file, when the folder itself could not be locked.
    _fallback: Option<File>,
    path: PathBuf,
}

impl InstallationLock {
    /// `~/Applications`, held open.
    pub fn applications(&self) -> BorrowedFd<'_> {
        self.applications.as_fd()
    }

    /// `~/Applications`' path, for messages and for public names outside the
    /// app (which keep their path-based checks).
    pub fn applications_path(&self) -> &Path {
        &self.path
    }
}

/// Whether an error means "this filesystem cannot lock", as opposed to a
/// real failure.
fn lock_unsupported(error: &rustix::io::Errno) -> bool {
    use rustix::io::Errno;
    *error == Errno::NOTSUP || *error == Errno::OPNOTSUPP || *error == Errno::NOLCK
}

/// Take the installation lock on `applications` (`~/Applications`),
/// creating the folder first if it is missing, waiting up to `wait` for a
/// running install or uninstall (`Duration::ZERO` tries once).
pub fn lock_installation(
    applications: &Path,
    wait: Duration,
) -> Result<InstallationLock, InstallLockError> {
    use rustix::fs::{FlockOperation, Mode, OFlags};
    let io_error = |error: io::Error| InstallLockError::Io {
        path: applications.to_path_buf(),
        error,
    };
    match std::fs::DirBuilder::new().create(applications) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(io_error(error)),
    }
    let folder = rustix::fs::open(
        applications,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| io_error(error.into()))?;
    let deadline = Instant::now() + wait;
    let try_lock = |fd: BorrowedFd<'_>| -> Result<bool, rustix::io::Errno> {
        match rustix::fs::flock(fd, FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => Ok(true),
            Err(error) if error == rustix::io::Errno::WOULDBLOCK => Ok(false),
            Err(error) => Err(error),
        }
    };
    let waited = |result: Result<(), WaitError>| match result {
        Ok(()) => Ok(()),
        Err(WaitError::Busy) => Err(InstallLockError::Busy),
        Err(WaitError::Errno(error)) => Err(io_error(error.into())),
    };
    // The folder itself first; the fallback file only where the folder
    // cannot be locked at all.
    let fallback = match try_lock(folder.as_fd()) {
        Ok(true) => None,
        Ok(false) => {
            waited(wait_for(&deadline, || try_lock(folder.as_fd())))?;
            None
        }
        Err(error) if lock_unsupported(&error) => {
            let path = applications.join(FALLBACK_LOCK_FILE);
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&path)
                .map_err(io_error)?;
            match try_lock(file.as_fd()) {
                Ok(true) => {}
                Ok(false) => waited(wait_for(&deadline, || try_lock(file.as_fd())))?,
                Err(error) if lock_unsupported(&error) => {
                    return Err(InstallLockError::Unsupported(applications.to_path_buf()));
                }
                Err(error) => return Err(io_error(error.into())),
            }
            Some(file)
        }
        Err(error) => return Err(io_error(error.into())),
    };
    Ok(InstallationLock {
        applications: folder,
        _fallback: fallback,
        path: applications.to_path_buf(),
    })
}

/// Retry `attempt` until it succeeds or `deadline` passes.
fn wait_for(
    deadline: &Instant,
    mut attempt: impl FnMut() -> Result<bool, rustix::io::Errno>,
) -> Result<(), WaitError> {
    loop {
        if Instant::now() >= *deadline {
            return Err(WaitError::Busy);
        }
        std::thread::sleep(LOCK_RETRY);
        if attempt().map_err(WaitError::Errno)? {
            return Ok(());
        }
    }
}

/// [`wait_for`]'s two ways out, so a busy lock can surface as
/// [`InstallLockError::Busy`] while every other error keeps its errno.
enum WaitError {
    Busy,
    Errno(rustix::io::Errno),
}

// ---------------------------------------------------------------------------
// The old installers' lock folder
// ---------------------------------------------------------------------------

/// Why the old installers' lock folder could not be taken.
#[derive(Debug, thiserror::Error)]
pub enum CompatLockError {
    /// An older installer or uninstaller holds it, or one was interrupted
    /// while holding it. Older tools' own advice (remove the folder by hand
    /// once nothing is running) applies.
    #[error(
        "an older Farhelm install or uninstall is changing the app right now, or one was \
         interrupted while doing so; wait a moment and try again, and if this persists and \
         nothing is installing, remove {} by hand", .0.display()
    )]
    HeldByOlderTool(PathBuf),
    #[error("{what} {}: {error}", path.display())]
    Io {
        what: &'static str,
        path: PathBuf,
        error: io::Error,
    },
}

/// The old installers' lock folder, held while this value lives and removed
/// (renamed away, then deleted) on [`CompatLockFolder::release`] or drop.
#[derive(Debug)]
pub struct CompatLockFolder<'lock> {
    applications: BorrowedFd<'lock>,
    path: PathBuf,
    released: bool,
}

/// Take [`COMPAT_LOCK_FOLDER`] while holding `lock`.
pub fn take_compat_lock_folder(
    lock: &InstallationLock,
) -> Result<CompatLockFolder<'_>, CompatLockError> {
    let applications = lock.applications();
    let path = lock.applications_path().join(COMPAT_LOCK_FOLDER);
    let io_error = |what: &'static str, error: io::Error| CompatLockError::Io {
        what,
        path: path.clone(),
        error,
    };
    // A leftover: marked means a newer tool died holding it (the kernel lock
    // we hold proves no newer tool is inside), unmarked means an older tool.
    match is_marked(applications, COMPAT_LOCK_FOLDER) {
        Ok(Some(true)) => remove_folder(applications, COMPAT_LOCK_FOLDER)
            .map_err(|error| io_error("removing the leftover lock folder", error))?,
        Ok(Some(false)) => return Err(CompatLockError::HeldByOlderTool(path)),
        Ok(None) => {}
        Err(error) => return Err(io_error("inspecting", error)),
    }
    let temporary = format!("{COMPAT_LOCK_FOLDER}.new-{}", uuid::Uuid::new_v4().simple());
    rustix::fs::mkdirat(applications, temporary.as_str(), rustix::fs::Mode::RWXU)
        .map_err(|error| io_error("creating the lock folder", error.into()))?;
    // A volume without extended attributes leaves the folder unmarked: it
    // then behaves like an older tool's folder if this run dies, which only
    // costs the old manual advice, never a wrong removal.
    let _ = mark(applications, &temporary);
    match rustix::fs::renameat_with(
        applications,
        temporary.as_str(),
        applications,
        COMPAT_LOCK_FOLDER,
        rustix::fs::RenameFlags::NOREPLACE,
    ) {
        Ok(()) => {}
        Err(error) if error == rustix::io::Errno::EXIST => {
            let _ = rustix::fs::unlinkat(
                applications,
                temporary.as_str(),
                rustix::fs::AtFlags::REMOVEDIR,
            );
            return Err(CompatLockError::HeldByOlderTool(path));
        }
        Err(error) => {
            let _ = rustix::fs::unlinkat(
                applications,
                temporary.as_str(),
                rustix::fs::AtFlags::REMOVEDIR,
            );
            return Err(io_error("placing the lock folder", error.into()));
        }
    }
    Ok(CompatLockFolder {
        applications,
        path,
        released: false,
    })
}

impl CompatLockFolder<'_> {
    /// Remove the folder: rename it away, then delete it, so an older tool
    /// never sees a half-removed folder under the real name.
    pub fn release(mut self) -> Result<(), CompatLockError> {
        self.released = true;
        remove_folder(self.applications, COMPAT_LOCK_FOLDER).map_err(|error| CompatLockError::Io {
            what: "removing the lock folder",
            path: self.path.clone(),
            error,
        })
    }
}

impl Drop for CompatLockFolder<'_> {
    fn drop(&mut self) {
        if !self.released {
            let _ = remove_folder(self.applications, COMPAT_LOCK_FOLDER);
        }
    }
}

/// Whether the entry `name` in `dir` exists and carries the mark: `None`
/// when absent, `Some(false)` when present without it (or not a folder,
/// which no tool of ours makes).
fn is_marked(dir: BorrowedFd<'_>, name: &str) -> io::Result<Option<bool>> {
    use rustix::fs::{Mode, OFlags};
    let folder = match rustix::fs::openat(
        dir,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(folder) => folder,
        Err(error) if error == rustix::io::Errno::NOENT => return Ok(None),
        Err(error) if error == rustix::io::Errno::NOTDIR || error == rustix::io::Errno::LOOP => {
            return Ok(Some(false));
        }
        Err(error) => return Err(error.into()),
    };
    let mut value = [0u8; 8];
    match rustix::fs::fgetxattr(&folder, COMPAT_MARK, &mut value[..]) {
        Ok(_) => Ok(Some(true)),
        Err(_) => Ok(Some(false)),
    }
}

/// Mark the folder `name` in `dir`.
fn mark(dir: BorrowedFd<'_>, name: &str) -> io::Result<()> {
    use rustix::fs::{Mode, OFlags};
    let folder = rustix::fs::openat(
        dir,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    rustix::fs::fsetxattr(&folder, COMPAT_MARK, b"1", rustix::fs::XattrFlags::empty())?;
    Ok(())
}

/// Rename the folder `name` in `dir` away to a unique name, then delete it.
fn remove_folder(dir: BorrowedFd<'_>, name: &str) -> io::Result<()> {
    let away = format!("{name}.gone-{}", uuid::Uuid::new_v4().simple());
    match rustix::fs::renameat(dir, name, dir, away.as_str()) {
        Ok(()) => {}
        Err(error) if error == rustix::io::Errno::NOENT => return Ok(()),
        Err(error) => return Err(error.into()),
    }
    rustix::fs::unlinkat(dir, away.as_str(), rustix::fs::AtFlags::REMOVEDIR)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// The ownership record
// ---------------------------------------------------------------------------

/// Where an installation stands, written with each generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallState {
    /// Everything the last install published is on disk.
    Complete,
    /// An install is running, or was interrupted.
    Installing,
    /// An uninstall is running, or was interrupted.
    Removing,
}

impl InstallState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Installing => "installing",
            Self::Removing => "removing",
        }
    }

    fn parse(text: &[u8]) -> Option<Self> {
        match text {
            b"complete" => Some(Self::Complete),
            b"installing" => Some(Self::Installing),
            b"removing" => Some(Self::Removing),
            _ => None,
        }
    }
}

/// The ownership record, `Contents/.farhelm-installation`.
///
/// `id` is `None` for the two-field record older installers write, which
/// means "complete, no id" (generation 0). An id is minted by a fresh
/// install and kept by every later install; a rebuild of the app carries it
/// over. The generation is odd exactly while an install or uninstall is
/// running or was interrupted ([`InstallState::Installing`] or
/// [`InstallState::Removing`]) and even when complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnershipRecord {
    /// The Terminal link this installation owns, as bytes (it is a path).
    pub link: Vec<u8>,
    pub id: Option<String>,
    pub generation: u64,
    pub state: InstallState,
}

/// Records are a few hundred bytes; anything far larger is not one.
pub const RECORD_LIMIT: usize = 64 * 1024;

impl OwnershipRecord {
    /// Read a record's bytes. Accepts every record older installers accept
    /// (format name, then an absolute link path ending in `/farhelm`) with
    /// either two fields or the three appended ones. Anything else is not a
    /// record of this layout.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > RECORD_LIMIT || !bytes.ends_with(&[0]) {
            return None;
        }
        let fields: Vec<&[u8]> = bytes[..bytes.len() - 1].split(|byte| *byte == 0).collect();
        let (magic, link) = (fields.first()?, fields.get(1)?);
        if *magic != RECORD_MAGIC.as_bytes()
            || !link.starts_with(b"/")
            || !link.ends_with(b"/farhelm")
        {
            return None;
        }
        let (id, generation, state) = match fields.len() {
            2 => (None, 0, InstallState::Complete),
            5 => {
                let id = std::str::from_utf8(fields[2]).ok()?;
                if id.is_empty() || !id.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
                    return None;
                }
                let generation: u64 = std::str::from_utf8(fields[3]).ok()?.parse().ok()?;
                let state = InstallState::parse(fields[4])?;
                if (state == InstallState::Complete) != generation.is_multiple_of(2) {
                    return None;
                }
                (Some(id.to_string()), generation, state)
            }
            _ => return None,
        };
        Some(Self {
            link: link.to_vec(),
            id,
            generation,
            state,
        })
    }

    /// The record's bytes, always in the extended form.
    pub fn render(&self) -> Vec<u8> {
        let id = self.id.as_deref().unwrap_or_default();
        let mut bytes = Vec::new();
        for field in [
            RECORD_MAGIC.as_bytes(),
            &self.link,
            id.as_bytes(),
            self.generation.to_string().as_bytes(),
            self.state.as_str().as_bytes(),
        ] {
            bytes.extend_from_slice(field);
            bytes.push(0);
        }
        bytes
    }

    /// A fresh installation's record: a new id, and the first "installing"
    /// generation.
    pub fn fresh(link: &Path) -> Self {
        Self {
            link: link.as_os_str().as_bytes().to_vec(),
            id: Some(new_installation_id()),
            generation: 1,
            state: InstallState::Installing,
        }
    }

    /// The record moved to the next generation in `state`: the next odd one
    /// for [`InstallState::Installing`] or [`InstallState::Removing`], the
    /// next even one for [`InstallState::Complete`]. A record without an id
    /// gets one here, after any comparison against an expectation.
    pub fn advanced(&self, state: InstallState) -> Self {
        let wants_odd = state != InstallState::Complete;
        let mut generation = self.generation + 1;
        if (generation % 2 == 1) != wants_odd {
            generation += 1;
        }
        Self {
            link: self.link.clone(),
            id: Some(self.id.clone().unwrap_or_else(new_installation_id)),
            generation,
            state,
        }
    }

    /// The link as a path.
    pub fn link_path(&self) -> &Path {
        Path::new(OsStr::from_bytes(&self.link))
    }
}

/// A random installation id: 32 lowercase hex digits.
pub fn new_installation_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two-field record every older installer writes reads as
    /// "complete, no id", and so does any absolute link to a `farhelm` (the
    /// "moved here" case older installers accept). Older uninstallers'
    /// acceptance is the compatibility surface here: whatever they wrote,
    /// L must read.
    #[farhelm_testtrace::test]
    fn a_two_field_record_reads_as_complete_without_an_id() {
        let record = OwnershipRecord::parse(b"farhelm-app-v2\0/Users/u/.local/bin/farhelm\0")
            .expect("today's record");
        assert_eq!(record.id, None);
        assert_eq!(record.generation, 0);
        assert_eq!(record.state, InstallState::Complete);
        assert_eq!(record.link_path(), Path::new("/Users/u/.local/bin/farhelm"));
        assert!(OwnershipRecord::parse(b"farhelm-app-v2\0/elsewhere/bin/farhelm\0").is_some());
    }

    /// The extended record keeps the first two fields exactly as older
    /// installers write them (they recognize a record by those), and reads
    /// back what was written.
    #[farhelm_testtrace::test]
    fn the_extended_record_round_trips_and_keeps_the_old_prefix() {
        let record = OwnershipRecord {
            link: b"/Users/u/.local/bin/farhelm".to_vec(),
            id: Some("0123abcd".to_string()),
            generation: 3,
            state: InstallState::Installing,
        };
        let bytes = record.render();
        assert!(bytes.starts_with(b"farhelm-app-v2\0/Users/u/.local/bin/farhelm\0"));
        assert_eq!(
            bytes,
            b"farhelm-app-v2\0/Users/u/.local/bin/farhelm\x000123abcd\x003\0installing\0"
        );
        assert_eq!(OwnershipRecord::parse(&bytes), Some(record));
    }

    /// Anything that is not one of the two shapes is not a record of this
    /// layout: a wrong format name, a relative or non-`farhelm` link, a
    /// missing terminator, three or four fields, an unknown state, a state
    /// that disagrees with the generation's parity, or an id that is not
    /// plain ASCII letters and digits.
    #[farhelm_testtrace::test]
    fn malformed_records_are_refused() {
        for bytes in [
            &b"farhelm-app-v1\0/u/.local/bin/farhelm\0"[..],
            b"farhelm-app-v2\0relative/farhelm\0",
            b"farhelm-app-v2\0/u/.local/bin/other\0",
            b"farhelm-app-v2\0/u/.local/bin/farhelm",
            b"farhelm-app-v2\0/u/.local/bin/farhelm\0abc\0",
            b"farhelm-app-v2\0/u/.local/bin/farhelm\0abc\x002\0",
            b"farhelm-app-v2\0/u/.local/bin/farhelm\0abc\x002\0paused\0",
            b"farhelm-app-v2\0/u/.local/bin/farhelm\0abc\x002\0installing\0",
            b"farhelm-app-v2\0/u/.local/bin/farhelm\0abc\x001\0complete\0",
            b"farhelm-app-v2\0/u/.local/bin/farhelm\0a-b\x002\0complete\0",
        ] {
            assert_eq!(
                OwnershipRecord::parse(bytes),
                None,
                "{:?}",
                String::from_utf8_lossy(bytes)
            );
        }
    }

    /// Generations move in the design's pattern: odd while in progress,
    /// even once complete, always forward; a record without an id gets one
    /// when it first advances (never before, so an updater's "no id"
    /// expectation still matches the record it read).
    #[farhelm_testtrace::test]
    fn generations_advance_by_parity_and_mint_an_id_once() {
        let old = OwnershipRecord::parse(b"farhelm-app-v2\0/u/.local/bin/farhelm\0").unwrap();
        let installing = old.advanced(InstallState::Installing);
        assert_eq!(installing.generation, 1);
        let id = installing.id.clone().expect("minted on the first advance");
        let complete = installing.advanced(InstallState::Complete);
        assert_eq!(
            (complete.generation, complete.id.as_deref()),
            (2, Some(id.as_str()))
        );
        let removing = complete.advanced(InstallState::Removing);
        assert_eq!(removing.generation, 3);
        // An interrupted install finished by another run: still forward,
        // still odd.
        assert_eq!(installing.advanced(InstallState::Installing).generation, 3);
        assert_eq!(
            OwnershipRecord::fresh(Path::new("/u/.local/bin/farhelm")).generation,
            1
        );
    }

    /// The installation lock excludes a second holder, and is free again as
    /// soon as the first is dropped: there is nothing to leave behind.
    #[farhelm_testtrace::test]
    fn the_installation_lock_excludes_a_second_holder_until_dropped() {
        let home = tempfile::tempdir().expect("temp dir");
        let applications = home.path().join("Applications");
        let first = lock_installation(&applications, Duration::ZERO).expect("first holder");
        assert!(applications.is_dir(), "the folder is created if missing");
        assert!(matches!(
            lock_installation(&applications, Duration::from_millis(120)),
            Err(InstallLockError::Busy)
        ));
        drop(first);
        lock_installation(&applications, Duration::ZERO).expect("free after the holder is gone");
        assert!(
            !applications.join(FALLBACK_LOCK_FILE).exists(),
            "a lockable folder needs no lock file"
        );
    }

    /// The old installers' folder: taken and released by renaming, so it
    /// exists exactly while held; an unmarked folder (an older tool's)
    /// refuses with the old manual advice; a marked leftover (a newer tool
    /// that died) is cleared under the kernel lock.
    #[farhelm_testtrace::test]
    fn the_compat_lock_folder_follows_the_marking_rules() {
        let home = tempfile::tempdir().expect("temp dir");
        let applications = home.path().join("Applications");
        let lock = lock_installation(&applications, Duration::ZERO).expect("kernel lock");
        let folder_path = applications.join(COMPAT_LOCK_FOLDER);

        let held = take_compat_lock_folder(&lock).expect("take the folder");
        assert!(folder_path.is_dir());
        held.release().expect("release");
        assert!(!folder_path.exists());

        std::fs::create_dir(&folder_path).expect("an older tool's folder");
        assert!(matches!(
            take_compat_lock_folder(&lock),
            Err(CompatLockError::HeldByOlderTool(_))
        ));
        std::fs::remove_dir(&folder_path).expect("the old advice: remove it by hand");

        let held = take_compat_lock_folder(&lock).expect("take the folder");
        std::mem::forget(held);
        let marked = is_marked(lock.applications(), COMPAT_LOCK_FOLDER).expect("inspect");
        if marked == Some(true) {
            let again = take_compat_lock_folder(&lock).expect("a marked leftover is cleared");
            drop(again);
            assert!(!folder_path.exists(), "dropping releases it");
        }
    }
}
