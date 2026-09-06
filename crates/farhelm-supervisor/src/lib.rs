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
//! its terminal is currently alive. The store carries a third fact —
//! the last outcome the supervisor WITNESSED, plus the boot id it last
//! saw — which is what makes interrupted-vs-exited a classification
//! rather than a guess once tmux is gone (PLAN_M3.md item 2, `service`
//! and `store` module docs). The per-session integration snapshot and
//! conversation-identity capture live behind the `AgentKind` seam
//! (`agent_kind` module, PLAN_M3.md items 7 and 8): which agent a session
//! runs, how a resume would be invoked, and — read purely by observing the
//! agents' own on-disk records — which conversation it belongs to. Where
//! a systemd user manager exists (`scope` module), each launch also gets
//! its own cgroup, which stop kills BEFORE (never instead of) the
//! portable sweep.
//!
//! Attachment uploads (PLAN_M4.md item 4): bytes streamed in over the
//! protocol land in a per-session directory whose layout, naming rules,
//! and lifecycle sweeps live in the `attachments` module, while the
//! transfer itself — data channels, credit, stall detection, commit —
//! belongs to `service` and the streaming write atomicity to `files`.
//!
//! Unix-only, unconditionally: the doorway is a unix socket and the
//! substrate is tmux, so unix APIs are used without cfg gates — a
//! non-unix port would be a redesign, not a compile flag. Differences
//! BETWEEN unixes are confined to one place, the private `procs` module:
//! Linux and macOS disagree only about how the process table is read, and
//! the sweep that decides what to kill is shared verbatim.

pub mod agent_kind;
pub mod attachments;
pub mod files;
pub mod launch;
// Private, and deliberately so: `procs` is the process-table read seam
// (`/proc` on Linux, `sysctl` on macOS) and nothing outside
// `service::sweep` has business reading a process table at all. Its own
// module docs carry the contract; the visibility is the part worth
// stating here. A doc comment rather than this plain one would re-home
// the module's docs into lib.rs's link scope and break every intra-doc
// link inside them.
mod procs;
pub mod scope;
pub mod service;
pub mod store;
pub mod tmux;

// Re-exported at the crate root so `crate::write_private_file` reads
// naturally at its call site: it keeps `service.rs`'s pre-existing
// spelling unchanged (the write-atomicity policy and its fault-injection
// seam live in `files`, PLAN_M3.md item 5, but moving the helper there
// was a re-homing, not a rename).
pub use files::write_private_file;

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

/// The state directory rule itself, for callers that resolved the two
/// inputs themselves: `$XDG_STATE_HOME/farhelm` when that variable carries
/// something, `~/.local/state/farhelm` otherwise.
///
/// The entry point for code that must not read the environment at all.
/// `farhelm helm setup` captures both values once in `main` and computes
/// every path from that capture, which is what lets its tests drive the
/// whole command without touching the test process's environment — and,
/// more importantly, what keeps the unit it writes pointing at the SAME
/// directory a helm or supervisor started from those units would pick for
/// itself. An empty value counts as unset, matching the shell's
/// `${VAR:-default}` and [`default_state_dir`].
pub fn default_state_dir_for(
    xdg_state_home: Option<&std::ffi::OsStr>,
    home: &std::path::Path,
) -> std::path::PathBuf {
    match xdg_state_home.filter(|value| !value.is_empty()) {
        Some(xdg) => std::path::PathBuf::from(xdg).join("farhelm"),
        None => home.join(".local").join("state").join("farhelm"),
    }
}

/// The pure decision half of [`default_state_dir`], separated so tests
/// can exercise missing and non-UTF-8 environment values without
/// mutating the test runner's process environment.
///
/// Only the "neither is set" refusal lives here; the layout rule itself
/// is [`default_state_dir_for`], so the environment-reading path and the
/// injected one cannot come to disagree about where state belongs.
fn default_state_dir_from(
    xdg: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> anyhow::Result<std::path::PathBuf> {
    let xdg = xdg.filter(|value| !value.is_empty());
    let home = home.filter(|value| !value.is_empty());
    if xdg.is_none() && home.is_none() {
        anyhow::bail!("neither XDG_STATE_HOME nor HOME is set; use --state-dir");
    }
    Ok(default_state_dir_for(
        xdg.as_deref(),
        std::path::Path::new(home.as_deref().unwrap_or_default()),
    ))
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
    #[farhelm_testtrace::test]
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
    #[farhelm_testtrace::test]
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

    /// An EMPTY `XDG_STATE_HOME` is unset, exactly as `${VAR:-default}`
    /// reads it. A unit or profile that writes `XDG_STATE_HOME=` means "no
    /// override" to whoever wrote it, and the alternative reading — a
    /// relative `farhelm` directory under whatever the service's working
    /// directory happens to be — would put the helm and its supervisor on
    /// different state trees and hide their socket from each other.
    ///
    /// Both entry points are checked because `farhelm helm setup` computes
    /// the path it PINS through the injected one while the running helm
    /// computes its own through the environment one; they must agree.
    #[farhelm_testtrace::test]
    fn an_empty_xdg_state_home_falls_back_to_the_home_layout() {
        let expected = std::path::PathBuf::from("/home/u/.local/state/farhelm");
        assert_eq!(
            super::default_state_dir_from(Some("".into()), Some("/home/u".into())).unwrap(),
            expected
        );
        assert_eq!(
            super::default_state_dir_for(
                Some(std::ffi::OsStr::new("")),
                std::path::Path::new("/home/u")
            ),
            expected
        );
        assert_eq!(
            super::default_state_dir_for(None, std::path::Path::new("/home/u")),
            expected
        );
        assert_eq!(
            super::default_state_dir_for(
                Some(std::ffi::OsStr::new("/xdg")),
                std::path::Path::new("/home/u")
            ),
            std::path::PathBuf::from("/xdg/farhelm")
        );
    }
}
