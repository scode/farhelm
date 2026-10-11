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
//!   it cannot serve, which also rules out creating a session. A starting
//!   supervisor waits a while for a held lock (a Farhelm that was just quit
//!   may still be shutting down), so it can outlast the removal; it then
//!   refuses because its own program is gone, and the app still does not
//!   open. On Linux
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
                } else if let Some(path) = dir
                    .to_str()
                    .filter(|text| !text.chars().any(char::is_control))
                {
                    format!(
                        "an install, update or uninstall may be running, or one was interrupted; once \
                         the recorded process is not running, remove the lock with `rm -f {pid} && \
                         rmdir {dir}` (or re-run the installer, which clears it), then retry \
                         uninstall",
                        pid = farhelm_proto::text::shell_quote(&format!("{path}/pid")),
                        dir = farhelm_proto::text::shell_quote(path),
                    )
                } else {
                    // Diagnostic byte escapes are not shell arguments. When
                    // the path cannot be printed faithfully on one line,
                    // leave recovery manual rather than offer a command that
                    // could remove a different path or execute substitutions.
                    "an install, update or uninstall may be running, or one was interrupted; once \
                     the recorded process is not running, remove the lock directory shown above \
                     by hand (or re-run the installer, which clears it), then retry uninstall"
                        .to_string()
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
/// it held. A missing state directory is created with the supervisor's
/// private 0700 rule for the same reason; uninstall otherwise retains it.
pub(super) struct RuntimeLocks {
    _files: Vec<fs::File>,
}

impl RuntimeLocks {
    /// Take both locks in each of `state_dirs`, or refuse if any is held.
    /// A missing directory is created privately before taking its locks:
    /// otherwise a first desktop launch could create it after uninstall
    /// skipped it and start while the app is being removed.
    ///
    /// The directories are resolved first and duplicates dropped: one
    /// directory spelled two ways (a symlinked `~/.local/state`, macOS's
    /// `/var` alias) would otherwise be locked twice, and a second flock on
    /// a new descriptor conflicts with this process's own first one.
    pub(super) fn acquire(state_dirs: &[PathBuf]) -> Result<Self> {
        use std::os::unix::fs::PermissionsExt as _;

        let mut resolved: Vec<PathBuf> = Vec::new();
        for state_dir in state_dirs {
            // Match the supervisor's private mode for directories we create,
            // but leave existing paths alone: a state directory may be a
            // symlink, and chmod would change its target even on refusal.
            let parent = state_dir.parent().with_context(|| {
                format!(
                    "finding parent of state directory {}",
                    super::path_text(state_dir)
                )
            })?;
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true).mode(0o700);
            builder.create(parent).with_context(|| {
                format!(
                    "creating parent of state directory {}",
                    super::path_text(state_dir)
                )
            })?;
            match fs::DirBuilder::new().mode(0o700).create(state_dir) {
                Ok(()) => fs::set_permissions(state_dir, fs::Permissions::from_mode(0o700))
                    .with_context(|| {
                        format!("securing state directory {}", super::path_text(state_dir))
                    })?,
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!("creating state directory {}", super::path_text(state_dir))
                    });
                }
            }
            let physical = fs::canonicalize(state_dir).with_context(|| {
                format!("resolving state directory {}", super::path_text(state_dir))
            })?;
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

    /// Recovery advice must act on the literal lock, even when its parent
    /// has spaces, shell substitutions, quotes or non-ASCII text. Executing
    /// the emitted command against a stale owned fixture catches quoting
    /// that looks plausible but expands or selects another path.
    #[test]
    fn stale_install_lock_advice_is_safe_to_paste() {
        let root = tempfile::tempdir().expect("fixture root");
        let install = root.path().join("space $HOME `touch escaped` 'café'");
        let dir = install.join(INSTALL_LOCK);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("pid"), "stale-fixture\n").unwrap();
        assert!(dir.is_dir() && dir.join("pid").is_file());

        let error = InstallLock::acquire(&install)
            .err()
            .expect("stale lock refuses")
            .to_string();
        let command = error
            .split_once("remove the lock with `")
            .expect("command advice")
            .1
            .split_once("` (or re-run")
            .expect("end of command")
            .0;
        assert_eq!(
            command,
            format!(
                "rm -f {} && rmdir {}",
                farhelm_proto::text::shell_quote(dir.join("pid").to_str().unwrap()),
                farhelm_proto::text::shell_quote(dir.to_str().unwrap())
            )
        );
        let status = std::process::Command::new("sh")
            .args(["-c", command])
            .current_dir(root.path())
            .status()
            .expect("run the printed command");
        assert!(status.success(), "{error}");
        assert!(!dir.exists(), "the literal stale lock was removed");
        assert!(install.is_dir(), "the installation remains");
        assert!(
            !root.path().join("escaped").exists(),
            "no command substitution ran"
        );
    }

    /// A display-only encoding cannot promise a lossless shell command.
    /// Unsupported bytes and controls must retain the lock and give manual
    /// advice, so pasting diagnostics cannot act on an invented pathname.
    #[test]
    fn stale_install_lock_advice_omits_commands_for_unprintable_paths() {
        use std::os::unix::ffi::OsStringExt as _;
        let root = tempfile::tempdir().expect("fixture root");
        let mut names = vec![std::ffi::OsString::from("control\npath")];
        // APFS refuses invalid UTF-8 names at creation (EILSEQ), so no stale
        // lock can sit under such a path on macOS. Keep the control-character
        // case everywhere and the raw-byte case where the filesystem allows it.
        if !cfg!(target_os = "macos") {
            names.push(std::ffi::OsString::from_vec(b"invalid-\xff".to_vec()));
        }
        for name in names {
            let install = root.path().join(name);
            let dir = install.join(INSTALL_LOCK);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("pid"), "stale-fixture\n").unwrap();
            assert!(dir.is_dir() && dir.join("pid").is_file());
            let error = InstallLock::acquire(&install)
                .err()
                .expect("stale lock refuses")
                .to_string();
            assert!(error.contains(&super::super::path_text(&dir)), "{error}");
            assert!(error.contains("by hand"), "{error}");
            assert!(
                !error.contains("rm -f") && !error.contains("rmdir"),
                "{error}"
            );
            assert!(dir.join("pid").is_file(), "the refusal changes nothing");
        }
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

    /// Uninstall claims both runtime locks before a first desktop launch
    /// can create its state directory and start serving.
    ///
    /// Why: on macOS the open desktop app's supervisor and helm hold these
    /// locks, and removing the app underneath them is the overlap SPEC.md's
    /// "Concurrent and interrupted runs" requires a correct outcome for;
    /// refusing is that outcome. A first launch can begin while uninstall
    /// runs, so a missing directory must be created and locked before it
    /// can be treated as idle. Spec: with another descriptor holding either
    /// lock, acquire refuses naming it; with both free, both are taken and
    /// released on drop, even when the directory was missing.
    #[test]
    fn runtime_locks_refuse_while_held_and_lock_a_missing_state_dir() {
        let base = tempfile::tempdir().expect("base");
        let state = base.path().join("state");
        assert!(!state.exists(), "the first launch has not made state yet");
        let held = RuntimeLocks::acquire(std::slice::from_ref(&state))
            .expect("create and lock the missing state dir");
        let mode = fs::metadata(&state)
            .expect("state dir")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700, "the new state dir is private");
        for name in [SUPERVISOR_LOCK, HELM_LOCK] {
            let probe = fs::OpenOptions::new()
                .write(true)
                .open(state.join(name))
                .expect("first launch's lock file");
            assert!(
                matches!(probe.try_lock(), Err(fs::TryLockError::WouldBlock)),
                "a first launch must not take {name} while uninstall holds it"
            );
        }
        drop(held);
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

    /// One state directory reached through two spellings is locked once,
    /// without changing permissions on a pre-existing symlink target.
    ///
    /// Why: uninstall checks both the `XDG_STATE_HOME` state directory and
    /// the default one, and when those are the same directory (a symlinked
    /// `~/.local/state`, say) locking it twice made uninstall refuse
    /// against its own first lock on every retry. Uninstall must not chmod
    /// a symlink target, even when a held lock makes it refuse. Spec: with
    /// one entry a symlink to the other, acquire succeeds and holds the
    /// lock without changing its mode.
    #[test]
    fn runtime_locks_take_one_directory_spelled_two_ways_once() {
        let base = tempfile::tempdir().expect("base");
        let state = base.path().join("state");
        fs::create_dir(&state).expect("state");
        fs::set_permissions(&state, fs::Permissions::from_mode(0o755)).expect("wide state mode");
        let alias = base.path().join("alias");
        std::os::unix::fs::symlink(&state, &alias).expect("alias");
        let holder = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(state.join(SUPERVISOR_LOCK))
            .expect("holder");
        holder.try_lock().expect("the holder takes the lock");
        let refusal = RuntimeLocks::acquire(std::slice::from_ref(&alias))
            .err()
            .expect("a held lock refuses through the alias")
            .to_string();
        assert!(
            refusal.contains(SUPERVISOR_LOCK) && refusal.contains("held by another process"),
            "{refusal}"
        );
        assert_eq!(
            fs::metadata(&state)
                .expect("state dir")
                .permissions()
                .mode()
                & 0o777,
            0o755,
            "refusal must not chmod the symlink target"
        );
        drop(holder);
        let held =
            RuntimeLocks::acquire(&[state.clone(), alias]).expect("one directory, locked once");
        assert_eq!(
            fs::metadata(&state)
                .expect("state dir")
                .permissions()
                .mode()
                & 0o777,
            0o755,
            "locking an existing directory must not chmod it"
        );
        let probe = fs::OpenOptions::new()
            .write(true)
            .open(state.join(SUPERVISOR_LOCK))
            .expect("probe");
        assert!(probe.try_lock().is_err(), "uninstall holds the lock");
        drop(held);
    }
}
