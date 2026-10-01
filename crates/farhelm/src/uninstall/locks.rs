//! The locks uninstall holds while it removes an installation, so an install,
//! update, setup, or desktop start cannot interleave with the removal.
//!
//! SPEC.md ("Concurrent and interrupted runs") requires a correct outcome
//! when uninstall overlaps those operations, and refusing is acceptable.
//! Uninstall therefore takes the locks the other operations already use,
//! never waits for any of them, and holds them until it finishes: a lock
//! that is already taken refuses the uninstall with nothing removed, and an
//! operation that starts while uninstall holds its lock refuses in its own
//! existing way. Nothing here changes how the other operations lock.
//!
//! The locks are taken after the user confirms, not before, and the caller
//! then re-checks what it is about to remove: two of them are directories,
//! which nothing removes when a process dies, so holding them across the
//! confirmation prompt would turn an ordinary Ctrl-C there into stale
//! locks. The flock-based ones (setup's, and the supervisor's and helm's)
//! are released by the kernel however the process ends.
//!
//! Which locks, and why each is enough:
//! - [`InstallLock`]: the installer's lock directory in the install
//!   directory. An installer finding it held by a live process refuses; one
//!   finding it left by a crashed uninstall clears it as stale.
//! - [`BundleLock`] (macOS): the installer's lock beside
//!   `~/Applications/Farhelm.app`, held while it rebuilds the app.
//! - setup's unit-directory flock, taken through
//!   `setup::lock_for_selected_services` (Linux).
//! - [`RuntimeLocks`] (macOS): the supervisor's and helm's state-directory
//!   flocks. The desktop app's managed supervisor and embedded helm hold
//!   them, so an open app refuses the uninstall, and an app started during
//!   it cannot serve, which also rules out creating a session. On Linux
//!   uninstall stops setup's services itself, after which only a process
//!   the user started by hand could hold these locks, and stopping those is
//!   already the operator's job (SPEC.md, "Operator prerequisites").
//!
//! A refusal names the lock and what holding it usually means, never as
//! proof that a process is running: a lock directory can be left by a
//! crash (SPEC.md forbids describing a file's existence as such proof).

use anyhow::{Context as _, Result, bail};
use std::{
    fs,
    io::ErrorKind,
    os::unix::fs::DirBuilderExt as _,
    path::{Path, PathBuf},
};

/// The installer's lock directory name inside the install directory
/// (`LOCK_DIR` in scripts/install.sh).
const INSTALL_LOCK: &str = ".farhelm-install.lock";
/// The installer's app-bundle lock beside `Farhelm.app` (scripts/install.sh).
const BUNDLE_LOCK: &str = ".farhelm-app.lock";
/// The supervisor's state-directory lock (`supervisor.lock`).
const SUPERVISOR_LOCK: &str = "supervisor.lock";
/// The helm's state-directory lock (`helm-token.lock`).
const HELM_LOCK: &str = "helm-token.lock";

/// The installer's lock on the install directory, held by this uninstall.
///
/// Created the way the installer creates it (`mkdir`, mode 0700, then a
/// `pid` file with this process's pid), so the installer's own checks read
/// it as one of its own: held while this process lives, stale and
/// recoverable once it has died.
pub(super) struct InstallLock {
    dir: PathBuf,
}

impl InstallLock {
    /// Take the lock in `install_root`, or refuse if it exists.
    pub(super) fn acquire(install_root: &Path) -> Result<Self> {
        let dir = install_root.join(INSTALL_LOCK);
        match fs::DirBuilder::new().mode(0o700).create(&dir) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                let recorded = fs::read_to_string(dir.join("pid"))
                    .map(|pid| format!(" (it records pid {})", pid.trim()))
                    .unwrap_or_default();
                // A journal means an installer stopped partway through
                // replacing files, and only the installer can restore the
                // previous installation from it. Without one (an install or
                // uninstall interrupted before or after its changes, this
                // command's own Ctrl-C included), the lock is only a marker
                // the user can clear once its process is gone, without a
                // reinstall.
                let advice = if dir.join("journal").exists() {
                    "an install or update may be running, or one was interrupted partway; wait for it \
                     to finish, or re-run the installer, which restores the previous installation \
                     from the interrupted run's journal, then retry uninstall"
                        .to_string()
                } else {
                    format!(
                        "an install, update or uninstall may be running, or one was interrupted; once \
                         the recorded process is not running, remove the lock with `rm -f {pid} && \
                         rmdir {dir}` (or re-run the installer, which clears it), then retry \
                         uninstall",
                        pid = super::path_text(&dir.join("pid")),
                        dir = super::path_text(&dir),
                    )
                };
                bail!(
                    "the install lock {} exists{recorded}: {advice}",
                    super::path_text(&dir)
                );
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("creating install lock {}", super::path_text(&dir)));
            }
        }
        let lock = Self { dir };
        fs::write(lock.dir.join("pid"), format!("{}\n", std::process::id())).with_context(
            || format!("recording this process in {}", super::path_text(&lock.dir)),
        )?;
        Ok(lock)
    }
}

impl Drop for InstallLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.dir.join("pid"));
        let _ = fs::remove_dir(&self.dir);
    }
}

/// The installer's lock beside the macOS app bundle, held by this uninstall.
pub(super) struct BundleLock {
    dir: PathBuf,
}

impl BundleLock {
    /// Take the lock in `applications` (the bundle's parent directory), or
    /// refuse if it exists. `None` when that directory does not exist: the
    /// confirmed and re-checked plan then has no bundle and no leftover
    /// receipt, so uninstall touches nothing there, whatever an installer
    /// (possibly for another install directory) builds in the meantime.
    pub(super) fn acquire(applications: &Path) -> Result<Option<Self>> {
        let dir = applications.join(BUNDLE_LOCK);
        match fs::DirBuilder::new().mode(0o700).create(&dir) {
            Ok(()) => Ok(Some(Self { dir })),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => bail!(
                "the app lock {} exists: the installer may be building Farhelm.app right now, or an \
                 earlier run was interrupted; if no installer is running, remove {} by hand, then \
                 retry uninstall",
                super::path_text(&dir),
                super::path_text(&dir)
            ),
            Err(error) => {
                Err(error).with_context(|| format!("creating app lock {}", super::path_text(&dir)))
            }
        }
    }
}

impl Drop for BundleLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.dir);
    }
}

/// The supervisor's and helm's state-directory flocks, held by this
/// uninstall. Released by the kernel when the files close.
///
/// A lock file that does not exist yet is created (mode 0600, the helm's
/// own mode for its lock) so a supervisor or helm starting meanwhile finds
/// it held; that leaves an empty lock file in a data directory uninstall
/// otherwise only retains.
pub(super) struct RuntimeLocks {
    _files: Vec<fs::File>,
}

impl RuntimeLocks {
    /// Take both locks in each of `state_dirs`, or refuse if any is held.
    /// Nothing is taken (and nothing created) in a state directory that
    /// does not exist: nothing has run there.
    ///
    /// The directories are resolved first and duplicates dropped: one
    /// directory spelled two ways (a symlinked `~/.local/state`, macOS's
    /// `/var` alias) would otherwise be locked twice, and a second flock on
    /// a new descriptor conflicts with this process's own first one.
    pub(super) fn acquire(state_dirs: &[PathBuf]) -> Result<Self> {
        let mut resolved: Vec<PathBuf> = Vec::new();
        for state_dir in state_dirs {
            let Ok(physical) = fs::canonicalize(state_dir) else {
                continue;
            };
            if !resolved.contains(&physical) {
                resolved.push(physical);
            }
        }
        let mut files = Vec::new();
        for state_dir in &resolved {
            files.extend(Self::acquire_one(state_dir)?);
        }
        Ok(Self { _files: files })
    }

    fn acquire_one(state_dir: &Path) -> Result<Vec<fs::File>> {
        use std::os::unix::fs::OpenOptionsExt as _;
        let mut files = Vec::new();
        if !state_dir.is_dir() {
            return Ok(files);
        }
        for name in [SUPERVISOR_LOCK, HELM_LOCK] {
            let path = state_dir.join(name);
            let file = fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .mode(0o600)
                .open(&path)
                .with_context(|| format!("opening {}", super::path_text(&path)))?;
            match file.try_lock() {
                Ok(()) => files.push(file),
                Err(fs::TryLockError::WouldBlock) => bail!(
                    "{} is held by another process (usually a Farhelm supervisor or helm using {}, \
                     such as the desktop app's); quit Farhelm Desktop and stop any Farhelm processes \
                     you started, then retry uninstall",
                    super::path_text(&path),
                    super::path_text(state_dir)
                ),
                Err(fs::TryLockError::Error(error)) => {
                    return Err(error)
                        .with_context(|| format!("locking {}", super::path_text(&path)));
                }
            }
        }
        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    /// The install lock uninstall takes is shaped exactly like the
    /// installer's own, refuses when one exists, and is gone afterwards.
    ///
    /// Why: the installer recognizes its lock by shape (only a `pid` file,
    /// owner-only directory) and treats a live pid as "another install is
    /// running" and a dead one as stale and recoverable; a lock of any other
    /// shape it refuses to touch. Uninstall's lock has to read as one of the
    /// installer's for an installer started meanwhile to refuse, and for a
    /// crashed uninstall's lock to be cleared by the next install. Spec: the
    /// lock is a 0700 directory holding only `pid` with this process's id; a
    /// second acquire refuses naming the lock; drop removes it.
    #[test]
    fn install_lock_matches_the_installers_shape_and_refuses_when_held() {
        let root = tempfile::tempdir().expect("install root");
        let lock = InstallLock::acquire(root.path()).expect("free lock");
        let dir = root.path().join(INSTALL_LOCK);
        let mode = fs::metadata(&dir).expect("lock dir").permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
        let entries: Vec<_> = fs::read_dir(&dir)
            .expect("lock entries")
            .map(|entry| entry.expect("entry").file_name())
            .collect();
        assert_eq!(entries, vec![std::ffi::OsString::from("pid")]);
        assert_eq!(
            fs::read_to_string(dir.join("pid")).expect("pid"),
            format!("{}\n", std::process::id())
        );
        let refusal = InstallLock::acquire(root.path())
            .err()
            .expect("a held lock refuses")
            .to_string();
        assert!(refusal.contains(".farhelm-install.lock") && refusal.contains("may be running"));
        drop(lock);
        assert!(!dir.exists(), "the lock is released");
    }

    /// The app lock refuses when held, is released on drop, and is skipped
    /// when there is no directory to hold an app.
    ///
    /// Why: creating `~/Applications` just to lock it would leave a stray
    /// directory on a machine that never had the app; and a held lock means
    /// the installer may be rebuilding the app right now. Spec: no parent
    /// gives `None` and creates nothing; a free lock is taken and released;
    /// an existing one refuses naming it.
    #[test]
    fn bundle_lock_refuses_when_held_and_skips_a_missing_parent() {
        let home = tempfile::tempdir().expect("home");
        let applications = home.path().join("Applications");
        assert!(
            BundleLock::acquire(&applications)
                .expect("no parent")
                .is_none()
        );
        assert!(!applications.exists(), "nothing was created");
        fs::create_dir(&applications).expect("applications");
        let lock = BundleLock::acquire(&applications)
            .expect("free lock")
            .expect("taken");
        let refusal = BundleLock::acquire(&applications)
            .err()
            .expect("a held lock refuses")
            .to_string();
        assert!(refusal.contains(".farhelm-app.lock"), "{refusal}");
        drop(lock);
        assert!(!applications.join(BUNDLE_LOCK).exists());
    }

    /// A supervisor or helm holding its state-directory lock refuses the
    /// uninstall; a state directory that does not exist is left alone.
    ///
    /// Why: on macOS the open desktop app's supervisor and helm hold these
    /// locks, and removing the app underneath them is the overlap SPEC.md's
    /// "Concurrent and interrupted runs" requires a correct outcome for;
    /// refusing is that outcome. Spec: with another descriptor holding
    /// either lock, acquire refuses naming it; with both free, both are
    /// taken and released on drop; a missing state directory takes nothing
    /// and creates nothing.
    #[test]
    fn runtime_locks_refuse_while_held_and_skip_a_missing_state_dir() {
        let base = tempfile::tempdir().expect("base");
        let state = base.path().join("state");
        RuntimeLocks::acquire(std::slice::from_ref(&state)).expect("no state dir");
        assert!(!state.exists(), "nothing was created");
        fs::create_dir(&state).expect("state");
        for name in [SUPERVISOR_LOCK, HELM_LOCK] {
            let holder = fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .open(state.join(name))
                .expect("holder");
            holder.try_lock().expect("the holder takes the lock");
            let refusal = RuntimeLocks::acquire(std::slice::from_ref(&state))
                .err()
                .expect("a held lock refuses")
                .to_string();
            assert!(
                refusal.contains(name) && refusal.contains("quit Farhelm Desktop"),
                "{refusal}"
            );
            drop(holder);
        }
        let held = RuntimeLocks::acquire(std::slice::from_ref(&state)).expect("both free");
        let probe = fs::OpenOptions::new()
            .write(true)
            .open(state.join(SUPERVISOR_LOCK))
            .expect("probe");
        assert!(probe.try_lock().is_err(), "uninstall holds the lock");
        drop(held);
        probe.try_lock().expect("released on drop");
    }

    /// One state directory reached through two spellings is locked once.
    ///
    /// Why: uninstall checks both the `XDG_STATE_HOME` state directory and
    /// the default one, and when those are the same directory (a symlinked
    /// `~/.local/state`, say) locking it twice made uninstall refuse
    /// against its own first lock on every retry. Spec: with one entry a
    /// symlink to the other, acquire succeeds and holds the lock.
    #[test]
    fn runtime_locks_take_one_directory_spelled_two_ways_once() {
        let base = tempfile::tempdir().expect("base");
        let state = base.path().join("state");
        fs::create_dir(&state).expect("state");
        let alias = base.path().join("alias");
        std::os::unix::fs::symlink(&state, &alias).expect("alias");
        let held =
            RuntimeLocks::acquire(&[state.clone(), alias]).expect("one directory, locked once");
        let probe = fs::OpenOptions::new()
            .write(true)
            .open(state.join(SUPERVISOR_LOCK))
            .expect("probe");
        assert!(probe.try_lock().is_err(), "uninstall holds the lock");
        drop(held);
    }
}
