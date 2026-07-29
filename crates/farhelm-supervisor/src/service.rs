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
//! PLAN_M2.md's "restart gap" paragraph is the contract.
//!
//! M3 adds the half M2 could not answer (PLAN_M3.md item 2): a durable
//! last-known outcome per session, written wherever this process actually
//! WITNESSES a transition, plus the host's boot id. Together they turn "the
//! terminal is gone" into two distinguishable answers — the agent exited
//! (with the code, when something still holds it) versus the host rebooted
//! and took every terminal with it, which is **interrupted**. The
//! classification precedence lives on `session_status`, the recording
//! rules on `Supervisor::record_outcome`/`record_stop`, and the boot
//! comparison on `Supervisor::reload_sessions`.

use crate::launch::{LaunchSpec, resolve_shell, window_command};
use crate::store::{
    Claimed, IntentClaim, LastOutcome, Reservation, ReservationOutcome, RetryClaim, SessionStore,
    Settlement, StoredSession, Transition,
};
use crate::tmux::{InputClient, OutputEvent, OutputStream, PaneModes, PaneState, TmuxDriver};
use anyhow::Context;
use farhelm_proto::io::{
    FrameReader, FrameWriter, ProgressWrite, handshake, parse_control, write_frame_before_stall,
};
use farhelm_proto::{
    AgentKind, ControlMsg, DETACH_REASON_STALLED, ErrorKind, Frame, RestartOffer, SessionInfo,
    SessionStatus,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Mutex, mpsc, oneshot, watch};
use tracing::{error, info, warn};

/// Data-frame chunk size for replay. Well under MAX_FRAME_LEN; small
/// enough that the first screenful renders while the rest streams.
const REPLAY_CHUNK: usize = 32 * 1024;

/// Depth of the per-connection writer queue — the single queue every
/// reply, notification, and terminal data frame on one connection passes
/// through on its way to the socket.
///
/// This bound is the supervisor half of PLAN_M2_5.md's "no unbounded
/// queue remains on the terminal output path". It was an
/// `unbounded_channel` through M2, which meant a helm slower than the
/// panes it was watching grew this process's memory with no ceiling at
/// all — the debt this milestone exists to close.
///
/// What the number buys: a bound at all, expressed in the only unit
/// `mpsc` offers. It counts FRAMES, and frame sizes vary by two orders of
/// magnitude, so it is a ceiling rather than a size. Terminal data frames
/// are chunked at [`REPLAY_CHUNK`] (32 KiB), which puts the worst case for
/// a data-only backlog at 2 MiB per connection; live pane output usually
/// arrives in far smaller notifications, so the typical backlog is a
/// fraction of that. Control frames are capped only by `MAX_FRAME_LEN`,
/// making the absolute worst case much larger on paper — in practice
/// nothing generates large replies back to back, and `LIST_BYTE_BUDGET`
/// already halves the one reply that can be big. Bounding by count keeps
/// this one legible rule instead of a byte-accounting scheme layered over
/// the frame layer.
///
/// The accepted consequence, stated plainly: when the single multiplexed
/// consumer (the helm) is slow, control REPLIES queue behind terminal
/// DATA at this bound, so a request can wait on a busy terminal's
/// backlog. That is acceptable because the alternative — unbounded — is
/// exactly the debt being closed, and because a client that reaches this
/// bound repeatedly is one the browser's watermark should already be
/// pausing. Nothing that holds a supervisor mutex may block on this
/// queue; see [`notify_detached`] for the one shape that would otherwise
/// want to, and the `Attach` handler's reserved permit for the other.
const CONNECTION_WRITER_QUEUE: usize = 64;

/// The longest a single attachment may stay paused before the supervisor
/// detaches it with [`DETACH_REASON_STALLED`].
///
/// Deliberately a hard maximum PAUSE DURATION rather than a "no progress"
/// test: between pause and resume the supervisor receives nothing from
/// that client, so progress during a pause is unobservable by design
/// (PLAN_M2_5.md). That is sound because a live client's pauses are short
/// by construction — the drainable backlog is bounded by the UI's
/// high-water mark plus the bounded queues below it, a few MiB, which
/// even the slowest real parser clears in seconds. A pause that outlives
/// this is a wedge (a crashed tab, a laptop asleep past its WebSocket
/// timeout), not a slow reader.
///
/// Generous on purpose: a false detach costs a reattach, which is cheap
/// and replays automatically, while a missed one pins buffers at every
/// hop for as long as the wedge lasts.
///
/// Injectable per supervisor (see [`SupervisorTimeouts`]) so integration
/// tests can shorten it to something a test can afford to wait out.
/// Deliberately NOT read from the environment: this repo's tests never
/// mutate the process environment.
pub const STALL_DETACH_TIMEOUT: Duration = Duration::from_secs(60);

/// How long one frame may sit unwritten before the connection's writer
/// task declares the peer gone.
///
/// Bounding the writer queue made this necessary, and the necessity is
/// not obvious, so: with an UNBOUNDED queue, a peer that stopped reading
/// grew memory without limit but never blocked anything — the read loop
/// kept running and tore the connection down the moment it saw EOF.
/// Bounded, the same peer instead backpressures every producer, including
/// `handle_control` itself once the admission permits are all held by
/// tasks parked on a full queue. The read loop then never reaches its
/// `select!` again, never observes EOF, and the whole connection task
/// leaks — the exact failure `WRITER_DRAIN_TIMEOUT` was introduced to
/// prevent, reintroduced through the other door.
///
/// This closes it at the only place that can still observe progress: a
/// write that does not complete inside this window is treated exactly
/// like a write ERROR, which drops the queue's receiver, unblocks every
/// parked sender with a closed-channel error, and lets the read loop reach
/// its `select!` and end the connection.
///
/// Measured as BYTE progress, not whole-frame completion (see
/// `farhelm_proto::io::ProgressWrite`): the window re-arms
/// on every byte the transport accepts, so a frame arbitrarily larger than
/// the window still reaches a peer that is merely slow. That is what
/// SPEC.md requires — a slow viewer is served for as long as it takes, and
/// only one that stops consuming entirely is cut. An earlier version timed
/// the whole frame and would have cut a healthy peer on a slow link the
/// moment one large `ListSessions` reply outlasted the window.
///
/// The residual is therefore only this: a transport that accepts not one
/// byte for a full minute is called gone. That is indistinguishable from
/// gone at every layer this process can see.
pub const WRITER_STALL_TIMEOUT: Duration = Duration::from_secs(60);

/// The timeouts a `Supervisor` treats as "this consumer is gone, not
/// merely slow".
///
/// Grouped into one injectable value rather than a growing list of
/// constructor parameters: both are properties of the same judgement call
/// (how long to serve something that may be wedged), both default to
/// generous production values, and integration tests need to shorten
/// whichever one their scenario exercises without caring about the other.
/// Injected at construction rather than settable later because long-lived
/// tasks read them — an attachment forwarder and a connection writer would
/// otherwise have no single answer to "how long may this take".
#[derive(Debug, Clone, Copy)]
pub struct SupervisorTimeouts {
    /// See [`STALL_DETACH_TIMEOUT`].
    pub stall_detach: Duration,
    /// See [`WRITER_STALL_TIMEOUT`].
    pub writer_stall: Duration,
}

impl Default for SupervisorTimeouts {
    fn default() -> Self {
        SupervisorTimeouts {
            stall_detach: STALL_DETACH_TIMEOUT,
            writer_stall: WRITER_STALL_TIMEOUT,
        }
    }
}

/// Where the current boot id comes from (PLAN_M3.md item 2).
///
/// A closure rather than a path, because tests must be able to simulate a
/// REBOOT — the one event this whole classification exists for and the one
/// thing a test may not actually do — by simply answering differently on
/// the second construction. Reading the real file is
/// [`read_host_boot_id`]; environment variables are deliberately not a
/// mechanism here (this project's tests never mutate the test process's
/// environment).
///
/// Three outcomes, deliberately distinguished (PLAN_M3.md item 2):
///
/// - `Ok(Some(id))` — this boot, positively identified.
/// - `Ok(None)` — this host does not publish a boot id AT ALL. Not an
///   error and never becomes one: with no evidence in either direction,
///   classification takes the same-boot path forever and nothing is ever
///   marked interrupted, the same no-guessing stance a pre-M3 database's
///   absent stored id gets.
/// - `Err` — the host HAS one and reading it failed this time. Distinct
///   from `Ok(None)` because the consequences are opposite: treating a
///   transient failure as "unsupported host" would take the same-boot path
///   and durably record exits for sessions a retry might have classified
///   interrupted — an irreversible answer derived from a temporary
///   condition. Reload therefore degrades instead (see `reload_sessions`).
pub type BootIdSource = Arc<dyn Fn() -> anyhow::Result<Option<String>> + Send + Sync>;

/// The boundaries inside a create at which a crash can be simulated —
/// the create-LIFECYCLE seam (PLAN_M3.md items 2 and 6), distinct from
/// item 5's file-write seam, which covers none of these windows.
///
/// Every stage is a point where a real crash leaves durable state in a
/// DIFFERENT shape, and the three shapes are exactly what item 6's
/// reconciliation has to tell apart — which is why they are named stages
/// rather than one undifferentiated "crash now" hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateStage {
    /// After the durable launching row is committed — together with a
    /// first-time intent's reservation, or by the takeover that reclaimed
    /// a pending one — and before ANY external side effect: no launch spec
    /// on disk, no tmux session. What survives is a row describing an
    /// attempt that provably never got anywhere, which is the state item
    /// 2's ordering rule exists to guarantee and the only one a retry may
    /// relaunch over.
    AfterRecord,
    /// After tmux has the session and its pane, and before that launch is
    /// confirmed durably. What survives is a `Launching` row with no pane
    /// recorded while the tmux session genuinely exists — reload's pane
    /// rediscovery is what reconciles it. (Whether the AGENT is running is
    /// a separate question at this instant: the shim may not have reached
    /// its `exec` yet, and may yet fail it.)
    DuringLaunch,
    /// After the launch is confirmed durably and before the reservation's
    /// outcome is recorded. The session fully exists; only the intent
    /// table does not yet know it. This is acceptance 7's "the reply is
    /// dropped AFTER the session durably exists", from the inside.
    ///
    /// Reached only by a create that CARRIES an intent key: an unkeyed
    /// create has no outcome to precede, so there is no window here to
    /// crash in.
    BeforeOutcome,
}

/// Marks an error as having come from the create-lifecycle seam rather
/// than from the create itself.
///
/// The distinction is load-bearing, not cosmetic: an ordinary create
/// failure settles its reservation (so a retry replays the same error),
/// while a CRASH runs no further code at all — every durable write after
/// the injected point simply never happens, which is the entire state the
/// stage is there to produce. Without this marker, the injected error
/// would flow into the settlement path and durably record a failure a real
/// crash could never have recorded, making all three stages replay an
/// error instead of reconciling.
///
/// Attached as a context layer by `Supervisor::simulate_crash`, so it is
/// findable by `downcast_ref` wherever the error ends up.
#[derive(Debug, thiserror::Error)]
#[error("simulated create crash")]
struct SimulatedCrash;

/// A simulated crash at one of [`CreateStage`]'s boundaries in
/// `create_session`.
///
/// Returning an error aborts the create immediately, skipping the cleanup
/// an ordinary in-process failure performs — which is the point: a real
/// crash does not get to run cleanup either, and the durable record it
/// leaves behind is exactly what item 2's ordering rule and item 6's
/// reconciliation are about. Production never sets one; a seam that does
/// not care about a given stage returns `Ok(())` for it.
pub type CreateCrashSeam = Arc<dyn Fn(CreateStage) -> anyhow::Result<()> + Send + Sync>;

/// The injectable seams a `Supervisor` is built with. All default to
/// production behavior; grouped into one struct so a new injection point
/// does not grow the constructor's signature again.
#[derive(Clone)]
pub struct SupervisorSeams {
    /// See [`BootIdSource`]. Defaults to reading this host's real boot id.
    pub boot_id: BootIdSource,
    /// See [`CreateCrashSeam`]. `None` in production.
    pub create_crash: Option<CreateCrashSeam>,
}

impl Default for SupervisorSeams {
    fn default() -> Self {
        SupervisorSeams {
            boot_id: Arc::new(read_host_boot_id),
            create_crash: None,
        }
    }
}

/// This host's current boot id (PLAN_M3.md item 2).
///
/// Linux publishes a per-boot UUID at `/proc/sys/kernel/random/boot_id`.
/// A host that does not have the file at all is reported as `Ok(None)` —
/// unsupported, honestly — while any OTHER read failure is an `Err`: see
/// [`BootIdSource`] for why the two must not be collapsed.
///
/// macOS is NOT handled here. The Mac-supervisor work owns finding the
/// equivalent (`kern.boottime`), recorded as a deferral in PLAN_M3.md's
/// Out section beside the /proc-less process sweep it will land with;
/// until then a Mac build would take the honest `Ok(None)` path and never
/// claim a reboot.
fn read_host_boot_id() -> anyhow::Result<Option<String>> {
    const PATH: &str = "/proc/sys/kernel/random/boot_id";
    let raw = match std::fs::read_to_string(PATH) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(anyhow::Error::new(e).context(format!("reading {PATH}"))),
    };
    let trimmed = raw.trim();
    // An empty file is not a usable id, and storing "" would make a later
    // real id look like a reboot on no evidence. Treated as unsupported
    // rather than as a failure: there is nothing to retry.
    Ok((!trimmed.is_empty()).then(|| trimmed.to_string()))
}

/// One process's claim on a state directory, and the token every mutating
/// startup step requires (PLAN_M3.md item 2).
///
/// SPEC.md allows at most one supervisor per user per host, and M2 already
/// enforced that with an `flock` — but only inside `serve`, which left the
/// entire startup sequence (schema migration, reboot classification,
/// outcome reconciliation) running BEFORE the claim was made. Two harms
/// followed, both silent: a candidate that loses the race could first
/// upgrade the incumbent's database out from under it, leaving an older
/// build that will refuse its own state on its next restart; and a startup
/// overlapping a still-running predecessor could record that predecessor's
/// live sessions as ended. Acquiring the lock BEFORE the store is even
/// opened is what closes both.
///
/// ## Why ownership is per (process, state dir), not per Supervisor
///
/// `flock` is exclusive across open file descriptions, INCLUDING two in
/// the same process, so a naive per-`Supervisor` lock would make a second
/// in-process `Supervisor` on the same directory impossible — the shape
/// every restart test in this repo uses to stand in for a restarted
/// process, and the shape `serve`'s own refusal test needs. The registry
/// below therefore hands every `Supervisor` for the same directory the
/// same claim: cross-process exclusivity is exactly as strict as before
/// (the file lock is held for as long as any of them lives), while
/// in-process construction stays possible. Production has one `Supervisor`
/// per process (`run`), so nothing there depends on the difference.
///
/// "At most one SERVING supervisor" is enforced separately by `serving`,
/// which is what `serve` swaps — so a second in-process `serve` is refused
/// with the same message a second process gets.
#[derive(Debug)]
pub struct StateDirOwnership {
    path: PathBuf,
    /// Held open for the lifetime of this value: dropping the file
    /// releases the `flock`, which is what makes ownership end when the
    /// last `Supervisor` for this directory goes away.
    _lock: std::fs::File,
    serving: std::sync::atomic::AtomicBool,
}

/// Claims this process holds, keyed by state directory.
///
/// `Weak` so a dropped claim releases the underlying lock file; entries
/// are removed by `StateDirOwnership`'s own `Drop`.
static STATE_DIR_CLAIMS: std::sync::LazyLock<
    std::sync::Mutex<HashMap<PathBuf, std::sync::Weak<StateDirOwnership>>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

impl Drop for StateDirOwnership {
    fn drop(&mut self) {
        if let Ok(mut claims) = STATE_DIR_CLAIMS.lock() {
            // Only remove an entry that is genuinely dead: a `Supervisor`
            // constructed for this same directory while this one was
            // dropping would already have replaced it.
            if claims
                .get(&self.path)
                .is_some_and(|existing| existing.strong_count() == 0)
            {
                claims.remove(&self.path);
            }
        }
    }
}

impl StateDirOwnership {
    /// Claim `state_dir` for this process, reusing the claim if this
    /// process already holds one.
    ///
    /// `Ok(None)` means another PROCESS holds it. That is not fatal here:
    /// a `Supervisor` without the claim can still be constructed and can
    /// still answer requests (which is what a handoff's brief overlap
    /// looks like) — it simply may not migrate the schema or write any
    /// reconciliation, and its `serve` will refuse.
    ///
    /// One benign race is left deliberately: a claim being DROPPED
    /// concurrently with this call can leave the registry entry already
    /// dead while its lock file has not finished closing, so the
    /// `try_lock` below fails and this caller starts read-only as though
    /// another process held the directory. It resolves itself on the next
    /// construction, and it fails in the safe direction (no migration, no
    /// reconciliation) — the only direction worth being sure of.
    fn claim(state_dir: &Path) -> anyhow::Result<Option<Arc<StateDirOwnership>>> {
        // Canonicalized so two spellings of the same directory cannot
        // yield two claims; the directory exists by now (`ensure_private_dir`).
        let path = std::fs::canonicalize(state_dir)
            .with_context(|| format!("resolving state dir {}", state_dir.display()))?;
        let mut claims = STATE_DIR_CLAIMS.lock().expect("claims mutex poisoned");
        if let Some(existing) = claims.get(&path).and_then(std::sync::Weak::upgrade) {
            return Ok(Some(existing));
        }
        let lock_path = path.join("supervisor.lock");
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock_path)
            .context("opening supervisor lock file")?;
        if lock.try_lock().is_err() {
            return Ok(None);
        }
        let owned = Arc::new(StateDirOwnership {
            path: path.clone(),
            _lock: lock,
            serving: std::sync::atomic::AtomicBool::new(false),
        });
        claims.insert(path, Arc::downgrade(&owned));
        Ok(Some(owned))
    }

    /// Take the right to serve, or report that something already has it.
    ///
    /// One-way: nothing ever hands the right back, because a supervisor
    /// only stops serving by ending. `SeqCst` costs nothing at this
    /// frequency (once per process) and keeps the ordering argument to
    /// "there is none to make".
    fn begin_serving(&self) -> bool {
        !self.serving.swap(true, std::sync::atomic::Ordering::SeqCst)
    }
}

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

/// Byte cap on `CreateSession`'s `intent_key` (PLAN_M3.md item 6),
/// enforced alongside `CREATE_FIELD_CAP` before any lookup or write.
///
/// Separate from that cap rather than folded into it, because what the two
/// protect is different in kind: the field cap bounds a REPLY that would
/// otherwise be undeliverable, while this one bounds a durable, deliberately
/// un-pruned table (see `store::Reservation`'s tombstone docs) whose primary
/// key is whatever the client sent. Without a bound, one client can spend
/// unbounded disk on keys nothing will ever replay. 512 bytes is two orders
/// of magnitude beyond a UUID — the shape the UI actually sends — while
/// still leaving room for a caller that prefers structured keys.
const INTENT_KEY_CAP: usize = 512;

/// Cap on how many argv elements `CreateSession`'s `resume_template`
/// override may carry (PLAN_M3.md items 6 and 7).
///
/// Independent of the byte cap it is enforced alongside, because the two
/// bound different things: a template of ten thousand EMPTY elements costs
/// almost nothing in bytes while still being nothing a resume invocation
/// could legitimately be, and it lands in the same never-pruned
/// reservation row. 64 elements is far beyond every real resume
/// invocation (`claude --resume {conversation}` is three).
const RESUME_TEMPLATE_ELEMENT_CAP: usize = 64;

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

/// The canonical form of everything an intent key is bound to
/// (PLAN_M3.md item 6): a create replays only when its key AND this string
/// both match what the key was claimed with.
///
/// ## What is in it, and what is deliberately not
///
/// Every SESSION-shaping field: `cwd`, `invocation`, `title` (as SENT —
/// `None` means "auto-generate", which is a different request from an
/// explicit title that happens to equal the derived one), and item 7's
/// `agent_kind`/`resume_template` overrides. `cols`/`rows` are excluded by
/// design: they shape the ATTACHMENT, not the session, so the same intent
/// retried from a differently-sized client is still the same intent — a
/// point the plan makes explicitly, and the reason this function takes no
/// dimensions at all rather than taking and ignoring them.
///
/// The overrides are threaded in here BEFORE anything else reads them: as
/// of this PR they shape nothing but the fingerprint (item 7 is what makes
/// them shape the session), and that is not a placeholder — acceptance 7
/// requires a request differing ONLY in an override to be rejected as a
/// key reuse, which is a property of the fingerprint alone.
///
/// ## Representation
///
/// The canonical FIELDS, JSON-encoded as a fixed-order tuple — not a
/// digest. A JSON array is unambiguous (no field can bleed into its
/// neighbour the way a delimiter-joined string can, since every element is
/// separately quoted and escaped), deterministic (element order is this
/// function's, and no map is involved to have an ordering question),
/// comparable with `==`, and readable by a human debugging a database.
///
/// The `agent_kind` element is spelled with this module's OWN stable
/// vocabulary rather than the wire type's serde representation: the two
/// agree today, but a future protocol rename would otherwise change every
/// stored fingerprint at once and turn identical requests into key-reuse
/// conflicts across an upgrade. The persisted spelling is pinned by a
/// golden test for the same reason `LastOutcome`'s column vocabulary is.
///
/// What a DIGEST would have bought, and why it is not here: a constant-size
/// row, and an end to the `invocation` — which may embed credentials —
/// being retained past its session's deletion in a tombstone. Only the
/// second is a real property, and it is about RETENTION rather than
/// exposure: the same string already sits in `sessions.invocation`, in the
/// same 0600 database inside the same 0700 state directory, so what changes
/// is how long a copy outlives its session, not who can read it. That is a
/// separate piece of work from bounding the row COUNT (see
/// `store::Reservation`), and neither is owned here.
fn create_fingerprint(
    cwd: &str,
    invocation: &str,
    title: Option<&str>,
    agent_kind: Option<AgentKind>,
    resume_template: Option<&[String]>,
) -> String {
    // Infallible in practice: every element is a string, an option, or an
    // array of strings, none of which can fail to serialize. The `expect`
    // documents that rather than inviting a caller to handle an error that
    // cannot occur.
    serde_json::to_string(&(
        cwd,
        invocation,
        title,
        agent_kind.map(agent_kind_fingerprint),
        resume_template,
    ))
    .expect("a fingerprint of strings and options always serializes")
}

/// This module's own stable spelling of an [`AgentKind`] for
/// [`create_fingerprint`]; see that function's representation notes for why
/// the wire encoding is deliberately not reused.
fn agent_kind_fingerprint(kind: AgentKind) -> &'static str {
    match kind {
        AgentKind::Claude => "claude",
        AgentKind::Codex => "codex",
        AgentKind::Generic => "generic",
    }
}

/// Serializes creates that share an intent key, so concurrent retries of
/// one intent collapse to ONE launch (PLAN_M3.md item 6) instead of racing
/// each other's reservation lookup.
///
/// ## Why an in-process lock is enough
///
/// Cross-process collapse is not needed because cross-process creates
/// cannot happen: SPEC.md allows at most one supervisor per user per host,
/// and this build enforces it before anything durable is touched — an
/// `flock` on the state directory taken in the constructor
/// (`StateDirOwnership`) plus the serve-right swap that refuses a second
/// `serve` even in-process. Every create against one state directory
/// therefore flows through one process, so one process's locks cover every
/// racer there is.
///
/// ## What the lock is, and is not, responsible for
///
/// The durable reservation is the mechanism that makes a duplicate
/// IMPOSSIBLE, and it does so without help: the claim is atomic
/// (`store::Claimed`), and a relaunch takes its pending reservation over
/// through an atomic conditional transition (`store::RetryClaim`), so a
/// racer that bypassed this lock entirely still cannot launch twice — it
/// loses one of those two transitions and answers from the winner instead.
///
/// What the lock adds is that a concurrent retry gets the RIGHT answer
/// rather than merely a safe one. Without it, a second request arriving
/// mid-launch would find a pending reservation whose side effects do not
/// exist YET and would be entitled to conclude the first attempt never
/// launched — evidence-gathering cannot distinguish "in flight right now"
/// from "died before doing anything", because they look identical from
/// outside the process. Serializing on the key removes that ambiguity at
/// the source: by the time the second request looks, the first has an
/// outcome. Nothing about correctness rests on WHEN the guard is released
/// relative to the settlement — a late settlement simply means the next
/// request reconciles instead of replaying.
///
/// Entries are pruned when the last holder leaves, so the map is bounded
/// by in-flight creates rather than by every key ever seen.
#[derive(Debug, Default)]
struct IntentLocks {
    /// `Weak` so an entry dies with its last guard; see [`IntentGuard`]'s
    /// `Drop`. A std mutex, not a tokio one: it is held only for the map
    /// lookup itself, never across the `await` that acquires the per-key
    /// lock.
    locks: std::sync::Mutex<HashMap<String, std::sync::Weak<tokio::sync::Mutex<()>>>>,
}

impl IntentLocks {
    /// Hold this key's lock until the returned guard is dropped, waiting
    /// out any create already running under the same key.
    async fn claim(self: &Arc<Self>, key: &str) -> IntentGuard {
        let lock = {
            let mut locks = self.locks.lock().expect("intent lock map poisoned");
            match locks.get(key).and_then(std::sync::Weak::upgrade) {
                Some(existing) => existing,
                None => {
                    let fresh = Arc::new(tokio::sync::Mutex::new(()));
                    locks.insert(key.to_string(), Arc::downgrade(&fresh));
                    fresh
                }
            }
        };
        IntentGuard {
            registry: Arc::clone(self),
            key: key.to_string(),
            _held: lock.lock_owned().await,
        }
    }
}

/// One create's exclusive hold on its intent key; see [`IntentLocks`].
struct IntentGuard {
    registry: Arc<IntentLocks>,
    key: String,
    /// Owned rather than borrowed so the guard is `'static` and can be
    /// held across every await in a create.
    _held: tokio::sync::OwnedMutexGuard<()>,
}

impl Drop for IntentGuard {
    fn drop(&mut self) {
        let Ok(mut locks) = self.registry.locks.lock() else {
            return;
        };
        // Only when this guard is the LAST holder: a waiter that has
        // already upgraded the `Weak` is counted here (strong count above
        // one), and removing the entry out from under it would let a third
        // create insert a DIFFERENT mutex for the same key and run
        // alongside it. Racers still inside `claim` are excluded by the map
        // mutex this holds.
        //
        // The residual window is deliberate and bounded by the durable
        // transitions rather than by this lock: a create arriving after
        // this removal but before this guard's own mutex is released gets
        // a fresh lock and proceeds concurrently. The departing create has
        // by then finished its launch and its rollback (whichever
        // happened), so the newcomer either finds a settled outcome or
        // finds a pending one whose takeover is itself atomic — it cannot
        // launch a second agent either way.
        if locks
            .get(&self.key)
            .is_some_and(|existing| existing.strong_count() <= 1)
        {
            locks.remove(&self.key);
        }
    }
}

/// The identities one launch will wear: the session id, and the tmux
/// session name derived from it.
///
/// Assigned BEFORE the launch (and, for a keyed create, stored with the
/// reservation) rather than discovered from it, because every later
/// reconciliation is a question about these two names — "did anything
/// happen under them?" — which cannot be asked about a name that was never
/// written down.
#[derive(Debug, Clone)]
struct SessionIdentity {
    session_id: String,
    tmux_name: String,
}

/// Mint identities for a launch that does not inherit any.
///
/// The FULL uuid, not a truncated prefix: an 8-hex-char prefix collides
/// often enough in practice for two sessions — one live, one a dead row
/// surviving in SQLite across a restart — to plausibly share a tmux name,
/// which would cross-wire attach between an unrelated pair of sessions
/// after a reload. The schema's `UNIQUE` constraint on `tmux_name` (see
/// `store.rs`) backstops this at the DB layer; a full UUID is what makes
/// that constraint never fire in the first place. Dashes are legal in tmux
/// session names (verified empirically against a scratch server).
fn new_session_identity() -> SessionIdentity {
    let session_id = uuid::Uuid::new_v4().to_string();
    let tmux_name = format!("fh-{session_id}");
    SessionIdentity {
        session_id,
        tmux_name,
    }
}

/// What an existing reservation means for the request that found it.
enum Resolution {
    /// This intent already has an answer — the session it created, the
    /// gone-error, the original failure, a key-reuse refusal, or an
    /// honest "cannot tell". Whatever it is, it is what the caller
    /// returns, unchanged.
    Answer(anyhow::Result<SessionInfo>),
    /// Nothing was ever launched under this reservation, so the caller
    /// performs the create under it — same key, same identities.
    Relaunch(Box<Reservation>),
}

/// What is known about whether a reserved launch reached tmux; see
/// `Supervisor::reserved_launch_evidence` for the sources and for why
/// absence must be POSITIVE rather than merely unobserved.
enum LaunchEvidence {
    Present,
    Absent,
    /// A source that should have answered could not be read. Carries the
    /// cause, which reaches the client: this is a state a human can
    /// usually clear (a wedged tmux, an unreadable state directory), and
    /// the retry that follows is expected to resolve properly.
    Unresolved(anyhow::Error),
}

/// Wrap a create failure whose OUTCOME could not be recorded against its
/// intent key.
///
/// The caller must not be handed the original error alone: doing so claims
/// a durability this create does not have. A client that sees "working
/// directory does not exist" reasonably concludes the key is spent and
/// that retrying is pointless, when in fact nothing was recorded and a
/// retry may do something entirely different. `Internal` because no
/// different request would have avoided it.
fn unrecorded_outcome(original: anyhow::Error, settle: anyhow::Error) -> anyhow::Error {
    let message = format!(
        "the create failed ({original:#}), and recording that outcome against its intent key \
         also failed ({settle:#}); the key is therefore NOT spent — a retry may produce a \
         different answer"
    );
    original.context(RequestError::new(ErrorKind::Internal, message))
}

/// One create's inputs, VALIDATED: the working directory exists, the
/// invocation parsed, and the title has already been defaulted from the
/// cwd if the caller omitted it.
///
/// Grouped rather than passed as six parameters because the idempotency
/// state machine has to hand the same bundle to a launch from three
/// different branches, and because the type is what says "these have been
/// checked" — `Supervisor::launch_session` performs no validation of its
/// own and would have no way to.
struct LaunchRequest<'a> {
    cwd: &'a str,
    invocation: &'a str,
    argv: Vec<String>,
    title: String,
    cols: u16,
    rows: u16,
}

/// What a launch owes the reservation table — the three cases
/// `Supervisor::launch_session` can be asked to run under — and, in every
/// case, the identities it runs under.
enum Reserved {
    /// A create with no intent key at all: pre-M3 behavior, no
    /// deduplication, nothing written to the reservation table.
    Unkeyed(SessionIdentity),
    /// A key seen for the first time. Its reservation is claimed in the
    /// same transaction as the launching row (`SessionStore::insert_session`).
    New {
        claim: IntentClaim,
        identity: SessionIdentity,
    },
    /// A key whose reservation is already `Pending` and under whose
    /// identities nothing was ever launched — a crash between the claim and
    /// the launch. This attempt redoes the launch under those SAME
    /// identities, so however many attempts an intent takes, it can only
    /// ever leave one session id and one tmux name behind.
    Retry(Box<Reservation>),
}

impl Reserved {
    /// The session id this launch runs under.
    fn session_id(&self) -> &str {
        match self {
            Reserved::Unkeyed(identity) | Reserved::New { identity, .. } => &identity.session_id,
            Reserved::Retry(reservation) => &reservation.session_id,
        }
    }

    /// The tmux session name this launch runs under.
    fn tmux_name(&self) -> &str {
        match self {
            Reserved::Unkeyed(identity) | Reserved::New { identity, .. } => &identity.tmux_name,
            Reserved::Retry(reservation) => &reservation.tmux_name,
        }
    }

    /// The settlement that records `outcome` against this launch's intent,
    /// or `None` for an unkeyed create. Identity-conditioned by
    /// construction (see `store::Settlement`), so a settlement built here
    /// can never land on a reservation pointing somewhere else.
    fn settlement(&self, outcome: ReservationOutcome) -> Option<Settlement> {
        let intent_key = match self {
            Reserved::Unkeyed(_) => return None,
            Reserved::New { claim, .. } => claim.intent_key.clone(),
            Reserved::Retry(reservation) => reservation.intent_key.clone(),
        };
        Some(Settlement {
            intent_key,
            session_id: self.session_id().to_string(),
            outcome,
        })
    }
}

/// Async wrapper around [`crate::launch::read_launch_sentinel`] for the
/// handlers in this module: `ListSessions` calls it on every poll for
/// every eligible session, which makes it the genuinely hot one;
/// `reload_sessions` calls it only once per construction (or handoff), far
/// off any hot path, but shares this wrapper anyway so the two call sites
/// can never diverge on how the read reaches the filesystem.
///
/// `spawn_blocking` wraps what is usually a single `ENOENT`-returning
/// `read` (cheap in the overwhelmingly common case: no launch has ever
/// failed for this session) because a synchronous syscall run inline on
/// an async worker thread blocks every OTHER session's terminal
/// forwarding sharing that thread for however long the underlying I/O
/// takes — worth paying on `ListSessions`'s polling path even though any
/// one call is ordinarily fast.
async fn read_launch_sentinel(state_dir: &Path, id: &str) -> anyhow::Result<Option<String>> {
    let state_dir = state_dir.to_path_buf();
    let id = id.to_string();
    tokio::task::spawn_blocking(move || crate::launch::read_launch_sentinel(&state_dir, &id))
        .await
        .context("launch sentinel read task panicked")?
}

/// Whether a launch sentinel discovered NOW could still change `outcome` —
/// the read-time mirror of `Transition::apply`'s own `SentinelError` rule
/// (`store.rs`), kept as one function so the two places that decide
/// whether reading the file is even worth attempting (this module's
/// `reload_sessions` and `ListSessions` handler) can never drift from what
/// the store would actually do with the reading once it is offered.
///
/// `false` only for an already-`Error` row (idempotent — nothing to gain)
/// and for a GENUINELY annotated `Exited` (a real stop: retained
/// knowledge, not an inference a sentinel could outrank). `true` for
/// everything else, INCLUDING `Interrupted` and an unannotated `Exited` —
/// PLAN_M3.md item 3 requires a late-discovered sentinel to still
/// supersede both, because neither is anything more than an inference
/// from an ordinary dead-or-vanished pane, exactly the evidence class a
/// sentinel is defined to beat.
fn sentinel_could_still_apply(outcome: &LastOutcome) -> bool {
    !matches!(
        outcome,
        LastOutcome::Error { .. }
            | LastOutcome::Exited {
                annotation: Some(_),
                ..
            }
    )
}

/// Remove both files a launch's `Error` classification can leave behind:
/// the sentinel itself, and the per-launch SPEC file the shim's own
/// missing/malformed-spec early-return paths (or a failed unlink partway
/// through one) can leave stranded holding the agent's full command line,
/// credentials included. Called once a launch's `Error` outcome is
/// confirmed durably committed — nothing ever needs either file again
/// once the classification is settled — and also, idempotently, on every
/// row already found to be `Error` on load: a crash between an EARLIER
/// pass's commit and the cleanup that should have followed it can leave
/// one or both files behind for an arbitrary number of startups, and this
/// is what finally sweeps them. Best-effort throughout
/// (`best_effort_remove`): a failure here is logged, never fatal, and
/// never blocks a reply — both files are cosmetic once the DURABLE
/// outcome already says what happened.
async fn cleanup_launch_artifacts(state_dir: &Path, id: &str) {
    let spec_path = crate::launch::spec_path_for_session(state_dir, id);
    let status_path = crate::launch::status_path_for_spec(&spec_path);
    best_effort_remove(&status_path, "consumed launch sentinel").await;
    best_effort_remove(&spec_path, "leftover launch spec").await;
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

/// Sweep `<state_dir>/launch/` at supervisor startup: remove orphaned
/// staged temp files and launch SPECS that no session in `sessions` still
/// owns. Called once from `Supervisor::serve`, after the exclusivity bind
/// (this process must be provably the state dir's one supervisor before
/// touching anything) and after the session map has been reloaded from
/// the store (this sweep needs it to answer "does anything still own
/// this spec").
///
/// Sentinels (`.status` files) are NEVER touched here, regardless of
/// ownership — PLAN_M3.md item 5's durability promise for them would be
/// worthless if a blanket startup sweep could erase the very evidence a
/// later classifier needs to read; their lifecycle (supersede on
/// relaunch, or explicit delete) belongs entirely to that future
/// consumer, never to this best-effort hygiene pass.
///
/// A spec's session id (its file stem) is checked against `sessions` —
/// rather than removing every entry unconditionally, which is what this
/// sweep used to do — because a supervisor restart does NOT kill tmux: a
/// session created just before the restart can have its login shell
/// STILL mid-flight toward `exec farhelm internal launch <spec>`,
/// arbitrarily long after tmux itself created the window (a slow or hung
/// rc-file is a real, if rare, way this stretches out). Its session id is
/// already durably recorded (the just-reloaded `sessions` map reflects
/// SQLite, loaded before this sweep runs), so "does a session with this
/// id exist" is a real ownership question, not a guess: a spec whose id
/// is UNKNOWN can only have gotten here two ways — the create that wrote
/// it crashed before the DB insert ever committed (nothing will ever read
/// it), or its session was since deleted and `DeleteSession`'s own
/// removal of it already failed (logged there) — either way, nothing
/// alive will ever come back for it.
///
/// Best-effort and log-only: this sweep is credential hygiene (specs hold
/// full agent command lines), so a failure that leaves debris behind must
/// at least say so in the log, but never fails startup over it.
async fn sweep_launch_dir(launch_dir: &Path, sessions: &std::collections::HashSet<String>) {
    let mut entries = match tokio::fs::read_dir(launch_dir).await {
        Ok(entries) => entries,
        Err(e) => {
            warn!(error = %e, "could not sweep launch dir; orphaned entries may remain");
            return;
        }
    };
    loop {
        let entry = match entries.next_entry().await {
            Ok(None) => break,
            Ok(Some(entry)) => entry,
            Err(e) => {
                warn!(error = %e, "launch-dir sweep aborted early; orphaned entries may remain");
                break;
            }
        };
        let name = entry.file_name();
        let name = name.to_string_lossy();

        let should_remove = if crate::files::is_staged_temp_name(&name) {
            true
        } else if let Some(id) = name.strip_suffix(".json") {
            !sessions.contains(id)
        } else {
            // `.status` sentinels, and anything else this sweep does not
            // recognize, are never its to remove — see the function's
            // own docs.
            false
        };

        if should_remove && let Err(e) = tokio::fs::remove_file(entry.path()).await {
            warn!(path = %entry.path().display(), error = %e,
                "could not remove orphaned launch-dir entry");
        }
    }
}

/// Sweep abandoned `overwrite_private_file` staging files (`.tmux.conf.tmp-*`)
/// directly out of `<state_dir>` (the tmux config's own location — the
/// one write-atomicity-tier file that lives at the state-dir ROOT rather
/// than under `launch/` or `snapshots/`, so neither of those sweeps would
/// ever see its debris). Same placement and reasoning as
/// [`sweep_snapshot_temp_files`]: after the exclusivity bind, best-effort
/// and log-only.
///
/// Scoped specifically to names starting with `.tmux.conf` (not a bare
/// [`crate::files::is_staged_temp_name`] check against the whole state
/// dir) because the state-dir root also holds `supervisor.db`,
/// `supervisor.sock`, and `supervisor.lock` — none of which stage temp
/// files this way — and `launch/`/`snapshots/` as subdirectories; a
/// prefix match keeps this sweep from ever needing to reason about
/// entries that are not its concern at all.
async fn sweep_tmux_config_temp_files(state_dir: &Path) {
    const CONFIG_TEMP_PREFIX: &str = ".tmux.conf.tmp-";
    let mut entries = match tokio::fs::read_dir(state_dir).await {
        Ok(entries) => entries,
        Err(e) => {
            warn!(error = %e, "could not sweep tmux-config temp files; orphaned staging files may remain");
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
                    .is_some_and(|name| name.starts_with(CONFIG_TEMP_PREFIX));
                if is_temp_file && let Err(e) = tokio::fs::remove_file(entry.path()).await {
                    warn!(path = %entry.path().display(), error = %e,
                        "could not remove orphaned tmux-config temp file");
                }
            }
            Err(e) => {
                warn!(error = %e,
                    "tmux-config temp-file sweep aborted early; orphaned staging files may remain");
                break;
            }
        }
    }
}

/// Sweep abandoned `overwrite_private_file` staging files (`*.tmp-*`)
/// out of `<state_dir>/snapshots/` at supervisor startup — called once
/// from `Supervisor::serve`, same spirit and placement as the launch-dir
/// sweep just above it (after the exclusivity bind, so this process is
/// provably the state dir's one supervisor before touching anything).
///
/// `overwrite_private_file` already cleans up its own temp file when its
/// write or rename fails (`crate::files::remove_temp_after_failure`), but
/// that cleanup only runs if THIS process is still alive to run it — a
/// hard crash (OOM kill, `kill -9`, power loss) between staging the temp
/// file and either renaming it into place or reaching the failure-cleanup
/// path skips it entirely, leaving an orphaned `.tmp-*` file behind
/// forever with nothing else that would ever remove it. This sweep is
/// that backstop.
///
/// Deliberately narrower than a blanket sweep: `snapshots/` also holds
/// legitimate, PERSISTENT snapshot files meant to survive a restart (see
/// `snapshot_path`'s "restart interplay" docs), so this sweep only ever
/// removes entries matching [`crate::files::is_staged_temp_name`] — the
/// SAME naming convention every write-atomicity tier's temp file shares,
/// so this one pattern covers debris from `crate::files`'s helpers
/// regardless of which tier staged it. A real snapshot, named after a
/// session id alone, can never match that pattern (a session id contains
/// no `.tmp-` substring by construction: it is a UUID's hyphenated hex
/// form).
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
                    .is_some_and(crate::files::is_staged_temp_name);
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
///
/// # Cancellation safety (the other half of the same race)
///
/// The `attachments`-lock analysis above assumes this function's own task
/// runs to completion. That is NOT guaranteed: `handle_connection`'s
/// shutdown tail (`HANDLER_SHUTDOWN_TIMEOUT`) can `abort()` whatever
/// `JoinSet`-tracked task is calling this — the `StopSession` handler —
/// mid-flight. An aborted task's local `attachments` `MutexGuard` is
/// dropped the moment cancellation unwinds its stack, even while it was
/// still `.await`ing a write; if that write were a plain
/// `spawn_blocking`-based one, the DETACHED blocking closure it kicked off
/// keeps running to completion regardless (blocking tasks are not
/// cancelled by dropping their `JoinHandle`) — so the rename that
/// publishes the snapshot can complete AFTER a concurrent `DeleteSession`,
/// unblocked by the just-released lock, has already found no file to
/// remove and finished tearing the session down entirely. The result: an
/// orphaned, secret-bearing snapshot file for a session the system
/// considers completely gone, which nothing will ever clean up.
///
/// The fix is to run the whole lock-acquire-check-write critical section
/// inside its OWN `tokio::spawn`'d task, entirely independent of whatever
/// task calls this function. Awaiting that inner task's `JoinHandle` is
/// itself cancellable — if THIS function's caller gets aborted while
/// waiting, only that await is cut short; the inner task keeps running to
/// natural completion exactly as if nothing happened, because nothing
/// besides its own (never-aborted) `JoinHandle` can cancel it. The
/// `attachments` lock is therefore held for the write's ENTIRE real
/// duration no matter what happens to this function's caller.
///
/// `seam` is a value, not a `&dyn` reference, and must be `Copy + Send +
/// 'static` so it can be moved into both the detached outer task and the
/// `spawn_blocking` closure the actual write runs inside — see
/// `crate::files::FaultSeam`'s own docs for why nothing in this crate
/// otherwise needs a seam to survive a thread hop. Production calls this
/// with [`crate::files::RealFs`] (see the `StopSession` call site); tests
/// can inject a failure through this exact function.
async fn publish_alt_screen_snapshot<S>(
    sup: &Arc<Supervisor>,
    session_id: &str,
    bytes: &[u8],
    seam: S,
) where
    S: crate::files::FaultSeam + Copy + Send + 'static,
{
    let dir = sup.state_dir.join("snapshots");
    if let Err(e) = crate::ensure_private_dir(&dir).await {
        warn!(session = %session_id, error = %e, "creating the snapshots directory failed");
        return;
    }
    let path = snapshot_path(&sup.state_dir, session_id);

    let sup = Arc::clone(sup);
    let session_id = session_id.to_string();
    let bytes = bytes.to_vec();
    let inner = tokio::spawn(async move {
        let attachments = sup.attachments.lock().await;
        let still_exists = sup.sessions.lock().await.contains_key(&session_id);
        if !still_exists {
            // A concurrent delete already finished (see the delete-race
            // analysis above): nothing to write, and — because nothing
            // was ever written — nothing to clean up either.
            drop(attachments);
            return;
        }
        let write_result = tokio::task::spawn_blocking(move || {
            crate::files::overwrite_private_file_sync(&path, &bytes, &seam)
        })
        .await;
        match write_result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                warn!(session = %session_id, error = %e, "writing the alt-screen snapshot failed");
            }
            Err(join_err) => {
                warn!(session = %session_id, error = %join_err,
                    "alt-screen snapshot write task panicked");
            }
        }
        drop(attachments);
    });
    // If THIS await is cancelled, `inner` is entirely unaffected — that
    // is the whole point (see the cancellation-safety docs above).
    if let Err(join_err) = inner.await {
        warn!(error = %join_err, "alt-screen snapshot publish task panicked");
    }
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

/// Read back the alt-screen stop snapshot a replay should append for a
/// DEAD pane, or `None` when there is nothing to append.
///
/// Only READS it; the framing and sending live in
/// `Forwarder::send_dead_pane_snapshot`, which owns the bounded writer
/// queue this milestone introduced. The split exists so the snapshot's
/// two SOURCES (file, then pending map) stay one decision while the
/// send-side chunking follows the same stall-aware path as every other
/// byte a forwarder writes.
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
/// Consults the FILE first, then [`Supervisor::pending_snapshots`] —
/// see that field's own docs for the "attach lands between kill and
/// publish" window this fallback closes, and for the honesty argument
/// (why serving an in-flight capture is never showing stale or
/// misleading content). Both sources missing is the ordinary case for
/// most sessions (never stopped at all, or already cleaned up by a
/// delete) and is not logged; any actual read failure — on the file, not
/// its mere absence — degrades to the plain prefill with a warning rather
/// than failing the whole attach over a best-effort visibility extra.
async fn load_alt_screen_snapshot(sup: &Supervisor, session_id: &str) -> Option<Vec<u8>> {
    match read_bounded_snapshot_file(
        &snapshot_path(&sup.state_dir, session_id),
        MAX_ALT_SCREEN_SNAPSHOT_BYTES,
    )
    .await
    {
        Ok(Some(bytes)) => Some(bytes),
        Ok(None) => sup.pending_snapshots.lock().await.get(session_id).cloned(),
        Err(e) => {
            warn!(
                session = %session_id, error = %e,
                "reading the alt-screen snapshot failed; degrading to the plain prefill"
            );
            None
        }
    }
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
    /// In-memory mirror of this session's durable last-known outcome
    /// (`crate::store::LastOutcome`, PLAN_M3.md item 2), so the common
    /// case — a `ListSessions` reply for a session whose outcome has not
    /// changed — needs no database round trip at all, and so the sticky-
    /// terminal-state rule can be evaluated before deciding to write.
    ///
    /// A `std::sync::Mutex` inside the entry rather than another map on
    /// `Supervisor`: the outcome belongs to the session, and a per-entry
    /// lock is never contended by anything but that session's own
    /// observers. It is NOT part of the `attachments`-then-`sessions`
    /// lock-ordering rule (see the `Supervisor` struct's docs) because it
    /// is never held across an await or across either of those mutexes —
    /// every hold is a read or a store of one small value, which is also
    /// why a blocking mutex is safe here inside async code.
    ///
    /// The mirror is only ever assigned the outcome the store reports as
    /// COMMITTED (`Supervisor::record`, `SessionStore::transition`), never
    /// the value a caller intended to write: transitions are arbitrated
    /// inside the transaction, so a refusal or a merge with a concurrent
    /// writer's result must be what lands here too. A failed write leaves
    /// both sides on the old value and the next observation retries — the
    /// conservative direction, matching the crash-ordering rule.
    outcome: std::sync::Mutex<LastOutcome>,
}

/// The one live attachment a session may have (SPEC.md: at most one,
/// last attach wins). `notify` reaches the owning connection's writer so
/// a takeover can tell the old client it was detached.
struct ActiveAttach {
    channel: u32,
    notify: mpsc::Sender<Frame>,
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
    /// When this attachment's client asked for its output to stop
    /// (`ControlMsg::PauseOutput`), or `None` while output may flow. Read
    /// by the forwarder task.
    ///
    /// A `watch` rather than a flag plus a `Notify`: the forwarder needs
    /// both "what is the state right now" (to decide whether to pull from
    /// tmux at all) and "wake me when it changes" (to resume promptly),
    /// and a watch channel is exactly those two together with no window
    /// where a notification can be missed between checking and parking.
    ///
    /// Carrying the pause's START INSTANT rather than a bare bool is what
    /// makes [`STALL_DETACH_TIMEOUT`] a hard maximum. Every place the
    /// forwarder can block computes its deadline as `start + timeout`, so
    /// one continuous pause has exactly ONE deadline no matter how many
    /// chunks, phases, or wakeups happen under it. With a bool and a
    /// per-await timer, a client that drains just fast enough to keep the
    /// forwarder moving between chunks would reset the clock forever and
    /// never be detached.
    ///
    /// A repeated `PauseOutput` must NOT overwrite the stored start (see
    /// `set_attachment_paused`), for the same reason: pause spam would
    /// otherwise restart the hard maximum indefinitely.
    pause: watch::Sender<Option<tokio::time::Instant>>,
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
    /// Per-supervisor state purely so integration tests can shorten
    /// these; see [`SupervisorTimeouts`].
    timeouts: SupervisorTimeouts,
    /// Injection points; production builds carry the defaults. Held for
    /// the process lifetime because `serve` reloads sessions again and
    /// must consult the same boot-id source the constructor did — a second
    /// reload that read a different source would classify the same host
    /// two ways.
    seams: SupervisorSeams,
    /// This process's claim on the state directory, or `None` when another
    /// process holds it. See [`StateDirOwnership`]: without a claim this
    /// supervisor may not migrate the schema, may not write any
    /// reconciliation, and may not serve.
    ownership: Option<Arc<StateDirOwnership>>,
    /// Whether this supervisor may record what it observes.
    ///
    /// Two independent conditions have to hold, and both can be false for
    /// a supervisor that is otherwise perfectly able to serve requests:
    /// this process must hold the state directory's claim (the sessions
    /// belong to whoever does), and its last reload must have been able to
    /// READ the host's boot id (PLAN_M3.md item 2 — an exit recorded while
    /// the reboot detector is blind is an irreversible answer derived from
    /// a temporary condition). Re-evaluated by every reload, so a boot-id
    /// read that starts working again restores recording without a
    /// restart.
    ///
    /// Read by the request paths that observe exits, so it is an atomic
    /// rather than a plain bool: `serve`'s reload can flip it while
    /// handlers are already running.
    may_record: std::sync::atomic::AtomicBool,
    /// Collapses concurrent creates that share an intent key into one
    /// launch; see [`IntentLocks`] for why an in-process lock is the whole
    /// mechanism and what it is (and is not) responsible for.
    intent_locks: Arc<IntentLocks>,
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
        Self::new_with_exe_and_timeouts(state_dir, farhelm_exe, SupervisorTimeouts::default()).await
    }

    /// Like [`Self::new_with_exe`], but with the gone-not-slow timeouts
    /// supplied explicitly — the seam integration tests use to observe a
    /// stall detach or a wedged-peer teardown without waiting out a full
    /// production minute. See [`SupervisorTimeouts`].
    pub async fn new_with_exe_and_timeouts(
        state_dir: &Path,
        farhelm_exe: PathBuf,
        timeouts: SupervisorTimeouts,
    ) -> anyhow::Result<Arc<Supervisor>> {
        Self::new_with_seams(state_dir, farhelm_exe, timeouts, SupervisorSeams::default()).await
    }

    /// The one real constructor: everything above delegates here with
    /// production defaults.
    ///
    /// `seams` is what lets tests reach the two behaviors that are
    /// otherwise unreachable in a test process — a host reboot (a
    /// different boot id on the second construction) and a crash at an
    /// exact ordering boundary inside `create_session`. See
    /// [`SupervisorSeams`].
    pub async fn new_with_seams(
        state_dir: &Path,
        farhelm_exe: PathBuf,
        timeouts: SupervisorTimeouts,
        seams: SupervisorSeams,
    ) -> anyhow::Result<Arc<Supervisor>> {
        // 0700 on both: the socket and the launch specs (which hold full
        // agent command lines) live here. See ensure_private_dir. The
        // database opened just below relies on this same boundary for its
        // own confidentiality (see `SessionStore::open`'s docs), so it
        // must not be opened before this call.
        crate::ensure_private_dir(state_dir).await?;
        crate::ensure_private_dir(&state_dir.join("launch")).await?;
        // Items 6/24: a durable sentinel (`crate::files` module docs) is
        // only as durable as ITS OWN DIRECTORY'S directory-entry — a
        // reboot immediately after the very first run could otherwise
        // lose the just-created `launch/` entry under `state_dir`
        // entirely (the same rename-atomicity-is-metadata-only gap the
        // durability-bearing tier's own directory fsync exists to close,
        // one level up), silently discarding every sentinel this policy
        // promises to keep before a single one is ever written. Cheap and
        // idempotent enough to pay unconditionally on every startup
        // rather than only detecting "was `launch/` actually freshly
        // created this time."
        tokio::fs::File::open(state_dir)
            .await?
            .sync_all()
            .await
            .context("fsyncing state dir after ensuring launch/ exists")?;

        // Exclusivity FIRST, before the database is even opened: every
        // startup step below this line either mutates durable state (the
        // schema migration, the boot-id comparison, outcome
        // reconciliation) or decides something on its basis, and none of
        // it may happen to a state directory another supervisor owns. See
        // [`StateDirOwnership`] for the two concrete harms — a bricked
        // incumbent and a predecessor's live sessions recorded as ended —
        // and for why an unclaimed supervisor is still constructible
        // rather than fatal here.
        let ownership = StateDirOwnership::claim(state_dir)?;
        if ownership.is_none() {
            warn!(
                state_dir = %state_dir.display(),
                "another supervisor holds this state directory; starting read-only \
                 (no schema migration, no reconciliation, and serve will refuse)"
            );
        }

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
        let store =
            SessionStore::open(&state_dir.join("supervisor.db"), ownership.is_some()).await?;
        let tmux = TmuxDriver::new(state_dir);
        tmux.ensure_server().await?;

        let (sessions, may_record) =
            Self::reload_sessions(state_dir, &store, &tmux, &seams, ownership.is_some()).await?;

        Ok(Arc::new(Supervisor {
            state_dir: state_dir.to_path_buf(),
            tmux,
            store,
            sessions: Mutex::new(sessions),
            attachments: Mutex::new(HashMap::new()),
            pending_snapshots: Mutex::new(HashMap::new()),
            farhelm_exe,
            admission: Arc::new(tokio::sync::Semaphore::new(HANDLER_ADMISSION_PERMITS)),
            timeouts,
            seams,
            ownership,
            may_record: std::sync::atomic::AtomicBool::new(may_record),
            intent_locks: Arc::new(IntentLocks::default()),
        }))
    }

    /// Rebuild the in-memory session map from SQLite plus one bulk tmux
    /// probe: rows with a live pane become normal live `SessionEntry`s,
    /// rows whose pane tmux no longer knows become the restart-gap's
    /// terminal-less entry, and every transition this pass witnesses is
    /// reconciled into the durable outcome.
    ///
    /// Called twice, for two different reasons. The constructor calls it
    /// once so an embedder or test that only ever constructs a
    /// `Supervisor` — never calling `serve()` — still gets a populated
    /// map. `serve()` calls it AGAIN, before accepting any connection,
    /// because the first call can be stale: two supervisor processes can
    /// overlap during a handoff (the old one still running, the new one
    /// constructing), and the old process can create a session — an insert
    /// this process's earlier load already missed — and only then exit.
    /// Without a second load, this process would serve a map missing that
    /// session for its entire lifetime, since nothing else ever refreshes
    /// it wholesale. Replacing `self.sessions` wholesale is only safe
    /// where no attachment can yet exist against the entries being
    /// replaced — true at both call sites (construction, and pre-accept in
    /// `serve`) but not a general-purpose operation this type exposes.
    ///
    /// ## Boot classification (PLAN_M3.md item 2)
    ///
    /// Before a single row is probed, the host's current boot id is
    /// compared against the one the last supervisor stored:
    ///
    /// - **Different** — the host rebooted, so tmux took every terminal
    ///   with it and no probe can say anything about what happened. Every
    ///   still-live session becomes **interrupted**, in the same
    ///   transaction that stores the new id (`SessionStore::record_boot`
    ///   owns the argument for why that atomicity is not optional).
    ///   Already-exited sessions keep their status, codes, and
    ///   annotations.
    /// - **Same** — M2's per-row probing still rules, now also recording
    ///   what it observes so the NEXT reboot has ground to stand on.
    /// - **Absent stored id** — a database written before this milestone.
    ///   No evidence in either direction, so no reboot is claimed: the
    ///   same-boot path runs and the id is adopted from now on.
    /// - **Host publishes no boot id** (`Ok(None)`) — permanently
    ///   evidence-free, so the same-boot path runs forever and nothing is
    ///   ever interrupted. Nothing is stored either: there is no id to
    ///   store, and writing a placeholder would make a host that LATER
    ///   gains one look like it rebooted.
    /// - **Boot id unreadable this time** (`Err`) — the reload runs
    ///   DEGRADED: sessions are still classified in memory from what is
    ///   stored, but nothing durable is written at all. A transient read
    ///   failure must not be allowed to produce an irreversible answer
    ///   (recording exits for sessions a successful read might have
    ///   classified interrupted), and failing startup outright would take
    ///   a whole host's sessions offline over a `/proc` hiccup.
    ///
    /// `may_write` is the other half of that: a supervisor without this
    /// state directory's claim (see [`StateDirOwnership`]) classifies for
    /// its own in-memory map but writes nothing, because the sessions it
    /// is looking at belong to whichever process does hold the claim.
    ///
    /// The returned flag is `may_write` as this pass actually resolved it
    /// — the caller's permission AND a boot-id read that worked — which
    /// becomes [`Supervisor::may_record`] for the request paths that
    /// witness exits later. Returned rather than recomputed there because
    /// the degradation is decided here, and a request path re-deriving it
    /// could disagree with the reload that just ran.
    ///
    /// Probing is ONE `pane_states` call for the whole map rather than
    /// M2's per-row `has_session`: this pass needs each pane's dead flag
    /// and exit code (not merely whether the session exists) to record a
    /// witnessed exit with the true code a surviving dead pane still
    /// holds, and asking tmux once is both cheaper and the only way to get
    /// that answer at all. The transitions it produces are likewise
    /// committed in ONE store call rather than one autocommit per row.
    ///
    /// Failing to WRITE the reconciliation is logged and tolerated, never
    /// fatal: the map then holds what is actually durable, the next
    /// observation retries, and a supervisor that refuses to start over a
    /// bookkeeping write would strand every live session it was supposed
    /// to be reattaching. A failed boot-id write IS fatal — continuing
    /// would mean serving a reboot classification this process knows it
    /// could not record, which the next startup would then contradict.
    async fn reload_sessions(
        state_dir: &Path,
        store: &SessionStore,
        tmux: &TmuxDriver,
        seams: &SupervisorSeams,
        may_write: bool,
    ) -> anyhow::Result<(HashMap<String, Arc<SessionEntry>>, bool)> {
        let stored_boot = store.boot_id().await?;
        let current_boot = (seams.boot_id)();
        // A read failure degrades the whole pass to read-only; see the
        // docs above for why this is neither fatal nor the same-boot path.
        let mut may_write = may_write;
        let rebooted = match (&current_boot, &stored_boot) {
            (Err(e), _) => {
                warn!(
                    error = %format!("{e:#}"),
                    "could not read this host's boot id; classifying from stored state \
                     without recording anything this pass"
                );
                may_write = false;
                false
            }
            (Ok(Some(current)), Some(previous)) => current != previous,
            // Nothing stored, or nothing to compare: never a reboot claim.
            (Ok(_), _) => false,
        };
        // Loaded BEFORE any boot-id write, deliberately: a reboot's blanket
        // interrupt conversion and a launch sentinel's `Error` classification
        // must land in the SAME transaction (`SessionStore::record_boot`'s
        // docs on `sentinel_overrides`), which means the rows have to be in
        // hand, and their sentinels checked, before that transaction runs —
        // not discovered afterward, by which point a blanket-converted row
        // would already be the terminal `Interrupted` this policy can never
        // reclassify (`Transition::apply`'s catch-all).
        let mut rows = store.load_all().await?;
        if may_write
            && let Ok(Some(current)) = &current_boot
            && stored_boot.as_ref() != Some(current)
        {
            if rebooted {
                info!(
                    "host boot id changed since the last supervisor ran; \
                     sessions that were still live are now interrupted"
                );
            } else {
                info!("adopting this host's boot id without claiming a reboot");
            }
            let mut sentinel_overrides = HashMap::new();
            if rebooted {
                for row in &rows {
                    if matches!(
                        row.outcome,
                        LastOutcome::Launching | LastOutcome::Running | LastOutcome::StopRequested
                    ) {
                        match read_launch_sentinel(state_dir, &row.id).await {
                            Ok(Some(detail)) => {
                                sentinel_overrides.insert(row.id.clone(), detail);
                            }
                            Ok(None) => {}
                            // Loud propagation, not fall-through (item 1 of
                            // the review-swarm fix batch): this reload
                            // ABORTS before `record_boot` is ever called,
                            // so neither the new boot id nor any outcome
                            // conversion commits. Continuing here would
                            // risk exactly what PLAN_M3.md item 3 forbids —
                            // a corrupt/unreadable sentinel silently
                            // classifying its row `Interrupted` instead of
                            // the `Error` it might actually be — and this
                            // is the one call site where "defer just this
                            // row" is not available at all: the blanket
                            // interrupt conversion is one indivisible
                            // `UPDATE` across every live row, so it cannot
                            // partially apply while this one row's
                            // classification stays pending. The next
                            // startup sees the stored boot id UNCHANGED and
                            // retries the whole classification from
                            // scratch, once the file is readable again.
                            Err(e) => {
                                return Err(e.context(format!(
                                    "could not read session {}'s launch sentinel while \
                                     classifying a reboot; aborting this reload before \
                                     recording anything, so the next startup retries against \
                                     the same stored boot id rather than risking a durable \
                                     misclassification",
                                    row.id
                                )));
                            }
                        }
                    }
                }
            }
            store
                .record_boot(current, rebooted, sentinel_overrides.clone(), None)
                .await?;
            // Reflect what was just committed into this pass's in-memory
            // copy: the reconciliation loop below reads `rows` directly and
            // must see the POST-boot outcome, not the pre-boot one it was
            // loaded with a moment ago.
            if rebooted {
                for row in &mut rows {
                    if let Some(detail) = sentinel_overrides.get(&row.id) {
                        row.outcome = LastOutcome::Error {
                            detail: detail.clone(),
                        };
                    } else if matches!(
                        row.outcome,
                        LastOutcome::Launching | LastOutcome::Running | LastOutcome::StopRequested
                    ) {
                        row.outcome = LastOutcome::Interrupted;
                    }
                }
                // These overrides committed INSIDE `record_boot`'s own
                // transaction, never through `transitions`/`transition_many`
                // below, so their cleanup runs here — immediately after
                // that commit is confirmed — rather than being folded into
                // the shared cleanup further down, which only ever sees
                // what THIS pass's `transitions` vec itself proposed
                // (item 4 of the review-swarm fix batch).
                for id in sentinel_overrides.keys() {
                    cleanup_launch_artifacts(state_dir, id).await;
                }
            }
        }

        let pane_states = tmux.pane_states().await?;
        // Two passes over the rows: decide every transition against the
        // freshly loaded outcomes, commit them together, then build the
        // map from what was COMMITTED (which may differ from what this
        // pass proposed — see `SessionStore::transition_many`).
        let mut found_panes: HashMap<String, (String, PaneState)> = HashMap::new();
        let mut transitions = Vec::new();
        // Sentinels this pass itself proposes as `Error` (as opposed to a
        // row the boot-conversion branch above already resolved, whose
        // cleanup already ran) — id to detail, so the removal loop after
        // `transition_many`'s call can gate cleanup on the commit actually
        // having landed; see that loop for the lifecycle rationale.
        let mut sentinel_hits: HashMap<String, String> = HashMap::new();
        for row in &rows {
            // Idempotent cleanup (item 4 of the review-swarm fix batch): a
            // row already durably `Error` on load may still have a
            // lingering sentinel/spec file from a crash between an
            // EARLIER pass's commit and the cleanup that should have
            // followed it — including the reboot-override branch above,
            // whose own cleanup cannot rule out a crash on some PREVIOUS
            // startup. Harmless no-op when both files are already gone,
            // so this runs unconditionally rather than trying to prove it
            // is necessary first.
            if matches!(row.outcome, LastOutcome::Error { .. }) {
                cleanup_launch_artifacts(state_dir, &row.id).await;
                continue;
            }

            // Deterministic pane lookup, computed ONCE and reused by both
            // the sentinel branch below and the ordinary reconciliation
            // branch further down (item 6 of the review-swarm fix batch):
            // a multi-pane tmux session must resolve to the SAME pane
            // regardless of which branch is asking, and a second,
            // independent `iter().find()` in the sentinel branch used to
            // risk disagreeing with this `min_by` tie-break.
            let found = if row.pane.is_empty() {
                pane_states
                    .iter()
                    .filter(|(_, state)| state.session_name == row.tmux_name)
                    .min_by(|(a, _), (b, _)| a.cmp(b))
                    .map(|(pane, state)| (pane.clone(), state.clone()))
            } else {
                pane_states
                    .get(&row.pane)
                    .filter(|state| state.session_name == row.tmux_name)
                    .map(|state| (row.pane.clone(), state.clone()))
            };

            // A launch sentinel discovered now outranks every pane-based
            // inference (PLAN_M3.md item 3) — including "no pane was even
            // found", which is exactly the case a vanished tmux window
            // (no remain-on-exit, or a crash before the window was ever
            // created) produces — and, per item 3's addition 18, including
            // a row ALREADY recorded as an inferred `Interrupted` or
            // unannotated `Exited`: both are themselves only inferences
            // from a dead-or-vanished pane, the exact evidence class a
            // sentinel is defined to beat. `sentinel_could_still_apply`
            // (not `is_terminal()`) is what lets this reach those two
            // states rather than wrongly skipping them.
            if sentinel_could_still_apply(&row.outcome) {
                match read_launch_sentinel(state_dir, &row.id).await {
                    Ok(Some(detail)) => {
                        transitions.push((
                            row.id.clone(),
                            Transition::SentinelError {
                                detail: detail.clone(),
                                pane: found.as_ref().map(|(pane, _)| pane.clone()),
                            },
                        ));
                        sentinel_hits.insert(row.id.clone(), detail);
                        // The pane still rides into `found_panes` even
                        // though this row takes the sentinel branch (item
                        // 5/19 of the review-swarm fix batch): a
                        // `SessionEntry` built with no `terminal` at all
                        // breaks `Attach` and leaks the tmux session out
                        // from under `DeleteSession`'s kill sweep — the
                        // pane genuinely exists in tmux right now
                        // regardless of which durable outcome this pass
                        // assigns the row.
                        if let Some((pane, state)) = found {
                            found_panes.insert(row.id.clone(), (pane, state));
                        }
                        continue;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        // Loud propagation, not fall-through (item 1): this
                        // row's reconciliation is DEFERRED for this pass —
                        // no `Transition` is proposed for it at all, so a
                        // durable misclassification can never be committed
                        // from unreliable evidence — while the file
                        // survives for a later, repaired pass to read. Its
                        // pane still rides into `found_panes` so a
                        // genuinely alive session keeps reporting `Alive`
                        // regardless (`session_status`'s own live-probe
                        // precedence), but this pass proposes nothing for
                        // it either way.
                        error!(
                            session = %row.id, error = %format!("{e:#}"),
                            "could not read this session's launch sentinel; deferring its \
                             reconciliation this pass rather than risking a durable \
                             misclassification from pane state alone"
                        );
                        if let Some((pane, state)) = found {
                            found_panes.insert(row.id.clone(), (pane, state));
                        }
                        continue;
                    }
                }
            }
            let Some((pane, state)) = found else {
                info!(
                    session = %row.id,
                    "session's tmux pane no longer exists; listing without a terminal"
                );
                // Nothing to ask. A terminal outcome is left alone (it
                // already knows more than this), and so is a LAUNCHING
                // row: "no side effects found" is not evidence the agent
                // ran, which is what `Exited` would claim — that row stays
                // pending for a later pass's sentinel check (already tried
                // once above, this pass) or item 6's reservation to
                // resolve.
                if matches!(
                    row.outcome,
                    LastOutcome::Running | LastOutcome::StopRequested
                ) {
                    transitions
                        .push((row.id.clone(), Transition::ObservedExit { exit_code: None }));
                }
                continue;
            };
            if !row.outcome.is_terminal() {
                if state.dead {
                    // The pane outlived its process (remain-on-exit) and
                    // still holds the code — "exited with the code the
                    // surviving dead pane retains". A rediscovered pane
                    // rides the same commit as the outcome it evidences,
                    // so no crash window can leave the pane recorded under
                    // a still-`Running` row.
                    transitions.push((
                        row.id.clone(),
                        if row.pane.is_empty() {
                            Transition::RediscoveredExit {
                                pane: pane.clone(),
                                exit_code: state.exit_code,
                            }
                        } else {
                            Transition::ObservedExit {
                                exit_code: state.exit_code,
                            }
                        },
                    ));
                } else if row.outcome != LastOutcome::Running || row.pane != pane {
                    // Live, and either not yet confirmed, confirmed
                    // against a different pane, or carrying a stop intent
                    // whose kill sweep evidently never landed — the last
                    // being the reconciliation that keeps a crashed stop
                    // from annotating a session that is still running.
                    transitions.push((
                        row.id.clone(),
                        Transition::ConfirmRunning { pane: pane.clone() },
                    ));
                }
            }
            found_panes.insert(row.id.clone(), (pane, state));
        }

        // Sentinel lifecycle (PLAN_M3.md item 3): once an `Error` this
        // pass proposed has actually committed durably, both files it can
        // leave behind — the sentinel itself and the per-launch spec
        // (item 25 of the review-swarm fix batch: the shim's own
        // missing/malformed-spec paths, or a failed unlink, can leave the
        // credential-bearing spec stranded too) — are removed together
        // via `cleanup_launch_artifacts`. The durable outcome row is now
        // the truth — nothing ever reads either file again to answer "is
        // this session an error" (`session_status` consults the store,
        // never the filesystem) — and leaving them around serves no
        // reader while risking a future collision: PR8's relaunch reuses
        // this exact session id's spec/sentinel path
        // (`spec_path_for_session`), and stale files from THIS launch
        // sitting at that path could otherwise be misread as evidence
        // about a launch that hasn't happened yet. Folded into this same
        // successful arm (item 7) rather than a separate loop afterward:
        // a failed write means nothing durable exists yet, so the files
        // must survive for the next pass to retry against — cleaning them
        // up then would silently convert a real, still-unrecorded failure
        // into "no sentinel found" for good.
        let committed = if may_write && !transitions.is_empty() {
            match store.transition_many(transitions).await {
                Ok(committed) => {
                    for id in sentinel_hits.keys() {
                        if matches!(committed.get(id), Some(LastOutcome::Error { .. })) {
                            cleanup_launch_artifacts(state_dir, id).await;
                        }
                    }
                    committed
                }
                Err(e) => {
                    warn!(
                        error = %format!("{e:#}"),
                        "could not record this reload's reconciliation; \
                         keeping the outcomes already stored"
                    );
                    HashMap::new()
                }
            }
        } else {
            HashMap::new()
        };

        let mut sessions = HashMap::new();
        // Sessions this pass can say REACHED tmux at some point; the
        // provenance the reservation settlement below requires, and
        // deliberately narrower than "has a terminal outcome". See that
        // settlement's own comment for what each source proves.
        let mut launched: HashSet<String> = HashSet::new();
        for row in rows {
            let terminal = found_panes.remove(&row.id).map(|(pane, _)| Terminal {
                tmux_name: row.tmux_name.clone(),
                pane,
            });
            let outcome = committed.get(&row.id).cloned().unwrap_or(row.outcome);
            if terminal.is_some()
                || !row.pane.is_empty()
                || matches!(outcome, LastOutcome::Error { .. })
            {
                launched.insert(row.id.clone());
            }
            sessions.insert(
                row.id.clone(),
                Arc::new(SessionEntry {
                    info: SessionInfo {
                        id: row.id,
                        title: row.title,
                        cwd: row.cwd,
                        invocation: row.invocation,
                        // Placeholder only: `ListSessions` recomputes
                        // `status` fresh from tmux plus the recorded
                        // outcome on every reply (see `session_status`),
                        // so nothing ever reads this particular value —
                        // `Unknown` is simply the honest "not yet
                        // computed" default. The annotation beside it is
                        // recomputed from the same place and for the same
                        // reason.
                        status: SessionStatus::default(),
                        annotation: None,
                        // No writer yet — PLAN_M3.md item 9 lands it;
                        // `FreshOnly` is the honest "nothing has ever
                        // populated this" value.
                        restart_offer: RestartOffer::default(),
                    },
                    terminal,
                    outcome: std::sync::Mutex::new(outcome),
                }),
            );
        }

        // Create reservations (PLAN_M3.md item 6), reconciled from the
        // same verdicts this pass just reached rather than by a second,
        // parallel probe: a pending reservation whose launching row this
        // pass resolved is the SAME case a retry sees from the other side
        // (`reserved_launch_evidence`, which carries the full rationale for
        // the provenance rule both share). Settling here is what makes the
        // crash windows survivable across a RESTART with no retry in sight
        // — by the time a client retries, the answer is already recorded.
        //
        // Settlement requires PROVENANCE, not merely a non-launching
        // status. A pane (recorded, or found by this pass) means something
        // saw this session in tmux; an `Error` outcome means the shim ran.
        // `Interrupted` proves nothing on its own — the reboot conversion
        // blankets `Launching` rows too, so a create that crashed before
        // ever reaching tmux comes back from a reboot looking terminal, and
        // settling THAT as created would replay a session that never
        // existed and can never run. Those, like every other reservation
        // whose launch left no trace, stay pending: only a retry can create
        // the session the client asked for, under the identities the
        // reservation already holds.
        if may_write {
            match store.pending_reservations().await {
                Ok(pending) => {
                    let settled: Vec<Settlement> = pending
                        .into_iter()
                        .filter_map(|reservation| {
                            launched
                                .contains(&reservation.session_id)
                                .then_some(Settlement {
                                    intent_key: reservation.intent_key,
                                    session_id: reservation.session_id,
                                    outcome: ReservationOutcome::Created,
                                })
                        })
                        .collect();
                    if let Err(e) = store.settle_reservations(settled).await {
                        warn!(
                            error = %format!("{e:#}"),
                            "could not settle this reload's create reservations; \
                             a retry of those intents will reconcile them itself"
                        );
                    }
                }
                // Tolerated like the reconciliation write above, and for
                // the same reason: a supervisor that refused to start over
                // idempotency bookkeeping would strand every live session
                // it was supposed to be reattaching. The cost of skipping
                // it is bounded — a retry reaching `create_session` does
                // the same reconciliation itself.
                Err(e) => warn!(
                    error = %format!("{e:#}"),
                    "could not load pending create reservations; leaving them for a retry"
                ),
            }
        }
        Ok((sessions, may_write))
    }

    /// Whether this supervisor holds its state directory's claim (see
    /// [`StateDirOwnership`]) — that is, whether it may migrate the schema,
    /// write reconciliation, and serve at all.
    ///
    /// Public for the restart tests, and it is not a convenience there but
    /// a correctness check on the TEST: a "restarted" supervisor
    /// constructed while its predecessor is still alive starts read-only
    /// and reconciles nothing, so a handoff test that forgot to release the
    /// old one would exercise a path production never takes and would pass
    /// for the wrong reason. Also honest diagnostics for an embedder.
    pub fn owns_state_dir(&self) -> bool {
        self.ownership.is_some()
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
        // session maps. The lock itself is now taken in the CONSTRUCTOR
        // (see `StateDirOwnership`, and PLAN_M3.md item 2's requirement
        // that nothing durable be touched before it is held), so what
        // remains here is claiming the right to serve under it — which is
        // also what refuses a second `serve` inside one process, where the
        // file lock alone cannot tell the two apart.
        if !self
            .ownership
            .as_ref()
            .is_some_and(|owned| owned.begin_serving())
        {
            anyhow::bail!(
                "a supervisor is already running against {} \n\
                 (SPEC.md allows at most one supervisor per user per host)",
                self.state_dir.display()
            );
        }
        // Reload the session map now that the right to serve is actually
        // held. The load the constructor already did can be stale: this
        // process's construction can overlap a still-running predecessor
        // during a handoff, which can insert a session (and exit,
        // releasing the lock) after that first load already ran. Nothing
        // else in this process ever refreshes the map wholesale, so
        // without this second pass such a session would be permanently
        // missing from `sessions` for this process's entire lifetime.
        // Safe to replace outright here: no connection has been accepted
        // yet, so no attachment can exist against any entry this replaces.
        let (sessions, may_record) =
            Self::reload_sessions(&self.state_dir, &self.store, &self.tmux, &self.seams, true)
                .await?;
        *self.sessions.lock().await = sessions;
        self.may_record
            .store(may_record, std::sync::atomic::Ordering::SeqCst);
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
        // Sweep launch-dir debris orphaned by a previous run. Deliberately
        // AFTER the bind above: the bind is what proves this process is
        // the state dir's one supervisor. Sweeping in the constructor let
        // a second `supervisor run` destroy the live supervisor's in-
        // flight specs and only then bail on the exclusivity check.
        // Deliberately AFTER the session-map reload just above too: this
        // sweep needs to know which sessions still exist to avoid
        // unlinking a spec a surviving shim might still read (see
        // `sweep_launch_dir`'s own docs).
        let known_sessions: std::collections::HashSet<String> =
            self.sessions.lock().await.keys().cloned().collect();
        sweep_launch_dir(&self.state_dir.join("launch"), &known_sessions).await;
        sweep_snapshot_temp_files(&self.state_dir).await;
        sweep_tmux_config_temp_files(&self.state_dir).await;
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
    ///
    /// ## Idempotency (PLAN_M3.md item 6)
    ///
    /// `claim` is the client's assertion that this request and any retry of
    /// it are ONE intended create. Without it nothing below changes at all:
    /// every call is its own create, exactly as before M3. With it, this
    /// function is a state machine over a durable reservation, run as a
    /// whole under that key's lock ([`IntentLocks`]) so concurrent retries
    /// collapse instead of racing:
    ///
    /// - **A settled reservation, same fingerprint** — replay: the session
    ///   it created (or the gone-error, if that session has since been
    ///   deleted), or the exact error the first attempt reported, kind
    ///   included.
    /// - **A reservation with a different fingerprint** — the client reused
    ///   a key for a different request. That is a client bug, never a
    ///   merge, so it is refused with `Conflict`.
    /// - **A pending reservation, same fingerprint** — reconcile against
    ///   reality using the identities the reservation carries. Evidence of
    ///   a launch means finish the bookkeeping and replay; positive
    ///   evidence of NO launch means perform the create under that same
    ///   reservation and its already-assigned ids.
    /// - **No reservation** — a genuinely new intent: claim it in the same
    ///   transaction as the launching row and launch.
    ///
    /// ## Why the lookup precedes validation
    ///
    /// Validation reads the FILESYSTEM, which changes underneath a
    /// reservation that does not. A replay must answer with what the intent
    /// already resolved to, not with what the world happens to look like
    /// now: a settled key whose working directory has since been removed
    /// must still replay its session (the session is running in a directory
    /// that no longer exists — an unusual state, but a real one), and a
    /// changed-fingerprint request must be refused as a key reuse even when
    /// its own cwd is nonsense. So the reservation lookup runs FIRST, and
    /// validation runs only for the two branches that are about to touch
    /// the world: a new intent and a pending relaunch.
    ///
    /// A keyed request refused BY that validation is itself an outcome and
    /// is recorded as one — acceptance 7's "a failed create replays its
    /// original error" has no precondition exception, so a retry of the
    /// same key gets the same "working directory does not exist" answer
    /// rather than a different one derived from a filesystem that changed
    /// in between.
    async fn create_session(
        &self,
        cwd: &str,
        invocation: &str,
        title: Option<String>,
        cols: u16,
        rows: u16,
        claim: Option<IntentClaim>,
    ) -> anyhow::Result<SessionInfo> {
        let Some(claim) = claim else {
            let request = Self::validate_create(cwd, invocation, title, cols, rows).await?;
            return self
                .launch_session(request, Reserved::Unkeyed(new_session_identity()))
                .await;
        };
        // Held for the whole of the rest of this create — lookup, launch,
        // and outcome settlement alike — so a concurrent retry of the same
        // intent waits for this one's answer instead of racing it.
        let _intent = self.intent_locks.claim(&claim.intent_key).await;
        let existing = self
            .store
            .reservation(&claim.intent_key)
            .await
            .context("reading the create reservation for this intent key")?;
        let reserved = match existing {
            Some(reservation) => match self.resolve_reservation(reservation, &claim).await {
                Resolution::Answer(answer) => return answer,
                Resolution::Relaunch(reservation) => Reserved::Retry(reservation),
            },
            None => Reserved::New {
                claim: claim.clone(),
                identity: new_session_identity(),
            },
        };
        let request = match Self::validate_create(cwd, invocation, title, cols, rows).await {
            Ok(request) => request,
            Err(refusal) => return self.record_refused_create(&reserved, refusal).await,
        };
        self.launch_session(request, reserved).await
    }

    /// Everything checkable before the world is touched: the working
    /// directory is usable, the invocation parses into an argv, and the
    /// title is defaulted from the cwd when the caller omitted one.
    ///
    /// Split out of `create_session` because the idempotency state machine
    /// must be able to run its reservation lookup WITHOUT it (see that
    /// function's docs on ordering) and then apply it to only the branches
    /// that are about to launch something. Associated rather than a method
    /// because it touches no supervisor state at all.
    async fn validate_create<'a>(
        cwd: &'a str,
        invocation: &'a str,
        title: Option<String>,
        cols: u16,
        rows: u16,
    ) -> anyhow::Result<LaunchRequest<'a>> {
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
        Ok(LaunchRequest {
            cwd,
            invocation,
            argv,
            title,
            cols,
            rows,
        })
    }

    /// Turn an existing reservation into either this request's answer or
    /// the decision to relaunch under it.
    ///
    /// The fingerprint is checked before anything else, including before
    /// the outcome: a reused key is refused whatever the original intent
    /// went on to do, and answering about the ORIGINAL session would hand
    /// the caller a session it never asked for.
    async fn resolve_reservation(
        &self,
        reservation: Reservation,
        claim: &IntentClaim,
    ) -> Resolution {
        if reservation.fingerprint != claim.fingerprint {
            return Resolution::Answer(Err(RequestError::new(
                ErrorKind::Conflict,
                format!(
                    "intent key {} was already used for a different create request; \
                     a reused key is a client bug rather than a merge, so this request \
                     is refused — send a new key for a new request",
                    truncate_for_error(&claim.intent_key)
                ),
            )
            .into()));
        }
        match &reservation.outcome {
            // Settled either way: the answer is whatever was recorded, and
            // `answer_from` is the one place that decides what that means
            // so every caller of it agrees (`ReservationOutcome::Failed`'s
            // own docs on why the kind rides along with the message).
            ReservationOutcome::Created | ReservationOutcome::Failed { .. } => {
                Resolution::Answer(self.answer_from(&reservation).await)
            }
            ReservationOutcome::Pending => {
                match self.reserved_launch_evidence(&reservation).await {
                    LaunchEvidence::Present => {
                        Resolution::Answer(self.settle_and_replay(&reservation).await)
                    }
                    LaunchEvidence::Absent => Resolution::Relaunch(Box::new(reservation)),
                    // Neither relaunch nor replay: this process cannot tell
                    // which is true, and both wrong answers are permanent (a
                    // duplicate agent, or a success that never ran). The
                    // reservation stays pending, so a later retry — or the next
                    // reload, once whatever failed is readable again — resolves
                    // it against evidence instead of a guess.
                    LaunchEvidence::Unresolved(why) => {
                        Resolution::Answer(Err(why.context(format!(
                            "cannot tell whether intent key {}'s create ever launched, so it is \
                     neither replayed nor retried; try again once the cause is cleared",
                            truncate_for_error(&claim.intent_key)
                        ))))
                    }
                }
            }
        }
    }

    /// What is durably known about whether a reserved launch ever reached
    /// tmux (PLAN_M3.md item 6's "the reserved identities' side effects").
    ///
    /// The bias is deliberate and asymmetric: relaunching requires POSITIVE
    /// evidence of absence, and everything else counts as present. The two
    /// wrong answers are not equally bad — a wrongly-replayed session hands
    /// the user a session that never ran (visible, recoverable, and
    /// classified honestly by the status rules), while a wrongly-relaunched
    /// one starts a second agent beside a first that is quietly still
    /// running, which is the exact duplicate SPEC.md forbids and which
    /// nothing downstream can detect.
    ///
    /// Three independent sources, any one of which is enough to say a
    /// launch happened:
    ///
    /// - **The durable row.** A recorded pane means something once saw this
    ///   session in tmux. An outcome past `Launching` means the same, with
    ///   ONE exception that is exactly why the pane is checked separately:
    ///   `Interrupted` is written by the reboot conversion, which blankets
    ///   `Launching` rows too — so an interrupted row with no pane is a
    ///   create that may never have launched at all, and treating the
    ///   status alone as provenance would replay a session that never
    ///   existed.
    /// - **The launch sentinel.** The shim wrote it, so the shim ran, so
    ///   tmux started something — even when no pane was ever recorded.
    /// - **tmux itself, right now.** Asked at DECISION time rather than
    ///   inferred from the session map, because the map is a snapshot taken
    ///   at reload: a create that completed after that snapshot (a
    ///   cancelled request whose work continued) is invisible to it and
    ///   very visible to tmux.
    ///
    /// A source that ERRORS is never read as absence: an unreadable
    /// sentinel or an unreachable tmux is exactly the situation in which a
    /// wrong relaunch is most likely, so it yields `Unresolved`.
    async fn reserved_launch_evidence(&self, reservation: &Reservation) -> LaunchEvidence {
        match self.store.session(&reservation.session_id).await {
            Ok(Some(row)) => {
                if !row.pane.is_empty()
                    || !matches!(
                        row.outcome,
                        LastOutcome::Launching | LastOutcome::Interrupted
                    )
                {
                    return LaunchEvidence::Present;
                }
            }
            Ok(None) => {}
            Err(e) => {
                return LaunchEvidence::Unresolved(e.context("reading the reserved session row"));
            }
        }
        match read_launch_sentinel(&self.state_dir, &reservation.session_id).await {
            Ok(Some(_)) => return LaunchEvidence::Present,
            Ok(None) => {}
            Err(e) => {
                return LaunchEvidence::Unresolved(
                    e.context("reading the reserved launch's sentinel"),
                );
            }
        }
        match self.tmux.has_session(&reservation.tmux_name).await {
            Ok(true) => LaunchEvidence::Present,
            Ok(false) => LaunchEvidence::Absent,
            Err(e) => LaunchEvidence::Unresolved(e.context(format!(
                "asking tmux whether the reserved session {} exists",
                reservation.tmux_name
            ))),
        }
    }

    /// Record that this intent's session exists, then replay it.
    ///
    /// A failed settlement is logged rather than propagated: the session
    /// exists and returning it is the correct answer, and the reservation
    /// simply stays pending for the next retry (or the next reload) to
    /// settle. Monotonic settlement makes that harmless — the eventual
    /// record says the same thing whenever it lands.
    async fn settle_and_replay(&self, reservation: &Reservation) -> anyhow::Result<SessionInfo> {
        if let Err(e) = self
            .store
            .settle_reservations(vec![Settlement {
                intent_key: reservation.intent_key.clone(),
                session_id: reservation.session_id.clone(),
                outcome: ReservationOutcome::Created,
            }])
            .await
        {
            warn!(
                session = %reservation.session_id, error = %format!("{e:#}"),
                "could not record that this intent key's session was created; \
                 replaying it anyway and leaving the reservation for a later pass"
            );
        }
        self.replay_created_session(reservation).await
    }

    /// Replay a reservation whose session was created — or say honestly
    /// that it is gone (PLAN_M3.md item 6's tombstone rule).
    ///
    /// The STORE, not the session map, is what decides whether the session
    /// still exists. The map is a mirror the delete handler updates only
    /// after its own commit, so a replay landing inside that window would
    /// otherwise return a live-looking success for a session whose row was
    /// already gone — a dead id handed to a caller that will attach to
    /// nothing.
    ///
    /// The gone case is a `Conflict`, not a `NotFound`, and the distinction
    /// is deliberate. Nothing the client asked about is missing: the intent
    /// key was found, and its answer is that the key is spent. `NotFound`
    /// (a 404 at the helm) on a create POST reads as "no such endpoint or
    /// resource" and invites a client to retry as if it had asked for the
    /// wrong thing, while `Conflict` (409) is the same "this identifier
    /// already means something else" this handler returns for a fingerprint
    /// mismatch — the two cases really are one rule: the key is used up.
    /// The message names what happened, because a bare conflict would
    /// otherwise be indistinguishable from a key-reuse bug.
    ///
    /// The replayed `SessionInfo` is rebuilt from the stored row with the
    /// same create-time placeholders a first attempt's reply carries
    /// (`status: Unknown`, no annotation, the default restart offer) — a
    /// replay is the same answer, so it must have the same shape.
    async fn replay_created_session(
        &self,
        reservation: &Reservation,
    ) -> anyhow::Result<SessionInfo> {
        let row = self
            .store
            .session(&reservation.session_id)
            .await
            .context("reading the session this intent key created")?;
        match row {
            Some(row) => Ok(SessionInfo {
                id: row.id,
                title: row.title,
                cwd: row.cwd,
                invocation: row.invocation,
                status: SessionStatus::Unknown,
                annotation: None,
                restart_offer: RestartOffer::default(),
            }),
            None => Err(RequestError::new(
                ErrorKind::Conflict,
                format!(
                    "intent key {} already created session {}, which has since been deleted; \
                     it will not be recreated under the same key — send a new key to create \
                     a new session",
                    truncate_for_error(&reservation.intent_key),
                    truncate_for_error(&reservation.session_id)
                ),
            )
            .into()),
        }
    }

    /// Record a keyed create refused by validation, so the retry replays
    /// the refusal instead of re-deriving it from a changed filesystem.
    ///
    /// Returns the error the caller should report, which is NOT always the
    /// one passed in: when the refusal itself could not be recorded, the
    /// caller is told that instead. Presenting the original error alone
    /// would claim a durability this create does not have — the client
    /// would reasonably conclude the key is spent, when in fact a retry can
    /// still do something else entirely.
    async fn record_refused_create(
        &self,
        reserved: &Reserved,
        refusal: anyhow::Error,
    ) -> anyhow::Result<SessionInfo> {
        let kind = error_kind(&refusal);
        let message = format!("{refusal:#}");
        let recorded = match reserved {
            // No key: nothing to record, and today's behavior exactly.
            Reserved::Unkeyed(_) => return Err(refusal),
            Reserved::New { claim, identity } => {
                self.store
                    .record_failed_intent(
                        claim.clone(),
                        &identity.session_id,
                        &identity.tmux_name,
                        kind,
                        &message,
                    )
                    .await
            }
            // A relaunch refused by validation settles the reservation it
            // was about to take over — and takes the stranded launching row
            // with it, in the same transaction. That row exists because a
            // previous attempt crashed after recording it, and this
            // request only got as far as validation BECAUSE the evidence
            // said nothing was ever launched under it. With the intent now
            // closed, nothing will ever reconcile that row again, so
            // leaving it would strand a session in the list that can never
            // resolve.
            Reserved::Retry(reservation) => {
                let removed = self
                    .store
                    .delete_session(
                        &reservation.session_id,
                        Some(Settlement {
                            intent_key: reservation.intent_key.clone(),
                            session_id: reservation.session_id.clone(),
                            outcome: ReservationOutcome::Failed { kind, message },
                        }),
                    )
                    .await;
                if removed.is_ok() {
                    self.sessions.lock().await.remove(&reservation.session_id);
                }
                removed
            }
        };
        Err(match recorded {
            Ok(()) => refusal,
            Err(e) => unrecorded_outcome(refusal, e),
        })
    }

    /// Perform one launch: the durable launching record, the launch spec,
    /// the tmux session, and the confirmation — under whatever the
    /// reservation table owes for it (see [`Reserved`]).
    ///
    /// Split out of `create_session` so that every one of the many failure
    /// exits below settles the reservation exactly once, in one place,
    /// rather than each rollback path having to remember to. Callers have
    /// already validated `cwd` and parsed `argv`.
    ///
    /// ## Which failures settle, and which stay pending
    ///
    /// Only a failure that CONFIRMED nothing is running settles `Failed`,
    /// and it does so where it happens rather than here: the rollback
    /// paths settle atomically with the row removal they already perform
    /// (`Supervisor::abandon_launching_record`), because a rollback whose
    /// settlement did not commit would leave a reservation pointing at a
    /// row that no longer exists. This wrapper therefore settles ONLY on
    /// success, and every other failure deliberately leaves the
    /// reservation PENDING.
    ///
    /// That is the important half: a failure that had to retain the
    /// launching row because an agent may be alive under it must not
    /// record `Failed`, because that would tell every later retry the
    /// intent is closed while a real agent kept running under a row
    /// nothing would ever reconcile — hiding it until the next restart.
    /// Pending is exactly the "side effects may be present → reconcile"
    /// state the retry path exists to resolve. The same goes for a
    /// relaunch that never started at all: nothing about the intent was
    /// decided, so nothing about it should be recorded.
    async fn launch_session(
        &self,
        request: LaunchRequest<'_>,
        reserved: Reserved,
    ) -> anyhow::Result<SessionInfo> {
        let result = self.launch_reserved(request, &reserved).await;
        // The last crash window item 6 names, and the one acceptance 7
        // describes directly: the session durably exists, but the intent
        // table does not yet know it. Returning here — before the
        // settlement below — leaves exactly the state a real crash would,
        // for the next reload (or the next retry) to reconcile.
        if result.is_ok() {
            self.simulate_crash(CreateStage::BeforeOutcome)?;
        }
        let outcome = match &result {
            Ok(_) => ReservationOutcome::Created,
            // No failure settles here, and the docs above say why for
            // each class: a crash never got to write an outcome at all
            // ([`SimulatedCrash`]), a retained-evidence failure must stay
            // reconcilable, and a confirmed-absence failure has already
            // settled itself atomically with its own rollback. What
            // reaches this point is therefore either a success or a
            // failure that must not settle.
            Err(_) => return result,
        };
        let Some(settlement) = reserved.settlement(outcome) else {
            return result;
        };
        if let Err(e) = self.store.settle_reservations(vec![settlement]).await {
            // Not fatal: the session genuinely exists, and failing the
            // create over a bookkeeping write would tell the caller a lie
            // about the world. The reservation stays pending, which the
            // next reload or retry reconciles against what actually
            // happened — and finds this very session.
            warn!(
                error = %format!("{e:#}"),
                "could not record this create's outcome against its intent key; \
                 a retry will reconcile it against what actually happened"
            );
        }
        result
    }

    /// The launch itself; see [`Supervisor::launch_session`], which wraps
    /// this to settle the reservation on the paths that may settle it.
    async fn launch_reserved(
        &self,
        request: LaunchRequest<'_>,
        reserved: &Reserved,
    ) -> anyhow::Result<SessionInfo> {
        let LaunchRequest {
            cwd,
            invocation,
            argv,
            title,
            cols,
            rows,
        } = request;
        let id = reserved.session_id().to_string();
        let tmux_name = reserved.tmux_name().to_string();
        if let Reserved::Retry(reservation) = reserved {
            // Clear the interrupted attempt's leftovers before reusing its
            // identities, FAIL-CLOSED: this id's spec and sentinel paths
            // are derived from the id alone (`spec_path_for_session`), so
            // anything left at them would sit exactly where this launch's
            // own files go — and a sentinel that survived would be read as
            // evidence about a launch that has not happened yet. A cleanup
            // that cannot be confirmed therefore ABORTS the relaunch
            // rather than proceeding: destroying evidence is bad, but
            // launching on top of evidence this process could not remove
            // is worse.
            let spec_path = crate::launch::spec_path_for_session(&self.state_dir, &id);
            let status_path = crate::launch::status_path_for_spec(&spec_path);
            for (path, what) in [
                (&status_path, "the interrupted attempt's launch sentinel"),
                (&spec_path, "the interrupted attempt's launch spec"),
            ] {
                if let Err(e) = remove_fail_closed(path, what).await {
                    return Err(anyhow::anyhow!(
                        "not relaunching intent key {}: {e}; the intent stays pending, so a \
                         retry can resolve it once the cause is cleared",
                        truncate_for_error(&reservation.intent_key)
                    ));
                }
            }
            // The atomic re-check of the decision that got us here: the
            // evidence was gathered a moment ago, and a delete or a
            // late-landing launch since then must win over it. See
            // `SessionStore::restart_pending_launch`.
            let row = StoredSession {
                id: id.clone(),
                title: title.clone(),
                cwd: cwd.to_string(),
                invocation: invocation.to_string(),
                tmux_name: tmux_name.clone(),
                pane: String::new(),
                outcome: LastOutcome::Launching,
            };
            match self
                .store
                .restart_pending_launch(row, &reservation.intent_key)
                .await
                .context("taking over the interrupted attempt's reservation")?
            {
                RetryClaim::Acquired => {}
                RetryClaim::Resolved(settled) => return self.answer_from(&settled).await,
                RetryClaim::Launched => return self.settle_and_replay(reservation).await,
            }
            // The in-memory mirror follows the row it mirrors, and it is
            // also what serializes this relaunch against `StopSession` and
            // `DeleteSession`: both resolve the session through this map,
            // so while a relaunch is in flight they answer `NotFound`
            // rather than tearing down a launch that is half-built. The
            // residual window — a delete that read the map just before
            // this removal — is closed at the other end, where the
            // confirmation below finds its row already gone.
            self.sessions.lock().await.remove(&id);
        }

        // Named after the SESSION id, not a separate per-launch id — a
        // deliberate, minimal choice made while settling PLAN_M3.md item 5's
        // write-atomicity policy, recorded here for whoever builds item 3's
        // stale-sentinel supersession or item 9's restart: today a session
        // launches exactly once for its entire life (no restart exists yet),
        // so the session id already IS the one and only launch's identity —
        // introducing a distinct launch id now, with nothing yet tracking
        // durably which launch is "current" for a session (that generation-
        // tracking is item 2/3's territory), would just be a second id with
        // no consumer. What keeping this name buys for free: because
        // `write_durable_sync` always publishes via `rename` (never
        // truncate-in-place), a FUTURE relaunch reusing this exact path
        // inherits atomic supersession of a stale sentinel at no extra
        // design cost — no new file-naming scheme to migrate to later.
        // What it does NOT buy: PROACTIVELY clearing a leftover sentinel
        // from a previously failed launch before a relaunch's process even
        // starts (so a successful relaunch doesn't leave a stale error
        // sentinel behind for nothing to overwrite) is still open work,
        // deliberately left to whichever milestone item adds relaunching at
        // all.
        // The durable launching record, committed BEFORE any external side
        // effect exists (PLAN_M3.md item 2). The ordering is the whole
        // point and it is the INVERSE of M2's, which inserted the row only
        // after tmux had the session: a crash between the two must leave
        // evidence that a launch was attempted — a launching row whose
        // side effects reload then goes looking for — rather than either
        // silence (M2: an unlisted running agent) or, once restart exists,
        // the PREVIOUS run's outcome still standing over a session that
        // has since been relaunched.
        //
        // A failure here fails the create with nothing to clean up, which
        // is exactly why this is the first step. A first-time intent key
        // (PLAN_M3.md item 6) is claimed in this SAME transaction — the
        // launching row IS the generation the reservation carries, and
        // committing them together is what leaves no window in which one
        // exists without the other (`SessionStore::insert_session`'s docs
        // spell out both failure modes that would otherwise open).
        //
        // A RETRY skips this step entirely: its reservation was claimed by
        // the attempt that crashed and its launching row was committed by
        // the takeover above (`restart_pending_launch`), which had to write
        // it there anyway to make the takeover atomic. Inserting again here
        // would collide with that row on the primary key it deliberately
        // reuses.
        if let Reserved::Unkeyed(_) | Reserved::New { .. } = reserved {
            let claim = match &reserved {
                Reserved::New { claim, .. } => Some(claim.clone()),
                _ => None,
            };
            let claimed = self
                .store
                .insert_session(
                    StoredSession {
                        id: id.clone(),
                        title: title.clone(),
                        cwd: cwd.to_string(),
                        invocation: invocation.to_string(),
                        tmux_name: tmux_name.clone(),
                        // Not known until tmux has created the session —
                        // see `StoredSession::pane`.
                        pane: String::new(),
                        outcome: LastOutcome::Launching,
                    },
                    claim,
                )
                .await
                .context("recording new session in the database")?;
            if let Claimed::TakenBy(winner) = claimed {
                // Someone else holds this key. Nothing was committed, so
                // there is nothing to roll back and nothing of ours to
                // settle — the honest answer is the WINNER's, which is what
                // a replay of that key would have returned had this request
                // arrived a moment later. Only reachable past the per-key
                // lock, i.e. from a second process, which the
                // state-directory claim already excludes; handled rather
                // than asserted because "cannot happen" is a poor thing to
                // stake a duplicate agent on.
                return self.answer_from(&winner).await;
            }
        }
        // Deliberately BEFORE the cleanup-bearing paths below: a simulated
        // crash must leave the launching row (and its reservation) exactly
        // as a real one would, with nothing tidied up after it.
        self.simulate_crash(CreateStage::AfterRecord)?;

        let spec_path = crate::launch::spec_path_for_session(&self.state_dir, &id);
        // Derived the SAME way the shim derives it from its own copy of
        // `spec_path` (`launch::status_path_for_spec`) — never computed
        // independently here — so the two sides can never disagree about
        // where a launch failure gets recorded, including for the failure
        // classes (missing/malformed spec) where the shim never gets to
        // read this struct's own `status_file` field at all.
        let status_file_path = crate::launch::status_path_for_spec(&spec_path);
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
        // Serialized before the write so the (practically impossible)
        // encoding failure shares the write's rollback path rather than
        // returning past it via `?` and stranding the launching row.
        let spec_bytes = match serde_json::to_vec(&spec) {
            Ok(bytes) => bytes,
            Err(e) => {
                return Err(self
                    .abandon_launching_record(
                        reserved,
                        anyhow::Error::new(e).context("encoding launch spec"),
                    )
                    .await);
            }
        };
        if let Err(e) = crate::write_private_file(&spec_path, &spec_bytes).await {
            // Nothing external happened yet — the spec is the FIRST side
            // effect and it did not land — so the launching row is
            // provably describing nothing and is rolled back. A crash
            // here would leave it instead, which is the case reload
            // reconciles; this path can do better because the process is
            // still alive to know.
            return Err(self
                .abandon_launching_record(
                    reserved,
                    anyhow::Error::new(e).context("writing launch spec"),
                )
                .await);
        }

        let shell = resolve_shell().await;
        let cmd = window_command(&shell, &self.farhelm_exe, &spec_path);
        let pane = match self
            .tmux
            .create_session(&tmux_name, cwd, cols, rows, &cmd)
            .await
        {
            Ok(pane) => pane,
            Err(e) => {
                // A tmux failure is AMBIGUOUS in a way the spec write is
                // not: `new-session` can fail after the session already
                // exists (a lost reply, a timeout mid-command), so
                // deleting the row on the strength of the error alone
                // would orphan a running agent — no row, no id, nothing
                // left that knows to reap it. Ask tmux instead, and only
                // roll back on a CONFIRMED absence; an ambiguous or failed
                // probe keeps the row, which is the only record anything
                // will ever have of that launch.
                let mut error = e.context("creating the session's tmux session");
                match self.tmux.has_session(&tmux_name).await {
                    Ok(false) => {
                        // The shim unlinks the spec once it has read it,
                        // so a launch that never happened would strand a
                        // file holding the agent's full command line —
                        // credentials included — with nothing left to
                        // clean it up. Removed BEFORE the row is abandoned
                        // so whatever this adds to the error is part of
                        // what gets recorded against the intent key.
                        if let Err(cleanup) = tokio::fs::remove_file(&spec_path).await {
                            error = error.context(format!(
                                "could not remove launch spec {} after tmux creation failed: \
                                 {cleanup}",
                                spec_path.display()
                            ));
                        }
                        error = self.abandon_launching_record(reserved, error).await;
                    }
                    // Both remaining arms RETAIN the launching row, so
                    // neither may settle the reservation: an agent may be
                    // running under it, and a `Failed` outcome would tell
                    // every later retry the intent is closed while that
                    // agent kept running unreconciled. Returning without
                    // going through `abandon_launching_record` is what
                    // leaves it pending (see `launch_session`'s docs).
                    Ok(true) => {
                        error = error.context(format!(
                            "the tmux session {tmux_name} exists despite the failure, so \
                             session {id} is kept as a launching record rather than deleted; \
                             stop or delete it to reap whatever is running there"
                        ));
                    }
                    Err(probe) => {
                        error = error.context(format!(
                            "could not determine whether tmux session {tmux_name} was created \
                             ({probe:#}), so session {id} is kept as a launching record rather \
                             than deleted"
                        ));
                    }
                }
                return Err(error);
            }
        };
        // The launch reached tmux — the session and its pane exist, and
        // the shim is on its way to `exec` (whether the agent itself then
        // starts is a separate question this create never answers) — but
        // nothing durable says so yet: the row is still `Launching` with
        // no pane, and the reservation still pending. Placed before the
        // confirmation below rather than after it so a simulated crash
        // leaves precisely that state for reload's pane rediscovery to
        // reconcile.
        self.simulate_crash(CreateStage::DuringLaunch)?;

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
            // Vocabulary only as of this PR: nothing writes a stop
            // annotation or computes a real restart offer at create time
            // yet (PLAN_M3.md items 4 and 9).
            annotation: None,
            restart_offer: RestartOffer::default(),
        };

        // Launch confirmed: the pane exists, so the durable record moves
        // from launching to running and gains the pane id it could not
        // know before (PLAN_M3.md item 2's "confirmed running once the
        // pane exists"). A failure here still fails the whole create — the
        // row would otherwise stay launching while a real agent runs under
        // it, and the caller would be told a create succeeded whose
        // terminal handle was never recorded — so the tmux session just
        // created is torn back down (best effort) rather than left running
        // and unlisted with no way for the caller to learn its id.
        let confirmed = self
            .store
            .transition(&id, Transition::ConfirmRunning { pane: pane.clone() })
            .await;
        if let Ok(None) = confirmed {
            // The row is GONE: a `DeleteSession` for this id resolved its
            // entry before this launch removed it from the map and then
            // committed while the launch was mid-flight. The delete won —
            // it was a deliberate user action against a session that
            // existed when it was issued — so this launch tears its own
            // work back down rather than leaving a tmux session and an
            // agent behind with no row that knows about them. Reported as
            // the same spent-key `Conflict` a replay for a deleted session
            // gets, because that is what the client is looking at.
            let mut error = anyhow::Error::new(RequestError::new(
                ErrorKind::Conflict,
                format!(
                    "session {} was deleted while it was being created, so the launch was \
                     torn back down; it will not be recreated under the same intent key",
                    truncate_for_error(&id)
                ),
            ));
            if let Err(kill_err) = self.tmux.kill_session(&tmux_name).await {
                warn!(
                    session = %id, error = %kill_err,
                    "could not kill the tmux session of a create that raced a delete; \
                     it may now be running unlisted"
                );
                error = error.context(format!(
                    "additionally, tmux session {tmux_name} could not be killed ({kill_err:#}); \
                     the agent may still be running with no session record left"
                ));
            }
            best_effort_remove(&spec_path, "launch spec").await;
            best_effort_remove(&status_file_path, "launch status file").await;
            // The deleter already tombstoned the reservation (its own
            // transaction settles every pending one for this session), so
            // there is nothing left to settle here — and settling `Failed`
            // would be refused as non-pending anyway.
            return Err(error);
        }
        if let Err(e) = confirmed {
            // The DB error is the root cause throughout — it is what
            // actually failed the create — but a kill failure on top of it
            // is not safe to only log: it means an untracked tmux session
            // may now be running with nobody able to learn its id from the
            // caller's point of view, which the returned error must say so
            // the caller (and whoever reads the resulting log/HTTP body)
            // has a chance of noticing and cleaning it up by hand.
            let mut result = e.context("confirming the new session's launch in the database");
            let killed = self.tmux.kill_session(&tmux_name).await;
            if let Err(kill_err) = &killed {
                warn!(
                    session = %id, error = %kill_err,
                    "could not kill tmux session after its DB insert failed; \
                     it may now be running unlisted"
                );
                result = result.context(format!(
                    "additionally, could not kill tmux session {tmux_name} for session {id} \
                     after the DB insert failed ({kill_err:#}); the agent may still be running, \
                     so session {id} is kept as a launching record rather than deleted"
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
            // Only once the terminal is CONFIRMED gone: an unkillable tmux
            // session may still be running the agent, and the launching
            // row is then the sole record anyone could use to find it
            // again. Retaining it is the recoverable failure; deleting it
            // is not — and a retained row must leave its reservation
            // pending for reconciliation rather than settling a failure
            // over a possibly-live agent, which is what skipping the
            // rollback below also skips.
            if killed.is_ok() {
                result = self.abandon_launching_record(reserved, result).await;
            }
            return Err(result);
        }

        info!(session = %id, tmux = %tmux_name, %pane, "session created");
        self.sessions.lock().await.insert(
            id,
            Arc::new(SessionEntry {
                info: info.clone(),
                terminal: Some(Terminal { tmux_name, pane }),
                outcome: std::sync::Mutex::new(LastOutcome::Running),
            }),
        );
        Ok(info)
    }

    /// Drop the durable launching record for a create this process is
    /// about to report as failed, and — in the SAME transaction — record
    /// that failure against the create's intent key.
    ///
    /// Reached only from the paths that CONFIRMED nothing is running (the
    /// spec never landed; tmux said the session does not exist; the
    /// terminal was killed successfully), which is what makes settling
    /// `Failed` honest here and nowhere else: every retry of this intent
    /// now replays this exact error rather than re-deriving one from a
    /// world that has since changed. Paths that had to retain the row take
    /// the other branch entirely: they return without calling this at all,
    /// which is what leaves their reservation pending.
    ///
    /// The two writes are one transaction because a settlement that did not
    /// commit alongside the removal would leave a reservation pointing at a
    /// row that no longer exists, and the retry that found it would relaunch
    /// an intent whose failure the client was already told about.
    ///
    /// Every rollback path is best-effort at DELETING but never silent at
    /// FAILING: the create error the caller receives is returned enriched,
    /// so a row this process could not remove is named to whoever reads the
    /// log or the HTTP body. A failed rollback also means the failure was
    /// not recorded, so the caller is told that too
    /// ([`unrecorded_outcome`]) rather than being handed an error it would
    /// reasonably read as final.
    async fn abandon_launching_record(
        &self,
        reserved: &Reserved,
        error: anyhow::Error,
    ) -> anyhow::Error {
        let settlement = reserved.settlement(ReservationOutcome::Failed {
            kind: error_kind(&error),
            message: format!("{error:#}"),
        });
        let keyed = settlement.is_some();
        let id = reserved.session_id();
        match self.store.delete_session(id, settlement).await {
            Ok(()) => error,
            Err(e) => {
                let removal = format!(
                    "additionally, the launching record for session {id} could not be removed \
                     ({e:#}); it will list as unknown until it is deleted"
                );
                if keyed {
                    unrecorded_outcome(error, anyhow::anyhow!(removal))
                } else {
                    error.context(removal)
                }
            }
        }
    }

    /// The answer a SETTLED reservation gives: the session it created (or
    /// the gone-error), or the failure it recorded.
    ///
    /// Shared by every place that finds an outcome already recorded — the
    /// ordinary replay, a lost claim race, and a relaunch takeover that
    /// discovered someone settled first — so all three answer identically.
    /// A reservation still `Pending` cannot be answered from at all: that
    /// means something settled it and then un-settled it, which nothing in
    /// this module can do, so it is reported rather than guessed at.
    async fn answer_from(&self, reservation: &Reservation) -> anyhow::Result<SessionInfo> {
        match &reservation.outcome {
            ReservationOutcome::Created => self.replay_created_session(reservation).await,
            ReservationOutcome::Failed { kind, message } => {
                Err(RequestError::new(*kind, message.clone()).into())
            }
            ReservationOutcome::Pending => Err(anyhow::anyhow!(
                "intent key {} is still pending after another create claimed it; \
                 retry to reconcile it",
                truncate_for_error(&reservation.intent_key)
            )),
        }
    }

    /// Run the create-lifecycle seam for `stage`, if one is installed.
    ///
    /// The only way a crash reaches the create path, and the one place the
    /// [`SimulatedCrash`] marker is attached — see its docs for why an
    /// injected crash must not be mistaken for a create that merely
    /// failed. Production installs no seam, so this is an `Ok(())` after
    /// one `Option` check.
    fn simulate_crash(&self, stage: CreateStage) -> anyhow::Result<()> {
        match self.seams.create_crash.as_ref() {
            Some(crash) => crash(stage).map_err(|e| e.context(SimulatedCrash)),
            None => Ok(()),
        }
    }

    /// Whether this supervisor may record what it observes right now; see
    /// [`Supervisor::may_record`]. Everything that witnesses a transition
    /// checks this first, so a supervisor that is only reading — a handoff
    /// candidate, or one whose boot-id read failed — still answers
    /// requests honestly from what is stored without writing conclusions
    /// it has no standing to draw.
    fn may_record(&self) -> bool {
        self.may_record.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Offer a witnessed transition to `session`'s durable outcome and
    /// bring the in-memory mirror in line with what was COMMITTED.
    ///
    /// The mirror deliberately follows the store's answer rather than the
    /// caller's intent: `Transition::apply` may refuse (retained knowledge
    /// beats a poorer observation) or merge (a concurrent writer got there
    /// first), and copying the intent instead would leave the map claiming
    /// something the database does not say. A failed write leaves both
    /// unchanged, which is the conservative direction — the next
    /// observation retries.
    ///
    /// Errors are returned, not swallowed: `StopSession` turns one into a
    /// failed reply (SPEC.md surfaces every failure), while the list path
    /// logs and carries on, because a list that failed to WRITE has still
    /// computed an honest answer to READ.
    async fn record(
        &self,
        session: &str,
        entry: &SessionEntry,
        transition: Transition,
    ) -> anyhow::Result<()> {
        if !self.may_record() {
            return Ok(());
        }
        if let Some(committed) = self.store.transition(session, transition).await? {
            *entry.outcome.lock().expect("outcome mutex poisoned") = committed;
        }
        Ok(())
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
    // Byte-level progress on the write half, so the writer task can tell a
    // slow peer from one that has stopped consuming — see `ProgressWrite`.
    let (w, bytes_written) = ProgressWrite::new(w);
    let mut reader = FrameReader::new(r);
    let mut writer = FrameWriter::new(w);
    handshake(&mut reader, &mut writer, "supervisor").await?;

    // Single writer task; everything that wants to send (request
    // handlers, the output forwarder, takeover notifications) goes
    // through this queue so frames never interleave mid-write. Bounded
    // since M2.5 — see CONNECTION_WRITER_QUEUE for what the bound buys
    // and what it costs.
    let (tx, mut rx) = mpsc::channel::<Frame>(CONNECTION_WRITER_QUEUE);
    let (writer_failed_tx, mut writer_failed_rx) = oneshot::channel();
    // Progress counter for the shutdown-tail drain: `drain_writer` reads
    // this to tell "peer merely slow" apart from "peer gone" instead of
    // enforcing one flat deadline. Relaxed is enough on both ends — this
    // is a liveness heartbeat, not a value anything is synchronized on.
    let frames_written = Arc::new(AtomicU64::new(0));
    let frames_written_for_writer = Arc::clone(&frames_written);
    let writer_stall = sup.timeouts.writer_stall;
    let mut writer_task = tokio::spawn(async move {
        while let Some(frame) = rx.recv().await {
            // A write that makes NO PROGRESS for a whole window is
            // treated exactly like a write that failed. See
            // WRITER_STALL_TIMEOUT: without this, bounding the queue would
            // let a peer that stops reading park every producer —
            // including this connection's own read loop, via the admission
            // permits — so the connection could never notice the peer was
            // gone. Breaking here drops `rx`, which is what unblocks those
            // producers with a closed-channel error.
            if let Err(detail) =
                write_frame_before_stall(&mut writer, &bytes_written, &frame, writer_stall).await
            {
                warn!(error = %detail, "frame write to client failed");
                let _ = writer_failed_tx.send(detail);
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
                                    notify_detached(
                                        &old.notify,
                                        old.channel,
                                        format!("terminal input failed: {e:#}"),
                                    );
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
///
/// ## Classification precedence (PLAN_M3.md items 2 and 3)
///
/// As of M3 the live probe is no longer the only input: the session's
/// durable last-known outcome answers the questions a vanished tmux
/// cannot. The order below is the precedence, and it is deliberate:
///
/// 1. A recorded **error** — the launch shim's exec-failure sentinel —
///    outranks every inference, because "the agent never started" is a
///    fact about THIS launch that no amount of pane probing can discover
///    (an unexec'd command leaves an ordinary dead pane behind, exactly
///    like a command that ran and exited). PLAN_M3.md item 3 owns the
///    READER that ever writes this state; this PR only makes sure it
///    already sits above the inference so item 3 has nothing to
///    restructure. **The sentinel is deliberately not read here.**
/// 2. A live pane decides `Alive` vs. `Exited` exactly as M2 did — a
///    stored outcome never overrides something still observable. What the
///    record still contributes to a DEAD pane is what the pane cannot
///    hold: the stop annotation, and an exit code the pane has already
///    forgotten (`known code wins`, matching the store's own monotonic
///    enrichment rule — tmux publishes `pane_dead` before
///    `pane_dead_status` is readable, so the live reading can be the
///    poorer of the two).
/// 3. With no pane to ask, the recorded outcome speaks: `Interrupted`
///    (the reboot conversion) and `Exited` (a previously witnessed exit,
///    with the code and annotation it was witnessed with) are RETAINED
///    KNOWLEDGE, not guesses, and outrank M2's blanket exited-unknown.
/// 4. A `Launching` row with no pane is `Unknown`, not `Exited`: SPEC.md's
///    exited means the agent RAN, and a launch whose side effects were
///    never found has not established that. It stays pending for item 3's
///    sentinel (error) or item 6's reservation (retry) to resolve.
/// 5. Anything else with no pane — `Running`, or a stop whose sweep is in
///    flight — falls back to M2's honest `Exited { exit_code: None }`.
///
/// The annotation returned alongside the status is SPEC.md's user-legible
/// qualifier ("stopped by user"), which lives with the recorded outcome
/// and therefore survives restarts and reboots. It is returned only for a
/// status that ends up `Exited`: a session that has since been relaunched
/// into a live pane must not still be labelled with how its PREVIOUS run
/// ended.
fn session_status(
    entry: &SessionEntry,
    pane_states: &HashMap<String, PaneState>,
) -> (SessionStatus, Option<String>) {
    // The guard is held across the whole match rather than cloned out of:
    // this function is synchronous (no await can intervene) and every arm
    // only reads, so the clone would have bought nothing but an allocation
    // on the hottest path the list reply has.
    let recorded = entry.outcome.lock().expect("outcome mutex poisoned");
    let live = entry.terminal.as_ref().and_then(|terminal| {
        pane_states
            .get(&terminal.pane)
            .filter(|state| state.session_name == terminal.tmux_name)
    });
    match (&*recorded, live) {
        (LastOutcome::Error { detail }, _) => (
            SessionStatus::Error {
                detail: detail.clone(),
            },
            None,
        ),
        (_, Some(state)) if !state.dead => (SessionStatus::Alive, None),
        (recorded, Some(state)) => {
            let (recorded_code, annotation) = match recorded {
                LastOutcome::Exited {
                    exit_code,
                    annotation,
                } => (*exit_code, annotation.clone()),
                _ => (None, None),
            };
            (
                SessionStatus::Exited {
                    exit_code: state.exit_code.or(recorded_code),
                },
                annotation,
            )
        }
        (LastOutcome::Interrupted, None) => (SessionStatus::Interrupted, None),
        (
            LastOutcome::Exited {
                exit_code,
                annotation,
            },
            None,
        ) => (
            SessionStatus::Exited {
                exit_code: *exit_code,
            },
            annotation.clone(),
        ),
        (LastOutcome::Launching, None) => (SessionStatus::Unknown, None),
        (LastOutcome::Running | LastOutcome::StopRequested, None) => {
            (SessionStatus::Exited { exit_code: None }, None)
        }
    }
}

/// The exit code tmux still holds for `terminal`'s pane, if the pane is
/// dead and tmux could reduce its death to one.
///
/// `pane_states`, not `pane_process`: the latter answers "is it dead, and
/// what pid did it have", and only the former carries
/// `#{pane_dead_status}` at all. Used by `StopSession` at both of its
/// exit-recording moments — a stop that found the agent already gone, and
/// a stop whose kill sweep just finished — because in both the code is
/// worth keeping for exactly as long as the pane survives to hold it, and
/// nothing else will look again.
///
/// A failed query is logged and degrades to `None` rather than failing the
/// stop: it costs the exit code, never the annotation, and the store's
/// monotonic enrichment lets a later list fill the code in.
async fn dead_pane_exit_code(
    sup: &Supervisor,
    terminal: Option<&Terminal>,
    session_id: &str,
) -> Option<i32> {
    let terminal = terminal?;
    match sup.tmux.pane_states().await {
        Ok(states) => states
            .get(&terminal.pane)
            .filter(|state| state.session_name == terminal.tmux_name && state.dead)
            .and_then(|state| state.exit_code),
        Err(e) => {
            warn!(
                session = %session_id, error = %format!("{e:#}"),
                "could not read the pane's exit code; recording the outcome without one"
            );
            None
        }
    }
}

/// What this observation should offer the durable record, or `None` when
/// there is nothing worth telling the store.
///
/// Only the OBSERVATION is decided here; whether it changes anything is
/// [`Transition::apply`]'s call, inside the transaction. Two cases produce
/// nothing at all: a session whose outcome is already terminal (no probe
/// can add to `Interrupted`, `Error`, or an exit that already has its
/// code), and a `Launching` row with no pane — see `session_status`'s
/// point 4 for why absence of side effects is not evidence of an exit.
fn observation(recorded: &LastOutcome, live: Option<&PaneState>) -> Option<Transition> {
    match live {
        Some(state) if !state.dead => None,
        Some(state) => {
            let exit_code = state.exit_code;
            match recorded {
                // An already-recorded exit still accepts the code tmux may
                // only now be able to report (monotonic enrichment).
                LastOutcome::Exited {
                    exit_code: recorded_code,
                    ..
                } if recorded_code.is_none() && exit_code.is_some() => {
                    Some(Transition::ObservedExit { exit_code })
                }
                _ if recorded.is_terminal() => None,
                _ => Some(Transition::ObservedExit { exit_code }),
            }
        }
        None => matches!(recorded, LastOutcome::Running | LastOutcome::StopRequested)
            .then_some(Transition::ObservedExit { exit_code: None }),
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
        | ControlMsg::SessionRestarted { req_id, .. }
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
///
/// Awaits on a FULL queue, which is the intended backpressure (see
/// [`CONNECTION_WRITER_QUEUE`]): every caller is either
/// `handle_connection`'s own read loop — where blocking is exactly the
/// "stop accepting requests from a peer that is not reading its replies"
/// behavior wanted — or a spawned handler task holding nothing but its
/// admission permit. It must NOT be called while a supervisor mutex is
/// held; the arms that reply after a lock-held section all drop the guard
/// first, and [`notify_detached`] exists for the one shape that cannot.
async fn send_reply(tx: &mpsc::Sender<Frame>, m: &ControlMsg) {
    let _ = tx.send(reply_frame(m)).await;
}

/// Enqueue a `Detached` notice for `channel` without ever blocking the
/// caller — the one send shape that runs with `Supervisor::attachments`
/// held.
///
/// Every teardown path (takeover, delete, failed input, stall) has to tell
/// a client it lost its attachment, and all but one of them do so while
/// holding the global attachments mutex, because the ownership check and
/// the teardown must be atomic against a racing attach. Awaiting a
/// bounded send there would hand a wedged peer the ability to freeze
/// EVERY session's attach, input, and delete behind its own full queue —
/// turning this milestone's bound into a supervisor-wide deadlock.
///
/// So: `try_send` first, and only if the queue is genuinely full does the
/// blocking send move to its own task. The notice is never dropped (the
/// spawned task owns a sender clone and waits), and the only cost is that
/// a full-queue notice may land after frames enqueued behind it. That is
/// harmless here: a `Detached` is the last thing that channel will ever
/// carry, so nothing it could be reordered against still matters.
fn notify_detached(tx: &mpsc::Sender<Frame>, channel: u32, reason: String) {
    let frame = Frame::control(&ControlMsg::Detached { channel, reason });
    // A `Closed` error needs no handling: the connection is gone, so there
    // is nobody left to tell.
    if let Err(mpsc::error::TrySendError::Full(frame)) = tx.try_send(frame) {
        let tx = tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(frame).await;
        });
    }
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

/// Why a forwarder stopped, and therefore what the client must be told.
enum ForwarderEnd {
    /// The client's connection is gone (its writer queue closed). Nothing
    /// left to notify.
    ClientGone,
    /// The pane's control client ended: session killed, tmux server gone.
    TerminalEnded,
    /// The control stream itself failed mid-read.
    StreamFailed(String),
    /// The attachment stayed paused past [`STALL_DETACH_TIMEOUT`]. Unlike
    /// every other outcome this one also TEARS THE ATTACHMENT DOWN — see
    /// `Forwarder::run`.
    Stalled,
}

/// One attachment's output pump: everything between a pane's control-mode
/// client and one client channel's data frames.
///
/// A struct rather than a closure because this task now owns four
/// intertwined responsibilities that each need most of the same state —
/// writing the initial replay, honoring client pause/resume, recovering
/// from a tmux-side pause, and detaching a stalled client — and threading
/// eight captured variables through free functions read far worse than
/// methods on the thing they all belong to.
///
/// It never takes `Supervisor::attachments`. That is the invariant — and
/// only that one — which lets the takeover, detach, and delete paths abort
/// *and await* a forwarder while holding that mutex; the one place this
/// task needs the map (the stall teardown) hands the work to a separate
/// task for exactly that reason. It is NOT lock-free in general:
/// `send_dead_pane_snapshot` briefly takes `pending_snapshots`, which is
/// safe precisely because nothing ever holds that lock across a wait on a
/// forwarder.
struct Forwarder {
    sup: Arc<Supervisor>,
    session_id: String,
    /// The tmux pane id, needed by the catch-up replay — the forwarder
    /// cannot go look it up, since consulting `Supervisor::sessions`
    /// would break the no-locks rule above.
    pane: String,
    channel: u32,
    tx: mpsc::Sender<Frame>,
    stream: OutputStream,
    pause_rx: watch::Receiver<Option<tokio::time::Instant>>,
    stall_timeout: Duration,
}

impl Forwarder {
    /// Write the attach replay, then pump live output until something
    /// ends it.
    ///
    /// The whole task body, so the teardown obligations live in exactly
    /// one place: the control client is always shut down, and a stall
    /// additionally removes the attachment (via `detach_stalled`, which
    /// must run on its own task — see there).
    async fn run(mut self, modes: PaneModes, prefill: Vec<u8>) {
        // The attach replay never resets: the client's terminal is brand
        // new. Only the catch-up path passes `true` — see `send_replay`.
        let end = match self.send_replay(modes, prefill, false).await {
            Ok(()) => self.pump().await,
            Err(end) => end,
        };
        // Ordered: kill the control client BEFORE announcing the detach,
        // matching every other teardown in this module. A client that
        // reattaches the instant it sees `Detached` must not race a
        // control client that is still dying — the documented
        // frozen-replay hazard.
        self.stream.shutdown().await;
        match end {
            ForwarderEnd::ClientGone => {}
            ForwarderEnd::TerminalEnded => {
                notify_detached(&self.tx, self.channel, "session terminal ended".to_string());
            }
            ForwarderEnd::StreamFailed(reason) => {
                // Must notify: swallowing this leaves the client with a
                // terminal that silently stops updating while still
                // accepting input, and no log line anywhere explaining why.
                warn!(channel = self.channel, error = %reason, "output stream failed");
                notify_detached(
                    &self.tx,
                    self.channel,
                    format!("output stream failed: {reason}"),
                );
            }
            ForwarderEnd::Stalled => {
                warn!(
                    channel = self.channel,
                    session = %self.session_id,
                    "attachment paused longer than {:?}; detaching as stalled",
                    self.stall_timeout
                );
                detach_stalled(&self.sup, self.session_id, self.channel, self.tx);
            }
        }
    }

    /// Write one replay to the client: optional reset, the pre-content
    /// mode sequences, the captured content, then the post-content
    /// sequences.
    ///
    /// Order is load-bearing (see `PaneModes`): the alternate-screen
    /// switch must precede the content because it CLEARS the buffer it
    /// switches to, and cursor placement must follow it because writing
    /// content moves the cursor.
    ///
    /// `reset_first` is the only difference between the two callers, and
    /// the whole correctness argument for the second. An ATTACH replays
    /// into a terminal the client just created (empty by construction),
    /// while a post-stall CATCH-UP replays into one already showing
    /// everything received before tmux cut the stream. PLAN_M2_5.md is
    /// explicit: never replay into a populated terminal. `\x1bc` (RIS) is
    /// what makes the second case equivalent to the first — it clears
    /// screen AND scrollback, so the catch-up's end state is a fresh
    /// reattach's end state rather than the old content with a second copy
    /// of history appended under it.
    async fn send_replay(
        &mut self,
        modes: PaneModes,
        content: Vec<u8>,
        reset_first: bool,
    ) -> Result<(), ForwarderEnd> {
        if reset_first {
            self.send_bytes(b"\x1bc".to_vec()).await?;
        }
        self.send_bytes(modes.pre_content_sequences().into_bytes())
            .await?;
        self.send_bytes(content).await?;
        self.send_bytes(modes.post_content_sequences().into_bytes())
            .await?;
        if modes.pane_dead {
            self.send_dead_pane_snapshot(modes.alternate_on).await?;
        }
        Ok(())
    }

    /// Append the stop-time alt-screen snapshot, if this session has one,
    /// after a dead pane's ordinary prefill.
    ///
    /// See [`load_alt_screen_snapshot`] for what is appended and why the
    /// gate is the snapshot's existence rather than the pane's current
    /// screen. `pane_alternate` decides only PLACEMENT: when the dead pane
    /// is still on the alternate screen, `\x1b[?1049l` leaves it first, so
    /// the divider and snapshot land on the primary screen whose real
    /// scrollback can absorb whatever does not fit — otherwise they would
    /// land in the scrollback-less alternate buffer the mode replay just
    /// re-entered and bury their own top rows with nowhere to overflow to.
    ///
    /// Sent as separate pieces rather than one concatenated buffer, and
    /// through [`Self::send_bytes`] like every other byte a forwarder
    /// writes: avoids a second full copy of a snapshot that may be
    /// megabytes, and inherits the same chunking, pause gating, and stall
    /// deadline as the rest of the replay.
    async fn send_dead_pane_snapshot(&mut self, pane_alternate: bool) -> Result<(), ForwarderEnd> {
        let Some(bytes) = load_alt_screen_snapshot(&self.sup, &self.session_id).await else {
            return Ok(());
        };
        if pane_alternate {
            self.send_bytes(b"\x1b[?1049l".to_vec()).await?;
        }
        self.send_bytes(b"\r\n\x1b[2m-- last screen before stop --\x1b[0m\r\n".to_vec())
            .await?;
        self.send_bytes(bytes).await?;
        self.send_bytes(b"\r\n".to_vec()).await
    }

    /// Park while the client has output paused, returning `Err(Stalled)`
    /// once this ONE pause has outlived [`STALL_DETACH_TIMEOUT`].
    ///
    /// The deadline is absolute: it is computed from the instant the pause
    /// STARTED (stored in the watch by `set_attachment_paused`), never
    /// from whenever this function happened to be called. That is what
    /// makes the timeout a hard maximum across every phase a forwarder can
    /// be in — initial replay, live pump, catch-up replay — and across
    /// every individual chunk within them. A per-call timer would be
    /// restarted by any progress at all, so a client draining just fast
    /// enough to keep the forwarder moving between chunks could stay
    /// paused forever.
    async fn park_while_paused(&mut self) -> Result<(), ForwarderEnd> {
        loop {
            let Some(paused_at) = *self.pause_rx.borrow_and_update() else {
                return Ok(());
            };
            tokio::select! {
                () = tokio::time::sleep_until(paused_at + self.stall_timeout) => {
                    return Err(ForwarderEnd::Stalled);
                }
                changed = self.pause_rx.changed() => {
                    // The sender is dropped only when this attachment has
                    // been removed from the map, at which point this task
                    // is being aborted anyway; falling through is the
                    // harmless answer.
                    if changed.is_err() {
                        return Ok(());
                    }
                }
            }
        }
    }

    /// Enqueue `bytes` as one or more data frames, honoring the client's
    /// pause before every one and blocking on the bounded writer queue
    /// when it is full — which is the backpressure this milestone exists
    /// to introduce.
    ///
    /// Gating EVERY chunk, not merely the top of the pump loop, is what
    /// makes the watermark actually work: a replay, or a single large
    /// output notification, can be megabytes, and a version that consulted
    /// the pause only between EVENTS would push that entire burst at a
    /// client which had already said stop.
    ///
    /// Chunked at [`REPLAY_CHUNK`] for a harsher reason than the replay's
    /// own progressiveness: one bounded output notification may still be
    /// larger than a protocol frame, and the encoder rejects that rather
    /// than sending something the far side cannot decode. This is the
    /// last chunking boundary; input and replay already do the same.
    async fn send_bytes(&mut self, bytes: Vec<u8>) -> Result<(), ForwarderEnd> {
        for chunk in bytes.chunks(REPLAY_CHUNK) {
            self.park_while_paused().await?;
            let frame = Frame::data(self.channel, chunk.to_vec());
            // Raced against the SAME absolute deadline so a client that
            // pauses mid-send cannot pin this task (and the frames behind
            // it) on a queue nobody is draining. Cancelling a
            // `Sender::send` is safe — the frame is either enqueued or it
            // is not — and on the stall path the whole attachment is torn
            // down anyway.
            tokio::select! {
                result = self.tx.send(frame) => {
                    if result.is_err() {
                        return Err(ForwarderEnd::ClientGone);
                    }
                }
                () = stalled_past_deadline(self.pause_rx.clone(), self.stall_timeout) => {
                    return Err(ForwarderEnd::Stalled);
                }
            }
        }
        Ok(())
    }

    /// Pump pane output to the client until the stream, the client, or
    /// the client's patience ends.
    async fn pump(&mut self) -> ForwarderEnd {
        loop {
            // Park while the client has asked for silence. Not reading
            // the control client at all IS the flow control: past
            // `pause-after`, tmux answers by either throttling the pane
            // (the agent's write blocks) or cutting this client's stream
            // with `%pause` — see TMUX_PAUSE_AFTER_SECS for why both
            // happen and why nothing here may assume which. Either way
            // the tmux server's memory stays flat, which is the property
            // this parking exists to get.
            if let Err(end) = self.park_while_paused().await {
                return end;
            }

            let event = tokio::select! {
                event = self.stream.next_output() => event,
                () = stalled_past_deadline(self.pause_rx.clone(), self.stall_timeout) => {
                    // Abandoning `next_output` mid-read can drop a
                    // partial line, which is why this is only ever done
                    // on a path that tears the stream down immediately
                    // afterwards. See that method's cancel-safety note.
                    return ForwarderEnd::Stalled;
                }
            };
            match event {
                Ok(Some(OutputEvent::Bytes(bytes))) => {
                    if let Err(end) = self.send_bytes(bytes).await {
                        return end;
                    }
                }
                Ok(Some(OutputEvent::Paused)) => {
                    if let Err(end) = self.catch_up_after_tmux_pause().await {
                        return end;
                    }
                }
                Ok(None) => return ForwarderEnd::TerminalEnded,
                Err(e) => return ForwarderEnd::StreamFailed(format!("{e:#}")),
            }
        }
    }

    /// Recover from tmux having cut this client's pane stream: continue
    /// the pane and replay it as a full reattach.
    ///
    /// A `%pause` means bytes were dropped from the LIVE path — they are
    /// not buffered anywhere and will never arrive. What tmux still has is
    /// its history, which is exactly what the attach path already knows
    /// how to replay, so the catch-up reuses that machinery verbatim
    /// (`OutputStream::resume_paused_with_replay`) instead of growing a
    /// second replay implementation. Alternate-screen and normal-screen
    /// panes are both covered for free: the shared snapshot code already
    /// picks the right capture for the pane's current mode.
    ///
    /// Reached only on the tmux behavior that cuts the stream — the other
    /// one (tmux throttling the pane instead) never gets here, because
    /// nothing was dropped and the pump simply keeps reading. See
    /// `TMUX_PAUSE_AFTER_SECS` for why both exist; this method is
    /// correctness-critical but not on every run's path.
    ///
    /// The client's terminal is reset first because the replay assumes an
    /// empty one — see [`Self::send_replay`]. Within `HISTORY_LIMIT`
    /// this is lossless; past it, it degrades to the history floor, which
    /// is the same floor the browser's own scrollback is capped at, so
    /// the end state stays observably equivalent to lossless slow
    /// delivery (PLAN_M2_5.md).
    ///
    /// A failure here is fatal to the attachment rather than ignorable: a
    /// pane left paused delivers nothing ever again, so pretending
    /// otherwise would leave a live-looking terminal that has silently
    /// stopped.
    async fn catch_up_after_tmux_pause(&mut self) -> Result<(), ForwarderEnd> {
        info!(
            channel = self.channel,
            session = %self.session_id,
            "tmux paused the pane for this client; catching up by reset and replay"
        );
        let (modes, content) = match self.stream.resume_paused_with_replay(&self.pane).await {
            Ok(replay) => replay,
            Err(e) => return Err(ForwarderEnd::StreamFailed(format!("{e:#}"))),
        };
        self.send_replay(modes, content, true).await
    }
}

/// Resolve only once the attachment has been paused CONTINUOUSLY past
/// `timeout` — the stall detector every blocking await in a forwarder is
/// raced against.
///
/// Safe to create fresh at each `select!` site precisely because the
/// deadline is derived from the pause's stored START instant rather than
/// from now: re-creating it cannot restart the clock, so the hard maximum
/// survives however many chunks, phases, or wakeups a single pause spans.
/// (An earlier version timed from `Instant::now()` and had exactly that
/// bug — a client draining slowly enough to keep the forwarder awaiting,
/// but fast enough to keep it moving, was never detached.)
///
/// Resolving only on a CONTINUOUS pause is what keeps this a hard maximum
/// pause duration rather than a cumulative budget: a client that pauses
/// and resumes repeatedly is a slow client being served correctly, not a
/// stalled one, and each resume clears the stored start.
async fn stalled_past_deadline(
    mut pause_rx: watch::Receiver<Option<tokio::time::Instant>>,
    timeout: Duration,
) {
    loop {
        let paused_at = *pause_rx.borrow_and_update();
        let Some(paused_at) = paused_at else {
            if pause_rx.changed().await.is_err() {
                // The attachment is gone; this future must simply never
                // resolve, so its `select!` arm cannot fabricate a stall
                // out of a teardown that is already under way.
                return std::future::pending().await;
            }
            continue;
        };
        tokio::select! {
            () = tokio::time::sleep_until(paused_at + timeout) => return,
            changed = pause_rx.changed() => {
                if changed.is_err() {
                    return std::future::pending().await;
                }
            }
        }
    }
}

/// Tear down an attachment whose client stalled, on a task of its own.
///
/// Spawned rather than run inline because forwarders must never take
/// `Supervisor::attachments`: the takeover, detach, and delete paths all
/// abort AND AWAIT a forwarder while holding that mutex, so a forwarder
/// blocking on it would deadlock the supervisor outright. A separate task
/// can wait for the lock safely: it is not the task being awaited, so a
/// teardown holding the mutex can always make progress and release it.
/// (This does NOT assume the spawning forwarder has already returned — it
/// may still be unwinding when this runs. Nothing here depends on that:
/// the forwarder has already shut its control client down before spawning
/// this, and the `abort`-then-`await` below reaps the handle whenever it
/// finishes.)
///
/// The identity check is the same two-part one every other ownership
/// check in this module uses (channel plus owning connection): by the
/// time this runs, a takeover may already have installed a different
/// attachment for this session, and tearing THAT one down would detach an
/// innocent client.
fn detach_stalled(
    sup: &Arc<Supervisor>,
    session_id: String,
    channel: u32,
    tx: mpsc::Sender<Frame>,
) {
    let sup = Arc::clone(sup);
    tokio::spawn(async move {
        let mut attachments = sup.attachments.lock().await;
        let mine = attachments
            .get(&session_id)
            .is_some_and(|a| a.channel == channel && a.notify.same_channel(&tx));
        if !mine {
            // A takeover (or a delete, or the connection dying) got here
            // first, so this stall belongs to an attachment that no longer
            // exists. Returning WITHOUT notifying is the point: the winner
            // is using the same channel id on the same connection, so a
            // stalled notice sent now would reach the new client and race
            // — or overtake — the truthful notice its own teardown path
            // already sent to the loser.
            return;
        }
        let removed = attachments.remove(&session_id);
        if let Some(old) = removed {
            // Abort-and-await like every other teardown, even though the
            // forwarder is the very task that asked for this: it has
            // already shut its control client down and returned, so this
            // only reaps the handle. Dropping the removed `ActiveAttach`
            // is also what kills the input client.
            old.forwarder.abort();
            let _ = old.forwarder.await;
        }
        drop(attachments);
        let _ = tx
            .send(Frame::control(&ControlMsg::Detached {
                channel,
                reason: DETACH_REASON_STALLED.to_string(),
            }))
            .await;
    });
}

/// Apply a client's `PauseOutput`/`ResumeOutput` to whichever attachment
/// it owns, or ignore it.
///
/// The ownership check is deliberately the SAME shape as the `Resize`
/// arm's — see that arm's comment for the TOCTOU argument in full: both
/// halves matter, because channel ids are unique only within a
/// connection, so `same_channel` is what identifies the owning
/// connection and the channel id is what tells apart clients multiplexed
/// over one connection (every browser tab rides the helm's single
/// supervisor connection). The check and the state change run under one
/// lock hold for the same reason too, though the stakes are lower here
/// than for input or resize: the worst a stale pause could do is silence
/// a terminal the sender no longer owns until its real owner resumes it.
///
/// The lookup is by channel rather than by session id because
/// `PauseOutput` carries no session id — unlike `Resize`, which needs one
/// to reach tmux. Mirrors the `Detach` arm's own search for the same
/// reason.
///
/// A pause records WHEN it started and a resume clears it, and a pause
/// arriving while already paused changes nothing at all. That last part is
/// load-bearing rather than tidy: the forwarder's hard maximum is measured
/// from this stored instant (see [`stalled_past_deadline`]), so letting a
/// repeated `PauseOutput` overwrite it would let a client hold an
/// attachment open forever simply by re-sending pause. `send_if_modified`
/// then also suppresses the pointless wakeup.
async fn set_attachment_paused(
    sup: &Arc<Supervisor>,
    tx: &mpsc::Sender<Frame>,
    channel: u32,
    paused: bool,
) {
    let attachments = sup.attachments.lock().await;
    if let Some(attachment) = attachments
        .values()
        .find(|a| a.channel == channel && a.notify.same_channel(tx))
    {
        attachment
            .pause
            .send_if_modified(|current| match (paused, *current) {
                // Already paused: keep the ORIGINAL start instant.
                (true, Some(_)) => false,
                (true, None) => {
                    *current = Some(tokio::time::Instant::now());
                    true
                }
                (false, None) => false,
                (false, Some(_)) => {
                    *current = None;
                    true
                }
            });
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
    tx: &mpsc::Sender<Frame>,
    input_routes: &mut HashMap<u32, Arc<SessionEntry>>,
    tasks: &mut tokio::task::JoinSet<()>,
) {
    match msg {
        ControlMsg::CreateSession {
            req_id,
            cwd,
            invocation,
            title,
            cols,
            rows,
            intent_key,
            // Bound, but only to be FINGERPRINTED: PLAN_M3.md item 7 is
            // what makes these overrides shape the session itself. Item
            // 6's contract already needs them here, though — a retry
            // differing only in an override is a different request and
            // must be refused as a key reuse, which is a property of the
            // fingerprint alone (`create_fingerprint`).
            agent_kind,
            resume_template,
        } => {
            // One accounting for every caller-supplied field that this
            // request can make the supervisor STORE — the reply-size
            // argument `CREATE_FIELD_CAP` was introduced for, plus item
            // 6's: the fingerprint holds a copy of all of them, in a
            // reservation row that is never pruned, so an unbounded
            // override is an unbounded permanent write.
            let template_bytes: usize = resume_template
                .iter()
                .flatten()
                .map(|element| element.len())
                .sum();
            let field_len = cwd.len()
                + invocation.len()
                + title.as_deref().map_or(0, str::len)
                + template_bytes;
            let refusal = if field_len > CREATE_FIELD_CAP {
                Some(format!(
                    "cwd, invocation, title, and resume template together are {field_len} bytes, \
                     exceeding the {CREATE_FIELD_CAP}-byte limit"
                ))
            } else if resume_template
                .as_ref()
                .is_some_and(|template| template.len() > RESUME_TEMPLATE_ELEMENT_CAP)
            {
                // Bounded separately from the byte total because the two
                // are independent: a template of ten thousand EMPTY
                // elements costs almost no bytes and is still nothing a
                // resume invocation could legitimately be.
                Some(format!(
                    "resume template has {} elements, exceeding the \
                     {RESUME_TEMPLATE_ELEMENT_CAP}-element limit",
                    resume_template.as_ref().map_or(0, Vec::len)
                ))
            } else {
                // Both refusals below are about a key that could never do
                // its job: an empty one would collapse every create that
                // forgot to set it into a single intent, and an unbounded
                // one buys durable, un-pruned table space with a request
                // (see `INTENT_KEY_CAP`). Checked before the lookup so
                // neither ever reaches the store.
                match intent_key.as_deref() {
                    Some("") => Some("intent key must not be empty".to_string()),
                    Some(key) if key.len() > INTENT_KEY_CAP => Some(format!(
                        "intent key is {} bytes, exceeding the {INTENT_KEY_CAP}-byte limit",
                        key.len()
                    )),
                    _ => None,
                }
            };
            if let Some(message) = refusal {
                send_reply(
                    tx,
                    &ControlMsg::Error {
                        req_id,
                        message,
                        kind: ErrorKind::InvalidRequest,
                    },
                )
                .await;
                return;
            }
            let idempotency = intent_key.map(|intent_key| IntentClaim {
                intent_key,
                fingerprint: create_fingerprint(
                    &cwd,
                    &invocation,
                    title.as_deref(),
                    agent_kind,
                    resume_template.as_deref(),
                ),
            });
            match sup
                .create_session(&cwd, &invocation, title, cols, rows, idempotency)
                .await
            {
                Ok(session) => {
                    send_reply(tx, &ControlMsg::SessionCreated { req_id, session }).await;
                }
                Err(e) => {
                    send_reply(
                        tx,
                        &ControlMsg::Error {
                            req_id,
                            message: format!("{e:#}"),
                            kind: error_kind(&e),
                        },
                    )
                    .await;
                }
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
                // change the answer: a terminal-less entry is decided
                // entirely by its recorded outcome (`session_status` never
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
                            )
                            .await;
                            return;
                        }
                    }
                } else {
                    HashMap::new()
                };
                // A list request is one of the places this supervisor
                // WITNESSES an exit (PLAN_M3.md item 2): the dead pane it
                // just found may be gone entirely by the next reboot,
                // taking its exit code with it, so the code is recorded
                // now — while tmux still has it — rather than recomputed
                // forever from a fact that expires. Every observation this
                // pass produces commits in ONE transaction, and the store
                // decides what each one means (`Transition::apply`), so a
                // stop running concurrently cannot have its annotation
                // erased by this list and this list cannot be misled by a
                // stale reading of its own.
                // A launch sentinel is READ regardless of whether this
                // supervisor `may_record()` (item 2 of the review-swarm
                // fix batch): a degraded supervisor (a handoff candidate,
                // or one whose boot-id read failed) still has standing to
                // REPORT what it can read, even though it must not WRITE a
                // conclusion it has no standing to store — the two halves
                // below are deliberately independent (`reply_status`
                // always reflects a sentinel this pass found; only
                // `observations` is gated on `may_record()`).
                //
                // A plain loop, not `observation()`'s pure `filter_map`
                // closure, because the sentinel check below is real I/O
                // (`read_launch_sentinel`) and therefore has to run
                // between two lock scopes rather than inside one
                // synchronous closure body: PLAN_M3.md item 3 wants the
                // SAME observation offered here that `reload_sessions`
                // offers — a non-terminal outcome whose pane is dead or
                // gone entirely gets its sentinel checked before falling
                // back to `observation()`'s plain exit inference, because
                // the sentinel outranks that inference exactly as surely
                // here as at reload, including (addition 18) for an entry
                // ALREADY recorded as an inferred `Interrupted` or
                // unannotated `Exited` — both are themselves only
                // inferences a sentinel is defined to beat.
                let mut observations: Vec<(String, Transition)> = Vec::new();
                // This pass's sentinel finds, id to detail — used both to
                // gate post-commit file cleanup on the transition actually
                // landing, and (`reply_status`, below) to surface the
                // Error for THIS reply even when it could not be
                // committed durably this pass (`may_record()` false, or
                // the commit itself fails) — PLAN_M3.md item 3's
                // write-inability note: retain the file, retry
                // persistence on a later poll, but never let the reply
                // itself regress to a stale `Exited` in the meantime.
                let mut sentinel_hits: HashMap<String, String> = HashMap::new();
                for entry in &entries {
                    let (recorded, dead_or_absent) = {
                        let recorded = entry
                            .outcome
                            .lock()
                            .expect("outcome mutex poisoned")
                            .clone();
                        let live = entry.terminal.as_ref().and_then(|terminal| {
                            pane_states
                                .get(&terminal.pane)
                                .filter(|state| state.session_name == terminal.tmux_name)
                        });
                        (recorded, live.is_none_or(|state| state.dead))
                    };

                    // Idempotent cleanup (item 4): an entry already durably
                    // `Error` may still have a lingering sentinel/spec file
                    // from a crash between an earlier pass's commit and the
                    // cleanup that should have followed it. Harmless no-op
                    // once both files are gone.
                    if matches!(recorded, LastOutcome::Error { .. }) {
                        cleanup_launch_artifacts(&sup.state_dir, &entry.info.id).await;
                        continue;
                    }

                    if sentinel_could_still_apply(&recorded) && dead_or_absent {
                        match read_launch_sentinel(&sup.state_dir, &entry.info.id).await {
                            Ok(Some(detail)) => {
                                sentinel_hits.insert(entry.info.id.clone(), detail.clone());
                                // No pane to rediscover here (unlike
                                // `reload_sessions`'s by-name search): this
                                // loop only ever visits sessions this
                                // process already tracks a `Terminal` for
                                // or explicitly does not, so there is
                                // nothing new for this transition to
                                // record beyond the outcome itself.
                                if sup.may_record() {
                                    observations.push((
                                        entry.info.id.clone(),
                                        Transition::SentinelError { detail, pane: None },
                                    ));
                                }
                                continue;
                            }
                            Ok(None) => {}
                            Err(e) => {
                                // Loud propagation, not fall-through (item
                                // 1): the WHOLE request fails rather than
                                // silently basing this — or any other —
                                // entry's reply on an inference the
                                // unreadable sentinel might contradict.
                                // Nothing gathered so far this pass is
                                // committed: this `return` happens before
                                // `transition_many` is ever called.
                                send_reply(
                                    &tx,
                                    &ControlMsg::Error {
                                        req_id,
                                        message: format!(
                                            "could not read session {}'s launch sentinel: {e:#}",
                                            entry.info.id
                                        ),
                                        kind: ErrorKind::Internal,
                                    },
                                )
                                .await;
                                return;
                            }
                        }
                    }

                    if sup.may_record() {
                        // The guard is held for the closure body only,
                        // with no await inside it, so nothing has to be
                        // cloned to be read.
                        let recorded = entry.outcome.lock().expect("outcome mutex poisoned");
                        let live = entry.terminal.as_ref().and_then(|terminal| {
                            pane_states
                                .get(&terminal.pane)
                                .filter(|state| state.session_name == terminal.tmux_name)
                        });
                        if let Some(transition) = observation(&recorded, live) {
                            observations.push((entry.info.id.clone(), transition));
                        }
                    }
                }
                if !observations.is_empty() {
                    match sup.store.transition_many(observations).await {
                        Ok(committed) => {
                            for entry in &entries {
                                if let Some(outcome) = committed.get(&entry.info.id) {
                                    *entry.outcome.lock().expect("outcome mutex poisoned") =
                                        outcome.clone();
                                }
                            }
                            // Cleanup folded into this successful arm
                            // (item 7), not a separate loop afterward: see
                            // `reload_sessions`'s identical step for the
                            // full lifecycle rationale (both files are
                            // cosmetic once the durable outcome already
                            // says what happened; a failed write must
                            // leave them for the next pass to retry
                            // against, hence gating on `committed` here
                            // rather than on `sentinel_hits` alone).
                            for id in sentinel_hits.keys() {
                                if matches!(committed.get(id), Some(LastOutcome::Error { .. })) {
                                    cleanup_launch_artifacts(&sup.state_dir, id).await;
                                }
                            }
                        }
                        // Logged, not fatal: the reply below is computed
                        // from what this pass OBSERVED plus what is
                        // durably recorded, both of which are still honest
                        // when the write fails — and the next list retries.
                        Err(e) => warn!(
                            error = %format!("{e:#}"),
                            "could not record observed session outcomes; \
                             the next list will retry"
                        ),
                    }
                }
                let sessions: Vec<SessionInfo> = entries
                    .iter()
                    .map(|entry| {
                        // A sentinel this pass found overrides whatever
                        // `session_status` would otherwise compute for
                        // THIS reply, whether or not it also got committed
                        // durably above — see `sentinel_hits`'s own docs.
                        let mut info = entry.info.clone();
                        if let Some(detail) = sentinel_hits.get(&entry.info.id) {
                            info.status = SessionStatus::Error {
                                detail: detail.clone(),
                            };
                            info.annotation = None;
                        } else {
                            let (status, annotation) = session_status(entry, &pane_states);
                            info.status = status;
                            info.annotation = annotation;
                        }
                        info
                    })
                    .collect();
                send_reply(
                    &tx,
                    &build_list_reply(req_id, sessions, total, LIST_BYTE_BUDGET),
                )
                .await;
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
                    )
                    .await;
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
                            )
                            .await;
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

                // The durable stop INTENT, before a single signal is sent
                // (PLAN_M3.md item 4). `kill_process_tree` below runs for
                // seconds — SIGTERM, a grace period, re-enumeration,
                // SIGKILL — and a crash anywhere in there used to leave a
                // session the next startup read as a plain exit, silently
                // converting "the user stopped this" into "the agent
                // finished on its own". With the intent recorded first,
                // reload reconciles it: a dead pane means the stop landed
                // (annotated exit), a live one means it never did (intent
                // cleared), and a reboot straddling it interrupts like any
                // other live session.
                //
                // Only for a pane observed ALIVE: an agent that had
                // already exited on its own is not something the user
                // stopped, and claiming otherwise would credit them with
                // an ending they had nothing to do with. That case records
                // the plain exit instead — with whatever code the dead
                // pane still retains — because a stop is also the moment
                // this supervisor witnesses an exit nobody had listed yet.
                //
                // The sentinel is checked FIRST, though (item 3a of the
                // review-swarm fix batch): a dead-or-absent pane at stop
                // time can just as easily mean the launch never execed at
                // all, and this is exactly the commit boundary PLAN_M3.md
                // item 3 warns about — a stop (or any other first
                // observer) committing a plain `ObservedExit` before
                // anything ever reads the sentinel locks in "exited"
                // behind terminal-stickiness, permanently outrunning a
                // classification the file already had evidence for.
                // Checking here, before the write, is what keeps a stop
                // from being the race that loses.
                let stop_intent = if alive_pane.is_some() {
                    Transition::StopRequested
                } else {
                    let current = entry
                        .outcome
                        .lock()
                        .expect("outcome mutex poisoned")
                        .clone();
                    let sentinel = if sentinel_could_still_apply(&current) {
                        read_launch_sentinel(&sup.state_dir, &session_id).await
                    } else {
                        Ok(None)
                    };
                    match sentinel {
                        Ok(Some(detail)) => Transition::SentinelError {
                            detail,
                            pane: entry.terminal.as_ref().map(|t| t.pane.clone()),
                        },
                        Ok(None) => Transition::ObservedExit {
                            exit_code: dead_pane_exit_code(
                                &sup,
                                entry.terminal.as_ref(),
                                &session_id,
                            )
                            .await,
                        },
                        Err(e) => {
                            // Loud propagation (item 1's discipline,
                            // extended to this call site): refuse the
                            // whole stop rather than durably committing a
                            // plain exit this sentinel might contradict.
                            // Nothing was alive to signal anyway
                            // (`alive_pane` is already `None` in this
                            // branch), so nothing beyond the classification
                            // write itself is lost — the caller can retry
                            // once the sentinel is readable again.
                            send_reply(
                                &tx,
                                &ControlMsg::Error {
                                    req_id,
                                    message: format!(
                                        "could not read this session's launch sentinel, so \
                                         nothing was recorded: {e:#}"
                                    ),
                                    kind: ErrorKind::Internal,
                                },
                            )
                            .await;
                            return;
                        }
                    }
                };
                if let Err(e) = sup.record(&session_id, &entry, stop_intent).await {
                    // Recording the intent is part of the stop's contract,
                    // not bookkeeping around it: proceeding to kill a tree
                    // whose intent could not be stored is exactly the
                    // crash window this write exists to close, and SPEC.md
                    // requires the failure to surface rather than be
                    // logged past.
                    send_reply(
                        &tx,
                        &ControlMsg::Error {
                            req_id,
                            message: format!(
                                "recording the stop failed, so nothing was killed: {e:#}"
                            ),
                            kind: ErrorKind::Internal,
                        },
                    )
                    .await;
                    return;
                }
                // Sentinel lifecycle: if the intent just recorded WAS a
                // `SentinelError` and it committed, this stop is the
                // moment that classification became durable — clean up
                // both files right away rather than waiting for a later
                // list or reload to notice (item 4/25 of the review-swarm
                // fix batch; see `cleanup_launch_artifacts`'s own docs).
                if matches!(
                    &*entry.outcome.lock().expect("outcome mutex poisoned"),
                    LastOutcome::Error { .. }
                ) {
                    cleanup_launch_artifacts(&sup.state_dir, &session_id).await;
                }

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
                    )
                    .await;
                    return;
                }
                // The stop's OUTCOME, committed before the snapshot is
                // published and before the reply is sent: the kill has
                // already happened, so any crash from here on must find a
                // record that says so. Publishing a snapshot first would
                // put a cosmetic write ahead of the durability the stop
                // promised.
                //
                // The exit code is re-queried rather than assumed. A
                // killed process usually leaves tmux nothing to reduce to
                // a plain code, but where it does, that code is worth
                // keeping — and `pane_states`, not `pane_process`, is what
                // carries `#{pane_dead_status}` at all. A failed query is
                // logged rather than swallowed: it costs the code, not the
                // annotation, and a later list can still enrich the record
                // (the store's transitions are monotonic).
                let mut stop_error = None;
                if alive_pane.is_some() {
                    let exit_code =
                        dead_pane_exit_code(&sup, entry.terminal.as_ref(), &session_id).await;
                    if let Err(e) = sup
                        .record(&session_id, &entry, Transition::StopCompleted { exit_code })
                        .await
                    {
                        // The process tree IS stopped; what failed is
                        // recording it. Reported rather than logged
                        // (SPEC.md surfaces every failure), and worded so
                        // the caller knows the kill itself succeeded.
                        stop_error = Some(format!(
                            "the session was stopped, but recording that outcome failed, so it \
                             may list as a plain exit: {e:#}"
                        ));
                    }
                }

                if let Some(bytes) = pending_snapshot {
                    publish_alt_screen_snapshot(&sup, &session_id, &bytes, crate::files::RealFs)
                        .await;
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
                match stop_error {
                    Some(message) => {
                        send_reply(
                            &tx,
                            &ControlMsg::Error {
                                req_id,
                                message,
                                kind: ErrorKind::Internal,
                            },
                        )
                        .await
                    }
                    None => send_reply(&tx, &ControlMsg::SessionStopped { req_id }).await,
                }
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
                    )
                    .await;
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
                            )
                            .await;
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
                    )
                    .await;
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
                        // Dropped with the rest: the forwarder is being
                        // aborted anyway, and a dropped sender simply
                        // makes its pause watch unobservable.
                        pause: _pause,
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
                    // Settles this session's create reservations in the
                    // same transaction as the row removal, which is what
                    // turns them into TOMBSTONES rather than stale claims:
                    // a replay of one of those intent keys must report the
                    // gone-error, never a dead id and never a fresh
                    // duplicate (PLAN_M3.md item 6; the store method's own
                    // docs carry the argument).
                    sup.store
                        .delete_session_settling_reservations(&session_id)
                        .await
                        .map_err(|e| format!("{e:#}"))
                }
                .await;

                if let Err(err_msg) = teardown {
                    if let Some((channel, notify)) = notify_detach {
                        notify_detached(
                            &notify,
                            channel,
                            format!("detached during a failed delete: {err_msg}"),
                        );
                    }
                    drop(attachments);
                    send_reply(
                        &tx,
                        &ControlMsg::Error {
                            req_id,
                            message: err_msg,
                            kind: ErrorKind::Internal,
                        },
                    )
                    .await;
                    return;
                }
                sup.sessions.lock().await.remove(&session_id);

                if let Some((channel, notify)) = notify_detach {
                    notify_detached(&notify, channel, "session deleted".to_string());
                }
                drop(attachments);
                send_reply(&tx, &ControlMsg::SessionDeleted { req_id }).await;
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
                send_reply(
                    tx,
                    &ControlMsg::Error {
                        req_id,
                        message,
                        kind: ErrorKind::InvalidRequest,
                    },
                )
                .await;
                return;
            }
            let entry = sup.sessions.lock().await.get(&session_id).cloned();
            let Some(entry) = entry else {
                send_reply(
                    tx,
                    &ControlMsg::Error {
                        req_id,
                        message: format!("no such session: {}", truncate_for_error(&session_id)),
                        kind: ErrorKind::NotFound,
                    },
                )
                .await;
                return;
            };
            // The restart-gap case (PLAN_M2.md): this entry was reloaded
            // from SQLite at startup and its tmux session was gone by
            // then. Reporting `NotFound` here — rather than fabricating a
            // dead terminal to attach to — is the same "do not guess"
            // discipline SPEC.md applies elsewhere; the session stays
            // visible in the list either way.
            let Some(terminal) = entry.terminal.as_ref() else {
                send_reply(
                    tx,
                    &ControlMsg::Error {
                        req_id,
                        message: format!(
                            "session {session_id} has no terminal: the supervisor (or its tmux \
                         server) restarted after the agent ended"
                        ),
                        kind: ErrorKind::NotFound,
                    },
                )
                .await;
                return;
            };

            // Reserve one writer slot BEFORE taking any lock, so this
            // arm's eventual reply — success or failure — can be enqueued
            // without awaiting. Everything below either holds
            // `attachments` or must not race the forwarder it is about to
            // spawn, and awaiting a bounded queue in either position is
            // what this reservation exists to make impossible. Waiting
            // here instead is exactly right: it backpressures the
            // connection's read loop before the attach has touched
            // anything.
            let Ok(permit) = tx.reserve().await else {
                // The connection is gone; there is nobody to attach for.
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
                notify_detached(
                    &old.notify,
                    old.channel,
                    "another client attached".to_string(),
                );
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
            let (modes, prefill, stream) = match sup
                .tmux
                .open_replay_stream(&terminal.tmux_name, &terminal.pane)
                .await
            {
                Ok(parts) => parts,
                Err(e) => {
                    drop(attachments);
                    permit.send(reply_frame(&ControlMsg::Error {
                        req_id,
                        message: format!("{e:#}"),
                        kind: error_kind(&e),
                    }));
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
                    permit.send(reply_frame(&ControlMsg::Error {
                        req_id,
                        message: format!("{e:#}"),
                        kind: error_kind(&e),
                    }));
                    return;
                }
            };

            // The `Attached` reply is enqueued HERE, before the forwarder
            // exists, using the capacity reserved before this handler took
            // any lock. Both halves matter. It must precede the replay so
            // the client's `attach()` can return and its consumer can
            // start draining — otherwise a large replay floods the helm's
            // bounded per-terminal queue while nobody is allowed to read
            // it yet, and a perfectly healthy attach trips the
            // stalled-terminal detach. And it must not AWAIT here, because
            // `attachments` is held: `permit` makes the enqueue
            // infallible and instant.
            permit.send(reply_frame(&ControlMsg::Attached { req_id, channel }));

            // Everything from here on — the replay prefill, the dead-pane
            // snapshot, and the live pump — happens inside the forwarder
            // task rather than here, and that placement is load-bearing.
            // A full replay is megabytes of 32 KiB frames, and this
            // handler runs under the supervisor-wide `attachments` mutex;
            // sending them here would mean AWAITING a bounded queue (see
            // CONNECTION_WRITER_QUEUE) with that lock held, letting one
            // slow client stall every other session's attach and input.
            // Ordering is unaffected: the forwarder is this channel's
            // only writer, so its prefill necessarily precedes its own
            // live output. The dead-pane stop snapshot moved with it —
            // see `Forwarder::send_dead_pane_snapshot`.
            let (pause_tx, pause_rx) = watch::channel(None);
            let forwarder = Forwarder {
                sup: Arc::clone(sup),
                session_id: session_id.clone(),
                pane: terminal.pane.clone(),
                channel,
                tx: tx.clone(),
                stream,
                pause_rx,
                stall_timeout: sup.timeouts.stall_detach,
            };
            let task = tokio::spawn(forwarder.run(modes, prefill));

            attachments.insert(
                session_id.clone(),
                ActiveAttach {
                    channel,
                    notify: tx.clone(),
                    forwarder: task,
                    input,
                    pause: pause_tx,
                },
            );
            drop(attachments);
            input_routes.insert(channel, entry);
        }
        ControlMsg::PauseOutput { channel } => {
            set_attachment_paused(sup, tx, channel, true).await;
        }
        ControlMsg::ResumeOutput { channel } => {
            set_attachment_paused(sup, tx, channel, false).await;
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
        // TEMPORARY, until PLAN_M3.md item 9 lands the real handler:
        // `RestartSession` carries a `req_id` a caller is genuinely
        // blocked on (unlike the fire-and-forget `PauseOutput`/
        // `ResumeOutput` vocabulary-only precedent), so falling through
        // to the generic "unexpected control message" catch-all below
        // would silently hang every v5 caller that sends one on this
        // build forever. An explicit, honestly-labeled `Error` reply is
        // the minimal truthful behavior a vocabulary-only PR can give it;
        // item 9 replaces this arm with the real restart sequence and
        // this comment (and the temporary message text) go with it.
        ControlMsg::RestartSession { req_id, .. } => {
            send_reply(
                tx,
                &ControlMsg::Error {
                    req_id,
                    message: "restart not implemented in this build".to_string(),
                    kind: ErrorKind::Internal,
                },
            )
            .await;
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

    /// Item 8: the alt-screen snapshot write must be injectable through
    /// its REAL production call site — `publish_alt_screen_snapshot`
    /// itself, called directly against a real `Supervisor` (constructed
    /// the same lightweight way `create_session_over_field_cap_...`
    /// does), not a synthetic call into `crate::files`. A seam that fails
    /// the write step must leave no snapshot file behind at all.
    #[tokio::test]
    async fn publish_alt_screen_snapshot_surfaces_an_injected_write_failure() {
        #[derive(Clone, Copy)]
        struct FailWrite;
        impl crate::files::FaultSeam for FailWrite {
            fn write(&self, _file: &mut std::fs::File, _bytes: &[u8]) -> std::io::Result<()> {
                Err(std::io::Error::other("injected snapshot write failure"))
            }
        }

        let state = tempfile::tempdir().expect("state dir");
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        let session_id = "test-session".to_string();
        sup.sessions.lock().await.insert(
            session_id.clone(),
            Arc::new(SessionEntry {
                info: SessionInfo {
                    id: session_id.clone(),
                    title: "t".to_string(),
                    cwd: "/".to_string(),
                    invocation: "agent".to_string(),
                    status: SessionStatus::Unknown,
                    annotation: None,
                    restart_offer: RestartOffer::default(),
                },
                terminal: None,
                outcome: std::sync::Mutex::new(LastOutcome::Running),
            }),
        );

        publish_alt_screen_snapshot(&sup, &session_id, b"frame bytes", FailWrite).await;

        assert!(
            !snapshot_path(&sup.state_dir, &session_id).exists(),
            "an injected write failure must never publish a partial snapshot"
        );
    }

    /// Item 1's regression, and the reason `sweep_launch_dir` exists at
    /// all instead of the old blanket "remove everything" sweep: a
    /// durable exec-failure sentinel must survive this sweep no matter
    /// what, even for a session no longer tracked (there is no session in
    /// this test at all) — only PR5's future classifier, or an explicit
    /// delete, may ever remove one. A staged temp file and an ORPHANED
    /// spec (its session id absent from `sessions`) are seeded alongside
    /// it and must both go, proving the sweep does not simply skip the
    /// whole directory.
    #[tokio::test]
    async fn sweep_launch_dir_never_removes_a_sentinel() {
        let tmp = tempfile::tempdir().unwrap();
        let launch_dir = tmp.path().join("launch");
        std::fs::create_dir(&launch_dir).unwrap();
        std::fs::write(
            launch_dir.join("abc.status"),
            b"exec_failed argv0=x errno=2",
        )
        .unwrap();
        std::fs::write(launch_dir.join("orphan.json"), b"{}").unwrap();
        std::fs::write(launch_dir.join(".orphan.json.tmp-deadbeef"), b"partial").unwrap();

        sweep_launch_dir(&launch_dir, &std::collections::HashSet::new()).await;

        assert!(
            launch_dir.join("abc.status").exists(),
            "a sentinel must never be removed by this sweep, regardless of session ownership"
        );
        assert!(
            !launch_dir.join("orphan.json").exists(),
            "a spec whose session id owns nothing in `sessions` must be removed"
        );
        assert!(
            !launch_dir.join(".orphan.json.tmp-deadbeef").exists(),
            "a staged temp file must always be removed"
        );
    }

    /// Item 22's restart race: a spec whose session id IS still present
    /// in `sessions` must survive the sweep untouched — a supervisor
    /// restart does not kill tmux, so the login shell behind that session
    /// can still be mid-flight toward reading this exact spec, arbitrarily
    /// long after the window itself was created.
    #[tokio::test]
    async fn sweep_launch_dir_preserves_a_spec_for_a_surviving_session() {
        let tmp = tempfile::tempdir().unwrap();
        let launch_dir = tmp.path().join("launch");
        std::fs::create_dir(&launch_dir).unwrap();
        std::fs::write(launch_dir.join("live.json"), b"{}").unwrap();

        let mut sessions = std::collections::HashSet::new();
        sessions.insert("live".to_string());
        sweep_launch_dir(&launch_dir, &sessions).await;

        assert!(
            launch_dir.join("live.json").exists(),
            "a spec for a session still on record must survive — its shim may still be \
             mid-flight toward reading it"
        );
    }

    /// Item 2's new sweep: a planted `.tmux.conf.tmp-*` orphan must be
    /// removed while a REAL, current `tmux.conf` right next to it survives
    /// untouched — proving the sweep is name-scoped rather than a blanket
    /// removal of the state-dir root.
    #[tokio::test]
    async fn sweep_tmux_config_temp_files_removes_only_the_orphan() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("tmux.conf"), b"set -g exit-empty off\n").unwrap();
        std::fs::write(tmp.path().join(".tmux.conf.tmp-deadbeef"), b"partial").unwrap();

        sweep_tmux_config_temp_files(tmp.path()).await;

        assert!(
            tmp.path().join("tmux.conf").exists(),
            "the real, current tmux config must never be removed by this sweep"
        );
        assert!(
            !tmp.path().join(".tmux.conf.tmp-deadbeef").exists(),
            "an orphaned tmux-config temp file must be removed"
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
                annotation: None,
                restart_offer: RestartOffer::default(),
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
                annotation: None,
                restart_offer: RestartOffer::default(),
            },
        };
        assert_eq!(reply_frame(&msg), Frame::control(&msg));
    }

    /// `SessionRestarted` joined `reply_frame`'s req_id correlator alongside
    /// the helm demux (PLAN_M3 review batch item 5): this is the
    /// `unreachable!`-on-unknown-variant match, so proving it accepts the
    /// new variant here — rather than only via the round-trip tests in
    /// farhelm-proto — is what would catch a future refactor that forgets
    /// this arm and reintroduces the panic for a message `handle_control`'s
    /// temporary `RestartSession` arm actually sends today.
    #[test]
    fn reply_frame_accepts_session_restarted() {
        let msg = ControlMsg::SessionRestarted {
            req_id: 9,
            session: SessionInfo {
                id: "s1".to_string(),
                title: "demo".to_string(),
                cwd: "/tmp".to_string(),
                invocation: "agent".to_string(),
                status: SessionStatus::Alive,
                annotation: None,
                restart_offer: RestartOffer::Resume,
            },
        };
        assert_eq!(reply_frame(&msg), Frame::control(&msg));
    }

    /// PLAN_M3 review batch item 4: `RestartSession` has a `req_id` a
    /// caller genuinely blocks on, so this build must answer it with an
    /// honest `Error` rather than silently falling into the generic
    /// unknown-message catch-all (which would hang a v5 caller forever —
    /// unlike `PauseOutput`/`ResumeOutput`'s fire-and-forget precedent, a
    /// dropped reply here is a real bug, not a no-op). This is temporary
    /// scaffolding removed once PLAN_M3.md item 9 lands the real handler.
    #[tokio::test]
    async fn restart_session_replies_with_a_temporary_not_implemented_error() {
        let state = tempfile::tempdir().expect("state dir");
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();

        handle_control(
            &sup,
            ControlMsg::RestartSession {
                req_id: 5,
                session_id: "does-not-matter".to_string(),
                mode: farhelm_proto::RestartMode::Fresh,
                stop_if_running: false,
            },
            &tx,
            &mut input_routes,
            &mut tasks,
        )
        .await;

        let reply = rx.try_recv().expect("a reply must have been sent");
        let decoded: ControlMsg = serde_json::from_slice(&reply.body).unwrap();
        assert!(matches!(
            decoded,
            ControlMsg::Error {
                req_id: 5,
                kind: ErrorKind::Internal,
                ..
            }
        ));
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

    /// A session entry with the given terminal and recorded outcome, for
    /// the classification tests below — which are about how those two
    /// inputs combine, and need no tmux, no store, and no session at all.
    fn entry_with(terminal: Option<Terminal>, outcome: LastOutcome) -> SessionEntry {
        SessionEntry {
            info: SessionInfo {
                id: "s1".to_string(),
                title: "t".to_string(),
                cwd: "/tmp".to_string(),
                invocation: "agent".to_string(),
                status: SessionStatus::default(),
                annotation: None,
                restart_offer: RestartOffer::default(),
            },
            terminal,
            outcome: std::sync::Mutex::new(outcome),
        }
    }

    /// The one terminal the classification tests below use. A function
    /// rather than a shared value because `Terminal` is deliberately not
    /// `Clone` — no production path duplicates one, and adding the derive
    /// for a test would weaken that.
    fn a_terminal() -> Terminal {
        Terminal {
            tmux_name: "fh-1".to_string(),
            pane: "%0".to_string(),
        }
    }

    /// A `pane_states` map containing exactly [`a_terminal`]'s pane in the
    /// given state.
    fn pane_map(dead: bool, exit_code: Option<i32>) -> HashMap<String, PaneState> {
        HashMap::from([(
            "%0".to_string(),
            PaneState {
                session_name: "fh-1".to_string(),
                dead,
                exit_code,
            },
        )])
    }

    /// The classification precedence PLAN_M3.md items 2 and 3 define, in
    /// one place: what a live probe says, what the durable record says,
    /// and which wins where. Table-driven because the RELATIONSHIPS are
    /// the contract — each case in isolation looks obvious, and only side
    /// by side do the two inversions stand out (a live pane beating a
    /// recorded outcome, and a recorded outcome beating "no pane found").
    #[test]
    fn classification_precedence_between_live_probing_and_the_recorded_outcome() {
        let live = pane_map(false, None);
        let dead = pane_map(true, Some(3));
        let empty = HashMap::new();

        // A live pane outranks a stale record: what can still be observed
        // is never overridden by what was once written down.
        assert_eq!(
            session_status(
                &entry_with(Some(a_terminal()), LastOutcome::Launching),
                &live
            ),
            (SessionStatus::Alive, None)
        );

        // A dead pane's own code is the answer, and the record supplies
        // the annotation the pane cannot know.
        assert_eq!(
            session_status(
                &entry_with(
                    Some(a_terminal()),
                    LastOutcome::Exited {
                        exit_code: None,
                        annotation: Some("stopped by user".to_string()),
                    }
                ),
                &dead
            ),
            (
                SessionStatus::Exited { exit_code: Some(3) },
                Some("stopped by user".to_string())
            )
        );

        // No pane to ask: the record answers, and interrupted is NOT
        // flattened into exited-unknown — the whole point of the state.
        assert_eq!(
            session_status(&entry_with(None, LastOutcome::Interrupted), &empty),
            (SessionStatus::Interrupted, None)
        );
        assert_eq!(
            session_status(
                &entry_with(
                    None,
                    LastOutcome::Exited {
                        exit_code: Some(7),
                        annotation: Some("stopped by user".to_string()),
                    }
                ),
                &empty
            ),
            (
                SessionStatus::Exited { exit_code: Some(7) },
                Some("stopped by user".to_string())
            ),
            "a code and annotation witnessed before the terminal vanished are retained \
             knowledge, not a guess"
        );

        // Nothing observed and nothing recorded: M2's honest fallback.
        assert_eq!(
            session_status(&entry_with(None, LastOutcome::Running), &empty),
            (SessionStatus::Exited { exit_code: None }, None)
        );

        // The seam PLAN_M3.md item 3 slots into: a recorded error outranks
        // every inference, including a pane tmux would call alive.
        assert_eq!(
            session_status(
                &entry_with(
                    Some(a_terminal()),
                    LastOutcome::Error {
                        detail: "Permission denied".to_string()
                    }
                ),
                &live
            ),
            (
                SessionStatus::Error {
                    detail: "Permission denied".to_string()
                },
                None
            )
        );
    }

    /// What each observation OFFERS the store, which is the half
    /// `session_status` does not decide. Two silences matter more than the
    /// writes: a terminal outcome is not re-observed at all (nothing a
    /// probe can see adds to it), and a `Launching` row with no pane
    /// offers nothing — "no side effects found" is not evidence the agent
    /// ran, and recording an exit for it would claim exactly that
    /// (PLAN_M3.md item 2 sends that row to item 3/6 instead).
    ///
    /// The enrichment case is the one a naive "already terminal, skip it"
    /// rule gets wrong: tmux publishes `pane_dead` before
    /// `pane_dead_status` is readable, so the poll that first sees the
    /// death routinely has no code while the next one does.
    #[test]
    fn observations_offered_to_the_store_cover_silence_and_enrichment() {
        let dead_with_code = PaneState {
            session_name: "fh-1".to_string(),
            dead: true,
            exit_code: Some(3),
        };
        let dead_without_code = PaneState {
            session_name: "fh-1".to_string(),
            dead: true,
            exit_code: None,
        };
        let alive = PaneState {
            session_name: "fh-1".to_string(),
            dead: false,
            exit_code: None,
        };

        assert_eq!(observation(&LastOutcome::Running, Some(&alive)), None);
        assert_eq!(
            observation(&LastOutcome::Running, Some(&dead_with_code)),
            Some(Transition::ObservedExit { exit_code: Some(3) })
        );
        assert_eq!(
            observation(&LastOutcome::Running, None),
            Some(Transition::ObservedExit { exit_code: None })
        );
        assert_eq!(
            observation(&LastOutcome::StopRequested, None),
            Some(Transition::ObservedExit { exit_code: None }),
            "a stop whose terminal vanished still ended; the store decides it was the stop"
        );
        assert_eq!(
            observation(&LastOutcome::Launching, None),
            None,
            "a launch with no side effects has not been shown to have run"
        );
        assert_eq!(
            observation(&LastOutcome::Interrupted, None),
            None,
            "a reboot is never re-observed into something poorer"
        );
        assert_eq!(
            observation(
                &LastOutcome::Exited {
                    exit_code: None,
                    annotation: None
                },
                Some(&dead_with_code)
            ),
            Some(Transition::ObservedExit { exit_code: Some(3) }),
            "a code tmux can only now report must still reach the record"
        );
        assert_eq!(
            observation(
                &LastOutcome::Exited {
                    exit_code: Some(3),
                    annotation: None
                },
                Some(&dead_without_code)
            ),
            None,
            "a known code is never re-offered to be replaced by a missing one"
        );
    }

    /// The launching row's OTHER reconciliation, and the one with no
    /// coverage before this test: a crash between the durable launching
    /// record and the confirmation leaves a row whose pane is unknown but
    /// whose tmux session very much exists. Reload has to find that pane
    /// by session name — the row has no pane id to look up — and then say
    /// what it sees: a live pane confirms the launch (`Running`, with the
    /// rediscovered pane now recorded), and a dead one is an exit with
    /// whatever code the pane retained.
    ///
    /// Driven against a real tmux server through the real reload, because
    /// the name-lookup path is exactly the kind of code that looks obvious
    /// and silently matches nothing.
    #[tokio::test]
    async fn reload_rediscovers_the_pane_of_a_launching_row_that_did_launch() {
        let state = tempfile::tempdir().expect("state dir");
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");

        // Two sessions tmux really has, neither of which any row knows
        // the pane of: one still running, one already dead (the tmux
        // config keeps dead panes around — that is what makes an exit
        // code readable at all).
        for (name, command) in [("fh-live", "sleep 300"), ("fh-dead", "exit 7")] {
            let argv = ["sh".to_string(), "-c".to_string(), command.to_string()];
            sup.tmux
                .create_session(name, "/tmp", 80, 24, &argv)
                .await
                .expect("create a tmux session directly");
        }
        for (id, tmux_name) in [("live", "fh-live"), ("dead", "fh-dead")] {
            sup.store
                .insert_session(
                    StoredSession {
                        id: id.to_string(),
                        title: id.to_string(),
                        cwd: "/tmp".to_string(),
                        invocation: "agent".to_string(),
                        tmux_name: tmux_name.to_string(),
                        pane: String::new(),
                        outcome: LastOutcome::Launching,
                    },
                    None,
                )
                .await
                .expect("insert a launching row");
        }
        // The dead pane has to actually be dead before the reload asks.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        loop {
            let states = sup.tmux.pane_states().await.expect("pane states");
            if states
                .values()
                .any(|state| state.session_name == "fh-dead" && state.dead)
            {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "the fixture pane never died"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        let (sessions, _) = Supervisor::reload_sessions(
            &sup.state_dir,
            &sup.store,
            &sup.tmux,
            &SupervisorSeams::default(),
            true,
        )
        .await
        .expect("reload");

        assert_eq!(
            *sessions["live"].outcome.lock().unwrap(),
            LastOutcome::Running,
            "a launching row whose pane is alive is a launch that DID happen"
        );
        assert!(
            sessions["live"].terminal.is_some(),
            "and it must be attachable again, which needs the rediscovered pane"
        );
        let dead = sessions["dead"].outcome.lock().unwrap().clone();
        match dead {
            LastOutcome::Exited { exit_code, .. } => {
                if let Some(code) = exit_code {
                    assert_eq!(code, 7, "the pane's own retained status, not a guess");
                }
            }
            other => panic!("a launching row whose pane is dead has exited, got {other:?}"),
        }

        // Both facts are DURABLE, and the rediscovered pane is stored
        // with the outcome it evidences — a later reload must not have to
        // rediscover anything.
        let rows = sup.store.load_all().await.expect("load");
        for row in rows {
            assert!(
                !row.pane.is_empty(),
                "the rediscovered pane must be recorded, not merely used once"
            );
            assert_ne!(row.outcome, LastOutcome::Launching);
        }
    }

    /// PLAN_M3.md item 4's crash boundary, both edges: what reload makes
    /// of a stop intent whose sweep never reported back.
    ///
    /// The intent is written before any signal is sent, so a crash can
    /// leave it standing over either outcome, and the two must be read
    /// oppositely. A DEAD pane means the kill landed: the session ended
    /// because the user stopped it, and the exit is annotated — without
    /// this, a crash mid-sweep silently converted "the user stopped this"
    /// into "the agent finished on its own". A LIVE pane means it never
    /// landed: the intent is cleared and the session goes back to being
    /// ordinary, because annotating a process that is still running would
    /// be a claim about an ending that has not happened.
    ///
    /// Written against a real tmux server through the real reload, since
    /// the reconciliation is a property of that pass and not of the
    /// transition policy alone.
    #[tokio::test]
    async fn reload_reconciles_a_stop_intent_against_the_pane_it_left_behind() {
        let state = tempfile::tempdir().expect("state dir");
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");

        let mut panes = HashMap::new();
        for (id, command) in [("landed", "exit 0"), ("never-landed", "sleep 300")] {
            let tmux_name = format!("fh-{id}");
            let argv = ["sh".to_string(), "-c".to_string(), command.to_string()];
            let pane = sup
                .tmux
                .create_session(&tmux_name, "/tmp", 80, 24, &argv)
                .await
                .expect("create a tmux session directly");
            sup.store
                .insert_session(
                    StoredSession {
                        id: id.to_string(),
                        title: id.to_string(),
                        cwd: "/tmp".to_string(),
                        invocation: "agent".to_string(),
                        tmux_name,
                        pane: pane.clone(),
                        outcome: LastOutcome::Running,
                    },
                    None,
                )
                .await
                .expect("insert");
            // Through the real transition, so the fixture is the state a
            // real interrupted stop leaves behind rather than a hand-made
            // approximation of it.
            sup.store
                .transition(id, Transition::StopRequested)
                .await
                .expect("record the intent");
            panes.insert(id, pane);
        }
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        loop {
            let states = sup.tmux.pane_states().await.expect("pane states");
            if states.get(&panes["landed"]).is_some_and(|state| state.dead) {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "the fixture pane never died"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        Supervisor::reload_sessions(
            &sup.state_dir,
            &sup.store,
            &sup.tmux,
            &SupervisorSeams::default(),
            true,
        )
        .await
        .expect("reload");

        let rows: HashMap<String, LastOutcome> = sup
            .store
            .load_all()
            .await
            .expect("load")
            .into_iter()
            .map(|row| (row.id, row.outcome))
            .collect();
        match &rows["landed"] {
            LastOutcome::Exited { annotation, .. } => assert_eq!(
                annotation.as_deref(),
                Some(farhelm_proto::STOP_ANNOTATION),
                "a stop intent over a dead pane is a stop that landed"
            ),
            other => panic!("expected an annotated exit, got {other:?}"),
        }
        assert_eq!(
            rows["never-landed"],
            LastOutcome::Running,
            "a stop intent over a LIVE pane never landed, and must not annotate a session \
             that is still running"
        );
    }

    /// SPEC.md surfaces every failure: a stop whose durable record cannot
    /// be written must REPORT that, not quietly succeed.
    ///
    /// The intent is written before any signal is sent, so a failure there
    /// means nothing was killed at all — and the reply says exactly that,
    /// because a caller told "stopped" would otherwise stop asking about a
    /// session whose agent is still running. (The other half, a failure
    /// recording the outcome AFTER a successful kill, reports the opposite
    /// nuance: the session really did stop, but may list as a plain exit.)
    ///
    /// The failure is injected by corrupting the row out of band, which is
    /// how an unreadable row fails for real: inside the transaction,
    /// before anything is written.
    #[tokio::test]
    async fn a_stop_that_cannot_record_its_intent_reports_instead_of_killing() {
        let state = tempfile::tempdir().expect("state dir");
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        sup.store
            .insert_session(
                StoredSession {
                    id: "s1".to_string(),
                    title: "t".to_string(),
                    cwd: "/tmp".to_string(),
                    invocation: "agent".to_string(),
                    tmux_name: "fh-1".to_string(),
                    pane: "%0".to_string(),
                    outcome: LastOutcome::Running,
                },
                None,
            )
            .await
            .expect("insert");
        sup.sessions.lock().await.insert(
            "s1".to_string(),
            Arc::new(entry_with(None, LastOutcome::Running)),
        );
        {
            let conn = rusqlite::Connection::open(state.path().join("supervisor.db"))
                .expect("open the database directly");
            conn.execute("UPDATE sessions SET outcome_state = 'teleported'", [])
                .expect("corrupt the row");
        }

        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        handle_control(
            &sup,
            ControlMsg::StopSession {
                req_id: 4,
                session_id: "s1".to_string(),
            },
            &tx,
            &mut input_routes,
            &mut tasks,
        )
        .await;
        let reply = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("the spawned stop handler never replied")
            .expect("reply channel closed");
        let decoded: ControlMsg = serde_json::from_slice(&reply.body).unwrap();
        let ControlMsg::Error { message, kind, .. } = decoded else {
            panic!("a stop that could not be recorded must not report success: {decoded:?}");
        };
        assert_eq!(kind, ErrorKind::Internal);
        assert!(
            message.contains("nothing was killed"),
            "the caller must be told the stop did not happen: {message}"
        );
    }

    /// The mirror follows the DATABASE, never the caller's intent: a
    /// failed write must leave the in-memory outcome exactly as it was, so
    /// the map never claims something SQLite does not say, and the next
    /// observation simply retries.
    ///
    /// The failure is injected by corrupting the row out of band (a state
    /// string outside this build's vocabulary), which is how a real
    /// unreadable row fails: inside the transaction, before anything is
    /// written. No test-only seam is needed — the database is a file, and
    /// this test opens it the same way a `sqlite3` prompt would.
    #[tokio::test]
    async fn a_failed_outcome_write_leaves_the_mirror_untouched() {
        let state = tempfile::tempdir().expect("state dir");
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        sup.store
            .insert_session(
                StoredSession {
                    id: "s1".to_string(),
                    title: "t".to_string(),
                    cwd: "/tmp".to_string(),
                    invocation: "agent".to_string(),
                    tmux_name: "fh-1".to_string(),
                    pane: "%0".to_string(),
                    outcome: LastOutcome::Running,
                },
                None,
            )
            .await
            .expect("insert");
        {
            let conn = rusqlite::Connection::open(state.path().join("supervisor.db"))
                .expect("open the database directly");
            conn.execute("UPDATE sessions SET outcome_state = 'teleported'", [])
                .expect("corrupt the row");
        }

        let entry = entry_with(None, LastOutcome::Running);
        let err = sup
            .record("s1", &entry, Transition::ObservedExit { exit_code: None })
            .await
            .expect_err("an unreadable row must fail the write");
        assert!(format!("{err:#}").contains("teleported"));
        assert_eq!(
            *entry.outcome.lock().unwrap(),
            LastOutcome::Running,
            "a failed write must not advance the mirror"
        );
    }

    /// PLAN_M3.md item 2's exclusivity requirement, in its second and
    /// quieter form: a supervisor that does NOT hold the state
    /// directory's claim must not reconcile anything durably.
    ///
    /// The scenario is an ordinary handoff — a candidate constructing
    /// while the incumbent is still serving. Its reload sees the
    /// incumbent's sessions and, with a tmux server that knows nothing
    /// about this row, would conclude "exited, unknown code" for every one
    /// of them. Writing that would be permanent (terminal outcomes are
    /// sticky) and wrong (the incumbent's agent is running), so the
    /// candidate classifies for its own map and writes nothing.
    ///
    /// The incumbent is a bare `flock` on the lock file rather than
    /// another `Supervisor`, and that is the whole point: `flock`
    /// conflicts across open file descriptions even within one process, so
    /// this reproduces the CROSS-PROCESS refusal exactly, at the syscall
    /// level, without spawning anything. (Two `Supervisor`s in one process
    /// deliberately share a claim — see `StateDirOwnership` — so building
    /// a second one here would test nothing.)
    ///
    /// Both halves are checked: the row must be untouched, and the
    /// candidate must still have CLASSIFIED it — "wrote nothing" must not
    /// be achieved by "computed nothing".
    #[tokio::test]
    async fn a_supervisor_without_the_state_dir_claim_reconciles_nothing_durably() {
        let state = tempfile::tempdir().expect("state dir");
        let db_path = state.path().join("supervisor.db");
        let store = SessionStore::open(&db_path, true).await.expect("store");
        store
            .insert_session(
                StoredSession {
                    id: "s1".to_string(),
                    title: "t".to_string(),
                    cwd: "/tmp".to_string(),
                    invocation: "agent".to_string(),
                    tmux_name: "fh-does-not-exist".to_string(),
                    pane: "%0".to_string(),
                    outcome: LastOutcome::Running,
                },
                None,
            )
            .await
            .expect("insert");
        drop(store);

        let incumbent = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(state.path().join("supervisor.lock"))
            .expect("lock file");
        incumbent.try_lock().expect("the incumbent takes the claim");

        let candidate = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("a candidate may still construct during a handoff");
        assert!(
            candidate.ownership.is_none(),
            "the incumbent holds the claim, so the candidate must have none"
        );

        let store = SessionStore::open(&db_path, true).await.expect("store");
        assert_eq!(
            store.load_all().await.expect("load")[0].outcome,
            LastOutcome::Running,
            "a claimless supervisor must not record the incumbent's live session as ended"
        );
        let entry = candidate
            .sessions
            .lock()
            .await
            .get("s1")
            .cloned()
            .expect("the candidate still lists the session");
        assert_eq!(
            *entry.outcome.lock().unwrap(),
            LastOutcome::Running,
            "the candidate's map holds what is durable, not an unwritten conclusion"
        );
        assert_eq!(
            session_status(&entry, &HashMap::new()).0,
            SessionStatus::Exited { exit_code: None },
            "it still classifies honestly for its own replies"
        );
    }

    /// `serve` is what enforces "at most one supervisor per user per
    /// host": the file lock alone cannot tell two `Supervisor`s in ONE
    /// process apart (they share the claim by design — see
    /// `StateDirOwnership`), so the serving flag has to. Pinned here at
    /// the unit level because the e2e suite exercises the cross-process
    /// shape but constructs both supervisors in one process, which is
    /// exactly the case this flag covers.
    #[tokio::test]
    async fn a_second_serve_on_the_same_state_dir_is_refused() {
        let state = tempfile::tempdir().expect("state dir");
        let first = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let second = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        assert!(
            first
                .ownership
                .as_ref()
                .expect("the first construction claims the dir")
                .begin_serving(),
            "nothing is serving yet"
        );
        let err = second
            .serve()
            .await
            .expect_err("a second serve must be refused");
        assert!(
            err.to_string().contains("already running"),
            "the refusal must say why: {err:#}"
        );
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
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
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
                intent_key: None,
                agent_kind: None,
                resume_template: None,
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

    /// Item 9: a launch spec that fails to publish must mean the tmux
    /// window that would `exec farhelm internal launch <spec>` never
    /// gets created at all — the shim must never have a CHANCE to
    /// observe a partial spec, because there must never BE one to
    /// observe. Driven through the REAL `create_session` call site (a
    /// genuine `EACCES` from an unwritable `launch/` directory, not a
    /// synthetic seam substitution), so this pins the actual ordering
    /// `create_session` relies on (`?` on the spec write, strictly before
    /// `self.tmux.create_session`) rather than only the write helper's
    /// own atomicity in isolation.
    #[tokio::test]
    async fn create_session_never_launches_tmux_after_a_failed_spec_publish() {
        let state = tempfile::tempdir().expect("state dir");
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                state.path().join("launch"),
                std::fs::Permissions::from_mode(0o500),
            )
            .expect("removing write permission from launch/");
        }

        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        handle_control(
            &sup,
            ControlMsg::CreateSession {
                req_id: 1,
                cwd: "/".to_string(),
                invocation: "agent".to_string(),
                title: None,
                cols: 80,
                rows: 24,
                intent_key: None,
                agent_kind: None,
                resume_template: None,
            },
            &tx,
            &mut input_routes,
            &mut tasks,
        )
        .await;

        let reply = rx.try_recv().expect("a reply must have been sent");
        let decoded: ControlMsg = serde_json::from_slice(&reply.body).unwrap();
        assert!(
            matches!(decoded, ControlMsg::Error { .. }),
            "create must fail when the spec cannot be published: {decoded:?}"
        );
        assert!(
            sup.sessions.lock().await.is_empty(),
            "no session may be recorded — proving no tmux window was ever created for a spec \
             that never made it to disk"
        );
        // The DURABLE half, which the in-memory map cannot show: the
        // launching row this create committed before touching the disk
        // must be rolled back, not merely left out of the session map.
        // The spec write is the first side effect, so its failure proves
        // nothing external happened and the row describes nothing.
        assert!(
            sup.store.load_all().await.expect("load").is_empty(),
            "a create that failed before any side effect must leave no phantom row behind"
        );
    }

    /// The fingerprint binds every SESSION-shaping field and nothing else
    /// (PLAN_M3.md item 6).
    ///
    /// The override cases are the ones with teeth: acceptance 7 requires a
    /// request differing ONLY in `agent_kind` or `resume_template` to be
    /// refused as a key reuse, and as of this PR the fingerprint is the
    /// only thing in the supervisor that reads those fields at all — so
    /// nothing else would catch a change that quietly dropped them. The
    /// `None`-vs-`Some` title case pins the other easy mistake: an omitted
    /// title asks the server to derive one, which is a different request
    /// from spelling out the same string by hand.
    #[test]
    fn the_create_fingerprint_covers_every_session_shaping_field() {
        let base = create_fingerprint("/work", "agent --flag", Some("t"), None, None);
        let cases = [
            (
                create_fingerprint("/other", "agent --flag", Some("t"), None, None),
                "cwd",
            ),
            (
                create_fingerprint("/work", "agent --other", Some("t"), None, None),
                "invocation",
            ),
            (
                create_fingerprint("/work", "agent --flag", Some("other"), None, None),
                "title",
            ),
            (
                create_fingerprint("/work", "agent --flag", None, None, None),
                "an omitted title",
            ),
            (
                create_fingerprint(
                    "/work",
                    "agent --flag",
                    Some("t"),
                    Some(AgentKind::Claude),
                    None,
                ),
                "the agent-kind override",
            ),
            (
                create_fingerprint(
                    "/work",
                    "agent --flag",
                    Some("t"),
                    None,
                    Some(&["claude".to_string(), "{conversation}".to_string()]),
                ),
                "the resume-template override",
            ),
        ];
        for (fingerprint, what) in cases {
            assert_ne!(fingerprint, base, "{what} must change the fingerprint");
        }
        assert_eq!(
            create_fingerprint("/work", "agent --flag", Some("t"), None, None),
            base,
            "the same request must fingerprint identically every time"
        );
        // Adjacent fields cannot bleed into one another: a delimiter-joined
        // encoding would make these two requests indistinguishable.
        assert_ne!(
            create_fingerprint("/a", "bc", None, None, None),
            create_fingerprint("/ab", "c", None, None, None),
        );
        // Distinct override VALUES are distinguished, not merely the
        // presence of an override: two integrated kinds are two different
        // requests, and so are two templates of the same length.
        assert_ne!(
            create_fingerprint("/work", "a", None, Some(AgentKind::Claude), None),
            create_fingerprint("/work", "a", None, Some(AgentKind::Codex), None),
        );
        assert_ne!(
            create_fingerprint("/work", "a", None, None, Some(&["x".to_string()])),
            create_fingerprint("/work", "a", None, None, Some(&["y".to_string()])),
        );
    }

    /// The PERSISTED fingerprint, byte for byte.
    ///
    /// A golden test because this string is written into a durable,
    /// never-pruned table and compared verbatim on every replay: any change
    /// to the encoding — a reordered element, a different `AgentKind`
    /// spelling, a switch to a digest — turns every stored fingerprint into
    /// a mismatch, so identical requests across the upgrade would be
    /// refused as key reuse. That is a migration, and this test is what
    /// makes it impossible to perform by accident. In particular the kind
    /// is spelled with this module's own vocabulary, so a future rename of
    /// the WIRE representation fails here rather than in the field.
    #[test]
    fn the_persisted_fingerprint_encoding_is_pinned() {
        assert_eq!(
            create_fingerprint(
                "/work",
                "claude --flag",
                Some("title"),
                Some(AgentKind::Claude),
                Some(&["claude".to_string(), "{conversation}".to_string()]),
            ),
            r#"["/work","claude --flag","title","claude",["claude","{conversation}"]]"#
        );
        assert_eq!(
            create_fingerprint("/work", "agent", None, None, None),
            r#"["/work","agent",null,null,null]"#
        );
    }

    /// Drive `handle_control`'s create arm three times against one intent
    /// key to pin the two halves of item 6 that need no successful launch:
    /// a failed create REPLAYS its original error, and a key reused for a
    /// request differing only in an override is a `Conflict`.
    ///
    /// The failure is provoked by an unwritable `launch/` (the same
    /// genuine `EACCES` `create_session_never_launches_tmux_after_a_failed_
    /// spec_publish` uses), and permissions are RESTORED before the
    /// replays — so a replay that re-ran the create instead of reading the
    /// stored outcome would visibly succeed rather than quietly return the
    /// same text, and this test would fail. The `kind` assertion is the
    /// other half of "the same answer": a replay that decayed to
    /// `Internal` would turn the first attempt's 400 into a 500.
    #[tokio::test]
    async fn a_failed_create_replays_its_error_and_a_changed_override_conflicts() {
        let state = tempfile::tempdir().expect("state dir");
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        let launch_dir = state.path().join("launch");
        let request = |req_id: u64, agent_kind: Option<AgentKind>| ControlMsg::CreateSession {
            req_id,
            cwd: "/".to_string(),
            invocation: "agent".to_string(),
            title: None,
            cols: 80,
            rows: 24,
            intent_key: Some("one-intent".to_string()),
            agent_kind,
            resume_template: None,
        };
        let reply = |rx: &mut mpsc::Receiver<Frame>| {
            let frame = rx.try_recv().expect("a reply must have been sent");
            serde_json::from_slice::<ControlMsg>(&frame.body).expect("decode")
        };

        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&launch_dir, std::fs::Permissions::from_mode(0o500))
                .expect("removing write permission from launch/");
        }
        handle_control(&sup, request(1, None), &tx, &mut input_routes, &mut tasks).await;
        let ControlMsg::Error {
            message: first_message,
            kind: first_kind,
            ..
        } = reply(&mut rx)
        else {
            panic!("the create must fail while launch/ is unwritable");
        };
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&launch_dir, std::fs::Permissions::from_mode(0o700))
                .expect("restoring launch/");
        }

        // Same key, same request: the ORIGINAL error, verbatim.
        handle_control(&sup, request(2, None), &tx, &mut input_routes, &mut tasks).await;
        let ControlMsg::Error { message, kind, .. } = reply(&mut rx) else {
            panic!("a replayed failure must still be an error");
        };
        assert_eq!(message, first_message, "the replay must be the same answer");
        assert_eq!(kind, first_kind);
        assert_eq!(
            kind,
            ErrorKind::Internal,
            "an unwritable state directory is not something the caller could have avoided"
        );

        // Same key, a request differing ONLY in the agent-kind override:
        // a reused key, refused rather than merged.
        handle_control(
            &sup,
            request(3, Some(AgentKind::Claude)),
            &tx,
            &mut input_routes,
            &mut tasks,
        )
        .await;
        let ControlMsg::Error { message, kind, .. } = reply(&mut rx) else {
            panic!("a key reused for a different request must be refused");
        };
        assert_eq!(kind, ErrorKind::Conflict);
        assert!(
            message.contains("one-intent") && message.contains("different create request"),
            "the refusal must name the key and say why: {message}"
        );

        assert!(
            sup.sessions.lock().await.is_empty(),
            "none of the three attempts may have created a session"
        );
    }

    /// The per-key lock actually excludes, actually hands off, and
    /// actually prunes.
    ///
    /// All three are load-bearing and none is visible from the outside: a
    /// lock that did not exclude would let two creates gather evidence
    /// about each other's in-flight launch (see [`IntentLocks`] for why
    /// that specific ambiguity is what it exists to remove), one that
    /// pruned too eagerly would hand a waiter a DIFFERENT mutex for the
    /// same key, and one that never pruned would grow a map entry per key
    /// this process has ever seen.
    #[tokio::test]
    async fn intent_locks_exclude_hand_off_and_prune() {
        let locks = Arc::new(IntentLocks::default());
        let first = locks.claim("key").await;
        assert_eq!(locks.locks.lock().unwrap().len(), 1);

        // A second claim on the same key blocks; a claim on a different
        // key does not.
        let waiter = {
            let locks = Arc::clone(&locks);
            tokio::spawn(async move { locks.claim("key").await })
        };
        let other = locks.claim("other-key").await;
        assert_eq!(locks.locks.lock().unwrap().len(), 2);
        tokio::task::yield_now().await;
        assert!(!waiter.is_finished(), "the same key must still be blocked");

        // Releasing hands the key over rather than dropping its entry: the
        // waiter is holding the SAME mutex it queued on, which is only
        // true if pruning skipped an entry with a live waiter.
        drop(first);
        let handed_over = waiter.await.expect("the waiter must acquire");
        assert!(
            locks.locks.lock().unwrap().contains_key("key"),
            "an entry with a live holder must not be pruned"
        );
        drop(handed_over);
        drop(other);
        assert!(
            locks.locks.lock().unwrap().is_empty(),
            "every entry must be gone once its last holder leaves"
        );
    }

    /// A delete that commits WHILE a create is mid-launch wins, and the
    /// create tears its own work back down instead of leaving an orphan.
    ///
    /// The race is the one item 6 of the review batch names, forced
    /// deterministically through the create-lifecycle seam: the delete is
    /// performed from inside the `DuringLaunch` stage, i.e. after tmux has
    /// the session and before the launch is confirmed — the exact window in
    /// which the create is about to write a row that no longer exists.
    /// Without the vanished-row check, that write is a silent no-op and the
    /// create returns success for a session with no record, leaving a real
    /// agent running that nothing can list, stop, or reap.
    ///
    /// A second connection is not needed to make this real: what matters is
    /// the ORDER of the durable operations, and the seam pins it exactly
    /// rather than hoping a spawned task interleaves the right way.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_delete_that_lands_mid_launch_wins_and_the_create_tears_down() {
        let state = tempfile::tempdir().expect("state dir");
        let db = state.path().join("supervisor.db");
        let deleted: Arc<std::sync::atomic::AtomicBool> = Arc::default();
        let sup = {
            let db = db.clone();
            let deleted = Arc::clone(&deleted);
            Supervisor::new_with_seams(
                state.path(),
                dummy_exe(),
                SupervisorTimeouts::default(),
                SupervisorSeams {
                    create_crash: Some(Arc::new(move |stage| {
                        if stage != CreateStage::DuringLaunch {
                            return Ok(());
                        }
                        // A second connection to the same database, which
                        // is what a concurrent handler would have: the
                        // busy timeout covers the overlap.
                        let db = db.clone();
                        let deleted = Arc::clone(&deleted);
                        tokio::task::block_in_place(move || {
                            tokio::runtime::Handle::current().block_on(async move {
                                let store = SessionStore::open(&db, false).await?;
                                for row in store.load_all().await? {
                                    store.delete_session_settling_reservations(&row.id).await?;
                                }
                                deleted.store(true, std::sync::atomic::Ordering::SeqCst);
                                anyhow::Ok(())
                            })
                        })?;
                        Ok(())
                    })),
                    ..SupervisorSeams::default()
                },
            )
            .await
            .expect("supervisor")
        };

        let error = sup
            .create_session(
                "/",
                "agent",
                None,
                80,
                24,
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint: create_fingerprint("/", "agent", None, None, None),
                }),
            )
            .await
            .expect_err("a create whose session was deleted mid-launch must not report success");
        assert!(
            deleted.load(std::sync::atomic::Ordering::SeqCst),
            "test fixture: the delete must actually have run"
        );
        assert_eq!(error_kind(&error), ErrorKind::Conflict);
        assert!(
            format!("{error:#}").contains("deleted while it was being created"),
            "the caller must be told which way the race went: {error:#}"
        );
        assert!(
            sup.store.load_all().await.expect("load").is_empty(),
            "the delete's removal must stand — the create must not have re-created a row"
        );
        let names = sup.tmux.pane_states().await.expect("pane states");
        assert!(
            names.is_empty(),
            "the create must have torn its own tmux session down rather than orphaning it: \
             {names:?}"
        );
        // And the intent is a tombstone, so a retry says so rather than
        // resurrecting the session the delete removed.
        let retry = sup
            .create_session(
                "/",
                "agent",
                None,
                80,
                24,
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint: create_fingerprint("/", "agent", None, None, None),
                }),
            )
            .await
            .expect_err("the key is spent");
        assert_eq!(error_kind(&retry), ErrorKind::Conflict);
        assert!(
            format!("{retry:#}").contains("deleted"),
            "the retry must report the tombstone: {retry:#}"
        );
    }

    /// A keyed create refused by VALIDATION is recorded as that intent's
    /// outcome and replayed — and the replay happens before validation
    /// runs again (PLAN_M3.md item 6's replay contract, and the ordering
    /// item 1 of the review batch pins).
    ///
    /// The working directory is CREATED between the two attempts, which is
    /// what makes this a real test of the ordering rather than of
    /// determinism: a retry that re-validated would now succeed and create
    /// a session, so the replayed refusal proves the reservation was
    /// consulted first. That is the contract acceptance 7 states without a
    /// precondition exception — one intent, one outcome, however the world
    /// moves underneath it.
    ///
    /// The third request pins the other half of the same ordering: a
    /// DIFFERENT fingerprint is refused as a key reuse even though its own
    /// cwd is invalid, which can only happen if the fingerprint is compared
    /// before the directory is looked at.
    #[tokio::test]
    async fn a_keyed_precondition_failure_is_recorded_and_replayed_verbatim() {
        let state = tempfile::tempdir().expect("state dir");
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let work = state.path().join("appears-later");
        let cwd = work.to_string_lossy().to_string();
        let claim = |fingerprint_cwd: &str, invocation: &str| IntentClaim {
            intent_key: "one-intent".to_string(),
            fingerprint: create_fingerprint(fingerprint_cwd, invocation, None, None, None),
        };

        let first = sup
            .create_session(&cwd, "agent", None, 80, 24, Some(claim(&cwd, "agent")))
            .await
            .expect_err("the working directory does not exist yet");
        assert_eq!(error_kind(&first), ErrorKind::InvalidRequest);
        assert!(
            format!("{first:#}").contains("does not exist"),
            "the refusal must name what was wrong: {first:#}"
        );

        std::fs::create_dir(&work).expect("the directory the user was about to create");
        let replay = sup
            .create_session(&cwd, "agent", None, 80, 24, Some(claim(&cwd, "agent")))
            .await
            .expect_err("the same intent must replay its refusal, not re-evaluate it");
        assert_eq!(format!("{replay:#}"), format!("{first:#}"));
        assert_eq!(error_kind(&replay), ErrorKind::InvalidRequest);
        assert!(
            sup.sessions.lock().await.is_empty(),
            "and must not have created a session on the retry"
        );

        // Same key, different request, cwd still bad: refused for the
        // REUSE, which only the lookup-before-validation ordering can do.
        let reused = sup
            .create_session(
                "/nonexistent/definitely/not/here",
                "agent",
                None,
                80,
                24,
                Some(claim("/nonexistent/definitely/not/here", "agent")),
            )
            .await
            .expect_err("a reused key must be refused");
        assert_eq!(error_kind(&reused), ErrorKind::Conflict);
    }

    /// While a relaunch is in flight its session is absent from the
    /// session map, which is what serializes it against `StopSession` and
    /// `DeleteSession`.
    ///
    /// Both handlers resolve their target through that map and reply
    /// `NotFound` when it has no entry (their own arms in `handle_control`
    /// do the lookup first), so this absence is not an implementation
    /// detail — it is the mechanism that keeps a stop or a delete from
    /// tearing down a launch that is half-built, with no lock held across
    /// the multi-second work either of them does. The window at the other
    /// end, where a delete resolved its entry just BEFORE the relaunch
    /// removed it, is closed by the vanished-row check that
    /// `a_delete_that_lands_mid_launch_wins_and_the_create_tears_down`
    /// pins.
    ///
    /// Observed from inside the launch through the create-lifecycle seam,
    /// because that is the only instant the property is about.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_relaunch_hides_its_session_from_stop_and_delete_until_it_completes() {
        let state = tempfile::tempdir().expect("state dir");
        let seen: Arc<std::sync::Mutex<Option<bool>>> = Arc::default();
        let handle: Arc<std::sync::OnceLock<std::sync::Weak<Supervisor>>> = Arc::default();
        let sup = {
            let seen = Arc::clone(&seen);
            let handle = Arc::clone(&handle);
            Supervisor::new_with_seams(
                state.path(),
                dummy_exe(),
                SupervisorTimeouts::default(),
                SupervisorSeams {
                    create_crash: Some(Arc::new(move |stage| {
                        if stage == CreateStage::DuringLaunch
                            && let Some(sup) = handle.get().and_then(std::sync::Weak::upgrade)
                        {
                            let present = tokio::task::block_in_place(|| {
                                tokio::runtime::Handle::current().block_on(async {
                                    sup.sessions.lock().await.contains_key("stranded")
                                })
                            });
                            *seen.lock().expect("seen mutex") = Some(present);
                        }
                        Ok(())
                    })),
                    ..SupervisorSeams::default()
                },
            )
            .await
            .expect("supervisor")
        };
        handle.set(Arc::downgrade(&sup)).expect("set once");

        // A pending reservation whose attempt never launched: the shape a
        // crash after the claim leaves, and the one a retry relaunches.
        let fingerprint = create_fingerprint("/", "agent", None, None, None);
        sup.store
            .insert_session(
                StoredSession {
                    id: "stranded".to_string(),
                    title: "stranded".to_string(),
                    cwd: "/".to_string(),
                    invocation: "agent".to_string(),
                    tmux_name: "fh-stranded".to_string(),
                    pane: String::new(),
                    outcome: LastOutcome::Launching,
                },
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint: fingerprint.clone(),
                }),
            )
            .await
            .expect("seed");
        sup.sessions.lock().await.insert(
            "stranded".to_string(),
            Arc::new(entry_with(None, LastOutcome::Launching)),
        );

        let session = sup
            .create_session(
                "/",
                "agent",
                None,
                80,
                24,
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint,
                }),
            )
            .await
            .expect("the retry performs the create");
        assert_eq!(session.id, "stranded", "under the reserved identity");
        assert_eq!(
            *seen.lock().expect("seen mutex"),
            Some(false),
            "mid-relaunch, the session must not be resolvable — that is what makes a \
             concurrent stop or delete answer NotFound instead of racing the launch"
        );
        assert!(
            sup.sessions.lock().await.contains_key("stranded"),
            "and it must be back in the map once the launch completes"
        );
    }

    /// A pending reservation whose session has already ENDED replays it
    /// rather than relaunching under it.
    ///
    /// The row is `Exited` with no terminal, which is exactly the shape
    /// that tempts a naive "is it alive?" check into concluding nothing is
    /// there. It is not evidence of absence: the session ran and finished,
    /// the create that made it succeeded, and relaunching would start a
    /// second agent for an intent that already had its one. The same
    /// applies to an `Error` row (the agent never execed) — both are
    /// outcomes of a launch that happened.
    #[tokio::test]
    async fn a_pending_reservation_whose_session_ended_replays_it() {
        let state = tempfile::tempdir().expect("state dir");
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let fingerprint = create_fingerprint("/", "agent", None, None, None);
        sup.store
            .insert_session(
                StoredSession {
                    id: "ended".to_string(),
                    title: "ended".to_string(),
                    cwd: "/".to_string(),
                    invocation: "agent".to_string(),
                    tmux_name: "fh-ended".to_string(),
                    pane: String::new(),
                    outcome: LastOutcome::Launching,
                },
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint: fingerprint.clone(),
                }),
            )
            .await
            .expect("seed a pending reservation");
        sup.store
            .transition("ended", Transition::ObservedExit { exit_code: Some(1) })
            .await
            .expect("the session ran and finished");

        let replayed = sup
            .create_session(
                "/",
                "agent",
                None,
                80,
                24,
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint,
                }),
            )
            .await
            .expect("an ended session is still this intent's session");
        assert_eq!(replayed.id, "ended");
        assert_eq!(
            sup.store.load_all().await.expect("load").len(),
            1,
            "no second session may have been launched"
        );
        assert_eq!(
            sup.store
                .reservation("key")
                .await
                .expect("read")
                .expect("still there")
                .outcome,
            ReservationOutcome::Created,
            "and the replay settles the intent it just reconciled"
        );
    }

    /// An intent key that could never do its job is refused at the edge,
    /// before it can reach the durable, deliberately un-pruned reservation
    /// table (`INTENT_KEY_CAP`).
    ///
    /// The empty key is the one worth spelling out: accepted, it would
    /// collapse every create from a client that forgot to fill the field
    /// into a single intent — the second such create would replay the
    /// first's session instead of making its own.
    #[tokio::test]
    async fn a_degenerate_intent_key_is_rejected_before_anything_is_stored() {
        let state = tempfile::tempdir().expect("state dir");
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();

        // Every refusal shape, including the boundary either side of the
        // cap and a multibyte key whose CHARACTER count is comfortably
        // under it — the cap is bytes, because bytes are what the row
        // costs, and a char-counted cap would let a four-byte-per-char key
        // store four times the intended maximum.
        let over_by_one_char = "\u{1f600}".repeat(INTENT_KEY_CAP / 4 + 1);
        assert!(
            over_by_one_char.chars().count() < INTENT_KEY_CAP,
            "test fixture: this key is over the cap only when counted in bytes"
        );
        for (req_id, key) in [
            (1u64, String::new()),
            (2, "k".repeat(INTENT_KEY_CAP + 1)),
            (3, over_by_one_char),
        ] {
            handle_control(
                &sup,
                ControlMsg::CreateSession {
                    req_id,
                    cwd: "/".to_string(),
                    invocation: "agent".to_string(),
                    title: None,
                    cols: 80,
                    rows: 24,
                    intent_key: Some(key),
                    agent_kind: None,
                    resume_template: None,
                },
                &tx,
                &mut input_routes,
                &mut tasks,
            )
            .await;
            let frame = rx.try_recv().expect("a reply must have been sent");
            let decoded: ControlMsg = serde_json::from_slice(&frame.body).expect("decode");
            let ControlMsg::Error { kind, message, .. } = decoded else {
                panic!("a degenerate intent key must be refused: {decoded:?}");
            };
            assert_eq!(kind, ErrorKind::InvalidRequest);
            assert!(
                message.contains("intent key"),
                "the refusal must name what was wrong: {message}"
            );
        }
        assert!(
            sup.store.load_all().await.expect("load").is_empty(),
            "a refused request must not have reached the store at all"
        );
        assert!(
            sup.store
                .pending_reservations()
                .await
                .expect("reservations")
                .is_empty(),
            "and must not have left a reservation either — a key refused at the edge is not \
             spent, so a corrected retry with the same key must still be able to use it"
        );
        // A key EXACTLY at the cap is accepted, which is what makes the
        // refusals above a boundary rather than a vague limit. It fails on
        // the working directory instead, well past the key check.
        handle_control(
            &sup,
            ControlMsg::CreateSession {
                req_id: 4,
                cwd: "/nonexistent/definitely/not/here".to_string(),
                invocation: "agent".to_string(),
                title: None,
                cols: 80,
                rows: 24,
                intent_key: Some("k".repeat(INTENT_KEY_CAP)),
                agent_kind: None,
                resume_template: None,
            },
            &tx,
            &mut input_routes,
            &mut tasks,
        )
        .await;
        let frame = rx.try_recv().expect("a reply must have been sent");
        let decoded: ControlMsg = serde_json::from_slice(&frame.body).expect("decode");
        let ControlMsg::Error { message, .. } = decoded else {
            panic!("expected the cwd refusal: {decoded:?}");
        };
        assert!(
            message.contains("working directory"),
            "a key at exactly the cap must be accepted and the request judged on its merits: \
             {message}"
        );
    }

    /// The resume-template override is bounded on BOTH axes, before it can
    /// reach the never-pruned reservation row that stores a copy of it.
    ///
    /// Two independent limits because they fail independently: a template
    /// of a few enormous elements is caught by the shared byte cap (which
    /// it now counts against, alongside cwd/invocation/title), while a
    /// template of very many tiny ones costs almost no bytes and is caught
    /// by the element cap. Either shape unbounded is a permanent write
    /// sized by the request.
    #[tokio::test]
    async fn an_oversized_resume_template_is_refused_before_anything_is_stored() {
        let state = tempfile::tempdir().expect("state dir");
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();

        for (req_id, template, expected) in [
            (1u64, vec!["x".repeat(CREATE_FIELD_CAP)], "exceeding the"),
            (
                2,
                vec![String::new(); RESUME_TEMPLATE_ELEMENT_CAP + 1],
                "element limit",
            ),
        ] {
            handle_control(
                &sup,
                ControlMsg::CreateSession {
                    req_id,
                    cwd: "/".to_string(),
                    invocation: "agent".to_string(),
                    title: None,
                    cols: 80,
                    rows: 24,
                    intent_key: Some("key".to_string()),
                    agent_kind: None,
                    resume_template: Some(template),
                },
                &tx,
                &mut input_routes,
                &mut tasks,
            )
            .await;
            let frame = rx.try_recv().expect("a reply must have been sent");
            let decoded: ControlMsg = serde_json::from_slice(&frame.body).expect("decode");
            let ControlMsg::Error { kind, message, .. } = decoded else {
                panic!("an oversized resume template must be refused: {decoded:?}");
            };
            assert_eq!(kind, ErrorKind::InvalidRequest);
            assert!(
                message.contains(expected),
                "the refusal must name the limit that was exceeded: {message}"
            );
        }
        assert!(
            sup.store.load_all().await.expect("load").is_empty()
                && sup
                    .store
                    .pending_reservations()
                    .await
                    .expect("reservations")
                    .is_empty(),
            "neither refusal may have written anything"
        );
    }

    /// The opposite rollback rule, and the one that costs something to
    /// get wrong: when a create fails AMBIGUOUSLY, the launching row is
    /// KEPT.
    ///
    /// `tmux new-session` can fail after the session already exists (a
    /// lost reply, a timeout mid-command), so deleting the row on the
    /// strength of the error alone would orphan a running agent — no row,
    /// no id, nothing left that knows to reap it. Provoked here by
    /// removing the tmux binary from this supervisor's reach, which makes
    /// both the create AND the has-session probe that follows it fail;
    /// with the probe unable to confirm absence, the row must survive and
    /// the error must say so.
    #[tokio::test]
    async fn an_ambiguous_tmux_failure_keeps_the_launching_record() {
        let state = tempfile::tempdir().expect("state dir");
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let work = tempfile::tempdir().expect("workdir");
        // Kill the server and put a DIRECTORY where its socket belongs:
        // tmux can then neither connect to it nor create it, so
        // `new-session` fails AND the has-session probe that decides the
        // rollback fails too — an unconfirmable failure. Deliberately not
        // done by freezing the state directory, which would break SQLite
        // (its journal lives there) and fail the create before a row was
        // ever written, testing nothing.
        std::process::Command::new("tmux")
            .arg("-S")
            .arg(state.path().join("tmux.sock"))
            .arg("kill-server")
            .output()
            .expect("kill-server");
        std::fs::remove_file(state.path().join("tmux.sock")).ok();
        std::fs::create_dir(state.path().join("tmux.sock")).expect("block the socket path");

        let error = sup
            .create_session(
                &work.path().to_string_lossy(),
                "sh -c 'sleep 300'",
                None,
                80,
                24,
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint: create_fingerprint(
                        &work.path().to_string_lossy(),
                        "sh -c 'sleep 300'",
                        None,
                        None,
                        None,
                    ),
                }),
            )
            .await
            .expect_err("tmux cannot create a session without a socket");

        let rows = sup.store.load_all().await.expect("load");
        assert_eq!(
            rows.len(),
            1,
            "an unconfirmable tmux failure must keep the only record of a possibly-running \
             agent; error was: {error:#}"
        );
        assert_eq!(rows[0].outcome, LastOutcome::Launching);
        let rendered = format!("{error:#}");
        assert!(
            rendered.contains("kept as a launching record"),
            "the caller must be told the row survives and why: {rendered}"
        );
        // The idempotency half of the same rule (PLAN_M3.md item 6 meeting
        // the retention rules above): a failure that RETAINED evidence must
        // leave its reservation pending. Settling it `Failed` would tell
        // every later retry the intent is closed while an agent may still
        // be running under that retained row — hiding it until the next
        // restart, when the reload that would have reconciled it finds a
        // reservation that no longer wants reconciling.
        assert_eq!(
            sup.store
                .reservation("key")
                .await
                .expect("read")
                .expect("the claim committed with the launching row")
                .outcome,
            ReservationOutcome::Pending,
            "an ambiguous failure is exactly the reconcile-me state, not a recorded failure"
        );
    }

    /// Evidence that cannot be READ is never read as absence (PLAN_M3.md
    /// item 6, and the same no-guessing stance item 3 takes for a deferred
    /// sentinel).
    ///
    /// A sentinel whose read fails is the case where a wrong relaunch is
    /// most likely — something is already wrong with the state directory —
    /// and it is indistinguishable from "the shim recorded an exec failure
    /// for a launch that did happen". So the retry does neither: it
    /// reports, leaves the reservation pending, and expects a human or a
    /// later pass to clear whatever broke. Provoked with a DIRECTORY at the
    /// sentinel's path, which fails the read with `EISDIR` rather than
    /// merely being absent.
    #[tokio::test]
    async fn an_unreadable_sentinel_blocks_the_relaunch_rather_than_guessing() {
        let state = tempfile::tempdir().expect("state dir");
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let fingerprint = create_fingerprint("/", "agent", None, None, None);
        sup.store
            .insert_session(
                StoredSession {
                    id: "stranded".to_string(),
                    title: "stranded".to_string(),
                    cwd: "/".to_string(),
                    invocation: "agent".to_string(),
                    tmux_name: "fh-stranded".to_string(),
                    pane: String::new(),
                    outcome: LastOutcome::Launching,
                },
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint: fingerprint.clone(),
                }),
            )
            .await
            .expect("seed a pending reservation");
        let spec = crate::launch::spec_path_for_session(state.path(), "stranded");
        std::fs::create_dir(crate::launch::status_path_for_spec(&spec))
            .expect("plant an unreadable sentinel");

        let error = sup
            .create_session(
                "/",
                "agent",
                None,
                80,
                24,
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint,
                }),
            )
            .await
            .expect_err("unreadable evidence must not be resolved either way");
        let rendered = format!("{error:#}");
        assert!(
            rendered.contains("cannot tell whether"),
            "the caller must be told why it got no answer: {rendered}"
        );
        assert_eq!(
            sup.store
                .reservation("key")
                .await
                .expect("read")
                .expect("still there")
                .outcome,
            ReservationOutcome::Pending,
            "and the intent must stay reconcilable rather than being closed on a guess"
        );
        assert_eq!(
            sup.store.load_all().await.expect("load").len(),
            1,
            "no second session may have been launched"
        );
    }

    /// A relaunch whose leftover artifacts cannot be removed does not
    /// launch (PLAN_M3.md item 6 meeting item 5's launch-spec rules).
    ///
    /// The spec and sentinel paths are derived from the session id alone,
    /// so a relaunch reuses them exactly. Launching on top of a spec this
    /// process could not remove would leave the shim reading a file from a
    /// dead attempt — and a surviving sentinel would be read as evidence
    /// about a launch that has not happened yet. Failing closed leaves the
    /// reservation pending, which is recoverable; launching anyway is not.
    #[tokio::test]
    async fn a_relaunch_refuses_to_start_over_artifacts_it_cannot_remove() {
        let state = tempfile::tempdir().expect("state dir");
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let fingerprint = create_fingerprint("/", "agent", None, None, None);
        sup.store
            .insert_session(
                StoredSession {
                    id: "stranded".to_string(),
                    title: "stranded".to_string(),
                    cwd: "/".to_string(),
                    invocation: "agent".to_string(),
                    tmux_name: "fh-stranded".to_string(),
                    pane: String::new(),
                    outcome: LastOutcome::Launching,
                },
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint: fingerprint.clone(),
                }),
            )
            .await
            .expect("seed a pending reservation");
        // A leftover spec (but no sentinel — a sentinel would itself be
        // evidence the launch happened) in a directory that refuses
        // unlinking.
        let spec = crate::launch::spec_path_for_session(state.path(), "stranded");
        std::fs::write(&spec, b"{}").expect("plant a leftover spec");
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                state.path().join("launch"),
                std::fs::Permissions::from_mode(0o500),
            )
            .expect("freeze launch/");
        }

        let error = sup
            .create_session(
                "/",
                "agent",
                None,
                80,
                24,
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint,
                }),
            )
            .await
            .expect_err("a relaunch that cannot clear its own path must not start");

        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                state.path().join("launch"),
                std::fs::Permissions::from_mode(0o700),
            )
            .expect("restore launch/");
        }
        assert!(
            format!("{error:#}").contains("not relaunching"),
            "the caller must be told the relaunch was refused and why: {error:#}"
        );
        assert_eq!(
            sup.store
                .reservation("key")
                .await
                .expect("read")
                .expect("still there")
                .outcome,
            ReservationOutcome::Pending,
            "the intent stays reconcilable: nothing about it was resolved"
        );
        assert!(
            sup.tmux
                .pane_states()
                .await
                .expect("pane states")
                .is_empty(),
            "and nothing was launched"
        );
    }

    /// When the FAILURE cannot be recorded, the caller is told that —
    /// never handed the original error as though the key were spent.
    ///
    /// A client that sees "working directory does not exist" reasonably
    /// concludes retrying is pointless. That conclusion is only safe when
    /// the outcome is durable; if it is not, the same key may still do
    /// something entirely different, and the reply has to say so. Forced
    /// with a trigger that refuses the settlement, so the rollback
    /// transaction (which carries it) fails as a whole.
    #[tokio::test]
    async fn a_create_whose_outcome_cannot_be_recorded_reports_the_ambiguity() {
        let state = tempfile::tempdir().expect("state dir");
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        {
            use rusqlite::Connection;
            let conn = Connection::open(state.path().join("supervisor.db")).expect("open raw");
            conn.execute_batch(
                "CREATE TRIGGER refuse_settlement BEFORE UPDATE ON create_reservations \
                 BEGIN SELECT RAISE(ABORT, 'refused by test trigger'); END;",
            )
            .expect("plant the trigger");
        }
        // The spec write fails (a frozen launch directory), which is a
        // CONFIRMED-absence failure and therefore one that tries to settle.
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                state.path().join("launch"),
                std::fs::Permissions::from_mode(0o500),
            )
            .expect("freeze launch/");
        }

        let error = sup
            .create_session(
                "/",
                "agent",
                None,
                80,
                24,
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint: create_fingerprint("/", "agent", None, None, None),
                }),
            )
            .await
            .expect_err("the create fails either way");

        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                state.path().join("launch"),
                std::fs::Permissions::from_mode(0o700),
            )
            .expect("restore launch/");
        }
        let rendered = format!("{error:#}");
        assert_eq!(error_kind(&error), ErrorKind::Internal);
        assert!(
            rendered.contains("NOT spent"),
            "the caller must be told the outcome was not recorded: {rendered}"
        );
        assert!(
            rendered.contains("writing launch spec"),
            "and must still carry the original failure: {rendered}"
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
                    annotation: None,
                    restart_offer: RestartOffer::default(),
                },
                terminal: Some(Terminal {
                    tmux_name: "fh-fake".to_string(),
                    pane: "%0".to_string(),
                }),
                outcome: std::sync::Mutex::new(LastOutcome::Running),
            }),
        );

        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
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
                            annotation: None,
                            restart_offer: RestartOffer::default(),
                        },
                        terminal: None,
                        outcome: std::sync::Mutex::new(LastOutcome::Running),
                    }),
                );
            }
        }

        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
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
                    annotation: None,
                    restart_offer: RestartOffer::default(),
                },
                terminal: None,
                outcome: std::sync::Mutex::new(LastOutcome::Running),
            }),
        );

        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
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
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
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
            annotation: None,
            restart_offer: RestartOffer::default(),
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

    /// The lock-held detach notice must never block, and must never be
    /// dropped, even against a queue that is completely full.
    ///
    /// Both halves are load-bearing and they pull in opposite directions.
    /// Every teardown path calls this while holding the supervisor-wide
    /// `attachments` mutex, so an awaiting send would let one wedged peer
    /// freeze every session's attach, input, and delete — a
    /// supervisor-wide deadlock introduced by the very bound meant to
    /// prevent unbounded growth. But a notice that is simply discarded
    /// leaves a client with a terminal that silently stopped.
    ///
    /// The `let () =` binding is the mutation guard for the first half:
    /// it fails to COMPILE if `notify_detached` ever becomes async or
    /// otherwise starts returning a future.
    #[tokio::test]
    async fn notify_detached_never_blocks_on_a_full_queue_and_never_drops_the_notice() {
        let (tx, mut rx) = mpsc::channel::<Frame>(1);
        tx.send(Frame::data(1, b"occupying the only slot".to_vec()))
            .await
            .expect("channel is open");

        let () = notify_detached(&tx, 7, "stalled".to_string());

        // The queue was full, so the notice had to be deferred — but it
        // must arrive once capacity appears.
        let first = rx.recv().await.expect("the pre-existing frame");
        assert_eq!(first.kind, farhelm_proto::FrameKind::Data);
        let notice = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("the deferred detach notice never arrived")
            .expect("channel is still open");
        assert!(
            matches!(
                parse_control(&notice).expect("valid control frame"),
                ControlMsg::Detached { channel: 7, reason } if reason == "stalled"
            ),
            "the deferred notice must be this channel's detach, unchanged"
        );
    }
}
