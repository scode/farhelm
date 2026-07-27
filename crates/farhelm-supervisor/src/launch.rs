//! Launch plumbing: how an agent invocation becomes a process inside the
//! session's login shell, and how exec failure is told apart from
//! ran-and-died.
//!
//! The chain is: tmux window command → user's shell as an interactive
//! login shell (`-l -i -c`) → `exec farhelm internal launch <spec>` → the
//! shim execs the real agent argv. The shim exists because a sentinel
//! written by the shell after a failed `exec` can never fire under zsh
//! (zsh terminates on failed exec in every mode — audited in SPEC_impl.md);
//! the shim always exists, so its exec always succeeds, and it can record
//! the real exec's errno before exiting. The `-l -i` reproduces the file
//! sourcing an SSH-and-type session gets, which is SPEC.md's environment
//! contract; `-i` is load-bearing (bare `-c` skips `.zshrc`/`.bashrc`).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// What the shim needs to launch the agent: written as JSON by the
/// supervisor, read by `farhelm internal launch` inside the session. A
/// file (not argv) so the invocation never fights shell quoting twice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchSpec {
    /// The agent argv, already shell-words-split by the supervisor.
    pub argv: Vec<String>,
    /// Where the shim attempts to record exec failure.
    ///
    /// Presence positively identifies a failed `exec`. Absence alone is
    /// not proof of success: the shim may not have attempted `exec` yet,
    /// or the sentinel write itself may have failed. Later status
    /// classification must combine this evidence with pane/process state.
    pub status_file: PathBuf,
}

/// Resolve the shell to launch sessions through: `$SHELL`, then the
/// passwd database, then `/bin/sh`.
///
/// The passwd fallback is not belt-and-braces: systemd user managers
/// older than 255 do not set `$SHELL` for services (SPEC_impl.md), and a
/// provisioned supervisor runs as exactly such a service — without it,
/// those hosts would silently launch agents under `/bin/sh` and lose the
/// user's rc-file environment, which is the contract SPEC.md makes.
pub async fn resolve_shell() -> String {
    resolve_shell_from(std::env::var("SHELL").ok(), passwd_shell().await)
}

/// The decision half of [`resolve_shell`], split out from the lookups so
/// the fallback chain is testable without mutating the test process's
/// environment (a project-wide prohibition).
pub fn resolve_shell_from(env_shell: Option<String>, passwd_shell: Option<String>) -> String {
    env_shell
        .filter(|s| !s.is_empty())
        .or_else(|| passwd_shell.filter(|s| !s.is_empty()))
        .unwrap_or_else(|| "/bin/sh".to_string())
}

/// The current user's login shell per the passwd database; None when
/// getent is missing or the entry is unreadable.
async fn passwd_shell() -> Option<String> {
    let out = tokio::process::Command::new("getent")
        .arg("passwd")
        .arg(whoami())
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let line = String::from_utf8(out.stdout).ok()?;
    // passwd format: name:passwd:uid:gid:gecos:home:shell
    Some(line.trim().rsplit(':').next()?.to_string())
}

/// Who to look up in the passwd database. `$USER` first because it is
/// free, the euid second because a systemd user service — the case
/// `passwd_shell` exists for in the first place — may not have `$USER`
/// set either.
fn whoami() -> String {
    std::env::var("USER").unwrap_or_else(|_| {
        // Last resort: numeric uid works with getent too.
        unsafe { libc_geteuid() }.to_string()
    })
}

// Minimal FFI shim instead of a libc crate dependency for one call.
unsafe extern "C" {
    #[link_name = "geteuid"]
    fn libc_geteuid() -> u32;
}

/// Build the tmux window command argv: the interactive login shell running
/// the launch shim. `farhelm_exe` is this supervisor's own binary — the
/// shim ships inside it, which is why one artifact per host suffices.
pub fn window_command(shell: &str, farhelm_exe: &Path, spec_path: &Path) -> Vec<String> {
    // Quoting matters because both paths derive from $HOME or user flags
    // and can contain spaces or quotes; shell_words::quote is the same
    // POSIX single-quote encoding the invocation parser expects.
    let inner = format!(
        "exec {} internal launch {}",
        shell_words::quote(&farhelm_exe.to_string_lossy()),
        shell_words::quote(&spec_path.to_string_lossy()),
    );
    vec![
        shell.to_string(),
        "-l".to_string(),
        "-i".to_string(),
        "-c".to_string(),
        inner,
    ]
}

/// The shim body: exec the spec's argv, recording failure to the status
/// file first. Lives here (not in the bin crate) so it is unit-testable;
/// `farhelm internal launch` is a thin caller. On success this function
/// never returns.
pub fn exec_launch_spec(spec_path: &Path) -> anyhow::Error {
    use std::os::unix::process::CommandExt;

    let bytes = match std::fs::read(spec_path) {
        Ok(bytes) => bytes,
        Err(e) => return anyhow::Error::from(e).context("reading launch spec"),
    };
    // Unlink the moment the bytes are in hand — before parsing, not just
    // before exec: the spec holds the agent's full command line, which
    // users do put credentials into, and an early return on a malformed
    // spec must not leave it on disk. Nothing else removes it during
    // this supervisor's lifetime; only the next restart's sweep would.
    if let Err(e) = std::fs::remove_file(spec_path) {
        return anyhow::Error::from(e)
            .context(format!("removing launch spec {}", spec_path.display()));
    }
    let spec: LaunchSpec = match serde_json::from_slice(&bytes) {
        Ok(spec) => spec,
        Err(e) => return anyhow::Error::from(e).context("parsing launch spec"),
    };
    if spec.argv.is_empty() {
        return anyhow::anyhow!("launch spec has empty argv");
    }
    // exec only returns on failure.
    let err = std::process::Command::new(&spec.argv[0])
        .args(&spec.argv[1..])
        .exec();
    let report = format!(
        "exec_failed argv0={} errno={}",
        spec.argv[0],
        err.raw_os_error().unwrap_or(-1)
    );
    let report = match std::fs::write(&spec.status_file, &report) {
        Ok(()) => report,
        Err(write_error) => format!(
            "{report}; could not record exec failure at {}: {write_error}",
            spec.status_file.display()
        ),
    };
    anyhow::Error::from(err).context(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The window command shape is a contract with SPEC_impl.md's
    /// environment section: login + interactive + exec-shim, exactly.
    /// `-i` is the load-bearing flag (bare `-c` skips rc files), so a
    /// regression that drops it must fail here.
    #[test]
    fn window_command_is_login_interactive_exec_shim() {
        let cmd = window_command(
            "/bin/bash",
            Path::new("/opt/farhelm"),
            Path::new("/state/launch/abc.json"),
        );
        assert_eq!(cmd[0..4], ["/bin/bash", "-l", "-i", "-c"]);
        assert_eq!(
            cmd[4],
            "exec /opt/farhelm internal launch /state/launch/abc.json"
        );
    }

    /// A state dir under a path with spaces or quotes must still produce
    /// a command the shell parses as one argument — otherwise sessions
    /// fail to launch on perfectly legal home directories.
    #[test]
    fn window_command_quotes_hostile_paths() {
        let cmd = window_command(
            "/bin/zsh",
            Path::new("/opt/far helm"),
            Path::new("/state/it's/abc.json"),
        );
        let parsed = shell_words::split(&cmd[4]).expect("shell-parseable");
        assert_eq!(
            parsed,
            vec![
                "exec",
                "/opt/far helm",
                "internal",
                "launch",
                "/state/it's/abc.json"
            ]
        );
    }

    /// The shell-resolution chain exists for systemd user services older
    /// than 255, which do not set $SHELL; getting it wrong means agents
    /// launch under /bin/sh without the user's environment.
    #[test]
    fn shell_resolution_prefers_env_then_passwd_then_sh() {
        assert_eq!(
            resolve_shell_from(Some("/bin/fish".into()), Some("/bin/zsh".into())),
            "/bin/fish"
        );
        // Empty $SHELL must not win — this is the systemd case.
        assert_eq!(
            resolve_shell_from(Some(String::new()), Some("/bin/zsh".into())),
            "/bin/zsh"
        );
        assert_eq!(
            resolve_shell_from(None, Some("/bin/zsh".into())),
            "/bin/zsh"
        );
        assert_eq!(resolve_shell_from(None, None), "/bin/sh");
        assert_eq!(resolve_shell_from(None, Some(String::new())), "/bin/sh");
    }
}
