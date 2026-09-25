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
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

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
/// spawning further children, which escapes the marker scan. That
/// residual is what the per-launch cgroup scope closes where a systemd
/// user manager exists (`crate::scope`, `service.rs`'s
/// `reap_process_tree`), and remains open where none does — the marker is
/// still the only mechanism there, which is why it is set here rather
/// than left to the cgroup.
pub const SESSION_ID_ENV_VAR: &str = "FARHELM_SESSION_ID";

/// The bearer value proving which session requested a restricted spawn.
pub const SESSION_TOKEN_ENV_VAR: &str = "FARHELM_SESSION_TOKEN";

/// The exact local supervisor socket this launch belongs to.
///
/// This is an implementation detail for the bundled spawn CLI, not part of
/// the third-party environment contract. Falling back to a default state
/// directory would silently dial the wrong supervisor after `--state-dir`.
pub const SUPERVISOR_SOCK_ENV_VAR: &str = "FARHELM_SUPERVISOR_SOCK";

/// Current-launch controls consumed by Goose's persisted reporter.
pub const GOOSE_REPORTER_ENABLED_ENV_VAR: &str = "FARHELM_GOOSE_REPORTER_ENABLED";
pub const GOOSE_INSTRUCTIONS_ENV_VAR: &str = "FARHELM_GOOSE_INSTRUCTIONS";
pub const GOOSE_REPORTER_EXE_ENV_VAR: &str = "FARHELM_GOOSE_REPORTER_EXE";

/// Absolute reporter executable supplied only to Pi launches Farhelm injected.
pub const PI_REPORTER_EXE_ENV_VAR: &str = "FARHELM_PI_REPORTER_EXE";

/// Absolute reporter executable supplied only to OMP launches Farhelm
/// injected. Deliberately its own variable rather than Pi's: the two
/// harnesses are different processes and a launch must never be able to
/// cross-report through the other vendor's channel.
pub const OMP_REPORTER_EXE_ENV_VAR: &str = "FARHELM_OMP_REPORTER_EXE";

/// The environment marker every TERMINAL TAB carries on top of
/// [`SESSION_ID_ENV_VAR`]: the tab's own id (PLAN_M4.md item 2). Set by
/// tmux itself (`new-window -e`) rather than by a shim, because a tab has
/// no shim — its window command IS the user's login shell.
///
/// Its job is SELECTION: closing a tab must reap that tab's shell and
/// everything it left behind, and nothing else of the session, so the
/// close sweep scans for an exact `FARHELM_TAB_ID=<tab>` entry alongside
/// the session marker. It never selects on its own — see
/// [`AGENT_ID_ENV_VAR`] for the shape of the whole split, and
/// `service.rs`'s `SweepTarget` for the rules.
pub const TAB_ID_ENV_VAR: &str = "FARHELM_TAB_ID";

/// The environment marker every AGENT launch carries on top of
/// [`SESSION_ID_ENV_VAR`]: its own session id again, under a name that
/// says which KIND of terminal it belongs to (PLAN_M4.md item 2).
///
/// # Why both kinds are marked positively
///
/// Stop and restart must reap the agent and leave the session's tabs
/// running (SPEC.md); delete must take everything; close must take one
/// tab. Three different sets over the same session-marked processes, so
/// something has to tell them apart.
///
/// An earlier cut did that by EXCLUSION — sweep the session marker,
/// subtract anything wearing a tab marker — and it had a hole that only
/// shows up when farhelm supervises farhelm. A supervisor running inside
/// somebody's tab inherits that tab's `FARHELM_TAB_ID`, its tmux server
/// inherits it, and every INNER agent it launches inherits it too. Under
/// exclusion, every one of those inner agents looks like a tab process and
/// escapes its own session's stop entirely. Positive marking closes it:
/// an inner agent carries `FARHELM_AGENT_ID=<its own session>`, which is
/// what stop selects on, and the stale outer tab marker decides nothing.
///
/// # Why the legacy bucket still exists
///
/// A positive agent marker alone would silently stop reaching daemons left
/// by sessions launched BEFORE this build — they carry the session marker
/// and neither kind marker. So the stop sweep also claims exactly that
/// shape (`SweepTarget::AgentOnly`'s third root set): session-marked,
/// wearing neither marker.
///
/// # Scrubbing
///
/// Every launch boundary removes the OTHER kind's marker before setting
/// its own — the agent shim `env_remove`s [`TAB_ID_ENV_VAR`], a tab's
/// window command runs through `env -u FARHELM_AGENT_ID` — so an ambient
/// marker inherited from an outer farhelm can never survive into an inner
/// launch of the opposite kind and misfile it.
pub const AGENT_ID_ENV_VAR: &str = "FARHELM_AGENT_ID";

/// What the shim needs to launch the agent: written as JSON by the
/// supervisor, read by `farhelm internal launch` inside the session. A
/// file (not argv) so the invocation never fights shell quoting twice.
#[derive(Clone, Serialize, Deserialize)]
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
    /// Recoverable per-session credential, unchanged across relaunches.
    ///
    /// This bearer value must never enter logs. `LaunchSpec` deliberately
    /// has no derived `Debug` implementation so tracing the surrounding
    /// launch request cannot disclose it by accident.
    pub session_token: String,
    /// Exact unix socket path the spawn CLI must dial.
    pub supervisor_sock: PathBuf,
    /// Directory containing the supervisor's own `farhelm` binary.
    ///
    /// The shim prepends it to the PATH produced by login-shell startup,
    /// so an agent invoking `farhelm` by name reaches the same artifact
    /// that launched it before considering any ambient installation.
    pub farhelm_bin_dir: PathBuf,
    /// When present, the shim must prepare a git working copy inside this
    /// session's terminal — clone, run the post-clone hook, and only then
    /// exec the agent — before launching anything. `None` (the serde
    /// default, so specs written before this field existed still parse) is
    /// an ordinary launch: the shim's behavior is byte-for-byte what it
    /// was without preparation.
    ///
    /// This field travels INSIDE the spec because the spec is the one
    /// message the supervisor hands the shim, but nothing bookkeeping-
    /// related may live inside the checkout directory itself: git requires
    /// the clone target to be an empty, allocated directory, so every
    /// durable artifact (the state file, the flock) lives under the
    /// supervisor's own state tree, addressed by
    /// [`CheckoutPreparation::state_path`].
    #[serde(default)]
    pub preparation: Option<CheckoutPreparation>,
}

/// One session-terminal checkout preparation: clone a repository into the
/// session's allocated directory, run an already-resolved post-clone hook,
/// then let the ordinary agent launch proceed — all INSIDE the session's
/// terminal, so credential prompts and clone output are ordinary terminal
/// work the user can see and answer.
///
/// Built by the helm/supervisor from validated user input and carried in
/// [`LaunchSpec::preparation`]; the shim (this module's
/// [`exec_launch_spec_with_seam`]) is the only component that advances the
/// durable state behind [`CheckoutPreparation::state_path`].
#[derive(Clone, Serialize, Deserialize)]
pub struct CheckoutPreparation {
    /// Stable id for the state file and lock under supervisor state. A
    /// working-copy id (not the session id) because the preparation
    /// outlives relaunches of one session and two sessions must never
    /// prepare the same id concurrently — the flock (see
    /// [`acquire_preparation_lock`]) is keyed on this id.
    pub working_copy_id: String,
    /// The clone URL, already constructed by the helm from validated
    /// fields. Verified here defensively (shape, no shell metacharacters)
    /// before any process runs: fail closed on anything unexpected.
    pub clone_url: String,
    /// The allocated target directory, accepted as an absolute path. May
    /// contain spaces or shell-hostile characters — the shim runs argv,
    /// never shell text, so the path only needs to BE a directory, never
    /// survive a quoting round-trip.
    pub cwd: String,
    /// The directory's identity as captured at allocation time (device +
    /// inode). The shim re-stats and refuses on mismatch: the allocation
    /// could have been replaced between creation and launch, and cloning
    /// into whatever now sits at `cwd` would defeat the whole reservation.
    pub directory_device: u64,
    pub directory_inode: u64,
    /// The post-clone hook, ALREADY resolved (global/override decisions
    /// made by the helm). Run as `<shell> -c <hook>` with the checkout as
    /// the working directory; the shim deliberately does not shell-expand
    /// the repo identifier or cwd into it.
    pub post_clone: Option<String>,
    /// The resolved shell the hook runs under.
    pub shell: String,
    /// Supervisor-generated path of this preparation's persistent state
    /// file (`checkout-preparation/<working-copy-id>.json` under
    /// supervisor state). Generated by the supervisor so both sides — the
    /// create path that publishes `NotStarted` and the shim that advances
    /// it — share one derivation.
    pub state_path: PathBuf,
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
    // runtime's worker threads. The join wait is bounded even though the
    // blocking task itself cannot be cancelled once it has started.
    match tokio::time::timeout(
        NSS_LOOKUP_TIMEOUT,
        tokio::task::spawn_blocking(passwd_shell_for_euid),
    )
    .await
    {
        Ok(Ok(shell)) => shell,
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "passwd lookup task panicked");
            None
        }
        Err(_) => {
            tracing::warn!(
                "abandoned getpwuid_r passwd lookup after timeout; its blocking thread will finish on its own"
            );
            None
        }
    }
}

/// Bound NSS-dependent shell resolution so a wedged identity service cannot
/// hold every launch waiting for either lookup rung indefinitely.
const NSS_LOOKUP_TIMEOUT: Duration = Duration::from_secs(5);

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
    getent_passwd_shell_with_program(Path::new("getent"), NSS_LOOKUP_TIMEOUT).await
}

/// Run the host `getent` lookup through an injectable executable path and
/// time bound.
///
/// Production passes the bare command name so normal `PATH` lookup remains
/// unchanged, and [`NSS_LOOKUP_TIMEOUT`]. Tests use the seam to own a
/// sleeping stand-in without changing the test process's environment or its
/// `PATH`, and a short REAL bound: the bound races a real child process, so
/// a paused clock (which elapses the moment the runtime is idle, before the
/// child has even started) cannot prove the child was killed rather than
/// never run.
async fn getent_passwd_shell_with_program(program: &Path, bound: Duration) -> Option<String> {
    // SAFETY: geteuid takes no arguments and cannot fail; it is always
    // safe to call.
    let euid = unsafe { libc::geteuid() };

    let output = match tokio::time::timeout(
        bound,
        tokio::process::Command::new(program)
            .kill_on_drop(true)
            .arg("passwd")
            .arg(euid.to_string())
            .output(),
    )
    .await
    {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => {
            tracing::debug!(
                error = %e,
                "getent unavailable; falling back to direct passwd lookup"
            );
            return None;
        }
        Err(_) => {
            tracing::warn!(
                euid,
                "getent passwd timed out; falling back to direct passwd lookup"
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
/// `None` covers a line with no passwd separator and a trailing-empty
/// shell field. Neither provides a shell value, so the caller warns and
/// lets the direct `getpwuid_r` lookup try the same account. Requiring a
/// colon matters because without one the whole malformed line would
/// otherwise look like a shell when selecting the last field.
fn parse_getent_passwd_line(line: &str) -> Option<String> {
    if !line.contains(':') {
        return None;
    }

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
///
/// `scope_prefix` is the cgroup hardening (`crate::scope`), empty on every
/// host without a systemd user manager. It is spliced between the shell's
/// `exec` and the shim rather than anywhere else in the chain for two
/// reasons that pull in the same direction: the login shell must still be
/// the outermost thing tmux starts (SPEC.md's environment contract is what
/// `-l -i` delivers, and a scope around the SHELL would put the rc files'
/// own subprocesses in the cgroup too), while everything the shim
/// promises — its `exec` of the agent, its sentinel, its session-id marker
/// — must stay inside it. `systemd-run --scope` `exec`s in place
/// (`crate::scope`'s module docs record the empirical check), so the
/// resulting process tree is byte-for-byte the shape it was without the
/// prefix; nothing downstream of here can tell the difference.
pub fn window_command(
    shell: &str,
    farhelm_exe: &Path,
    spec_path: &Path,
    scope_prefix: Vec<String>,
) -> Vec<String> {
    // Quoting matters because both paths derive from $HOME or user flags
    // and can contain spaces or quotes; shell_words::quote is the same
    // POSIX single-quote encoding the invocation parser expects. The scope
    // prefix goes through the same quoting even though its words are all
    // literals this crate wrote: one encoder for the whole command line is
    // one fewer place for a future flag with a shell metacharacter in it
    // to break the launch.
    let mut words: Vec<String> = vec!["exec".to_string()];
    words.extend(scope_prefix);
    words.push(farhelm_exe.to_string_lossy().into_owned());
    words.push("internal".to_string());
    words.push("launch".to_string());
    words.push(spec_path.to_string_lossy().into_owned());
    let inner = words
        .iter()
        .map(|w| shell_words::quote(w).into_owned())
        .collect::<Vec<_>>()
        .join(" ");
    vec![
        shell.to_string(),
        "-l".to_string(),
        "-i".to_string(),
        "-c".to_string(),
        inner,
    ]
}

/// Build the tmux window command for a TERMINAL TAB: the user's shell as
/// an interactive login shell, and nothing else (PLAN_M4.md item 2).
///
/// The contrast with [`window_command`] is the whole design, not an
/// omission. An agent launch needs a shim between the shell and the agent
/// so a failed `exec` can leave a sentinel; a tab has nothing to exec —
/// the shell IS what the user asked for — so there is no spec, no shim,
/// no sentinel, and no error-vs-exited classification to make. A tab shell
/// that starts and later exits is just a dead pane (`remain-on-exit` keeps
/// it viewable), and a tab shell that never starts at all is caught at
/// open time by the pane-liveness check in the `OpenTab` handler.
///
/// `-l -i` is the same SSH-and-type contract the agent launch delivers,
/// and `-i` is equally load-bearing here: without it the shell skips
/// `.zshrc`/`.bashrc`, so a tab would not see the environment the user's
/// own terminals do.
///
/// `scope_prefix` wraps the SHELL, unlike [`window_command`] where it
/// wraps only the shim. That difference is deliberate and follows from the
/// same rule both obey — everything the operation promises to reap must be
/// inside the cgroup. For an agent that is the shim and its exec'd agent,
/// and keeping the login shell outside spares the rc files' own
/// subprocesses from the scope. For a tab, "kills that shell and its
/// processes" (SPEC.md) names the shell itself, so the shell must be in
/// the cgroup, rc-file subprocesses and all.
///
/// tmux runs this argv directly, so — unlike `window_command`'s `-c`
/// payload — nothing here is ever re-parsed by a shell and nothing needs
/// quoting.
///
/// The `env -u` is the tab side of the marker scrub ([`AGENT_ID_ENV_VAR`]).
/// tmux's own `-e` can only SET a variable, so removing an inherited one
/// takes a process that can: `env` is POSIX-mandated, and putting it
/// INNERMOST — after any scope prefix, immediately before the shell —
/// guarantees the shell and everything under it start without an ambient
/// agent marker from an outer farhelm, whatever the wrapper above did.
pub fn tab_window_command(shell: &str, scope_prefix: Vec<String>) -> Vec<String> {
    let mut argv = scope_prefix;
    argv.push("env".to_string());
    argv.push("-u".to_string());
    argv.push(AGENT_ID_ENV_VAR.to_string());
    argv.push(shell.to_string());
    argv.push("-l".to_string());
    argv.push("-i".to_string());
    argv
}

/// Where one LAUNCH's spec file lives:
/// `<state_dir>/launch/<id>.<generation>.json`.
///
/// Keyed by session id AND launch generation (`store::StoredSession::
/// generation`), which is the naming PLAN_M3.md item 3 asked for and item
/// 9's restart is what finally needs: a session can now launch many times,
/// and the two files a launch owns — this spec and the sentinel derived
/// from it — are evidence ABOUT ONE LAUNCH. Sharing one path across
/// launches made a stale sentinel and a fresh one indistinguishable except
/// by careful deletion, so a cleanup that failed (or a crash before it
/// ran) could paint a perfectly good relaunch as `error`. Different
/// generations are different paths, so that misattribution is not a race
/// to win but a state that cannot be constructed.
///
/// Generation 0 is a session's original launch, which is also what every
/// row predating the generation column carries after the schema migration
/// — so a session created by an older build finds its files exactly where
/// this build looks for them.
///
/// `service.rs`'s launch path and the sentinel reader both call this SAME
/// function, so the two sides can never compute diverging paths for the
/// same launch.
pub fn spec_path_for_launch(state_dir: &Path, id: &str, generation: i64) -> PathBuf {
    state_dir
        .join("launch")
        .join(format!("{id}.{generation}.json"))
}

/// Split a launch-directory spec file NAME back into the session id and
/// generation that produced it, or `None` when it is not one of ours.
///
/// The inverse of [`spec_path_for_launch`], and it exists for exactly one
/// caller: the startup sweep, which must decide whether a leftover file
/// belongs to a session that still exists. A session id is a UUID and
/// contains no `.`, so splitting on the LAST two dot-separated components
/// is unambiguous; anything that does not parse is left alone rather than
/// guessed at, since an unrecognized file in this directory is not this
/// code's to delete.
pub fn parse_launch_file_name(name: &str) -> Option<(&str, i64)> {
    let (rest, extension) = name.rsplit_once('.')?;
    if extension != "json" && extension != "status" {
        return None;
    }
    let (id, generation) = rest.rsplit_once('.')?;
    let generation = generation.parse().ok()?;
    (!id.is_empty()).then_some((id, generation))
}

/// Read one LAUNCH's exec-failure sentinel, if one exists.
///
/// `generation` names the launch (see [`spec_path_for_launch`]): a caller
/// passes the generation of the run it is classifying, so a sentinel left
/// by an earlier launch is not merely ignored but unreachable.
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
pub fn read_launch_sentinel(
    state_dir: &Path,
    id: &str,
    generation: i64,
) -> anyhow::Result<Option<String>> {
    let status_path = status_path_for_spec(&spec_path_for_launch(state_dir, id, generation));
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
    // Checkout preparation (session-terminal clone + post-clone hook),
    // when this launch carries one. Every abort path inside has already
    // written the durable sentinel and printed the actionable error to
    // the terminal, so an Err here is just "do not exec, report and
    // exit"; the terminal and any partial checkout are left in place for
    // inspection, exactly like a failed exec.
    if let Some(preparation) = &spec.preparation
        && let Err(e) = run_checkout_preparation(&spec, preparation, &status_path, seam)
    {
        return e;
    }
    // Nothing is written to the terminal before `exec`. The shim once
    // painted a "preamble" here — the previous run's captured last screen,
    // handed over by a restart — and SPEC.md now forbids exactly that: a
    // frame with no process behind it looks live, accepts typing, and is
    // overwritten when the real program draws. The agent's own first byte
    // is the first thing the relaunched pane shows.
    //
    // `exec` only returns on failure. `Command::env` adds to (never clears)
    // the shim's own inherited environment, so this joins the rc-file
    // variables the login shell already sourced rather than replacing
    // them — SPEC.md's environment contract is otherwise untouched.
    //
    // The one thing deliberately REMOVED is the opposite kind's marker
    // (see [`AGENT_ID_ENV_VAR`]): a supervisor running inside somebody's
    // farhelm tab passes that tab's `FARHELM_TAB_ID` down through its own
    // tmux server, and an inner agent still wearing it would be filed as a
    // tab process by its own session's stop sweep and never reaped. This
    // is the exec that ends that inheritance.
    let err = agent_command(&spec).exec();
    let report = format!(
        "exec_failed argv0={} errno={}",
        spec.argv[0],
        err.raw_os_error().unwrap_or(-1)
    );
    let report = record_launch_failure(&status_path, report, seam);
    anyhow::Error::from(err).context(report)
}

/// Build the final agent command, including Farhelm's private launch
/// contract and the truecolor capability agents need to match the browser.
/// The inherited environment remains untouched outside this child process.
fn agent_command(spec: &LaunchSpec) -> std::process::Command {
    let mut command = launch_child_command(&spec.argv[0], spec);
    command
        .args(&spec.argv[1..])
        .env(SESSION_TOKEN_ENV_VAR, &spec.session_token)
        .env(SUPERVISOR_SOCK_ENV_VAR, &spec.supervisor_sock);
    command
}

/// The environment and PATH contract every child process the launch shim
/// creates shares: the supervisor binary's directory wins PATH resolution
/// (so every child — agent, `git`, post-clone hook — invoking `farhelm`
/// by name reaches the same artifact that launched it), the session id
/// marker and the AGENT-kind marker (both [`SESSION_ID_ENV_VAR`] and
/// [`AGENT_ID_ENV_VAR`] carry the session id, so Stop/Delete's process
/// sweeps find a git clone or a hung hook exactly as they find the agent
/// itself), the truecolor capability, and the removal of the opposite
/// kind's markers (the same scrub [`AGENT_ID_ENV_VAR`]'s docs describe
/// for the exec below the shim — a git clone inside an agent's tab must
/// not inherit that tab's `FARHELM_TAB_ID` either).
///
/// The bearer spawn credential ([`SESSION_TOKEN_ENV_VAR`]) and the
/// supervisor socket deliberately stay OUT of git and hook children
/// (added only by [`agent_command`]): they exist to authorize a spawn
/// against the supervisor over its socket, which only the agent ever
/// does. Git and the hook share everything else because the task of the
/// shared contract is "which process tree is this" — a credential is the
/// one thing a clone or a user-authored hook has no business holding.
fn launch_child_command(
    program: impl AsRef<std::ffi::OsStr>,
    spec: &LaunchSpec,
) -> std::process::Command {
    let mut command = std::process::Command::new(program);
    let inherited_path = std::env::var_os("PATH").unwrap_or_default();
    let mut path = spec.farhelm_bin_dir.as_os_str().to_os_string();
    if !inherited_path.is_empty() {
        path.push(":");
        path.push(inherited_path);
    }
    command
        .env("PATH", path)
        .env(SESSION_ID_ENV_VAR, &spec.session_id)
        .env(AGENT_ID_ENV_VAR, &spec.session_id)
        .env("COLORTERM", "truecolor")
        // A nested supervisor can inherit another session's credential.
        // Preparation children must lose it; agent_command installs only
        // this launch's credential after the shared environment is built.
        .env_remove(SESSION_TOKEN_ENV_VAR)
        .env_remove(SUPERVISOR_SOCK_ENV_VAR)
        .env_remove(TAB_ID_ENV_VAR)
        .env_remove(GOOSE_REPORTER_ENABLED_ENV_VAR)
        .env_remove(GOOSE_INSTRUCTIONS_ENV_VAR)
        .env_remove(GOOSE_REPORTER_EXE_ENV_VAR)
        .env_remove(PI_REPORTER_EXE_ENV_VAR)
        // The OMP reporter's executable pointer: preparation children
        // never receive `-e`, so the variable is inert there — the same
        // reason the Goose/Pi reporter variables are scrubbed. Its only
        // in-support consumer is the OMP asset itself, which reads it from
        // the agent command's environment, installed separately.
        .env_remove(OMP_REPORTER_EXE_ENV_VAR);
    command
}

/// The durable state of one checkout preparation, advanced ONLY by the
/// launch shim while it works inside the session terminal and read by the
/// supervisor's create/reload paths to classify what happened.
///
/// The restart contract this encodes is deliberate and the code below is
/// its only author:
///
/// - **Ready** means clone AND hook both completed and were durably
///   recorded. A relaunch skips both entirely and execs the agent
///   directly — never clone again, never re-run a hook again (a hook is
///   user-authored and not assumed idempotent).
/// - **CloneStarted / HookStarted / Failed** mean setup did not run to
///   completion, and the shim REFUSES to continue: setup is not
///   automatically repeated. The user can inspect the terminal and the
///   partial checkout, launch an ordinary session in the directory, or
///   delete and create afresh. This refusal is unconditional — even a
///   crash between a successful hook exit and the durable `Ready` write
///   lands here, because "the hook exited zero" and "a `Ready` record
///   exists" cannot be told apart, and the hook may not be idempotent.
///   In particular the shim NEVER infers completion from `.git` existing:
///   a partial clone leaves `.git` behind too, and the file's state is
///   the only truth.
/// - **NotStarted** means no step has run. The shim proceeds on it as the
///   ordinary first-attempt case; the not-yet-launched create reservation
///   recovery path is the only caller for whom "NotStarted implies no
///   live launch" could matter, and THAT proof (no live launch for the
///   reservation) is the create path's own job, not the shim's.
///
/// Every reader of this state treats a MISSING, unreadable, or corrupt
/// state file as an ERROR, never as `NotStarted`: `NotStarted` is a
/// state the create path deliberately published before spawning the
/// window, so its absence means something else removed or mangled the
/// file, and silently proceeding would turn "state unknown" into "state
/// trusted". The file-absent case is representable ONLY through
/// [`read_preparation_state`]'s `Ok(None)`, for the create path's own
/// initial publication bookkeeping.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PreparationState {
    /// Published by the create path before the terminal is spawned; the
    /// shim's signal that it may proceed through clone, hook, and Ready.
    NotStarted,
    /// The durable record that a clone is about to run or is running;
    /// written BEFORE the clone child is spawned, never after.
    CloneStarted,
    /// The durable record that the post-clone hook is about to run;
    /// written BEFORE the hook child is spawned, never after.
    HookStarted,
    /// Clone and hook both completed; the agent may launch and may be
    /// relaunched, but setup never runs again for this working copy.
    Ready,
    /// A step failed; the terminal carries the visible error and the
    /// partial checkout is left in place for inspection. `stage` names
    /// which step failed (`clone`, `post-clone`, `cwd-identity`,
    /// `validate`); `detail` carries the failure reason without logging
    /// the hook's command text (user-authored content).
    Failed { stage: String, detail: String },
}

/// The versioned on-disk shape of a preparation's state file — the JSON
/// at [`CheckoutPreparation::state_path`]. `version` exists so a future
/// schema change can be detected rather than silently misread: a reader
/// that encounters an unrecognized version fails closed, the same
/// treatment as corrupt content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparationRecord {
    /// The state-file schema version; currently always 1.
    pub version: u32,
    /// The working copy this record belongs to; a reader refuses a file
    /// whose id does not match the preparation being considered.
    pub working_copy_id: String,
    /// The state itself; see [`PreparationState`] for the contract.
    pub state: PreparationState,
    /// When this record was written, as seconds since the unix epoch.
    /// Informational (crash forensics), not a readiness oracle — the
    /// state's MEANING is what the restart rules act on.
    pub updated_at_unix_seconds: u64,
}

/// The schema version this build writes and accepts for
/// [`PreparationRecord`].
const PREPARATION_STATE_VERSION: u32 = 1;

/// Durably write one preparation-state transition.
///
/// Uses [`crate::files::write_durable_sync`], the durability-bearing tier
/// (`crate::files` module docs): each transition is the evidence a
/// later restart classifies, so a torn or lost transition would be the
/// same class of silent conversion the launch sentinel's tier exists to
/// prevent — and the file must survive a supervisor crash or host reboot
/// with the same fidelity.
pub fn write_preparation_state(
    state_path: &Path,
    working_copy_id: &str,
    state: &PreparationState,
    seam: &dyn crate::files::FaultSeam,
) -> std::io::Result<()> {
    let record = PreparationRecord {
        version: PREPARATION_STATE_VERSION,
        working_copy_id: working_copy_id.to_string(),
        state: state.clone(),
        updated_at_unix_seconds: unix_seconds_now(),
    };
    let bytes = serde_json::to_vec(&record)
        .map_err(|e| std::io::Error::other(format!("serializing preparation state: {e}")))?;
    crate::files::write_durable_sync(state_path, &bytes, seam)
}

/// Read one preparation's state file: `Ok(None)` means the file is
/// ABSENT — a distinction only the create path may treat as "nothing
/// published yet" (its initial `NotStarted` write); every other caller,
/// notably the shim, must treat absence as an error, never as
/// `NotStarted` (see [`PreparationState`]'s docs).
///
/// Any other read failure, unparseable JSON, an unrecognized schema
/// version, or a working-copy id that does not match the expected one is
/// `Err` — fail closed, never folded into "not started".
pub fn read_preparation_state(
    state_path: &Path,
    working_copy_id: &str,
) -> anyhow::Result<Option<PreparationRecord>> {
    let bytes = match std::fs::read(state_path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(anyhow::Error::from(e).context(format!(
                "reading preparation state at {} (absence is `Ok(None)`; anything else is a \
                 hard failure, never a silent `NotStarted`)",
                state_path.display()
            )));
        }
    };
    let record: PreparationRecord = serde_json::from_slice(&bytes).with_context(|| {
        format!(
            "preparation state at {} is not parseable; a durability-bearing write never leaves \
             torn content, so this is corrupt state, not an early crash",
            state_path.display()
        )
    })?;
    if record.version != PREPARATION_STATE_VERSION {
        anyhow::bail!(
            "preparation state at {} has schema version {} but this build understands {}",
            state_path.display(),
            record.version,
            PREPARATION_STATE_VERSION
        );
    }
    if record.working_copy_id != working_copy_id {
        anyhow::bail!(
            "preparation state at {} names working copy {:?} but this preparation is {:?}",
            state_path.display(),
            record.working_copy_id,
            working_copy_id
        );
    }
    Ok(Some(record))
}

/// Seconds since the unix epoch for [`PreparationRecord`]'s timestamp.
fn unix_seconds_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The flock target for one working copy's preparation: a lock file
/// BESIDE the state file (`checkout-preparation/<id>.lock`), not inside
/// the checkout — nothing bookkeeping-related may live inside the
/// checkout directory itself, which git requires to be empty.
pub fn preparation_lock_path(state_path: &Path) -> PathBuf {
    state_path.with_extension("lock")
}

/// The shim-side preparation run: clone, hook, Ready, agent — inside the
/// session terminal, under a per-checkout flock, with every abort path
/// durable and visible.
///
/// Returns `Ok(())` when the ordinary agent exec should proceed (either
/// setup just completed or a prior `Ready` says it already did). On any
/// abort this function has ALREADY written the launch sentinel (the
/// ordinary classification evidence) and — when the state's meaning
/// allows it — recorded `Failed`, and returns `Err` whose chain carries
/// the user-facing message; the caller prints it to the terminal and
/// never execs.
///
/// ## Order of the checks, and why the lock comes before the state read
///
/// Field validation and the cwd identity check happen before any
/// process could run (fail closed). The flock is acquired before the
/// state is READ as well as before the clone: two concurrent launches of
/// the same working copy must serialize their whole read-decide-clone
/// sequence, not merely their writes, or both could read `NotStarted`
/// and both clone. The identity check precedes the state read so a
/// launch pointing at a replaced directory is refused on the identity
/// evidence, before the state's meaning is even consulted.
fn run_checkout_preparation(
    spec: &LaunchSpec,
    preparation: &CheckoutPreparation,
    status_path: &Path,
    seam: &dyn crate::files::FaultSeam,
) -> anyhow::Result<()> {
    // Fail closed on structurally invalid input BEFORE any process runs.
    // This is not the helm's validation re-run for politeness: the shim is
    // the last writer and treats its input as untrusted.
    if let Err(reason) = validate_preparation(preparation) {
        return abort_preparation(preparation, status_path, seam, "validate", reason, true);
    }

    // Identity: the directory the clone is aimed at must still be the
    // one that was allocated. stat the CURRENT directory and compare
    // device+inode with what the allocation recorded.
    let cwd = Path::new(&preparation.cwd);
    let identity = match std::fs::metadata(cwd) {
        Ok(metadata) => (metadata.dev(), metadata.ino()),
        Err(e) => {
            return abort_preparation(
                preparation,
                status_path,
                seam,
                "cwd-identity",
                format!("the allocated directory {} is gone: {e}", preparation.cwd),
                true,
            );
        }
    };
    if identity != (preparation.directory_device, preparation.directory_inode) {
        return abort_preparation(
            preparation,
            status_path,
            seam,
            "cwd-identity",
            format!(
                "the directory at {} was replaced since it was allocated (device/inode \
                 {}:{} no longer {}:{}); refusing to clone into it",
                preparation.cwd,
                identity.0,
                identity.1,
                preparation.directory_device,
                preparation.directory_inode
            ),
            true,
        );
    }

    // One preparation at a time per working copy. Blocking flock: a
    // second concurrent launch WAITS for the first to finish rather than
    // failing the launch — the first may simply be a slow clone, and
    // after it records `Ready` this launch's restart rules do the right
    // thing (skip setup, run the agent).
    let lock = match acquire_preparation_lock(&preparation_lock_path(&preparation.state_path)) {
        Ok(lock) => lock,
        Err(e) => {
            return abort_preparation(
                preparation,
                status_path,
                seam,
                "lock",
                format!(
                    "could not acquire the checkout preparation lock: {e} (the lock must be \
                     held for the whole preparation and must never leak into any child \
                     process)"
                ),
                false,
            );
        }
    };

    // The state read happens UNDER the lock (see this function's docs).
    // Absent is an error here — `Ok(None)` exists only for the create
    // path's own bookkeeping; the create path publishes `NotStarted`
    // BEFORE this window is spawned, so a missing file at this point
    // means the state evidence is gone, and proceeding would turn
    // "state unknown" into "state trusted".
    let record = match read_preparation_state(&preparation.state_path, &preparation.working_copy_id)
    {
        Ok(Some(record)) => record,
        Ok(None) => {
            return abort_preparation(
                preparation,
                status_path,
                seam,
                "state-read",
                format!(
                    "the preparation state file {} does not exist although the create path \
                     publishes NotStarted before spawning this terminal; refusing to prepare \
                     without state evidence (the file's absence is never treated as \
                     NotStarted)",
                    preparation.state_path.display()
                ),
                false,
            );
        }
        Err(e) => {
            // Unreadable or corrupt: fail closed WITHOUT overwriting
            // whatever is there — the file is the evidence, and a
            // `Failed` write would destroy it.
            return abort_preparation(
                preparation,
                status_path,
                seam,
                "state-read",
                format!("{e:#}"),
                false,
            );
        }
    };

    match record.state {
        PreparationState::Ready => {
            // Setup completed in a previous launch. Skip clone AND hook;
            // the ordinary agent exec below handles restart. The lock is
            // released before the exec (its Drop); the agent must never
            // inherit it.
            return Ok(());
        }
        state @ (PreparationState::CloneStarted | PreparationState::HookStarted) => {
            return abort_preparation(
                preparation,
                status_path,
                seam,
                "restart",
                format!(
                    "this working copy's checkout preparation is recorded as {state:?}: \
                     setup did not finish and is not automatically repeated. Inspect the \
                     directory (a partial checkout may be there), launch an ordinary \
                     session in it, or delete and create the session afresh."
                ),
                false,
            );
        }
        PreparationState::Failed { stage, detail } => {
            return abort_preparation(
                preparation,
                status_path,
                seam,
                "restart",
                format!(
                    "this working copy's checkout preparation previously failed at stage \
                     {stage:?} ({detail}) and is not automatically repeated. Inspect the \
                     directory, launch an ordinary session in it, or delete and create the \
                     session afresh."
                ),
                false,
            );
        }
        // NotStarted is the ordinary first attempt; the create path
        // published it before spawning this window and owns the proof
        // that no other launch is live for this reservation.
        PreparationState::NotStarted => {}
    }

    // Clone. NO timeout: a credential prompt or a slow network is
    // ordinary terminal work; Stop/Delete own this process tree.
    if let Err(stage_detail) = write_preparation_state(
        &preparation.state_path,
        &preparation.working_copy_id,
        &PreparationState::CloneStarted,
        seam,
    ) {
        return abort_preparation(
            preparation,
            status_path,
            seam,
            "clone",
            format!("could not durably record CloneStarted: {stage_detail}"),
            false,
        );
    }
    // argv, never shell text: the URL and the directory were validated
    // above, and hostile characters in either must be able to end up in
    // a file NAME, never in a command.
    let mut clone = launch_child_command(std::ffi::OsStr::new("git"), spec);
    clone
        .args([
            "clone".to_string(),
            "--".to_string(),
            preparation.clone_url.clone(),
            preparation.cwd.clone(),
        ])
        .env("GIT_TERMINAL_PROMPT", "1");
    if let Err(detail) = run_preparation_child_process(&mut clone, "clone") {
        return abort_preparation(preparation, status_path, seam, "clone", detail, true);
    }

    // Post-clone hook, if the helm resolved one. Run as
    // `<shell> -c <hook>` with the checkout as cwd — the hook is
    // user-authored shell text and may do anything the user configured;
    // the repo identifier and cwd are deliberately NOT expanded into it.
    // No second `-l -i` initialization: the hook is not a login shell,
    // it is a one-shot command in the environment the session already
    // has.
    if let Some(hook) = &preparation.post_clone {
        if let Err(stage_detail) = write_preparation_state(
            &preparation.state_path,
            &preparation.working_copy_id,
            &PreparationState::HookStarted,
            seam,
        ) {
            return abort_preparation(
                preparation,
                status_path,
                seam,
                "post-clone",
                format!("could not durably record HookStarted: {stage_detail}"),
                false,
            );
        }
        let mut hook_command = launch_child_command(std::ffi::OsStr::new(&preparation.shell), spec);
        hook_command
            .args(["-c".to_string(), hook.clone()])
            .current_dir(cwd);
        if let Err(detail) = run_preparation_child_process(&mut hook_command, "post-clone") {
            // The hook's own text is user content; the report names the
            // stage and the exit status only.
            return abort_preparation(preparation, status_path, seam, "post-clone", detail, true);
        }
    }

    if let Err(stage_detail) = write_preparation_state(
        &preparation.state_path,
        &preparation.working_copy_id,
        &PreparationState::Ready,
        seam,
    ) {
        return abort_preparation(
            preparation,
            status_path,
            seam,
            "ready",
            format!("could not durably record Ready: {stage_detail}"),
            false,
        );
    }

    // Released before the agent exec (the lock file is closed here), and
    // CLOEXEC'd at open so git/hook children never inherited it either.
    drop(lock);
    Ok(())
}

/// Run one preparation child process (the clone or the hook) with the
/// terminal's own stdin/stdout/stderr inherited, and classify its
/// outcome. There is deliberately NO timeout: credential prompts and
/// slow networks are ordinary terminal work, and Stop/Delete own the
/// process tree.
///
/// The returned detail names the stage's failure reason only — exit
/// status or spawn error — never the command text itself (the hook is
/// user-authored content that must not land in logs or sentinels).
fn run_preparation_child_process(
    command: &mut std::process::Command,
    stage: &'static str,
) -> Result<(), String> {
    match command.status() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(match status.code() {
            Some(code) => format!("{stage} exited with status {code}"),
            None => format!("{stage} was terminated by a signal"),
        }),
        Err(e) => Err(format!("could not start the {stage} command: {e}")),
    }
}

/// One abort path of [`run_checkout_preparation`]: record `Failed` in the
/// state file (when `overwrite_state` allows it), write the ordinary
/// launch sentinel, print the actionable message to the terminal, and
/// return the error the shim exits with. Nothing is exec'd after this.
///
/// `overwrite_state` is false for the refusals that must PRESERVE the
/// existing evidence (unreadable state, an unfinished state, a lock
/// failure): the state file's meaning is the restart rules' input, and a
/// refusal is not a new fact about the working copy.
fn abort_preparation(
    preparation: &CheckoutPreparation,
    status_path: &Path,
    seam: &dyn crate::files::FaultSeam,
    stage: &str,
    detail: String,
    overwrite_state: bool,
) -> anyhow::Result<()> {
    if overwrite_state {
        // Best-effort: a Failed write that itself fails is folded into
        // the message; the sentinel below still carries the evidence.
        if let Err(e) = write_preparation_state(
            &preparation.state_path,
            &preparation.working_copy_id,
            &PreparationState::Failed {
                stage: stage.to_string(),
                detail: detail.clone(),
            },
            seam,
        ) {
            let detail = format!(
                "{detail}; additionally, recording the failure durably \
                 failed: {e}"
            );
            let report = record_launch_failure(
                status_path,
                format!("preparation failed at stage {stage}: {detail}"),
                seam,
            );
            eprintln!("checkout preparation failed: {detail}");
            return Err(anyhow::anyhow!(report));
        }
    }
    let report = record_launch_failure(
        status_path,
        format!("preparation failed at stage {stage}: {detail}"),
        seam,
    );
    eprintln!("checkout preparation failed: {detail}");
    Err(anyhow::anyhow!(report))
}

/// The shim's field validation of a preparation, independent of anything
/// the helm already checked: `clone_url` must be an https GitHub URL with
/// no shell metacharacters (the shim runs argv, but the URL is
/// user-influenced content and refusing metacharacters closes the door on
/// any future caller that composes a command line), `cwd` must be
/// absolute, and the id and shell must be non-empty. Every refusal is a
/// fail-closed abort before any process runs.
fn validate_preparation(preparation: &CheckoutPreparation) -> Result<(), String> {
    const GITHUB_PREFIX: &str = "https://github.com/";
    if !preparation.clone_url.starts_with(GITHUB_PREFIX) {
        return Err(format!(
            "clone_url must start with {GITHUB_PREFIX:?}, got {:?}",
            preparation.clone_url
        ));
    }
    const SHELL_METACHARACTERS: &str = ";|&`$()<>\\\"'!*?[]{}#~\n\r\t ";
    if preparation
        .clone_url
        .contains(|c: char| SHELL_METACHARACTERS.contains(c))
    {
        return Err(format!(
            "clone_url contains a shell metacharacter, which is refused: {:?}",
            preparation.clone_url
        ));
    }
    if preparation.clone_url == GITHUB_PREFIX {
        return Err("clone_url must name a repository".to_string());
    }
    if preparation.working_copy_id.is_empty() {
        return Err("working_copy_id is empty".to_string());
    }
    if preparation.shell.is_empty() {
        return Err("shell is empty".to_string());
    }
    let cwd = Path::new(&preparation.cwd);
    if !cwd.is_absolute() {
        return Err(format!(
            "cwd must be an absolute path, got {:?}",
            preparation.cwd
        ));
    }
    Ok(())
}

/// The per-working-copy preparation lock, held across the whole
/// read-decide-clone-hook-record sequence and released BEFORE the agent
/// exec.
///
/// Two properties, both load-bearing:
///
/// - **Exclusivity** via `flock(LOCK_EX)`, blocking: a second concurrent
///   preparation of the same working copy WAITS rather than failing, so
///   a slow first clone does not fail an unrelated relaunch.
/// - **CLOEXEC**: the descriptor is explicitly marked close-on-exec
///   (`std::fs::File` opens with `O_CLOEXEC` already; the `F_SETFD` call
///   is kept as the load-bearing defense so this property survives
///   whatever code opens the file in the future), so neither git, the
///   hook, nor the agent ever inherits the lock. A child holding an
///   flock would keep the guard alive after the shim released it,
///   silently re-serializing unrelated preparations for as long as the
///   agent lived.
struct PreparationLock(std::fs::File);

impl PreparationLock {
    fn open(path: &Path) -> std::io::Result<Self> {
        use std::os::unix::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(path)?;
        // Belt and braces on CLOEXEC: std already opens O_CLOEXEC, but
        // this property is the whole point of the type, so assert it in
        // place rather than trusting the open's current implementation.
        // SAFETY: F_SETFD/FD_CLOEXEC on a valid fd cannot fail with
        // these arguments and has no preconditions beyond the fd.
        let existing = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFD) };
        if existing < 0
            || unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFD, existing | libc::FD_CLOEXEC) }
                < 0
        {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self(file))
    }

    /// Take the exclusive lock, blocking until it is free.
    fn acquire(&self) -> std::io::Result<()> {
        // SAFETY: flock on a valid fd; LOCK_EX is the exclusive mode.
        if unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_EX) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }
}

/// Open and hold the preparation lock for a working copy: creates the
/// lock file if absent (its directory already exists — the create path
/// made it when publishing the state file) and blocks until the lock is
/// free.
fn acquire_preparation_lock(lock_path: &Path) -> std::io::Result<PreparationLock> {
    let lock = PreparationLock::open(lock_path)?;
    lock.acquire()?;
    Ok(lock)
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
    //! Exec-failure tests run the shim in a disposable child process. Rust's
    //! `CommandExt::exec` documentation warns that a failed exec can leave the
    //! process environment inconsistent; on Unix, the returned call can leave
    //! libc's `environ` pointing at the command's freed environment vector.
    //! That is a process-wide hazard for any later `getenv`, not a local error
    //! the test can safely recover from. Production shims exit immediately;
    //! these tests must do the same without poisoning the shared test process.

    use super::*;
    use std::os::unix::fs::PermissionsExt;

    const EXEC_FAILURE_CHILD_SPEC: &str = "FARHELM_LAUNCH_EXEC_FAILURE_CHILD_SPEC";
    const EXEC_FAILURE_CHILD_KIND: &str = "FARHELM_LAUNCH_EXEC_FAILURE_CHILD_KIND";

    /// Store the failed exec's error for the parent, then leave immediately.
    ///
    /// Nothing after `exec_launch_spec` may return to libtest: its normal
    /// bookkeeping can consult the process environment after the failed exec
    /// left that environment unsafe. The parent owns every assertion and
    /// treats either a failed write or an abnormal child exit as a test
    /// failure.
    fn finish_exec_failure_child(spec_path: &Path, error: &anyhow::Error) -> ! {
        let wrote_error = std::fs::write(
            exec_failure_child_result_path(spec_path),
            format!("{error:#}"),
        )
        .is_ok();
        // SAFETY: this is the disposable child process, after the result file
        // is complete. Skipping Rust and libtest cleanup is the point: either
        // may read the process-wide environment that failed exec invalidated.
        unsafe { libc::_exit(if wrote_error { 0 } else { 1 }) }
    }

    /// Keep a child's error report beside its spec without colliding with the
    /// durable `.status` sentinel the production shim writes.
    fn exec_failure_child_result_path(spec_path: &Path) -> PathBuf {
        spec_path.with_extension("child-error")
    }

    /// Run the ignored child helper as a single-purpose exec-failure process.
    ///
    /// `--exact` and `--ignored` are both load-bearing: a normal test run can
    /// never enter the child path because an unrelated caller happened to set
    /// one of its environment variables. The child writes the returned error
    /// to a file and calls `_exit`; this parent remains safe to inspect that
    /// output, the sentinel, and every original invariant.
    fn run_exec_failure_child(spec_path: &Path, kind: &str) -> String {
        let exe = std::env::current_exe().expect("locate the supervisor test binary");
        let run = std::process::Command::new(&exe)
            .args([
                "--exact",
                "launch::tests::exec_failure_child",
                "--ignored",
                "--nocapture",
            ])
            .env(EXEC_FAILURE_CHILD_SPEC, spec_path)
            .env(EXEC_FAILURE_CHILD_KIND, kind)
            .output()
            .unwrap_or_else(|error| panic!("re-running {exe:?}: {error}"));
        assert!(
            run.status.success(),
            "the exec-failure child must exit cleanly: status={:?}, stdout={}, stderr={}",
            run.status,
            String::from_utf8_lossy(&run.stdout),
            String::from_utf8_lossy(&run.stderr)
        );
        std::fs::read_to_string(exec_failure_child_result_path(spec_path))
            .expect("the exec-failure child must leave its error report")
    }

    /// Drive one real failed exec, record its result for the parent, and exit.
    ///
    /// This is the only test that enters the unsafe post-exec-failure state.
    /// It is ignored during ordinary runs and invoked deliberately by
    /// [`run_exec_failure_child`] under its exact test name. The two modes
    /// cover the real filesystem and the sentinel-write fault seam without
    /// returning either case to libtest cleanup.
    #[farhelm_testtrace::test]
    #[ignore = "the child half of the exec-failure isolation tests"]
    fn exec_failure_child() {
        let spec_path = std::env::var_os(EXEC_FAILURE_CHILD_SPEC)
            .map(PathBuf::from)
            .expect("the parent must provide the child spec path");
        let kind = std::env::var(EXEC_FAILURE_CHILD_KIND)
            .expect("the parent must provide the child scenario");
        let error = match kind.as_str() {
            "real" => exec_launch_spec(&spec_path),
            "fail-at-write" => {
                struct FailAtWrite;
                impl crate::files::FaultSeam for FailAtWrite {
                    fn write(
                        &self,
                        _file: &mut std::fs::File,
                        _bytes: &[u8],
                    ) -> std::io::Result<()> {
                        Err(std::io::Error::other("injected sentinel-write failure"))
                    }
                }
                exec_launch_spec_with_seam(&spec_path, &FailAtWrite)
            }
            other => panic!("unknown exec-failure child scenario: {other}"),
        };
        finish_exec_failure_child(&spec_path, &error);
    }

    /// The ordinary case: no launch has ever failed for this session, so
    /// there is no sentinel file at all, and the reader must say so
    /// plainly rather than treating a missing file as any kind of error.
    #[farhelm_testtrace::test]
    fn read_launch_sentinel_is_none_when_nothing_was_ever_written() {
        let tmp = tempfile::tempdir().unwrap();
        let result = read_launch_sentinel(tmp.path(), "never-launched", 0).unwrap();
        assert_eq!(result, None);
    }

    /// The reader's whole reason to exist: surfacing exactly what
    /// [`exec_launch_spec`] left behind, at the SAME path
    /// [`spec_path_for_session`]/[`status_path_for_spec`] derive
    /// independently — proving the two sides of this contract (the
    /// writer inside the shim, the reader inside the supervisor) actually
    /// agree on where a launch's sentinel lives.
    #[farhelm_testtrace::test]
    fn read_launch_sentinel_reads_back_what_the_shim_wrote() {
        let tmp = tempfile::tempdir().unwrap();
        let id = "session-1";
        let spec_path = spec_path_for_launch(tmp.path(), id, 0);
        std::fs::create_dir_all(spec_path.parent().unwrap()).unwrap();
        let missing_binary = tmp.path().join("no-such-farhelm-test-binary");
        let spec = LaunchSpec {
            argv: vec![missing_binary.to_string_lossy().into_owned()],
            status_file: status_path_for_spec(&spec_path),
            session_id: id.to_string(),
            session_token: "test-token".to_string(),
            supervisor_sock: PathBuf::from("/tmp/supervisor.sock"),
            farhelm_bin_dir: PathBuf::from("/opt/farhelm/bin"),
            preparation: None,
        };
        std::fs::write(&spec_path, serde_json::to_vec(&spec).unwrap()).unwrap();
        run_exec_failure_child(&spec_path, "real");

        // Compared against the file's FULL content, not merely `contains`:
        // the reader's whole job is to hand back exactly what is on disk,
        // and a substring check would not catch a reader that silently
        // truncated, trimmed, or otherwise mangled the bytes on the way
        // through.
        let status_path = status_path_for_spec(&spec_path);
        let on_disk = std::fs::read_to_string(&status_path).expect("sentinel must exist on disk");
        let sentinel = read_launch_sentinel(tmp.path(), id, 0)
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
    #[farhelm_testtrace::test]
    fn read_launch_sentinel_reports_a_directory_at_the_status_path_loudly() {
        let tmp = tempfile::tempdir().unwrap();
        let id = "session-dir";
        let status_path = status_path_for_spec(&spec_path_for_launch(tmp.path(), id, 0));
        std::fs::create_dir_all(&status_path).unwrap();

        let err = read_launch_sentinel(tmp.path(), id, 0)
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
    #[farhelm_testtrace::test]
    fn read_launch_sentinel_rejects_empty_and_whitespace_only_content() {
        let tmp = tempfile::tempdir().unwrap();
        for (id, content) in [("empty", ""), ("blank", "   \n\t  ")] {
            let status_path = status_path_for_spec(&spec_path_for_launch(tmp.path(), id, 0));
            std::fs::create_dir_all(status_path.parent().unwrap()).unwrap();
            std::fs::write(&status_path, content).unwrap();

            let err = read_launch_sentinel(tmp.path(), id, 0)
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
    #[farhelm_testtrace::test]
    fn read_launch_sentinel_refuses_to_treat_invalid_utf8_as_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let id = "session-2";
        let status_path = status_path_for_spec(&spec_path_for_launch(tmp.path(), id, 0));
        std::fs::create_dir_all(status_path.parent().unwrap()).unwrap();
        std::fs::write(&status_path, [0xff, 0xfe, 0xfd]).unwrap();

        let err = read_launch_sentinel(tmp.path(), id, 0)
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
    #[farhelm_testtrace::test]
    fn exec_launch_spec_writes_a_durable_sentinel_on_exec_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("spec.json");
        let missing_binary = tmp.path().join("no-such-farhelm-test-binary");
        let spec = LaunchSpec {
            argv: vec![missing_binary.to_string_lossy().into_owned()],
            status_file: status_path_for_spec(&spec_path),
            session_id: "test-session".to_string(),
            session_token: "test-token".to_string(),
            supervisor_sock: PathBuf::from("/tmp/supervisor.sock"),
            farhelm_bin_dir: PathBuf::from("/opt/farhelm/bin"),
            preparation: None,
        };
        std::fs::write(&spec_path, serde_json::to_vec(&spec).unwrap()).unwrap();

        let rendered = run_exec_failure_child(&spec_path, "real");
        assert!(
            rendered.contains("exec_failed"),
            "error chain should surface the exec-failure report: {rendered}"
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
    #[farhelm_testtrace::test]
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
            session_token: "test-token".to_string(),
            supervisor_sock: PathBuf::from("/tmp/supervisor.sock"),
            farhelm_bin_dir: PathBuf::from("/opt/farhelm/bin"),
            preparation: None,
        };
        std::fs::write(&spec_path, serde_json::to_vec(&spec).unwrap()).unwrap();

        let rendered = run_exec_failure_child(&spec_path, "real");
        assert!(rendered.contains("exec_failed"));

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
    #[farhelm_testtrace::test]
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
    #[farhelm_testtrace::test]
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
    #[farhelm_testtrace::test]
    fn exec_launch_spec_records_a_sentinel_for_empty_argv() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("spec.json");
        let spec = LaunchSpec {
            argv: vec![],
            status_file: status_path_for_spec(&spec_path),
            session_id: "test-session".to_string(),
            session_token: "test-token".to_string(),
            supervisor_sock: PathBuf::from("/tmp/supervisor.sock"),
            farhelm_bin_dir: PathBuf::from("/opt/farhelm/bin"),
            preparation: None,
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
    #[farhelm_testtrace::test]
    fn exec_launch_spec_with_seam_reports_both_failures_without_a_torn_sentinel() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("spec.json");
        let missing_binary = tmp.path().join("no-such-farhelm-test-binary");
        let spec = LaunchSpec {
            argv: vec![missing_binary.to_string_lossy().into_owned()],
            status_file: status_path_for_spec(&spec_path),
            session_id: "test-session".to_string(),
            session_token: "test-token".to_string(),
            supervisor_sock: PathBuf::from("/tmp/supervisor.sock"),
            farhelm_bin_dir: PathBuf::from("/opt/farhelm/bin"),
            preparation: None,
        };
        std::fs::write(&spec_path, serde_json::to_vec(&spec).unwrap()).unwrap();

        let rendered = run_exec_failure_child(&spec_path, "fail-at-write");
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
    #[farhelm_testtrace::test]
    fn window_command_is_login_interactive_exec_shim() {
        let cmd = window_command(
            "/bin/bash",
            Path::new("/opt/farhelm"),
            Path::new("/state/launch/abc.json"),
            Vec::new(),
        );
        assert_eq!(cmd[0..4], ["/bin/bash", "-l", "-i", "-c"]);
        assert_eq!(
            cmd[4],
            "exec /opt/farhelm internal launch /state/launch/abc.json"
        );
    }

    /// The shim's window command writes NOTHING to the terminal before the
    /// `exec` — SPEC.md's rule that Farhelm must never paint a captured
    /// screen back ahead of a new process (a launch "preamble" once did
    /// exactly that from inside the shim). Pinned structurally rather than
    /// by full-string equality so incidental argv growth cannot rot it:
    /// the `-c` script must BE a single `exec` command — it starts with
    /// `exec ` and contains no separator that could smuggle an earlier
    /// command (and therefore an earlier terminal write) in front of it.
    /// Exercised with a scope prefix, the one thing that legitimately
    /// splices into the command, to show it lands after the `exec` word
    /// rather than as a command of its own.
    #[farhelm_testtrace::test]
    fn window_command_script_runs_nothing_before_the_exec() {
        let cmd = window_command(
            "/bin/bash",
            Path::new("/opt/farhelm"),
            Path::new("/state/launch/abc.json"),
            vec!["systemd-run".to_string(), "--scope".to_string()],
        );
        let script = &cmd[4];
        assert!(
            script.starts_with("exec "),
            "the shim script's first and only command must be the exec: {script:?}"
        );
        for separator in [";", "&&", "||", "\n", "|", "`", "$("] {
            assert!(
                !script.contains(separator),
                "the shim script must be a single command with no {separator:?} that could \
                 run anything (or write anything to the terminal) before the exec: {script:?}"
            );
        }
    }

    /// The final exec receives the session credential and the exact socket
    /// that minted it; neither may depend on a guessed state-directory path.
    #[farhelm_testtrace::test]
    fn agent_command_injects_the_spawn_contract() {
        let spec = LaunchSpec {
            argv: vec!["agent".to_string()],
            status_file: PathBuf::from("/tmp/status"),
            session_id: "session-7".to_string(),
            session_token: "private-token".to_string(),
            supervisor_sock: PathBuf::from("/run/user/1000/farhelm.sock"),
            farhelm_bin_dir: PathBuf::from("/opt/farhelm/bin"),
            preparation: None,
        };
        let command = agent_command(&spec);
        let env = command
            .get_envs()
            .map(|(name, value)| {
                (
                    name.to_string_lossy().into_owned(),
                    value.map(|value| value.to_string_lossy().into_owned()),
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(
            env.get(SESSION_ID_ENV_VAR).and_then(Option::as_deref),
            Some("session-7")
        );
        assert_eq!(
            env.get(SESSION_TOKEN_ENV_VAR).and_then(Option::as_deref),
            Some("private-token")
        );
        assert_eq!(
            env.get(SUPERVISOR_SOCK_ENV_VAR).and_then(Option::as_deref),
            Some("/run/user/1000/farhelm.sock")
        );
        assert_eq!(
            env.get("COLORTERM").and_then(Option::as_deref),
            Some("truecolor")
        );
        assert_eq!(env.get(TAB_ID_ENV_VAR), Some(&None));
        assert!(
            env.get("PATH")
                .and_then(Option::as_deref)
                .is_some_and(
                    |path| path == "/opt/farhelm/bin" || path.starts_with("/opt/farhelm/bin:")
                ),
            "the running binary's directory must win PATH resolution"
        );
    }

    /// The cgroup wrapper (PLAN_M3.md item 10) must land BETWEEN the
    /// shell's `exec` and the shim, leaving both ends of the chain exactly
    /// as they were: the login shell still outermost (the environment
    /// contract), the shim still `exec`ing the agent (the sentinel
    /// contract). A prefix that wrapped the shell instead, or that
    /// displaced the `exec`, would break one of the two silently.
    #[farhelm_testtrace::test]
    fn a_scope_prefix_wraps_only_the_shim_invocation() {
        // A hand-written prefix, not one from `ScopeManager`: what is under
        // test here is the COMPOSITION (where the prefix lands in the chain),
        // and the manager's own flag list has its own test — against the real
        // probed binary, which is the only place that list means anything.
        let prefix: Vec<String> = ["systemd-run", "--user", "--scope", "--collect", "--quiet"]
            .iter()
            .map(|s| s.to_string())
            .chain(["--unit=farhelm-abc-0.scope".to_string(), "--".to_string()])
            .collect();
        let cmd = window_command(
            "/bin/bash",
            Path::new("/opt/farhelm"),
            Path::new("/state/launch/abc.0.json"),
            prefix,
        );
        assert_eq!(cmd[0..4], ["/bin/bash", "-l", "-i", "-c"]);
        // Asserted through the SHELL's own parser rather than on the raw
        // string: `shell_words` quotes `--unit=...` (its safe-character
        // set excludes `=`), which is harmless — the shell strips the
        // quotes — but would make a literal comparison a test of the
        // quoting library rather than of this function's composition.
        let parsed = shell_words::split(&cmd[4]).expect("shell-parseable");
        assert_eq!(
            parsed,
            vec![
                "exec",
                "systemd-run",
                "--user",
                "--scope",
                "--collect",
                "--quiet",
                "--unit=farhelm-abc-0.scope",
                "--",
                "/opt/farhelm",
                "internal",
                "launch",
                "/state/launch/abc.0.json",
            ]
        );
    }

    /// A tab is a bare interactive login shell — no shim, no spec, no
    /// `-c` payload — and `-i` is the flag whose loss would silently cost
    /// the user their rc-file environment (SPEC.md's environment
    /// contract). Pinned as a complete argv so a future addition to the
    /// chain cannot slip in unnoticed.
    #[farhelm_testtrace::test]
    fn tab_window_command_is_a_bare_login_interactive_shell() {
        assert_eq!(
            tab_window_command("/bin/zsh", Vec::new()),
            vec!["env", "-u", AGENT_ID_ENV_VAR, "/bin/zsh", "-l", "-i"]
        );
    }

    /// The tab scope wraps the SHELL, where the agent's wraps only the
    /// shim — the asymmetry `tab_window_command`'s docs argue for, pinned
    /// here so it reads as a decision rather than as a bug the next reader
    /// "fixes" into symmetry with `window_command`. A prefix that ended up
    /// inside or after the shell would leave the tab's own shell outside
    /// the cgroup, which is exactly the process close promises to kill.
    #[farhelm_testtrace::test]
    fn a_tab_scope_prefix_wraps_the_shell_itself() {
        let prefix: Vec<String> = ["systemd-run", "--user", "--scope", "--"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            tab_window_command("/bin/bash", prefix),
            vec![
                "systemd-run",
                "--user",
                "--scope",
                "--",
                // The scrub stays INNERMOST even under a wrapper, so the
                // shell itself is what starts without the ambient marker.
                "env",
                "-u",
                AGENT_ID_ENV_VAR,
                "/bin/bash",
                "-l",
                "-i"
            ]
        );
    }

    /// A state dir under a path with spaces or quotes must still produce
    /// a command the shell parses as one argument — otherwise sessions
    /// fail to launch on perfectly legal home directories.
    #[farhelm_testtrace::test]
    fn window_command_quotes_hostile_paths() {
        let cmd = window_command(
            "/bin/zsh",
            Path::new("/opt/far helm"),
            Path::new("/state/it's/abc.json"),
            Vec::new(),
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
    #[farhelm_testtrace::test]
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
    #[farhelm_testtrace::test]
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
    #[farhelm_testtrace::test]
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
    #[farhelm_testtrace::test]
    fn parse_getent_passwd_line_rejects_empty_shell() {
        assert_eq!(parse_getent_passwd_line("root:x:0:0:root:/root:"), None);
    }

    /// Separator-free successful output is not a passwd record; treating
    /// it as a shell would skip the warning and direct database fallback.
    #[farhelm_testtrace::test]
    fn parse_getent_passwd_line_rejects_missing_separator() {
        assert_eq!(parse_getent_passwd_line("weird"), None);
    }

    /// A `getent` process that never answers must not hold shell resolution
    /// past the NSS bound. The marker proves the stand-in was actually
    /// running; checking its recorded PID after the lookup proves the
    /// timeout dropped the child with `kill_on_drop` instead of orphaning it.
    ///
    /// Real time, deliberately: the bound races a real child process, and a
    /// paused clock elapses the instant the runtime goes idle, which can be
    /// before the stand-in has run its first line — the marker is then
    /// missing and the test cannot tell "killed" from "never started". A
    /// half-second real bound is long enough for `sh` to write one file on a
    /// loaded machine and short enough to keep the test cheap.
    #[farhelm_testtrace::test]
    async fn getent_timeout_kills_the_child() {
        const TEST_BOUND: Duration = Duration::from_millis(500);
        let tmp = tempfile::tempdir().unwrap();
        let marker = tmp.path().join("getent.pid");
        let program = tmp.path().join("getent");
        let script = format!(
            r#"#!/bin/sh
printf '%s' "$$" > {}
exec sleep 60
"#,
            marker.display()
        );
        std::fs::write(&program, script).unwrap();
        let mut permissions = std::fs::metadata(&program).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&program, permissions).unwrap();

        let started_at = std::time::Instant::now();
        let shell = getent_passwd_shell_with_program(&program, TEST_BOUND).await;
        let elapsed = started_at.elapsed();

        let pid = std::fs::read_to_string(&marker)
            .expect("stand-in must have started before timeout completed")
            .parse::<libc::pid_t>()
            .expect("stand-in PID marker must be numeric");
        assert_eq!(shell, None);
        // Generous on the upper side: the point is "the bound fired", not
        // "it fired promptly under load"; a wedged child would take 60 s.
        assert!(
            elapsed < TEST_BOUND * 20,
            "lookup took {elapsed:?}, bound is {TEST_BOUND:?}"
        );

        // The kill is asynchronous with respect to this task: `kill_on_drop`
        // sends SIGKILL when the timed-out future is dropped, and the child's
        // exit then has to be observed by the kernel and, possibly, reaped by
        // tokio's own reaper. Polling with WNOHANG behind a real, bounded
        // interval is the readiness oracle for "the child is gone"; a
        // yield-only loop was a hundred scheduler turns that a loaded host
        // finishes long before the kill lands.
        let mut status = 0;
        let mut reaped = false;
        let reap_deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < reap_deadline {
            match unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) } {
                waited if waited == pid => {
                    reaped = true;
                    break;
                }
                // WNOHANG above is the actual oracle; this is only the pace.
                // sleep-ok: polling interval while the killed stand-in exits
                0 => tokio::time::sleep(Duration::from_millis(10)).await,
                -1 if std::io::Error::last_os_error().raw_os_error() == Some(libc::ECHILD) => {
                    // Tokio may have reaped the child while dropping its
                    // internal process handle; ECHILD is then the proof
                    // that no child with this PID remains for this test.
                    reaped = true;
                    break;
                }
                other => panic!("unexpected waitpid result for stand-in: {other}"),
            }
        }
        assert!(reaped, "timed-out getent child was not reaped");
    }

    /// `ERANGE` must actually grow the buffer passed to the next attempt,
    /// not just retry with the same undersized allocation — that would be
    /// an infinite loop against a real too-small hint. The closure
    /// records every buffer length it was handed so the test can verify
    /// growth directly instead of trusting the return value alone.
    #[farhelm_testtrace::test]
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
    #[farhelm_testtrace::test]
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
    #[farhelm_testtrace::test]
    fn lookup_with_growing_buffer_errno_returns_none() {
        let shell = lookup_with_growing_buffer(64, 0, |_| PwAttempt::Errno(13));
        assert_eq!(shell, None);
    }

    /// `rc == 0` with a null result pointer means "no such passwd entry",
    /// which must resolve the same way as any other unresolvable lookup
    /// (`None`), not be mistaken for success with an empty shell.
    #[farhelm_testtrace::test]
    fn lookup_with_growing_buffer_no_entry_returns_none() {
        let shell = lookup_with_growing_buffer(64, 0, |_| PwAttempt::NoEntry);
        assert_eq!(shell, None);
    }

    /// An entry can exist yet carry an unusable shell field (null, empty,
    /// or non-UTF-8); the closure signals this as `Shell(None)`, and the
    /// driver must pass that through as "no shell" rather than treating
    /// "entry found" as proof a shell was found.
    #[farhelm_testtrace::test]
    fn lookup_with_growing_buffer_shell_none_returns_none() {
        let shell = lookup_with_growing_buffer(64, 0, |_| PwAttempt::Shell(None));
        assert_eq!(shell, None);
    }

    // =========================================================================
    // Checkout preparation (Design D). Every suite drives the REAL shim
    // (`exec_launch_spec`) inside a disposable child process — the same
    // exec-failure isolation the tests above use — with fake `git`, a
    // fake hook shell, and a fake agent installed in a CHILD-ONLY PATH
    // directory. All fixtures and environment arrive on child `Command`s
    // or inside the serialized LaunchSpec; no test below mutates the test
    // process's own environment.
    // =========================================================================

    /// The env vars the parent sets on a prep-child Command to point it
    /// at its spec file and scenario (same pattern as EXEC_FAILURE_CHILD_*).
    const PREP_CHILD_SPEC: &str = "FARHELM_LAUNCH_PREP_CHILD_SPEC";
    const PREP_CHILD_KIND: &str = "FARHELM_LAUNCH_PREP_CHILD_KIND";

    /// Everything one preparation test needs on disk: a fixture bin
    /// directory (becomes the shim child's entire PATH), an allocated
    /// empty checkout directory whose device/inode identity is recorded,
    /// a supervisor state tree with the `NotStarted` state file already
    /// published (the create path's job — the shim refuses an ABSENT
    /// state file), and a launch spec pointing at all of it.
    struct PrepFixture {
        tmp: tempfile::TempDir,
        spec_path: PathBuf,
        preparation: CheckoutPreparation,
    }

    /// The launch argv the fake agent must see, and the session id /
    /// token contract checks below assert on.
    const FAKE_AGENT_ARGV: [&str; 2] = ["fake-agent", "--serve"];

    impl PrepFixture {
        /// Build a fixture. `shell` names the executable (found via the
        /// child-only PATH) the hook runs under.
        fn new(clone_url: &str, post_clone: Option<String>, shell: &str) -> Self {
            let tmp = tempfile::tempdir().unwrap();
            let bin_dir = tmp.path().join("bin");
            std::fs::create_dir(&bin_dir).unwrap();
            let cwd = tmp.path().join("checkout");
            std::fs::create_dir(&cwd).unwrap();
            let state_dir = tmp.path().join("state");
            std::fs::create_dir_all(state_dir.join("checkout-preparation")).unwrap();
            let state_path = state_dir.join("checkout-preparation").join("wc-1.json");
            // The create path publishes NotStarted before spawning the
            // window; without it the shim's state-read refusal fires.
            write_preparation_state(
                &state_path,
                "wc-1",
                &PreparationState::NotStarted,
                &crate::files::RealFs,
            )
            .unwrap();
            let cwd_metadata = std::fs::metadata(&cwd).unwrap();
            let preparation = CheckoutPreparation {
                working_copy_id: "wc-1".to_string(),
                clone_url: clone_url.to_string(),
                cwd: cwd.to_string_lossy().into_owned(),
                directory_device: cwd_metadata.dev(),
                directory_inode: cwd_metadata.ino(),
                post_clone,
                shell: shell.to_string(),
                state_path,
            };
            Self {
                spec_path: tmp.path().join("launch-spec.json"),
                preparation,
                tmp,
            }
        }

        /// The fixture bin directory that becomes the shim child's PATH.
        fn bin_dir(&self) -> PathBuf {
            self.tmp.path().join("bin")
        }

        /// Install an executable fixture script in the bin directory.
        fn install_script(&self, name: &str, body: &str) {
            let path = self.bin_dir().join(name);
            std::fs::write(&path, body).unwrap();
            let mut permissions = std::fs::metadata(&path).unwrap().permissions();
            permissions.set_mode(0o700);
            std::fs::set_permissions(&path, permissions).unwrap();
        }

        /// Run the real shim in a disposable child whose ENTIRE PATH is
        /// the fixture bin directory. Returns the child's error report
        /// (the shim's own error chain), its combined stdout+stderr (the
        /// "terminal" the child inherited), and its exit status.
        /// (Re)write the launch spec. Rewritten on EVERY run: the shim
        /// unlinks the spec the moment it has read it, so a restart needs
        /// a fresh copy.
        fn write_spec(&self) -> PathBuf {
            let spec = LaunchSpec {
                argv: FAKE_AGENT_ARGV.iter().map(|s| s.to_string()).collect(),
                status_file: status_path_for_spec(&self.spec_path),
                session_id: "prep-session".to_string(),
                session_token: "prep-token".to_string(),
                supervisor_sock: self.tmp.path().join("supervisor.sock"),
                farhelm_bin_dir: self.bin_dir(),
                preparation: Some(self.preparation.clone()),
            };
            std::fs::write(&self.spec_path, serde_json::to_vec(&spec).unwrap()).unwrap();
            self.spec_path.clone()
        }

        fn run_shim_in_child(&self) -> (String, String, std::process::ExitStatus) {
            self.run_shim_in_child_with(None, &[], &[])
        }

        /// The runner with overrides: `path_override` replaces the
        /// child's PATH (whose default is the fixture bin dir alone) and
        /// `envs` adds child-only variables (D2's GIT_CONFIG_*). Both are
        /// Command-level; the test process's own environment is never
        /// touched.
        fn run_shim_in_child_with(
            &self,
            path_override: Option<&std::ffi::OsStr>,
            envs: &[(&str, &std::ffi::OsStr)],
            env_removes: &[&str],
        ) -> (String, String, std::process::ExitStatus) {
            self.write_spec();
            let exe = std::env::current_exe().expect("locate the supervisor test binary");
            let mut command = std::process::Command::new(&exe);
            command
                .args([
                    "--exact",
                    "launch::tests::prep_failure_child",
                    "--ignored",
                    "--nocapture",
                ])
                .env(PREP_CHILD_SPEC, &self.spec_path)
                .env(PREP_CHILD_KIND, "real");
            let path_value = path_override
                .map(std::ffi::OsStr::to_os_string)
                .unwrap_or_else(|| self.bin_dir().into_os_string());
            command.env("PATH", path_value);
            for (name, value) in envs {
                command.env(name, value);
            }
            for name in env_removes {
                command.env_remove(name);
            }
            let output = command.output().expect("re-run the prep child");
            let report = std::fs::read_to_string(self.spec_path.with_extension("child-error"))
                .unwrap_or_default();
            (
                report,
                format!(
                    "{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ),
                output.status,
            )
        }

        /// The durable state currently recorded for the fixture's working
        /// copy: `Err` on absent/corrupt (fail closed), `Ok(None)` never
        /// happens for a state file the create path published.
        fn state(&self) -> anyhow::Result<PreparationState> {
            Ok(
                read_preparation_state(&self.preparation.state_path, "wc-1")?
                    .expect("the fixture publishes NotStarted first")
                    .state,
            )
        }

        /// Whether the ordinary launch sentinel exists and contains
        /// `needle` — the same classification evidence the supervisor
        /// reads after any failed launch.
        fn sentinel_contains(&self, needle: &str) -> bool {
            std::fs::read_to_string(status_path_for_spec(&self.spec_path))
                .map(|c| c.contains(needle))
                .unwrap_or(false)
        }

        /// The preparation lock path (derived exactly as the shim does).
        fn lock_path(&self) -> PathBuf {
            preparation_lock_path(&self.preparation.state_path)
        }

        /// Spawn the shim child with null stdio and keep the handle, for
        /// the D4 tests that must kill it mid-run or hold contention
        /// evidence while it runs.
        fn spawn_shim(&self) -> std::process::Child {
            self.write_spec();
            let exe = std::env::current_exe().expect("locate the supervisor test binary");
            std::process::Command::new(&exe)
                .args([
                    "--exact",
                    "launch::tests::prep_failure_child",
                    "--ignored",
                    "--nocapture",
                ])
                .env(PREP_CHILD_SPEC, &self.spec_path)
                .env(PREP_CHILD_KIND, "real")
                .env("PATH", self.bin_dir())
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .expect("spawn the prep child")
        }
    }

    /// The ignored child helper every preparation test re-runs under its
    /// exact name: run the REAL shim and leave the error beside the spec,
    /// then exit without returning to libtest (same isolation reasoning
    /// as `exec_failure_child` above). The child's PATH is the fixture
    /// bin dir; its trace and wiring variables were set by the parent.
    #[farhelm_testtrace::test]
    #[ignore = "the child half of the checkout-preparation tests"]
    fn prep_failure_child() {
        let spec_path = std::env::var_os(PREP_CHILD_SPEC)
            .map(PathBuf::from)
            .expect("the parent must provide the child spec path");
        let kind = std::env::var(PREP_CHILD_KIND).expect("the parent must provide the kind");
        let error = match kind.as_str() {
            "real" => exec_launch_spec(&spec_path),
            "fail-at-write" => {
                struct FailEveryWrite;
                impl crate::files::FaultSeam for FailEveryWrite {
                    fn write(
                        &self,
                        _file: &mut std::fs::File,
                        _bytes: &[u8],
                    ) -> std::io::Result<()> {
                        Err(std::io::Error::other(
                            "injected durable-state write failure",
                        ))
                    }
                }
                exec_launch_spec_with_seam(&spec_path, &FailEveryWrite)
            }
            other => panic!("unknown prep child scenario: {other}"),
        };
        let wrote_error = std::fs::write(
            spec_path.with_extension("child-error"),
            format!("{error:#}"),
        )
        .is_ok();
        // SAFETY: nothing may return to libtest after the shim has run.
        unsafe { libc::_exit(if wrote_error { 0 } else { 1 }) }
    }

    /// Write one JSON line to the trace file `PREP_TRACE` names: the
    /// child's kind, its exact argv, and its cwd. Used by every fixture
    /// script via a tiny JSON emitter; `argv_json` is the shell-quoted
    /// JSON array the script template builds.
    /// The fake `git` the D1/D3/D4 fixtures install: records its exact
    /// argv and cwd as one JSON line, refuses a second invocation (D3's
    /// counting), creates the git marker proving the clone "ran", and —
    /// when the wiring file says so — fails instead.
    fn fake_git_script(argv_record: &str, marker: &str) -> String {
        format!(
            r#"#!/bin/sh
# One invocation only: D3 counts invocations via this marker.
if [ -f "{marker}.git-ran" ]; then
    echo "fake git: invoked twice" >&2
    exit 99
fi
: > "{marker}.git-ran"
# Record the EXACT argv, one file per element, plus this process's cwd.
i=0
for a in "$@"; do
    printf '%s' "$a" > "{argv_record}.$i"
    i=$((i+1))
done
printf '%s' "$i" > "{argv_record}.count"
printf '%s' "$(pwd)" > "{argv_record}.cwd"
# The CLOEXEC evidence: while THIS child runs, the shim holds the
# preparation flock; list this process's descriptors.
# Absolute paths: the child-only PATH contains ONLY fixture scripts.
/bin/ls -l /proc/self/fd > "{argv_record}.fds" 2>/dev/null || true
# Optional nonzero exit, wired per test. Leaves a partial file inside
# the clone target first, so D3 can prove it is retained untouched.
if [ -f "{argv_record}.git-fail" ]; then
    printf 'partial\n' > "$4/.git-partial" 2>/dev/null || true
    exit 1
fi
# The marker the hook requires: the ORDER proof.
: > "{marker}"
"#
        )
    }

    /// The fake hook shell the D1/D3/D4 fixtures install as the
    /// preparation's `shell`: it is what `<shell> -c <hook>` resolves to.
    /// Records its exact argv (proving `-c <hook>` shape), requires the
    /// git marker (order proof), writes the hook marker the fake agent
    /// requires, and optionally blocks/fails per wiring.
    fn fake_hook_shell_script(argv_record: &str, marker: &str, git_marker: &str) -> String {
        format!(
            r#"#!/bin/sh
# Append before any other action so a forbidden repeat cannot hide behind
# an unchanged completion marker or overwritten argv files.
printf 'hook\n' >> "{argv_record}.calls"
# Record the EXACT argv (-c <hook text>), one file per element.
i=0
for a in "$@"; do
    printf '%s' "$a" > "{argv_record}.$i"
    i=$((i+1))
done
printf '%s' "$i" > "{argv_record}.count"
printf '%s' "$(pwd)" > "{argv_record}.cwd"
# D4: block on the release fifo named by the wiring file, then proceed —
# no readiness is ever read from elapsed time.
if [ -f "{argv_record}.hook-block" ]; then
    release="$(/bin/cat "{argv_record}.hook-block")"
    exec 3<>"$release"
    # Publish only after opening the fifo: an earlier signal lets the
    # writer close before any reader exists, discarding the release byte.
    printf '%s' "$$" > "{argv_record}.hook-ready"
    read -r _ <&3 || true
fi
# The ORDER proof: the hook may only run after the clone completed.
if [ ! -f "{git_marker}" ]; then
    echo "fake hook: git marker missing - hook ran before clone" >&2
    exit 42
fi
# Optional nonzero exit, wired per test.
if [ -f "{argv_record}.hook-fail" ]; then exit 43; fi
: > "{marker}"
# Run the actual hook body the shim passed us.
exec /bin/sh "$@"
"#
        )
    }

    /// The fake agent the D1/D3/D4 fixtures install as the spec's argv[0]
    /// (found through the child-only PATH): requires the hook marker (the
    /// third link of the order chain), then signals it ran.
    fn fake_agent_script(marker: &str, hook_marker: &str) -> String {
        format!(
            r#"#!/bin/sh
if [ ! -f "{hook_marker}" ]; then
    echo "fake agent: hook marker missing — agent ran before hook" >&2
    exit 41
fi
printf 'AGENT-RAN\n'
: > "{marker}"
"#
        )
    }

    /// Read one fixture argv record (the per-element files the fake git
    /// or hook shell wrote): `(argv, cwd)`.
    fn read_recorded_argv(prefix: &Path) -> (Vec<String>, String) {
        let count: usize =
            std::fs::read_to_string(PathBuf::from(format!("{}.count", prefix.display())))
                .unwrap_or_else(|e| panic!("fixture argv count at {prefix:?}: {e}"))
                .trim()
                .parse()
                .unwrap_or_else(|e| panic!("fixture argv count not a number: {e}"));
        let argv = (0..count)
            .map(|i| {
                std::fs::read_to_string(PathBuf::from(format!("{}.{i}", prefix.display())))
                    .unwrap_or_else(|e| panic!("fixture argv element {i}: {e}"))
            })
            .collect();
        let cwd = std::fs::read_to_string(PathBuf::from(format!("{}.cwd", prefix.display())))
            .unwrap_or_else(|e| panic!("fixture argv cwd: {e}"));
        (argv, cwd)
    }

    /// Run one D-suite happy-path scenario: NotStarted state, fake git
    /// succeeds, hook (when configured) succeeds, fake agent runs.
    /// Returns the fixture and the child's outputs for the caller's
    /// assertions. `hook_body` is the `-c` payload the hook shell
    /// receives (may be empty for no hook); with a hook, the
    /// preparation's `shell` names the fixture's fake hook shell.
    fn build_d1_fixture(hook_body: Option<String>, marker: &Path) -> PrepFixture {
        let git_marker = marker.with_extension("git");
        let hook_marker = marker.with_extension("hook");
        let agent_marker = marker.with_extension("agent");
        let shell = if hook_body.is_some() {
            "fixture-hook-sh".to_string()
        } else {
            "/bin/sh".to_string()
        };
        let fixture = PrepFixture::new("https://github.com/example/repo.git", hook_body, &shell);
        let git_argv_record = marker.with_extension("git.argv");
        let hook_argv_record = marker.with_extension("hook.argv");
        fixture.install_script(
            "git",
            &fake_git_script(
                &git_argv_record.to_string_lossy(),
                &git_marker.to_string_lossy(),
            ),
        );
        if fixture.preparation.post_clone.is_some() {
            fixture.install_script(
                "fixture-hook-sh",
                &fake_hook_shell_script(
                    &hook_argv_record.to_string_lossy(),
                    &hook_marker.to_string_lossy(),
                    &git_marker.to_string_lossy(),
                ),
            );
        }
        fixture.install_script(
            "fake-agent",
            &fake_agent_script(
                &agent_marker.to_string_lossy(),
                &hook_marker.to_string_lossy(),
            ),
        );
        fixture
    }

    /// A supervisor nested inside another session must not pass that
    /// session's bearer/socket to Git or a hook. Only the final agent gets
    /// the new spec's authority; all input environment belongs to a child.
    #[farhelm_testtrace::test]
    fn preparation_scrubs_inherited_authority_and_agent_gets_its_own() {
        use std::ffi::OsStr;

        let fixture = PrepFixture::new(
            "https://github.com/example/repo.git",
            Some("fixture hook".to_string()),
            "fixture-hook-sh",
        );
        for (program, stage) in [
            ("git", "git"),
            ("fixture-hook-sh", "hook"),
            ("fake-agent", "agent"),
        ] {
            fixture.install_script(program, &format!(
                "#!/bin/sh\nprintf '%s\\n%s\\n' \"${{FARHELM_SESSION_TOKEN-unset}}\" \"${{FARHELM_SUPERVISOR_SOCK-unset}}\" > '{}'\n",
                fixture.tmp.path().join(stage).display(),
            ));
        }
        let (report, terminal, status) = fixture.run_shim_in_child_with(
            None,
            &[
                (SESSION_TOKEN_ENV_VAR, OsStr::new("outer-token")),
                (
                    SUPERVISOR_SOCK_ENV_VAR,
                    OsStr::new("/outer/supervisor.sock"),
                ),
            ],
            &[],
        );
        assert!(status.success(), "{report}: {terminal}");
        assert_eq!(fixture.state().unwrap(), PreparationState::Ready);
        for stage in ["git", "hook"] {
            assert_eq!(
                std::fs::read_to_string(fixture.tmp.path().join(stage)).unwrap(),
                "unset\nunset\n",
                "{stage} must run without inherited authority",
            );
        }
        assert_eq!(
            std::fs::read_to_string(fixture.tmp.path().join("agent")).unwrap(),
            format!(
                "prep-token\n{}\n",
                fixture.tmp.path().join("supervisor.sock").display()
            ),
        );
    }

    /// Build the standard fixture, apply the caller's per-test wiring
    /// (state overwrites, failure wiring files), run the REAL shim in a
    /// child, and hand back everything worth asserting on.
    fn run_d1_style_fixture(
        hook_body: Option<String>,
        marker: &Path,
    ) -> (PrepFixture, String, String, std::process::ExitStatus) {
        let fixture = build_d1_fixture(hook_body, marker);
        let (report, terminal, status) = fixture.run_shim_in_child();
        (fixture, report, terminal, status)
    }

    /// Whether NO preparation child ran in this fixture: none of the
    /// three stage markers exists. Used by every refusal test.
    fn no_stage_ran(_fixture: &PrepFixture, marker: &Path) -> bool {
        let git_ran = PathBuf::from(format!(
            "{}.git-ran",
            marker.with_extension("git").display()
        ));
        !git_ran.exists()
            && !marker.with_extension("hook").exists()
            && !marker.with_extension("agent").exists()
    }

    /// D1: the real shim, driving a fake `git`, a fake hook shell, and a
    /// fake agent — all in a child-only PATH — must run the three stages
    /// IN ORDER (the hook refuses to run before the git marker exists;
    /// the agent refuses before the hook marker), with the EXACT argv
    /// vectors recorded (`clone -- <url> <cwd>`; `shell -c <hook>`), and
    /// leave the durable state at Ready.
    #[farhelm_testtrace::test]
    fn d1_fake_git_hook_agent_run_in_order_with_exact_argv() {
        let marker = std::env::temp_dir().join(format!("prep-d1-{}", std::process::id()));
        let _ = std::fs::remove_file(&marker);
        let (fixture, report, terminal, status) =
            run_d1_style_fixture(Some("echo hook-body-ran".to_string()), &marker);
        assert!(
            status.success() && report.is_empty(),
            "the shim must complete the preparation without error: \
             status={status:?} report={report} terminal={terminal}"
        );
        // Exact argv of the clone: `clone -- <url> <absolute cwd>` —
        // asserted as a vector, never as a shell string.
        let (git_argv, _) = read_recorded_argv(&marker.with_extension("git.argv"));
        assert_eq!(
            git_argv,
            vec![
                "clone",
                "--",
                "https://github.com/example/repo.git",
                &fixture.preparation.cwd
            ]
        );
        // Exact argv of the hook shell: `-c <hook text>`.
        let (hook_argv, hook_cwd) = read_recorded_argv(&marker.with_extension("hook.argv"));
        assert_eq!(hook_argv[0], "-c");
        assert_eq!(hook_argv[1], "echo hook-body-ran");
        // The hook must have run INSIDE the checkout (cwd contract).
        assert_eq!(hook_cwd, fixture.preparation.cwd);
        // Order was proven inside the children (each refuses its
        // predecessor's marker), but assert the durable end state too.
        assert_eq!(fixture.state().unwrap(), PreparationState::Ready);
        // The lock file exists (the shim created it) but holds no lock
        // after the shim finished (the child exited, so the fd is gone).
        assert!(fixture.lock_path().exists());
        // The "terminal" (the child's inherited stdout/stderr) shows the
        // hook's echo and the agent's output, in that order: inherited
        // terminal fds, not captured pipes.
        let hook_pos = terminal.find("hook-body-ran").expect("hook output visible");
        let agent_pos = terminal.find("AGENT-RAN").expect("agent output visible");
        assert!(hook_pos < agent_pos, "hook output must precede the agent's");
        // The state file recorded Ready (asserted above), the sentinel
        // was never written (no failure).
        assert!(!fixture.sentinel_contains("exec_failed"));
    }

    /// D1's hostile-argv check: shell-hostile characters in the clone URL
    /// must REACH the fake git as data (one argv element), never execute
    /// as a command. The fake git records exactly what it received and
    /// the record must equal the hostile string verbatim; the shim itself
    /// refuses metacharacters, so this pins the VALIDATION side instead:
    /// a metacharacter-bearing URL must fail closed at `validate`.
    #[farhelm_testtrace::test]
    fn d1_hostile_url_is_refused_before_any_process_runs() {
        let marker = std::env::temp_dir().join(format!("prep-d1h-{}", std::process::id()));
        let _ = std::fs::remove_file(&marker);
        let fixture = PrepFixture::new(
            "https://github.com/example/repo.git; touch /tmp/pwned",
            None,
            "/bin/sh",
        );
        let (report, terminal, _status) = fixture.run_shim_in_child();
        assert!(
            !report.is_empty(),
            "the shim must refuse with a report: terminal={terminal}"
        );
        assert!(
            fixture.sentinel_contains("validate"),
            "the sentinel must name the validate stage: {}",
            fixture.sentinel_contains("")
        );
        assert!(
            report.contains("shell metacharacter"),
            "the error must name the refusal reason: {report}"
        );
        // Nothing ran: no state transition, no git, no agent.
        assert!(no_stage_ran(&fixture, &marker));
    }

    // ---------------------------------------------------------------------
    // D3: failure injection through the shim's real seams — clone spawn
    // failure (no `git` on the child-only PATH), clone nonzero exit, hook
    // nonzero exit, the durable-state write seam, and the restart rules.
    // Every scenario asserts the same fail-closed shape: no agent, the
    // stage error visible on the terminal, the ordinary sentinel, and the
    // durable state naming the failed stage.
    // ---------------------------------------------------------------------

    /// A unique per-test marker path (the fixture's argv records and
    /// wiring files hang off it).
    fn fresh_marker(name: &str) -> PathBuf {
        let marker = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_file(&marker);
        marker
    }

    /// D3: with NO `git` anywhere on the child-only PATH, the clone's
    /// spawn itself fails — the shim must record Failed{stage: clone},
    /// leave the ordinary sentinel, print the stage error, and never
    /// exec the agent or run the hook.
    #[farhelm_testtrace::test]
    fn d3_missing_git_binary_fails_closed_at_clone() {
        let marker = fresh_marker("prep-d3a");
        let fixture = build_d1_fixture(Some("echo hook-body-ran".to_string()), &marker);
        std::fs::remove_file(fixture.bin_dir().join("git")).unwrap();
        let (report, terminal, _status) = fixture.run_shim_in_child();
        assert!(
            report.contains("preparation failed at stage clone")
                && report.contains("could not start"),
            "the report must name the clone spawn failure: {report}"
        );
        assert!(fixture.sentinel_contains("preparation failed at stage clone"));
        match fixture.state().unwrap() {
            PreparationState::Failed { stage, detail } => {
                assert_eq!(stage, "clone");
                assert!(detail.contains("could not start"));
            }
            other => panic!("expected Failed at clone, got {other:?}"),
        }
        assert!(no_stage_ran(&fixture, &marker));
        assert!(
            terminal.contains("checkout preparation failed"),
            "the terminal must show the stage error: {terminal}"
        );
    }

    /// D3: a fake git that exits nonzero after leaving a partial file in
    /// the clone target must record Failed{stage: clone}, and the partial
    /// checkout must be retained untouched for inspection.
    #[farhelm_testtrace::test]
    fn d3_git_nonzero_exit_records_failed_clone_and_keeps_the_partial_checkout() {
        let marker = fresh_marker("prep-d3b");
        let fixture = build_d1_fixture(Some("echo hook-body-ran".to_string()), &marker);
        std::fs::write(
            PathBuf::from(format!(
                "{}.git-fail",
                marker.with_extension("git.argv").display()
            )),
            b"",
        )
        .unwrap();
        let (report, terminal, _status) = fixture.run_shim_in_child();
        assert!(
            report.contains("clone exited with status 1"),
            "the report must name the clone exit status: {report}"
        );
        assert!(fixture.sentinel_contains("preparation failed at stage clone"));
        match fixture.state().unwrap() {
            PreparationState::Failed { stage, .. } => assert_eq!(stage, "clone"),
            other => panic!("expected Failed at clone, got {other:?}"),
        }
        let partial = fixture.tmp.path().join("checkout").join(".git-partial");
        assert_eq!(
            std::fs::read_to_string(&partial).unwrap(),
            "partial\n",
            "the partial checkout file must be retained untouched"
        );
        assert!(
            !marker.with_extension("hook").exists(),
            "the hook never ran"
        );
        assert!(!terminal.contains("AGENT-RAN"), "the agent never ran");
    }

    /// D3: a hook that exits nonzero must record Failed{stage:
    /// post-clone} naming the stage and status WITHOUT logging the hook's
    /// own text (user content), and the agent must never run.
    #[farhelm_testtrace::test]
    fn d3_hook_nonzero_exit_names_post_clone_without_logging_the_command() {
        let marker = fresh_marker("prep-d3c");
        let fixture = build_d1_fixture(Some("echo hook-body-ran".to_string()), &marker);
        std::fs::write(
            PathBuf::from(format!(
                "{}.hook-fail",
                marker.with_extension("hook.argv").display()
            )),
            b"",
        )
        .unwrap();
        let (report, terminal, _status) = fixture.run_shim_in_child();
        assert!(
            report.contains("post-clone exited with status 43"),
            "the report must name the stage and status: {report}"
        );
        assert!(fixture.sentinel_contains("preparation failed at stage post-clone"));
        assert!(
            !fixture.sentinel_contains("hook-body-ran"),
            "the durable sentinel must never carry the hook's command text"
        );
        match fixture.state().unwrap() {
            PreparationState::Failed { stage, .. } => assert_eq!(stage, "post-clone"),
            other => panic!("expected Failed at post-clone, got {other:?}"),
        }
        assert!(
            PathBuf::from(format!(
                "{}.git-ran",
                marker.with_extension("git").display()
            ))
            .exists(),
            "the clone DID run before the hook failed"
        );
        assert!(!terminal.contains("AGENT-RAN"), "the agent never ran");
    }

    /// D3: with a seam that fails EVERY durable write, the very first
    /// transition (CloneStarted) aborts the launch before any process
    /// runs, the abort's own Failed and sentinel writes fail without
    /// destroying the pre-existing NotStarted record, and nothing runs.
    #[farhelm_testtrace::test]
    fn d3_state_write_failure_fails_closed_before_the_clone() {
        let marker = fresh_marker("prep-d3d");
        let fixture = build_d1_fixture(Some("echo hook-body-ran".to_string()), &marker);
        fixture.write_spec();
        let exe = std::env::current_exe().unwrap();
        let output = std::process::Command::new(&exe)
            .args([
                "--exact",
                "launch::tests::prep_failure_child",
                "--ignored",
                "--nocapture",
            ])
            .env(PREP_CHILD_SPEC, &fixture.spec_path)
            .env(PREP_CHILD_KIND, "fail-at-write")
            .env("PATH", fixture.bin_dir())
            .output()
            .expect("re-run the prep child");
        let report = std::fs::read_to_string(fixture.spec_path.with_extension("child-error"))
            .unwrap_or_default();
        assert!(
            report.contains("could not durably record CloneStarted"),
            "the first failed transition must be the root cause: {report}"
        );
        assert!(
            report.contains("could not record launch failure"),
            "the sentinel write's own failure must be surfaced too: {report}"
        );
        // The original NotStarted record is intact and NO sentinel was
        // published (its write failed too).
        assert_eq!(fixture.state().unwrap(), PreparationState::NotStarted);
        assert!(!status_path_for_spec(&fixture.spec_path).exists());
        assert!(no_stage_ran(&fixture, &marker), "nothing ran");
        assert!(
            output.status.success(),
            "the child must exit cleanly: {report}"
        );
    }

    /// D3: after a successful preparation, a restart must skip clone AND
    /// hook entirely and exec the agent directly — the fake git REFUSES a
    /// second invocation (exit 99), so any re-clone fails this loudly,
    /// and the hook's echo proves it did not run again either.
    #[farhelm_testtrace::test]
    fn d3_ready_restart_skips_clone_and_hook() {
        let marker = fresh_marker("prep-d3e");
        let (fixture, report1, terminal1, _status) =
            run_d1_style_fixture(Some("echo hook-body-ran".to_string()), &marker);
        assert!(report1.is_empty(), "first run must succeed: {report1}");
        assert_eq!(fixture.state().unwrap(), PreparationState::Ready);
        let (report2, terminal2, _status) = fixture.run_shim_in_child();
        assert!(report2.is_empty(), "restart must succeed: {report2}");
        assert_eq!(fixture.state().unwrap(), PreparationState::Ready);
        let hooks =
            terminal1.matches("hook-body-ran").count() + terminal2.matches("hook-body-ran").count();
        assert_eq!(hooks, 1, "the hook must never run again once Ready");
        let agents =
            terminal1.matches("AGENT-RAN").count() + terminal2.matches("AGENT-RAN").count();
        assert_eq!(agents, 2, "the agent runs on BOTH launches");
        let git_ran = PathBuf::from(format!(
            "{}.git-ran",
            marker.with_extension("git").display()
        ));
        assert!(git_ran.exists(), "the clone ran exactly once (the marker)");
        // The git argv record proves there was exactly ONE clone
        // invocation (a second would have failed with exit 99).
        let (_, _) = read_recorded_argv(&marker.with_extension("git.argv"));
        let (hook_argv2, _) = read_recorded_argv(&marker.with_extension("hook.argv"));
        assert_eq!(hook_argv2, vec!["-c", "echo hook-body-ran"]);
    }

    /// D3 restart rules: a durable CloneStarted (a crash mid-clone) must
    /// be REFUSED — no automatic retry, no agent, state preserved.
    #[farhelm_testtrace::test]
    fn d3_clone_started_restart_is_refused_without_rerunning_anything() {
        let marker = fresh_marker("prep-d3f");
        let fixture = build_d1_fixture(Some("echo hook-body-ran".to_string()), &marker);
        write_preparation_state(
            &fixture.preparation.state_path,
            "wc-1",
            &PreparationState::CloneStarted,
            &crate::files::RealFs,
        )
        .unwrap();
        let (report, terminal, _status) = fixture.run_shim_in_child();
        assert!(
            report.contains("did not finish") && report.contains("CloneStarted"),
            "the refusal must name the state and the no-retry rule: {report}"
        );
        assert!(fixture.sentinel_contains("preparation failed at stage restart"));
        assert_eq!(
            fixture.state().unwrap(),
            PreparationState::CloneStarted,
            "the refusal must not overwrite the state evidence"
        );
        assert!(no_stage_ran(&fixture, &marker));
        assert!(terminal.contains("checkout preparation failed"));
    }

    /// D3 restart rules: a durable Failed state must be refused with the
    /// recorded stage in the message, state preserved, nothing rerun.
    #[farhelm_testtrace::test]
    fn d3_failed_state_restart_is_refused() {
        let marker = fresh_marker("prep-d3g");
        let fixture = build_d1_fixture(Some("echo hook-body-ran".to_string()), &marker);
        write_preparation_state(
            &fixture.preparation.state_path,
            "wc-1",
            &PreparationState::Failed {
                stage: "clone".to_string(),
                detail: "git clone exited with status 1".to_string(),
            },
            &crate::files::RealFs,
        )
        .unwrap();
        let (report, _terminal, _status) = fixture.run_shim_in_child();
        assert!(
            report.contains("previously failed at stage") && report.contains("clone"),
            "the refusal must quote the recorded failure: {report}"
        );
        assert_eq!(
            fixture.state().unwrap(),
            PreparationState::Failed {
                stage: "clone".to_string(),
                detail: "git clone exited with status 1".to_string(),
            },
            "the refusal must not overwrite the state evidence"
        );
        assert!(no_stage_ran(&fixture, &marker));
        assert!(fixture.sentinel_contains("preparation failed at stage restart"));
    }

    /// D3 fail-closed contract: an ABSENT state file is an error, never a
    /// silent NotStarted — the create path publishes NotStarted before
    /// spawning the terminal, so absence means the evidence is gone.
    #[farhelm_testtrace::test]
    fn d3_absent_state_file_fails_closed() {
        let marker = fresh_marker("prep-d3h");
        let fixture = build_d1_fixture(Some("echo hook-body-ran".to_string()), &marker);
        std::fs::remove_file(&fixture.preparation.state_path).unwrap();
        let (report, _terminal, _status) = fixture.run_shim_in_child();
        assert!(
            report.contains("does not exist") && report.contains("never treated as NotStarted"),
            "absence must be refused loudly: {report}"
        );
        assert!(fixture.sentinel_contains("preparation failed at stage state-read"));
        assert!(
            !fixture.preparation.state_path.exists(),
            "the refusal must not recreate the state file"
        );
        assert!(no_stage_ran(&fixture, &marker));
    }

    /// D3 fail-closed contract: a corrupt state file is refused (and
    /// preserved) — a durability-bearing write never leaves torn content,
    /// so corruption means something beneath that promise went wrong.
    #[farhelm_testtrace::test]
    fn d3_corrupt_state_file_fails_closed() {
        let marker = fresh_marker("prep-d3i");
        let fixture = build_d1_fixture(Some("echo hook-body-ran".to_string()), &marker);
        std::fs::write(&fixture.preparation.state_path, b"{not json").unwrap();
        let (report, _terminal, _status) = fixture.run_shim_in_child();
        assert!(
            report.contains("not parseable"),
            "corruption must be refused loudly: {report}"
        );
        assert_eq!(
            std::fs::read(&fixture.preparation.state_path).unwrap(),
            b"{not json",
            "the refusal must preserve the corrupt file as evidence"
        );
        assert!(no_stage_ran(&fixture, &marker));
    }

    /// D3 identity check: a launch aimed at a directory that was REPLACED
    /// since allocation (device/inode no longer match) must be refused
    /// before any process runs, and record Failed{stage: cwd-identity}.
    #[farhelm_testtrace::test]
    fn d3_replaced_directory_fails_the_identity_check() {
        let marker = fresh_marker("prep-d3j");
        let mut fixture = build_d1_fixture(Some("echo hook-body-ran".to_string()), &marker);
        fixture.preparation.directory_device += 1;
        fixture.preparation.directory_inode += 1;
        let (report, _terminal, _status) = fixture.run_shim_in_child();
        assert!(
            report.contains("cwd-identity") && report.contains("was replaced"),
            "the identity refusal must be actionable: {report}"
        );
        match fixture.state().unwrap() {
            PreparationState::Failed { stage, .. } => assert_eq!(stage, "cwd-identity"),
            other => panic!("expected Failed at cwd-identity, got {other:?}"),
        }
        assert!(fixture.sentinel_contains("preparation failed at stage cwd-identity"));
        assert!(no_stage_ran(&fixture, &marker));
    }

    // ---------------------------------------------------------------------
    // D2: the REAL git, offline — a local bare repo made reachable
    // through the exact HTTPS fixture URL via `url.<base>.insteadOf`,
    // configured with GIT_CONFIG_* variables ON THE CHILD COMMAND (never
    // the test process). No network, no credentials.
    // ---------------------------------------------------------------------

    /// D2: the shim, running the REAL git through the insteadOf mapping,
    /// must produce a checkout holding the tracked file's content, with
    /// `origin` pointing at the REWRITTEN (local) url — and record Ready
    /// end to end, hook included.
    #[farhelm_testtrace::test]
    fn d2_real_git_offline_clones_the_instead_of_fixture() {
        let setup = tempfile::tempdir().unwrap();
        let work = setup.path().join("work");
        let bare = setup.path().join("fixture.git");
        std::fs::create_dir(&work).unwrap();

        // Setup git commands run in the test process with their
        // environment isolated ON THE COMMAND (never the process).
        let isolated_git = |args: &[&str]| -> std::process::Output {
            std::process::Command::new("git")
                .args(args)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_SYSTEM", "/dev/null")
                .env_remove("GIT_DIR")
                .env_remove("GIT_WORK_TREE")
                .env_remove("GIT_INDEX_FILE")
                .output()
                .expect("run setup git")
        };
        // Fixture premise: one commit with a tracked file, mirrored into
        // a bare repo, and the bare repo actually HAS that commit.
        std::fs::write(work.join("tracked-file.txt"), "tracked content\n").unwrap();
        for args in [
            vec!["init", "-q", work.to_str().unwrap()],
            vec!["-C", work.to_str().unwrap(), "add", "."],
            vec![
                "-C",
                work.to_str().unwrap(),
                "-c",
                "user.name=fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "-q",
                "-m",
                "fixture",
            ],
        ] {
            let out = isolated_git(&args);
            assert!(
                out.status.success(),
                "setup git {args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        let out = isolated_git(&[
            "clone",
            "-q",
            "--bare",
            work.to_str().unwrap(),
            bare.to_str().unwrap(),
        ]);
        assert!(
            out.status.success(),
            "bare clone: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let out = isolated_git(&["-C", bare.to_str().unwrap(), "rev-parse", "HEAD"]);
        assert!(
            out.status.success(),
            "premise: the bare repo must hold the commit: {}",
            String::from_utf8_lossy(&out.stderr)
        );

        // A fixture WITHOUT a fake git: its bin dir stays empty, and the
        // child's PATH gains the ambient PATH after it so the REAL git
        // resolves. The hook runs under the real /bin/sh.
        let hook_marker = setup.path().join("d2-hook-marker");
        let agent_marker = setup.path().join("d2-agent-marker");
        let hook_body = format!("touch {}", hook_marker.display());
        let fixture = PrepFixture::new(
            "https://github.com/example/repo",
            Some(hook_body),
            "/bin/sh",
        );
        fixture.install_script(
            "fake-agent",
            &fake_agent_script(
                &agent_marker.to_string_lossy(),
                &hook_marker.to_string_lossy(),
            ),
        );
        let child_path = format!(
            "{}:{}",
            fixture.bin_dir().display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let instead_of_key = format!("url.{}.insteadOf", bare.display());
        let envs: Vec<(&str, &std::ffi::OsStr)> = vec![
            ("GIT_CONFIG_COUNT", std::ffi::OsStr::new("1")),
            ("GIT_CONFIG_KEY_0", std::ffi::OsStr::new(&instead_of_key)),
            (
                "GIT_CONFIG_VALUE_0",
                std::ffi::OsStr::new("https://github.com/example/repo"),
            ),
            ("GIT_CONFIG_GLOBAL", std::ffi::OsStr::new("/dev/null")),
            ("GIT_CONFIG_SYSTEM", std::ffi::OsStr::new("/dev/null")),
        ];
        let (report, terminal, _status) = fixture.run_shim_in_child_with(
            Some(std::ffi::OsStr::new(&child_path)),
            &envs,
            // The ambient git context (this test runner's own
            // GIT_DIR/GIT_WORK_TREE) must never reach the shim's git.
            &["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE"],
        );
        assert!(
            report.is_empty(),
            "the full preparation must succeed end to end: {report} {terminal}"
        );
        assert_eq!(fixture.state().unwrap(), PreparationState::Ready);
        // The clone really happened: the tracked file is IN the checkout.
        let tracked = std::fs::read_to_string(fixture.tmp.path().join("checkout/tracked-file.txt"))
            .expect("tracked file in the checkout");
        assert_eq!(tracked, "tracked content\n");
        // Origin behavior: git stores the ORIGINAL url in the config and
        // applies `insteadOf` at fetch/push/read time, so the rewrite is
        // observable only WITH the mapping env applied to the read
        // command too — exactly how a later fetch from this checkout
        // would behave.
        let out = std::process::Command::new("git")
            .args([
                "-C",
                fixture.tmp.path().join("checkout").to_str().unwrap(),
                "remote",
                "get-url",
                "origin",
            ])
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", &instead_of_key)
            .env("GIT_CONFIG_VALUE_0", "https://github.com/example/repo")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .output()
            .expect("read origin url");
        assert!(
            out.status.success(),
            "reading origin: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            bare.to_str().unwrap(),
            "origin must resolve to the insteadOf-rewritten (local) url"
        );
        assert!(
            hook_marker.exists(),
            "the post-clone hook ran in the real checkout"
        );
        assert!(agent_marker.exists(), "the agent ran after the real clone");
    }

    // ---------------------------------------------------------------------
    // D4: interruption and concurrency. The blocking hook parks on a
    // fifo (a pipe oracle, never a sleep); the killed shim's restart is
    // itself bounded through the same file oracles, so a leaked lock
    // (a CLOEXEC regression) would fail these tests, not hang them.
    // ---------------------------------------------------------------------

    /// Bounded polling for a file-oracle: `predicate` is the readiness
    /// oracle; the deadline only bounds a failed wait. Every caller's
    /// predicate observes fixture files, never elapsed time alone. The
    /// 30 s default bound is generous ON PURPOSE: a fully loaded nextest
    /// run (100 launch tests over 4 slots, each re-running the test
    /// binary as a child) makes one child startup cost seconds, and a
    /// false timeout would look exactly like the bug the suite exists to
    /// catch.
    fn poll_until(deadline: Duration, mut predicate: impl FnMut() -> bool) -> bool {
        let started = std::time::Instant::now();
        loop {
            if predicate() {
                return true;
            }
            if started.elapsed() >= deadline {
                return false;
            }
            // sleep-ok: polling pace behind the file oracle above, which alone decides readiness.
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// The D4 fixture: hook shell wired to park on a fifo until the
    /// parent releases it. Returns (fixture, fifo path, hook record
    /// prefix).
    fn build_d4_fixture(marker: &Path) -> (PrepFixture, PathBuf) {
        let fixture = build_d1_fixture(Some("echo hook-body-ran".to_string()), marker);
        let fifo = fixture.tmp.path().join("hook-release.fifo");
        let c_fifo = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
        // SAFETY: mkfifo on a fresh path inside the fixture tempdir.
        assert_eq!(unsafe { libc::mkfifo(c_fifo.as_ptr(), 0o600) }, 0);
        std::fs::write(
            PathBuf::from(format!(
                "{}.hook-block",
                marker.with_extension("hook.argv").display()
            )),
            fifo.to_string_lossy().as_bytes(),
        )
        .unwrap();
        (fixture, fifo)
    }

    /// D4: kill the shim WHILE the hook runs. The durable HookStarted
    /// record (written BEFORE the hook spawned) must make the restart
    /// refuse: no second hook execution, no agent spawn, partial files
    /// retained. The hook itself must exit after release (cleanup proof).
    #[farhelm_testtrace::test]
    fn d4_killed_shim_never_repeats_the_hook_or_spawns_the_agent() {
        let marker = fresh_marker("prep-d4a");
        let (fixture, fifo) = build_d4_fixture(&marker);
        let hook_record = marker.with_extension("hook.argv");
        let git_ran = PathBuf::from(format!(
            "{}.git-ran",
            marker.with_extension("git").display()
        ));

        let mut child = fixture.spawn_shim();
        let child_pid = child.id();
        // The PID is published only after the hook has opened the FIFO.
        // An argv marker precedes that open and cannot establish readiness.
        let hook_entered = PathBuf::from(format!("{}.hook-ready", hook_record.display()));
        assert!(
            poll_until(Duration::from_secs(30), || {
                std::fs::read_to_string(&hook_entered).is_ok_and(|text| text.parse::<u32>().is_ok())
            }),
            "the blocking hook must publish its PID after opening the FIFO; shim status: {:?}",
            child.try_wait()
        );
        let hook_pid: u32 = std::fs::read_to_string(&hook_entered)
            .unwrap()
            .parse()
            .unwrap();
        let (parent, hook_start, hook_state) = crate::procs::read_process(hook_pid)
            .expect("inspect the hook identity")
            .expect("hook is alive at readiness");
        assert_eq!(parent, child_pid, "the hook belongs to this shim");
        assert_eq!(hook_state, crate::procs::ProcessState::Running);
        let hook_calls = PathBuf::from(format!("{}.calls", hook_record.display()));
        assert_eq!(std::fs::read(&hook_calls).unwrap(), b"hook\n");
        // HookStarted was durably written BEFORE the hook could spawn.
        assert_eq!(fixture.state().unwrap(), PreparationState::HookStarted);
        // Crash the shim mid-hook.
        // SAFETY: SIGKILL to the spawned child's pid.
        assert_eq!(
            unsafe { libc::kill(child_pid as libc::pid_t, libc::SIGKILL) },
            0
        );
        child.wait().expect("reap the killed shim");

        // Cleanup proof: release the parked hook and watch it exit.
        assert!(
            matches!(crate::procs::read_process(hook_pid).unwrap(),
                Some((_, start, crate::procs::ProcessState::Running)) if start == hook_start
            ),
            "the same hook must still be alive before release"
        );
        {
            use std::io::Write as _;
            let mut writer = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&fifo)
                .expect("open the release fifo");
            writer.write_all(b"r\n").expect("write the release byte");
        }
        let hook_marker = marker.with_extension("hook");
        assert!(
            poll_until(Duration::from_secs(30), || hook_marker.exists()),
            "the orphaned hook must finish its fixture work after release"
        );
        // The marker is written before exec and therefore cannot prove exit.
        // A zombie has exited but belongs to its adoptive parent's reap; PID
        // reuse also proves this recorded process ended. Read errors fail.
        assert!(
            poll_until(Duration::from_secs(30), || {
                match crate::procs::read_process(hook_pid).expect("inspect hook exit") {
                    None => true,
                    Some((_, start, state)) => {
                        start != hook_start || state == crate::procs::ProcessState::Zombie
                    }
                }
            }),
            "owned hook did not exit: {:?}",
            crate::procs::read_process(hook_pid)
        );

        // Restart: HookStarted must refuse. Bounded through the
        // child-error oracle: a leaked lock would block the restart
        // forever and this poll would time out instead.
        let mut restart = fixture.spawn_shim();
        let refused = poll_until(Duration::from_secs(30), || {
            fixture.spec_path.with_extension("child-error").exists()
        });
        let _ = restart.kill();
        restart.wait().expect("reap the restart");
        assert!(
            refused,
            "the restart must refuse in bounded time (HookStarted)"
        );
        let report =
            std::fs::read_to_string(fixture.spec_path.with_extension("child-error")).unwrap();
        assert!(
            report.contains("did not finish") && report.contains("HookStarted"),
            "the refusal must name the state and the no-retry rule: {report}"
        );
        // The state evidence is preserved; nothing was erased.
        assert_eq!(fixture.state().unwrap(), PreparationState::HookStarted);
        assert!(
            git_ran.exists(),
            "the partial checkout evidence is retained"
        );
        assert!(!marker.with_extension("agent").exists(), "no agent spawn");
        assert!(fixture.sentinel_contains("preparation failed at stage restart"));
        assert_eq!(
            std::fs::read(hook_calls).unwrap(),
            b"hook\n",
            "refused restart must not invoke the hook again"
        );
    }

    /// Reap a disposable shim even when a readiness assertion panics.
    /// The contention fixture cannot leave a child parked on a lock after
    /// its owning test has failed and released the fixture directory.
    #[cfg(target_os = "linux")]
    struct ReapedPrepChild(std::process::Child);

    #[cfg(target_os = "linux")]
    impl Drop for ReapedPrepChild {
        fn drop(&mut self) {
            if !matches!(self.0.try_wait(), Ok(Some(_))) {
                let _ = self.0.kill();
            }
            let _ = self.0.wait();
        }
    }

    /// Linux reports blocked flock waiters separately from held locks.
    /// Match the owned child's PID and the exact device/inode so process
    /// startup, an unrelated lock, or elapsed time cannot satisfy readiness.
    #[cfg(target_os = "linux")]
    fn preparation_flock_is_waiting(pid: u32, lock: &std::fs::File) -> bool {
        use std::io::Read;

        let metadata = lock.metadata().unwrap();
        let object = format!(
            "{:02x}:{:02x}:{}",
            libc::major(metadata.dev()),
            libc::minor(metadata.dev()),
            metadata.ino(),
        );
        let mut rows = String::new();
        std::fs::File::open("/proc/locks")
            .unwrap()
            .take(1024 * 1024 + 1)
            .read_to_string(&mut rows)
            .unwrap();
        assert!(
            rows.len() <= 1024 * 1024,
            "bounded lock observation exceeded"
        );
        rows.lines().any(|row| {
            let fields: Vec<_> = row.split_whitespace().collect();
            fields.len() == 9
                && fields[1..5] == ["->", "FLOCK", "ADVISORY", "WRITE"]
                && fields[5].parse::<u32>() == Ok(pid)
                && fields[6] == object
        })
    }

    /// D4: observe the owned shim blocked in the production flock before
    /// checking that preparation has not started. Releasing the same lock
    /// must let the entire setup finish. Linux's kernel waiter record makes
    /// this discriminate a missing lock even under delayed child startup.
    #[farhelm_testtrace::test]
    #[cfg(target_os = "linux")]
    fn d4_second_concurrent_launch_waits_for_the_lock() {
        let marker = fresh_marker("prep-d4b");
        let fixture = build_d1_fixture(Some("echo hook-body-ran".to_string()), &marker);

        // The TEST process holds the lock (opened and flocked here; the
        // spawned contender must contend through its own open because
        // the descriptor is CLOEXEC).
        let held = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(fixture.lock_path())
            .unwrap();
        // SAFETY: flock LOCK_EX on a valid descriptor.
        assert_eq!(unsafe { libc::flock(held.as_raw_fd(), libc::LOCK_EX) }, 0);
        let mut contender = ReapedPrepChild(fixture.spawn_shim());
        assert!(
            poll_until(Duration::from_secs(30), || {
                assert!(
                    contender.0.try_wait().unwrap().is_none(),
                    "contender exited before blocking"
                );
                preparation_flock_is_waiting(contender.0.id(), &held)
            }),
            "owned shim never became a waiter on the held preparation lock; state: {:?}",
            fixture.state(),
        );
        assert!(
            !PathBuf::from(format!(
                "{}.git-ran",
                marker.with_extension("git").display()
            ))
            .exists(),
            "no clone may run while the lock is held"
        );
        assert!(
            !fixture.sentinel_contains(""),
            "no failure sentinel may exist while the lock is held"
        );

        // Release: the contender completes the WHOLE preparation.
        drop(held);
        assert!(
            poll_until(Duration::from_secs(30), || marker
                .with_extension("agent")
                .exists()),
            "after release the preparation must complete (git-ran: {})",
            PathBuf::from(format!(
                "{}.git-ran",
                marker.with_extension("git").display()
            ))
            .exists()
        );
        let status = contender.0.wait().expect("reap the contender");
        assert!(status.success(), "the contender exits cleanly");
        assert_eq!(fixture.state().unwrap(), PreparationState::Ready);
        assert!(!fixture.sentinel_contains(""), "no sentinel on success");
    }

    /// CLOEXEC proof (Linux): the fake git lists its OWN /proc fds while
    /// the shim holds the preparation flock — no descriptor in that
    /// listing may name the lock file. This is the in-situ proof chosen
    /// over a synthetic probe: it is the REAL child, under the REAL
    /// lock, at the moment the leak would matter. (Non-Linux unixes lack
    /// /proc; the crate is unconditionally unix-only and this proof is
    /// Linux-gated.)
    #[farhelm_testtrace::test]
    #[cfg(target_os = "linux")]
    fn d4_no_preparation_child_inherits_the_flock() {
        let marker = fresh_marker("prep-d4c");
        let (fixture, report, _terminal, _status) =
            run_d1_style_fixture(Some("echo hook-body-ran".to_string()), &marker);
        assert!(report.is_empty(), "the run must succeed: {report}");
        let fds = std::fs::read_to_string(PathBuf::from(format!(
            "{}.fds",
            marker.with_extension("git.argv").display()
        )))
        .expect("the fake git must have listed its fds");
        assert!(fds.contains("->"), "the listing must be real: {fds}");
        let lock_name = fixture
            .lock_path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(
            !fds.lines().any(|line| line.contains(&lock_name)),
            "no preparation child may inherit the flock descriptor: {fds}"
        );
    }
}
