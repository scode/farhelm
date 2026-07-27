//! The supervisor: per-host session management and nothing else.
//!
//! Per SPEC.md's Concepts, the supervisor launches agents, owns their
//! terminals, and is the authority on its sessions. It has no UI and no
//! knowledge of other hosts, and listens on no network port — it is
//! reached over a unix socket, remotely via `farhelm internal stdio`
//! through the user's ssh. The terminal substrate is a private headless
//! tmux server (`tmux` module); the wire protocol is farhelm-proto
//! (`service` module); the launch path through the user's login shell and
//! the exec shim lives in `launch`; SQLite-backed session metadata lives
//! in `store`.
//!
//! M2 state model: SQLite (`store` module) is the truth that a session
//! exists and what its metadata is; tmux remains the truth for whether
//! its terminal is currently alive. Durability classification proper
//! (interrupted-vs-exited via boot-id comparison) and agent-kind
//! integrations arrive in later milestones (PLAN.md).
//!
//! Unix-only, unconditionally: the doorway is a unix socket and the
//! substrate is tmux, so unix APIs are used without cfg gates — a
//! non-unix port would be a redesign, not a compile flag.

pub mod launch;
pub mod service;
pub mod store;
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

/// Write a file readable by the owner alone, atomically replacing any
/// prior content at the same path — the sibling of [`write_private_file`]
/// for callers wanting a STABLE, repeatedly-rewritten path (keyed by
/// something like a session id, whose contents get replaced on each
/// write) rather than a fresh, write-once one.
///
/// Used for the alt-screen stop snapshot
/// (`crates/farhelm-supervisor/src/service.rs`'s `StopSession` handler):
/// the path is keyed by the session id, and a later stop overwriting an
/// earlier stop's snapshot at that SAME path is the whole point — only
/// the most recent screen before a kill is worth keeping. (Both this
/// file's path and [`write_private_file`]'s launch-spec paths happen to
/// be named after the same session id, a UUID generated at session
/// creation; the difference between the two functions is REPLACE-vs-
/// REFUSE-on-collision, not the naming scheme.)
///
/// Implemented as write-to-a-fresh-temp-file-then-`rename`, in the SAME
/// directory as `path` (so the rename is same-filesystem and therefore
/// atomic), rather than open-with-`truncate` in place. That fixes three
/// holes a truncate-in-place leaves open:
/// - a PRE-EXISTING file at `path` created with a wider mode (by
///   something else, or by an earlier bug) would keep that wider mode
///   forever — `OpenOptions::mode` only applies at CREATE time, never
///   when an existing file is merely opened for writing — whereas the
///   fresh temp file is always created 0600 from scratch, and `rename`
///   replaces `path` outright rather than reusing its inode;
/// - a reader (an `Attach` racing this write) could otherwise observe a
///   PARTIALLY-written, torn snapshot mid-truncate; impossible here,
///   because `rename` swaps the whole file atomically, so a concurrent
///   reader always sees either the complete old content or the complete
///   new content, never a mix;
/// - a symlink an attacker planted at a predictable, session-id-derived
///   `path` would make a truncate-in-place follow it and clobber
///   whatever it points to; `rename` instead REPLACES whatever `path`
///   names, symlink or not, without ever opening through it.
///
/// The temp file's name carries a fresh UUID suffix (`create_new` makes
/// a collision a hard error rather than a silent overwrite of some other
/// writer's in-flight temp file) purely so concurrent callers targeting
/// the same destination can never collide on their staging file. Any
/// failure before OR during the rename removes the temp file, so a
/// failed write never leaves debris behind on its own — see
/// [`remove_temp_after_failure`] for what happens when that cleanup
/// ITSELF fails, and [`crate::service::sweep_snapshot_temp_files`] for
/// the startup-time backstop covering a crash that skips this cleanup
/// entirely.
pub async fn overwrite_private_file(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use tokio::io::AsyncWriteExt;
    let dir = path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "{} has no parent directory to stage a temp file in",
                path.display()
            ),
        )
    })?;
    let temp_name = format!(
        ".{}.tmp-{}",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("overwrite"),
        uuid::Uuid::new_v4()
    );
    let temp_path = dir.join(temp_name);

    let mut opts = tokio::fs::OpenOptions::new();
    opts.write(true).create_new(true).mode(0o600);
    let write_result: std::io::Result<()> = async {
        let mut file = opts.open(&temp_path).await?;
        file.write_all(bytes).await?;
        // tokio's File buffers internally and its Drop does NOT flush —
        // without this the temp file could be renamed into place empty.
        file.flush().await
    }
    .await;
    if let Err(e) = write_result {
        return Err(remove_temp_after_failure(&temp_path, e).await);
    }

    // The rename can ALSO fail (destination is a directory, a permission
    // problem, cross-device if `dir` and `path` ever diverge) — a failure
    // here is just as much "the temp file is now debris" as a write
    // failure, so it gets the identical cleanup treatment rather than
    // leaving the staged file behind under the mistaken assumption that
    // only the write step can go wrong.
    if let Err(e) = tokio::fs::rename(&temp_path, path).await {
        return Err(remove_temp_after_failure(&temp_path, e).await);
    }
    Ok(())
}

/// Remove a staged temp file after its OWN write or rename already
/// failed, folding a cleanup failure into the RETURNED error rather than
/// silently discarding it.
///
/// A `remove_file` that itself fails here (permissions, a concurrent
/// removal racing this one) means the temp file may still be sitting on
/// disk — exactly the debris this whole function exists to prevent — so
/// the caller needs to learn that too, not just the original write/rename
/// failure. `original`'s `kind()` is preserved on the combined error
/// (rather than the cleanup error's own kind) because the write/rename
/// failure is the causally primary one; the cleanup failure is
/// context layered on top, mirroring how [`write_private_file`] already
/// handles its own analogous partial-file cleanup.
async fn remove_temp_after_failure(
    temp_path: &std::path::Path,
    original: std::io::Error,
) -> std::io::Error {
    match tokio::fs::remove_file(temp_path).await {
        Ok(()) => original,
        Err(cleanup_error) => std::io::Error::new(
            original.kind(),
            format!(
                "{original}; could not remove staged temp file {}: {cleanup_error}",
                temp_path.display()
            ),
        ),
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

    /// The rename-based replace must leave EXACTLY the new content behind
    /// — no trailing bytes from a longer previous write. A truncate-based
    /// implementation would pass this too, but a naive "seek to 0 and
    /// write" one would not: this is the regression the atomic rewrite
    /// (temp file + rename) is pinned against, longer-then-shorter being
    /// the direction that actually exposes a missing truncate.
    #[tokio::test]
    async fn overwrite_private_file_replaces_longer_content_with_shorter_exactly() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("snapshot");

        super::overwrite_private_file(&path, b"a much longer first payload")
            .await
            .unwrap();
        super::overwrite_private_file(&path, b"short")
            .await
            .unwrap();

        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"short",
            "the replaced file must contain exactly the new content, not the old content \
             truncated-and-overwritten with leftover trailing bytes"
        );
    }

    /// The written file must be 0600 regardless of how many times the
    /// path has already been written — pins that the temp-file-then-
    /// rename replacement always creates its staging file fresh (mode set
    /// at CREATE time) rather than ever reopening the destination in
    /// place, where an existing file's mode would otherwise be whatever
    /// it already was.
    #[tokio::test]
    async fn overwrite_private_file_is_0600_on_repeat_writes() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("snapshot");

        super::overwrite_private_file(&path, b"first")
            .await
            .unwrap();
        super::overwrite_private_file(&path, b"second")
            .await
            .unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "overwritten file must be 0600, got {mode:o}");
    }

    /// A file that predates this code (or was created some other way)
    /// with a too-wide mode must not keep that mode forever just because
    /// its content happened to get replaced. This is exactly the hole a
    /// `truncate`-in-place implementation leaves open — `OpenOptions::mode`
    /// only applies at file CREATION, so reopening an existing file for
    /// writing never narrows it — and exactly what the rename-based
    /// replacement (a fresh 0600 temp file swapped in over the old one)
    /// fixes structurally rather than by remembering to chmod.
    #[tokio::test]
    async fn overwrite_private_file_repairs_a_pre_existing_wide_mode() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("snapshot");

        std::fs::write(&path, b"planted by something else").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        super::overwrite_private_file(&path, b"replaced")
            .await
            .unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "a pre-existing wide-mode file must be replaced with a 0600 one, got {mode:o}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"replaced");
    }

    /// A `rename` failure (not just a write failure) must still clean up
    /// the staged temp file — the hole `remove_temp_after_failure` closes
    /// on the rename path specifically. A DIRECTORY at the destination is
    /// what reliably makes `rename` fail here: POSIX `rename(2)` refuses
    /// to replace a directory with a non-directory regardless of
    /// permissions, unlike a plain permission trick that might not
    /// reliably fail for the same reason on every filesystem.
    #[tokio::test]
    async fn overwrite_private_file_removes_the_temp_file_when_rename_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("snapshot");
        std::fs::create_dir(&path).unwrap();

        super::overwrite_private_file(&path, b"content")
            .await
            .expect_err("rename onto a directory must fail");

        let leftover: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp-"))
            .collect();
        assert!(
            leftover.is_empty(),
            "a failed rename must not leave a staged temp file behind, found: {leftover:?}"
        );
    }

    /// `path` being a SYMLINK must not make this function write through
    /// it: `rename` replaces whatever directory entry `path` names
    /// (symlink or not) with the temp file, rather than ever opening
    /// `path` itself and following it — the third hole the atomic
    /// rewrite closes (see the function's own docs). Pins both halves:
    /// the destination ends up a plain, 0600 regular file with the new
    /// content, and whatever the symlink used to point at is completely
    /// untouched.
    #[tokio::test]
    async fn overwrite_private_file_replaces_a_symlink_without_following_it() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target");
        let path = tmp.path().join("snapshot");
        std::fs::write(&target, b"target content").unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();

        super::overwrite_private_file(&path, b"replacement")
            .await
            .unwrap();

        assert!(
            !std::fs::symlink_metadata(&path)
                .unwrap()
                .file_type()
                .is_symlink(),
            "the destination must become a regular file, not remain a symlink"
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"replacement");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "the replacement file must be 0600, got {mode:o}"
        );
        assert_eq!(
            std::fs::read(&target).unwrap(),
            b"target content",
            "the symlink's OLD target must be left completely untouched"
        );
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
