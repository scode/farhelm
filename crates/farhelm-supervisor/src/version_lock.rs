//! Version locks: how a running Farhelm keeps its own version's folder from
//! being deleted under it (SPEC_impl.md, "Side-by-side versions inside
//! Farhelm.app", and the race-free installer design it implements).
//!
//! Every version kept in the Mac app bundle is a folder,
//! `Contents/Versions/<v>/`, holding exactly one program, `farhelm`. Anything
//! that runs that program, or may still start it, holds a shared `flock` on
//! the program file for as long as it does. Whatever deletes a version (an
//! install pruning old ones, uninstall) first takes an exclusive lock on the
//! same file without waiting; while anyone holds the version that fails, and
//! the version stays. The kernel drops a lock when the last descriptor
//! sharing it closes, including when its holder is killed, so no lock can be
//! left behind.
//!
//! A deletion renames the version's folder out of the app while it holds the
//! exclusive lock. So a would-be user that opened the file just before the
//! rename and only then got its shared lock holds a file that is no longer at
//! its path. Every acquisition therefore checks, after locking, that the
//! locked file is still the one at the path (same device and inode); if not,
//! the version is gone and the caller recovers instead of running a version
//! that is about to disappear.
//!
//! A process holds one lock on its version, through one open file: two
//! separate opens of the same file are two independent `flock` holders even
//! in one process, which would stop uninstall from ever converting its own
//! shared lock to an exclusive one. A program started by the forwarder
//! therefore adopts the descriptor the forwarder locked for it (named in
//! [`LOCK_FD_ENV`]) instead of opening its own, after checking that the
//! descriptor really is its own program file.
//!
//! Only programs running from the app bundle's version layout take part
//! (recognized by [`crate::app_bundle::versioned_program`]); Linux hosts,
//! development builds and every other `farhelm` behave exactly as before.

use std::ffi::OsStr;
use std::fs::{File, TryLockError};
use std::io;
use std::os::fd::{AsFd, BorrowedFd, FromRawFd, RawFd};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Environment variable naming the descriptor a launcher already locked for
/// the program it starts (the forwarder sets it to `8`).
///
/// A program that finds it adopts that descriptor as its lock and removes
/// the variable before starting anything, so it never reaches tmux,
/// sessions or agents. A value that does not name the program's own file is
/// ignored: the variable can leak through an environment it was never meant
/// for, and adopting an unrelated descriptor would leave the program
/// unprotected.
pub const LOCK_FD_ENV: &str = "FARHELM_VERSION_LOCK_FD";

/// How long a shared acquisition waits for an exclusive holder to finish.
///
/// Exclusive locks are only ever held for a moment by pruning (one rename)
/// or for the duration of an uninstall. A bounded wait keeps a stopped
/// uninstall or installer from hanging the app or an agent's hook; past it
/// the caller reports that Farhelm is being updated or uninstalled.
pub const SHARED_WAIT: Duration = Duration::from_secs(5);

/// How often a waiting shared acquisition retries.
const RETRY_INTERVAL: Duration = Duration::from_millis(25);

/// Exit status of `farhelm internal version-lock` when it holds the version.
pub const HELPER_HELD: i32 = 0;
/// Exit status of `farhelm internal version-lock` when the version is gone
/// or stayed exclusively held past [`SHARED_WAIT`]: the forwarder picks a
/// version again. Chosen to differ from 2 (an older `farhelm` rejecting the
/// unknown subcommand) and from 126/127 (the shell could not run the file).
pub const HELPER_RETRY: i32 = 75;
/// Exit status of `farhelm internal version-lock` when the filesystem does
/// not support the lock: the forwarder starts the version without one.
pub const HELPER_UNSUPPORTED: i32 = 69;

/// The outcome of taking a shared lock and checking it.
#[derive(Debug)]
pub enum Shared {
    /// The lock is held and the locked file is still the one at the path.
    Held,
    /// The file at the path is not the locked one any more (or there is
    /// none): the version was deleted between the open and the lock.
    Gone,
    /// An exclusive holder kept the lock past the wait.
    Busy,
    /// The filesystem does not support `flock`.
    Unsupported(io::Error),
}

/// Take a shared lock on `file`, waiting up to `wait` for an exclusive
/// holder, then check that `file` is still the file at `path`.
///
/// Locking the same open file again is a no-op, so this is also how an
/// adopted descriptor is confirmed. Never unlocks: a descriptor this
/// process shares with another (an inherited one) shares its lock, and an
/// explicit unlock would drop it for both.
pub fn lock_shared_checked(file: &File, path: &Path, wait: Duration) -> io::Result<Shared> {
    let deadline = Instant::now() + wait;
    loop {
        match file.try_lock_shared() {
            Ok(()) => break,
            Err(TryLockError::WouldBlock) => {
                if Instant::now() >= deadline {
                    return Ok(Shared::Busy);
                }
                std::thread::sleep(RETRY_INTERVAL);
            }
            Err(TryLockError::Error(error)) if is_unsupported(&error) => {
                return Ok(Shared::Unsupported(error));
            }
            Err(TryLockError::Error(error)) => return Err(error),
        }
    }
    if same_file(file.as_fd(), path)? {
        Ok(Shared::Held)
    } else {
        Ok(Shared::Gone)
    }
}

/// Whether `fd` refers to the file currently at `path` (same device and
/// inode). A missing path is "no".
pub fn same_file(fd: BorrowedFd<'_>, path: &Path) -> io::Result<bool> {
    let held = rustix::fs::fstat(fd).map_err(io::Error::from)?;
    match std::fs::metadata(path) {
        Ok(at_path) => {
            Ok(held.st_dev as u64 == at_path.dev() && held.st_ino as u64 == at_path.ino())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// Whether a lock error means the filesystem cannot lock at all, as opposed
/// to a real failure.
fn is_unsupported(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::Unsupported
        || matches!(
            error.raw_os_error(),
            Some(code) if code == libc::ENOLCK || code == libc::EOPNOTSUPP || code == libc::ENOTSUP
        )
}

/// What [`hold_for_process`] did.
#[derive(Debug, PartialEq, Eq)]
pub enum ProcessHold {
    /// This program does not run from the app bundle's version layout;
    /// nothing was locked.
    NotInLayout,
    /// The process holds its version for the rest of its life.
    Held,
    /// The filesystem cannot lock; the process runs without protection,
    /// exactly as before version locks existed.
    Unsupported,
}

/// Why [`hold_for_process`] could not hold the version.
#[derive(Debug)]
pub enum HoldError {
    /// The version's folder was deleted (by an update or an uninstall)
    /// before this process could hold it.
    Gone { program: PathBuf },
    /// An update or uninstall held the version exclusively past
    /// [`SHARED_WAIT`].
    Busy { program: PathBuf },
    /// Opening or inspecting the program file failed.
    Io { program: PathBuf, error: io::Error },
}

impl std::fmt::Display for HoldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HoldError::Gone { program } => write!(
                f,
                "{} was removed by a Farhelm update or uninstall while it was starting; run the command again",
                program.display()
            ),
            HoldError::Busy { program } => write!(
                f,
                "{} is being updated or uninstalled; try again in a moment",
                program.display()
            ),
            HoldError::Io { program, error } => {
                write!(
                    f,
                    "could not lock {} for this run: {error}",
                    program.display()
                )
            }
        }
    }
}

impl std::error::Error for HoldError {}

/// The process's own version lock, kept open until the process exits.
static PROCESS_HOLD: OnceLock<File> = OnceLock::new();

/// The open file through which this process holds its version, once
/// [`hold_for_process`] has taken it. Uninstall converts this lock to an
/// exclusive one on its own version rather than opening a second holder.
pub fn process_hold() -> Option<&'static File> {
    PROCESS_HOLD.get()
}

/// Hold this process's own version for the rest of its life, if it runs
/// from the app bundle's version layout.
///
/// `exe` is the running program's path. `inherited` is the value of
/// [`LOCK_FD_ENV`] as the process found it; the caller removes the variable
/// from its environment either way. A valid inherited descriptor (one that
/// is `exe`'s own file) is adopted, marked close-on-exec so children never
/// inherit it, and confirmed; otherwise the program file is opened and
/// locked here. Either way the lock is checked against the path after it is
/// taken.
pub fn hold_for_process(exe: &Path, inherited: Option<&OsStr>) -> Result<ProcessHold, HoldError> {
    if crate::app_bundle::versioned_program(exe).is_none() {
        return Ok(ProcessHold::NotInLayout);
    }
    let io_error = |error: io::Error| HoldError::Io {
        program: exe.to_path_buf(),
        error,
    };
    let file = match inherited.and_then(|value| adoptable_fd(value, exe)) {
        Some(fd) => {
            // SAFETY: `adoptable_fd` confirmed `fd` is an open descriptor
            // for this program's own file, which nothing else in this
            // process owns: the launcher handed it over and this is the only
            // place that takes it.
            let file = unsafe { File::from_raw_fd(fd) };
            rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::CLOEXEC)
                .map_err(|error| io_error(error.into()))?;
            file
        }
        None => File::open(exe).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                HoldError::Gone {
                    program: exe.to_path_buf(),
                }
            } else {
                io_error(error)
            }
        })?,
    };
    match lock_shared_checked(&file, exe, SHARED_WAIT).map_err(io_error)? {
        Shared::Held => {
            // A second call (there is none in a real run; tests that share a
            // process can make one) keeps the first hold, and this file is
            // kept open too rather than dropped, so its lock is never
            // released while the process lives.
            if let Err(file) = PROCESS_HOLD.set(file) {
                std::mem::forget(file);
            }
            Ok(ProcessHold::Held)
        }
        Shared::Gone => Err(HoldError::Gone {
            program: exe.to_path_buf(),
        }),
        Shared::Busy => Err(HoldError::Busy {
            program: exe.to_path_buf(),
        }),
        Shared::Unsupported(_) => Ok(ProcessHold::Unsupported),
    }
}

/// The descriptor `value` names, if it is open and refers to `exe`'s own
/// file. Anything else (not a number, closed, another file) is not adopted,
/// and is left untouched: it may belong to whoever leaked the variable.
fn adoptable_fd(value: &OsStr, exe: &Path) -> Option<RawFd> {
    let fd: RawFd = value.to_str()?.parse().ok()?;
    if fd < 0 {
        return None;
    }
    // SAFETY: only used for `fstat` within this call; an fd that is not
    // open makes `fstat` fail with EBADF, which declines adoption.
    let borrowed = unsafe { BorrowedFd::borrow_raw(fd) };
    same_file(borrowed, exe)
        .ok()
        .filter(|same| *same)
        .map(|_| fd)
}

/// `farhelm internal version-lock <fd>`: lock the inherited descriptor
/// `fd` (the forwarder's claim on this very program) shared, check it, and
/// report through the exit status. Returns one of [`HELPER_HELD`],
/// [`HELPER_RETRY`] or [`HELPER_UNSUPPORTED`].
///
/// The lock belongs to the open file the forwarder shell shares with this
/// process, so it stays after this process exits; the descriptor is left
/// open here and never unlocked.
pub fn run_helper(exe: &Path, fd: RawFd) -> i32 {
    if fd < 0 {
        return HELPER_RETRY;
    }
    // SAFETY: the descriptor is inherited from the forwarder, which opened
    // it for exactly this call. `ManuallyDrop` keeps this process from
    // closing it (closing would not release the shared lock, but there is
    // no reason to touch it), and an fd that is not open makes every call
    // below fail, which reports "retry".
    let file = std::mem::ManuallyDrop::new(unsafe { File::from_raw_fd(fd) });
    match same_file(file.as_fd(), exe) {
        Ok(true) => {}
        _ => return HELPER_RETRY,
    }
    match lock_shared_checked(&file, exe, SHARED_WAIT) {
        Ok(Shared::Held) => HELPER_HELD,
        Ok(Shared::Unsupported(_)) => HELPER_UNSUPPORTED,
        Ok(Shared::Gone | Shared::Busy) | Err(_) => HELPER_RETRY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::{AsRawFd, IntoRawFd};

    /// A fake bundle with one version folder holding a `farhelm` file.
    /// Locks and identity checks only need a regular file at the right
    /// path; nothing runs it.
    fn fake_version(root: &Path, version: &str) -> PathBuf {
        let dir = root.join("Farhelm.app/Contents/Versions").join(version);
        std::fs::create_dir_all(&dir).expect("fixture: version folder");
        let program = dir.join("farhelm");
        std::fs::write(&program, b"program").expect("fixture: program file");
        program
    }

    /// The core guarantee: while any process holds a version shared, an
    /// exclusive no-wait attempt (what pruning and uninstall do) fails, and
    /// it succeeds once the holder is gone.
    #[farhelm_testtrace::test]
    fn a_shared_holder_keeps_deleters_out_until_it_lets_go() {
        let root = tempfile::tempdir().expect("tempdir");
        let program = fake_version(root.path(), "1.2.3");
        let holder = File::open(&program).expect("open");
        assert!(matches!(
            lock_shared_checked(&holder, &program, Duration::ZERO).expect("lock"),
            Shared::Held
        ));
        let deleter = File::open(&program).expect("open");
        assert!(
            matches!(deleter.try_lock(), Err(TryLockError::WouldBlock)),
            "an exclusive attempt fails while a version is held"
        );
        drop(holder);
        deleter
            .try_lock()
            .expect("the exclusive attempt succeeds once nothing holds the version");
    }

    /// A user that opened the program just before a deletion renamed its
    /// folder away, and only then got its lock, must see the version as
    /// gone rather than run it: that is how a deletion that won the race
    /// stays safe.
    #[farhelm_testtrace::test]
    fn a_lock_on_a_file_renamed_away_is_reported_gone() {
        let root = tempfile::tempdir().expect("tempdir");
        let program = fake_version(root.path(), "1.2.3");
        let opened = File::open(&program).expect("open before the deletion");
        let version_dir = program.parent().expect("version folder");
        std::fs::rename(version_dir, root.path().join("trash"))
            .expect("deletion renames the folder away");
        assert!(matches!(
            lock_shared_checked(&opened, &program, Duration::ZERO).expect("lock"),
            Shared::Gone
        ));
    }

    /// A shared acquisition waits only a bounded time for an exclusive
    /// holder, so a stopped uninstall cannot hang a hook or the app.
    #[farhelm_testtrace::test]
    fn an_exclusive_holder_makes_a_bounded_wait_report_busy() {
        let root = tempfile::tempdir().expect("tempdir");
        let program = fake_version(root.path(), "1.2.3");
        let deleter = File::open(&program).expect("open");
        deleter.try_lock().expect("exclusive");
        let user = File::open(&program).expect("open");
        let started = Instant::now();
        assert!(matches!(
            lock_shared_checked(&user, &program, Duration::from_millis(100)).expect("lock"),
            Shared::Busy
        ));
        assert!(
            started.elapsed() >= Duration::from_millis(100),
            "it waited the given time"
        );
    }

    /// Outside the version layout nothing is locked and nothing can fail:
    /// Linux hosts and development builds are untouched.
    #[farhelm_testtrace::test]
    fn programs_outside_the_layout_hold_nothing() {
        let root = tempfile::tempdir().expect("tempdir");
        let program = root.path().join("farhelm");
        assert_eq!(
            hold_for_process(&program, None).expect("no hold needed"),
            ProcessHold::NotInLayout
        );
    }

    /// The helper's claim stays after the helper is gone, because the lock
    /// belongs to the open file the forwarder shares with it. Simulated
    /// here with two descriptors for one open file: the "helper" one locks,
    /// is closed, and the "forwarder" one still keeps deleters out.
    #[farhelm_testtrace::test]
    fn the_helpers_claim_outlives_the_helper() {
        let root = tempfile::tempdir().expect("tempdir");
        let program = fake_version(root.path(), "1.2.3");
        let forwarder = File::open(&program).expect("forwarder opens the program");
        let helper_fd = rustix::io::dup(&forwarder).expect("the helper inherits a copy");
        let status = run_helper(&program, helper_fd.as_raw_fd());
        assert_eq!(status, HELPER_HELD);
        drop(helper_fd);
        let deleter = File::open(&program).expect("open");
        assert!(
            matches!(deleter.try_lock(), Err(TryLockError::WouldBlock)),
            "the shared lock is still held through the forwarder's descriptor"
        );
        drop(forwarder);
        deleter
            .try_lock()
            .expect("released once the last descriptor closes");
    }

    /// The helper reports "retry" when what the forwarder opened is no
    /// longer the program at the path, and when an exclusive holder does not
    /// let go in time; both make the forwarder pick a version again.
    #[farhelm_testtrace::test]
    fn the_helper_asks_for_a_retry_when_the_version_is_gone() {
        let root = tempfile::tempdir().expect("tempdir");
        let program = fake_version(root.path(), "1.2.3");
        let opened = File::open(&program).expect("open");
        std::fs::rename(program.parent().expect("folder"), root.path().join("trash"))
            .expect("rename away");
        assert_eq!(run_helper(&program, opened.as_raw_fd()), HELPER_RETRY);
        assert_eq!(run_helper(&program, -1), HELPER_RETRY);
    }

    /// A descriptor handed down in the variable is adopted only when it is
    /// the program's own file; a leaked or unrelated value is ignored and
    /// left alone. Adoption marks the descriptor close-on-exec.
    #[farhelm_testtrace::test]
    fn only_the_programs_own_file_is_adopted() {
        let root = tempfile::tempdir().expect("tempdir");
        let program = fake_version(root.path(), "1.2.3");
        let other = root.path().join("other");
        std::fs::write(&other, b"x").expect("fixture");
        let unrelated = File::open(&other).expect("open");
        let unrelated_fd = unrelated.as_raw_fd().to_string();
        assert_eq!(adoptable_fd(OsStr::new(&unrelated_fd), &program), None);
        assert_eq!(adoptable_fd(OsStr::new("not-a-number"), &program), None);
        assert_eq!(adoptable_fd(OsStr::new("987654"), &program), None);

        let own = File::open(&program).expect("open");
        let own_fd = own.into_raw_fd();
        assert_eq!(
            adoptable_fd(OsStr::new(&own_fd.to_string()), &program),
            Some(own_fd)
        );
        // SAFETY: test-owned descriptor, closed right here.
        drop(unsafe { File::from_raw_fd(own_fd) });
    }

    /// The forwarder's handoff end to end, as the started program sees it:
    /// a descriptor the launcher locked and left open (not close-on-exec,
    /// so it survived the exec) is adopted as the process's one lock, made
    /// close-on-exec so the program's own children never inherit it, and
    /// still keeps deleters out.
    #[farhelm_testtrace::test]
    fn the_process_adopts_a_handed_down_lock_as_its_own() {
        let root = tempfile::tempdir().expect("tempdir");
        let program = fake_version(root.path(), "1.2.3");
        let handed = File::open(&program).expect("launcher opens the program");
        handed
            .try_lock_shared()
            .expect("launcher's helper locked it");
        rustix::io::fcntl_setfd(&handed, rustix::io::FdFlags::empty())
            .expect("inheritable, as across an exec");
        let fd = handed.into_raw_fd();

        let outcome = hold_for_process(&program, Some(OsStr::new(&fd.to_string()))).expect("hold");
        assert_eq!(outcome, ProcessHold::Held);
        let held = process_hold().expect("the process keeps its hold");
        assert_eq!(
            held.as_raw_fd(),
            fd,
            "the handed-down descriptor itself, not a second open"
        );
        assert!(
            rustix::io::fcntl_getfd(held)
                .expect("flags")
                .contains(rustix::io::FdFlags::CLOEXEC),
            "children of the program never inherit its lock"
        );
        let deleter = File::open(&program).expect("open");
        assert!(matches!(deleter.try_lock(), Err(TryLockError::WouldBlock)));
    }

    /// Without a handed-down descriptor, the process opens and locks its
    /// own program file; a program whose file is already gone refuses to
    /// run rather than run a version that is being deleted.
    #[farhelm_testtrace::test]
    fn the_process_locks_its_own_file_or_refuses_when_it_is_gone() {
        let root = tempfile::tempdir().expect("tempdir");
        let program = fake_version(root.path(), "1.2.3");
        assert_eq!(
            hold_for_process(&program, None).expect("hold"),
            ProcessHold::Held
        );
        let deleter = File::open(&program).expect("open");
        assert!(matches!(deleter.try_lock(), Err(TryLockError::WouldBlock)));

        let missing = root
            .path()
            .join("Farhelm.app/Contents/Versions/9.9.9/farhelm");
        assert!(matches!(
            hold_for_process(&missing, None),
            Err(HoldError::Gone { .. })
        ));
    }
}
