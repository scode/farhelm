//! The supervisor service: farhelm-proto over a unix socket.
//!
//! This is the only doorway to sessions — the helm (local or via the ssh
//! stdio proxy) and every future caller speak the same protocol to the
//! same handlers, which is what keeps "CLI flags bypass the creation UI,
//! never the creation API" true. The supervisor listens on no network
//! port (SPEC.md): the unix socket plus ssh exec is the entire reachable
//! surface.
//!
//! M2 scope: SQLite (`crate::store`) is the truth that a session exists
//! and what its metadata is, written at creation and reloaded at startup
//! so sessions survive a supervisor restart. tmux stays the truth for
//! whether a session's terminal is currently alive; a session whose own
//! tmux session no longer exists — whether because the whole private tmux
//! server did not survive a restart, or because just that one session was
//! killed independently — is still listed (metadata came back from the
//! DB) but loses its terminal handle — see `SessionEntry`'s `terminal`
//! field and the `Attach`/`Resize` handlers' handling of `None`.
//! PLAN_M2.md's "restart gap" paragraph is the contract; M3 replaces this
//! crude exited-unknown answer with real interrupted classification.

use crate::launch::{LaunchSpec, resolve_shell, window_command};
use crate::store::{SessionStore, StoredSession};
use crate::tmux::{InputClient, PaneState, TmuxDriver};
use anyhow::Context;
use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
use farhelm_proto::{ControlMsg, ErrorKind, Frame, SessionInfo, SessionStatus};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::{info, warn};

/// Data-frame chunk size for replay. Well under MAX_FRAME_LEN; small
/// enough that the first screenful renders while the rest streams.
const REPLAY_CHUNK: usize = 32 * 1024;

/// Combined byte cap on `CreateSession`'s `cwd` + `invocation` + `title`,
/// enforced before `create_session` does anything.
///
/// Without this, a request whose fields nearly fill `MAX_FRAME_LEN` can
/// succeed at creating the session, and only then discover that its
/// `SessionCreated` reply — the same fields again, plus the generated id
/// and the frame wrapper — exceeds the cap and gets degraded to an
/// `Error` by `reply_frame`. That leaves the session alive while the
/// caller is told the request failed, with no way to learn the id needed
/// to attach to (or tear down) the very session it just created. 64 KiB
/// is orders of magnitude beyond any real cwd, invocation, or title —
/// each of which must also survive being embedded in a tmux command line
/// — so capping the inputs this far below the frame limit makes an
/// oversized `SessionCreated` reply structurally impossible, and does so
/// before `create_session` has touched tmux or the filesystem.
const CREATE_FIELD_CAP: usize = 64 * 1024;

/// The no-progress window `handle_connection`'s shutdown tail allows the
/// writer task before giving up on it — NOT a total deadline on the drain.
///
/// "Drain everything queued" and "never hang" cannot both hold against a
/// peer that stops reading without erroring (a full TCP/pipe window — a
/// wedged ssh session, say): the writer's `write_frame` call just parks,
/// there is no error for the `writer_failed` oneshot to carry, and without
/// a bound the connection task (plus its entire backlog) leaks for the
/// process lifetime. But a flat deadline over-punishes a peer that is
/// merely slow rather than gone — backpressured but still reading, so
/// frames keep landing, just not inside any one fixed window — and killing
/// that connection breaks the half-close contract that replies already
/// queued before shutdown began still reach a peer that is still reading.
/// `drain_writer` resets this window every time a frame completes, so a
/// slow-but-live peer gets unbounded total time to drain; only a peer that
/// lands zero frames across one whole window is treated as gone. See
/// `drain_writer` for the honest residual this still leaves (a single
/// frame slower than the window). This only bounds shutdown —
/// steady-state backpressure while a connection is still active is out of
/// scope here (M2.5, PLAN.md).
///
/// Interplay with `HANDLER_SHUTDOWN_TIMEOUT`: that bound runs FIRST, and
/// is what this one implicitly assumes has already happened. Every slow
/// handler task (`ListSessions`/`StopSession`/`DeleteSession`) that
/// finishes within `HANDLER_SHUTDOWN_TIMEOUT`'s window enqueues its reply
/// exactly like a synchronous handler would, and THIS window is what then
/// gets that reply to a still-reading peer. A straggling task aborted by
/// `HANDLER_SHUTDOWN_TIMEOUT` instead, by contrast, never enqueues
/// anything at all — there is nothing left for this drain to wait out on
/// its behalf, and its own multi-second tmux work is exactly why it gets
/// a separate, longer budget rather than being folded into this one.
const WRITER_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

/// Cap on slow handler tasks (`ListSessions`/`StopSession`/`DeleteSession`
/// — see their own arms' comments in `handle_control` on why they're
/// spawned rather than awaited inline) allowed in flight AT ONCE across
/// the WHOLE supervisor process (`Supervisor::admission`), not per
/// connection: the resource actually being bounded — tmux subprocesses,
/// `/proc` sweeps — is process-global, and a per-connection cap would let
/// every additional helm connection multiply the real concurrency by
/// another 8, defeating the point of having a bound at all. A permit is
/// acquired (via `spawn_admitted`) BEFORE spawning each task — in the
/// caller's own await point, which for every real caller is
/// `handle_control`, itself driven directly from `handle_connection`'s
/// read loop — so an unbounded flood of slow requests backpressures
/// whichever connection sent them once the cap is hit, rather than
/// spawning an unbounded number of tasks each holding a tmux subprocess or
/// a multi-second kill sweep open. Acquiring INSIDE the spawned task
/// instead would still bound how many run concurrently, but would not
/// bound how many accumulate — every request would still spawn (and
/// `JoinSet` would still track) a task immediately, just one that sits
/// parked on the semaphore; that is exactly the unbounded-queuing failure
/// mode this ordering exists to close. 8 is generous headroom for
/// ordinary use (a polling UI keeps at most one `ListSessions` in flight
/// per connection at a time) while still being a REAL bound against a
/// pathological flood or a buggy client that fires requests without
/// waiting for replies.
const HANDLER_ADMISSION_PERMITS: usize = 8;

/// How long `handle_connection`'s shutdown tail waits for spawned slow-
/// handler tasks (see `HANDLER_ADMISSION_PERMITS`) to finish on their own,
/// tracked in a `JoinSet`, before aborting whatever remains and logging
/// it. Generous — `kill_process_tree`'s own sequence (grace period,
/// quiesce passes, kill confirmation) can legitimately take several
/// seconds — but not unbounded: a wedged tmux must not leak a task (and
/// this connection's own shutdown) forever. See `WRITER_DRAIN_TIMEOUT`'s
/// docs for how the two windows interact.
const HANDLER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

/// A classified request failure: attached at the few call sites that
/// actually know *why* a request failed (bad cwd, unparseable invocation,
/// ...), and recovered later by `error_kind` to pick the `ControlMsg::Error`
/// reply's `kind`.
///
/// This is deliberately the only place `ErrorKind` gets decided: everything
/// else that can fail (a tmux hiccup, an I/O error reading directory
/// metadata) has no opinion on classification and is left to default to
/// `ErrorKind::Internal` via `error_kind`'s fallback — every fallible call
/// site does not have to pick a kind for errors it cannot actually
/// distinguish. (Some handlers build a classified `ControlMsg::Error` reply
/// directly instead of going through `anyhow`, e.g. the `Attach` handler's
/// channel-in-use check — this type covers only the `anyhow::Result`-typed
/// paths through `create_session`.)
///
/// Attach this either as the root cause (`anyhow::Error::new`, when there
/// is no underlying error worth preserving) or as `.context(...)` layered
/// over one (when there is, e.g. a `shell_words` parse failure — see the
/// invocation-parsing call site). Both work identically for classification:
/// `anyhow::Error::downcast_ref` searches the root error AND every context
/// layer at any depth, so `error_kind` finds a `RequestError` wherever it
/// was attached, however much further context piles on top afterwards.
/// Using it as context also means its own `Display` (`"{message}"`) layers
/// over the wrapped cause's, so `{e:#}` — what callers render for the user
/// — still shows both.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
struct RequestError {
    kind: ErrorKind,
    message: String,
}

impl RequestError {
    fn new(kind: ErrorKind, message: impl Into<String>) -> RequestError {
        RequestError {
            kind,
            message: message.into(),
        }
    }
}

/// Recovers the `ErrorKind` a handler attached via [`RequestError`].
///
/// `downcast_ref` (not a hand-rolled `Error::source()` walk) is what makes
/// this indifferent to whether `RequestError` sits at the root of the
/// `anyhow::Error` or several `.context(...)` layers down — see that
/// struct's docs. Anything with no `RequestError` anywhere in its chain — a
/// tmux failure, a filesystem error with no classification opinion —
/// reports `ErrorKind::Internal`, the honest default for a failure no call
/// site claimed a more specific reason for.
fn error_kind(e: &anyhow::Error) -> ErrorKind {
    e.downcast_ref::<RequestError>()
        .map(|r| r.kind)
        .unwrap_or(ErrorKind::Internal)
}

/// Best-effort credential-hygiene cleanup: remove `path`, treating its
/// absence as success (the shim may already have consumed and unlinked
/// it) and logging anything else as a warning naming both the file and
/// what it was, rather than propagating — every call site here is itself
/// already unwinding a different failure, and this cleanup must not mask
/// that original error with an unrelated filesystem one.
async fn best_effort_remove(path: &Path, what: &str) {
    match tokio::fs::remove_file(path).await {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            warn!(path = %path.display(), error = %e, "could not remove {what}");
        }
    }
}

/// Remove `path`, tolerating its absence (the shim usually already
/// unlinked it — see launch.rs) but treating any OTHER failure as fatal,
/// unlike `best_effort_remove`'s log-and-continue.
///
/// Used only by `DeleteSession`: a leftover launch spec may hold the
/// agent's full command line, credentials included, and delete is the
/// last moment anything will ever come back to clean it up — a caller
/// here cannot shrug off a removal failure the way create's failure-
/// unwind path does (which returns a different, already-fatal error
/// either way).
async fn remove_fail_closed(path: &Path, what: &str) -> Result<(), String> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("removing {what} ({}): {e}", path.display())),
    }
}

/// Cap on a caller-supplied session id echoed into an error message
/// (`NotFound` replies, chiefly). 128 bytes is far beyond any real
/// session id (a UUID is 36 characters) while still bounding a hostile or
/// accidental multi-megabyte `session_id` string from ever reaching
/// `reply_frame`'s oversize check — better to never construct a
/// near-`MAX_FRAME_LEN` error message than to rely on that check's
/// last-resort substitution to catch it after the fact.
const ECHOED_ID_MAX: usize = 128;

/// Truncate `id` to [`ECHOED_ID_MAX`] bytes (at a UTF-8 char boundary, so
/// the cut cannot split a multi-byte sequence) before it is embedded in an
/// error message, appending `...` when truncation actually happened.
fn truncate_for_error(id: &str) -> std::borrow::Cow<'_, str> {
    if id.len() <= ECHOED_ID_MAX {
        return std::borrow::Cow::Borrowed(id);
    }
    let mut end = ECHOED_ID_MAX;
    while !id.is_char_boundary(end) {
        end -= 1;
    }
    std::borrow::Cow::Owned(format!("{}...", &id[..end]))
}

/// Grace period between `SIGTERM` and the SIGSTOP-quiesce step in
/// [`kill_process_tree`]. Long enough for a well-behaved agent (or an
/// MCP/dev-server child) to run its own shutdown hooks; short enough that
/// `stop`/`delete` still feel immediate to a human waiting on them.
const KILL_GRACE: Duration = Duration::from_millis(500);

/// Bounds how many SIGSTOP-and-re-enumerate rounds
/// [`kill_process_tree`]'s quiesce fixpoint runs before giving up on
/// convergence. Stopped processes cannot fork, so each round can only
/// discover pids that forked in the brief window before THIS round's
/// SIGSTOP landed; the set shrinks fast in practice, and this cap is a
/// backstop against a pathological fork storm rather than an expected
/// ceiling — exhausting it without converging is itself a reported sweep
/// failure (see that function's docs), not a silent "close enough".
const MAX_QUIESCE_PASSES: usize = 5;

/// Total time [`kill_process_tree`] polls for its SIGKILLed pids to
/// actually disappear before giving up and reporting survivors as a sweep
/// failure. `SIGKILL` cannot be blocked or ignored, so this is generous
/// purely for scheduler noise (a loaded host taking a moment to actually
/// schedule the kill), not because the kernel might refuse.
const KILL_CONFIRM_TIMEOUT: Duration = Duration::from_secs(2);

/// Poll interval within [`KILL_CONFIRM_TIMEOUT`].
const KILL_CONFIRM_STEP: Duration = Duration::from_millis(50);

/// Whether an I/O error reading some `/proc/<pid>/...` path means the
/// process (or its row) is simply gone, as opposed to a genuine problem
/// this sweep must report. `ENOENT` is the ordinary shape (the path
/// itself vanished), but `ESRCH` comes through this path too: opening a
/// still-listed `/proc/<pid>/stat` whose process dies mid-read fails
/// with ESRCH rather than ENOENT (observed on CI — the confirmation poll
/// raced a SIGKILL'd pid's teardown and reported a false sweep failure),
/// so both mean the same thing here: nothing left to worry about.
fn is_gone_errno(e: &std::io::Error) -> bool {
    e.kind() == std::io::ErrorKind::NotFound || e.raw_os_error() == Some(libc::ESRCH)
}

/// Read one stat field by its position AFTER the `comm` field (index 0 is
/// `state`, the third stat field overall). `rest` is everything following
/// the LAST `)` in a `/proc/<pid>/stat` line — see [`parse_stat`] for why
/// that boundary is found on raw bytes before this ever touches `&str`.
fn stat_field(rest: &str, index: usize) -> Option<&str> {
    rest.split_whitespace().nth(index)
}

/// Parse a `/proc/<pid>/stat` line's state, parent pid, and kernel
/// start-time (state is field 3 overall, the first token after `comm`;
/// start-time is field 22, the 20th token after `comm`) from raw bytes.
///
/// Bytes in, not `&str`, and the `comm` field's own bytes are never
/// decoded at all: `comm` (the process name in parentheses) is whatever
/// bytes the process named itself with via `PR_SET_NAME`/argv[0] and can
/// contain arbitrary, non-UTF-8 data, spaces, or even parentheses — so
/// this locates the LAST `)` as a raw byte search (valid regardless of
/// what came before it) and only decodes the fixed-format, always-ASCII
/// fields after it. A `comm` with a non-UTF-8 byte would otherwise fail
/// the whole read, silently misreporting a live process as "gone" to
/// [`snapshot_proc`]'s caller — exactly the kind of resource-exhaustion-
/// disguised-as-success bug this module works hard elsewhere to avoid.
///
/// State is what lets [`confirm_gone`] recognize a zombie — a process
/// that has already exited but has no ancestor left to reap it — as gone
/// rather than as a stuck SIGKILL: nothing this module does can force a
/// reap, and a zombie cannot run anything regardless, so treating one as
/// still-alive would fail a sweep for a reason no amount of signaling
/// could ever fix. Start-time is what makes a discovered pid safe to act
/// on LATER, after other work (a signal, a sleep, another `/proc` walk)
/// has given the kernel a chance to reuse it: [`signal_validated`]
/// re-reads this same field immediately before signaling and refuses to
/// act unless it still matches, which is the only way a numeric pid
/// recorded minutes, seconds, or even microseconds ago can still be
/// trusted.
fn parse_stat(bytes: &[u8]) -> Result<(u32, u64, char), String> {
    let Some(after_comm) = bytes.iter().rposition(|&b| b == b')') else {
        return Err(format!(
            "stat content has no ')' delimiting comm: {bytes:?}"
        ));
    };
    let rest = std::str::from_utf8(&bytes[after_comm + 1..])
        .map_err(|e| format!("stat fields after comm are not valid UTF-8: {e}"))?;
    let state = stat_field(rest, 0)
        .ok_or("stat content is missing the state field")?
        .chars()
        .next()
        .ok_or("stat content has an empty state field")?;
    let ppid = stat_field(rest, 1)
        .ok_or("stat content is missing the ppid field")?
        .parse::<u32>()
        .map_err(|e| format!("stat ppid field is unparseable: {e}"))?;
    let starttime = stat_field(rest, 19)
        .ok_or("stat content is missing the starttime field")?
        .parse::<u64>()
        .map_err(|e| format!("stat starttime field is unparseable: {e}"))?;
    Ok((ppid, starttime, state))
}

/// This process's own effective uid, for [`is_own_pid_dir`].
fn euid() -> u32 {
    // SAFETY: geteuid takes no arguments and cannot fail.
    unsafe { libc::geteuid() }
}

/// Whether `/proc/<pid>` is owned by this process's own effective uid.
///
/// Exists for hosts whose `/proc` is mounted `hidepid=1` (or stricter): a
/// legitimate, common hardening option under which OTHER users' pid
/// directories stay visible to `readdir` — so this module's `/proc` walk
/// still enumerates them — but their contents (`stat`, `environ`, ...)
/// become `EACCES`. That is routine and expected, not a sweep failure,
/// so this check runs BEFORE any fail-closed stat parsing: a foreign-uid
/// pid is skipped outright, rather than letting an ordinary permission
/// restriction turn into a reported error for every unrelated process on
/// a shared or hidepid-hardened host. A pid that has already exited (or
/// otherwise can't be stat'd at the directory level) is not this check's
/// business to adjudicate — `read_stat`'s own `ENOENT` handling covers
/// that — so failure to read the directory's metadata defaults to "ours",
/// leaving the decision to the caller's normal fail-closed path.
fn is_own_pid_dir(pid: u32) -> bool {
    use std::os::unix::fs::MetadataExt;
    match std::fs::metadata(format!("/proc/{pid}")) {
        Ok(metadata) => metadata.uid() == euid(),
        Err(_) => true,
    }
}

/// Read and parse `/proc/<pid>/stat`.
///
/// `Ok(None)` means the process is simply gone (`ENOENT`) — the ordinary,
/// expected outcome of racing a process's own exit, and never worth
/// reporting. Anything else that goes wrong — a permission error this
/// process should never see for a pid it is supposed to own (callers are
/// expected to have already screened out foreign-uid pids via
/// [`is_own_pid_dir`]), a malformed or unrecognized stat format — comes
/// back as `Err` rather than being folded into "gone": treating a real
/// failure as absence would let this module silently under-collect a
/// live descendant and report a sweep as clean when it was not.
fn read_stat(pid: u32) -> Result<Option<(u32, u64, char)>, String> {
    match std::fs::read(format!("/proc/{pid}/stat")) {
        Ok(bytes) => parse_stat(&bytes).map(Some),
        Err(e) if is_gone_errno(&e) => Ok(None),
        Err(e) => Err(format!("reading /proc/{pid}/stat: {e}")),
    }
}

/// Whether `environ` (raw `/proc/<pid>/environ` content: NUL-delimited
/// `KEY=VALUE` entries) contains an EXACT entry for `session_id`'s
/// [`crate::launch::SESSION_ID_ENV_VAR`] marker.
///
/// Split out from [`environ_has_marker`] purely so this matching logic is
/// unit-testable against constructed byte buffers, without a real process
/// or a real `/proc` behind it.
///
/// Matches a complete NUL-delimited entry, never a substring: `environ`
/// packs `KEY=VALUE\0KEY=VALUE\0...`, and a substring match would count
/// `FARHELM_SESSION_ID=abc-1` as containing session `abc`, or misfire on
/// an unrelated variable that happens to embed the same text.
fn environ_contains_marker(environ: &[u8], session_id: &str) -> bool {
    let marker = format!("{}={session_id}", crate::launch::SESSION_ID_ENV_VAR);
    environ
        .split(|&b| b == 0)
        .any(|entry| entry == marker.as_bytes())
}

/// Whether `pid`'s environment carries this session's marker — the
/// environment [`crate::launch::SESSION_ID_ENV_VAR`] sets at launch and
/// every descendant inherits automatically, transitively, across any
/// number of forks and execs, UNLESS a process along the way deliberately
/// scrubs or replaces its own environment (`env -i`, an `exec` with an
/// explicit empty/rebuilt envp, and the like) — that residual is accepted
/// until M3's cgroups hardening, alongside the reparented-daemon case
/// documented on `kill_process_tree`.
///
/// A process belonging to a different user makes `environ` unreadable
/// (mode 0400, owner-only) — that failure is silently treated as "no
/// marker", which is exactly the "same-user" scoping this scan is
/// supposed to have; no separate uid check is needed because the kernel
/// already enforces it (callers additionally skip foreign-uid pids before
/// ever reaching this function — see [`is_own_pid_dir`] — so this scoping
/// is belt and braces, not the only place it happens). A SAME-uid process
/// escapes this scan too if it has called
/// `prctl(PR_SET_DUMPABLE, 0)` (directly, or via a setuid/setgid exec,
/// which clears dumpability as a kernel security measure): `environ`
/// requires `PTRACE_MODE_READ`-equivalent access even from the owning
/// user once a process is non-dumpable, so it becomes just as unreadable
/// as another user's. That is the same accepted-residual bucket as the
/// environment-scrubbing case above — a legitimate hardening choice by
/// the target process defeats the marker scan, and only cgroups (or
/// running as root) close it. Unlike [`read_stat`], this has no error
/// path at all: every failure mode here (gone, permission, non-dumpable)
/// is routine and expected for a directory-wide scan of processes this
/// sweep does not necessarily own.
fn environ_has_marker(pid: u32, session_id: &str) -> bool {
    let Ok(bytes) = std::fs::read(format!("/proc/{pid}/environ")) else {
        return false;
    };
    environ_contains_marker(&bytes, session_id)
}

/// One `/proc` walk's findings: every readable process's (ppid,
/// start-time), plus which of those carry `session_id`'s environment
/// marker. Both halves are read in the same sequential pass over
/// `/proc`'s directory entries — not, importantly, at one consistent
/// instant in time (see [`snapshot_proc`]'s own docs on that) — which is
/// still enough to keep the PPID closure and the marker scan agreeing on
/// one walk's worth of data rather than two independently-timed ones.
struct ProcSnapshot {
    /// pid → (ppid, starttime). Absent means this process's `/proc` row
    /// could not be read because it exited mid-scan — the ordinary,
    /// tolerated case ([`read_stat`]'s `Ok(None)`). A partial map built
    /// this way only ever UNDER-collects candidates for the PPID closure
    /// below; it can never invent an ancestor relationship that does not
    /// exist. That under-collection is a SEPARATE concern from pid
    /// reuse, which this map does nothing to close on its own: a pid
    /// found here can still have been reused by the time it is acted on
    /// later (after a sleep, a signal round, another walk).
    /// [`signal_validated`] is what actually closes that gap, by
    /// re-checking start-time at the moment of signaling regardless of
    /// how fresh or stale the value recorded here is.
    ///
    /// Accepted residual (until M3's cgroups hardening): this map is
    /// still just ONE sequential pass, not an atomic system-wide
    /// snapshot, so in principle a pid could exit and be recycled to an
    /// unrelated process WHILE this same walk is still in progress — an
    /// edge this one snapshot cannot itself detect or rule out. Two
    /// things bound how much that can matter rather than eliminating it
    /// outright: `signal_validated` re-reads and re-checks identity at
    /// the actual moment of signaling (so a within-this-walk recycling
    /// only risks a wrong PPID edge being followed, never an unvalidated
    /// signal reaching the wrong process), and `kill_process_tree`
    /// re-runs the whole closure on every later round rather than
    /// trusting this one walk's shape indefinitely. The remainder — a
    /// closure edge briefly followed on the strength of a since-recycled
    /// pid, within one walk, before the next walk or signal re-validates
    /// it — is accepted as the honest cost of a single-pass, `/proc`-only
    /// implementation.
    stats: HashMap<u32, (u32, u64)>,
    /// pids whose environment carried this session's marker in this same
    /// walk.
    marked: HashSet<u32>,
}

/// Walk `/proc` once, in full, for `session_id`. Fails closed: a
/// directory-level problem (can't open or iterate `/proc` at all)
/// propagates rather than returning an empty, falsely-clean snapshot —
/// silently reporting "found nothing" when the walk never actually ran is
/// exactly the failure mode this module exists to avoid. A per-pid stat
/// read that fails for a reason OTHER than the process being gone
/// (`read_stat`'s `Err` case) is collected into the returned error list
/// rather than aborting the whole walk, so one unreadable row does not
/// blind this scan to every other process in the tree.
///
/// The walk itself is a plain sequential scan over directory entries, not
/// an atomic system-wide snapshot: a fork happening WHILE this function is
/// mid-scan can land a new pid in a part of `/proc` already passed, or
/// simply not exist yet when `read_dir` enumerated the directory. Either
/// way this one walk can under-report a tree that is still actively
/// forking — a sequential-scan race, not (as an earlier version of this
/// comment mis-described) some impossible ordering between a parent and a
/// child that does not exist yet. [`kill_process_tree`] is what actually
/// compensates for this: it re-walks `/proc` between each signal phase
/// rather than trusting one walk to see a tree that can still be growing.
fn snapshot_proc(session_id: &str) -> Result<(ProcSnapshot, Vec<String>), String> {
    let mut stats = HashMap::new();
    let mut marked = HashSet::new();
    let mut soft_errors = Vec::new();
    let entries = std::fs::read_dir("/proc").map_err(|e| format!("reading /proc: {e}"))?;
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                soft_errors.push(format!("iterating /proc: {e}"));
                continue;
            }
        };
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        // A foreign-uid pid under hidepid-hardened /proc: visible to
        // readdir, but its contents are routinely EACCES — see
        // is_own_pid_dir's docs. Skipped entirely, before either read
        // below could turn that ordinary restriction into a reported
        // failure.
        if !is_own_pid_dir(pid) {
            continue;
        }
        match read_stat(pid) {
            Ok(Some((ppid, starttime, _state))) => {
                stats.insert(pid, (ppid, starttime));
            }
            Ok(None) => {}
            Err(e) => soft_errors.push(e),
        }
        if environ_has_marker(pid, session_id) {
            marked.insert(pid);
        }
    }
    Ok((ProcSnapshot { stats, marked }, soft_errors))
}

/// One enumeration for [`kill_process_tree`]: the transitive PPID closure
/// expanded from EVERY root — the pane's process (`root_pid`, only ever
/// meaningful on the very first call; see below), every process carrying
/// `session_id`'s environment marker, and every `(pid, starttime)`
/// identity in `seeds` (everything a PREVIOUS round of this same kill
/// already found).
///
/// Marker pids are folded into the closure's STARTING set, before the
/// closure expands — not appended as leaves afterward — because a
/// reparented daemon can itself have gone on to spawn further children
/// after reparenting; those children are only reachable if the daemon
/// itself is a root the closure walks from, not a leaf the walk never
/// continues past.
///
/// `root_pid` carries no prior identity to validate on its first use —
/// this is the moment its (pid, starttime) pair gets established, by
/// trusting whatever the snapshot reports for it right now (it was just
/// queried fresh from tmux). Every LATER round passes `root_pid: None`
/// and relies on `seeds` instead, because by then the root's identity, if
/// it was found, is already sitting in `seeds` (folded in by
/// `kill_process_tree` reusing its own previous result) — from that point
/// on it is validated exactly like any other seed: a seed pid whose
/// CURRENT starttime does not match the recorded one is dropped outright
/// (that process is gone, or a different one now wears its number), and
/// re-enters the result only if independently rediscovered via the PPID
/// closure or the marker scan this same round.
///
/// Returns pid → starttime for everything found in THIS one walk, plus
/// any soft per-pid errors `snapshot_proc` collected — the values a
/// caller re-validates via [`signal_validated`] before ever signaling, so
/// nothing here is trusted past the moment it was read.
fn enumerate_tree(
    root_pid: Option<u32>,
    session_id: &str,
    seeds: &HashMap<u32, u64>,
) -> Result<(HashMap<u32, u64>, Vec<String>), String> {
    let (snapshot, soft_errors) = snapshot_proc(session_id)?;
    let mut found: HashMap<u32, u64> = HashMap::new();

    // Marker pids first: roots the closure expands FROM.
    for &pid in &snapshot.marked {
        if let Some(&(_, starttime)) = snapshot.stats.get(&pid) {
            found.insert(pid, starttime);
        }
    }
    // The pane root, establishing its identity fresh on round one only.
    if let Some(pid) = root_pid
        && let Some(&(_, starttime)) = snapshot.stats.get(&pid)
    {
        found.insert(pid, starttime);
    }
    // Every previously-found identity, re-validated against this walk.
    for (&pid, &starttime) in seeds {
        if snapshot.stats.get(&pid).map(|&(_, st)| st) == Some(starttime) {
            found.insert(pid, starttime);
        }
    }

    loop {
        let mut grew = false;
        for (&pid, &(ppid, starttime)) in &snapshot.stats {
            if found.contains_key(&ppid) && found.insert(pid, starttime).is_none() {
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    Ok((found, soft_errors))
}

/// Re-read `pid`'s `/proc` start-time and signal it only if that read
/// still matches `expected_starttime` — the barrier that makes a pid
/// recorded at some earlier moment (before a sleep, a signal round, or a
/// re-enumeration) safe to act on now. If the original process already
/// exited and the kernel handed its number to something unrelated, the
/// start-time will have moved on (or the pid will be gone outright), and
/// this refuses to touch whatever now holds that number.
///
/// Returns `Ok(())` for every outcome that is NOT a genuine failure worth
/// reporting: the pid being gone entirely, its start-time no longer
/// matching (pid reused), and `ESRCH` from the signal call itself (a
/// benign race against the process's own concurrent exit — a DIFFERENT
/// check from the `ENOENT` `read_stat` handles, since `ESRCH` is `kill`'s
/// own "no such process" errno, not a filesystem one). Any other
/// `read_stat` failure, or a signal errno other than `ESRCH` (`EPERM`,
/// chiefly), comes back as `Err`: both mean a process this sweep was
/// supposed to reach could not be confirmed reachable, which the caller
/// must learn about rather than silently treat as handled.
fn signal_validated(pid: u32, expected_starttime: u64, signal: i32) -> Result<(), String> {
    match read_stat(pid) {
        Ok(Some((_, starttime, _state))) if starttime == expected_starttime => {
            // SAFETY: `libc::kill` validates `pid` itself; passing a pid
            // this process does not own simply yields EPERM, handled
            // below like any other non-ESRCH errno. No memory safety
            // concern either way.
            let ret = unsafe { libc::kill(pid as libc::pid_t, signal) };
            if ret != 0 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() != Some(libc::ESRCH) {
                    return Err(format!("signaling pid {pid} with signal {signal}: {err}"));
                }
            }
            Ok(())
        }
        // Gone, or a different process now wears this number: neither is
        // ours to signal, and neither is an error.
        Ok(_) => Ok(()),
        Err(e) => Err(format!("re-validating pid {pid} before signaling: {e}")),
    }
}

/// Validate-and-signal every `(pid, starttime)` pair, continuing through
/// the whole set even when some fail — a single unsignalable process
/// (`EPERM`, say) must not stop the rest of the sweep from at least being
/// attempted — and returning every failure message collected along the
/// way for the caller to aggregate.
fn signal_all(pids: &HashMap<u32, u64>, signal: i32) -> Vec<String> {
    let mut errors = Vec::new();
    for (&pid, &starttime) in pids {
        if let Err(e) = signal_validated(pid, starttime, signal) {
            errors.push(e);
        }
    }
    errors
}

/// Enumerate one round for [`kill_process_tree`], folding `fallback` (the
/// previous round's result) in as this round's seeds — see
/// [`enumerate_tree`]'s docs for what that buys. `root_pid` should be
/// `Some` only on the very first call; every later round passes `None`
/// and lets `fallback` carry the root's identity forward instead, exactly
/// like every other seed.
///
/// A hard enumeration failure (an unreadable `/proc` directory, or the
/// blocking task itself panicking) is recorded into `errors` rather than
/// aborting the sweep, and `fallback` is returned unchanged so the caller
/// still has SOMETHING to signal. Reusing stale starttimes here is safe:
/// whoever actually signals re-validates them fresh via
/// [`signal_validated`] regardless of how old the value passed in is.
async fn enumerate_or_reuse(
    root_pid: Option<u32>,
    session_id: &str,
    fallback: &HashMap<u32, u64>,
    errors: &mut Vec<String>,
) -> HashMap<u32, u64> {
    let session_id = session_id.to_string();
    let seeds = fallback.clone();
    match tokio::task::spawn_blocking(move || enumerate_tree(root_pid, &session_id, &seeds)).await {
        Ok(Ok((found, soft_errors))) => {
            errors.extend(soft_errors);
            found
        }
        Ok(Err(e)) => {
            errors.push(format!("enumerating process tree: {e}"));
            fallback.clone()
        }
        Err(e) => {
            errors.push(format!("process enumeration task panicked: {e}"));
            fallback.clone()
        }
    }
}

/// Poll every identity in `found` until each has been CONFIRMED gone — a
/// starttime mismatch, an outright-gone `/proc` row, or a zombie still
/// awaiting an ancestor's reap (nothing this sweep does can force a reap,
/// and a zombie cannot run anything regardless, so waiting for one to be
/// reaped before declaring success would fail a sweep for a reason no
/// amount of signaling could ever fix) — or `timeout` elapses, whichever
/// comes first.
///
/// This exists because `SIGKILL` succeeding only proves the signal was
/// DELIVERED, not that the kernel has finished tearing the process down
/// by the time [`kill_process_tree`] returns; a caller trusting `Ok(())`
/// to mean "the tree is gone" deserves that to be actually true, not just
/// "the last signal in the sequence did not error".
///
/// The error direction matters as much as the confirmation logic: only
/// the three cases above count as confirmed-absent. A `read_stat` failure
/// that is NOT one of those (a permission problem this process should not
/// see for a pid it is supposed to own, a malformed row) is a genuine
/// confirmation failure and is reported as an error — never silently
/// folded into "gone", which would let a sweep claim success over a pid
/// nobody actually confirmed dead. Likewise a panic in the polling task
/// itself aborts this function with a reported error rather than treating
/// the unexamined remainder as gone by default (the bug a bare
/// `.unwrap_or_default()` on the task join would otherwise hide).
async fn confirm_gone(found: &HashMap<u32, u64>, timeout: Duration) -> Vec<String> {
    let deadline = tokio::time::Instant::now() + timeout;
    let mut remaining = found.clone();
    let mut errors = Vec::new();
    loop {
        let still_alive = remaining.clone();
        let poll = tokio::task::spawn_blocking(move || {
            let mut alive = HashMap::new();
            let mut poll_errors = Vec::new();
            for (pid, starttime) in still_alive {
                match read_stat(pid) {
                    // Identity still matches and the process has not yet
                    // become a zombie: genuinely still alive.
                    Ok(Some((_, st, state))) if st == starttime && state != 'Z' => {
                        alive.insert(pid, starttime);
                    }
                    // Gone outright, a different process now wears this
                    // pid, or a zombie: all three are confirmed absence.
                    Ok(_) => {}
                    // A real read/parse problem: not confirmed either
                    // way, and must not be silently counted as gone.
                    Err(e) => poll_errors.push(format!("confirming pid {pid} is gone: {e}")),
                }
            }
            (alive, poll_errors)
        })
        .await;
        let (alive, poll_errors) = match poll {
            Ok(result) => result,
            Err(e) => {
                // The polling task itself panicked: nothing this round
                // was actually confirmed, which is a hard failure to
                // report, not license to assume success for whatever was
                // still pending.
                errors.push(format!("kill-confirmation polling task panicked: {e}"));
                errors.extend(remaining.keys().map(|&pid| {
                    format!("pid {pid} could not be confirmed gone: polling task panicked")
                }));
                return errors;
            }
        };
        errors.extend(poll_errors);
        remaining = alive;
        if remaining.is_empty() || tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(KILL_CONFIRM_STEP).await;
    }
    errors.extend(
        remaining
            .keys()
            .map(|&pid| format!("pid {pid} still alive {timeout:?} after SIGKILL")),
    );
    errors
}

/// Kill an agent's entire process tree (SPEC.md: stop/delete reap the
/// agent and every descendant — MCP servers, dev servers, anything it
/// started), per the sequence lore/2026-07-27-m2-process-tree-stop.md
/// settled on after simpler cuts proved insufficient:
///
/// 1. Enumerate (PPID closure from `root_pid` if any, unioned with the
///    environment-marker scan for `session_id`) and SIGTERM the result.
/// 2. After a grace period, re-enumerate (seeded with everything round 1
///    found, so a reparented survivor is not lost) and SIGSTOP it —
///    freezing every survivor before the fixpoint below closes the
///    fork-during-teardown race: a `SIGTERM` handler that forks a child
///    and then exits would otherwise let that child slip past a kill that
///    only ever looks at what existed at signal time.
/// 3. Re-enumerate to a bounded fixpoint (stopped processes cannot fork,
///    so this converges): any pid not seen in the previous round is new —
///    a fork that landed in the gap before this round's SIGSTOP — and
///    gets SIGSTOPped too, so nothing sneaks past quiescence. Exhausting
///    [`MAX_QUIESCE_PASSES`] while a pass STILL found something new is
///    itself a reported failure — a sweep that never converged cannot
///    honestly claim to have frozen the whole tree.
/// 4. SIGKILL everyone found, stopped or not (`SIGKILL` terminates a
///    stopped process unconditionally, so no `SIGCONT` step is needed
///    first), then poll until every one of them has actually disappeared
///    (see [`confirm_gone`]).
///
/// `root_pid` is `None` for a dead or absent pane, or a terminal-less
/// (restart-gap) entry: there is no live pid worth trusting in any of
/// those cases (a dead pane's remembered pid may already be recycled),
/// but SPEC.md still assigns reaping any leftover descendants of a PAST
/// run to the session's next stop or delete — and the environment-marker
/// scan is the only mechanism that can still find such a survivor once
/// there is no live pane process to walk ancestry from at all. So this is
/// called on every stop and delete, not only when the pane looks alive;
/// `None` simply means the PPID closure has nothing to seed itself with
/// beyond the marker scan's own findings.
///
/// Every signal is starttime-validated (`signal_validated`) — a pid
/// carried across the grace period, the quiesce fixpoint, or simply
/// reused between rounds is never signaled unless a fresh `/proc` read
/// still agrees it is the same process this sweep found.
///
/// Errors accumulate rather than short-circuit: enumeration failures,
/// non-`ESRCH` signal errors, non-convergence, and post-SIGKILL survivors
/// are all collected while the sweep otherwise runs to completion
/// (signaling everyone it can), and only then does this return the
/// aggregated failure — a caller must not conclude "half the tree died so
/// the rest can wait" from an early return. `Ok(())` is the caller's only
/// license to treat the sweep as complete.
///
/// Residual, accepted until M3's cgroups hardening (see the lore entry):
/// a descendant that `exec`'d with a scrubbed environment after
/// reparenting to init is invisible to both the PPID closure (wrong
/// parent) and the marker scan (marker gone), and survives.
async fn kill_process_tree(root_pid: Option<u32>, session_id: &str) -> anyhow::Result<()> {
    let mut errors: Vec<String> = Vec::new();

    let mut found = enumerate_or_reuse(root_pid, session_id, &HashMap::new(), &mut errors).await;
    errors.extend(signal_all(&found, libc::SIGTERM));

    tokio::time::sleep(KILL_GRACE).await;

    found = enumerate_or_reuse(None, session_id, &found, &mut errors).await;
    errors.extend(signal_all(&found, libc::SIGSTOP));

    let mut converged = false;
    for _ in 0..MAX_QUIESCE_PASSES {
        let next = enumerate_or_reuse(None, session_id, &found, &mut errors).await;
        // Identity, not just pid: a pid present in BOTH `found` and `next`
        // but with a DIFFERENT starttime is not the same process anymore
        // — the old one died and the kernel already recycled its number —
        // so it counts as newly found here and gets SIGSTOPped like any
        // other survivor, rather than being silently treated as already
        // handled because its number happened to repeat.
        let newly_found: HashMap<u32, u64> = next
            .iter()
            .filter(|&(&pid, &starttime)| found.get(&pid) != Some(&starttime))
            .map(|(&pid, &starttime)| (pid, starttime))
            .collect();
        found = next;
        if newly_found.is_empty() {
            converged = true;
            break;
        }
        errors.extend(signal_all(&newly_found, libc::SIGSTOP));
    }
    if !converged {
        errors.push(format!(
            "quiesce did not converge within {MAX_QUIESCE_PASSES} passes; the process tree may \
             not be fully frozen"
        ));
    }

    errors.extend(signal_all(&found, libc::SIGKILL));
    errors.extend(confirm_gone(&found, KILL_CONFIRM_TIMEOUT).await);

    if errors.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("process-tree kill hit {}", summarize_errors(&errors))
    }
}

/// Bounds how many individual error strings [`summarize_errors`] includes
/// verbatim, so a pathological `/proc` (a fork storm producing thousands
/// of unsignalable pids, say) cannot build an unbounded — and, chained
/// through `ControlMsg::Error`, potentially oversized — reply out of this
/// module's own error aggregation. The total count is always reported
/// even when the detail list is truncated, so a caller still learns HOW
/// BAD the sweep was, just not every last message.
const MAX_REPORTED_ERRORS: usize = 20;

/// Render an aggregated error list as `"N error(s): msg; msg; ..."`,
/// truncating the detail strings at [`MAX_REPORTED_ERRORS`] while always
/// stating the true total.
fn summarize_errors(errors: &[String]) -> String {
    let total = errors.len();
    let shown = errors
        .iter()
        .take(MAX_REPORTED_ERRORS)
        .cloned()
        .collect::<Vec<_>>()
        .join("; ");
    if total > MAX_REPORTED_ERRORS {
        format!(
            "{total} error(s), first {MAX_REPORTED_ERRORS} shown: {shown} \
             ({} more omitted)",
            total - MAX_REPORTED_ERRORS
        )
    } else {
        format!("{total} error(s): {shown}")
    }
}

/// The two tmux handles that address one session's terminal.
///
/// Both are needed and neither substitutes for the other: session name is
/// the target for anything window-scoped (`resize-window`, the
/// control-mode attach), pane id (`%N`) for anything pane-scoped
/// (`send-keys`, `capture-pane`, format queries).
struct Terminal {
    tmux_name: String,
    pane: String,
}

/// Upper bound on an alt-screen snapshot, applied at BOTH ends of its
/// life: how much of a live `capture-pane` invocation's output
/// [`TmuxDriver::capture_alt_screen_if_active`]'s bounded reader will
/// buffer before killing the child and discarding, and how much of a
/// STORED snapshot file [`read_bounded_snapshot_file`] will read before
/// giving up and degrading an attach to the plain prefill. Both bounds
/// exist for the same underlying reason and share this one constant so
/// they can never silently drift apart.
///
/// A hostile or merely huge pane (an agent running at an enormous
/// terminal size, deliberately or not) must not turn `stop` — a rare but
/// latency-sensitive operation callers are waiting on — into an unbounded
/// in-memory buffer and an unbounded private-file write; nor should a
/// corrupted or tampered-with snapshot FILE be able to make an ordinary
/// `Attach` read an unbounded amount off disk. 2 MiB is generous for a
/// single screen's worth of styled cells (SPEC.md's own replay floor,
/// [`HISTORY_LIMIT`](crate::tmux::HISTORY_LIMIT), budgets for 12,000
/// LINES of full scrollback; this cap covers a single frame) while still
/// bounding the worst case. An over-cap capture or read is dropped with a
/// warning, exactly like any other best-effort snapshot failure.
const MAX_ALT_SCREEN_SNAPSHOT_BYTES: usize = 2 * 1024 * 1024;

/// Path to a session's alt-screen stop snapshot (whether or not it
/// currently exists).
///
/// A STABLE path keyed by the session id — not a fresh, one-time name
/// like the per-write files a versioned log would use — because the file
/// is deliberately REPLACED by every later stop rather than accumulated,
/// since only the most recent screen before a kill is worth keeping (see
/// [`capture_alt_screen_before_stop`] / [`publish_alt_screen_snapshot`]).
/// Same confidentiality class as a launch spec — terminal content can
/// carry secrets an agent echoed — hence living under its own
/// `ensure_private_dir`-protected subdirectory rather than next to the
/// launch specs themselves.
///
/// Restart interplay (no extra code needed beyond this path being keyed
/// by session id, which is stable across a restart): snapshot files
/// persist across supervisor restarts on the same state dir, so a
/// reloaded session whose tmux pane survived the restart can still hit
/// the `Attach` handler's dead-pane-replay path later using a snapshot
/// from a stop that predates the restart. A terminal-less (restart-gap)
/// session's snapshot, if any, is simply unreachable — `Attach` refuses a
/// terminal-less entry before ever consulting a snapshot — until
/// `DeleteSession` cleans it up.
fn snapshot_path(state_dir: &Path, session_id: &str) -> PathBuf {
    state_dir.join("snapshots").join(session_id)
}

/// Sweep abandoned `overwrite_private_file` staging files (`*.tmp-*`)
/// out of `<state_dir>/snapshots/` at supervisor startup — called once
/// from `Supervisor::serve`, same spirit and placement as the launch-dir
/// sweep just above it (after the exclusivity bind, so this process is
/// provably the state dir's one supervisor before touching anything).
///
/// `overwrite_private_file` already cleans up its own temp file when its
/// write or rename fails (`remove_temp_after_failure`, lib.rs), but that
/// cleanup only runs if THIS process is still alive to run it — a hard
/// crash (OOM kill, `kill -9`, power loss) between staging the temp file
/// and either renaming it into place or reaching the failure-cleanup path
/// skips it entirely, leaving an orphaned `.tmp-*` file behind forever
/// with nothing else that would ever remove it. This sweep is that
/// backstop.
///
/// Deliberately narrower than the launch-dir sweep, which removes EVERY
/// file it finds: `snapshots/` also holds legitimate, PERSISTENT snapshot
/// files meant to survive a restart (see `snapshot_path`'s "restart
/// interplay" docs), so this sweep only ever removes entries whose name
/// matches the temp-file convention (`.<name>.tmp-<uuid>`) — a real
/// snapshot, named after a session id alone, can never match that pattern
/// (a session id contains no `.tmp-` substring by construction: it is
/// either a UUID's hyphenated hex form).
///
/// Best-effort and log-only, like the launch-dir sweep: an absent
/// `snapshots/` directory (no supervisor on this state dir has ever
/// captured a snapshot yet) is the ordinary case and not worth a log
/// line; any other read/remove failure is warned about but never fails
/// startup — a leftover temp file is debris, not a correctness problem
/// for anything this sweep's caller is trying to do.
async fn sweep_snapshot_temp_files(state_dir: &Path) {
    let snapshots_dir = state_dir.join("snapshots");
    let mut entries = match tokio::fs::read_dir(&snapshots_dir).await {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => {
            warn!(error = %e, "could not sweep snapshot temp files; orphaned staging files may remain");
            return;
        }
    };
    loop {
        match entries.next_entry().await {
            Ok(None) => break,
            Ok(Some(entry)) => {
                let is_temp_file = entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.contains(".tmp-"));
                if is_temp_file && let Err(e) = tokio::fs::remove_file(entry.path()).await {
                    warn!(path = %entry.path().display(), error = %e,
                        "could not remove orphaned snapshot temp file");
                }
            }
            Err(e) => {
                warn!(error = %e,
                    "snapshot temp-file sweep aborted early; orphaned staging files may remain");
                break;
            }
        }
    }
}

/// Capture an alt-screen pane's visible content just before
/// [`kill_process_tree`] destroys it — WITHOUT writing anything to disk
/// yet. See [`publish_alt_screen_snapshot`] for why the write is a
/// separate, later step gated on the kill's own outcome.
///
/// Returns `None` whenever there is nothing worth storing: the pane was
/// not actually on the alternate screen at capture time (checked
/// ATOMICALLY with the capture itself, in the same tmux invocation — see
/// [`TmuxDriver::capture_alt_screen_if_active`]'s docs for the race two
/// separate calls would open, and for why that same call ALSO enforces
/// [`MAX_ALT_SCREEN_SNAPSHOT_BYTES`] itself via a bounded reader rather
/// than this function checking a length after the fact), a stale/recycled
/// pane id no longer belongs to this session, or the tmux call itself
/// failed. Every one of those is logged (except the unremarkable "was on
/// the primary screen" case) and swallowed here: the caller must still
/// proceed to `kill_process_tree` either way, and a lost snapshot is a
/// visibility regression, not a reason to fail the stop the caller
/// actually needs.
async fn capture_alt_screen_before_stop(
    sup: &Supervisor,
    session_id: &str,
    terminal: &Terminal,
) -> Option<Vec<u8>> {
    let capture = match sup
        .tmux
        .capture_alt_screen_if_active(
            &terminal.tmux_name,
            &terminal.pane,
            MAX_ALT_SCREEN_SNAPSHOT_BYTES,
        )
        .await
    {
        Ok(capture) => capture,
        Err(e) => {
            warn!(
                session = %session_id, error = %e,
                "capturing alt-screen snapshot before stop failed; reattach after stop will \
                 show a blank screen instead of the app's last frame"
            );
            return None;
        }
    };
    match capture {
        crate::tmux::AltScreenCapture::Captured(bytes) => Some(bytes),
        crate::tmux::AltScreenCapture::NotAlternate => None,
        crate::tmux::AltScreenCapture::SessionMismatch => {
            warn!(
                session = %session_id,
                "alt-screen capture's pane no longer belongs to this session (a stale pane id \
                 after a tmux server restart); skipping the snapshot"
            );
            None
        }
        crate::tmux::AltScreenCapture::TooLarge => {
            warn!(
                session = %session_id, cap = MAX_ALT_SCREEN_SNAPSHOT_BYTES,
                "alt-screen snapshot exceeds the size cap; skipping"
            );
            None
        }
    }
}

/// Write a snapshot [`capture_alt_screen_before_stop`] already captured,
/// guarded against a `DeleteSession` that raced the stop which produced
/// it.
///
/// Called ONLY after `kill_process_tree` has returned `Ok` — see the
/// `StopSession` call site. A stop that fails to kill must never publish:
/// without that ordering, a LATER natural exit's own dead-pane replay
/// could show "last screen before stop" for a stop that never actually
/// completed.
///
/// # Delete-race analysis
///
/// This file carries the same secrets-an-agent-echoed confidentiality
/// class as a launch spec, so writing one for a session that a concurrent
/// `DeleteSession` has ALREADY finished tearing down would orphan it
/// forever — nothing would ever come back to remove it. The fix is to
/// check `sup.sessions` for the session's continued existence BEFORE
/// writing anything, not after: `DeleteSession` removes a session's
/// snapshot file (`remove_fail_closed`) and then, still under the SAME
/// `attachments` lock, removes the session from `sup.sessions` itself
/// (see that handler's teardown block and the `Supervisor` struct's
/// lock-ordering docs). This function acquires that identical lock across
/// its own existence-check-then-write, which makes the two operations
/// mutually exclusive rather than merely racily ordered:
/// - If this function's lock acquisition wins, a concurrent delete cannot
///   even START its teardown until this function releases `attachments`
///   (it needs the same lock) — so the existence check below is
///   guaranteed accurate for the ENTIRE write that follows it, and the
///   delete that runs afterward will find (and fail-closed-remove) the
///   file this function just wrote, like any other artifact.
/// - If a concurrent delete's lock acquisition wins instead, its entire
///   teardown — snapshot removal AND the session's removal from
///   `sup.sessions` — completes before this function ever gets the lock.
///   The existence check then correctly finds the session gone and skips
///   the write entirely: there is nothing to clean up, because nothing
///   was ever written.
///
/// This is strictly simpler than a write-then-recheck-and-clean-up-if-
/// orphaned design (an earlier version of this function did exactly
/// that): checking first means an already-deleted session is a fast,
/// side-effect-free no-op, rather than a write immediately followed by
/// its own removal. (Full per-session lifecycle serialization — a lock
/// scoped to one session's whole stop/delete/attach lifecycle — was
/// considered and deliberately not built for this: reusing the existing
/// coarse `attachments` lock for this one short critical section is
/// enough to close the race without a new locking primitive.)
async fn publish_alt_screen_snapshot(sup: &Supervisor, session_id: &str, bytes: &[u8]) {
    let dir = sup.state_dir.join("snapshots");
    if let Err(e) = crate::ensure_private_dir(&dir).await {
        warn!(session = %session_id, error = %e, "creating the snapshots directory failed");
        return;
    }
    let path = snapshot_path(&sup.state_dir, session_id);

    let attachments = sup.attachments.lock().await;
    let still_exists = sup.sessions.lock().await.contains_key(session_id);
    if !still_exists {
        // A concurrent delete already finished (see the analysis above):
        // nothing to write, and — because nothing was ever written —
        // nothing to clean up either.
        drop(attachments);
        return;
    }
    if let Err(e) = crate::overwrite_private_file(&path, bytes).await {
        warn!(session = %session_id, error = %e, "writing the alt-screen snapshot failed");
    }
    drop(attachments);
}

/// Read a stored alt-screen snapshot file, bounded the same way capture
/// time is (see [`MAX_ALT_SCREEN_SNAPSHOT_BYTES`]'s docs): reads at most
/// `cap + 1` bytes via [`AsyncReadExt::take`], so a corrupt, tampered-
/// with, or simply mis-sized file on disk can never be read into memory
/// unbounded — the same discipline
/// [`TmuxDriver::capture_alt_screen_if_active`]'s bounded reader already
/// applies on the write side. `Ok(None)` means the file does not exist,
/// the ordinary case for any session that either was never stopped on
/// the alternate screen or has since had its snapshot cleaned up by a
/// delete. An over-cap file (reading successfully hits `cap + 1`, the
/// smallest length a bounded reader can produce that proves there was
/// more) is reported as an `Err`, identical in shape to any other read
/// failure — see [`within_snapshot_cap`](crate::tmux::within_snapshot_cap)
/// for the shared at-cap-vs-one-over boundary this and the capture-side
/// reader both use.
async fn read_bounded_snapshot_file(path: &Path, cap: usize) -> std::io::Result<Option<Vec<u8>>> {
    let file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut buf = Vec::new();
    file.take(cap as u64 + 1).read_to_end(&mut buf).await?;
    if !crate::tmux::within_snapshot_cap(buf.len(), cap) {
        return Err(std::io::Error::other(format!(
            "snapshot file at {} exceeds the {cap}-byte cap",
            path.display()
        )));
    }
    Ok(Some(buf))
}

/// Append the alt-screen stop snapshot to a fresh attach's prefill, for a
/// pane the `Attach` handler has already determined is DEAD.
///
/// The gate is snapshot EXISTENCE, not the pane's current screen —
/// deliberately corrected from an earlier version of this function that
/// also required `!alternate_on`, reasoning that a dead pane still on the
/// alternate screen would already show its last frame via the ordinary
/// prefill above. That reasoning was empirically wrong: tmux replaces a
/// DEAD pane's own content — alternate screen or not, history or not —
/// with its own "Pane is dead" placeholder the moment the backing process
/// exits, so a dead-and-still-alternate pane's prefill shows nothing
/// useful either. That state is very much reachable in exactly the case
/// this feature exists for: a pane running an app that ignores SIGTERM,
/// which `StopSession`'s `kill_process_tree` escalates all the way to
/// SIGKILL — captured while alive and on the alternate screen, then
/// killed without ever getting a chance to restore the primary screen.
/// Gating on `!alternate_on` would blank exactly that case.
///
/// `pane_alternate` (the dead pane's CURRENT `#{alternate_on}`, from the
/// same mode query the ordinary prefill already used) decides screen
/// placement, not whether to append at all: when true, this first sends
/// `\x1b[?1049l` (leave the alternate screen) BEFORE the divider. Without
/// that, the snapshot would land inside the scrollback-less alternate
/// buffer the mode-replay sequence above (`PaneModes::pre_content_sequences`)
/// just re-entered, burying its own top rows with nowhere for the
/// overflow to go; leaving the alternate screen first moves the divider
/// and snapshot onto the primary screen, whose real scrollback can absorb
/// whatever does not fit visible — the same tradeoff already documented
/// at the call site.
///
/// Consults the FILE first, then [`Supervisor::pending_snapshots`] —
/// see that field's own docs for the "attach lands between kill and
/// publish" window this fallback closes, and for the honesty argument
/// (why serving an in-flight capture is never showing stale or
/// misleading content); that fallback applies identically regardless of
/// `pane_alternate`, since the escape-sequence decision above only cares
/// about the pane's CURRENT screen, not which source the bytes came from.
/// Both sources missing is the ordinary case for most sessions (never
/// stopped at all, or already cleaned up by a delete) and is not logged;
/// any actual read failure — on the file, not its mere absence —
/// degrades to the plain prefill with a warning rather than failing the
/// whole attach over a best-effort visibility extra.
///
/// Streams the divider and the snapshot as their own separate
/// [`REPLAY_CHUNK`]-sized frames — mirroring how the ordinary prefill
/// above is sent in `pre_content_sequences`/chunked-content/
/// `post_content_sequences` pieces — rather than concatenating everything
/// into one buffer first: avoids an extra full-snapshot copy on top of
/// whatever copy reading the source (file or pending map) already made,
/// and keeps every frame this handler ever sends the same size-bounded
/// shape.
async fn send_alt_screen_snapshot(
    sup: &Supervisor,
    session_id: &str,
    channel: u32,
    tx: &mpsc::UnboundedSender<Frame>,
    pane_alternate: bool,
) {
    let file_result = read_bounded_snapshot_file(
        &snapshot_path(&sup.state_dir, session_id),
        MAX_ALT_SCREEN_SNAPSHOT_BYTES,
    )
    .await;
    let bytes = match file_result {
        Ok(Some(bytes)) => bytes,
        Ok(None) => match sup.pending_snapshots.lock().await.get(session_id) {
            Some(bytes) => bytes.clone(),
            None => return,
        },
        Err(e) => {
            warn!(
                session = %session_id, error = %e,
                "reading the alt-screen snapshot failed; degrading to the plain prefill"
            );
            return;
        }
    };
    if pane_alternate {
        let _ = tx.send(Frame::data(channel, b"\x1b[?1049l".to_vec()));
    }
    let _ = tx.send(Frame::data(
        channel,
        b"\r\n\x1b[2m-- last screen before stop --\x1b[0m\r\n".to_vec(),
    ));
    for chunk in bytes.chunks(REPLAY_CHUNK) {
        let _ = tx.send(Frame::data(channel, chunk.to_vec()));
    }
    let _ = tx.send(Frame::data(channel, b"\r\n".to_vec()));
}

/// A session as the supervisor tracks it: the wire-visible metadata plus
/// its terminal handle, if it still has one.
///
/// `terminal` is `None` for exactly one reason: this entry was
/// reconstructed from a SQLite row whose tmux session `has_session` no
/// longer finds (`Supervisor::reload_sessions`) — the restart-gap case
/// PLAN_M2.md specifies. A session created
/// in this same process always gets `Some` immediately; nothing in a
/// live process ever demotes an entry from `Some` to `None` after the
/// fact — that would require noticing an already-open terminal died,
/// which is M3's interrupted-classification job, not this PR's. Entries
/// are otherwise immutable once created — shared as `Arc` and never
/// mutated in place — so nothing has to hold the session map while
/// talking to tmux.
struct SessionEntry {
    info: SessionInfo,
    terminal: Option<Terminal>,
}

/// The one live attachment a session may have (SPEC.md: at most one,
/// last attach wins). `notify` reaches the owning connection's writer so
/// a takeover can tell the old client it was detached.
struct ActiveAttach {
    channel: u32,
    notify: mpsc::UnboundedSender<Frame>,
    /// The forwarder task. A `JoinHandle` rather than an `AbortHandle`
    /// because a takeover must be able to *wait* for the old forwarder to
    /// finish: `abort()` only schedules cancellation, so the old
    /// control-mode client's process would otherwise still be alive when
    /// the new one starts.
    forwarder: tokio::task::JoinHandle<()>,
    /// A second control-mode client, dedicated to this attachment's input,
    /// opened alongside the replay stream in the attach handler. `send`
    /// on it only returns once tmux has actually executed the command —
    /// see [`InputClient`] — which is what lets a failed send here mean
    /// "this attachment's input is broken" rather than "the bytes went
    /// somewhere unconfirmed". Dropped (and so killed, via
    /// `kill_on_drop`) whenever this `ActiveAttach` is removed from the
    /// map, on every teardown path: takeover, detach, connection loss, and
    /// the input-failure branch below.
    input: InputClient,
}

/// One host's session authority, shared by every connection.
///
/// Sessions are keyed by session id, attachments by session id too —
/// separate maps because their lifetimes differ: a session outlives any
/// number of attach/detach cycles, and SPEC.md caps the attachments at one
/// per session while placing no such cap on sessions.
///
/// Lock discipline: the two mutexes are never held at once, with two
/// deliberate exceptions (`DeleteSession` and `publish_alt_screen_
/// snapshot`, below), and no tmux call happens while `sessions` is held
/// on its own. `attachments` is deliberately the exception for holding it
/// across tmux calls — the whole attach takeover runs under it, because
/// that is the only way "at most one attachment, last attach wins"
/// survives two concurrent attaches (see the `Attach` handler), and the
/// input and `Resize` arms hold it across their tmux calls for the same
/// reason: an ownership check that releases the lock before acting goes
/// stale the moment a takeover interleaves. `DeleteSession` is the second
/// holder of this same exception, for the same underlying reason: it
/// holds `attachments` across its own forwarder-abort and tmux calls so a
/// concurrent `Attach` cannot install a fresh attachment on a session
/// that is disappearing out from under it — but deliberately NOT across
/// its process-tree sweep, which can take seconds and would otherwise
/// stall every other session's attach/input behind one slow delete (see
/// that handler's own comment). `DeleteSession` is also ONE of two paths
/// that ever hold both mutexes at once, briefly: it removes the
/// in-memory map entry while `attachments` is still held for the notify
/// decision, and `publish_alt_screen_snapshot` (service.rs) does the same
/// — checks `sessions` for continued existence while still holding
/// `attachments` — to make its own existence-check-then-write atomic with
/// respect to a racing delete (see that function's own docs for the full
/// argument). Both establish, and both rely on, the SAME lock-ordering
/// rule, the only one that needs to exist as long as nothing else ever
/// needs both: `attachments` first, `sessions` second, never the reverse
/// (anything acquiring both in the opposite order would be a deadlock
/// waiting for a second caller to exist).
///
/// Known coarseness, acceptable in M1 and revisit at M2: the map-wide
/// mutex serializes input for EVERY session behind any in-flight attach
/// (which holds the lock across takeover, resize, and the bounded
/// control-mode cutover). With many sessions per supervisor, this wants
/// per-session attachment slots so sessions stall independently.
pub struct Supervisor {
    state_dir: PathBuf,
    tmux: TmuxDriver,
    /// Session metadata's durable half — see `crate::store` and the
    /// module docs above for the split of truth this implements.
    store: SessionStore,
    sessions: Mutex<HashMap<String, Arc<SessionEntry>>>,
    attachments: Mutex<HashMap<String, ActiveAttach>>,
    /// Alt-screen snapshots captured by an IN-FLIGHT `StopSession` call,
    /// keyed by session id, visible to `Attach` before the corresponding
    /// `publish_alt_screen_snapshot` has written anything to disk.
    ///
    /// Exists to close a real gap: `StopSession` captures the snapshot
    /// BEFORE calling `kill_process_tree` (which can itself take up to a
    /// couple of seconds against an uncooperative process tree — see that
    /// function's own docs on the SIGTERM-grace/SIGSTOP-quiesce/SIGKILL
    /// sequence), and only publishes it to disk AFTER the kill returns
    /// `Ok`. tmux itself, independently, can mark the pane dead the
    /// MOMENT the process actually exits — which for a tree-kill can be
    /// well before `kill_process_tree` itself returns, since it keeps
    /// polling for confirmation and reaping stragglers afterward. An
    /// `Attach` landing in that window would otherwise see a dead pane
    /// with no snapshot file yet on disk and stay blank forever — the
    /// exact bug this whole feature exists to fix, reintroduced at a
    /// smaller time scale. `Attach`'s dead-primary replay path
    /// (`send_alt_screen_snapshot`) consults this map only as a fallback,
    /// after the file itself is confirmed absent.
    ///
    /// Honesty argument for why this is safe to SERVE, not just safe to
    /// exist: during the window this map is consulted, the bytes it holds
    /// are exactly what the pane's alternate screen held at the moment
    /// `StopSession` captured them — a real, complete frame, not a
    /// partial or synthesized one. Serving it early is strictly more
    /// useful than the alternative (blocking or blanking), and no less
    /// accurate than serving the same bytes moments later once they have
    /// been written to disk.
    ///
    /// An entry is inserted right after a successful capture (before
    /// `kill_process_tree` runs) and removed only after
    /// `publish_alt_screen_snapshot` has run (on the success path) or
    /// immediately (on a failed kill, where nothing is ever published at
    /// all) — see the `StopSession` handler. Never touched by anything
    /// else: unlike `sessions`/`attachments`, no cross-handler lock-
    /// ordering rule applies to this map, since nothing ever holds it
    /// alongside either of the other two.
    pending_snapshots: Mutex<HashMap<String, Vec<u8>>>,
    /// This binary's own path: the launch shim is a subcommand of it.
    farhelm_exe: PathBuf,
    /// Admission control for the slow handlers spawned by
    /// `handle_control` (`ListSessions`/`StopSession`/`DeleteSession` —
    /// see `HANDLER_ADMISSION_PERMITS`'s own docs). Deliberately
    /// SUPERVISOR-wide, not per-connection: the resource being bounded is
    /// tmux subprocesses and `/proc` sweeps, which are global to this
    /// process regardless of how many helm connections are open at once.
    /// A per-connection semaphore would let N connections each run 8
    /// concurrent kill sweeps — `8*N`, not 8 — defeating the bound the
    /// moment more than one connection exists. `JoinSet` task TRACKING,
    /// by contrast, stays per-connection (see `handle_connection`): each
    /// connection only ever needs to know about — and clean up after —
    /// its OWN spawned tasks at its OWN shutdown, so sharing that part
    /// globally would buy nothing and would entangle unrelated
    /// connections' teardowns.
    admission: Arc<tokio::sync::Semaphore>,
}

impl Supervisor {
    /// Production constructor: the launch shim is this very process's
    /// binary. Test harnesses must NOT use this — their `current_exe` is
    /// the libtest runner, not farhelm — and use `new_with_exe` instead.
    pub async fn new(state_dir: &Path) -> anyhow::Result<Arc<Supervisor>> {
        let exe = std::env::current_exe().context("resolving own binary path")?;
        Self::new_with_exe(state_dir, exe).await
    }

    /// Constructor with an explicit farhelm binary path for the launch
    /// shim. Used by tests (pointing at the built `farhelm` artifact)
    /// and by any embedder whose own executable is not farhelm.
    pub async fn new_with_exe(
        state_dir: &Path,
        farhelm_exe: PathBuf,
    ) -> anyhow::Result<Arc<Supervisor>> {
        // 0700 on both: the socket and the launch specs (which hold full
        // agent command lines) live here. See ensure_private_dir. The
        // database opened just below relies on this same boundary for its
        // own confidentiality (see `SessionStore::open`'s docs), so it
        // must not be opened before this call.
        crate::ensure_private_dir(state_dir).await?;
        crate::ensure_private_dir(&state_dir.join("launch")).await?;

        // Store before tmux, deliberately: opening the DB (or applying its
        // schema) is the one step in this constructor that can fail for
        // reasons unrelated to tmux at all (a corrupt file, an
        // unrecognized schema version), and rows can be loaded without a
        // tmux server yet existing — liveness is only decided later, once
        // one does. Doing this first means a DB failure aborts
        // construction having started nothing persistent; the old
        // ordering (`ensure_server` first) left a freshly started tmux
        // server — `exit-empty off`, so it does not even exit on its own —
        // behind a constructor that then failed on the database.
        let store = SessionStore::open(&state_dir.join("supervisor.db")).await?;
        let tmux = TmuxDriver::new(state_dir);
        tmux.ensure_server().await?;

        let sessions = Self::reload_sessions(&store, &tmux).await?;

        Ok(Arc::new(Supervisor {
            state_dir: state_dir.to_path_buf(),
            tmux,
            store,
            sessions: Mutex::new(sessions),
            attachments: Mutex::new(HashMap::new()),
            pending_snapshots: Mutex::new(HashMap::new()),
            farhelm_exe,
            admission: Arc::new(tokio::sync::Semaphore::new(HANDLER_ADMISSION_PERMITS)),
        }))
    }

    /// Rebuild the in-memory session map from SQLite plus a tmux liveness
    /// probe per row: alive rows become a normal live `SessionEntry`, rows
    /// whose tmux session tmux no longer recognizes become the
    /// restart-gap's terminal-less entry.
    ///
    /// Called twice, for two different reasons. `new_with_exe` calls it
    /// once so an embedder or test that only ever constructs a
    /// `Supervisor` — never calling `serve()` — still gets a populated
    /// map. `serve()` calls it AGAIN, immediately after acquiring the
    /// exclusivity lock and before accepting any connection, because the
    /// first call can be stale: two supervisor processes can overlap
    /// during a handoff (the old one still running, the new one
    /// constructing), and the old process can create a session — an
    /// insert this process's earlier load already missed — and only then
    /// exit, releasing the lock. Without a second load taken under the
    /// lock, this process would serve a map missing that session for its
    /// entire lifetime, since nothing else ever refreshes it wholesale.
    /// Replacing `self.sessions` wholesale is only safe where no
    /// attachment can yet exist against the entries being replaced —
    /// true at both call sites (construction, and pre-accept in `serve`)
    /// but not a general-purpose operation this type exposes elsewhere.
    async fn reload_sessions(
        store: &SessionStore,
        tmux: &TmuxDriver,
    ) -> anyhow::Result<HashMap<String, Arc<SessionEntry>>> {
        let mut sessions = HashMap::new();
        for row in store.load_all().await? {
            let terminal = if tmux.has_session(&row.tmux_name).await? {
                Some(Terminal {
                    tmux_name: row.tmux_name,
                    pane: row.pane,
                })
            } else {
                info!(
                    session = %row.id,
                    "session's tmux session no longer exists; listing without a terminal"
                );
                None
            };
            sessions.insert(
                row.id.clone(),
                Arc::new(SessionEntry {
                    info: SessionInfo {
                        id: row.id,
                        title: row.title,
                        cwd: row.cwd,
                        invocation: row.invocation,
                        // Placeholder only: `ListSessions` recomputes
                        // `status` fresh from tmux on every reply (see
                        // `session_status`), so nothing ever reads this
                        // particular value — `Unknown` is simply the
                        // honest "not yet computed" default.
                        status: SessionStatus::default(),
                    },
                    terminal,
                }),
            );
        }
        Ok(sessions)
    }

    /// The supervisor's unix socket path within a state dir. Shared with
    /// `farhelm internal stdio`, which is just a dumb pipe to this.
    pub fn socket_path(state_dir: &Path) -> PathBuf {
        state_dir.join("supervisor.sock")
    }

    /// Accept connections forever on this supervisor's own state dir.
    /// The directory is not a parameter: a caller passing a different one
    /// would bind the socket in one place while launch specs land in
    /// another.
    pub async fn serve(self: &Arc<Self>) -> anyhow::Result<()> {
        let path = Self::socket_path(&self.state_dir);
        // Exclusivity is an OS lock, not an inspect-the-socket dance: a
        // probe-then-remove-then-bind sequence is a TOCTOU where two
        // racing supervisors can each pass the probe, and the slower one
        // then unlinks the faster one's freshly bound socket — leaving
        // two processes driving the same tmux server with disjoint
        // session maps. The flock is atomic, and holding it for the
        // process lifetime is what makes the socket removal and the
        // launch-dir sweep below single-owner. The file stays locked
        // until this function returns, which for a healthy supervisor is
        // never.
        let lock_path = self.state_dir.join("supervisor.lock");
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock_path)
            .context("opening supervisor lock file")?;
        if let Err(e) = lock.try_lock() {
            anyhow::bail!(
                "a supervisor is already running against {} \n\
                 (SPEC.md allows at most one supervisor per user per host; lock: {e})",
                self.state_dir.display()
            );
        }
        // Reload the session map now that exclusivity is actually held.
        // The load `new_with_exe` already did can be stale: this
        // process's construction can overlap a still-running predecessor
        // during a handoff, which can insert a session (and exit,
        // releasing the lock) after that first load already ran. Nothing
        // else in this process ever refreshes the map wholesale, so
        // without this second pass such a session would be permanently
        // missing from `sessions` for this process's entire lifetime.
        // Safe to replace outright here: the lock was just acquired and
        // no connection has been accepted yet, so no attachment can exist
        // against any entry this replaces.
        *self.sessions.lock().await = Self::reload_sessions(&self.store, &self.tmux).await?;
        // Holding the lock proves any existing socket file is a leftover
        // from a dead supervisor (the lock dies with its process), so
        // removing it is safe.
        if path.exists() {
            let _ = tokio::fs::remove_file(&path).await;
        }
        let listener = UnixListener::bind(&path).context("binding supervisor socket")?;
        // Belt to the state dir's braces: connecting to this socket means
        // running commands as this user, so do not inherit the umask.
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).await?;
        }
        // Sweep launch specs orphaned by a previous run — a login shell
        // that died before reaching the shim leaves one behind, and it
        // holds the agent's command line; nothing later would remove it.
        // Deliberately AFTER the bind above: the bind is what proves this
        // process is the state dir's one supervisor. Sweeping in the
        // constructor let a second `supervisor run` destroy the live
        // supervisor's in-flight specs and only then bail on the
        // exclusivity check.
        // Best-effort, but never silent: this sweep is credential
        // hygiene, so a failure that leaves specs behind must at least
        // say so in the log.
        let launch_dir = self.state_dir.join("launch");
        match tokio::fs::read_dir(&launch_dir).await {
            Err(e) => warn!(error = %e, "could not sweep launch dir; orphaned specs may remain"),
            Ok(mut entries) => loop {
                match entries.next_entry().await {
                    Ok(None) => break,
                    Ok(Some(entry)) => {
                        if let Err(e) = tokio::fs::remove_file(entry.path()).await {
                            warn!(spec = %entry.path().display(), error = %e,
                                "could not remove orphaned launch spec");
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "launch-dir sweep aborted early; orphaned specs may remain");
                        break;
                    }
                }
            },
        }
        sweep_snapshot_temp_files(&self.state_dir).await;
        info!(socket = %path.display(), "supervisor listening");
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let sup = Arc::clone(self);
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(sup, stream).await {
                            warn!(error = %e, "connection ended with error");
                        }
                    });
                }
                // A transient accept failure (EMFILE under fd pressure,
                // say) must not kill the process whose entire purpose is
                // outliving everything else.
                Err(e) => warn!(error = %e, "accept failed; continuing"),
            }
        }
    }

    /// The one true creation path: validate preconditions, hand the agent
    /// argv to the launch shim, and start a tmux session running it.
    ///
    /// Failure semantics follow SPEC.md's split. Everything checkable up
    /// front — the directory exists, the invocation parses into an argv —
    /// fails here, leaving no session behind at all. Whether the agent
    /// itself then execs successfully is *not* decided here. The shim
    /// writes evidence of `exec` failure for a later milestone's status
    /// classifier, but M1 does not consume that evidence; the terminal
    /// remains available either way so the launch diagnostic is visible.
    ///
    /// `title` defaults to the working directory's basename, which is what
    /// SPEC.md's "auto-generated when omitted" means in M1.
    async fn create_session(
        &self,
        cwd: &str,
        invocation: &str,
        title: Option<String>,
        cols: u16,
        rows: u16,
    ) -> anyhow::Result<SessionInfo> {
        let cwd_path = PathBuf::from(cwd);
        // Preserve the distinction between a bad caller precondition and
        // a host I/O failure. Calling both "does not exist" sends users
        // looking for a typo when the real problem is permission, a
        // symlink loop, or a failing filesystem.
        match tokio::fs::metadata(&cwd_path).await {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => {
                return Err(RequestError::new(
                    ErrorKind::InvalidRequest,
                    format!("working directory is not a directory: {cwd}"),
                )
                .into());
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(RequestError::new(
                    ErrorKind::InvalidRequest,
                    format!("working directory does not exist: {cwd}"),
                )
                .into());
            }
            // `NotADirectory` is a path like `/tmp/some-file/child`, where a
            // non-final component is a regular file — a distinct precondition
            // from the top-level "cwd itself is a file" case above, but the
            // same caller mistake. `InvalidInput` is the OS rejecting the
            // path text itself (a NUL byte, say) before it ever reaches the
            // filesystem. Both are things the caller could have avoided by
            // sending a different `cwd`, unlike the fallback below.
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotADirectory | std::io::ErrorKind::InvalidInput
                ) =>
            {
                return Err(RequestError::new(
                    ErrorKind::InvalidRequest,
                    format!("working directory is not usable: {cwd} ({error})"),
                )
                .into());
            }
            Err(error) => {
                // Not classified: this is an I/O failure the caller could
                // not have avoided by sending a different request (a
                // permission problem, a symlink loop, a failing
                // filesystem), so it defaults to `ErrorKind::Internal`.
                return Err(error)
                    .with_context(|| format!("reading working directory metadata for {cwd}"));
            }
        }
        // The invocation itself stays out of the error: it may carry
        // credentials (`--api-key ...`), and this message travels into
        // the HTTP error body and the helm's stderr/journal. shell-words'
        // own error names the syntax problem. Attached as `.context(...)`
        // (not the root cause) specifically so that diagnostic keeps
        // reaching the user through the `{e:#}` chain — `RequestError` is
        // still findable via `downcast_ref` at this depth (see its docs).
        let argv = shell_words::split(invocation).context(RequestError::new(
            ErrorKind::InvalidRequest,
            "parsing agent invocation",
        ))?;
        if argv.is_empty() {
            return Err(
                RequestError::new(ErrorKind::InvalidRequest, "agent invocation is empty").into(),
            );
        }

        let id = uuid::Uuid::new_v4().to_string();
        // The FULL uuid, not a truncated prefix: an 8-hex-char prefix
        // collides often enough in practice for two sessions — one live,
        // one a dead row surviving in SQLite across a restart — to
        // plausibly share a tmux name, which would cross-wire attach
        // between an unrelated pair of sessions after a reload. The
        // schema's `UNIQUE` constraint on `tmux_name` (see `store.rs`)
        // backstops this at the DB layer; a full UUID is what makes that
        // constraint never fire in the first place. Dashes are legal in
        // tmux session names (verified empirically against a scratch
        // server).
        let tmux_name = format!("fh-{id}");
        let title = title.unwrap_or_else(|| {
            // `cwd` arrived as a `String` over the protocol — farhelm-proto's
            // UTF-8-only wire contract — so every component of `cwd_path`,
            // including its basename, is UTF-8 by construction and
            // `to_str()` on it cannot fail today. The `expect` documents and
            // *enforces* that invariant rather than quietly relying on it:
            // if `cwd` ever stopped being a validated `String` (e.g. a
            // future caller threading an `OsString` through), this panics
            // at the point of violation instead of falling back to
            // "session" and silently mislabeling the session. The
            // `unwrap_or` fallback below is unrelated to UTF-8 — it only
            // covers a `cwd` with no basename at all (e.g. "/").
            cwd_path
                .file_name()
                .map(|n| {
                    n.to_str()
                        .expect("cwd arrived as UTF-8 via the protocol; its components are UTF-8")
                })
                .unwrap_or("session")
                .to_owned()
        });

        let spec_path = self.state_dir.join("launch").join(format!("{id}.json"));
        let status_file_path = self.state_dir.join("launch").join(format!("{id}.status"));
        let spec = LaunchSpec {
            argv,
            status_file: status_file_path.clone(),
            // The kill machinery's environment-marker sweep (see
            // `kill_process_tree`) is keyed on this exact value reaching
            // the agent's process and everything it forks.
            session_id: id.clone(),
        };
        // 0600 from the first byte: the spec holds the full agent command
        // line, which users do put credentials into (`--api-key ...`).
        // Mode is set at open, not chmod-after-write — a write-then-chmod
        // leaves a window where the default umask exposes the contents.
        // A failed write cleans up too: a partial spec (disk full after
        // create) would otherwise strand a credential prefix on disk
        // until the next supervisor restart's sweep.
        crate::write_private_file(&spec_path, &serde_json::to_vec(&spec)?)
            .await
            .context("writing launch spec")?;

        let shell = resolve_shell().await;
        let cmd = window_command(&shell, &self.farhelm_exe, &spec_path);
        let pane = match self
            .tmux
            .create_session(&tmux_name, cwd, cols, rows, &cmd)
            .await
        {
            Ok(pane) => pane,
            Err(e) => {
                // The shim unlinks the spec once it has read it, so a
                // launch that never happens would strand a file holding
                // the agent's full command line — credentials included —
                // with nothing left to clean it up.
                return match tokio::fs::remove_file(&spec_path).await {
                    Ok(()) => Err(e),
                    Err(cleanup) => Err(e.context(format!(
                        "could not remove launch spec {} after tmux creation failed: {cleanup}",
                        spec_path.display()
                    ))),
                };
            }
        };

        let info = SessionInfo {
            id: id.clone(),
            title,
            cwd: cwd.to_string(),
            invocation: invocation.to_string(),
            // Create-time placeholder, deliberately NOT `Alive`:
            // `SessionCreated`'s own docs say creation establishes that
            // the session and terminal exist, not that the agent's later
            // `exec` inside it succeeded — a fast-exiting command (a
            // typo'd invocation, `true`, ...) can already be dead by the
            // time this reply reaches the caller. `Unknown` is the
            // honest "not yet computed" answer, exactly like
            // `reload_sessions`'s own placeholder; `ListSessions` computes
            // the real answer from tmux (`session_status`), and this
            // value is never persisted (see `StoredSession`'s docs)
            // either way.
            status: SessionStatus::Unknown,
        };

        // DB insert AFTER the tmux session already exists: a session that
        // exists in tmux but was never recorded would silently vanish from
        // the list on the next restart, so a failure here must fail the
        // whole create — the tmux session just created is torn back down
        // (best effort) rather than left running and unlisted with no way
        // for the caller to learn its id.
        if let Err(e) = self
            .store
            .insert_session(StoredSession {
                id: id.clone(),
                title: info.title.clone(),
                cwd: info.cwd.clone(),
                invocation: info.invocation.clone(),
                tmux_name: tmux_name.clone(),
                pane: pane.clone(),
            })
            .await
        {
            // The DB error is the root cause throughout — it is what
            // actually failed the create — but a kill failure on top of it
            // is not safe to only log: it means an untracked tmux session
            // may now be running with nobody able to learn its id from the
            // caller's point of view, which the returned error must say so
            // the caller (and whoever reads the resulting log/HTTP body)
            // has a chance of noticing and cleaning it up by hand.
            let mut result = e.context("recording new session in the database");
            if let Err(kill_err) = self.tmux.kill_session(&tmux_name).await {
                warn!(
                    session = %id, error = %kill_err,
                    "could not kill tmux session after its DB insert failed; \
                     it may now be running unlisted"
                );
                result = result.context(format!(
                    "additionally, could not kill tmux session {tmux_name} for session {id} \
                     after the DB insert failed ({kill_err:#}); the agent may still be running \
                     unlisted"
                ));
            }
            // The shim may already have consumed and unlinked the spec by
            // now (it does so as soon as it has read it) — that is the
            // ordinary case and not a problem, hence tolerating NotFound.
            // But if it has NOT run yet, killing the session guarantees it
            // never will, and nothing else would ever unlink a file
            // holding the agent's full command line, credentials
            // included. Same reasoning for the status file, which the
            // shim may have started writing.
            best_effort_remove(&spec_path, "launch spec").await;
            best_effort_remove(&status_file_path, "launch status file").await;
            return Err(result);
        }

        info!(session = %id, tmux = %tmux_name, %pane, "session created");
        self.sessions.lock().await.insert(
            id,
            Arc::new(SessionEntry {
                info: info.clone(),
                terminal: Some(Terminal { tmux_name, pane }),
            }),
        );
        Ok(info)
    }
}

/// Serve one protocol connection. Generic over the byte stream so tests
/// can drive it over an in-process duplex pipe with the same code path
/// production uses over the unix socket.
pub async fn handle_connection<S>(sup: Arc<Supervisor>, stream: S) -> anyhow::Result<()>
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    let (r, w) = tokio::io::split(stream);
    let mut reader = FrameReader::new(r);
    let mut writer = FrameWriter::new(w);
    handshake(&mut reader, &mut writer, "supervisor").await?;

    // Single writer task; everything that wants to send (request
    // handlers, the output forwarder, takeover notifications) goes
    // through this queue so frames never interleave mid-write.
    let (tx, mut rx) = mpsc::unbounded_channel::<Frame>();
    let (writer_failed_tx, mut writer_failed_rx) = oneshot::channel();
    // Progress counter for the shutdown-tail drain: `drain_writer` reads
    // this to tell "peer merely slow" apart from "peer gone" instead of
    // enforcing one flat deadline. Relaxed is enough on both ends — this
    // is a liveness heartbeat, not a value anything is synchronized on.
    let frames_written = Arc::new(AtomicU64::new(0));
    let frames_written_for_writer = Arc::clone(&frames_written);
    let mut writer_task = tokio::spawn(async move {
        while let Some(frame) = rx.recv().await {
            if let Err(e) = writer.write_frame(&frame).await {
                warn!(error = %e, "frame write to client failed");
                let _ = writer_failed_tx.send(e.to_string());
                break;
            }
            frames_written_for_writer.fetch_add(1, Ordering::Relaxed);
        }
    });

    // Which session each of this connection's data channels types into.
    // Connection-local by necessity: channel ids are unique only within a
    // connection, since every client numbers its channels from 1.
    let mut input_routes: HashMap<u32, Arc<SessionEntry>> = HashMap::new();

    // Tracking (not admission — that is now `sup.admission`, shared
    // across every connection this supervisor serves; see its own docs
    // for why it must NOT be per-connection) for the slow handlers
    // (`ListSessions`/`StopSession`/`DeleteSession`) that `handle_control`
    // spawns instead of awaiting inline. `HANDLER_SHUTDOWN_TIMEOUT`'s own
    // docs cover why leaving these untracked is not safe: without a
    // `JoinSet`, this function's shutdown tail would have nothing to wait
    // on or clean up after.
    let mut tasks: tokio::task::JoinSet<()> = tokio::task::JoinSet::new();

    let result: anyhow::Result<()> = async {
        loop {
            // Reap whatever spawned handler tasks have already finished
            // before touching the next frame — see `reap_finished_tasks`'s
            // own docs for why a long-lived connection needs this every
            // iteration, not just at shutdown.
            reap_finished_tasks(&mut tasks);
            // Either half failing ends the connection. Waiting only for
            // read EOF leaks attachments when a half-broken peer keeps
            // writing after it has stopped reading our replies.
            let frame = tokio::select! {
                frame = reader.read_frame() => frame?,
                error = &mut writer_failed_rx => {
                    let detail = error.unwrap_or_else(|_| "writer task exited".to_string());
                    return Err(anyhow::anyhow!("frame write to client failed: {detail}"));
                }
            };
            let Some(frame) = frame else {
                break;
            };
            match frame.kind {
                farhelm_proto::FrameKind::Data => {
                    // Route input only if this channel is still the
                    // session's live attachment: a client kicked by a
                    // takeover must not keep typing into a pane it no
                    // longer owns, and the supervisor enforces that
                    // rather than trusting clients to stop.
                    if let Some(entry) = input_routes.get(&frame.channel).cloned() {
                        // The check and the send-keys delivery run under
                        // ONE lock hold, like the Resize arm: releasing
                        // between them is a TOCTOU where a takeover
                        // completes in the gap and the kicked client's
                        // already-validated keystrokes land in the
                        // winner's pane — and keystrokes into an agent
                        // terminal are command execution. Safe against
                        // deadlock because forwarders never take this
                        // lock, and the Attach handler already holds it
                        // across its own tmux calls.
                        //
                        // Both halves of the check matter: channel ids are
                        // unique only within a connection (every client
                        // numbers from 1), so comparing the channel alone
                        // would let a kicked client on another connection
                        // pass whenever the numbers collide.
                        // `same_channel` identifies the owning connection.
                        let mut attachments = sup.attachments.lock().await;
                        // Borrow the matched `ActiveAttach` directly and
                        // send on it in place, rather than cloning a
                        // handle out of the map: `InputClient` owns a
                        // child process and its pipes, so unlike the old
                        // `InputWriter` there is nothing cheap to clone.
                        // The borrow (via `a`) ends when this match
                        // expression finishes evaluating, which is what
                        // lets the failure arm below still mutate
                        // `attachments` (`.remove`) under the SAME lock
                        // hold — no gap where a takeover could interleave.
                        let send_result = match attachments.get_mut(&entry.info.id) {
                            Some(a) if a.channel == frame.channel && a.notify.same_channel(&tx) => {
                                Some(a.input.send(&frame.body).await)
                            }
                            _ => None,
                        };
                        match send_result {
                            Some(Ok(())) => {}
                            // A failed send is this session's problem,
                            // not the shared connection's. It is still
                            // fatal to this attachment: accepting later
                            // chunks after silently losing one can turn
                            // a command into a different command.
                            Some(Err(e)) => {
                                warn!(session = %entry.info.id, error = %e, "input dropped");
                                if let Some(old) = attachments.remove(&entry.info.id) {
                                    old.forwarder.abort();
                                    let _ = old.forwarder.await;
                                    let _ =
                                        old.notify.send(Frame::control(&ControlMsg::Detached {
                                            channel: old.channel,
                                            reason: format!("terminal input failed: {e:#}"),
                                        }));
                                }
                                input_routes.remove(&frame.channel);
                            }
                            None => {
                                drop(attachments);
                                // This channel lost its attachment; stop
                                // holding the session entry alive for it.
                                input_routes.remove(&frame.channel);
                            }
                        }
                    }
                }
                farhelm_proto::FrameKind::Control => {
                    let msg = parse_control(&frame)?;
                    handle_control(&sup, msg, &tx, &mut input_routes, &mut tasks).await;
                }
            }
        }
        Ok(())
    }
    .await;

    // Connection gone: tear down any attachments it owned so the next
    // attach doesn't fight a dead forwarder. Abort AND await, exactly
    // like the takeover path: abort only schedules cancellation, and the
    // old control-mode client's process is not gone until the cancelled
    // task has been polled to completion. Removing the entry without
    // waiting would let a new connection's attach — which finds no
    // incumbent to kick — open its control client while the old one is
    // still dying, the documented frozen-replay hazard. Awaiting under
    // the lock is safe (forwarders never take it) and is what serializes
    // that new attach behind this teardown.
    let mut attachments = sup.attachments.lock().await;
    let mine: Vec<ActiveAttach> = attachments
        .extract_if(|_, attachment| attachment.notify.same_channel(&tx))
        .map(|(_, attachment)| attachment)
        .collect();
    for attachment in mine {
        attachment.forwarder.abort();
        let _ = attachment.forwarder.await;
    }
    drop(attachments);
    drop(tx);
    // Give the connection's spawned slow-handler tasks (list/stop/delete —
    // see `HANDLER_ADMISSION_PERMITS`'s docs) a bounded chance to finish
    // and enqueue their replies BEFORE the writer drain below starts
    // waiting on the queue those replies land in — a task that finishes
    // after the writer has already given up would have enqueued a reply
    // nobody drains. `HANDLER_SHUTDOWN_TIMEOUT` is generous (kill sweeps
    // legitimately take seconds), but a tmux wedged forever must not leak
    // this connection's shutdown forever either: past that bound, every
    // remaining task is aborted and logged rather than awaited
    // unconditionally.
    if tokio::time::timeout(HANDLER_SHUTDOWN_TIMEOUT, async {
        while tasks.join_next().await.is_some() {}
    })
    .await
    .is_err()
    {
        warn!(
            remaining = tasks.len(),
            "spawned request handler(s) did not finish within {HANDLER_SHUTDOWN_TIMEOUT:?}; \
             aborting"
        );
        tasks.abort_all();
        // `abort_all` only SCHEDULES cancellation — exactly like every
        // other abort in this module (the attachment forwarders just
        // above, `drain_writer` below), a task is not actually gone until
        // its cancellation has been delivered and polled to completion.
        // Draining `join_next` to empty here is what proves that: only
        // once every aborted task has been reaped are its resources
        // (the `tx` clone it held, any locks it could have been mid-
        // acquiring) provably released. Proceeding to `drain_writer`
        // before that could undercount `frames_written` against a
        // straggling task that was about to enqueue a reply, or race a
        // lock a still-cancelling task had not yet released.
        while tasks.join_next().await.is_some() {}
    }
    // Progress-bounded drain, not an unconditional await: see
    // WRITER_DRAIN_TIMEOUT and drain_writer. A peer that stopped reading
    // without erroring leaves the writer parked mid-write forever, with no
    // error for `writer_failed` to report — that path is already handled
    // above; this is the silent-stall case. Unlike a flat deadline,
    // `drain_writer` only gives up once a whole window passes with zero
    // frames landing, so a live-but-slow peer still gets its queued
    // replies.
    drain_writer(&mut writer_task, &frames_written, WRITER_DRAIN_TIMEOUT).await;
    result
}

/// Wait for `writer_task` to finish, giving up only once a full `window`
/// passes without a single frame completing — not after `window` elapses
/// in total.
///
/// This is what lets `handle_connection`'s shutdown tail honor the
/// half-close contract (queued replies reach a peer that is still
/// reading) without also being willing to wait forever: a peer that is
/// backpressured but alive keeps landing frames, each of which resets the
/// window, so it gets unbounded total time to drain. Only a peer that
/// lands zero frames across one whole window — the "gone" case this
/// function exists to bound — gets its writer task aborted. The abort is
/// always followed by an await of the same handle, matching every other
/// abort-then-await pairing in this module: a bare `abort()` only
/// schedules cancellation, and returning before the task is actually
/// polled to completion would let a new attach race the old writer's last
/// touch of the socket.
///
/// Honest residual: progress is observed at frame granularity via
/// `frames_written`, not at the byte level. A single frame whose own
/// write spans the entire window with nothing else completing — i.e. a
/// link slower than the frame size divided by `window` (for the 32 KiB
/// `REPLAY_CHUNK` and the default 5s window, under roughly 6.4 KB/s) —
/// is indistinguishable here from a peer that is truly gone, and gets
/// aborted. That is accepted, not accidental: catching it would need
/// sub-frame progress reporting, which `FrameWriter` does not have.
async fn drain_writer(
    writer_task: &mut tokio::task::JoinHandle<()>,
    frames_written: &AtomicU64,
    window: Duration,
) {
    loop {
        let before = frames_written.load(Ordering::Relaxed);
        if tokio::time::timeout(window, &mut *writer_task)
            .await
            .is_ok()
        {
            // Writer task finished (queue drained or it hit a write
            // error) within this window; nothing left to bound.
            return;
        }
        if frames_written.load(Ordering::Relaxed) != before {
            // At least one frame landed during the window that just
            // timed out: the peer is slow, not gone. Give it another
            // window instead of counting this as no progress.
            continue;
        }
        warn!("no frame completed for a full {window:?} window; aborting writer task");
        writer_task.abort();
        let _ = writer_task.await;
        return;
    }
}

/// `ListSessions`'s count cap (PLAN_M2.md's "Proto growth"). ~500 keeps a
/// single reply's session count bounded before the byte budget below ever
/// has to do the harder job of bounding fat, variable-length records.
const LIST_SESSION_CAP: usize = 500;

/// `ListSessions`'s encoded-size budget, independent of the count cap: a
/// count alone cannot bound encoded bytes when each session's title, cwd,
/// and invocation are caller-controlled strings of unbounded length — 500
/// sessions with fat titles can still blow past `MAX_FRAME_LEN` on their
/// own. Deliberately well under `MAX_FRAME_LEN` (half of it) rather than
/// flush against it: `Frame::encoded_len` (what this budget is compared
/// against, in `build_list_reply`) already accounts for the frame's own
/// envelope — the header and the `SessionList` object's fixed fields —
/// which is a few dozen bytes, negligible next to a multi-megabyte cap.
/// The margin is headroom for a future additive `SessionList`/
/// `SessionInfo` field instead: a number tuned flush against today's
/// fields would need re-tuning the moment PLAN_M2.md adds another one.
/// `reply_frame`'s oversize defusal stays as the last-resort backstop
/// regardless — this budget is meant to make that backstop unreachable in
/// practice, not to replace it.
const LIST_BYTE_BUDGET: usize = (farhelm_proto::MAX_FRAME_LEN / 2) as usize;

/// Compute one session's liveness for a `ListSessions` reply. tmux is the
/// truth (module docs); this function only ever reports what it can
/// actually observe, never a guess.
///
/// Three cases all collapse into the same honest `Exited { exit_code:
/// None }` rather than assuming alive:
/// - no terminal at all (the restart-gap entry);
/// - this pane id is entirely absent from `pane_states` (removed mid-
///   query, or never existed on this server at all);
/// - this pane id IS present, but for a DIFFERENT session name than the
///   one this entry remembers creating it under. Pane ids reset to `%0`
///   on a fresh tmux server (`PaneState::session_name`'s own docs), so a
///   stale, never-reloaded entry's pane id can be silently recycled by an
///   unrelated NEW session after a server restart; matching pane id alone
///   would let that entry inherit the new session's liveness. Requiring
///   BOTH identifiers to agree is also what a tmux-side rename of the
///   session name (a rare, deliberately-provoked edge case, not a normal
///   product flow) trips: this function has no positive way to confirm
///   the renamed pane is still "the same session" rather than tmux having
///   handed that pane to something else entirely, so it reports the same
///   honest `Exited` rather than guessing either way.
///
/// Only a pane found under BOTH its remembered pane id and its remembered
/// tmux session name gets to decide `Alive` vs. `Exited` from tmux's own
/// dead flag and status.
fn session_status(entry: &SessionEntry, pane_states: &HashMap<String, PaneState>) -> SessionStatus {
    let Some(state) = entry.terminal.as_ref().and_then(|terminal| {
        pane_states
            .get(&terminal.pane)
            .filter(|state| state.session_name == terminal.tmux_name)
    }) else {
        return SessionStatus::Exited { exit_code: None };
    };
    if state.dead {
        SessionStatus::Exited {
            exit_code: state.exit_code,
        }
    } else {
        SessionStatus::Alive
    }
}

/// Byte-budget half of PLAN_M2.md's list truncation. The count cap is the
/// CALLER's job, applied before this is ever reached — `handle_control`'s
/// `ListSessions` arm takes at most `LIST_SESSION_CAP` entries from the
/// session map before cloning or status-annotating a single one of them
/// (see that arm's own comment for why paying that cost for entries this
/// function would only drop anyway is wasteful to avoid in the first
/// place). Because of that, this function cannot reconstruct the true
/// pre-cap session count from `sessions.len()` — `total` is supplied by
/// the caller instead, and is reported as-is; `truncated` is set whenever
/// fewer than `total` survive (whether the caller's cap or this
/// function's own byte budget is why).
///
/// Truncation drops from the TAIL of whatever `sessions` it receives.
/// Ordering note: sessions have no defined order today (the module docs,
/// and `SessionList`'s own doc comment), so "the tail" is whatever order
/// the caller's map iteration happened to yield — an arbitrary subset
/// survives, not a deliberately chosen one. That is acceptable for a
/// budget meant to bound worst-case reply size, not to page through a
/// stable ordering; a defined order (and real pagination) is M6's concern
/// (PLAN_M2.md), not this one.
///
/// Single-pass, exact size accounting, and the final reply is constructed
/// exactly ONCE — a previous version re-encoded a shrinking candidate on
/// every dropped entry, which is quadratic in the number of entries
/// eventually dropped. Instead: `envelope_len` is the encoded size of this
/// SAME reply shape with an empty `sessions` array, measured once via the
/// real `Frame`/`ControlMsg` path (never hand-computed, so it can't drift
/// from what `Frame::control` actually produces); each candidate entry is
/// serialized exactly once (`serde_json::to_vec`) and its EXACT marginal
/// contribution to the `sessions` JSON array — its own bytes, plus one
/// comma separator once it is not the first surviving entry — is added to
/// a running total seeded from `envelope_len`. An entry that would push
/// the running total over `byte_budget` stops the scan; everything kept
/// up to that point is the final answer.
///
/// The envelope is deliberately measured with `truncated: false` even
/// though the real answer might turn out `true`: JSON's `false` encodes
/// ONE BYTE LONGER than `true` (5 ASCII characters vs. 4), so basing the
/// accounting on the longer of the two can only ever OVER-count the
/// envelope's own size, never under-count it — whatever this function
/// returns is always at least as small as the accounting assumed, never
/// larger. Getting this backwards (measuring against the SHORTER `true`)
/// would under-count an untruncated reply by exactly one byte, which is
/// academic for an ordinary reply deep under budget but tightens to a
/// real one-byte overshoot — tripping the `debug_assert!` below in tests,
/// and in release risking a reply one byte over `byte_budget` — for a
/// reply that lands EXACTLY at the budget boundary.
///
/// A `debug_assert!` re-encodes the actual returned reply as a sanity
/// check that the accounting above never drifted from reality. It is
/// deliberately not a release-mode check: `reply_frame`'s `MAX_FRAME_LEN`
/// defusal remains the real last-resort backstop in production; this
/// assert exists only to catch an accounting bug in tests/debug builds
/// before it could ever reach that backstop. `byte_budget.max(envelope_len)`
/// tolerates the degenerate case of a budget smaller than the envelope
/// itself (only reachable with a pathologically tiny `byte_budget`, never
/// `LIST_BYTE_BUDGET` in production) — this function must still return
/// SOMETHING even then, and the assert should not fire over a caller
/// having chosen an unreasonable budget.
fn build_list_reply(
    req_id: u64,
    sessions: Vec<SessionInfo>,
    total: u64,
    byte_budget: usize,
) -> ControlMsg {
    let envelope_len = Frame::control(&ControlMsg::SessionList {
        req_id,
        sessions: Vec::new(),
        total,
        truncated: false,
    })
    .encoded_len();

    let mut kept = Vec::with_capacity(sessions.len());
    let mut used = envelope_len;
    for session in sessions {
        let separator = if kept.is_empty() { 0 } else { 1 };
        let entry_len = serde_json::to_vec(&session)
            .expect("SessionInfo is always serializable")
            .len()
            + separator;
        if used + entry_len > byte_budget {
            break;
        }
        used += entry_len;
        kept.push(session);
    }

    let truncated = (kept.len() as u64) < total;
    let reply = ControlMsg::SessionList {
        req_id,
        sessions: kept,
        total,
        truncated,
    };
    debug_assert!(
        Frame::control(&reply).encoded_len() <= byte_budget.max(envelope_len),
        "build_list_reply's single-pass size accounting drifted from the real encoded size"
    );
    reply
}

/// Build the frame for a per-request reply, degrading to `ControlMsg::Error`
/// if the honest reply would not fit on the wire.
///
/// Callers send by pushing onto `tx`, the channel the writer task
/// (`handle_connection`) drains — without ever observing whether the
/// frame they built actually encodes. The writer discovers an oversized
/// frame only later, as a write error indistinguishable from the
/// transport genuinely breaking, and (correctly, for a real transport
/// failure) treats ANY write failure as connection-fatal. An oversized
/// frame is not a transport failure, though; it is a message this
/// connection can never send, and finding that out at the writer would
/// tear down every attachment sharing the connection over one bad reply.
/// So the check has to happen here, before the frame is ever enqueued.
/// `ListSessions` is the only M1 reply that can realistically hit this:
/// one frame carries every session, so a host with enough sessions (or
/// unusually large titles) can legitimately exceed `MAX_FRAME_LEN`. M2
/// adds a count cap plus an encoded-size budget to the list reply, and
/// real pagination is M6 (PLAN.md); this defusal stays as the last-resort
/// backstop even then. The substituted `Error` reply is small by
/// construction — just a `req_id` and a fixed-shape message — so it
/// always fits.
///
/// Only call this for messages that carry a `req_id`: it panics otherwise,
/// which is deliberate. A reply silently sent unchecked here would be the
/// same oversized-frame-reaches-the-writer bug this function exists to
/// close; better to fail loudly at the call site than send something that
/// might carry no `req_id` for the substitute `Error` to correlate against.
fn reply_frame(msg: &ControlMsg) -> Frame {
    let req_id = match *msg {
        ControlMsg::SessionCreated { req_id, .. }
        | ControlMsg::SessionList { req_id, .. }
        | ControlMsg::SessionStopped { req_id, .. }
        | ControlMsg::SessionDeleted { req_id, .. }
        | ControlMsg::Attached { req_id, .. }
        | ControlMsg::Error { req_id, .. } => req_id,
        ref other => {
            unreachable!("reply_frame called with a message that carries no req_id: {other:?}")
        }
    };
    let frame = Frame::control(msg);
    if frame.exceeds_max_len() {
        warn!(
            req_id,
            size = frame.encoded_len(),
            "control reply exceeds max frame size; substituting an error reply"
        );
        Frame::control(&ControlMsg::Error {
            req_id,
            message: format!(
                "reply encodes to {} bytes, exceeding the {}-byte frame limit",
                frame.encoded_len(),
                farhelm_proto::MAX_FRAME_LEN,
            ),
            // Encoding succeeded; the encoded frame is simply too big for
            // the wire. Still the server's own limit, not something the
            // caller's request got wrong.
            kind: ErrorKind::Internal,
        })
    } else {
        frame
    }
}

/// Push `m` (through [`reply_frame`]'s oversize check) onto `tx`.
///
/// A tiny free function rather than a closure over `tx` specifically so
/// spawned handler tasks (`ListSessions`/`StopSession`/`DeleteSession` in
/// `handle_control` — see those arms' own comments on why they spawn) can
/// share it after moving their own OWNED clone of `tx` into the task: a
/// closure captured by reference cannot outlive the stack frame that
/// spawned it, but this function only ever borrows `tx` for the instant
/// of the call, so the same helper works whether the caller holds `tx` by
/// reference (the synchronous arms below) or by owned clone (the spawned
/// ones).
fn send_reply(tx: &mpsc::UnboundedSender<Frame>, m: &ControlMsg) {
    let _ = tx.send(reply_frame(m));
}

/// Acquire an admission permit and spawn `future` onto `tasks`, holding
/// the permit for the future's entire lifetime — the one, shared
/// implementation of the admission-then-spawn pattern every slow
/// `handle_control` arm (`ListSessions`/`StopSession`/`DeleteSession`)
/// uses, so the ordering below cannot drift between call sites.
///
/// The permit is acquired HERE, in THIS function's own await — which
/// means in the CALLER's await point, since this is not itself spawned —
/// not inside `future` once it is already running as its own task. That
/// ordering is the entire point: every real caller is `handle_control`,
/// invoked directly from `handle_connection`'s read loop, so an
/// admission-exhausted flood of slow requests blocks THAT loop right
/// here, before a task (or a `JoinSet` entry for it) exists at all —
/// rather than spawning and tracking an unbounded number of not-yet-
/// admitted tasks that all sit parked on the semaphore. See
/// `HANDLER_ADMISSION_PERMITS`'s docs for why that distinction matters.
async fn spawn_admitted<F>(
    admission: &Arc<tokio::sync::Semaphore>,
    tasks: &mut tokio::task::JoinSet<()>,
    future: F,
) where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let permit = Arc::clone(admission)
        .acquire_owned()
        .await
        .expect("admission semaphore is never closed");
    tasks.spawn(async move {
        let _permit = permit;
        future.await;
    });
}

/// Non-blockingly collect every spawned handler task that has ALREADY
/// finished, logging (not propagating) any `JoinError` — a panic inside a
/// handler, or a cancellation from `handle_connection`'s own shutdown
/// tail. `JoinSet` does not free a task's slot on its own just because
/// the task completed; something has to call `join_next`/
/// `try_join_next` to actually collect it, so this must run periodically
/// on any connection expected to live a while — `handle_connection`'s read
/// loop calls it once per iteration specifically so a long-lived polling
/// connection (a UI's `ListSessions` loop, potentially running for hours)
/// does not accumulate one finished-but-unreaped entry per request for
/// its entire lifetime. `try_join_next` (not `join_next`) is what keeps
/// this non-blocking: it returns `None` immediately once nothing is
/// ready, rather than waiting for the next task to finish.
fn reap_finished_tasks(tasks: &mut tokio::task::JoinSet<()>) {
    while let Some(result) = tasks.try_join_next() {
        if let Err(e) = result {
            warn!(error = %e, "spawned request handler task panicked or was cancelled");
        }
    }
}

/// Dispatch one control message from a connected client.
///
/// Failures belonging to one request—bad cwd, a tmux hiccup, an unknown
/// session—become `ControlMsg::Error` replies here. They must not escape
/// into the connection loop: one connection carries every session the
/// helm is driving, so request-local failure cannot be allowed to detach
/// unrelated terminals.
///
/// `tx` doubles as this connection's identity: `same_channel` against it
/// is how the handlers tell "the connection that owns this attachment"
/// from any other, which channel ids alone cannot do.
async fn handle_control(
    sup: &Arc<Supervisor>,
    msg: ControlMsg,
    tx: &mpsc::UnboundedSender<Frame>,
    input_routes: &mut HashMap<u32, Arc<SessionEntry>>,
    tasks: &mut tokio::task::JoinSet<()>,
) {
    let send = |m: &ControlMsg| send_reply(tx, m);
    match msg {
        ControlMsg::CreateSession {
            req_id,
            cwd,
            invocation,
            title,
            cols,
            rows,
        } => {
            let field_len = cwd.len() + invocation.len() + title.as_deref().map_or(0, str::len);
            if field_len > CREATE_FIELD_CAP {
                send(&ControlMsg::Error {
                    req_id,
                    message: format!(
                        "cwd, invocation, and title together are {field_len} bytes, \
                         exceeding the {CREATE_FIELD_CAP}-byte limit"
                    ),
                    kind: ErrorKind::InvalidRequest,
                });
                return;
            }
            match sup
                .create_session(&cwd, &invocation, title, cols, rows)
                .await
            {
                Ok(session) => send(&ControlMsg::SessionCreated { req_id, session }),
                Err(e) => send(&ControlMsg::Error {
                    req_id,
                    message: format!("{e:#}"),
                    kind: error_kind(&e),
                }),
            }
        }
        ControlMsg::ListSessions { req_id } => {
            // Spawned onto its own task rather than awaited inline: this
            // arm is reached from `handle_connection`'s single serial read
            // loop, and `TmuxDriver::pane_states` is a real subprocess
            // round trip that can block for as long as tmux takes to
            // answer (a wedged or merely slow tmux, under load). Awaiting
            // it inline would stall every OTHER request on this
            // connection — attach, input, another session's list/stop/
            // delete — behind this one `ListSessions`. Spawning is safe:
            // this arm only reads `sup.sessions` under its own lock hold
            // and never touches `input_routes` (connection-local state,
            // never shared with a spawned task), the map-wide mutex
            // already tolerates concurrent requests interleaving (see the
            // `Supervisor` struct's lock-discipline docs), and replies are
            // correlated by `req_id` rather than by arrival or completion
            // order (already true of every request on this connection).
            //
            // Tracked in `tasks` (a `JoinSet`) and admitted through
            // `spawn_admitted` rather than a bare `tokio::spawn`: see
            // `HANDLER_ADMISSION_PERMITS`/`HANDLER_SHUTDOWN_TIMEOUT`'s own
            // docs for why an unbounded, untracked spawn per slow request
            // is not safe to leave unmanaged.
            let sup2 = Arc::clone(sup);
            let tx = tx.clone();
            spawn_admitted(&sup.admission, tasks, async move {
                let sup = sup2;
                // `total` is captured, and the count cap applied, BEFORE
                // a single entry is cloned or status-annotated: cloning
                // (an `Arc` bump, cheap) is bounded by `.take(cap)` here,
                // but the PER-ENTRY status computation just below is not
                // free, and doing it for entries that `build_list_reply`
                // would only drop a moment later wastes work proportional
                // to however far over the cap the host is.
                let (entries, total): (Vec<Arc<SessionEntry>>, u64) = {
                    let sessions = sup.sessions.lock().await;
                    let total = sessions.len() as u64;
                    let entries = sessions.values().take(LIST_SESSION_CAP).cloned().collect();
                    (entries, total)
                };
                // ONE query for every session's liveness, not one per
                // session (`TmuxDriver::pane_states`'s own docs on why
                // that multiplies subprocess spawns under a polling UI) —
                // and skipped altogether when it could not possibly
                // change the answer: a terminal-less entry's status is
                // `Exited` unconditionally (`session_status` never
                // consults the map for one), so a capped subset that is
                // ALL terminal-less (including the empty list) is fully
                // decidable without asking tmux anything. This matters
                // beyond just saving a subprocess spawn: it is what keeps
                // an authoritative "every session is a restart gap" (or
                // simply empty) listing from being turned into a spurious
                // `Internal` error by a private tmux server that happens
                // to ALSO be down for an unrelated reason.
                let pane_states = if entries.iter().any(|entry| entry.terminal.is_some()) {
                    match sup.tmux.pane_states().await {
                        Ok(states) => states,
                        // Reached only for a genuinely UNCLASSIFIED tmux
                        // failure: `TmuxDriver::pane_states` itself now
                        // tolerates a vanished private tmux server (the
                        // whole reason a dead-tmux-server `ListSessions`
                        // no longer lands here at all — see that method's
                        // own docs for why an empty pane-states map is
                        // honest, not fabricated, in that case).
                        Err(e) => {
                            send_reply(
                                &tx,
                                &ControlMsg::Error {
                                    req_id,
                                    message: format!("{e:#}"),
                                    kind: ErrorKind::Internal,
                                },
                            );
                            return;
                        }
                    }
                } else {
                    HashMap::new()
                };
                let sessions: Vec<SessionInfo> = entries
                    .iter()
                    .map(|entry| {
                        let mut info = entry.info.clone();
                        info.status = session_status(entry, &pane_states);
                        info
                    })
                    .collect();
                send_reply(
                    &tx,
                    &build_list_reply(req_id, sessions, total, LIST_BYTE_BUDGET),
                );
            })
            .await;
        }
        ControlMsg::StopSession { req_id, session_id } => {
            // Spawned for the same reason as `ListSessions`: the process-
            // tree sweep below (`kill_process_tree`) is a grace-period
            // sleep plus repeated `/proc` walks and confirmation polls
            // that can take real wall-clock seconds (see that function's
            // own docs), and awaiting it inline would stall every OTHER
            // session's attach, input, and list/stop/delete behind this
            // one stop. Safe for the same locking reason: this arm's
            // `sessions` lookup is a single lock-guarded clone, and stop
            // deliberately never touches `attachments` or `input_routes`
            // at all (see `ControlMsg::StopSession`'s own docs on why the
            // existing attachment is left untouched). Tracked and admitted
            // exactly like `ListSessions` above — see
            // `HANDLER_ADMISSION_PERMITS`/`HANDLER_SHUTDOWN_TIMEOUT`.
            let sup2 = Arc::clone(sup);
            let tx = tx.clone();
            spawn_admitted(&sup.admission, tasks, async move {
                let sup = sup2;
                let entry = sup.sessions.lock().await.get(&session_id).cloned();
                let Some(entry) = entry else {
                    send_reply(
                        &tx,
                        &ControlMsg::Error {
                            req_id,
                            message: format!(
                                "no such session: {}",
                                truncate_for_error(&session_id)
                            ),
                            kind: ErrorKind::NotFound,
                        },
                    );
                    return;
                };
                // A dead or absent pane, or a terminal-less (restart-gap)
                // entry, all mean there is no live pid worth walking
                // ancestry from — but the environment-marker sweep still
                // runs regardless (`root_pid: None`), because SPEC.md
                // assigns reaping any leftover descendants of a PAST run
                // to the session's next stop or delete, and the marker
                // scan is the only mechanism that can still find such a
                // survivor once there is no live pane to walk from at
                // all. See `kill_process_tree`'s docs.
                let pane_state = match entry.terminal.as_ref() {
                    Some(terminal) => match sup
                        .tmux
                        .pane_process(&terminal.tmux_name, &terminal.pane)
                        .await
                    {
                        Ok(pane) => pane,
                        Err(e) => {
                            send_reply(
                                &tx,
                                &ControlMsg::Error {
                                    req_id,
                                    message: format!("{e:#}"),
                                    kind: error_kind(&e),
                                },
                            );
                            return;
                        }
                    },
                    None => None,
                };
                // One "alive" check feeding BOTH the pid a tree-kill walks
                // from and the decision to even attempt an alt-screen
                // capture below, rather than two independent `!pane.dead`
                // checks scattered across this handler. The stale pid a
                // dead pane still reports is deliberately never read via
                // either use of `alive_pane`; it may already be recycled.
                let alive_pane = pane_state.filter(|pane| !pane.dead);
                let root_pid = alive_pane.map(|pane| pane.pid);

                // Capture the alt-screen snapshot (if any) BEFORE the kill
                // destroys it, but do NOT write it to disk yet — see
                // `publish_alt_screen_snapshot`'s docs for why publishing
                // waits until the kill's own outcome is known. `alive_pane`
                // being `Some` is what gates this to a pane actually worth
                // querying at all; `capture_alt_screen_before_stop` itself
                // decides (atomically, in tmux) whether that pane is really
                // on the alternate screen.
                let pending_snapshot = match (entry.terminal.as_ref(), alive_pane) {
                    (Some(terminal), Some(_)) => {
                        capture_alt_screen_before_stop(&sup, &session_id, terminal).await
                    }
                    _ => None,
                };
                // Published into `Supervisor::pending_snapshots` (see that
                // field's own docs) BEFORE the kill runs: `kill_process_tree`
                // can take up to a couple of seconds against an uncooperative
                // tree, and tmux can mark the pane dead well before that
                // returns. Making the capture visible to a concurrent
                // `Attach` for this whole window — not only after
                // `publish_alt_screen_snapshot` finally writes it to disk —
                // is what closes the "attach lands mid-stop, sees a dead pane
                // with nothing to show" gap. Cloned rather than moved: this
                // handler still needs its own copy below regardless of what
                // `Attach` does with the map's copy concurrently.
                if let Some(bytes) = pending_snapshot.clone() {
                    sup.pending_snapshots
                        .lock()
                        .await
                        .insert(session_id.clone(), bytes);
                }

                let kill_result = kill_process_tree(root_pid, &session_id).await;
                if let Err(e) = kill_result {
                    // The sweep itself failed (not just "nothing was found
                    // to kill") — this is not a false success. See
                    // ControlMsg::StopSession's docs: an unknown id is the
                    // only PRECONDITION failure; a sweep that could not
                    // complete is reported the same honest way. Any captured
                    // (but not yet written) snapshot bytes are simply dropped
                    // here on the way out — including the pending-map entry
                    // just inserted above, removed without ever being
                    // published: a failed stop must never plant a snapshot
                    // file a later, unrelated exit's own dead-pane replay
                    // could be mistaken for.
                    sup.pending_snapshots.lock().await.remove(&session_id);
                    send_reply(
                        &tx,
                        &ControlMsg::Error {
                            req_id,
                            message: format!("{e:#}"),
                            kind: ErrorKind::Internal,
                        },
                    );
                    return;
                }
                if let Some(bytes) = pending_snapshot {
                    publish_alt_screen_snapshot(&sup, &session_id, &bytes).await;
                }
                // Removed only now, AFTER publish has run (or been skipped
                // because there was never anything to publish): a concurrent
                // `Attach` must be able to see this entry for the entire
                // capture-to-published-file window, not just up to this
                // point — see `Supervisor::pending_snapshots`'s docs.
                sup.pending_snapshots.lock().await.remove(&session_id);
                // Deliberately untouched: the DB row, the sessions map, and
                // any live attachment. The pane survives (remain-on-exit),
                // so an attached client's stream simply goes quiet after the
                // agent's death output — there is nothing here for it to be
                // notified of, unlike delete below.
                send_reply(&tx, &ControlMsg::SessionStopped { req_id });
            })
            .await;
        }
        ControlMsg::DeleteSession { req_id, session_id } => {
            // Spawned for the same reason as `StopSession` — the same
            // process-tree sweep, plus tmux teardown and SQLite writes on
            // top — and, being the slowest of the three handlers spawned
            // here, the one this change matters most for. Safe for the
            // same reason: everything this arm touches (`sessions`,
            // `attachments`, `tmux`, `store`) is already designed to
            // tolerate concurrent requests interleaving (see the
            // `Supervisor` struct's lock-discipline docs, and this arm's
            // own existing comments on why the sweep runs before any lock
            // is held at all). Tracked and admitted exactly like
            // `ListSessions` above — see
            // `HANDLER_ADMISSION_PERMITS`/`HANDLER_SHUTDOWN_TIMEOUT`.
            let sup2 = Arc::clone(sup);
            let tx = tx.clone();
            spawn_admitted(&sup.admission, tasks, async move {
                let sup = sup2;
                let entry = sup.sessions.lock().await.get(&session_id).cloned();
                let Some(entry) = entry else {
                    send_reply(
                        &tx,
                        &ControlMsg::Error {
                            req_id,
                            message: format!(
                                "no such session: {}",
                                truncate_for_error(&session_id)
                            ),
                            kind: ErrorKind::NotFound,
                        },
                    );
                    return;
                };

                // The process-tree sweep runs BEFORE any lock is held: it can
                // take seconds (a grace period plus several /proc walks), and
                // holding `attachments` for that long would stall every OTHER
                // session's attach/input behind one slow delete — the map-
                // wide mutex's already-documented coarseness (see the
                // `Supervisor` struct's lock-discipline docs) made worse if a
                // multi-second sweep sat inside it. A concurrent Attach can
                // therefore install a fresh attachment WHILE this runs; the
                // lock-held phase below tears down WHATEVER attachment exists
                // by the time it runs, new or old, and gives it the deleted
                // notice — that is the one acceptable consequence of not
                // holding the lock here, not an oversight.
                //
                // Same dead/absent/terminal-less handling as `StopSession`:
                // the marker sweep still runs even with no live pane pid, for
                // the same leftover-reaping reason documented there.
                let root_pid = match entry.terminal.as_ref() {
                    Some(terminal) => match sup
                        .tmux
                        .pane_process(&terminal.tmux_name, &terminal.pane)
                        .await
                    {
                        Ok(Some(pane)) if !pane.dead => Some(pane.pid),
                        Ok(_) => None,
                        Err(e) => {
                            send_reply(
                                &tx,
                                &ControlMsg::Error {
                                    req_id,
                                    message: format!("querying pane process: {e:#}"),
                                    kind: ErrorKind::Internal,
                                },
                            );
                            return;
                        }
                    },
                    None => None,
                };
                if let Err(e) = kill_process_tree(root_pid, &session_id).await {
                    send_reply(
                        &tx,
                        &ControlMsg::Error {
                            req_id,
                            message: format!("killing process tree: {e:#}"),
                            kind: ErrorKind::Internal,
                        },
                    );
                    return;
                }

                // Everything from here on is fast (one tmux round trip, two
                // best-effort-but-fail-closed file removals, one sqlite
                // write) and runs under `attachments`, mirroring the Attach
                // handler's takeover for the same reason: a concurrent Attach
                // must not be able to install itself mid-teardown. This is
                // also the one path that acquires BOTH locks at once — `map
                // removal` below briefly takes `sessions` too, while still
                // holding `attachments` — which is the ordering rule this
                // establishes and the only one that needs to exist as long as
                // nothing else ever needs both: `attachments` first,
                // `sessions` second.
                let mut attachments = sup.attachments.lock().await;
                // Abort the forwarder now, before it can race its own natural
                // "session terminal ended" Detached against whatever truthful
                // notice this handler sends once the real outcome below is
                // known — but do not send that notice yet. Destructured
                // (rather than kept as one `ActiveAttach`) because `forwarder`
                // is consumed by the await and `channel`/`notify` are needed
                // again afterwards; `input` is dropped here, which is fine —
                // dropping it kills its control-mode client via
                // `kill_on_drop`, exactly like every other teardown path.
                let notify_detach = match attachments.remove(&session_id) {
                    Some(ActiveAttach {
                        channel,
                        notify,
                        forwarder,
                        input: _input,
                    }) => {
                        forwarder.abort();
                        let _ = forwarder.await;
                        Some((channel, notify))
                    }
                    None => None,
                };

                // Fail-closed and sequenced deliberately: artifacts before the
                // DB row (a leftover launch spec may hold credentials, and
                // this is the last moment anything will ever come back to
                // remove it — see `remove_fail_closed`'s docs), and the row
                // only after the terminal and process tree are positively
                // gone (a crash here leaves a listed-but-dead session,
                // recoverable by the next delete or a manual cleanup, rather
                // than an unlisted-but-running agent, invisible and
                // unreapable — see lore/2026-07-27-m2-process-tree-stop.md's
                // final paragraph). One `Result`-returning block with `?`
                // rather than a hand-threaded `teardown_error` variable, now
                // that none of these steps need to happen outside the lock.
                let teardown: Result<(), String> = async {
                    if let Some(terminal) = entry.terminal.as_ref() {
                        sup.tmux
                            .kill_session(&terminal.tmux_name)
                            .await
                            .map_err(|e| format!("killing tmux session: {e:#}"))?;
                    }
                    let launch_dir = sup.state_dir.join("launch");
                    remove_fail_closed(
                        &launch_dir.join(format!("{session_id}.json")),
                        "launch spec",
                    )
                    .await?;
                    remove_fail_closed(
                        &launch_dir.join(format!("{session_id}.status")),
                        "launch status file",
                    )
                    .await?;
                    // Same fail-closed treatment as the launch artifacts
                    // above and for the same reason: the snapshot can hold
                    // secrets an agent echoed to an alt-screen app, and
                    // delete is the last moment anything will ever come
                    // back to remove it.
                    remove_fail_closed(
                        &snapshot_path(&sup.state_dir, &session_id),
                        "alt-screen snapshot",
                    )
                    .await?;
                    sup.store
                        .delete_session(&session_id)
                        .await
                        .map_err(|e| format!("{e:#}"))
                }
                .await;

                if let Err(err_msg) = teardown {
                    if let Some((channel, notify)) = notify_detach {
                        let _ = notify.send(Frame::control(&ControlMsg::Detached {
                            channel,
                            reason: format!("detached during a failed delete: {err_msg}"),
                        }));
                    }
                    drop(attachments);
                    send_reply(
                        &tx,
                        &ControlMsg::Error {
                            req_id,
                            message: err_msg,
                            kind: ErrorKind::Internal,
                        },
                    );
                    return;
                }
                sup.sessions.lock().await.remove(&session_id);

                if let Some((channel, notify)) = notify_detach {
                    let _ = notify.send(Frame::control(&ControlMsg::Detached {
                        channel,
                        reason: "session deleted".to_string(),
                    }));
                }
                drop(attachments);
                send_reply(&tx, &ControlMsg::SessionDeleted { req_id });
            })
            .await;
        }
        ControlMsg::Attach {
            req_id,
            session_id,
            channel,
            cols,
            rows,
        } => {
            if channel == 0 || input_routes.contains_key(&channel) {
                let message = if channel == 0 {
                    "attachment channel 0 is reserved".to_string()
                } else {
                    format!("attachment channel {channel} is already in use")
                };
                send(&ControlMsg::Error {
                    req_id,
                    message,
                    kind: ErrorKind::InvalidRequest,
                });
                return;
            }
            let entry = sup.sessions.lock().await.get(&session_id).cloned();
            let Some(entry) = entry else {
                send(&ControlMsg::Error {
                    req_id,
                    message: format!("no such session: {}", truncate_for_error(&session_id)),
                    kind: ErrorKind::NotFound,
                });
                return;
            };
            // The restart-gap case (PLAN_M2.md): this entry was reloaded
            // from SQLite at startup and its tmux session was gone by
            // then. Reporting `NotFound` here — rather than fabricating a
            // dead terminal to attach to — is the same "do not guess"
            // discipline SPEC.md applies elsewhere; the session stays
            // visible in the list either way.
            let Some(terminal) = entry.terminal.as_ref() else {
                send(&ControlMsg::Error {
                    req_id,
                    message: format!(
                        "session {session_id} has no terminal: the supervisor (or its tmux \
                         server) restarted after the agent ended"
                    ),
                    kind: ErrorKind::NotFound,
                });
                return;
            };

            // The whole takeover — kick the old attachment, set up tmux,
            // install the new one — runs under one lock. Without it, two
            // concurrent attaches can both pass the kick step and both
            // install forwarders, leaving two live attachments for a
            // session SPEC.md says has at most one. It also makes the
            // winner the *last* attach rather than whichever client's
            // tmux calls happened to finish last.
            let mut attachments = sup.attachments.lock().await;

            if let Some(old) = attachments.remove(&session_id) {
                old.forwarder.abort();
                // Awaiting the abort is what actually makes the old
                // control-mode client gone: dropping its OutputStream
                // (and so killing the process) happens when the task is
                // polled after cancellation, not when abort() returns.
                // The forwarder never takes this lock, so awaiting it
                // here cannot deadlock.
                let _ = old.forwarder.await;
                let _ = old.notify.send(Frame::control(&ControlMsg::Detached {
                    channel: old.channel,
                    reason: "another client attached".to_string(),
                }));
            }

            // Size the window now, not during prep: resizing is a
            // mutation the incumbent would have seen, and a later prep
            // failure would have left its terminal reflowed to a size
            // nobody is using.
            if let Err(e) = sup
                .tmux
                .resize_window(&terminal.tmux_name, cols, rows)
                .await
            {
                warn!(session = %session_id, error = %e, "resize during attach failed");
            }

            // The incumbent must be fully gone before the replacement
            // control client starts. Overlap reproducibly froze the new
            // stream after replay, even though two steady-state control
            // clients both receive output in isolation. The replacement
            // attaches with output disabled, captures replay, and enables
            // live output through that SAME client; its final command
            // block is the exact replay/live boundary.
            //
            // This setup happens after takeover on purpose. There is no
            // safe way to preserve the incumbent while also avoiding
            // control-client overlap, so failure here leaves the session
            // detached and reports only this attach request as failed.
            let (modes, prefill, mut stream) = match sup
                .tmux
                .open_replay_stream(&terminal.tmux_name, &terminal.pane)
                .await
            {
                Ok(parts) => parts,
                Err(e) => {
                    drop(attachments);
                    send(&ControlMsg::Error {
                        req_id,
                        message: format!("{e:#}"),
                        kind: error_kind(&e),
                    });
                    return;
                }
            };
            // A second, dedicated control-mode client for this
            // attachment's input (see `InputClient`) — opened here rather
            // than derived from `stream`, since the two are now
            // independent control connections rather than one shared
            // stdin. A failure here must tear down the replay stream just
            // opened above: leaving it live would attach this session to
            // a client nothing will ever read from or write to again.
            let input = match sup.tmux.open_input_client(&terminal.pane).await {
                Ok(input) => input,
                Err(e) => {
                    drop(attachments);
                    stream.shutdown().await;
                    send(&ControlMsg::Error {
                        req_id,
                        message: format!("{e:#}"),
                        kind: error_kind(&e),
                    });
                    return;
                }
            };

            send(&ControlMsg::Attached { req_id, channel });
            // Order is load-bearing: the alternate-screen switch must
            // precede the content (it clears the buffer it switches to),
            // and cursor placement must follow it (writing content moves
            // the cursor). See PaneModes.
            let _ = tx.send(Frame::data(
                channel,
                modes.pre_content_sequences().into_bytes(),
            ));
            for chunk in prefill.chunks(REPLAY_CHUNK) {
                let _ = tx.send(Frame::data(channel, chunk.to_vec()));
            }
            let _ = tx.send(Frame::data(
                channel,
                modes.post_content_sequences().into_bytes(),
            ));
            // Append the alt-screen stop snapshot for any DEAD pane —
            // gated on the snapshot's own EXISTENCE (inside
            // `send_alt_screen_snapshot`), not on the pane's current
            // screen. `modes.alternate_on` decides only where the
            // snapshot lands, not whether it is worth appending at all.
            //
            // An earlier version of this gate also required
            // `!modes.alternate_on`, on the theory that a dead pane still
            // on the alternate screen already shows its last frame via
            // the ordinary prefill above. That reasoning was empirically
            // wrong (verified against a real tmux server, not merely
            // assumed): tmux replaces a DEAD pane's content — alternate
            // screen or history, it makes no difference — with its own
            // "Pane is dead" placeholder the instant the backing process
            // exits, so the ordinary prefill shows nothing useful in that
            // case either. And that state is exactly the one this feature
            // exists for: an app that ignores SIGTERM entirely, which
            // `kill_process_tree` escalates all the way to SIGKILL —
            // captured while alive and on the alternate screen (`capture_
            // alt_screen_before_stop`), then killed with no chance to
            // restore the primary screen on its own. Gating on
            // `!alternate_on` blanked exactly that case.
            //
            // When `modes.alternate_on` IS true for this dead pane,
            // `send_alt_screen_snapshot` itself sends `\x1b[?1049l`
            // (leave the alternate screen) before the divider — otherwise
            // the snapshot would land inside the scrollback-less
            // alternate buffer the mode-replay sequence above just
            // re-entered, burying its own top rows with nowhere for the
            // overflow to go. Leaving the alternate screen first moves
            // everything onto the primary screen instead, whose real
            // scrollback absorbs whatever does not fit visible — a
            // full-height frame's top rows scroll off screen, same as any
            // other output, but stay reachable by scrolling up rather
            // than being lost. This is the SAME accepted tradeoff a
            // pane that restored on its own already gets; it now also
            // covers the pane that never got the chance to restore
            // itself.
            if modes.pane_dead {
                send_alt_screen_snapshot(sup, &session_id, channel, tx, modes.alternate_on).await;
            }

            let fwd_tx = tx.clone();
            let task = tokio::spawn(async move {
                loop {
                    match stream.next_output().await {
                        Ok(Some(bytes)) => {
                            // Chunked like the replay above, and for a
                            // harsher reason: one bounded `%output`
                            // notification may still be larger than a
                            // protocol frame, and the encoder now rejects
                            // that rather than sending something the far
                            // side cannot decode. This is the last
                            // chunking boundary; input and replay already
                            // do the same.
                            let client_gone = bytes
                                .chunks(REPLAY_CHUNK)
                                .any(|c| fwd_tx.send(Frame::data(channel, c.to_vec())).is_err());
                            if client_gone {
                                break;
                            }
                        }
                        Ok(None) => {
                            let _ = fwd_tx.send(Frame::control(&ControlMsg::Detached {
                                channel,
                                reason: "session terminal ended".to_string(),
                            }));
                            break;
                        }
                        // Must notify too: swallowing this leaves the
                        // client with a terminal that silently stops
                        // updating while still accepting input, and no
                        // log line anywhere explaining why.
                        Err(e) => {
                            warn!(channel, error = %e, "output stream failed");
                            let _ = fwd_tx.send(Frame::control(&ControlMsg::Detached {
                                channel,
                                reason: format!("output stream failed: {e:#}"),
                            }));
                            break;
                        }
                    }
                }
                stream.shutdown().await;
            });

            attachments.insert(
                session_id.clone(),
                ActiveAttach {
                    channel,
                    notify: tx.clone(),
                    forwarder: task,
                    input,
                },
            );
            drop(attachments);
            input_routes.insert(channel, entry);
        }
        ControlMsg::Detach { channel } => {
            input_routes.remove(&channel);
            let mut attachments = sup.attachments.lock().await;
            let mine = attachments.iter().find_map(|(id, a)| {
                (a.channel == channel && a.notify.same_channel(tx)).then(|| id.clone())
            });
            if let Some(id) = mine
                && let Some(a) = attachments.remove(&id)
            {
                // Abort AND await, mirroring the takeover path: detach
                // followed by an immediate reattach (a browser reload is
                // exactly this) finds no incumbent to kick, so the only
                // thing keeping the old control-mode client from
                // overlapping the new one — the documented frozen-replay
                // hazard — is waiting for it here, before the lock is
                // released. Awaiting cannot deadlock: forwarders never
                // take this lock.
                a.forwarder.abort();
                let _ = a.forwarder.await;
            }
        }
        ControlMsg::Resize {
            session_id,
            channel,
            cols,
            rows,
        } => {
            // Same trust boundary as input, and the same two-part check
            // as the Data arm: `same_channel` identifies the owning
            // connection, and the channel id tells apart clients
            // multiplexed over ONE connection — every browser tab rides
            // the helm's single supervisor connection, so a
            // connection-level check alone would let a tab that just
            // lost a takeover reflow the winner's terminal.
            //
            // The session entry is fetched first so the two supervisor
            // locks are never held at once (see the struct docs); the
            // resize itself then runs UNDER the attachments lock, like
            // the Attach handler's tmux calls. Checking ownership and
            // then resizing after releasing the lock is a TOCTOU: a
            // takeover can interleave in that gap, and the kicked
            // client's already-authorized resize would land after the
            // winner's attach-time resize, reflowing the winner's
            // terminal with nothing to correct it.
            let entry = sup.sessions.lock().await.get(&session_id).cloned();
            if let Some(entry) = entry {
                let attachments = sup.attachments.lock().await;
                let owns = attachments
                    .get(&session_id)
                    .is_some_and(|a| a.channel == channel && a.notify.same_channel(tx));
                if owns {
                    // `owns` being true is only possible if a terminal
                    // exists: the Attach handler never registers an
                    // attachment for a terminal-less entry (see its
                    // restart-gap check), so an owned attachment with no
                    // terminal here means that invariant broke elsewhere —
                    // worth failing loudly over, not papering past.
                    let terminal = entry.terminal.as_ref().expect(
                        "attachments are only ever registered for entries with a terminal — \
                         see the Attach handler",
                    );
                    // Fire-and-forget: a resize has no req_id to answer,
                    // and a tmux failure here must not take the
                    // connection (and every other session on it) down.
                    if let Err(e) = sup
                        .tmux
                        .resize_window(&terminal.tmux_name, cols, rows)
                        .await
                    {
                        warn!(session = %session_id, error = %e, "resize failed");
                    }
                }
            }
        }
        ControlMsg::Hello { .. } => {
            // A second hello is a protocol violation; ignore rather than
            // kill the connection over it.
        }
        // Response/event messages arriving at the supervisor are peer
        // bugs; log and continue.
        other => warn!(?other, "unexpected control message at supervisor"),
    }
}

/// `farhelm supervisor run` in one call: build a supervisor on `state_dir`
/// and serve its socket until the process dies. Returns only on a fatal
/// error — a successful supervisor never returns.
pub async fn run(state_dir: &Path) -> anyhow::Result<()> {
    let sup = Supervisor::new(state_dir).await?;
    sup.serve().await
}

/// Connect to a running supervisor's socket (used by `internal stdio`).
/// Fails if no supervisor is listening; this deliberately does not start
/// one, because discovery-first (SPEC.md) means an absent supervisor is
/// the caller's decision to make, not a side effect of dialing.
pub async fn connect(state_dir: &Path) -> anyhow::Result<UnixStream> {
    let path = Supervisor::socket_path(state_dir);
    UnixStream::connect(&path)
        .await
        .with_context(|| format!("connecting to supervisor socket {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The common case: an id well under the cap is echoed verbatim, with
    /// no allocation (`Cow::Borrowed`) and no trailing ellipsis.
    #[test]
    fn truncate_for_error_passes_short_ids_through_unchanged() {
        let id = "s1";
        assert_eq!(truncate_for_error(id), id);
        assert!(matches!(
            truncate_for_error(id),
            std::borrow::Cow::Borrowed(_)
        ));
    }

    /// An id at EXACTLY the cap must not be truncated — the boundary
    /// condition `<=` in `truncate_for_error` exists to get right, since
    /// an off-by-one here would either truncate a fitting id or let one
    /// byte over the cap through unclipped.
    #[test]
    fn truncate_for_error_leaves_an_id_at_exactly_the_cap_untouched() {
        let id = "a".repeat(ECHOED_ID_MAX);
        assert_eq!(truncate_for_error(&id), id.as_str());
    }

    /// One byte over the cap must be truncated and marked with `...` —
    /// the case the cap exists for at all.
    #[test]
    fn truncate_for_error_truncates_one_byte_over_the_cap() {
        let id = "a".repeat(ECHOED_ID_MAX + 1);
        let truncated = truncate_for_error(&id);
        assert_eq!(truncated.len(), ECHOED_ID_MAX + "...".len());
        assert!(truncated.starts_with(&"a".repeat(ECHOED_ID_MAX)));
        assert!(truncated.ends_with("..."));
    }

    /// A multi-byte UTF-8 character straddling the cap boundary must not
    /// be split. Naively slicing at the byte offset `ECHOED_ID_MAX` here
    /// would land INSIDE a 4-byte emoji (which starts one byte before the
    /// cap) and panic — Rust's own `&str` indexing refuses to produce
    /// invalid UTF-8, so a truncation routine that didn't search for a
    /// real char boundary would crash the whole request rather than
    /// truncate it. The correct behavior is to back up to the boundary
    /// just before the emoji, dropping it whole rather than splitting it.
    #[test]
    fn truncate_for_error_does_not_split_a_multibyte_char_at_the_boundary() {
        let mut id = "a".repeat(ECHOED_ID_MAX - 1);
        id.push('🎉'); // 4 bytes, starting one byte before the cap
        id.push_str("tail");

        let truncated = truncate_for_error(&id); // must not panic

        assert_eq!(
            truncated,
            format!("{}...", "a".repeat(ECHOED_ID_MAX - 1)),
            "truncation must back up to the boundary before the emoji, dropping it whole"
        );
    }

    /// The environment-marker matcher's whole job: match a complete,
    /// exact `FARHELM_SESSION_ID=<id>` entry and nothing looser. A
    /// substring match would misfire on a session whose id is a prefix of
    /// another's, or on an unrelated variable that happens to embed the
    /// marker text — either would make `kill_process_tree` reap or spare
    /// the wrong session's processes. Constructed byte buffers, not a
    /// real process, since `environ_contains_marker` is factored exactly
    /// so this needs neither.
    #[test]
    fn environ_marker_matches_exact_entries_only() {
        let session_id = "abc-123";

        // Exact match, alone or alongside unrelated entries.
        assert!(environ_contains_marker(
            b"FARHELM_SESSION_ID=abc-123\0",
            session_id
        ));
        let mut buf = b"PATH=/bin\0".to_vec();
        buf.extend_from_slice(b"FARHELM_SESSION_ID=abc-123\0");
        buf.extend_from_slice(b"HOME=/root\0");
        assert!(environ_contains_marker(&buf, session_id));

        // A value merely PREFIXED by the id ("abc-123" is a proper prefix
        // of "abc-1234", a DIFFERENT session) must not match.
        assert!(!environ_contains_marker(
            b"FARHELM_SESSION_ID=abc-1234\0",
            session_id
        ));
        // A different variable that merely embeds the marker text must
        // not match — only a complete NUL-delimited entry counts.
        assert!(!environ_contains_marker(
            b"OTHER_VAR=FARHELM_SESSION_ID=abc-123\0",
            session_id
        ));
        assert!(!environ_contains_marker(
            b"FARHELM_SESSION_ID_ALT=abc-123\0",
            session_id
        ));
        // No marker at all.
        assert!(!environ_contains_marker(
            b"PATH=/bin\0HOME=/root\0",
            session_id
        ));
    }

    /// `parse_stat`'s whole reason to exist: a `comm` field containing
    /// spaces AND a stray closing paren must not fool the last-`)` search
    /// into stopping early. This pins the kernel's actual escape hatch —
    /// `comm` can contain anything, including `)`, so only the LAST `)`
    /// in the whole line is the real delimiter, no matter how many
    /// look-alikes precede it.
    #[test]
    fn parse_stat_handles_comm_with_parens_and_spaces() {
        // comm = "1 (weird) name)" — spaces, an internal paren pair, AND
        // a trailing stray ')' that is NOT the kernel's own delimiter.
        // Wrapped by the kernel in its own parens, the line's tail reads
        // "...name))" — two closing parens back to back — and only the
        // second is real.
        let line: &[u8] = b"123 (1 (weird) name)) S 456 1 1 0 -1 0 0 0 0 0 0 0 0 0 20 0 1 0 789";
        let (ppid, starttime, state) = parse_stat(line).expect("well-formed synthetic stat line");
        assert_eq!(ppid, 456, "ppid must be read from AFTER the true delimiter");
        assert_eq!(starttime, 789, "starttime is the 20th field after comm");
        assert_eq!(state, 'S', "state is the first field after comm");
    }

    /// `comm` is whatever bytes the process named itself with — it can be
    /// genuinely non-UTF-8 — and `parse_stat` must not choke on that, since
    /// only the LAST `)` is located via a raw byte search and everything
    /// before it (the non-UTF-8 comm included) is never decoded at all.
    /// Failing this would misreport a live, oddly-named process as
    /// unparseable, folding it into a reported sweep error over nothing
    /// more than a name it never chose to be `/proc`-friendly about.
    #[test]
    fn parse_stat_survives_non_utf8_bytes_in_comm() {
        let mut line = b"123 (bad".to_vec();
        line.push(0xff); // not valid UTF-8 on its own or in context here
        line.extend_from_slice(b"name) Z 456 1 1 0 -1 0 0 0 0 0 0 0 0 0 20 0 1 0 789");
        let (ppid, starttime, state) =
            parse_stat(&line).expect("a non-UTF-8 comm must not fail parsing");
        assert_eq!(ppid, 456);
        assert_eq!(starttime, 789);
        assert_eq!(state, 'Z');
    }

    /// A stat line with no `)` at all (never happens for a real kernel-
    /// written row, but a corrupted read or a hostile fixture could
    /// produce one) must be a reported parse error, not a silent "gone" —
    /// conflating "malformed" with "absent" would let a genuinely live,
    /// misread process vanish from a sweep without a trace.
    #[test]
    fn parse_stat_rejects_a_line_with_no_delimiter() {
        assert!(parse_stat(b"garbage with no parens at all").is_err());
    }

    /// `signal_validated`'s entire reason to exist: a pid whose CURRENT
    /// `/proc` start-time does not match what was recorded earlier must
    /// be left alone, even under a signal as unblockable as `SIGKILL`.
    /// Uses a REAL child process (there is no way to fabricate a `/proc`
    /// entry) and a starttime that cannot possibly be correct, then
    /// confirms the child is still alive before cleaning it up for real.
    #[test]
    fn signal_validated_skips_a_pid_whose_starttime_does_not_match() {
        let mut child = std::process::Command::new("sleep")
            .arg("5")
            .spawn()
            .expect("spawn a real child to validate against");
        let pid = child.id();
        let (_, real_starttime, _state) = read_stat(pid)
            .expect("reading a just-spawned child's stat must not error")
            .expect("a just-spawned child must have a readable stat row");

        // Any starttime other than the real one proves the point; adding
        // a large offset makes collision with the real value effectively
        // impossible without needing to reason about clock/jiffy units.
        let bogus_starttime = real_starttime.wrapping_add(999_999_999);
        signal_validated(pid, bogus_starttime, libc::SIGKILL)
            .expect("a starttime mismatch is a skip, not an error");

        std::thread::sleep(Duration::from_millis(200));
        assert!(
            matches!(child.try_wait(), Ok(None)),
            "child must survive a signal validated against the wrong starttime"
        );

        // Clean up for real, with the correct identity this time —
        // otherwise this test leaks a `sleep 5` process.
        signal_validated(pid, real_starttime, libc::SIGKILL)
            .expect("signaling with the correct starttime must succeed");
        let _ = child.wait();
    }

    /// The defusal this whole change exists for: an oversized reply must
    /// never reach the writer task, because the writer's fatal-on-any-
    /// write-error handling would tear down the shared connection (and
    /// every attachment on it) over what should have been a single
    /// request's failure. Assert the substitute frame decodes as an
    /// `Error` correlated to the same `req_id` the caller was waiting on,
    /// and that its message actually names the problem (both the size
    /// figure and the fact that it exceeds the limit) rather than being an
    /// opaque placeholder string.
    ///
    /// `ListSessions` is the one M1 reply built entirely from unbounded
    /// caller-controlled data (session titles), uncapped until M2's list
    /// budget — so it is the realistic way a control reply exceeds
    /// `MAX_FRAME_LEN`. One oversized title is enough to clear the cap
    /// (JSON escaping plus the frame header add overhead on top of the
    /// title itself), so a single `SessionInfo` suffices here.
    #[test]
    fn reply_frame_substitutes_error_for_oversized_reply() {
        let req_id = 42;
        let oversized = ControlMsg::SessionList {
            req_id,
            sessions: vec![SessionInfo {
                id: "s1".to_string(),
                title: "x".repeat(farhelm_proto::MAX_FRAME_LEN as usize),
                cwd: "/tmp".to_string(),
                invocation: "agent".to_string(),
                status: SessionStatus::Alive,
            }],
            total: 1,
            truncated: false,
        };
        assert!(
            Frame::control(&oversized).exceeds_max_len(),
            "test fixture must actually exceed MAX_FRAME_LEN"
        );

        let frame = reply_frame(&oversized);
        assert!(!frame.exceeds_max_len(), "substituted reply must fit");
        let decoded: ControlMsg = serde_json::from_slice(&frame.body).unwrap();
        let ControlMsg::Error {
            req_id: got_req_id,
            message,
            kind,
        } = decoded
        else {
            panic!("expected ControlMsg::Error, got {decoded:?}");
        };
        assert_eq!(got_req_id, req_id);
        assert!(
            message.contains(&farhelm_proto::MAX_FRAME_LEN.to_string()),
            "error message must name the limit that was exceeded: {message}"
        );
        assert!(
            message.contains("exceeding"),
            "error message must describe the problem concretely: {message}"
        );
        assert_eq!(
            kind,
            ErrorKind::Internal,
            "the reply was too big for the wire, not something the caller's request got wrong"
        );
    }

    /// The common case: a reply that fits comes back byte-identical to
    /// what `Frame::control` alone would produce. `reply_frame` always
    /// serializes and size-checks the message — there is no cheaper path
    /// that skips that for the common case — but the *result* for a
    /// fitting reply must be indistinguishable from the unwrapped frame.
    #[test]
    fn reply_frame_passes_through_normal_reply_unchanged() {
        let msg = ControlMsg::SessionCreated {
            req_id: 7,
            session: SessionInfo {
                id: "s1".to_string(),
                title: "demo".to_string(),
                cwd: "/tmp".to_string(),
                invocation: "agent".to_string(),
                // Matches real `create_session` output: `Unknown`, not
                // `Alive` (see that function's own doc comment).
                status: SessionStatus::Unknown,
            },
        };
        assert_eq!(reply_frame(&msg), Frame::control(&msg));
    }

    /// `reply_frame` panics on a message with no `req_id` to correlate an
    /// oversize substitution against. This is pinning an explicit
    /// invariant, not documenting an accident: a silent fallback here
    /// (inventing a `req_id`, or sending the oversized reply unchecked)
    /// would recreate the exact oversize-reaches-the-writer bug this
    /// function exists to close, just for a different message shape. If a
    /// future caller ever routes an event message (like `Detached`)
    /// through `reply_frame`, this test is what catches it.
    #[test]
    #[should_panic(expected = "carries no req_id")]
    fn reply_frame_panics_on_message_without_req_id() {
        reply_frame(&ControlMsg::Detached {
            channel: 1,
            reason: "x".into(),
        });
    }

    /// A dummy launch-shim path for tests that never create a session:
    /// `Supervisor::new_with_exe` never touches this path itself (only
    /// `create_session` does, via `window_command`), so a nonexistent
    /// file is fine wherever a test's request is rejected — by the
    /// `CREATE_FIELD_CAP` guard, say — before any side effect happens.
    fn dummy_exe() -> PathBuf {
        PathBuf::from("/nonexistent/farhelm")
    }

    /// The pre-side-effect half of the `CREATE_FIELD_CAP` guard: a request
    /// whose fields already exceed the cap must be rejected — with an
    /// `Error` reply correlated to its `req_id` and naming the cap — before
    /// `create_session` ever runs, so no session is left behind for a
    /// caller who was told the request failed. Drives the real
    /// `handle_control` dispatcher (not just the cap arithmetic in
    /// isolation) against a real `Supervisor`, since the invariant this
    /// protects is about what happens at the call site.
    #[tokio::test]
    async fn create_session_over_field_cap_is_rejected_before_any_side_effect() {
        let state = tempfile::tempdir().expect("state dir");
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        let req_id = 99;

        handle_control(
            &sup,
            ControlMsg::CreateSession {
                req_id,
                cwd: "x".repeat(CREATE_FIELD_CAP),
                invocation: "agent".to_string(),
                title: None,
                cols: 80,
                rows: 24,
            },
            &tx,
            &mut input_routes,
            &mut tasks,
        )
        .await;

        let reply = rx.try_recv().expect("a reply must have been sent");
        let decoded: ControlMsg = serde_json::from_slice(&reply.body).unwrap();
        let ControlMsg::Error {
            req_id: got_req_id,
            message,
            kind,
        } = decoded
        else {
            panic!("expected ControlMsg::Error, got {decoded:?}");
        };
        assert_eq!(got_req_id, req_id);
        assert!(
            message.contains(&CREATE_FIELD_CAP.to_string()),
            "error message must name the limit that was exceeded: {message}"
        );
        assert_eq!(
            kind,
            ErrorKind::InvalidRequest,
            "an oversized request is the caller's mistake, not a server fault"
        );
        assert!(
            sup.sessions.lock().await.is_empty(),
            "a rejected request must create nothing"
        );
    }

    /// Call-site regression: drives `handle_control` itself, not
    /// `build_list_reply`/`reply_frame` in isolation. Before M2's list
    /// cap and byte budget existed, an oversized `ListSessions` reply
    /// could only be caught by `reply_frame`'s backstop degrading it to
    /// an `Error` — that scenario is still pinned directly against
    /// `reply_frame` by `reply_frame_substitutes_error_for_oversized_reply`
    /// above. Once `build_list_reply` sits in front of it at this call
    /// site, though, the SAME oversized fixture (a single session whose
    /// title alone exceeds `MAX_FRAME_LEN`) never reaches `reply_frame` in
    /// an oversized state at all: the byte budget already drops it from
    /// the reply, honestly reporting `total: 1, truncated: true` with an
    /// empty `sessions` list — a normal, well-formed answer, not the
    /// `Error` substitution. This test pins THAT outcome, so a future
    /// change that quietly dropped `build_list_reply` from this call site
    /// (reverting to plain, uncapped `Frame::control`) would pass every
    /// other test in this module — they all call `reply_frame` or
    /// `build_list_reply` directly — and only this test would catch it. It
    /// also proves the degrade is per-request: a second, ordinary request
    /// on the same connection (same `tx`) must still get an honest,
    /// untruncated reply.
    #[tokio::test]
    async fn list_sessions_call_site_applies_the_byte_budget_and_keeps_serving() {
        let state = tempfile::tempdir().expect("state dir");
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");

        // Populate the session map directly with fake data, sidestepping
        // tmux/launch entirely: only the size-driven reply behavior at
        // the ListSessions call site is under test here.
        sup.sessions.lock().await.insert(
            "s1".to_string(),
            Arc::new(SessionEntry {
                info: SessionInfo {
                    id: "s1".to_string(),
                    title: "x".repeat(farhelm_proto::MAX_FRAME_LEN as usize),
                    cwd: "/tmp".to_string(),
                    invocation: "agent".to_string(),
                    status: SessionStatus::default(),
                },
                terminal: Some(Terminal {
                    tmux_name: "fh-fake".to_string(),
                    pane: "%0".to_string(),
                }),
            }),
        );

        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();

        handle_control(
            &sup,
            ControlMsg::ListSessions { req_id: 1 },
            &tx,
            &mut input_routes,
            &mut tasks,
        )
        .await;
        // `ListSessions` is now spawned onto its own task (see that arm's
        // own comment on why), so `handle_control`'s `.await` above only
        // proves the request was ACCEPTED, not that the reply has been
        // sent yet — an immediate `try_recv` would be a race against the
        // spawned task's own tmux round trip. `recv().await`, bounded by
        // a timeout so a genuine regression fails fast instead of hanging
        // the test suite, is what actually waits for it.
        let reply = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("spawned ListSessions handler never replied")
            .expect("reply channel closed before a reply arrived");
        let decoded: ControlMsg = serde_json::from_slice(&reply.body).unwrap();
        let ControlMsg::SessionList {
            req_id,
            sessions,
            total,
            truncated,
        } = decoded
        else {
            panic!("expected a budget-truncated ControlMsg::SessionList, got {decoded:?}");
        };
        assert_eq!(req_id, 1);
        assert!(
            sessions.is_empty(),
            "the one oversized session must be dropped by the byte budget"
        );
        assert_eq!(
            total, 1,
            "total is the count BEFORE the budget's truncation"
        );
        assert!(truncated);

        // Clear the oversized fixture and send a normal request through
        // the SAME tx: a healthy reply here is what proves the earlier
        // substitution was scoped to its one request. Clearing only
        // AFTER the first reply was actually received (not merely after
        // the request was accepted) keeps this ordering deliberate rather
        // than racing the first spawned task's still-in-flight tmux
        // query.
        sup.sessions.lock().await.clear();
        handle_control(
            &sup,
            ControlMsg::ListSessions { req_id: 2 },
            &tx,
            &mut input_routes,
            &mut tasks,
        )
        .await;
        let reply2 = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("spawned ListSessions handler never replied a second time")
            .expect("reply channel closed before a second reply arrived");
        let decoded2: ControlMsg = serde_json::from_slice(&reply2.body).unwrap();
        let ControlMsg::SessionList {
            req_id,
            sessions,
            total,
            truncated,
        } = decoded2
        else {
            panic!("expected a normal ControlMsg::SessionList, got {decoded2:?}");
        };
        assert_eq!(req_id, 2);
        assert!(sessions.is_empty());
        assert_eq!(total, 0);
        assert!(!truncated);
    }

    /// Production call-site coverage for `LIST_SESSION_CAP` itself — the
    /// cheapest honest way to exercise the REAL wiring (`handle_control`'s
    /// `ListSessions` arm applying `.take(LIST_SESSION_CAP)` before ever
    /// cloning or status-annotating an entry) rather than only
    /// `build_list_reply`'s own pure-function tests, which never touch the
    /// handler at all. Creating `LIST_SESSION_CAP + 1` REAL tmux sessions
    /// to exercise this would be slow and environment-dependent for no
    /// added signal; every entry here is synthetic and terminal-less
    /// (`terminal: None`), which is enough to drive the cap/total/
    /// truncated wiring without needing a single real tmux round trip to
    /// succeed for any of them (`session_status` returns `Exited` for a
    /// terminal-less entry without ever consulting `pane_states`).
    #[tokio::test]
    async fn list_sessions_honors_the_session_cap_at_the_handler_level() {
        let state = tempfile::tempdir().expect("state dir");
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");

        {
            let mut sessions = sup.sessions.lock().await;
            for i in 0..LIST_SESSION_CAP + 1 {
                let id = format!("s{i}");
                sessions.insert(
                    id.clone(),
                    Arc::new(SessionEntry {
                        info: SessionInfo {
                            id,
                            title: "t".to_string(),
                            cwd: "/tmp".to_string(),
                            invocation: "agent".to_string(),
                            status: SessionStatus::default(),
                        },
                        terminal: None,
                    }),
                );
            }
        }

        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        handle_control(
            &sup,
            ControlMsg::ListSessions { req_id: 1 },
            &tx,
            &mut input_routes,
            &mut tasks,
        )
        .await;
        let reply = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("spawned ListSessions handler never replied")
            .expect("reply channel closed before a reply arrived");
        let decoded: ControlMsg = serde_json::from_slice(&reply.body).unwrap();
        let ControlMsg::SessionList {
            sessions,
            total,
            truncated,
            ..
        } = decoded
        else {
            panic!("expected ControlMsg::SessionList, got {decoded:?}");
        };
        assert_eq!(
            sessions.len(),
            LIST_SESSION_CAP,
            "the cap must win over the full count at the real handler call site"
        );
        assert_eq!(
            total,
            (LIST_SESSION_CAP + 1) as u64,
            "total is the count BEFORE the cap"
        );
        assert!(truncated);
    }

    /// PLAN_M2.md's list-status contract: a `ListSessions` reply whose
    /// (capped) subset contains NO entry with a terminal at all —
    /// including the empty-list case, but exercised here with one
    /// terminal-less entry so the reply is checked for real content too —
    /// must succeed even if tmux itself is completely unreachable, because
    /// those statuses are decidable without asking tmux anything
    /// (`session_status` returns `Exited` for a terminal-less entry
    /// unconditionally). Proven by actually killing the supervisor's own
    /// private tmux server (bypassing the supervisor entirely) rather than
    /// just supplying terminal-less fixtures against a healthy one — if
    /// `ListSessions` asked tmux anything here, this test would see an
    /// `Error` reply instead of the expected `SessionList`.
    #[tokio::test]
    async fn list_sessions_skips_pane_states_when_nothing_has_a_terminal() {
        let state = tempfile::tempdir().expect("state dir");
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");

        let sock = state.path().join("tmux.sock");
        let killed = std::process::Command::new("tmux")
            .arg("-S")
            .arg(&sock)
            .arg("kill-server")
            .output()
            .expect("run tmux kill-server");
        assert!(
            killed.status.success(),
            "test setup: tmux kill-server must succeed, got: {}",
            String::from_utf8_lossy(&killed.stderr)
        );

        sup.sessions.lock().await.insert(
            "s1".to_string(),
            Arc::new(SessionEntry {
                info: SessionInfo {
                    id: "s1".to_string(),
                    title: "t".to_string(),
                    cwd: "/tmp".to_string(),
                    invocation: "agent".to_string(),
                    status: SessionStatus::default(),
                },
                terminal: None,
            }),
        );

        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        handle_control(
            &sup,
            ControlMsg::ListSessions { req_id: 1 },
            &tx,
            &mut input_routes,
            &mut tasks,
        )
        .await;
        let reply = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("spawned ListSessions handler never replied")
            .expect("reply channel closed before a reply arrived");
        let decoded: ControlMsg = serde_json::from_slice(&reply.body).unwrap();
        let ControlMsg::SessionList { sessions, .. } = decoded else {
            panic!(
                "expected ControlMsg::SessionList (tmux must not have been consulted at all), \
                 got {decoded:?}"
            );
        };
        assert_eq!(sessions.len(), 1);
        assert_eq!(
            sessions[0].status,
            SessionStatus::Exited { exit_code: None }
        );
    }

    /// Steady-state contract for `reap_finished_tasks` (its own docs): a
    /// connection that stays open and keeps issuing requests must not
    /// accumulate one unreaped `JoinSet` entry per request. Drives many
    /// `ListSessions` requests through the REAL `handle_control` dispatch
    /// — the same spawn/admission path production uses, not a synthetic
    /// `JoinSet` fixture — waiting for each reply before reaping
    /// (mirroring one iteration of `handle_connection`'s read loop: read,
    /// dispatch, reap) and asserting the tracked task count returns to
    /// ZERO every time, rather than growing with the number of requests
    /// issued so far.
    #[tokio::test]
    async fn steady_state_reaping_keeps_the_tracked_task_count_bounded() {
        let state = tempfile::tempdir().expect("state dir");
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();

        const REQUESTS: u64 = 50;
        for req_id in 0..REQUESTS {
            handle_control(
                &sup,
                ControlMsg::ListSessions { req_id },
                &tx,
                &mut input_routes,
                &mut tasks,
            )
            .await;
            let reply = tokio::time::timeout(Duration::from_secs(5), rx.recv())
                .await
                .expect("spawned ListSessions handler never replied")
                .expect("reply channel closed before a reply arrived");
            let decoded: ControlMsg = serde_json::from_slice(&reply.body).unwrap();
            assert!(
                matches!(decoded, ControlMsg::SessionList { .. }),
                "expected ControlMsg::SessionList, got {decoded:?}"
            );

            reap_finished_tasks(&mut tasks);
            assert_eq!(
                tasks.len(),
                0,
                "the tracked task count must return to zero after reaping (request {req_id} \
                 of {REQUESTS}), not grow with the number of requests issued so far"
            );
        }
    }

    /// A minimal, distinct `SessionInfo` for `build_list_reply`'s own
    /// tests — distinct ids so a truncation bug that drops the wrong
    /// entries (rather than merely the wrong COUNT) would still be
    /// caught.
    fn fake_session(id: &str, title_len: usize) -> SessionInfo {
        SessionInfo {
            id: id.to_string(),
            title: "x".repeat(title_len),
            cwd: "/tmp".to_string(),
            invocation: "agent".to_string(),
            status: SessionStatus::Alive,
        }
    }

    /// The common case: everything fits under the byte budget, so nothing
    /// is dropped and `truncated` is honestly `false`. `total` is passed
    /// explicitly here (as the real `ListSessions` call site does — see
    /// that arm's own comment) rather than derived from `sessions.len()`,
    /// since `build_list_reply` no longer owns count-cap enforcement (the
    /// caller applies it before this is ever reached; the handler-level
    /// cap wiring itself is pinned by
    /// `list_sessions_honors_the_session_cap_at_the_handler_level` below).
    #[test]
    fn build_list_reply_keeps_everything_under_the_byte_budget() {
        let sessions: Vec<SessionInfo> = (0..10).map(|i| fake_session(&i.to_string(), 4)).collect();
        let reply = build_list_reply(1, sessions, 10, LIST_BYTE_BUDGET);
        let ControlMsg::SessionList {
            req_id,
            sessions,
            total,
            truncated,
        } = reply
        else {
            panic!("expected ControlMsg::SessionList, got {reply:?}");
        };
        assert_eq!(req_id, 1);
        assert_eq!(sessions.len(), 10);
        assert_eq!(total, 10);
        assert!(!truncated);
    }

    /// The byte-budget's whole job: a count well under any cap can still
    /// overflow a small budget if the records themselves are fat, and the
    /// reply must keep dropping from the tail until it fits.
    #[test]
    fn build_list_reply_enforces_the_byte_budget_independent_of_count() {
        let sessions: Vec<SessionInfo> =
            (0..5).map(|i| fake_session(&i.to_string(), 200)).collect();
        let reply = build_list_reply(1, sessions, 5, 400);
        let ControlMsg::SessionList {
            sessions,
            total,
            truncated,
            ..
        } = reply
        else {
            panic!("expected ControlMsg::SessionList, got {reply:?}");
        };
        assert_eq!(total, 5);
        assert!(
            sessions.len() < 5,
            "fat records must be dropped even though the count never reached any cap"
        );
        assert!(truncated);
        assert!(
            Frame::control(&ControlMsg::SessionList {
                req_id: 1,
                sessions,
                total,
                truncated,
            })
            .encoded_len()
                <= 400,
            "the kept reply must actually respect the byte budget"
        );
    }

    /// Exact-prefix pin for the single-pass accounting itself: a budget
    /// derived from a REAL encoded reply (via `Frame::control`, not by
    /// repeating `build_list_reply`'s own per-entry/envelope arithmetic —
    /// an independent measurement, not a restatement of the same math a
    /// bug in that arithmetic could just as easily share) for EXACTLY `K`
    /// entries must keep exactly those `K` and drop the rest.
    ///
    /// Derived with `truncated: false`, even though the real answer for
    /// `K < total` is `true`: `build_list_reply`'s own envelope accounting
    /// is conservatively based on the (longer) `false` shape throughout
    /// the whole scan (see its doc comment on the envelope-length flip),
    /// so a budget sized to a `true`-shaped K-entry reply is one byte too
    /// tight for the algorithm to actually keep entry K — empirically,
    /// it keeps only `K-1` (an earlier version of this test used `true`
    /// and had exactly that failure). Budgeting against the `false`-shaped
    /// size matches the conservative basis the algorithm itself commits
    /// to and is the honest boundary this test can pin.
    #[test]
    fn build_list_reply_keeps_exactly_the_entries_a_derived_budget_fits() {
        let sessions: Vec<SessionInfo> = (0..5).map(|i| fake_session(&i.to_string(), 20)).collect();
        let total = sessions.len() as u64;
        const K: usize = 3;

        let k_reply = ControlMsg::SessionList {
            req_id: 1,
            sessions: sessions[..K].to_vec(),
            total,
            truncated: false,
        };
        let budget = Frame::control(&k_reply).encoded_len();

        // Sanity: one more entry must genuinely exceed this budget, or
        // the test would not be pinning a real boundary.
        let k_plus_one_reply = ControlMsg::SessionList {
            req_id: 1,
            sessions: sessions[..K + 1].to_vec(),
            total,
            truncated: true,
        };
        assert!(
            Frame::control(&k_plus_one_reply).encoded_len() > budget,
            "test fixture must actually grow past the derived budget with one more entry"
        );

        let reply = build_list_reply(1, sessions.clone(), total, budget);
        let ControlMsg::SessionList {
            sessions: kept,
            truncated,
            ..
        } = reply
        else {
            panic!("expected ControlMsg::SessionList, got {reply:?}");
        };
        assert_eq!(
            kept,
            sessions[..K],
            "a budget derived from a real K-entry reply must keep exactly those K"
        );
        assert!(truncated);
    }

    /// The envelope-flip boundary itself: a budget sized to fit ALL
    /// sessions EXACTLY (again derived from a real, untruncated reply's
    /// own encoded size via `Frame::control`) must keep every one of them
    /// with `truncated: false` — not silently drop the last one. This is
    /// the scenario the envelope-flip fix (measuring the envelope with
    /// `truncated: false`, the LONGER of the two JSON booleans — `"false"`
    /// is 5 ASCII characters, `"true"` is 4) exists for: getting that flip
    /// backwards would under-count the envelope by exactly one byte for
    /// this untruncated case, tripping `build_list_reply`'s own
    /// `debug_assert!` in tests and, in a release build, risking a reply
    /// that lands one byte over `byte_budget` for the one case that was
    /// supposed to fit with room to spare.
    #[test]
    fn build_list_reply_keeps_everything_at_an_exact_untruncated_boundary() {
        let sessions: Vec<SessionInfo> = (0..5).map(|i| fake_session(&i.to_string(), 20)).collect();
        let total = sessions.len() as u64;

        let full_reply = ControlMsg::SessionList {
            req_id: 1,
            sessions: sessions.clone(),
            total,
            truncated: false,
        };
        let budget = Frame::control(&full_reply).encoded_len();

        let reply = build_list_reply(1, sessions.clone(), total, budget);
        let ControlMsg::SessionList {
            sessions: kept,
            truncated,
            ..
        } = reply
        else {
            panic!("expected ControlMsg::SessionList, got {reply:?}");
        };
        assert_eq!(
            kept, sessions,
            "an exact-fit budget must not drop the last entry"
        );
        assert!(!truncated);
    }

    /// The degenerate case for the single-pass entry scan: an empty
    /// `sessions` vec simply never enters the `for` loop at all, so this
    /// pins that the empty case still produces a well-formed reply —
    /// `total: 0`, `truncated: false` — through the ordinary path, not a
    /// special case that could drift from it.
    #[test]
    fn build_list_reply_handles_zero_sessions() {
        let reply = build_list_reply(1, Vec::new(), 0, LIST_BYTE_BUDGET);
        let ControlMsg::SessionList {
            sessions,
            total,
            truncated,
            ..
        } = reply
        else {
            panic!("expected ControlMsg::SessionList, got {reply:?}");
        };
        assert!(sessions.is_empty());
        assert_eq!(total, 0);
        assert!(!truncated);
    }

    /// `spawn_admitted`'s entire contract (see its own docs): the permit
    /// is acquired in the CALLER's own await, before the task is ever
    /// spawned — not inside the spawned future. Proven with a 2-permit
    /// semaphore and manually-controlled tasks (a `Notify`, not real
    /// timing) rather than routing through real `StopSession`/tmux, which
    /// would make "has the Nth task started running yet" unobservable
    /// without racing real kill-sweep durations — exactly the flakiness
    /// this test is designed to avoid.
    ///
    /// A regression that moved `acquire_owned().await` to INSIDE the
    /// spawned task (permit acquired AFTER `tasks.spawn`, not before)
    /// would make the third `spawn_admitted` call below return
    /// immediately regardless of how many permits are free, since nothing
    /// would then block spawning it — the bounded-timeout assertion in
    /// the middle of this test is exactly what catches that.
    #[tokio::test]
    async fn spawn_admitted_acquires_the_permit_before_spawning_not_inside_the_task() {
        let admission = Arc::new(tokio::sync::Semaphore::new(2));
        let mut tasks = tokio::task::JoinSet::new();
        let release = Arc::new(tokio::sync::Notify::new());
        let started = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        // Claim both permits with two tasks that run and then block on
        // `release`, so they stay "in flight" (holding their permits)
        // until this test lets them go.
        for _ in 0..2 {
            let started = Arc::clone(&started);
            let release = Arc::clone(&release);
            spawn_admitted(&admission, &mut tasks, async move {
                started.fetch_add(1, Ordering::SeqCst);
                release.notified().await;
            })
            .await;
        }
        // `tasks.spawn` only SCHEDULES the task; it does not run until
        // this task yields to the executor. Neither does
        // `spawn_admitted`'s own `acquire_owned().await` necessarily
        // yield — an uncontended semaphore can resolve without ever
        // suspending. `yield_now` is what actually lets the two spawned
        // tasks run up to their own first await point (`notified()`).
        tokio::task::yield_now().await;
        assert_eq!(
            started.load(Ordering::SeqCst),
            2,
            "both permits must have been claimed by two running tasks"
        );

        // A third admission call, with both permits still held, must not
        // resolve at all yet. Scoped in its own block: `tokio::pin!`'s
        // hidden storage borrows `tasks` for the rest of ITS enclosing
        // scope regardless of when the `Pin<&mut _>` handle itself is
        // dropped, so the block boundary — not a manual `drop` — is what
        // releases that borrow before `tasks` is touched again below.
        {
            let started3 = Arc::clone(&started);
            let release3 = Arc::clone(&release);
            let third = spawn_admitted(&admission, &mut tasks, async move {
                started3.fetch_add(1, Ordering::SeqCst);
                release3.notified().await;
            });
            tokio::pin!(third);
            assert!(
                tokio::time::timeout(Duration::from_millis(50), &mut third)
                    .await
                    .is_err(),
                "spawn_admitted must block acquiring a permit while both are held, not spawn \
                 (and run) immediately"
            );
            assert_eq!(
                started.load(Ordering::SeqCst),
                2,
                "the third task must not have started running while its spawn_admitted call \
                 is still blocked on admission"
            );

            // Free exactly one permit: wakes one of the first two tasks,
            // which finishes and drops its permit, which is what lets the
            // third admission proceed.
            release.notify_one();
            tokio::time::timeout(Duration::from_secs(5), &mut third)
                .await
                .expect("spawn_admitted must proceed once a permit frees");
            // `third` resolving only proves the permit was acquired and
            // the task was handed to `tasks.spawn` — not that the newly
            // spawned task has been polled yet (the same
            // spawn-schedules-but-does-not-run distinction as the first
            // `yield_now` above).
            tokio::task::yield_now().await;
            assert_eq!(started.load(Ordering::SeqCst), 3);
        }

        // Let every remaining task finish and reap them all.
        release.notify_waiters();
        while tasks.join_next().await.is_some() {}
    }

    use std::sync::atomic::AtomicBool;

    /// A guard whose `Drop` flips a sentinel, used below to distinguish a
    /// task that was genuinely aborted-and-polled-to-completion from one
    /// merely abandoned. A tokio task's future is dropped only once
    /// cancellation has actually been delivered and polled, so observing
    /// the sentinel is proof of that, not just proof that `abort()` was
    /// called (which alone only schedules cancellation).
    struct DropSignal(Arc<AtomicBool>);

    impl Drop for DropSignal {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    /// Pins the abort-then-await ordering in `drain_writer`'s no-progress
    /// path: a writer landing zero frames across one whole window must be
    /// aborted — and that abort actually awaited to completion — before
    /// the helper returns, not merely timed out and abandoned.
    ///
    /// The task under test never completes on its own
    /// (`std::future::pending`) and holds a `DropSignal` guard; the
    /// sentinel firing is the only way to know the task's future was
    /// truly dropped, which only happens once its cancellation has been
    /// delivered and polled to completion. The outer 5s timeout catches
    /// the regression this test exists to prevent: a helper whose abort
    /// path fires `abort()` without awaiting the handle, or that never
    /// reaches the abort branch at all, would leave `drain_writer` — and
    /// this test — hanging.
    ///
    /// `start_paused` puts the test on tokio's virtual clock: real time
    /// never elapses, so the window boundary is crossed deterministically
    /// at the next await point instead of racing a wall-clock sleep
    /// against scheduler jitter.
    #[tokio::test(start_paused = true)]
    async fn drain_writer_aborts_and_awaits_on_sustained_no_progress() {
        let dropped = Arc::new(AtomicBool::new(false));
        let guard = DropSignal(Arc::clone(&dropped));
        let mut task = tokio::spawn(async move {
            let _guard = guard;
            std::future::pending::<()>().await;
        });
        let frames_written = AtomicU64::new(0);

        tokio::time::timeout(
            Duration::from_secs(5),
            drain_writer(&mut task, &frames_written, Duration::from_millis(50)),
        )
        .await
        .expect("drain_writer must not hang waiting on a task making zero progress");

        assert!(
            dropped.load(Ordering::SeqCst),
            "writer task must have been aborted, and that abort awaited to \
             completion, before drain_writer returns"
        );
    }

    /// The whole reason `drain_writer` exists: a writer that is merely
    /// slow — landing a frame every window instead of finishing inside a
    /// single one — must be allowed to keep going rather than being
    /// aborted the moment one window elapses. This is what tells
    /// "backpressured but alive" apart from "gone" (the case the sibling
    /// test above covers).
    ///
    /// The task increments `frames_written` on a cadence shorter than the
    /// drain window, several times over, before completing — so
    /// `drain_writer` has to ride out multiple windows on renewed
    /// progress and only return once the task finishes naturally. The
    /// `completed` flag is set exclusively on that natural-completion
    /// path (the abort path in `drain_writer` never gets to run it), so
    /// seeing it true is proof the return came from completion, not from
    /// `drain_writer` giving up and aborting the task after the fact.
    ///
    /// Also on `start_paused`'s virtual clock, for the same reason as the
    /// sibling test: the multi-window wait this test exercises would
    /// otherwise depend on real sleeps landing inside real window
    /// boundaries, which is exactly the kind of timing assumption that is
    /// fine on a fast idle machine and flaky under load.
    #[tokio::test(start_paused = true)]
    async fn drain_writer_waits_through_progress_and_returns_on_completion() {
        let frames_written = Arc::new(AtomicU64::new(0));
        let completed = Arc::new(AtomicBool::new(false));
        let writer_counter = Arc::clone(&frames_written);
        let completed_flag = Arc::clone(&completed);
        let mut task = tokio::spawn(async move {
            for _ in 0..10 {
                tokio::time::sleep(Duration::from_millis(20)).await;
                writer_counter.fetch_add(1, Ordering::Relaxed);
            }
            completed_flag.store(true, Ordering::SeqCst);
        });

        tokio::time::timeout(
            Duration::from_secs(5),
            drain_writer(&mut task, &frames_written, Duration::from_millis(60)),
        )
        .await
        .expect("drain_writer must return once the task completes naturally");

        assert!(
            completed.load(Ordering::SeqCst),
            "task must have run to natural completion, not been aborted \
             mid-flight while progress was still arriving"
        );
    }
}
