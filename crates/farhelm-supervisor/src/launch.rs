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
///
/// Fallback transitions and failures are logged: `$SHELL`-missing is
/// expected and unremarkable (`debug!`), and landing on `/bin/sh` breaks
/// the SPEC.md environment contract and must be loud (`warn!`) — the
/// previous implementation swallowed every failure silently, which is
/// exactly how this fallback chain went unnoticed when it mattered. A
/// usable `$SHELL`, or a passwd lookup that resolves cleanly, logs
/// nothing at all.
pub async fn resolve_shell() -> String {
    let env_shell = std::env::var("SHELL").ok();
    let env_shell_usable = env_shell.as_deref().is_some_and(|s| !s.is_empty());

    // Only pay for the passwd lookup when $SHELL can't answer the
    // question; it also lets the debug log below fire exactly when the
    // lookup is actually needed rather than on every call.
    let passwd = if env_shell_usable {
        None
    } else {
        tracing::debug!("$SHELL is unset or empty; looking up the login shell in passwd");
        passwd_shell().await
    };
    let passwd_usable = passwd.as_deref().is_some_and(|s| !s.is_empty());

    if !env_shell_usable && !passwd_usable {
        tracing::warn!(
            "no usable $SHELL and no passwd entry for this process's euid; falling back to \
             /bin/sh, which means the launched session will not see the user's rc-file \
             environment (SPEC.md's environment contract)"
        );
    }

    resolve_shell_from(env_shell, passwd)
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

/// The current process's login shell per the passwd database, looked up
/// by euid; `None` when both lookup rungs fail (each rung logs its own
/// reason, at `debug!` when the next rung can plausibly cover it and
/// `warn!` when the failure looks like a real problem rather than an
/// absent tool or an unusable entry).
///
/// Two rungs, in order, each covering the other's blind spot:
///
/// 1. `getent passwd <euid>`, shelled out to. This supervisor binary is
///    statically linked against musl for release builds (SPEC_impl.md),
///    so its own libc can only read `/etc/passwd` directly (or nscd) — it
///    cannot load the host's glibc NSS modules, so LDAP/SSSD-backed
///    accounts would resolve to nothing through an in-process lookup.
///    `getent`, run as a *host* binary, goes through the host's own NSS
///    stack and sees those accounts.
/// 2. [`passwd_shell_for_euid`]'s direct `getpwuid_r` call, used when
///    `getent` isn't there to ask (normal on macOS and musl-based distros
///    such as Alpine, which don't ship it) or ran but produced nothing
///    usable.
///
/// Looked up by euid rather than `$USER` in both rungs: `$USER` can be
/// stale or spoofed (it is just an environment variable, inherited across
/// `su`/`sudo` without necessarily being updated), which would select
/// another account's login shell, and the case this fallback exists for —
/// a systemd user service with no `$SHELL` — often has no `$USER` either,
/// so euid is the only input guaranteed to be present and trustworthy.
async fn passwd_shell() -> Option<String> {
    if let Some(shell) = getent_passwd_shell().await {
        return Some(shell);
    }

    // getpwuid_r is blocking C code (it may do NSS/network lookups under
    // e.g. sssd or LDAP-backed passwd), so it must not run on the async
    // runtime's worker threads.
    match tokio::task::spawn_blocking(passwd_shell_for_euid).await {
        Ok(shell) => shell,
        Err(e) => {
            tracing::warn!(error = %e, "passwd lookup task panicked");
            None
        }
    }
}

/// The `getent passwd <euid>` rung of [`passwd_shell`]; see that
/// function's docstring for why `getent` is tried before `getpwuid_r`.
///
/// Every failure mode here is expected on *some* host and covered by the
/// `getpwuid_r` fallback, so this returns `None` rather than propagating
/// an error: a missing binary (macOS, musl distros) and a non-zero exit
/// (exit 2 is "no such key"; anything else is an NSS-backend error) are
/// both logged at `debug!`. A successful exit with unparseable output
/// would mean `getent`'s contract itself is violated, which is worth a
/// `warn!` even though the fallback still saves the caller.
async fn getent_passwd_shell() -> Option<String> {
    // SAFETY: geteuid takes no arguments and cannot fail; it is always
    // safe to call.
    let euid = unsafe { libc::geteuid() };

    let output = match tokio::process::Command::new("getent")
        .arg("passwd")
        .arg(euid.to_string())
        .output()
        .await
    {
        Ok(output) => output,
        Err(e) => {
            tracing::debug!(
                error = %e,
                "getent unavailable; falling back to direct passwd lookup"
            );
            return None;
        }
    };

    if !output.status.success() {
        tracing::debug!(
            euid,
            code = output.status.code(),
            "getent passwd exited non-zero; falling back to direct passwd lookup"
        );
        return None;
    }

    // Only the first line matters: `getent passwd <uid>` queried by a
    // single numeric key returns at most one entry, but guard against a
    // misbehaving NSS module or wrapper script emitting extra lines.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some(line) = stdout.lines().next() else {
        tracing::warn!(euid, "getent passwd succeeded but produced no output");
        return None;
    };

    match parse_getent_passwd_line(line) {
        Some(shell) => Some(shell),
        None => {
            tracing::warn!(euid, line, "getent passwd line has no shell field");
            None
        }
    }
}

/// Pull the shell field (the last colon-separated column) out of one
/// `getent passwd` line (`name:passwd:uid:gid:gecos:home:shell`).
///
/// `None` covers both an empty line and a trailing-empty shell field —
/// both mean the entry has nothing usable, which the caller must treat
/// the same way it treats `getent` being entirely absent.
fn parse_getent_passwd_line(line: &str) -> Option<String> {
    let shell = line.rsplit(':').next()?.trim();
    (!shell.is_empty()).then(|| shell.to_string())
}

/// Synchronous `getpwuid_r` lookup for the process's effective UID.
///
/// The buffer-growth retry protocol lives in [`lookup_with_growing_buffer`]
/// so it can be unit-tested without a real passwd database; this function
/// supplies the initial size hint and the actual unsafe libc call as a
/// closure.
fn passwd_shell_for_euid() -> Option<String> {
    // SAFETY: geteuid takes no arguments and cannot fail; it is always
    // safe to call.
    let euid = unsafe { libc::geteuid() };

    // SAFETY: sysconf with a valid, always-recognized name; reads no
    // pointers, cannot fail in a way that is unsafe to observe (a
    // negative result just means "no hint available"). glibc's own docs
    // suggest 1 KiB as the fallback guess when no hint is available.
    let initial_len: usize = match unsafe { libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) } {
        hint if hint > 0 => hint as usize,
        _ => 1024,
    };

    lookup_with_growing_buffer(initial_len, euid, |buf| {
        // zeroed() is fine here: passwd is a C struct of pointers/ints/
        // longs, all of which are valid when all-zero-bits, and it is
        // fully overwritten by getpwuid_r before use on success.
        let mut pwd: libc::passwd = unsafe { std::mem::zeroed() };
        let mut result: *mut libc::passwd = std::ptr::null_mut();

        // SAFETY: `buf` is a valid, `buf.len()`-byte scratch region that
        // outlives this call (it is not touched again until after
        // `getpwuid_r` returns); `pwd` is a valid, writable `passwd` on
        // this stack frame; `result` is a valid out-pointer. `pwd`'s
        // string fields (including `pw_shell`, read below) point into
        // `buf`'s storage, so `buf` must not be dropped or reused before
        // those fields are read — it lives in this closure's stack frame
        // and is not reused until the next call, after `pwd` has already
        // been consumed.
        let rc =
            unsafe { libc::getpwuid_r(euid, &mut pwd, buf.as_mut_ptr(), buf.len(), &mut result) };

        if rc == libc::ERANGE {
            return PwAttempt::Erange;
        }
        if rc != 0 {
            return PwAttempt::Errno(rc);
        }
        if result.is_null() {
            // Not an error (rc == 0): glibc/musl both report "no such
            // entry" this way, e.g. a uid removed from passwd underneath
            // a still-running process.
            return PwAttempt::NoEntry;
        }

        // A degenerate passwd entry can leave pw_shell null on some
        // platforms; CStr::from_ptr on null is UB, so this check is
        // load-bearing, not defensive decoration.
        if pwd.pw_shell.is_null() {
            tracing::warn!(euid, "passwd entry has a null shell field");
            return PwAttempt::Shell(None);
        }
        // SAFETY: result is non-null, and both glibc and musl guarantee
        // that when non-null it points at `pwd` (populated above); the
        // string fields it references, including pw_shell (null-checked
        // above), live in `buf`, which is still in scope and unmodified.
        let shell = unsafe { std::ffi::CStr::from_ptr(pwd.pw_shell) };
        PwAttempt::Shell(match shell.to_str() {
            Ok(s) if !s.is_empty() => Some(s.to_string()),
            Ok(_) => {
                tracing::warn!(euid, "passwd entry has an empty shell field");
                None
            }
            Err(e) => {
                tracing::warn!(euid, error = %e, "passwd pw_shell is not valid UTF-8");
                None
            }
        })
    })
}

/// Outcome of a single `getpwuid_r`-shaped lookup attempt, reported by the
/// closure passed to [`lookup_with_growing_buffer`] so the retry driver
/// stays generic over libc.
enum PwAttempt {
    /// The scratch buffer was too small; the driver should grow it and
    /// call again.
    Erange,
    /// `getpwuid_r` returned a nonzero errno other than `ERANGE`.
    Errno(i32),
    /// The call succeeded (`rc == 0`) but reported no entry for the key.
    NoEntry,
    /// An entry was found. `None` means its shell field was present but
    /// unusable (null, empty, or not valid UTF-8) — the closure is
    /// responsible for logging why in that case, since only it has the
    /// raw field to describe.
    Shell(Option<String>),
}

/// Drive a `getpwuid_r`-shaped lookup through the ERANGE buffer-growth
/// retry protocol, generic over the actual libc call via `call` so the
/// retry logic itself is testable without a real passwd database.
///
/// Owns the parts of the protocol that have nothing to do with libc:
/// clamping the initial size hint (`initial_len` may come from an
/// unbounded `sysconf` hint), doubling the buffer on `Erange` up to a
/// 1 MiB cap — past which a pathological or hostile NSS backend could
/// otherwise grow this allocation without bound — and logging each
/// terminal, non-success outcome. `euid` is carried through purely to
/// label those log lines; the driver never inspects it otherwise.
fn lookup_with_growing_buffer(
    initial_len: usize,
    euid: libc::uid_t,
    mut call: impl FnMut(&mut Vec<libc::c_char>) -> PwAttempt,
) -> Option<String> {
    const MAX_BUF_LEN: usize = 1024 * 1024;
    let mut buf_len = initial_len.min(MAX_BUF_LEN);

    loop {
        let mut buf: Vec<libc::c_char> = vec![0; buf_len];
        match call(&mut buf) {
            PwAttempt::Erange => {
                if buf_len >= MAX_BUF_LEN {
                    tracing::warn!(
                        euid,
                        buf_len,
                        "getpwuid_r buffer exceeded 1 MiB cap; giving up on passwd lookup"
                    );
                    return None;
                }
                buf_len = (buf_len * 2).min(MAX_BUF_LEN);
            }
            PwAttempt::Errno(errno) => {
                tracing::warn!(euid, errno, "getpwuid_r failed");
                return None;
            }
            PwAttempt::NoEntry => {
                tracing::warn!(euid, "no passwd entry found for this euid");
                return None;
            }
            PwAttempt::Shell(shell) => return shell,
        }
    }
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

    /// A smoke test against whatever real passwd entry the test runner
    /// happens to have — it exercises the direct `getpwuid_r` path
    /// end-to-end, but proves nothing about the macOS or musl behavior
    /// the fallback chain was written for (CI runs glibc Linux, where the
    /// `getent` rung in [`passwd_shell`] would normally win first).
    /// Containers and user namespaces can legitimately run under a euid
    /// with no passwd entry at all, which production code already treats
    /// as `None`, so this only asserts shape when a result comes back.
    #[test]
    fn passwd_shell_for_euid_resolves_to_absolute_path() {
        if let Some(shell) = passwd_shell_for_euid() {
            assert!(
                shell.starts_with('/'),
                "expected an absolute path, got {shell:?}"
            );
        }
    }

    /// `getent`'s `name:passwd:uid:gid:gecos:home:shell` format puts the
    /// shell last; this is the only contract [`parse_getent_passwd_line`]
    /// relies on, so pin it against the documented field order rather
    /// than trusting the `rsplit` call by inspection alone.
    #[test]
    fn parse_getent_passwd_line_extracts_last_field() {
        assert_eq!(
            parse_getent_passwd_line("root:x:0:0:root:/root:/bin/bash"),
            Some("/bin/bash".to_string())
        );
    }

    /// A trailing-empty shell field (account exists but has no shell set)
    /// must be treated the same as `getent` producing nothing at all, so
    /// the caller falls through to the `getpwuid_r` rung instead of
    /// "resolving" to an empty string.
    #[test]
    fn parse_getent_passwd_line_rejects_empty_shell() {
        assert_eq!(parse_getent_passwd_line("root:x:0:0:root:/root:"), None);
    }

    /// `ERANGE` must actually grow the buffer passed to the next attempt,
    /// not just retry with the same undersized allocation — that would be
    /// an infinite loop against a real too-small hint. The closure
    /// records every buffer length it was handed so the test can verify
    /// growth directly instead of trusting the return value alone.
    #[test]
    fn lookup_with_growing_buffer_grows_on_erange_then_succeeds() {
        let mut seen_lens = Vec::new();
        let shell = lookup_with_growing_buffer(64, 0, |buf| {
            seen_lens.push(buf.len());
            if seen_lens.len() == 1 {
                PwAttempt::Erange
            } else {
                PwAttempt::Shell(Some("/bin/zsh".to_string()))
            }
        });
        assert_eq!(shell, Some("/bin/zsh".to_string()));
        assert_eq!(seen_lens, vec![64, 128]);
    }

    /// A backend that never stops returning `ERANGE` (a hostile or
    /// simply broken NSS module) must still terminate and must never
    /// grow the scratch allocation past the 1 MiB cap — the whole point
    /// of the cap is to bound this exact scenario. The closure records
    /// every length it saw so the assertion checks the cap directly
    /// rather than trusting that the loop merely returned eventually.
    #[test]
    fn lookup_with_growing_buffer_gives_up_past_cap() {
        let mut seen_lens = Vec::new();
        let shell = lookup_with_growing_buffer(1024, 0, |buf| {
            seen_lens.push(buf.len());
            PwAttempt::Erange
        });
        assert_eq!(shell, None);
        assert!(
            seen_lens.iter().all(|&len| len <= 1024 * 1024),
            "buffer length exceeded the 1 MiB cap: {seen_lens:?}"
        );
        assert_eq!(*seen_lens.last().unwrap(), 1024 * 1024);
    }

    /// A non-`ERANGE` errno is a hard failure the driver must not retry —
    /// distinguishing it from `Erange` is the entire reason `PwAttempt`
    /// has separate variants instead of a bare `Result<_, i32>`.
    #[test]
    fn lookup_with_growing_buffer_errno_returns_none() {
        let shell = lookup_with_growing_buffer(64, 0, |_| PwAttempt::Errno(13));
        assert_eq!(shell, None);
    }

    /// `rc == 0` with a null result pointer means "no such passwd entry",
    /// which must resolve the same way as any other unresolvable lookup
    /// (`None`), not be mistaken for success with an empty shell.
    #[test]
    fn lookup_with_growing_buffer_no_entry_returns_none() {
        let shell = lookup_with_growing_buffer(64, 0, |_| PwAttempt::NoEntry);
        assert_eq!(shell, None);
    }

    /// An entry can exist yet carry an unusable shell field (null, empty,
    /// or non-UTF-8); the closure signals this as `Shell(None)`, and the
    /// driver must pass that through as "no shell" rather than treating
    /// "entry found" as proof a shell was found.
    #[test]
    fn lookup_with_growing_buffer_shell_none_returns_none() {
        let shell = lookup_with_growing_buffer(64, 0, |_| PwAttempt::Shell(None));
        assert_eq!(shell, None);
    }
}
