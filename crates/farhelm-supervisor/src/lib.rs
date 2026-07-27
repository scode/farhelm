//! The supervisor: per-host session management and nothing else.
//!
//! Per SPEC.md's Concepts, the supervisor launches agents, owns their
//! terminals, and is the authority on its sessions. It has no UI and no
//! knowledge of other hosts, and listens on no network port — it is
//! reached over a unix socket, remotely via `farhelm internal stdio`
//! through the user's ssh. The terminal substrate is a private headless
//! tmux server (`tmux` module); the wire protocol is farhelm-proto
//! (`service` module); the launch path through the user's login shell and
//! the exec shim lives in `launch`.
//!
//! M1 state model: in-memory sessions with tmux as the truth. Durability
//! classification, SQLite, and agent-kind integrations arrive in later
//! milestones (PLAN.md).
//!
//! Unix-only, unconditionally: the doorway is a unix socket and the
//! substrate is tmux, so unix APIs are used without cfg gates — a
//! non-unix port would be a redesign, not a compile flag.

pub mod launch;
pub mod service;
pub mod tmux;

/// Create a state directory (and parents) restricted to the owning user.
///
/// The 0700 mode is a security boundary, not tidiness: the supervisor's
/// unix socket lives here, and connecting to it means creating sessions
/// with an arbitrary command line — code execution as this user. The
/// protocol has no authentication by design (SPEC.md: no network port,
/// reached via ssh), so filesystem permissions are the whole boundary,
/// and inheriting an ambient umask of 002 or 000 would silently open it
/// to other local users. Same reasoning for the helm's directory, which
/// holds ssh ControlMaster sockets.
/// The mode is applied at creation (`DirBuilder::mode`), not by a chmod
/// afterwards: create-then-chmod leaves a window where a permissive umask
/// exposes the directory, and a local attacker who opens it in that window
/// keeps their descriptor across the chmod. mkdir's mode is only ever
/// masked *down* by the umask, so the directory is never briefly wider
/// than 0700. The `set_permissions` that follows is the repair path — for
/// a pre-existing directory, or one a restrictive umask narrowed below
/// 0700 — not the mechanism.
pub async fn ensure_private_dir(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut builder = tokio::fs::DirBuilder::new();
    builder.recursive(true).mode(0o700);
    builder.create(path).await?;
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).await?;
    Ok(())
}

/// Write a file readable by the owner alone, failing if it already exists.
///
/// The 0600 mode is set at open, for the same window-free reason as
/// [`ensure_private_dir`]: these files carry agent command lines that users
/// put credentials into, and write-then-chmod would expose the bytes under
/// a permissive umask exactly long enough to lose. `create_new` is safety,
/// not convenience — callers name files after fresh UUIDs, so a collision
/// means something is impersonating the supervisor and deserves an error.
pub async fn write_private_file(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use tokio::io::AsyncWriteExt;
    let mut opts = tokio::fs::OpenOptions::new();
    opts.write(true).create_new(true).mode(0o600);
    let mut file = opts.open(path).await?;
    let result = async {
        file.write_all(bytes).await?;
        // tokio's File buffers internally and its Drop does NOT flush —
        // without this the file can hit disk empty.
        file.flush().await
    }
    .await;
    let Err(write_error) = result else {
        return Ok(());
    };

    // This function knows the open succeeded, so this path owns the
    // partial file. Callers cannot safely do the cleanup themselves: an
    // open-time create_new collision means they do NOT own the existing
    // path and must leave it untouched.
    drop(file);
    match tokio::fs::remove_file(path).await {
        Ok(()) => Err(write_error),
        Err(cleanup_error) => Err(std::io::Error::new(
            write_error.kind(),
            format!(
                "{write_error}; could not remove partial private file {}: {cleanup_error}",
                path.display()
            ),
        )),
    }
}

/// Default state directory: `~/.local/state/farhelm` (SPEC_impl.md's
/// layout), honoring `XDG_STATE_HOME` when set. Everything the supervisor
/// persists — tmux socket, launch specs, its own unix socket — lives
/// under here.
///
/// There is deliberately no current-directory fallback. Helm and
/// supervisor often start in different directories (especially under a
/// service manager), so inventing `./.local/state` makes them silently
/// disagree about the socket and may persist credentials in an arbitrary
/// working directory.
pub fn default_state_dir() -> anyhow::Result<std::path::PathBuf> {
    default_state_dir_from(std::env::var_os("XDG_STATE_HOME"), std::env::var_os("HOME"))
}

/// The pure decision half of [`default_state_dir`], separated so tests
/// can exercise missing and non-UTF-8 environment values without
/// mutating the test runner's process environment.
fn default_state_dir_from(
    xdg: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> anyhow::Result<std::path::PathBuf> {
    if let Some(xdg) = xdg.filter(|value| !value.is_empty()) {
        return Ok(std::path::PathBuf::from(xdg).join("farhelm"));
    }
    let home = home.filter(|value| !value.is_empty()).ok_or_else(|| {
        anyhow::anyhow!("neither XDG_STATE_HOME nor HOME is set; use --state-dir")
    })?;
    Ok(std::path::PathBuf::from(home)
        .join(".local")
        .join("state")
        .join("farhelm"))
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    /// These modes ARE the security boundary (no other authentication
    /// exists on the socket or the launch specs), so the final modes are
    /// pinned here — under the default 022 umask, plain creation yields
    /// world-readable 0755/0644 and nothing else would notice a dropped
    /// `set_permissions` or `OpenOptions::mode`. What end-state
    /// assertions cannot observe is the window-free-creation property
    /// (mode at mkdir/open rather than chmod-after); that mechanism is
    /// documented on the functions and reviewed, not tested.
    #[tokio::test]
    async fn private_dir_is_created_0700_and_repaired_to_it() {
        let tmp = tempfile::tempdir().unwrap();

        let fresh = tmp.path().join("a").join("b");
        super::ensure_private_dir(&fresh).await.unwrap();
        let mode = std::fs::metadata(&fresh).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "fresh dir must be 0700, got {mode:o}");

        // Pre-existing too-wide directory: the repair path must narrow it.
        let wide = tmp.path().join("wide");
        std::fs::create_dir(&wide).unwrap();
        std::fs::set_permissions(&wide, std::fs::Permissions::from_mode(0o755)).unwrap();
        super::ensure_private_dir(&wide).await.unwrap();
        let mode = std::fs::metadata(&wide).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "existing dir must be repaired to 0700");
    }

    /// The launch-spec write path: owner-only from the first byte, and
    /// refusing to overwrite (spec names are fresh UUIDs; a collision is
    /// an impersonation attempt, not a retry).
    #[tokio::test]
    async fn private_file_is_0600_and_never_overwrites() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("spec.json");

        super::write_private_file(&path, b"secret").await.unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "private file must be 0600, got {mode:o}");
        assert_eq!(std::fs::read(&path).unwrap(), b"secret");

        super::write_private_file(&path, b"clobber")
            .await
            .expect_err("existing file must not be overwritten");
        assert_eq!(std::fs::read(&path).unwrap(), b"secret");
    }

    /// Helm and supervisor must never invent cwd-relative state when
    /// service-manager environments omit HOME. The helper also proves
    /// paths stay native OsStrings instead of rejecting non-UTF-8 homes.
    #[test]
    fn default_state_dir_requires_a_real_home_and_accepts_native_paths() {
        use std::os::unix::ffi::OsStringExt;

        assert!(super::default_state_dir_from(None, None).is_err());
        assert_eq!(
            super::default_state_dir_from(Some("/xdg".into()), Some("/home/u".into())).unwrap(),
            std::path::PathBuf::from("/xdg/farhelm")
        );
        assert_eq!(
            super::default_state_dir_from(None, Some("/home/u".into())).unwrap(),
            std::path::PathBuf::from("/home/u/.local/state/farhelm")
        );
        let native_home = std::ffi::OsString::from_vec(b"/home/\xff".to_vec());
        assert_eq!(
            super::default_state_dir_from(None, Some(native_home.clone())).unwrap(),
            std::path::PathBuf::from(native_home).join(".local/state/farhelm")
        );
    }
}
