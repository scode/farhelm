//! The directory a supervisor's host sockets live in: `<state dir>/hosts`.
//!
//! A host trusts every process that can reach its socket
//! (`crates/terminal-host/SPEC.md`, "Security"), and connecting means typing
//! into a terminal, which is code execution as this user. So the directory's permissions are the whole
//! boundary: it must be a real directory, owned by this user, and private
//! (0700). One that exists in any other shape is refused rather than repaired:
//! something other than Farhelm made it, or loosened it, and a supervisor that
//! quietly fixed the mode would still hand its terminals to whoever was able to
//! get in before.

use std::fs;
use std::io;
use std::os::unix::fs::{DirBuilderExt as _, MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

/// The socket directory's name inside the state directory. The test-state
/// sweep (`farhelm-teststate`) looks for directories of this name to end the
/// hosts a test run leaves behind; a test pins that the two agree.
pub const HOSTS_DIR_NAME: &str = "hosts";

/// The socket directory of `state_dir`, whether or not it exists.
pub fn hosts_dir(state_dir: &Path) -> PathBuf {
    state_dir.join(HOSTS_DIR_NAME)
}

/// Why the socket directory cannot be used.
#[derive(Debug, thiserror::Error)]
pub enum HostsDirError {
    /// Creating it, or reading what is there, failed.
    #[error("terminal host directory {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    /// Something other than a directory (a file, a symlink) has its name.
    #[error("terminal host directory {path} is not a directory")]
    NotADirectory { path: PathBuf },
    /// It belongs to another user.
    #[error(
        "terminal host directory {path} belongs to uid {owner}, not to this user (uid {expected})"
    )]
    WrongOwner {
        path: PathBuf,
        owner: u32,
        expected: u32,
    },
    /// Its permissions are not exactly 0700.
    #[error("terminal host directory {path} has mode {mode:04o}; it must be 0700")]
    WrongMode { path: PathBuf, mode: u32 },
}

/// Create the socket directory if it is missing, and check it either way,
/// returning its path.
///
/// The state directory itself must exist; it is not created here. A directory
/// this call creates is made 0700 even under a umask that would narrow it
/// (mkdir's mode is only ever masked down, never widened, so it is not briefly
/// wider in between). One that already exists is only checked.
pub fn ensure_hosts_dir(state_dir: &Path) -> Result<PathBuf, HostsDirError> {
    // SAFETY: geteuid has no preconditions and cannot fail.
    ensure_hosts_dir_as(state_dir, unsafe { libc::geteuid() })
}

/// [`ensure_hosts_dir`] with the expected owner injected, so the ownership
/// check can be proved without a privileged `chown`.
fn ensure_hosts_dir_as(state_dir: &Path, owner: libc::uid_t) -> Result<PathBuf, HostsDirError> {
    let path = hosts_dir(state_dir);
    let io_error = |source| HostsDirError::Io {
        path: path.clone(),
        source,
    };
    match fs::DirBuilder::new().mode(0o700).create(&path) {
        Ok(()) => {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).map_err(io_error)?;
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(io_error(error)),
    }
    // `symlink_metadata`, so a symlink planted at the name is refused as not
    // a directory instead of being followed to wherever it points.
    let meta = fs::symlink_metadata(&path).map_err(io_error)?;
    if !meta.is_dir() {
        return Err(HostsDirError::NotADirectory { path });
    }
    if meta.uid() != owner {
        return Err(HostsDirError::WrongOwner {
            path,
            owner: meta.uid(),
            expected: owner,
        });
    }
    let mode = meta.permissions().mode() & 0o7777;
    if mode != 0o700 {
        return Err(HostsDirError::WrongMode { path, mode });
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn euid() -> libc::uid_t {
        // SAFETY: no preconditions.
        unsafe { libc::geteuid() }
    }

    /// A missing socket directory is created private, and an existing private
    /// one is accepted as it is.
    ///
    /// Why: the directory's mode is the only thing between other local users
    /// and every terminal's input. Spec: after the first call `<state>/hosts`
    /// is a directory with mode exactly 0700, and a second call returns the
    /// same path without error.
    #[test]
    fn a_missing_directory_is_created_private_and_an_existing_one_is_kept() {
        let state = tempfile::tempdir().unwrap();
        let path = ensure_hosts_dir(state.path()).expect("created");
        assert_eq!(path, state.path().join("hosts"));
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o7777;
        assert_eq!(mode, 0o700);
        assert_eq!(ensure_hosts_dir(state.path()).expect("accepted"), path);
    }

    /// An existing directory with any mode other than 0700 is refused, not
    /// repaired.
    ///
    /// Why: a looser directory may already have let someone in (a process
    /// that opened it keeps its descriptor across a chmod), so the supervisor
    /// must not start terminals there, and must say why. Spec: modes 0755,
    /// 0750, 0701 and 0300 each give [`HostsDirError::WrongMode`] naming that
    /// mode, and the mode on disk is left as it was.
    #[test]
    fn a_directory_with_other_permissions_is_refused() {
        for mode in [0o755, 0o750, 0o701, 0o300] {
            let state = tempfile::tempdir().unwrap();
            let path = state.path().join(HOSTS_DIR_NAME);
            fs::create_dir(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
            match ensure_hosts_dir(state.path()) {
                Err(HostsDirError::WrongMode { mode: found, .. }) => assert_eq!(found, mode),
                other => panic!("mode {mode:o}: expected a refusal, got {other:?}"),
            }
            let on_disk = fs::symlink_metadata(&path).unwrap().permissions().mode() & 0o7777;
            assert_eq!(on_disk, mode, "the refusal must not change the mode");
            // Restore search permission so the temporary directory can be
            // removed whatever mode the case left.
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        }
    }

    /// A file or a symlink at the directory's name is refused, and a symlink
    /// is not followed.
    ///
    /// Why: a symlink to some other private directory would otherwise pass
    /// every check on its target and put this supervisor's sockets wherever
    /// the link points. Spec: a regular file and a symlink to a 0700
    /// directory both give [`HostsDirError::NotADirectory`].
    #[test]
    fn a_file_or_symlink_at_the_name_is_refused() {
        let state = tempfile::tempdir().unwrap();
        fs::write(state.path().join(HOSTS_DIR_NAME), b"").unwrap();
        assert!(matches!(
            ensure_hosts_dir(state.path()),
            Err(HostsDirError::NotADirectory { .. })
        ));

        let state = tempfile::tempdir().unwrap();
        let elsewhere = state.path().join("elsewhere");
        fs::create_dir(&elsewhere).unwrap();
        fs::set_permissions(&elsewhere, fs::Permissions::from_mode(0o700)).unwrap();
        std::os::unix::fs::symlink(&elsewhere, state.path().join(HOSTS_DIR_NAME)).unwrap();
        assert!(matches!(
            ensure_hosts_dir(state.path()),
            Err(HostsDirError::NotADirectory { .. })
        ));
    }

    /// A directory owned by another user is refused.
    ///
    /// Why: its owner can change its mode at any time, so 0700 on someone
    /// else's directory protects nothing. Spec: judged against an owner other
    /// than the directory's, an otherwise valid directory gives
    /// [`HostsDirError::WrongOwner`]; judged against its real owner, it is
    /// accepted (so the refusal is the owner check's, nothing else's).
    #[test]
    fn a_directory_owned_by_someone_else_is_refused() {
        let state = tempfile::tempdir().unwrap();
        ensure_hosts_dir(state.path()).expect("premise: a valid directory");
        assert!(matches!(
            ensure_hosts_dir_as(state.path(), euid().wrapping_add(1)),
            Err(HostsDirError::WrongOwner { .. })
        ));
        assert!(ensure_hosts_dir_as(state.path(), euid()).is_ok());
    }

    /// The supervisor's directory name and the one the test-state sweep looks
    /// for are the same.
    ///
    /// Why: the sweep ends the hosts a killed test run leaves behind by
    /// finding directories of this name; if the two drifted apart, test hosts
    /// would leak on every killed run with nothing failing. Spec: the two
    /// constants are equal.
    #[test]
    fn the_test_state_sweep_looks_for_this_directory_name() {
        assert_eq!(HOSTS_DIR_NAME, farhelm_teststate::hosts::HOSTS_DIR_NAME);
    }
}
