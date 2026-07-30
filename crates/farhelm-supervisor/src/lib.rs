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
//! State model: SQLite (`store` module) is the truth that a session
//! exists and what its metadata is; tmux remains the truth for whether
//! its terminal is currently alive. M3 adds a third fact to the store —
//! the last outcome the supervisor WITNESSED, plus the boot id it last
//! saw — which is what makes interrupted-vs-exited a classification
//! rather than a guess once tmux is gone (PLAN_M3.md item 2, `service`
//! and `store` module docs). M3 also adds the per-session integration
//! snapshot and conversation-identity capture behind the `AgentKind` seam
//! (`agent_kind` module, PLAN_M3.md items 7 and 8): which agent a session
//! runs, how a resume would be invoked, and — read purely by observing the
//! agents' own on-disk records — which conversation it belongs to. The
//! `scope` module adds M3's last piece: where a systemd user manager
//! exists, each launch also gets its own cgroup, which stop kills BEFORE
//! (never instead of) the portable sweep.
//!
//! Unix-only, unconditionally: the doorway is a unix socket and the
//! substrate is tmux, so unix APIs are used without cfg gates — a
//! non-unix port would be a redesign, not a compile flag.

pub mod agent_kind;
pub mod files;
pub mod launch;
pub mod scope;
pub mod service;
pub mod store;
pub mod tmux;

// Re-exported at the crate root so `crate::write_private_file` /
// `crate::overwrite_private_file` reads naturally at every call site: both
// keep `service.rs`'s pre-existing spelling unchanged (the write-atomicity
// policy and its fault-injection seam live in `files`, PLAN_M3.md item 5,
// but moving the helpers there is a re-homing, not a rename), while
// `tmux.rs` is a NEW consumer of `overwrite_private_file` added in this
// same change (see that module's docs for why the tmux config joined this
// tier), not a call site this re-export is preserving.
pub use files::{overwrite_private_file, write_private_file};

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
