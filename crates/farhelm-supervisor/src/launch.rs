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

use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The environment marker every launched session carries: its own session
/// id, set by the shim just before `exec`. Two independent consumers rely
/// on it, both documented in SPEC_impl.md/lore/2026-07-27-m2-process-tree-
/// stop.md: it is the per-session spawn credential's session identifier,
/// and — the reason it exists at THIS layer rather than only being passed
/// as an argument — `kill_process_tree` (service.rs) scans same-user
/// `/proc/*/environ` for an exact `FARHELM_SESSION_ID=<id>` entry to find
/// descendants that already reparented to init before a PPID walk could
/// see them. Environment variables are inherited across fork and exec by
/// the kernel, so setting this once here reaches every process the agent
/// ever spawns, transitively, with no further plumbing — UNLESS a
/// descendant deliberately scrubs or replaces its own environment before
/// spawning further children, which escapes the marker scan; that
/// residual is accepted until M3's cgroups hardening (see
/// `kill_process_tree`'s docs in service.rs).
pub const SESSION_ID_ENV_VAR: &str = "FARHELM_SESSION_ID";

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
    /// This session's id, injected into the agent's environment as
    /// [`SESSION_ID_ENV_VAR`] — see that constant's docs for why.
    pub session_id: String,
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

/// Where a session's per-launch spec file lives:
/// `<state_dir>/launch/<id>.json`.
///
/// Named by SESSION id, not by a per-launch id: today a session gets
/// exactly one spec-file path for its whole lifetime, so a relaunch
/// (PR8's territory) writes a fresh spec to this SAME path, silently
/// superseding whatever an earlier failed launch left behind. That is
/// also why a stale sentinel from launch N cannot survive to be
/// misattributed to launch N+1 without deliberate handling — today's half
/// of that handling is that a CONSUMED sentinel is deleted once its
/// `Error` outcome commits durably (`service.rs`'s `reload_sessions` and
/// `ListSessions` handler both document the lifecycle and the rationale);
/// PR8's own seam note there covers the other half, an UNCONSUMED
/// sentinel superseded by a fresh launch before anything ever read it.
/// `service.rs`'s `create_session` and the sentinel reader both call this
/// SAME function so the two sides can never compute diverging paths for
/// the same session.
pub fn spec_path_for_session(state_dir: &Path, id: &str) -> PathBuf {
    state_dir.join("launch").join(format!("{id}.json"))
}

/// Read a session's CURRENT-launch exec-failure sentinel, if one exists.
///
/// `Ok(None)` is the ordinary case: no launch failure is known for this
/// session, which covers both "never failed" and "the shim has not run
/// yet" — this function draws no distinction between them, exactly like
/// [`LaunchSpec::status_file`]'s own docs on presence-vs-absence.
///
/// A read failure that is NOT "the file does not exist" — a permission
/// error, the path naming a directory instead of a file, content that
/// fails to decode as UTF-8, or content that decodes but is empty or pure
/// whitespace — is returned as `Err` rather than folded into `Ok(None)`.
/// This is deliberate, not over-caution: the sentinel is written through
/// [`crate::files::write_durable_sync`], the durability-bearing tier
/// (`crate::files` module docs), whose entire contract is that a reader
/// only ever observes complete old content, complete new content, or
/// nothing at all — and [`record_launch_failure`] never writes an empty
/// report (every call site passes a real, non-empty message). A torn,
/// corrupt, or empty sentinel is therefore a state this tier is supposed
/// to make impossible, so encountering one means that promise has been
/// violated somewhere beneath this function (a hand-edited state dir, a
/// corrupted filesystem, a filesystem that lied about `fsync`) — exactly
/// the class of anomaly this crate treats loudly elsewhere
/// (`LastOutcome::from_columns`'s refusal to guess at a corrupt row is the
/// same instinct applied to SQLite instead of a plain file). Silently
/// treating any of these as "no sentinel" would let a launch that truly
/// failed read back as a plain, wrong `Exited` — precisely the conversion
/// PLAN_M3.md item 5 forbids.
pub fn read_launch_sentinel(state_dir: &Path, id: &str) -> anyhow::Result<Option<String>> {
    let status_path = status_path_for_spec(&spec_path_for_session(state_dir, id));
    match std::fs::read(&status_path) {
        Ok(bytes) => {
            let content = String::from_utf8(bytes).with_context(|| {
                format!(
                    "launch sentinel at {} is not valid UTF-8; the durability-bearing write \
                     tier should make a torn or corrupt sentinel impossible, so this means \
                     something has gone wrong beneath that promise",
                    status_path.display()
                )
            })?;
            if content.trim().is_empty() {
                anyhow::bail!(
                    "launch sentinel at {} is empty or pure whitespace; the shim never writes \
                     an empty report, so this is corrupt state, not evidence of anything",
                    status_path.display()
                );
            }
            Ok(Some(content))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(anyhow::Error::from(e).context(format!(
            "reading launch sentinel at {} (durability-bearing tier; expected to fail only by \
             absence)",
            status_path.display()
        ))),
    }
}

/// Deterministically derive a launch's SENTINEL path from its SPEC path —
/// pure, requiring no JSON parsing — so the shim can still report a
/// launch failure when the spec itself cannot be read, or turns out to
/// be malformed, unreadable, or empty. Without this, those failure modes
/// have no known status-file path to write to at all (the path only
/// lives INSIDE the parsed JSON), and silently degrade to a plain
/// "exited" classification later — exactly the conversion PLAN_M3.md item
/// 5 forbids. `service.rs`'s `create_session` derives this SAME path the
/// same way when it first writes the spec, so the two sides always agree
/// without the spec's own payload needing to restate it.
pub fn status_path_for_spec(spec_path: &Path) -> PathBuf {
    spec_path.with_extension("status")
}

/// The shim body: exec the spec's argv, recording failure to the status
/// file first. Lives here (not in the bin crate) so it is unit-testable;
/// `farhelm internal launch` is a thin caller. On success this function
/// never returns. Delegates to [`exec_launch_spec_with_seam`] with the
/// real filesystem; see that function for the actual logic.
pub fn exec_launch_spec(spec_path: &Path) -> anyhow::Error {
    exec_launch_spec_with_seam(spec_path, &crate::files::RealFs)
}

/// [`exec_launch_spec`]'s real body, parameterized over the write-
/// atomicity seam so a test can inject a failure into the sentinel write
/// itself while driving the ACTUAL production shim logic — not a
/// synthetic call directly into `crate::files` that merely resembles it
/// (PLAN_M3.md item 5's seam requirement: every tier-owed window driven
/// through each real call site).
///
/// EVERY early-return path here writes a sentinel via
/// [`status_path_for_spec`]'s derived path, not just the exec-failure one
/// at the bottom: a spec that cannot even be read, or that parses into
/// garbage, or whose argv is empty, must classify as error exactly as
/// surely as a real exec failure does. Before this, those paths returned
/// bare, unrecorded errors — invisible to any classifier that only ever
/// looks for a sentinel file.
pub fn exec_launch_spec_with_seam(
    spec_path: &Path,
    seam: &dyn crate::files::FaultSeam,
) -> anyhow::Error {
    use std::os::unix::process::CommandExt;

    let status_path = status_path_for_spec(spec_path);

    let bytes = match std::fs::read(spec_path) {
        Ok(bytes) => bytes,
        Err(e) => {
            let report = record_launch_failure(
                &status_path,
                format!("launch spec unreadable at {}: {e}", spec_path.display()),
                seam,
            );
            return anyhow::Error::from(e).context(report);
        }
    };
    // Unlink the moment the bytes are in hand — before parsing, not just
    // before exec: the spec holds the agent's full command line, which
    // users do put credentials into, and an early return on a malformed
    // spec must not leave it on disk. Nothing else removes it during
    // this supervisor's lifetime; only the next restart's sweep would.
    if let Err(e) = std::fs::remove_file(spec_path) {
        let report = record_launch_failure(
            &status_path,
            format!("removing launch spec {}: {e}", spec_path.display()),
            seam,
        );
        return anyhow::Error::from(e).context(report);
    }
    let spec: LaunchSpec = match serde_json::from_slice(&bytes) {
        Ok(spec) => spec,
        Err(e) => {
            let report = record_launch_failure(
                &status_path,
                format!("launch spec at {} is malformed: {e}", spec_path.display()),
                seam,
            );
            return anyhow::Error::from(e).context(report);
        }
    };
    if spec.argv.is_empty() {
        let report =
            record_launch_failure(&status_path, "launch spec has empty argv".to_string(), seam);
        return anyhow::anyhow!(report);
    }
    debug_assert_eq!(
        spec.status_file, status_path,
        "a spec's own status_file field must always match what status_path_for_spec derives \
         from its own path — service.rs's create_session and this derivation must never \
         disagree, or the exec-failure path below would write to the wrong place"
    );
    // exec only returns on failure. `Command::env` adds to (never clears)
    // the shim's own inherited environment, so this joins the rc-file
    // variables the login shell already sourced rather than replacing
    // them — SPEC.md's environment contract is otherwise untouched.
    let err = std::process::Command::new(&spec.argv[0])
        .args(&spec.argv[1..])
        .env(SESSION_ID_ENV_VAR, &spec.session_id)
        .exec();
    let report = format!(
        "exec_failed argv0={} errno={}",
        spec.argv[0],
        err.raw_os_error().unwrap_or(-1)
    );
    let report = record_launch_failure(&status_path, report, seam);
    anyhow::Error::from(err).context(report)
}

/// Write `report` as this launch's durable sentinel (`crate::files`
/// module docs: the exec-failure sentinel is the durability-bearing
/// tier), returning either the bare report or one augmented with the
/// write's own failure detail. Shared by every early-return path in
/// [`exec_launch_spec_with_seam`], not just the exec-failure one at the
/// bottom, so every launch-failure class gets identical durable evidence.
fn record_launch_failure(
    status_path: &Path,
    report: String,
    seam: &dyn crate::files::FaultSeam,
) -> String {
    match crate::files::write_durable_sync(status_path, report.as_bytes(), seam) {
        Ok(()) => report,
        Err(write_error) => format!(
            "{report}; could not record launch failure at {}: {write_error}",
            status_path.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// The ordinary case: no launch has ever failed for this session, so
    /// there is no sentinel file at all, and the reader must say so
    /// plainly rather than treating a missing file as any kind of error.
    #[test]
    fn read_launch_sentinel_is_none_when_nothing_was_ever_written() {
        let tmp = tempfile::tempdir().unwrap();
        let result = read_launch_sentinel(tmp.path(), "never-launched").unwrap();
        assert_eq!(result, None);
    }

    /// The reader's whole reason to exist: surfacing exactly what
    /// [`exec_launch_spec`] left behind, at the SAME path
    /// [`spec_path_for_session`]/[`status_path_for_spec`] derive
    /// independently — proving the two sides of this contract (the
    /// writer inside the shim, the reader inside the supervisor) actually
    /// agree on where a launch's sentinel lives.
    #[test]
    fn read_launch_sentinel_reads_back_what_the_shim_wrote() {
        let tmp = tempfile::tempdir().unwrap();
        let id = "session-1";
        let spec_path = spec_path_for_session(tmp.path(), id);
        std::fs::create_dir_all(spec_path.parent().unwrap()).unwrap();
        let missing_binary = tmp.path().join("no-such-farhelm-test-binary");
        let spec = LaunchSpec {
            argv: vec![missing_binary.to_string_lossy().into_owned()],
            status_file: status_path_for_spec(&spec_path),
            session_id: id.to_string(),
        };
        std::fs::write(&spec_path, serde_json::to_vec(&spec).unwrap()).unwrap();
        exec_launch_spec(&spec_path);

        // Compared against the file's FULL content, not merely `contains`:
        // the reader's whole job is to hand back exactly what is on disk,
        // and a substring check would not catch a reader that silently
        // truncated, trimmed, or otherwise mangled the bytes on the way
        // through.
        let status_path = status_path_for_spec(&spec_path);
        let on_disk = std::fs::read_to_string(&status_path).expect("sentinel must exist on disk");
        let sentinel = read_launch_sentinel(tmp.path(), id)
            .expect("read must succeed")
            .expect("a sentinel must exist after the exec failure above");
        assert_eq!(
            sentinel, on_disk,
            "the reader must hand back the exact bytes the shim wrote, not a paraphrase"
        );
        assert!(sentinel.contains("exec_failed"));
    }

    /// Item 17's directory case: the status path naming a DIRECTORY rather
    /// than a file is a different `read` failure than a plain permission
    /// error (`EISDIR`, not `EACCES`), and must get the same loud,
    /// contextual `Err` treatment as every other non-`NotFound` failure —
    /// never folded into `Ok(None)`, which would silently convert a real
    /// anomaly (something else has clobbered this launch's sentinel path)
    /// into "no launch failure known".
    #[test]
    fn read_launch_sentinel_reports_a_directory_at_the_status_path_loudly() {
        let tmp = tempfile::tempdir().unwrap();
        let id = "session-dir";
        let status_path = status_path_for_spec(&spec_path_for_session(tmp.path(), id));
        std::fs::create_dir_all(&status_path).unwrap();

        let err = read_launch_sentinel(tmp.path(), id)
            .expect_err("a directory at the sentinel path must be a hard error, not absence");
        assert!(
            format!("{err:#}").contains("reading launch sentinel"),
            "must name what it was doing, not just relay a bare OS error: {err:#}"
        );
    }

    /// Item 20: the shim never writes an empty report (every
    /// `record_launch_failure` call site passes a real message), so an
    /// empty or pure-whitespace sentinel is corrupt state — the same class
    /// of anomaly as invalid UTF-8 — and must be refused just as loudly,
    /// not treated as "no sentinel" (`Ok(None)`) nor as an empty-but-valid
    /// detail string.
    #[test]
    fn read_launch_sentinel_rejects_empty_and_whitespace_only_content() {
        let tmp = tempfile::tempdir().unwrap();
        for (id, content) in [("empty", ""), ("blank", "   \n\t  ")] {
            let status_path = status_path_for_spec(&spec_path_for_session(tmp.path(), id));
            std::fs::create_dir_all(status_path.parent().unwrap()).unwrap();
            std::fs::write(&status_path, content).unwrap();

            let err = read_launch_sentinel(tmp.path(), id)
                .expect_err("an empty or whitespace-only sentinel must be a hard error");
            assert!(
                format!("{err:#}").contains("empty or pure whitespace"),
                "{id}: {err:#}"
            );
        }
    }

    /// The loud half of the reader's contract (its own docs): a sentinel
    /// that fails to decode as UTF-8 is exactly the anomaly the
    /// durability-bearing write tier is supposed to make impossible, so
    /// it must come back as `Err`, never be folded into `Ok(None)` as if
    /// nothing had ever gone wrong. A silent "no sentinel" here would let
    /// a genuine launch failure read back as a plain, wrong `Exited`.
    #[test]
    fn read_launch_sentinel_refuses_to_treat_invalid_utf8_as_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let id = "session-2";
        let status_path = status_path_for_spec(&spec_path_for_session(tmp.path(), id));
        std::fs::create_dir_all(status_path.parent().unwrap()).unwrap();
        std::fs::write(&status_path, [0xff, 0xfe, 0xfd]).unwrap();

        let err = read_launch_sentinel(tmp.path(), id)
            .expect_err("invalid UTF-8 must be a hard error, not a silent absence");
        assert!(format!("{err:#}").contains("not valid UTF-8"));
    }

    /// The shim's own end-to-end path from a spec on disk to a durable
    /// sentinel: an argv0 GUARANTEED not to exist (a name inside this
    /// test's own fresh tempdir, never created — item 23: the old version
    /// of this test used a hardcoded `/nonexistent/...` path that the
    /// real filesystem is never actually guaranteed to lack) makes the
    /// real `exec` fail with `ENOENT`, and this pins that the sentinel
    /// [`exec_launch_spec`] leaves behind is exactly what a later
    /// classifier will read — 0600, containing the errno, at the path the
    /// spec named — via the same code path production uses, not a
    /// synthetic call into `crate::files` directly.
    #[test]
    fn exec_launch_spec_writes_a_durable_sentinel_on_exec_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("spec.json");
        let missing_binary = tmp.path().join("no-such-farhelm-test-binary");
        let spec = LaunchSpec {
            argv: vec![missing_binary.to_string_lossy().into_owned()],
            status_file: status_path_for_spec(&spec_path),
            session_id: "test-session".to_string(),
        };
        std::fs::write(&spec_path, serde_json::to_vec(&spec).unwrap()).unwrap();

        let err = exec_launch_spec(&spec_path);
        assert!(
            format!("{err:#}").contains("exec_failed"),
            "error chain should surface the exec-failure report: {err:#}"
        );

        let status_path = status_path_for_spec(&spec_path);
        let content = std::fs::read_to_string(&status_path).unwrap();
        assert!(
            content.contains("exec_failed")
                && content.contains(&missing_binary.to_string_lossy()[..]),
            "sentinel content must name the exec failure and the argv0 that failed: {content:?}"
        );
        let mode = std::fs::metadata(&status_path)
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "sentinel must be 0600, got {mode:o}");

        // The spec itself must always be gone by this point, success or
        // failure — it carries the agent's full command line.
        assert!(
            !spec_path.exists(),
            "launch spec must be unlinked once read"
        );
    }

    /// The EACCES half of "exec fails" (item 23), distinct from the
    /// ENOENT case above: a real, existing file with no execute bit set
    /// at all makes `exec` fail for a completely different reason
    /// (permissions, not absence), and both must classify identically —
    /// a durable sentinel naming the failure, never a silent "exited".
    #[test]
    fn exec_launch_spec_writes_a_durable_sentinel_on_a_non_executable_file() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("spec.json");
        let non_executable = tmp.path().join("not-executable");
        std::fs::write(&non_executable, b"not a real binary").unwrap();
        std::fs::set_permissions(&non_executable, std::fs::Permissions::from_mode(0o600)).unwrap();
        let spec = LaunchSpec {
            argv: vec![non_executable.to_string_lossy().into_owned()],
            status_file: status_path_for_spec(&spec_path),
            session_id: "test-session".to_string(),
        };
        std::fs::write(&spec_path, serde_json::to_vec(&spec).unwrap()).unwrap();

        let err = exec_launch_spec(&spec_path);
        assert!(format!("{err:#}").contains("exec_failed"));

        let content = std::fs::read_to_string(status_path_for_spec(&spec_path)).unwrap();
        assert!(
            content.contains("exec_failed"),
            "a non-executable target must still classify as a recorded exec failure: {content:?}"
        );
    }

    /// Item 21's core regression: a MISSING spec (the login shell reached
    /// the shim, but the spec file it names is simply not there — a
    /// supervisor-side bug, a race, or a sweep gone wrong) must still
    /// leave a durable sentinel behind at the path derived from the spec
    /// path alone. Before this fix, `exec_launch_spec` returned bare on a
    /// read failure with NOTHING written anywhere — invisible to any
    /// later classifier, silently converting what should be "error" into
    /// "exited" (there being no evidence of a launch attempt at all).
    #[test]
    fn exec_launch_spec_records_a_sentinel_for_a_missing_spec() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("spec.json");
        // Never created: this is the whole point of the test.

        let err = exec_launch_spec(&spec_path);
        assert!(format!("{err:#}").contains("unreadable"));

        let status_path = status_path_for_spec(&spec_path);
        let content = std::fs::read_to_string(&status_path).unwrap();
        assert!(
            content.contains("unreadable"),
            "a missing spec must still leave a durable failure sentinel: {content:?}"
        );
    }

    /// Item 21's other core regression: a spec that EXISTS but is not
    /// valid JSON (truncated, corrupted, or simply garbage) must ALSO
    /// leave a durable sentinel — the failure is discovered only after
    /// parsing, but the destination path is derived from `spec_path`
    /// alone (never from the unparseable content), so it is known
    /// regardless of how badly the JSON is broken.
    #[test]
    fn exec_launch_spec_records_a_sentinel_for_a_malformed_spec() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("spec.json");
        std::fs::write(&spec_path, b"this is not json").unwrap();

        let err = exec_launch_spec(&spec_path);
        assert!(format!("{err:#}").contains("malformed"));

        let content = std::fs::read_to_string(status_path_for_spec(&spec_path)).unwrap();
        assert!(content.contains("malformed"));
        // Malformed-but-present specs are still unlinked before parsing
        // is attempted — see the function's own docs on why.
        assert!(!spec_path.exists());
    }

    /// The empty-argv case, the one launch-failure class that was ALREADY
    /// reachable from valid JSON before item 21: it must get the same
    /// durable-sentinel treatment as every other failure branch, not the
    /// bare unrecorded error the pre-fix code returned.
    #[test]
    fn exec_launch_spec_records_a_sentinel_for_empty_argv() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("spec.json");
        let spec = LaunchSpec {
            argv: vec![],
            status_file: status_path_for_spec(&spec_path),
            session_id: "test-session".to_string(),
        };
        std::fs::write(&spec_path, serde_json::to_vec(&spec).unwrap()).unwrap();

        let err = exec_launch_spec(&spec_path);
        assert!(format!("{err:#}").contains("empty argv"));

        let content = std::fs::read_to_string(status_path_for_spec(&spec_path)).unwrap();
        assert!(content.contains("empty argv"));
    }

    /// The seam-injectable real call site (items 7/8): a failure injected
    /// into the sentinel write ITSELF, driven through
    /// [`exec_launch_spec_with_seam`] — the actual production shim
    /// logic, not a standalone call into `crate::files` — must still
    /// leave the shim's own error naming the exec failure (the root cause
    /// the caller most needs), with the write failure folded in as
    /// additional context, and must never leave a torn sentinel: absent
    /// before the write's publish step, complete at or after it. This
    /// supersedes the earlier, generic-duplicate version of this test
    /// that bypassed the shim entirely.
    #[test]
    fn exec_launch_spec_with_seam_reports_both_failures_without_a_torn_sentinel() {
        struct FailAtWrite;
        impl crate::files::FaultSeam for FailAtWrite {
            fn write(&self, _file: &mut std::fs::File, _bytes: &[u8]) -> std::io::Result<()> {
                Err(std::io::Error::other("injected sentinel-write failure"))
            }
        }

        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("spec.json");
        let missing_binary = tmp.path().join("no-such-farhelm-test-binary");
        let spec = LaunchSpec {
            argv: vec![missing_binary.to_string_lossy().into_owned()],
            status_file: status_path_for_spec(&spec_path),
            session_id: "test-session".to_string(),
        };
        std::fs::write(&spec_path, serde_json::to_vec(&spec).unwrap()).unwrap();

        let err = exec_launch_spec_with_seam(&spec_path, &FailAtWrite);
        let rendered = format!("{err:#}");
        assert!(
            rendered.contains("exec_failed"),
            "the exec failure remains the reported root cause: {rendered}"
        );
        assert!(
            rendered.contains("could not record launch failure"),
            "the sentinel write's own failure must be surfaced too: {rendered}"
        );
        assert!(
            !status_path_for_spec(&spec_path).exists(),
            "a failure injected at the write step must never publish a torn sentinel"
        );
    }

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
