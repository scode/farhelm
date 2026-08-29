//! The `Supervisor`: session bookkeeping and the create/restart/relaunch
//! lifecycle.
//!
//! This is the module every other `service` submodule ultimately answers
//! to — sweep, uploads, terminals, and the rest each own one slice of
//! *how* a session's state changes on disk or in tmux, but the map of
//! what sessions exist, their durable outcome, and their conversation-
//! capture identity lives here. See the crate-root `service` module doc
//! (`mod.rs`) for the state model and the shape of the split.
//!
//! Reading that state back out does NOT live here. `status` derives a
//! session's liveness, its restart offer, and the `SessionInfo` a reply
//! carries; its classification core takes what it needs as plain arguments
//! (an entry, a pane-state map), which is what keeps it out of this
//! module's private fields and testable with no supervisor at all.
//! `listing` owns the paged walk, and that one is NOT supervisor-free:
//! `list_page` takes a `&Supervisor` because walking a page means locking
//! the session map and probing tmux. What it does not do is reach into
//! private fields — it goes through the same API any other submodule
//! would.

#[cfg(test)]
use super::capture::overlapping_windows_reason;
use super::capture::{
    CaptureCoordination, CaptureGate, CaptureHistory, CaptureState, CaptureStoreFault, FirstInput,
};
use super::connection::{handle_connection, notify_detached};
use super::launch_artifacts::{
    best_effort_remove, cleanup_launch_artifacts, clear_launch_artifacts_fail_closed,
    read_launch_sentinel, remove_fail_closed, sentinel_could_still_apply, sweep_launch_dir,
    wrapper_failure_detail,
};
use super::snapshots::{
    MAX_ALT_SCREEN_SNAPSHOT_BYTES, capture_alt_screen_before_stop, snapshot_path,
    sweep_snapshot_temp_files,
};
use super::status::source_profile_existence;
use super::sweep::{
    StopFailure, SweepTarget, TabReapAnchor, launch_scope_unit, reap_process_tree, stop_live_agent,
};
use super::terminals::{
    ActiveAttach, AttachmentKey, OutputReapRegistry, SINK_READY_TIMEOUT, SinkRegistry,
    TAB_LAUNCH_SETTLE, TAB_LAUNCH_SETTLE_STEP, Terminal, TerminalId, agent_pane_from_states,
    resolve_terminal_for_close, tabs_from_pane_states,
};
use super::ticker::{
    ACTIVITY_STAMP_QUANTUM, ActivitySample, SAMPLING_ADMISSION_PERMITS, TICKER_INTERVAL,
    start_ticker,
};
use super::uploads::UploadHandle;
use crate::agent_kind::{CaptureWindowBounds, IntegrationSnapshot, RecordStamp};
use crate::launch::{LaunchSpec, resolve_shell, window_command};
use crate::store::DedupScope;
use crate::store::{
    Claimed, IntentClaim, LastOutcome, ProfileSnapshot, Reservation, ReservationOutcome,
    RetryClaim, SessionStore, Settlement, StoredSession, Transition, now_unix,
};
use crate::tmux::{AGENT_WINDOW_OPTION, PaneProbe, PaneState, TAB_WINDOW_OPTION, TmuxDriver};
use anyhow::Context;
use farhelm_proto::{
    AgentKind, ErrorKind, ProfileExistence, RestartMode, RestartOffer, SessionInfo, SessionStatus,
    SourceProfile, TabInfo,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::Duration;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

/// The longest a single attachment may stay paused before the supervisor
/// detaches it with [`farhelm_proto::DETACH_REASON_STALLED`].
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

/// How long an accepted upload may go without a byte arriving before the
/// supervisor gives up on it (PLAN_M4.md item 4's per-hop progress
/// timeout, which is SPEC.md's health-check requirement applied to the
/// paste path).
///
/// A PROGRESS bound, not a total-duration one — the opposite shape from
/// [`STALL_DETACH_TIMEOUT`], and for the opposite reason. There the
/// supervisor is deliberately blind between pause and resume, so only a
/// hard maximum is measurable; here every chunk is an observation, so a
/// transfer that is merely large keeps re-arming this window for as long
/// as it needs (which is what "no size cap" requires) while one that has
/// stopped moving is caught within one window.
///
/// The failure it exists to prevent is a forever-pending upload: a client
/// whose network died mid-transfer leaves a connection that never errors,
/// a temp file nothing removes, and a paste that never resolves. A minute
/// is generous next to any real stall and cheap next to that.
///
/// Injectable per supervisor (see [`SupervisorTimeouts`]) for the same
/// reason the detach timeouts are: a test cannot afford to wait out the
/// production value, and this repo's tests never mutate the process
/// environment.
pub const UPLOAD_PROGRESS_TIMEOUT: Duration = Duration::from_secs(60);

/// How long ONE blocking filesystem operation of an upload — a chunk
/// write, or the fsync-and-link that publishes — may take before the
/// transfer gives up on it.
///
/// A different bound from [`UPLOAD_PROGRESS_TIMEOUT`] because it covers a
/// different gap. The progress timeout runs BETWEEN commands and so cannot
/// see inside a blocking call at all: a write to a wedged filesystem (an
/// unresponsive NFS mount, a device that stopped answering) would sit
/// there indefinitely with the transfer neither progressing nor timing
/// out, and — because the publication holds the session's lifecycle claim
/// — would take that session's stop, restart, and delete down with it.
///
/// Generous enough that no working disk reaches it (a 256 KiB write and an
/// fsync are milliseconds), short enough that a session is not held
/// hostage for long. A blocking operation cannot actually be cancelled, so
/// what this bounds is how long the TRANSFER waits: past it the operation
/// is abandoned to finish (or not) on its own, and its staging file is
/// removed when it does — see `await_disk_stage`.
pub const UPLOAD_DISK_STAGE_TIMEOUT: Duration = Duration::from_secs(30);

/// How long the supervisor waits for the attached helm to answer one
/// agent request before telling the asking session it timed out — see
/// [`super::agent_relay`].
///
/// The bound exists at all because the asking side has none. `farhelm
/// agent` blocks on a socket read with no deadline of its own,
/// deliberately: the supervisor is the only party that can tell "no helm
/// is attached" from "a helm took it and is slow", and a client-side
/// deadline would collapse those into one unactionable "it did not
/// answer".
///
/// Thirty seconds is generous for what a helm actually does here — the
/// two read-only verbs answer from memory and helm.db, with no fan-out to
/// hosts — and the generosity is on purpose: an agent's request racing a
/// helm that is busy serving a browser should wait, not fail. What the
/// bound protects is the other direction, a helm that has stopped
/// answering without its connection dying, where the alternative is an
/// agent's shell command wedged forever.
///
/// Injectable per supervisor (see [`SupervisorTimeouts`]) for the reason
/// every other timeout here is: a test that asserts the timeout FIRES
/// cannot afford to wait out the production value, and this repo's tests
/// never mutate the process environment.
pub const AGENT_UPCALL_TIMEOUT: Duration = Duration::from_secs(30);

/// How long an agent request may wait for ROOM on the helm connection's
/// writer queue before the relay gives up on delivering it.
///
/// Separate from [`AGENT_UPCALL_TIMEOUT`] because the two bound different
/// failures and produce different answers. This one ends in
/// `ErrorKind::Unavailable`: the request was never sent, so retrying is
/// free. The other ends in `ErrorKind::Timeout`: the helm has it and may
/// still be working, so retrying might repeat it. One budget over both legs
/// reported the first case as the second, which is the exact distinction
/// protocol version 13 introduced.
///
/// Five seconds rather than thirty because parking here means the queue is
/// full — the connection is not draining, or is saturated with terminal
/// output — and no amount of further waiting makes an agent's listing more
/// likely to get out. Long enough to ride out an ordinary burst; short
/// enough that a wedged connection is reported as one.
pub const AGENT_DELIVER_TIMEOUT: Duration = Duration::from_secs(5);

/// The timeouts a `Supervisor` treats as "this consumer is gone, not
/// merely slow".
///
/// Grouped into one injectable value rather than a growing list of
/// constructor parameters: each is a property of the same judgement call
/// (how long to serve something that may be wedged), all default to
/// generous production values, and integration tests need to shorten
/// whichever one their scenario exercises without caring about the others.
/// Injected at construction rather than settable later because long-lived
/// tasks read them — an attachment forwarder, a connection writer, and an
/// upload transfer would otherwise have no single answer to "how long may
/// this take".
#[derive(Debug, Clone, Copy)]
pub struct SupervisorTimeouts {
    /// See [`STALL_DETACH_TIMEOUT`].
    pub stall_detach: Duration,
    /// See [`WRITER_STALL_TIMEOUT`].
    pub writer_stall: Duration,
    /// See [`UPLOAD_PROGRESS_TIMEOUT`].
    pub upload_progress: Duration,
    /// See [`UPLOAD_DISK_STAGE_TIMEOUT`].
    pub upload_disk_stage: Duration,
    /// The budget for one tmux control-mode exchange — see
    /// [`crate::tmux::CONTROL_EXCHANGE_TIMEOUT`], whose docs cover why
    /// PRODUCTION keeps this tight (it bounds how long a wedged tmux can
    /// hold the supervisor-wide attachments mutex).
    ///
    /// Grouped with the other "gone, not slow" timeouts above for the same
    /// reason they are: a test scenario that drives real tmux traffic on a
    /// loaded CI runner needs this loosened too, or a merely-busy tmux
    /// reads as wedged and the test fails for a reason that has nothing to
    /// do with what it is testing. Threaded into the [`TmuxDriver`] this
    /// supervisor constructs — see `new_with_seams` — because that driver,
    /// not the supervisor itself, is what actually reads it.
    pub tmux_exchange: Duration,
    /// The budget for the attach-time pane-listing step — see
    /// [`crate::tmux::PANE_LIST_TIMEOUT`]. Threaded the same way and for
    /// the same CI-load reason as `tmux_exchange`.
    pub tmux_pane_list: Duration,
    /// See [`SINK_READY_TIMEOUT`]: how long [`Supervisor::ensure_session_
    /// sink`] waits for a sink that is between incarnations before failing
    /// the attach it was called for.
    ///
    /// Grouped here for the same CI-load reason as `tmux_exchange` and
    /// `tmux_pane_list`, and coupled to them in practice: a sink's respawn
    /// attempt opens a fresh control-mode client through `tmux_exchange`'s
    /// own budget, so a test suite that widens `tmux_exchange` without
    /// also widening this one can end up with a sink-ready wait shorter
    /// than the respawn attempt it is meant to cover.
    pub sink_ready: Duration,
    /// See [`AGENT_UPCALL_TIMEOUT`]. The one entry here that bounds a wait
    /// on the PEER rather than on tmux or a socket write — the supervisor
    /// is the client for the duration of an agent request. Counts only the
    /// wait AFTER the request reached the connection; getting it there is
    /// [`Self::agent_deliver`]'s budget.
    pub agent_upcall: Duration,
    /// See [`AGENT_DELIVER_TIMEOUT`]: how long the relay waits for room on
    /// the helm connection's writer queue before reporting the request as
    /// undelivered.
    pub agent_deliver: Duration,
}

impl Default for SupervisorTimeouts {
    fn default() -> Self {
        SupervisorTimeouts {
            stall_detach: STALL_DETACH_TIMEOUT,
            writer_stall: WRITER_STALL_TIMEOUT,
            upload_progress: UPLOAD_PROGRESS_TIMEOUT,
            upload_disk_stage: UPLOAD_DISK_STAGE_TIMEOUT,
            tmux_exchange: crate::tmux::CONTROL_EXCHANGE_TIMEOUT,
            tmux_pane_list: crate::tmux::PANE_LIST_TIMEOUT,
            sink_ready: SINK_READY_TIMEOUT,
            agent_upcall: AGENT_UPCALL_TIMEOUT,
            agent_deliver: AGENT_DELIVER_TIMEOUT,
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

/// Which of a sampling pass's two tmux reads a [`SampleFault`] is being
/// asked about (PLAN_M6_75.md items 1 and 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleRead<'a> {
    /// The single batched liveness probe the whole pass is built on.
    /// Failing it means the pass learns nothing about ANY session.
    PaneStates,
    /// One selected session's own screen capture.
    Tail { session: &'a str },
}

/// A failure injected in place of one of a sampling pass's tmux reads,
/// returning the message the read should fail with.
///
/// A seam rather than a fault-injecting tmux wrapper, and it exists for a
/// property that is otherwise untestable: what a pass does to the retained
/// screens when it cannot read them. Both failure paths INVALIDATE
/// sharpening evidence without recording an observation
/// (`ticker::ActivitySample::forget_tail`), and neither can be reached from
/// a test by arranging real conditions — a pane must be alive in the very
/// probe that selected it for its capture to be attempted at all, so
/// "selected, then unreadable" is a race with no handle on it, and a failed
/// server-wide probe means breaking tmux underneath a running supervisor.
///
/// Testing the invalidation by calling `forget_tail` directly was the
/// alternative and is not equivalent: it stays green if production stops
/// calling it, which is precisely the regression. Production installs none,
/// so the cost is one `Option` check per read.
pub type SampleFault = Arc<dyn Fn(SampleRead<'_>) -> Option<String> + Send + Sync>;

/// A hook awaited after a missing sink is reserved but before opening starts.
///
/// The reservation race test needs to hold this exact boundary: another
/// ensure must already see the candidate barrier even though no tmux process
/// has begun opening. Production installs none.
pub type SinkReservationGate =
    Arc<dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync>;

/// A hook awaited immediately before the locked sink-registry decision.
///
/// Tests release two callers from this boundary together to prove the locked
/// lookup admits only one missing-sink reservation. Production installs none.
pub type SinkLookupGate = SinkReservationGate;

/// A hook awaited before a natural forwarder verdict claims its attachment.
///
/// The arbitration race test holds this boundary while an explicit takeover
/// removes the attachment. Production installs none.
pub type NaturalDetachGate = SinkReservationGate;

/// A hook awaited after a forwarder's output client is safely gone but before
/// that result is published to teardown waiters.
///
/// Restart and tab-close must still notify their viewer and recover their
/// session state when cleanup has moved behind a runtime-owned barrier. Tests
/// hold this boundary to make that normally tiny interval deterministic.
/// Production installs none.
pub type ForwarderCleanupGate = SinkReservationGate;

/// A hook awaited after a tab's window exists and is marked, but before the
/// dead-at-open-reply settle looks at its pane.
///
/// The refusal it guards is defined by a race the SUPERVISOR is supposed to
/// win — a shell that cannot start dies within the settle window, so the open
/// refuses — and a test cannot arrange the losing side of that race from
/// outside: the pane it needs to watch does not exist until `new-window`
/// returns, and between that and the settle (sizing, marking) there was no
/// boundary a test could hold — the settle began whenever the open reached
/// it, bounded only by [`TAB_LAUNCH_SETTLE`]. Under enough load the fixture shell's death has been
/// observed landing after the settle (2026-08-18, three concurrent e2e
/// binaries on a four-core box), turning the refusal test into a successful
/// open of a live tab. Holding this boundary lets a test wait for tmux itself
/// to report the pane dead before the settle runs, so "already dead by reply
/// time" is a fact rather than a bet.
///
/// Production installs none, so the cost is one `Option` check per open, and
/// the settle's own timing is untouched.
pub type TabSettleGate = SinkReservationGate;

/// A named boundary in archive teardown where tests may pause or fail the
/// operation before it publishes the archived row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveStage {
    PaneProbe,
    TabRediscovery,
    ScopeEnumeration,
    Sweep,
    ArtifactRemoval,
}

/// An asynchronous archive fault seam.
///
/// Archive correctness depends on failures and disconnects at boundaries
/// that real tmux, systemd, and filesystem calls cannot produce reliably.
/// Production installs no hook; tests use one to prove every such boundary
/// fails closed and that a connection disappearing cannot cancel teardown.
pub type ArchiveGate = Arc<
    dyn Fn(
            ArchiveStage,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send>>
        + Send
        + Sync,
>;

/// The injectable seams a `Supervisor` is built with. All default to
/// production behavior; grouped into one struct so a new injection point
/// does not grow the constructor's signature again.
#[derive(Clone)]
pub struct SupervisorSeams {
    /// See [`BootIdSource`]. Defaults to reading this host's real boot id.
    pub boot_id: BootIdSource,
    /// The tmux binary this supervisor drives — the resolved value of
    /// `--tmux` / `FARHELM_TMUX` (see
    /// [`crate::tmux::resolve_tmux_program`]), defaulting to plain `tmux`
    /// off `PATH`.
    ///
    /// A production knob rather than a test fault seam, and it lives here
    /// anyway for the reason this struct exists: the alternative was a
    /// fifth positional parameter on `new_with_seams` that every one of
    /// the crate's several dozen test constructions would have to spell
    /// out as "no opinion". Resolution deliberately happens ONCE, at
    /// `farhelm supervisor run`'s startup, so nothing below this line ever
    /// consults the environment again and no two launch paths can disagree
    /// about which tmux this supervisor is driving.
    pub tmux_program: PathBuf,
    /// Which agent kinds get the per-launch conversation hook appended to
    /// their argv — the `FARHELM_AGENT_HOOKS` opt-out (plan D5). See
    /// [`crate::agent_kind::AgentHooks`], and
    /// [`Supervisor::with_hook_argv`] for the one place it is consulted.
    ///
    /// A production knob for the same reason `tmux_program` above is one,
    /// and it obeys the same rule this struct's `Default` states: nothing
    /// below this line ever consults the environment. The single
    /// `std::env::var("FARHELM_AGENT_HOOKS")` read happens in `farhelm
    /// supervisor run`'s CLI arm, beside the `--tmux` resolution, and the
    /// PARSED value arrives here — so a test sets the policy by setting
    /// this field and never by mutating its own process's environment,
    /// which this repo forbids.
    ///
    /// [`crate::agent_kind::AgentHooks::All`] by default, unconditionally:
    /// a supervisor constructed by anything other than the CLI arm hooks
    /// every integrated kind, because the opt-out is an operator override
    /// and not a value with a discoverable "real" setting.
    pub agent_hooks: crate::agent_kind::AgentHooks,
    /// See [`CreateCrashSeam`]. `None` in production.
    pub create_crash: Option<CreateCrashSeam>,
    /// Where the agents' own record directories are rooted (PLAN_M3.md
    /// item 8): `~/.claude/projects/...`, `~/.codex/sessions/...`.
    ///
    /// `None` means "resolve `$HOME` at construction", which is what
    /// production does. Injected as a seam rather than read from the
    /// environment at every scan for two reasons: the capture fixtures
    /// need a private tree per test, and this repo's tests never mutate
    /// the test process's environment — a per-process `HOME` override
    /// would additionally be shared by every concurrently-running harness.
    /// A supervisor that resolves to nothing at all (no `HOME`, no
    /// override) simply performs no capture; see
    /// [`Supervisor::agent_home`].
    pub agent_home: Option<PathBuf>,
    /// The home directory a create's `~`-prefixed working directory expands
    /// against (see [`expand_tilde_cwd`]).
    ///
    /// `None` means "resolve `$HOME` at construction", as `agent_home`
    /// does, and it is a SEPARATE seam from `agent_home` on purpose: that
    /// one names where agent record trees hang for conversation capture,
    /// and tests point it at private fixture trees that would be nonsense
    /// as a user's home. Injected for the same environment-hygiene reason —
    /// this repo's tests never mutate the test process's environment. A
    /// supervisor that resolves to nothing refuses `~` creates with a
    /// clear error rather than guessing.
    pub user_home: Option<PathBuf>,
    /// How wide a session's capture window is around its first-input time
    /// (see [`CaptureWindowBounds`], and `crate::agent_kind`'s constants
    /// for the trade the production values make). Shortened by tests so
    /// proving two sessions in one directory do NOT overlap does not mean
    /// waiting out a production minute.
    pub capture_window: CaptureWindowBounds,
    /// See [`CaptureStoreFault`]. `None` in production.
    pub capture_store_fault: Option<CaptureStoreFault>,
    /// See `super::capture::CaptureGate`. `None` in production.
    pub capture_gate: Option<CaptureGate>,
    /// See [`SinkReservationGate`]. `None` in production.
    pub sink_reservation_gate: Option<SinkReservationGate>,
    /// See [`SinkLookupGate`]. `None` in production.
    pub sink_lookup_gate: Option<SinkLookupGate>,
    /// Test signal emitted after locked lookup observes a candidate barrier.
    pub sink_candidate_wait_gate: Option<SinkLookupGate>,
    /// See [`NaturalDetachGate`]. `None` in production.
    pub natural_detach_gate: Option<NaturalDetachGate>,
    /// See [`ForwarderCleanupGate`]. `None` in production.
    pub forwarder_cleanup_gate: Option<ForwarderCleanupGate>,
    /// See [`ArchiveGate`]. `None` in production.
    pub archive_gate: Option<ArchiveGate>,
    /// See [`SampleFault`]. `None` in production.
    pub sample_fault: Option<SampleFault>,
    /// How often the supervisor's own periodic task fires — see
    /// [`crate::service::ticker`] for what rides that cadence and
    /// [`TICKER_INTERVAL`] for why production picks the value it does.
    ///
    /// A seam rather than a [`SupervisorTimeouts`] entry because it is not
    /// a "this consumer is gone, not merely slow" budget at all: nothing
    /// fails when a tick is late, and shortening it does not loosen a
    /// deadline, it makes the supervisor do MORE work per second. Tests
    /// shorten it to milliseconds so proving "capture advances with nobody
    /// polling" costs a moment rather than several production intervals.
    pub ticker_interval: Duration,
    /// How much newer an observed pane change must be before it moves a
    /// session's `last_activity_at` — see
    /// [`crate::service::ticker::ACTIVITY_STAMP_QUANTUM`] for why
    /// production picks a whole minute.
    ///
    /// A seam for one reason: without it, the only way to observe an
    /// advance is to wait out a production minute of real time, and this
    /// repo does not put minute-long sleeps in its test suite. Tests set
    /// it to zero, which makes every observed change advance the value —
    /// the behavior the quantum exists to suppress, which is exactly what
    /// makes it the right setting for proving the value moves at all.
    ///
    /// Whole seconds only, matching the resolution of the timestamps it
    /// gates; a sub-second value truncates to zero. Zero does NOT mean
    /// sub-second resolution: the stamp itself is unix SECONDS, so two
    /// changes observed within one second still land on the same value,
    /// and a test that wants to see the value move must start from a seed
    /// at least a second in the past rather than from "now".
    pub activity_quantum: Duration,
    /// Extra environment entries every launch of every session in this
    /// supervisor starts with, injected into tmux (`-e`) so they reach the
    /// login shell before it sources anything.
    ///
    /// Empty in production, and deliberately not a user-facing feature:
    /// SPEC.md's environment contract says a session behaves as if the user
    /// had SSHed in and typed the command, which is what the launch chain's
    /// `-l -i` shell already delivers from the supervisor's own
    /// environment. This exists because that contract's other half — "the
    /// environment is evaluated at each launch: edit your rc files and the
    /// next launch or restart sees the change" — is otherwise untestable in
    /// this repo: proving it needs an rc file the test controls, which
    /// means a `HOME` the test controls, and mutating the test process's
    /// environment is forbidden here (and would be shared by every
    /// concurrently-running harness besides). Dependency injection is the
    /// sanctioned alternative, and this is it — the same reasoning as
    /// [`SupervisorSeams::agent_home`], one layer lower.
    ///
    /// Applied identically to a create and to a restart's relaunch, since
    /// a difference between the two would be exactly the divergence the
    /// contract forbids.
    pub launch_env: Vec<(String, String)>,
    /// This supervisor's access to a systemd user manager (PLAN_M3.md item
    /// 10). Defaults to the real one, which reports itself unavailable on
    /// every host that has none — CI included.
    ///
    /// A seam because the two paths must BOTH be provable on one host:
    /// `ScopeManager::disabled()` pins the fallback (the M2 behavior CI
    /// proves by having no manager at all) on a developer machine that does
    /// have one, and `ScopeManager::fake` makes the ordering of the scope
    /// kill and the backstop sweep observable, which nothing about the end
    /// state can show — both mechanisms leave the same corpse.
    pub scopes: Arc<crate::scope::ScopeManager>,
    /// The shell every launch of this supervisor runs through, overriding
    /// [`crate::launch::resolve_shell`]'s `$SHELL`/passwd chain.
    ///
    /// `None` in production, which is the real chain. It exists because a
    /// TERMINAL TAB has no invocation of its own (PLAN_M4.md item 2: the
    /// window command IS the shell), so the only way to drive a tab launch
    /// into a chosen state — a shell that exits immediately, which is what
    /// makes the dead-at-open-reply refusal testable — is to choose the
    /// shell. The agent path has never needed this because its argv is the
    /// caller's, and a test can simply pass a command that fails.
    ///
    /// Dependency injection rather than `$SHELL`, for this repo's standing
    /// reason (tests never mutate the test process's environment) and one
    /// more specific to it: `$SHELL` is process-wide, and this file's
    /// harnesses run concurrently, so a per-process override would leak
    /// between unrelated supervisors.
    ///
    /// Applied to the AGENT launch as well as to tabs, deliberately: the
    /// two share one shell-resolution contract, and a seam that covered
    /// only one of them would be a second place for that contract to live.
    pub launch_shell: Option<String>,
    /// See [`TabOpenFault`]. `None` in production.
    pub tab_open_fault: Option<TabOpenFault>,
    /// See [`TabSettleGate`]. `None` in production.
    pub tab_settle_gate: Option<TabSettleGate>,
    /// The filesystem an attachment upload stages through
    /// (`crate::files::FaultSeam`). [`crate::files::RealFs`] in
    /// production, which is the real syscalls.
    ///
    /// A seam because an upload's storage failures are otherwise
    /// unreachable from a test at this level: a write that fails
    /// mid-stream (a full disk, a vanished mount) must abort the transfer
    /// with a visible reason and leave nothing published, and engineering
    /// a genuine ENOSPC around a temp directory is neither portable nor
    /// deterministic. Injected as `Arc<dyn ... + Send + Sync>` rather than
    /// the plain `&dyn FaultSeam` the whole-file tiers pass, because the
    /// staged writes happen inside `spawn_blocking` and so cross a thread
    /// boundary (see `FaultSeam`'s own note on why the TRAIT itself needs
    /// no such bound).
    pub upload_fs: Arc<dyn crate::files::FaultSeam + Send + Sync>,
}

/// Where an [`TabOpenFault`] can fail a tab open.
///
/// One variant, because one stage has an unwind worth proving: the window
/// exists and its shell is already running, but the tab has no identity
/// yet. Nothing else can reach that state on demand — the tmux call that
/// creates the window either works or leaves nothing behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabOpenStage {
    /// Immediately after `new-window`, before the window is marked.
    BeforeMarking,
}

/// A failure injected into an `OpenTab` at one of its stages.
///
/// A seam rather than a fault-injecting tmux wrapper because what needs
/// exercising is the SUPERVISOR's unwind: a window that exists but was
/// never marked is invisible to rediscovery, so leaving it would strand a
/// live shell nothing can list or close — and the only way to reach that
/// state deliberately is to fail the marking step. Production installs
/// none, so the call site is one `Option` check.
pub type TabOpenFault = Arc<dyn Fn(TabOpenStage) -> anyhow::Result<()> + Send + Sync>;

impl Default for SupervisorSeams {
    fn default() -> Self {
        SupervisorSeams {
            boot_id: Arc::new(read_host_boot_id),
            tmux_program: PathBuf::from(crate::tmux::DEFAULT_TMUX_PROGRAM),
            agent_hooks: crate::agent_kind::AgentHooks::default(),
            create_crash: None,
            agent_home: None,
            user_home: None,
            capture_window: CaptureWindowBounds::default(),
            capture_store_fault: None,
            capture_gate: None,
            sink_reservation_gate: None,
            sink_lookup_gate: None,
            sink_candidate_wait_gate: None,
            natural_detach_gate: None,
            forwarder_cleanup_gate: None,
            archive_gate: None,
            sample_fault: None,
            ticker_interval: TICKER_INTERVAL,
            activity_quantum: ACTIVITY_STAMP_QUANTUM,
            launch_env: Vec::new(),
            scopes: Arc::new(crate::scope::ScopeManager::systemd()),
            launch_shell: None,
            tab_open_fault: None,
            tab_settle_gate: None,
            upload_fs: Arc::new(crate::files::RealFs),
        }
    }
}

/// This host's current boot id (PLAN_M3.md item 2).
///
/// Linux publishes a per-boot UUID at `/proc/sys/kernel/random/boot_id`.
/// A host that does not have the file at all is reported as `Ok(None)` —
/// unsupported, honestly — while any OTHER read failure is an `Err`: see
/// [`BootIdSource`] for why the two must not be collapsed.
#[cfg(not(target_os = "macos"))]
fn read_host_boot_id() -> anyhow::Result<Option<String>> {
    read_boot_id_from(Path::new("/proc/sys/kernel/random/boot_id"))
}

/// This host's current boot id (PLAN_M3.md item 2): macOS's
/// `kern.bootsessionuuid`, a kernel-minted per-boot UUID and the exact
/// analogue of Linux's `/proc/sys/kernel/random/boot_id`.
///
/// PLAN_M3.md's deferral note named `kern.boottime` as the presumed Mac
/// equivalent, and this deliberately is NOT that. `boottime` is derived
/// (realtime minus uptime) and the kernel recomputes it when NTP steps the
/// clock, so it can change MID-BOOT — and a boot id that changes mid-boot
/// would make the next supervisor restart mass-convert every live session
/// to `Interrupted` over a clock adjustment. `bootsessionuuid` is minted
/// once per boot and never rewritten, which is the entire property this
/// classifier rests on.
///
/// The same three-way contract as the Linux reader: a kernel that does not
/// know the name at all (`ENOENT`) is `Ok(None)` — unsupported, honestly —
/// while any other failure is an `Err`, because "has an id and reading it
/// failed this time" must degrade the reload rather than silently take the
/// same-boot path (see [`BootIdSource`]).
#[cfg(target_os = "macos")]
fn read_host_boot_id() -> anyhow::Result<Option<String>> {
    use anyhow::Context as _;
    let name = c"kern.bootsessionuuid";
    // A UUID string is 36 bytes plus a trailing NUL; the headroom costs
    // nothing and keeps a future longer form from turning into ENOMEM.
    let mut buf = [0u8; 128];
    let mut len = buf.len();
    // SAFETY: `name` is a NUL-terminated C string; `buf` owns `len` bytes
    // and `len` says exactly that, so sysctlbyname writes only within the
    // buffer and reports the bytes actually written back through `len`.
    let rc = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            buf.as_mut_ptr().cast::<std::ffi::c_void>(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::ENOENT) {
            return Ok(None);
        }
        return Err(anyhow::Error::new(err).context("reading kern.bootsessionuuid"));
    }
    let text =
        std::str::from_utf8(&buf[..len]).context("kern.bootsessionuuid is not valid UTF-8")?;
    // String sysctls report their trailing NUL in the length; an id that
    // trims to nothing is unusable and treated as unsupported rather than
    // stored, for `read_boot_id_from`'s reason: recording "" would make a
    // later real id look like a reboot on no evidence.
    let trimmed = text.trim_end_matches('\0').trim();
    Ok((!trimmed.is_empty()).then(|| trimmed.to_string()))
}

/// [`read_host_boot_id`]'s logic, parameterized on the path it reads —
/// split out purely so a unit test can point it at a tempdir-backed file
/// instead of the real `/proc` entry, per this project's rule against
/// mutating the test process's own environment. Not itself exposed as a
/// [`BootIdSource`]: production always wants the fixed `/proc` path, so
/// only the zero-argument wrapper is wired into [`Seams::default`].
#[cfg(any(not(target_os = "macos"), test))]
fn read_boot_id_from(path: &Path) -> anyhow::Result<Option<String>> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(anyhow::Error::new(e).context(format!("reading {}", path.display()))),
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
    /// The `bool` alongside the claim is true only when THIS call took
    /// the OS lock — as opposed to reusing a claim some other
    /// `Supervisor` in this process already holds. The distinction
    /// matters to exactly one caller: the startup reap of stale tmux
    /// control clients, whose "everything attached is a dead
    /// predecessor's" premise is true when the lock was just acquired
    /// (nothing in this process OR any other owned the directory a
    /// moment ago) and FALSE for a reused claim (the incumbent holding
    /// it has live, healthy clients this constructor must not touch).
    ///
    /// One benign race is left deliberately: a claim being DROPPED
    /// concurrently with this call can leave the registry entry already
    /// dead while its lock file has not finished closing, so the
    /// `try_lock` below fails and this caller starts read-only as though
    /// another process held the directory. It resolves itself on the next
    /// construction, and it fails in the safe direction (no migration, no
    /// reconciliation) — the only direction worth being sure of.
    fn claim(state_dir: &Path) -> anyhow::Result<Option<(Arc<StateDirOwnership>, bool)>> {
        // Canonicalized so two spellings of the same directory cannot
        // yield two claims; the directory exists by now (`ensure_private_dir`).
        let path = std::fs::canonicalize(state_dir)
            .with_context(|| format!("resolving state dir {}", state_dir.display()))?;
        let mut claims = STATE_DIR_CLAIMS.lock().expect("claims mutex poisoned");
        if let Some(existing) = claims.get(&path).and_then(std::sync::Weak::upgrade) {
            return Ok(Some((existing, false)));
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
        Ok(Some((owned, true)))
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
pub(crate) struct RequestError {
    pub(crate) kind: ErrorKind,
    pub(crate) message: String,
}

impl RequestError {
    pub(crate) fn new(kind: ErrorKind, message: impl Into<String>) -> RequestError {
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
pub(crate) fn error_kind(e: &anyhow::Error) -> ErrorKind {
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
/// explicit title that happens to equal the derived one), item 7's
/// `agent_kind`/`resume_template` overrides, the chosen launch selector and
/// its value, and — as of PLAN_M7.md item 2 — `parent`.
/// `cols`/`rows` are excluded by
/// design: they shape the ATTACHMENT, not the session, so the same intent
/// retried from a differently-sized client is still the same intent — a
/// point the plan makes explicitly, and the reason this function takes no
/// dimensions at all rather than taking and ignoring them.
///
/// ## The launch selector is encoded explicitly
///
/// [`CreateMode`] exists only after `handlers::create_mode` has proved that
/// exactly one of `invocation`, `profile_id`, or `profile_name` was present.
/// Its variant therefore records the selector as well as the value. A retry
/// under the same key that changes either produces a different fingerprint
/// and is refused rather than launching something else.
///
/// A retry MUST NOT be able to flip modes, and that is worth stating as a
/// safety property rather than a consistency one: the selectors can resolve
/// to entirely different agents in the same directory, and a create is not
/// undoable.
///
/// ## The RAW encoding is frozen, byte for byte
///
/// Existing interactive reservations are PERMANENT tombstones (see
/// `store::DedupScope`), and every replay compares the stored string
/// verbatim. So their fingerprints are not caches that age out: an
/// encoding change re-reads every key a supervisor has ever seen as
/// belonging to a DIFFERENT request, and the identical retry that should
/// have replayed is refused as a key reuse (`Conflict`) instead. Forever,
/// for that key, with no way for the client to recover except minting a new
/// one — which is exactly the confusion an idempotency key exists to
/// prevent.
///
/// That is why version 10 does NOT append its mode to the existing tuple.
/// The raw mode's five elements are the encoding pre-M6.75 supervisors
/// wrote, unchanged and unreordered, so a raw retry across the upgrade
/// still matches its own tombstone and replays. `invocation` is `Option`
/// now, but `Some("x")` and `"x"` serialize to the same JSON, so the bytes
/// are identical rather than merely equivalent.
///
/// The PROFILE-ID mode gets its own, structurally distinct encoding instead: a
/// leading `"profile"` discriminant and a shorter tuple. Distinctness is
/// what makes the mode unflippable, and it is stronger here than an
/// appended element would have been — the two encodings differ in LENGTH as
/// well as in content, so no raw request can collide with a profile one
/// whatever any field happens to contain (a `cwd` literally named
/// `profile`, say). The raw tuple carries no discriminant of its own
/// precisely because it cannot: adding one would change the frozen bytes.
///
/// Version 11's parented and profile-name creates get new discriminated
/// tuples rather than extending either frozen encoding. That preserves
/// every existing row byte for byte while making parent, selector, and
/// selector value independently collision-proof.
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
/// The `agent_kind` element is spelled with the STORE's stable column
/// vocabulary (`store::agent_kind_column`) rather than the wire type's
/// serde representation: the two agree today, but a future protocol rename
/// would otherwise change every stored fingerprint at once and turn
/// identical requests into key-reuse conflicts across an upgrade. Sharing
/// the store's spelling rather than defining a second one is deliberate —
/// item 7 writes the same kind into the session row, and two vocabularies
/// that drifted apart would produce exactly that upgrade-time conflict
/// from the inside. The persisted encoding is pinned by a golden test for
/// the same reason `LastOutcome`'s column vocabulary is.
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
pub(crate) fn create_fingerprint(
    parent: Option<&str>,
    cwd: &str,
    mode: &CreateMode,
    title: Option<&str>,
) -> String {
    // Infallible in practice: every element is a string, an option, or an
    // array of strings, none of which can fail to serialize. The `expect`
    // documents that rather than inviting a caller to handle an error that
    // cannot occur.
    //
    // The raw-only fields cannot accompany a profile selection because
    // [`CreateMode`] cannot represent that at all — an earlier shape passed
    // both halves as separate `Option`s and needed a `debug_assert` to say
    // so, which is a comment with a runtime cost rather than a guarantee.
    match (parent, mode) {
        // FROZEN — see this function's own docs. Five elements, in this
        // order, exactly as every pre-M6.75 supervisor wrote them.
        (
            None,
            CreateMode::Raw {
                invocation,
                agent_kind,
                resume_template,
            },
        ) => serde_json::to_string(&(
            cwd,
            Some(invocation.as_str()),
            title,
            agent_kind.map(crate::store::agent_kind_column),
            resume_template.as_deref(),
        )),
        // The profile mode's own encoding, discriminated and shorter, so it
        // cannot collide with the frozen tuple above under any input.
        (None, CreateMode::Profile { profile_id }) => {
            serde_json::to_string(&("profile", cwd, title, profile_id))
        }
        // A parent cannot be appended to either frozen encoding: doing so
        // would change every pre-v11 fingerprint. New discriminants give
        // parented creates their own collision-proof shapes instead.
        (
            Some(parent),
            CreateMode::Raw {
                invocation,
                agent_kind,
                resume_template,
            },
        ) => serde_json::to_string(&(
            "parented_raw",
            parent,
            cwd,
            invocation,
            title,
            agent_kind.map(crate::store::agent_kind_column),
            resume_template.as_deref(),
        )),
        (Some(parent), CreateMode::Profile { profile_id }) => {
            serde_json::to_string(&("parented_profile", parent, cwd, title, profile_id))
        }
        // Profile names have no pre-v11 encoding to preserve. Keeping the
        // selector name in the tuple prevents a name equal to a profile id
        // from colliding with the id-backed mode.
        (parent, CreateMode::ProfileName { profile_name }) => {
            serde_json::to_string(&("profile_name", parent, cwd, title, profile_name))
        }
        (parent, CreateMode::DerivedProfile) => {
            serde_json::to_string(&("derived_profile", parent, cwd, title))
        }
    }
    .expect("a fingerprint of strings and options always serializes")
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
pub(crate) struct KeyedLocks {
    /// `Weak` so an entry dies with its last guard; see [`KeyedGuard`]'s
    /// `Drop`. A std mutex, not a tokio one: it is held only for the map
    /// lookup itself, never across the `await` that acquires the per-key
    /// lock.
    locks: std::sync::Mutex<HashMap<String, std::sync::Weak<tokio::sync::Mutex<()>>>>,
}

impl KeyedLocks {
    /// Hold this key's lock until the returned guard is dropped, waiting
    /// out any holder already running under the same key.
    pub(crate) async fn claim(self: &Arc<Self>, key: &str) -> KeyedGuard {
        let lock = {
            let mut locks = self.locks.lock().expect("keyed lock map poisoned");
            match locks.get(key).and_then(std::sync::Weak::upgrade) {
                Some(existing) => existing,
                None => {
                    let fresh = Arc::new(tokio::sync::Mutex::new(()));
                    locks.insert(key.to_string(), Arc::downgrade(&fresh));
                    fresh
                }
            }
        };
        KeyedGuard {
            registry: Arc::clone(self),
            key: key.to_string(),
            _held: lock.lock_owned().await,
        }
    }

    /// Report whether a key is held at this instant for deterministic
    /// lifecycle-race tests.
    ///
    /// This is observation only: production code must acquire the claim and
    /// let serialization decide when work may proceed. Tests use the probe
    /// while another operation is parked at an injected teardown boundary,
    /// where the answer cannot race the holder's normal completion.
    #[cfg(test)]
    pub(crate) fn claimed_for_test(&self, key: &str) -> bool {
        let lock = self
            .locks
            .lock()
            .expect("keyed lock map poisoned")
            .get(key)
            .and_then(std::sync::Weak::upgrade);
        lock.is_some_and(|lock| lock.try_lock().is_err())
    }
}

/// One holder's exclusive claim on a key; see [`KeyedLocks`].
pub(crate) struct KeyedGuard {
    registry: Arc<KeyedLocks>,
    key: String,
    /// Owned rather than borrowed so the guard is `'static` and can be
    /// held across every await in a create.
    _held: tokio::sync::OwnedMutexGuard<()>,
}

impl Drop for KeyedGuard {
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
///
/// Both variants are boxed, which is the only shape that stays balanced:
/// `SessionInfo` is a wire record that keeps growing (`source_profile` at
/// `PROTOCOL_VERSION` 10 was the addition that tipped it), so an inline
/// `Answer` makes every `Resolution` as large as the biggest reply this
/// protocol has ever carried. One allocation on a path that is already
/// doing durable writes and process launches is not a cost worth
/// measuring; a struct that silently widens with the protocol is.
enum Resolution {
    /// This intent already has an answer — the session it created, the
    /// gone-error, the original failure, a key-reuse refusal, or an
    /// honest "cannot tell". Whatever it is, it is what the caller
    /// returns, unchanged.
    Answer(Box<anyhow::Result<SessionInfo>>),
    /// Nothing was ever launched under this reservation, so the caller
    /// performs the create under it — same key, same identities.
    Relaunch(Box<Reservation>),
    /// A bounded reservation outlived its child and was pruned. The caller
    /// proceeds exactly as if the key had never been claimed.
    Fresh,
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
struct LaunchRequest {
    /// Direct organizational parentage, persisted verbatim with the child.
    parent: Option<String>,
    /// The working directory as the caller spelled it — after `~`
    /// expansion, which is the one rewrite the caller's spelling gets
    /// ([`expand_tilde_cwd`]): what the session records and what every
    /// error names, symlinks and all. Deliberately not the resolved
    /// spelling beyond that: see `store::StoredSession::canonical_cwd`
    /// for why the user-facing path must stay the user's. Owned because a
    /// retry rebuilds it from the stored row rather than borrowing the
    /// request's literal string.
    cwd: String,
    /// The directory this launch actually hands to tmux, which is NOT the
    /// same question as what it records.
    ///
    /// For a create the two are identical: there is no prior identity to
    /// check the path against, and canonicalization is best-effort there
    /// (a failure costs capture correlation, never the create), so
    /// substituting a resolved path would only add a way for the launch to
    /// disagree with the request.
    ///
    /// For a RETRY it is the canonical path [`ensure_cwd_identity`] just
    /// verified, and that is the whole point of the field. Checking that
    /// `cwd` still resolves to the session's stored identity and then
    /// handing tmux the ORIGINAL symlink is a time-of-check/time-of-use
    /// gap: the link can be repointed between the two, and the launch lands
    /// in a directory nobody validated — often with a permissively
    /// configured agent. Launching into the resolved path closes it, since
    /// a path with no symlinks left in it cannot be repointed out from
    /// under the launch.
    launch_cwd: String,
    /// The command line this session will run and record, whoever supplied
    /// it: the caller's own, or the one the resolved profile carried.
    /// OWNED rather than borrowed from `CreateInputs` for exactly that
    /// reason — a profile-backed create's invocation belongs to a catalog
    /// row read during validation, which outlives nothing.
    invocation: String,
    argv: Vec<String>,
    title: String,
    cols: u16,
    rows: u16,
    /// The resolved integration snapshot (PLAN_M3.md item 7). Part of the
    /// VALIDATED bundle rather than something the launch derives, because
    /// resolving it is itself one of the checks — an integrated kind with
    /// a placeholder-free resume template is refused right here, before
    /// any side effect exists.
    snapshot: IntegrationSnapshot,
    /// `cwd` with symlinks, `.`/`..`, and a trailing slash resolved away
    /// — the spelling every correlation uses (PLAN_M3.md item 8; see
    /// `store::StoredSession::canonical_cwd` for why the user-facing one
    /// cannot be). Resolved during validation because that is where the
    /// directory is already being stat'ed, and stored immutably with the
    /// session because re-resolving later could follow a symlink that has
    /// since been repointed.
    canonical_cwd: String,
    /// The profile this create resolved, as the session will remember it
    /// forever (PLAN_M6_75.md item 4), or `None` for a raw create.
    ///
    /// Resolved during validation beside the integration snapshot, and for
    /// the same reason: the resolution can FAIL (the unknown-profile
    /// precondition), and every refusal from validation is one a keyed
    /// create records and replays verbatim.
    source_profile: Option<ProfileSnapshot>,
}

/// The launch inputs retained after a create's wire shape is fingerprinted.
///
/// A bundle rather than a parameter list because the idempotency state
/// machine threads these through three functions unchanged. `parent`
/// travels with the durable session after being bound into the fingerprint.
/// Terminal dimensions ride along despite shaping the attachment rather
/// than the session (and are deliberately absent from the fingerprint),
/// since the launch does need them.
pub(crate) struct CreateInputs<'a> {
    pub(crate) cwd: &'a str,
    pub(crate) parent: Option<String>,
    /// Which launch selector this request chose, already resolved to one.
    pub(crate) mode: CreateMode,
    pub(crate) title: Option<String>,
    pub(crate) cols: u16,
    pub(crate) rows: u16,
}

/// Which launch selector a `CreateSession` chose, once it has been shown
/// to select exactly one (PLAN_M7.md item 2).
///
/// A resolved value rather than a pair of `Option`s, so that everything
/// downstream of `handlers::create_mode` is working from a request whose
/// meaning is settled: there is deliberately no way to construct a
/// `CreateMode` that names multiple selectors, no selector, or a profile
/// alongside the raw-mode overrides. The overrides live INSIDE the raw variant for
/// that last reason — an earlier shape carried them beside the mode as
/// their own fields, which made "a profile-backed create must not carry
/// these" an invariant maintained by comment and `debug_assert` rather than
/// by the type.
///
/// Lives here rather than in `handlers` because the create path is what
/// consumes it: [`create_fingerprint`] encodes it, and
/// [`Supervisor::validate_create`] resolves it. `handlers::create_mode` is
/// only where a wire request is proven to name one.
pub(crate) enum CreateMode {
    /// The caller gave a command line, and may have overridden what would
    /// be derived from it.
    Raw {
        invocation: String,
        /// `None` means "derive the kind from the invocation's basename",
        /// which is a guess a raw caller may want. A profile is never a
        /// guess (see `farhelm_proto::Profile::agent_kind`).
        agent_kind: Option<AgentKind>,
        resume_template: Option<Vec<String>>,
    },
    /// The caller named one of this supervisor's profiles, which supplies
    /// every launch-shaping value. Resolved against the catalog during
    /// validation, so a profile deleted between a client's picker read and
    /// its submit fails the create visibly, before any launch, with no
    /// session left behind.
    Profile { profile_id: String },
    /// The caller named a profile by its human-facing name. Validation
    /// resolves the exact name atomically inside create and refuses zero or
    /// multiple matches with the candidates named.
    ProfileName { profile_name: String },
    /// A restricted spawn omitted `--agent`; resolve the host's last-used
    /// profile during validation without granting catalog-read authority.
    DerivedProfile,
}

/// Everything durable a session knows about resuming itself: its
/// integration snapshot (PLAN_M3.md item 7), the conversation identity
/// captured against it (item 8), and the two answers derived from the
/// pair.
///
/// Returned by [`Supervisor::session_snapshot`], which is the read PR8's
/// restart performs. Both derived fields are included rather than left to
/// the caller so that "what would restart do" and "what exactly would it
/// run" can never be computed two different ways by two callers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSnapshot {
    pub kind: AgentKind,
    pub resume_template: Option<Vec<String>>,
    pub captured_conversation: Option<String>,
    pub restart_offer: RestartOffer,
    /// The resume template with `{conversation}` filled in, or `None` when
    /// there is no template or nothing captured to fill it with. A
    /// `RestartOffer::Resume` session always has one; that is what the
    /// offer means.
    pub resume_argv: Option<Vec<String>>,
    /// When this session first had input CONFIRMED delivered, in seconds
    /// since the Unix epoch — the correlator capture keys on. Exposed for
    /// the capture tests, which need to distinguish "no record appeared"
    /// from "no first input was ever recorded" when a capture does not
    /// happen, and which wait on it to make their window arithmetic
    /// deterministic rather than sleep-based.
    pub first_input_at: Option<i64>,
    /// Whether correlation for this session was found ambiguous and no
    /// identity will ever be claimed for this launch. Durable, so this is
    /// also what a restart-after-ambiguity test asserts survived.
    pub capture_ambiguous: bool,
    /// The working directory correlation actually uses — resolved, not as
    /// the user spelled it. Exposed so the symlink and dot-path tests can
    /// assert the resolution happened rather than inferring it from a
    /// capture that might have succeeded for another reason.
    pub canonical_cwd: Option<String>,
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

    /// The reservation window this launch is running under, if keyed.
    fn dedup_scope(&self) -> Option<DedupScope> {
        match self {
            Reserved::Unkeyed(_) => None,
            Reserved::New { claim, .. } => Some(claim.dedup_scope),
            Reserved::Retry(reservation) => Some(reservation.dedup_scope),
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

/// How much a failed relaunch changed outside this process — the input to
/// every recovery decision `Supervisor::relaunch` makes.
///
/// The first two decide whether the previous run's outcome may be put back
/// (`SessionStore::abort_relaunch`). Restoring "exited, stopped by user"
/// over a session that may have a live agent under its new generation would
/// be a worse lie than the honest `Launching` that reload knows how to
/// reconcile — so only failures that PROVED nothing spawned are
/// `Definitive`, and every ambiguity (including every failure to probe one)
/// stays on the safe side.
///
/// [`RelaunchDisposition::Published`] is a different kind of answer
/// entirely and is the reason this is an enum rather than a `bool`. See its
/// own docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelaunchDisposition {
    /// Nothing outside this process changed: the previous outcome is still
    /// the truth about this session and may be restored.
    Definitive,
    /// An agent may be running under the new generation; the durable
    /// `Launching` record stays and reload reconciles it.
    Ambiguous,
    /// The relaunch REACHED ITS PUBLICATION: the new terminal, generation,
    /// and outcome are already installed on the map, and only the reply
    /// that describes them could not be produced. Recovery must not run.
    ///
    /// This exists because classifying such a failure as merely `Ambiguous`
    /// was actively destructive rather than conservative. The recovery path
    /// republishes the entry built from the PRE-restart `SessionEntry` — the
    /// old terminal, the old generation's pane, a `Launching` outcome — so a
    /// reply-build failure would overwrite the entry the relaunch had just
    /// published moments earlier. On a fresh-terminal restart that discards
    /// the live terminal outright: the map ends up pointing at a pane the
    /// restart replaced, while the agent it actually started runs in a
    /// terminal nothing references. SPEC.md's restart contract is that a
    /// successful restart's live terminal is what the session has, and a
    /// failure to build a REPLY has no standing to revise that.
    ///
    /// The error still surfaces — the caller is told what it must not
    /// assume — but the published state stands, and the next list describes
    /// the session correctly.
    Published,
}

/// Archive-state half of failed-relaunch recovery.
///
/// Only a definitive failure restores the prior durable generation; an
/// ambiguous failure may have launched an agent and must remain visible.
pub(crate) fn recovered_archive_flag(definitive: bool, prior_archived: bool) -> bool {
    definitive && prior_archived
}

/// Why a relaunch failed, and what the caller may do about it.
struct RelaunchFailure {
    error: anyhow::Error,
    disposition: RelaunchDisposition,
}

impl RelaunchFailure {
    /// See [`RelaunchDisposition::Definitive`].
    fn definitive(error: anyhow::Error) -> RelaunchFailure {
        RelaunchFailure {
            error,
            disposition: RelaunchDisposition::Definitive,
        }
    }

    /// See [`RelaunchDisposition::Ambiguous`].
    fn ambiguous(error: anyhow::Error) -> RelaunchFailure {
        RelaunchFailure {
            error,
            disposition: RelaunchDisposition::Ambiguous,
        }
    }

    /// See [`RelaunchDisposition::Published`]. Only ever constructed AFTER
    /// `publish_relaunched` has returned, which is the invariant that makes
    /// skipping recovery safe.
    fn published(error: anyhow::Error) -> RelaunchFailure {
        RelaunchFailure {
            error,
            disposition: RelaunchDisposition::Published,
        }
    }
}

/// A [`SessionEntry::last_activity_at`] cell holding `at`.
///
/// A named constructor rather than the `Arc::new(AtomicI64::new(..))`
/// spelling at each of the half-dozen sites that build an entry, for
/// [`ActivitySample::unsampled`]'s reason: the wrapper is part of the
/// contract and a bare value at the call sites invites getting it wrong
/// without noticing. Here the contract is that this cell is SESSION-wide —
/// only a session's first entry mints one, and every later entry for the
/// same session (rename, archive, relaunch) clones the `Arc` instead. A
/// call to this function anywhere but a create, a reload, or a test
/// fixture is a bug; see the field's own docs.
pub(crate) fn activity_stamp(at: i64) -> Arc<std::sync::atomic::AtomicI64> {
    Arc::new(std::sync::atomic::AtomicI64::new(at))
}

/// A [`SessionEntry::hooked`] / [`SessionEntry::hook_warned`] cell holding
/// `raised`.
///
/// A named constructor for [`activity_stamp`]'s reason: the `Arc` is part
/// of the contract rather than an implementation detail — a title-only
/// replacement SHARES the cell so a tick already holding the old entry
/// still writes where the published one is read from, and a relaunch mints
/// a fresh one, unconditionally and with a fresh `false`, so the previous
/// launch's injection and its diagnosis do not leak into the next.
/// Spelling the wrapper out at each entry-construction site is how that
/// distinction gets quietly lost.
///
/// Every fresh launch starts unhooked and unwarned; `with_hook_argv` is
/// the only thing that ever passes `true`.
pub(crate) fn hook_flag(raised: bool) -> Arc<std::sync::atomic::AtomicBool> {
    Arc::new(std::sync::atomic::AtomicBool::new(raised))
}

/// The decision half of [`Supervisor::with_hook_argv`], with every input
/// it needs passed in rather than read off a `Supervisor` (plan §2.3).
///
/// Split out for testability, and the split is worth its keep: the
/// interesting behaviour here is a list of REFUSALS — seven of them, five
/// logged and two silent — and reaching them through a real supervisor would
/// mean constructing one per case, tmux and SQLite included, to exercise a
/// function that touches neither. Its only effect beyond its return value
/// is tracing.
///
/// Returns the argv to launch and whether it was HOOKED, i.e. whether the
/// tail was actually appended. The caller carries that bool to the
/// [`SessionEntry::hooked`] tripwire; it is deliberately not recoverable
/// by inspecting the returned argv, because "does this argv end in flags
/// that look like ours" is exactly the kind of re-derivation that goes
/// quietly wrong when a vendor's flags change.
///
/// ## Why the order of the checks is the order it is
///
/// A kind with no integration ([`AgentKind::Generic`]) returns SILENTLY
/// and does so FIRST, before the opt-out is consulted at all: an
/// un-integrated session is the ordinary case, not a skipped injection,
/// and logging one line per generic launch would bury the skips that
/// do mean something. Asking [`crate::agent_kind::AgentHooks::allows`]
/// first would additionally start logging `disabled by FARHELM_AGENT_HOOKS`
/// against every generic session the moment an operator narrows the
/// opt-out, which describes nothing that operator did.
///
/// The other silent return is the same judgement one level down: an
/// integration whose [`crate::agent_kind::AgentIntegration::hook_argv`]
/// yields an EMPTY tail cannot be hooked at all, which is a fact about the
/// kind rather than about this launch, so logging it would repeat the
/// identical line on every launch of that kind forever.
///
/// The remaining five skips are all worth a line, because each is a case
/// where identity capture silently degrades to the record scan and the
/// only evidence is this log.
fn with_hook_argv_using(
    mut argv: Vec<String>,
    snapshot: &IntegrationSnapshot,
    hooks: &crate::agent_kind::AgentHooks,
    exe: Option<&str>,
    session: &str,
) -> (Vec<String>, bool) {
    let Some(integration) = snapshot.integration() else {
        return (argv, false);
    };
    let skip = |reason: &'static str| {
        info!(
            session = %session,
            kind = ?snapshot.kind,
            reason,
            "conversation hook flags not injected"
        );
    };
    if !hooks.allows(snapshot.kind) {
        skip("disabled by FARHELM_AGENT_HOOKS");
        return (argv, false);
    }
    let Some(exe) = exe else {
        skip("farhelm executable path is not utf-8");
        return (argv, false);
    };
    // Plan D4: both vendors take a trailing positional prompt, and a bare
    // `--` turns everything after it into that prompt's text. Appending
    // past one would not configure a hook, it would type our flags at the
    // agent.
    if argv.iter().any(|element| element == "--") {
        skip("invocation contains a bare --");
        return (argv, false);
    }
    // Plan D3: Claude Code honours only the LAST `--settings`, so a second
    // one appended after the user's would silently discard theirs —
    // turning an identity improvement into a lost configuration. Only
    // Claude's injection uses that flag, so only Claude has to yield here;
    // a Codex invocation carrying `--settings` means something else
    // entirely (or nothing) and is left alone.
    if snapshot.kind == AgentKind::Claude
        && argv
            .iter()
            .any(|element| element == "--settings" || element.starts_with("--settings="))
    {
        skip("invocation already passes --settings");
        return (argv, false);
    }
    // The Codex counterpart, and it refuses for two reasons at once (see
    // [`codex_invocation_configures_hooks`]): a second
    // `--dangerously-bypass-hook-trust` risks being rejected outright by
    // the vendor's own argument parser, which would turn an identity
    // improvement into a FAILED LAUNCH rather than a degraded one, and an
    // invocation already writing the `hooks.`/`features.hooks` tables owns
    // that configuration — appending ours after it is a merge nobody asked
    // for, over a table whose last-writer semantics we do not control.
    // Like every other skip this falls back to the record scan, so the
    // cost of being wrong here is an inferred identity, not a broken
    // session.
    if snapshot.kind == AgentKind::Codex && codex_invocation_configures_hooks(&argv) {
        skip("invocation already configures codex hooks");
        return (argv, false);
    }
    let tail = integration.hook_argv(exe);
    // An integration that offers no tail cannot be hooked — the trait's
    // default `hook_argv` returns exactly this, so a kind that grows a
    // record layout before it grows a hook lands here. Silently, and
    // WITHOUT the line below: claiming flags were injected when none were
    // would arm the tripwire against a launch that never had a hook to
    // begin with, and every reader of that log line would be chasing a
    // vendor bug that does not exist.
    if tail.is_empty() {
        return (argv, false);
    }
    argv.extend(tail);
    info!(
        session = %session,
        kind = ?snapshot.kind,
        "conversation hook flags injected"
    );
    (argv, true)
}

/// Whether a Codex invocation is already steering Codex's hook
/// configuration itself — the test behind the
/// `invocation already configures codex hooks` skip.
///
/// True for either of the two shapes farhelm's own tail would collide
/// with:
///
/// - `--dangerously-bypass-hook-trust`, the flag the injected tail leads
///   with. Whether Codex tolerates the same flag twice is the vendor's
///   business and unverified here; the downside of guessing wrong is a
///   launch that fails to start, which is the one outcome injection is
///   never allowed to cause.
/// - a `-c` override whose value assigns into `hooks.` or
///   `features.hooks`, which is precisely the namespace the tail writes.
///
/// Both `-c value` and `-cvalue` count, mirroring the two spellings the
/// Claude `--settings` rule above accepts for the same reason: they are
/// one flag to the vendor's parser, so a check that saw only one spelling
/// would be trivially and silently bypassed by the other.
///
/// Deliberately NOT a general "does this argv touch config" test: an
/// invocation carrying unrelated `-c` overrides (`-c model=...`) has no
/// quarrel with the hook tables and stays hooked.
fn codex_invocation_configures_hooks(argv: &[String]) -> bool {
    /// The two config prefixes the injected tail assigns into.
    fn steers_hook_tables(value: &str) -> bool {
        value.starts_with("hooks.") || value.starts_with("features.hooks")
    }

    let mut elements = argv.iter().peekable();
    while let Some(element) = elements.next() {
        if element == "--dangerously-bypass-hook-trust" {
            return true;
        }
        // The separated spelling PEEKS rather than consuming: the value
        // element is examined again on the next turn, where it matches
        // neither arm, which keeps this loop a plain scan rather than a
        // half-implementation of the vendor's argument grammar.
        let value = if element == "-c" {
            elements.peek().map(|next| next.as_str())
        } else {
            element.strip_prefix("-c").filter(|value| !value.is_empty())
        };
        if value.is_some_and(steers_hook_tables) {
            return true;
        }
    }
    false
}

/// Where one session's hook trace lives: `<state_dir>/hook-log/<id>.log`.
///
/// The hook process is the ONLY place a hook-side failure is ever visible
/// — it may not write to stdout or stderr, because the terminal it would
/// write to is the agent's — so it appends one line per run to this file.
/// The supervisor never reads it; it owns this path for exactly one
/// reason, which is to remove the file when the session is deleted, the
/// same way a session's attachments and launch specs go.
///
/// ## The two derivations must agree
///
/// The writer (`crates/farhelm/src/hook.rs`, a separate step of the same
/// plan) is a short-lived process that knows nothing about the supervisor
/// beyond the three environment variables the agent was launched with, so
/// it derives the state directory as the PARENT of `FARHELM_SUPERVISOR_SOCK`
/// — the socket is `<state_dir>/supervisor.sock` — and rebuilds this same
/// path from it. Nothing checks that the two agree at runtime: a
/// divergence is silent, and its symptom is per-session trace files that
/// accumulate forever because delete no longer finds them. Change one side
/// and you must change the other.
pub(crate) fn hook_log_path(state_dir: &Path, session_id: &str) -> PathBuf {
    state_dir.join("hook-log").join(format!("{session_id}.log"))
}

/// Build the entry that describes the SAME launch under a new title
/// (PLAN_M5.md item 3) — everything but `info.title` carried over.
///
/// A rebuild rather than a mutation because [`SessionEntry`] is
/// immutable-once-created behind an `Arc` (see its own docs); this is the
/// in-memory half of rename's two-part write, and the half that makes a
/// renamed title show up in the very next `ListSessions` reply. The
/// durable row alone would not: the supervisor serves list replies from
/// these entries and never re-reads SQLite mid-process, so a store-only
/// rename would stay invisible until a restart.
///
/// Generation, scope and terminal carry over unchanged, which is the
/// difference from [`relaunched_entry`]: a rename is not a new run, so
/// nothing about the run may be reset.
///
/// The mutable cells are all SHARED — the `Arc`s are cloned, not their
/// values — and that is the whole reason [`SessionEntry`] wraps them in
/// `Arc` at all. A title-only replacement does not end anything, so every
/// holder of the old entry is still a legitimate writer about the SAME
/// run: the input path writes the first-input anchor through whatever
/// entry its `InputRoute` pinned at attach time, a capture pass advances
/// state it gathered before the rename, and a list pass commits an
/// outcome it observed a moment ago. Snapshotting instead would strand
/// every one of those writes in a cell nothing reads again — silently, and
/// most damagingly for capture, whose window would never open (see the
/// struct's own docs).
fn renamed_entry(entry: &SessionEntry, title: String) -> Arc<SessionEntry> {
    let mut info = entry.info.clone();
    info.title = title;
    Arc::new(SessionEntry {
        info,
        terminal: entry.terminal.clone(),
        outcome: Arc::clone(&entry.outcome),
        snapshot: entry.snapshot.clone(),
        canonical_cwd: entry.canonical_cwd.clone(),
        first_input: Arc::clone(&entry.first_input),
        capture: Arc::clone(&entry.capture),
        hooked: Arc::clone(&entry.hooked),
        hook_warned: Arc::clone(&entry.hook_warned),
        activity: Arc::clone(&entry.activity),
        last_activity_at: Arc::clone(&entry.last_activity_at),
        generation: entry.generation,
        scope: entry.scope.clone(),
    })
}

/// Build the entry that describes a session's NEW launch generation,
/// carrying over exactly what describes the CONVERSATION rather than the
/// run.
///
/// A fresh `Arc` rather than a mutation, which is what makes
/// [`SessionEntry::generation`] a real fence: anything still holding the
/// previous entry keeps describing the previous run, and its durable
/// writes are rejected rather than silently landing on this one.
///
/// `reset_capture` mirrors the durable decision
/// [`SessionStore::begin_relaunch`] made: a relaunch that opened a fresh
/// capture window must not carry the previous run's first-input anchor or
/// verdict in memory either, or the very next capture pass would correlate
/// the new run against a window that closed long ago.
///
/// Every RUN-SCOPED mutable cell is fresh here — new `Arc`s, never the
/// previous entry's — even where the VALUE is carried over. That isolation
/// is the in-memory half of the generation fence: a list or capture pass
/// still holding the old entry is describing a run that has ended, and its
/// late write must land in the abandoned run's cell rather than on the
/// launch that replaced it. It is the exact opposite of what
/// [`renamed_entry`] needs, which is why the two build their cells
/// differently.
///
/// `last_activity_at` is the one cell that is NOT run-scoped and is
/// therefore shared rather than re-minted; the comment at its field below
/// argues why fencing it would be a bug rather than a safeguard. Anyone
/// adding a cell here should decide which of the two it is before copying
/// either pattern.
fn relaunched_entry(
    entry: &SessionEntry,
    info: SessionInfo,
    terminal: Option<Terminal>,
    generation: i64,
    scope: Option<String>,
    outcome: LastOutcome,
    reset_capture: bool,
) -> Arc<SessionEntry> {
    let (first_input, capture) = if reset_capture {
        (
            FirstInput {
                at: None,
                durable: true,
            },
            CaptureState::Unclaimed,
        )
    } else {
        (
            *entry
                .first_input
                .lock()
                .expect("first-input mutex poisoned"),
            entry
                .capture
                .lock()
                .expect("capture mutex poisoned")
                .clone(),
        )
    };
    Arc::new(SessionEntry {
        info,
        terminal,
        outcome: Arc::new(std::sync::Mutex::new(outcome)),
        snapshot: entry.snapshot.clone(),
        canonical_cwd: entry.canonical_cwd.clone(),
        first_input: Arc::new(std::sync::Mutex::new(first_input)),
        capture: Arc::new(std::sync::Mutex::new(capture)),
        // Fresh cells with fresh VALUES, on every relaunch and whatever
        // `reset_capture` says — the one pair here that does not follow the
        // capture window. Both describe a LAUNCH's hook injection and the
        // diagnostic spent on it, not the conversation: a launch this
        // process has not spawned yet is not hooked until `with_hook_argv`
        // says so, and `publish_relaunched` is what raises the flag when it
        // did. Carrying them over a Resume would state something about the
        // new launch that only the old one had established, and would carry
        // a warning already spent — so a second broken launch of the same
        // session would trip the wire silently.
        hooked: hook_flag(false),
        hook_warned: hook_flag(false),
        // Never carried over, whatever `reset_capture` says: the sampled
        // tail and the unchanged-sample streak beside it both describe a
        // process that no longer exists. Inheriting them would classify the
        // replacement launch from its predecessor's screen — quiet because
        // the OLD pane stopped changing, or sharpened `Waiting` from a
        // dialog the previous run was showing when it died.
        activity: ActivitySample::unsampled(),
        // The one cell SHARED across a relaunch, and the exception to the
        // paragraph above is deliberate. The generation fence exists
        // because the cells beside it describe one RUN: a late write about
        // a process that has ended must not be read as describing the
        // process that replaced it. This value is not scoped to a run at
        // all — "the last time anything was seen happening in this
        // session" is equally true whichever launch produced the output —
        // so there is nothing here for a fence to protect, and minting a
        // fresh cell would instead open a hole. A sampler that read the
        // old entry, awaited, and then advanced it would leave the
        // published entry BEHIND the row its own write just advanced in
        // SQLite, and the discrepancy would survive until the next
        // observed change past the quantum. Sharing the `Arc` makes that
        // impossible rather than merely unlikely: there is one cell per
        // session, and every holder of any entry for it writes to the same
        // place. Monotonicity across processes and clock steps is still
        // the store's predicate to enforce.
        last_activity_at: Arc::clone(&entry.last_activity_at),
        generation,
        scope,
    })
}

/// Refuse a relaunch whose working directory no longer resolves to the
/// path this session was created against (fix-batch item 21; PLAN_M6_75.md
/// item 4 gave it a second caller).
///
/// `ensure_cwd_usable` answers "is there a usable directory here"; this
/// answers "is it the SAME one". They are different questions the moment a
/// symlink is involved: a path that pointed at one worktree at create time
/// can point at another by restart time, and relaunching an agent — often
/// one launched with permissive flags — into a directory somebody else
/// chose is not a refusal a user could reasonably be expected to make for
/// themselves. The stored canonical path is the identity to compare
/// against precisely because it was resolved once, at create, and is
/// immutable after (`store::StoredSession::canonical_cwd`).
///
/// The two callers are `restart_session` and
/// [`Supervisor::validate_retry`], and the second wants it for a reason
/// worth stating separately: a pending retry carries the crashed attempt's
/// `canonical_cwd` forward, so a symlink repointed between the two attempts
/// would launch the agent in the NEW target while conversation capture went
/// on correlating against the OLD one — no failure anywhere, just a session
/// that never captures its conversation (or correlates against another
/// project's records) for the rest of its life.
///
/// Sessions with no stored canonical path (rows predating the column) skip
/// the check rather than fail it: there is nothing to compare, and
/// inventing a mismatch would refuse every such session's restart forever.
/// That case answers `None`, and its caller launches into `cwd` unchanged.
///
/// ## Why it fails closed, and why it returns a path
///
/// Both were bugs, and they were the same bug seen from two sides: the
/// check looked like a guard while leaving the door open.
///
/// A path that cannot be canonicalized used to be waved through with a
/// warning, on the reasoning that `ensure_cwd_usable` had already found a
/// directory there. That reasoning inverts the threat. The whole scenario
/// this defends against is a path whose meaning CHANGED, and a
/// canonicalization that fails against a directory that stats fine is
/// exactly the shape a hostile or broken path takes — so the one input that
/// most deserves refusing was the one it let through. A session that has a
/// recorded identity and cannot be checked against it is refused; a session
/// with no recorded identity is untouched, since there was never anything
/// to check.
///
/// And a successful comparison is not the end of the guard's job. The
/// caller was passing the ORIGINAL, still-symlinked `cwd` to tmux
/// afterwards, so a link repointed between the comparison and the launch
/// put the agent in a directory nothing validated — the classic
/// time-of-check/time-of-use window, and a wide one, since a launch is
/// several awaits and a subprocess away. Handing back the RESOLVED path and
/// launching into that closes it: a path with no symlink components left
/// cannot be repointed, so what was verified is what runs.
async fn ensure_cwd_identity(cwd: &str, canonical: Option<&str>) -> anyhow::Result<Option<String>> {
    let Some(canonical) = canonical else {
        return Ok(None);
    };
    let resolved = match tokio::fs::canonicalize(cwd).await {
        Ok(resolved) => resolved.to_string_lossy().into_owned(),
        Err(e) => {
            return Err(RequestError::new(
                ErrorKind::InvalidRequest,
                format!(
                    "working directory {cwd} could not be resolved ({e}), so it cannot be \
                     confirmed to still name {canonical} where this session was created; \
                     refusing to relaunch its agent into a directory nothing could check"
                ),
            )
            .into());
        }
    };
    if resolved == canonical {
        return Ok(Some(resolved));
    }
    Err(RequestError::new(
        ErrorKind::InvalidRequest,
        format!(
            "working directory {cwd} now resolves to {resolved}, not to {canonical} where this \
             session was created; refusing to relaunch its agent somewhere else"
        ),
    )
    .into())
}

/// The geometry a relaunch's FRESH terminal starts at, for the same reason
/// the helm's create API has defaults: a restart request carries no
/// terminal size — it can come from a session view whose pane is not laid
/// out, from the API, or from a client that never had a terminal at all.
/// These decide how the agent's first output wraps until something resizes
/// the window, which for the ordinary flow is the next attach
/// (`Attach` resizes to its own client's size) — a client that never
/// attaches leaves the window at exactly this size. A REUSED terminal
/// keeps whatever geometry it already had and never consults these.
const RELAUNCH_COLS: u16 = 80;
const RELAUNCH_ROWS: u16 = 24;

/// The command a restart runs, and the validation of `mode` against the
/// session's CURRENT offer that decides whether there is one at all
/// (PLAN_M3.md item 9; `ControlMsg::RestartSession`'s staleness contract).
///
/// One function for both halves deliberately: the offer and the argv are
/// two statements of the same fact, and computing them apart is how a
/// build ends up validating against one answer and running the other. A
/// pure function of the durable snapshot plus the launch invocation, so the
/// whole mode/offer matrix is unit-testable without a supervisor.
///
/// The mode/offer pairing is exact in both directions — there is no
/// "mode the user is allowed to downgrade to". `Fresh` against a `Resume`
/// offer is refused for the reason SPEC.md gives ("no fresh-restart variant
/// in v1 — for a clean conversation, create a new session in the same
/// directory"), and `Resume` against a `FreshOnly` offer is refused because
/// there is nothing to fill the template with — the case that would
/// otherwise run a `{conversation}` placeholder unfilled, which SPEC.md
/// forbids outright.
///
/// The `Conflict` names the CURRENT offer, because the client's next move
/// is to re-present that offer to the user rather than to retry.
///
/// Whatever the mode, the vector that comes back has been through the
/// shared executable-argv rule (`agent_kind::ensure_executable_argv`): all
/// three sources are durable columns, and a restart is the moment a stored
/// argv stops being data and becomes an exec.
fn relaunch_argv(
    mode: RestartMode,
    snapshot: &SessionSnapshot,
    invocation: &str,
) -> anyhow::Result<Vec<String>> {
    let expected = match snapshot.restart_offer {
        RestartOffer::FreshOnly => RestartMode::Fresh,
        RestartOffer::Resume => RestartMode::Resume,
        RestartOffer::FallbackTemplate => RestartMode::FallbackTemplate,
    };
    if mode != expected {
        return Err(RequestError::new(
            ErrorKind::Conflict,
            format!(
                "this session's restart offer is {}, not {}; refresh the session and \
                 re-present the offer rather than retrying",
                offer_wording(snapshot.restart_offer),
                mode_wording(mode)
            ),
        )
        .into());
    }
    let argv = match mode {
        // Already substituted, slot by slot, from the DURABLE identity
        // (`IntegrationSnapshot::filled_resume_argv`) — never spliced into
        // a command string, so an id cannot become a different command.
        RestartMode::Resume => snapshot.resume_argv.clone().ok_or_else(|| {
            anyhow::Error::new(RequestError::new(
                ErrorKind::Internal,
                "this session offers a resume but has no filled resume command; refusing to \
                 relaunch rather than guessing one",
            ))
        }),
        RestartMode::FallbackTemplate => snapshot.resume_template.clone().ok_or_else(|| {
            anyhow::Error::new(RequestError::new(
                ErrorKind::Internal,
                "this session offers a fallback resume command but has no template; refusing \
                 to relaunch rather than guessing one",
            ))
        }),
        // The session's own launch invocation, parsed exactly as the
        // create parsed it. A parse failure here means the stored
        // invocation is not a command line any more, which is the caller's
        // to fix (by creating a new session), not this build's to guess at.
        RestartMode::Fresh => shell_words::split(invocation).map_err(|e| {
            anyhow::Error::new(RequestError::new(
                ErrorKind::InvalidRequest,
                format!("this session's launch invocation no longer parses: {e}"),
            ))
        }),
    }?;
    // The last gate before an argv becomes a real exec, whichever of the
    // three modes produced it. All three read from DURABLE columns — the
    // launch invocation, the stored template, the substituted identity —
    // and a row written by an older build (or edited by hand) can carry a
    // vector that names no program or hides a NUL. The same rule profile
    // writes enforce, applied where the vector is about to be run rather
    // than only where it was accepted.
    crate::agent_kind::ensure_executable_argv("this session's restart command", &argv).map_err(
        |message| anyhow::Error::new(RequestError::new(ErrorKind::InvalidRequest, message)),
    )?;
    Ok(argv)
}

/// How an offer reads in a refusal aimed at a user. Deliberately prose
/// rather than the wire spelling: this text lands in an HTTP body and a UI
/// line, not in anything a client branches on (`ErrorKind` carries that).
fn offer_wording(offer: RestartOffer) -> &'static str {
    match offer {
        RestartOffer::FreshOnly => "a fresh launch (no conversation was captured for it)",
        RestartOffer::Resume => "resuming its captured conversation",
        RestartOffer::FallbackTemplate => "its configured fallback resume command",
    }
}

/// The requested mode's half of the same sentence; see [`offer_wording`].
fn mode_wording(mode: RestartMode) -> &'static str {
    match mode {
        RestartMode::Fresh => "a fresh launch",
        RestartMode::Resume => "resuming its captured conversation",
        RestartMode::FallbackTemplate => "its configured fallback resume command",
    }
}

/// What a successful [`Supervisor::spawn_agent`] produced: the pane the
/// agent is starting in, plus the two per-launch paths it published to —
/// which a caller that later fails to confirm the launch has to clean up,
/// and which the shim itself normally consumes on its way to `exec`.
///
/// The paths are RETURNED rather than recomputed by callers so nothing
/// downstream can derive a path the spawn did not actually use — the
/// spec's own `status_file` field is what the shim writes to, and a caller
/// cleaning up a differently-derived path would leave the real sentinel in
/// place for the next launch to misread.
struct Spawned {
    pane: String,
    spec_path: PathBuf,
    status_path: PathBuf,
    /// Whether this launch's argv carried the conversation hook — see
    /// [`Supervisor::with_hook_argv`], which decided it.
    ///
    /// Returned rather than left on the supervisor because the cell that
    /// records it, [`SessionEntry::hooked`], belongs to the entry
    /// PUBLISHED for this launch, and on the create path that entry does
    /// not exist until the launch is confirmed. Both callers carry this
    /// value forward to that publication; nothing else reads it.
    hooked: bool,
}

/// Why a launch's side effects did not complete, split by what the caller
/// may conclude from it — see [`Supervisor::spawn_agent`] for why the two
/// cannot be unwound the same way.
enum SpawnFailure {
    /// The spec never landed, so nothing external happened at all. Carries
    /// no path to clean up on purpose: the write helper's own contract is
    /// that a failed staged write leaves neither a temp file nor a
    /// published one (`files::write_staged`), so there is nothing here for
    /// a caller to remove.
    Spec(anyhow::Error),
    /// tmux refused, or answered ambiguously. The agent may or may not be
    /// running; only a probe can say. The spec DID land, and the shim may
    /// never run to consume it, so the caller is handed the path: it holds
    /// the agent's full command line, credentials included.
    Tmux {
        spec_path: PathBuf,
        error: anyhow::Error,
    },
}

/// Expand a `~`-prefixed working directory against the SUPERVISOR's home
/// (SPEC.md: `~` names the home of the user running the supervisor on the
/// target host). Anything not starting with `~` passes through untouched.
///
/// This runs at create time, before validation and before anything is
/// stored, so the durable record only ever holds the expanded absolute
/// path. That placement is what keeps `ensure_cwd_usable`'s anti-drift
/// argument intact: expansion happens exactly once, against a home
/// resolved once at supervisor construction, so the stored path can never
/// re-resolve differently after a daemon restart. Restart and tab-open
/// re-check the STORED cwd and therefore never see a `~`.
///
/// Expansion is deliberately supervisor-side rather than in the helm or
/// UI: the helm runs on the user's machine, and expanding there would
/// substitute the wrong home for a remote host.
///
/// `~user` forms are refused rather than expanded — resolving other
/// users' homes is a separate feature nobody asked for — and `~` with no
/// home to expand against (no seam, no usable `$HOME`) is refused with a
/// message naming the real problem instead of falling through to a
/// baffling "not absolute" error.
fn expand_tilde_cwd<'a>(
    cwd: &'a str,
    home: Option<&Path>,
) -> anyhow::Result<std::borrow::Cow<'a, str>> {
    use std::borrow::Cow;
    let rest = match cwd {
        "~" => "",
        _ => match cwd.strip_prefix("~/") {
            Some(rest) => rest,
            None if cwd.starts_with('~') => {
                return Err(RequestError::new(
                    ErrorKind::InvalidRequest,
                    format!(
                        "working directory {cwd} looks like a ~user path, which is not \
                         supported; use ~ or ~/path (expanded against the supervisor's own \
                         home) or an absolute path"
                    ),
                )
                .into());
            }
            None => return Ok(Cow::Borrowed(cwd)),
        },
    };
    let home = home
        .ok_or_else(|| {
            RequestError::new(
                ErrorKind::InvalidRequest,
                format!(
                    "working directory {cwd} needs ~ expansion, but this supervisor has no \
                     home directory (no HOME in its environment); use an absolute path"
                ),
            )
        })?
        .to_str()
        .ok_or_else(|| {
            RequestError::new(
                ErrorKind::Internal,
                "the supervisor's home directory is not valid UTF-8, so ~ cannot be expanded \
                 into a session working directory; use an absolute path",
            )
        })?;
    // A relative home would expand `~/x` into another relative path, and
    // `ensure_cwd_usable` would then blame the CALLER for a host
    // misconfiguration ("working directory is not absolute: home/x").
    // Refusing here keeps the diagnosis pointed at the real problem.
    if !Path::new(home).is_absolute() {
        return Err(RequestError::new(
            ErrorKind::Internal,
            format!(
                "the supervisor's home directory ({home}) is not an absolute path, so ~ cannot \
                 be expanded into a session working directory; use an absolute path"
            ),
        )
        .into());
    }
    if rest.is_empty() {
        return Ok(Cow::Owned(home.to_string()));
    }
    Ok(Cow::Owned(format!("{}/{rest}", home.trim_end_matches('/'))))
}

/// Whether `cwd` is usable as a session's working directory, as the one
/// precondition both a create and a restart check (SPEC.md: "Operations
/// that need the working directory — restart, opening a terminal tab —
/// fail with a clear error naming the directory if it has vanished since
/// creation").
///
/// Shared rather than duplicated specifically so restart's refusal is the
/// SAME refusal create's is, down to the wording and the `ErrorKind`: a
/// directory that has since been removed is the ordinary way this fails
/// for a restart, and a user who has seen the create-time message should
/// not have to recognize a second phrasing of it.
///
/// Every classification below preserves the distinction between a bad
/// caller precondition and a host I/O failure. Calling both "does not
/// exist" sends users looking for a typo when the real problem is
/// permission, a symlink loop, or a failing filesystem.
///
/// `cwd` must be absolute. A relative path, if accepted, would be stored
/// durably and handed to tmux, which resolves it against the SUPERVISOR
/// DAEMON's own working directory, not the client's — so its meaning would
/// shift with wherever the daemon happens to have been started, and
/// re-shift on every daemon restart. That has already produced a real
/// failure mode: a session created with a relative cwd would resolve fine
/// as long as the daemon kept its original working directory, then either
/// fail to restart with "does not exist" or, worse, silently relaunch the
/// agent in the wrong directory once the daemon was restarted from
/// elsewhere. Rejecting relative paths up front — for both create and
/// restart, since this check is shared — also catches cwds that were
/// stored before this check existed. A `~`-prefixed cwd never reaches
/// this check on the create path: [`expand_tilde_cwd`] has already turned
/// it into an absolute path (or refused it) by the time validation runs.
async fn ensure_cwd_usable(cwd: &str) -> anyhow::Result<()> {
    if !std::path::Path::new(cwd).is_absolute() {
        return Err(RequestError::new(
            ErrorKind::InvalidRequest,
            format!(
                "working directory is not absolute: {cwd} (a relative path would resolve against the supervisor process, not the client)"
            ),
        )
        .into());
    }
    match tokio::fs::metadata(cwd).await {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(RequestError::new(
            ErrorKind::InvalidRequest,
            format!("working directory is not a directory: {cwd}"),
        )
        .into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(RequestError::new(
            ErrorKind::InvalidRequest,
            format!("working directory does not exist: {cwd}"),
        )
        .into()),
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
            Err(RequestError::new(
                ErrorKind::InvalidRequest,
                format!("working directory is not usable: {cwd} ({error})"),
            )
            .into())
        }
        // Not classified: this is an I/O failure the caller could not have
        // avoided by sending a different request (a permission problem, a
        // symlink loop, a failing filesystem), so it defaults to
        // `ErrorKind::Internal`.
        Err(error) => {
            Err(error).with_context(|| format!("reading working directory metadata for {cwd}"))
        }
    }
}

/// Refuse a CALLER-SUPPLIED title that could not survive being printed as
/// a one-line label — the rule `validate_create`'s explicit-title arm
/// states in full, shared here so create and rename cannot drift apart.
///
/// Sharing is the point rather than a convenience: PLAN_M5.md item 3 makes
/// rename's validation "`validate_create`'s explicit-title arm, no more and
/// no less", and a client's whole contract for a refused title is the
/// supervisor's own words — two copies of this message would eventually
/// become two different answers to the same question depending on which
/// verb the user reached for.
///
/// An empty title passes, deliberately: SPEC.md names control characters as
/// THE refusal for a supplied title, and neither verb invents a stricter
/// rule. Server-DERIVED titles do not come here at all — they are sanitized
/// instead of refused, for the asymmetry `validate_create` argues out.
pub(crate) fn ensure_title_printable(title: &str) -> Result<(), RequestError> {
    if title.chars().any(char::is_control) {
        return Err(RequestError::new(
            ErrorKind::InvalidRequest,
            "title must not contain control characters",
        ));
    }
    Ok(())
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
pub(crate) fn truncate_for_error(id: &str) -> std::borrow::Cow<'_, str> {
    if id.len() <= ECHOED_ID_MAX {
        return std::borrow::Cow::Borrowed(id);
    }
    let mut end = ECHOED_ID_MAX;
    while !id.is_char_boundary(end) {
        end -= 1;
    }
    std::borrow::Cow::Owned(format!("{}...", &id[..end]))
}

/// What one relaunch produced, as [`Supervisor::publish_relaunched`]
/// needs it.
///
/// A struct rather than five more parameters because they describe ONE
/// thing — the state this relaunch arrived at — and because three of them
/// (`outcome`, `reset_capture`, `tabs`) are easy to transpose at a call
/// site while still type-checking.
struct Relaunched {
    terminal: Terminal,
    /// The cgroup scope the NEW generation launched into.
    scope: Option<String>,
    /// The outcome that actually committed, which is not always the one
    /// the relaunch intended — see the call sites.
    outcome: LastOutcome,
    /// Whether the new run starts conversation capture from scratch.
    reset_capture: bool,
    /// Whether THIS relaunch's argv carried the conversation hook, as
    /// [`Supervisor::with_hook_argv`] decided and [`Spawned::hooked`]
    /// reported back.
    ///
    /// It travels with the publication rather than being applied to
    /// `entry` because [`SessionEntry::hooked`] is per-LAUNCH: the entry
    /// this relaunch was computed FROM describes the run that just ended,
    /// and `relaunched_entry` mints the new generation's cell as `false`
    /// unconditionally — including for a Resume, whose retained capture
    /// state says nothing about whether the new argv was injected.
    /// Raising it on the published entry is the only placement that
    /// survives.
    hooked: bool,
    /// The session's tabs as they were BEFORE the relaunch: restart
    /// touches the agent terminal alone, so these are still exactly the
    /// tabs it has. Captured early rather than rediscovered here — see
    /// `relaunch_into_terminal`'s own comment for why a post-restart
    /// query must not be allowed to report `[]`.
    tabs: Vec<TabInfo>,
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
/// talking to tmux. Changing one therefore means PUBLISHING A REPLACEMENT
/// into the map: [`relaunched_entry`] for a new launch generation,
/// [`renamed_entry`] for the title (the only field a user can change after
/// creation).
///
/// ## Two kinds of replacement, and why the mutable cells are `Arc`ed
///
/// The four interior-mutable cells below (`outcome`, `first_input`,
/// `capture`, `activity`) are `Arc<Mutex<..>>` rather than plain `Mutex<..>` because
/// the two replacement paths need OPPOSITE things from them, and the
/// wrapper is what lets one type express both.
///
/// A RELAUNCH must isolate: the new generation gets fresh cells, so a
/// list pass or capture scan still holding the previous entry writes its
/// late conclusion into the abandoned run's cells and cannot contaminate
/// the new one (the generation fence does the same job durably; this is
/// its in-memory half).
///
/// [`SessionEntry::last_activity_at`] is beside those four and obeys
/// NEITHER rule as stated: it is shared by both paths, because it is the
/// one mutable value here that belongs to the session rather than to a
/// run. Its own docs argue why fencing it would lose writes instead of
/// containing them.
///
/// A RENAME must share: it describes the SAME run, so its replacement
/// clones the Arcs. Anything still holding the pre-rename entry — an
/// `InputRoute` pinned at attach time, a list pass mid-flight, a capture
/// pass mid-scan — keeps writing into the very cells the published entry
/// reads. Snapshotting the values instead silently split the session in
/// two: a rename before first input would leave `super::capture::note_first_input`
/// writing an anchor nobody would ever read, and the capture pass would
/// scan forever against a window that never opened — SPEC.md's resume
/// promise broken by renaming a session at the wrong moment, with nothing
/// anywhere reporting it.
pub(crate) struct SessionEntry {
    pub(crate) info: SessionInfo,
    pub(crate) terminal: Option<Terminal>,
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
    ///
    /// Shared with any title-only replacement of this entry; see the
    /// struct's own docs for why sharing and isolation are both needed.
    pub(crate) outcome: Arc<std::sync::Mutex<LastOutcome>>,
    /// This session's integration snapshot (PLAN_M3.md item 7), resolved
    /// at create and immutable for the session's life — hence a plain
    /// field rather than another mutex. Read by the capture pass (to know
    /// which agent's records to look for, if any) and by every reply that
    /// computes a restart offer.
    pub(crate) snapshot: IntegrationSnapshot,
    /// This session's working directory with symlinks, `.`/`..`, and a
    /// trailing slash resolved away, resolved at create and immutable
    /// (`store::StoredSession::canonical_cwd` explains why correlation
    /// cannot use the user-facing spelling). `None` only for a row that
    /// predates the column, which is necessarily non-integrated.
    pub(crate) canonical_cwd: Option<String>,
    /// When this supervisor first confirmed delivery of input to this
    /// session (PLAN_M3.md item 8's correlator), and whether that fact has
    /// reached the database yet. See [`FirstInput`] and
    /// `super::capture::note_first_input`.
    ///
    /// The cell most exposed to the sharing rule above: its writer is the
    /// INPUT path, which holds whatever entry its `InputRoute` pinned at
    /// attach time and is never handed a newer one.
    pub(crate) first_input: Arc<std::sync::Mutex<FirstInput>>,
    /// Where this session stands on conversation-identity capture. See
    /// [`CaptureState`]. Shared across title-only replacements like the
    /// two cells above — a capture pass mid-scan must be advancing the
    /// same state the published entry is read from.
    pub(crate) capture: Arc<std::sync::Mutex<CaptureState>>,
    /// Whether THIS launch's argv carried the conversation-reporting hook
    /// ([`Supervisor::with_hook_argv`], which is what decides it).
    ///
    /// Diagnostics only: nothing about capture, restart, or the wire reply
    /// consults it, and nothing persists it. It describes what this
    /// process did when it spawned this generation, so a supervisor that
    /// did not perform the launch — one that reloaded the session after a
    /// restart — has nothing to say about it and correctly comes back with
    /// the flag clear.
    ///
    /// Its one reader is the liveness tripwire in
    /// [`crate::service::capture`]. A hooked launch that is still not
    /// [`CaptureState::Reported`] well past its capture horizon means no
    /// report was successfully ACCEPTED — which is a broader statement
    /// than "the hook never ran", and the tripwire's message says so.
    /// A hook that ran and was refused (a credential the store could not
    /// validate, an implausible id, a supervisor that was not recording)
    /// leaves exactly the same silence here; the hook's own trace file is
    /// what separates the cases, which is why the two are diagnosed
    /// together rather than from this flag alone.
    ///
    /// Either way the failure is otherwise INVISIBLE, because the scan
    /// fallback keeps working and the session looks entirely ordinary — a
    /// vendor that renamed its flag, a wrapper script that swallowed the
    /// tail, a hook that never got to run. For those the tripwire is the
    /// only place anything surfaces.
    ///
    /// What it deliberately does NOT cover is the other half of the
    /// picture: a launch that was REFUSED injection in the first place —
    /// an invocation carrying its own `--settings`, a bare `--`, a kind
    /// excluded by `FARHELM_AGENT_HOOKS`. Those are diagnosed at INJECTION
    /// time, by [`with_hook_argv_using`]'s skip line, and they leave this
    /// flag clear, so the tripwire never arms for them and its silence is
    /// correct rather than a second failure. The two questions a reader
    /// has to keep apart are "were the flags ever put on the command line"
    /// (the skip line answers it, at launch) and "were they put on and
    /// nothing came back" (this flag plus the tripwire, well after).
    ///
    /// STRICTLY per-launch — and note that this is a narrower scope than
    /// the capture window beside it, which is the one thing about this
    /// field most likely to be "tidied" wrong. [`relaunched_entry`] mints
    /// it `false` on EVERY relaunch, including a Resume that keeps its
    /// capture state, because the question it answers is "was THIS launch's
    /// argv injected", and no previous launch can answer that. Injection
    /// can genuinely differ between two launches of one session: a resume
    /// template that already carries a `--` or a `--settings`, a kind newly
    /// excluded, a `farhelm_exe` that stopped resolving. A carried-over
    /// `true` would arm the tripwire for a launch that never had a hook and
    /// warn about a report that was never going to come.
    ///
    /// Whoever raises it must therefore raise it on the entry PUBLISHED for
    /// the new generation, not on the previous entry a relaunch was
    /// computed from. The two writers honour that in the two shapes their
    /// paths allow: a create builds its entry only after the launch is
    /// confirmed and seeds the cell from [`Spawned::hooked`] directly,
    /// while a restart's entry is minted by `relaunched_entry` inside
    /// [`Supervisor::publish_relaunched`], which raises the flag there when
    /// the new launch was injected. Nothing outside a launch ever writes
    /// it.
    ///
    /// One accepted hole follows from that: a restart whose tmux command
    /// failed AMBIGUOUSLY may in fact be running a hooked agent, and the
    /// generic recovery republishes it through `relaunched_entry` with this
    /// flag clear, because the failure path never learns what the spawn
    /// decided. Reconstructing it is not worth it — the flag feeds nothing
    /// but the tripwire, so the entire cost is one launch whose missing
    /// report goes unwarned about.
    pub(crate) hooked: Arc<std::sync::atomic::AtomicBool>,
    /// Whether the tripwire described above has already logged for this
    /// launch, so a hook that never reports costs one line per launch
    /// rather than one per tick for the life of the session.
    ///
    /// Minted `false` exactly where `hooked` is, and for the sharper half
    /// of the same reason: a warning already spent belongs to the launch
    /// that earned it. Carrying it across a relaunch would silence the
    /// tripwire for the NEXT broken launch of the same session — the case
    /// where a user restarting to fix things most needs to see the line.
    pub(crate) hook_warned: Arc<std::sync::atomic::AtomicBool>,
    /// What the status sampler last saw on this session's agent pane
    /// (PLAN_M6_75.md item 1). See [`ActivitySample`], and
    /// [`crate::service::ticker`] for the cadence that fills it.
    ///
    /// Lives on the ENTRY rather than in a supervisor-held map for one
    /// decisive reason: the consumer landing next is `status`'s
    /// classifier, and `session_status` is a pure function of an entry
    /// plus a pane map — no supervisor, no store, no I/O (see that
    /// module's own docs, which make that purity a contract). A map on
    /// `Supervisor` would have to be threaded into it, and the
    /// classification tests would stop being buildable from hand-made
    /// entries. The lifecycle argument agrees: a sample describes ONE
    /// launch of one session, so it wants exactly the relaunch-isolates,
    /// rename-shares behavior this struct's `Arc` cells already give,
    /// and it wants to disappear with the entry on a delete rather than
    /// leaving a map key for somebody to remember to evict.
    ///
    /// Not part of the `attachments`-then-`sessions` lock-ordering rule,
    /// for the same reason `outcome` is not: a leaf `std::sync::Mutex`
    /// held across no await and alongside no other lock.
    pub(crate) activity: Arc<std::sync::Mutex<ActivitySample>>,
    /// When the sampler last saw this session's pane CHANGE, in unix
    /// seconds — the durable half of the activity signal, mirroring
    /// `store::StoredSession::last_activity_at`.
    ///
    /// Separate from [`SessionEntry::activity`] on purpose, and the
    /// separation is the whole design rather than a layering accident.
    /// `ActivitySample` deliberately holds no clock — its own docs argue
    /// at length why a wall-clock window is the wrong shape for STATUS
    /// classification under a budgeted round robin — and that argument is
    /// about classification only. Sorting a list by recency is a different
    /// question, it genuinely wants a time, and answering it inside
    /// `ActivitySample` would put a timestamp exactly where the next
    /// reader would be tempted to compare it against `now` and reintroduce
    /// the population-dependent status bug. So the two live side by side:
    /// one counts observations, this one dates the last interesting one.
    ///
    /// An atomic rather than one more `Arc<Mutex<..>>` cell because there
    /// is nothing to keep consistent with anything else — it is a single
    /// `i64` whose only writer already decided, under the sample's own
    /// lock, that it should move. `Relaxed` ordering throughout for the
    /// same reason: nothing is published through it.
    ///
    /// `info.last_activity_at` carries the value this entry was BUILT
    /// with (creation, reload, or restart); this cell carries the current
    /// one, and `status::entry_info` overwrites the clone from here on
    /// every reply — the same shape `outcome` uses, and for the same
    /// reason: the entry itself is immutable behind its `Arc`.
    ///
    /// SESSION-SCOPED, not generation-scoped — the one mutable cell here
    /// that a relaunch shares instead of re-minting, and the distinction
    /// deserves to be read before anyone "fixes" it into line with its
    /// neighbours. The cells above describe one RUN, so
    /// [`relaunched_entry`] gives the new launch fresh ones and a late
    /// write from a pass still holding the old entry lands harmlessly in
    /// the abandoned run's copy. This one describes the SESSION: "the last
    /// time anything was seen happening here" does not become false
    /// because a new process is producing the output, and a restart that
    /// reset it would jump the session backwards in a recency sort at the
    /// exact moment a user acted on it.
    ///
    /// Fencing it would therefore not protect anything — it would open a
    /// hole. A sampler that read an entry, awaited, and then advanced its
    /// cell would advance the abandoned copy while its own durable write
    /// moved the row, leaving this process answering from a value older
    /// than its own database until the next observed change crossed the
    /// quantum. Sharing one cell per session across renames, archives, and
    /// relaunches alike makes every holder of any entry a writer to the
    /// same place, which is what the value's meaning already implies.
    /// Cross-process and clock-step monotonicity stay the store's job
    /// (`SessionStore::record_activity`).
    pub(crate) last_activity_at: Arc<std::sync::atomic::AtomicI64>,
    /// Which LAUNCH of this session this entry describes
    /// (`store::StoredSession::generation`).
    ///
    /// Immutable per entry, which is the point: a restart PUBLISHES A NEW
    /// ENTRY rather than mutating this one, so anything still holding the
    /// old `Arc` — a `ListSessions` pass that already cloned it, a capture
    /// pass mid-scan, an `Attach` that resolved before the restart — is
    /// holding, and can be recognized as holding, a description of the
    /// previous run. Every durable write those paths perform carries this
    /// value and is rejected by the store when it is no longer current
    /// (`SessionStore::transition_many` and the capture writers), and
    /// `Attach` compares it before installing an attachment on what may be
    /// a respawned pane.
    ///
    /// The fence is for state that DESCRIBES A RUN, which is most of this
    /// struct but not all of it. [`SessionEntry::last_activity_at`] is
    /// session-wide — the last time anything was seen happening here is
    /// equally true whichever launch produced the output — so it is shared
    /// across a relaunch rather than fenced, and
    /// `SessionStore::record_activity` deliberately takes no generation.
    /// Adding one later would not tighten anything; it would drop the very
    /// writes the value wants. Decide which of the two kinds a new field
    /// is before copying either pattern.
    pub(crate) generation: i64,
    /// The cgroup scope THIS generation launched into
    /// (`store::StoredSession::launch_scope`), or `None` for a launch that
    /// selected the portable sweep alone.
    ///
    /// Immutable per entry for exactly the reason `generation` is, and
    /// carried here rather than re-read from the store at stop time so the
    /// scope a kill aims at is the one belonging to the run whose liveness
    /// the caller just decided about — a row re-read mid-restart could
    /// already name the NEXT generation's unit, and signaling that would
    /// mean killing the launch that is replacing this one.
    pub(crate) scope: Option<String>,
}

/// One host's session authority, shared by every connection.
///
/// Sessions are keyed by session id; attachments by (session, terminal)
/// — separate maps because their lifetimes differ: a session outlives any
/// number of attach/detach cycles, and it may have several terminals
/// attached at once — each with its own channel and its own PAIR of
/// control-mode clients, one streaming output and one dedicated to input
/// (PLAN_M4.md item 3) — while SPEC.md caps the attached CLIENTS at one.
/// That cap lives in the `Attach` handler's lease check, not in the shape
/// of this map: see [`AttachmentKey`] and `terminals::same_lease_client`.
///
/// Lock discipline: the two mutexes are never held at once, with two
/// deliberate exceptions (`DeleteSession` and `snapshots`'
/// `publish_alt_screen_snapshot`), and no tmux call happens while `sessions` is held
/// on its own. ("The two" means `sessions` and `attachments` throughout
/// this discussion; `sinks` and `uploads` arrived later and neither is
/// ever held alongside either of the two — see their own docs — so they
/// add no case to the rules below.) `attachments` is deliberately the exception for holding it
/// across tmux calls — the whole attach takeover runs under it, because
/// that is the only way "at most one client, last attach wins" survives
/// two concurrent attaches — and the only way the lease sweep and the new
/// attachment's installation are one atomic step, which is what
/// `ControlMsg::Attach` promises (see the `Attach` handler) — and the
/// input and `Resize` arms hold it across their tmux calls for the same
/// reason: an ownership check that releases the lock before acting goes
/// stale the moment a takeover interleaves. `DeleteSession` is the second
/// holder of this same exception, for the same underlying reason: it
/// holds `attachments` across its own forwarder shutdown and tmux calls so a
/// concurrent `Attach` cannot install a fresh attachment on a session
/// that is disappearing out from under it — but deliberately NOT across
/// its process-tree sweep, which can take seconds and would otherwise
/// stall every other session's attach/input behind one slow delete (see
/// that handler's own comment). `DeleteSession` is also ONE of two paths
/// that ever hold both mutexes at once, briefly: it removes the
/// in-memory map entry while `attachments` is still held for the notify
/// decision, and `publish_alt_screen_snapshot` (`snapshots`) does the same
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
    pub(crate) state_dir: PathBuf,
    pub(crate) tmux: TmuxDriver,
    /// Session metadata's durable half — see `crate::store` and the
    /// `service` module docs (`mod.rs`) for the split of truth this
    /// implements.
    pub(crate) store: SessionStore,
    /// This supervisor's identity (PLAN_M6.md item 2): a UUIDv4 resolved
    /// once at construction and held for the process lifetime — the field
    /// is a plain value, never re-resolved later (not a cell), because the
    /// identity itself is immutable for the life of the install (SPEC.md),
    /// and a mutable slot here would misleadingly suggest otherwise.
    /// Carried in every hello (`connection::handle_connection`); see that
    /// field's own docs on `ControlMsg::Hello` for what a peer does with
    /// it.
    ///
    /// `Option` — NOT `String` — because minting (`SessionStore::
    /// ensure_host_identity`, a durable write) is only ever performed by a
    /// construction that holds the state directory's exclusivity
    /// (`StateDirOwnership`); a claimless construction (a losing racer in a
    /// handoff, or a corrupted-lock edge case — see `StateDirOwnership::
    /// claim`'s docs) has no standing to write and instead only READS
    /// whatever identity already exists (`SessionStore::
    /// read_host_identity`). On a genuinely fresh install raced by two
    /// processes, the loser can therefore legitimately see `None` here —
    /// there is nothing durable yet for it to read, and it must not be the
    /// one to create it. `None` propagates as `Hello::host_identity: None`,
    /// the same shape every identity-less test double and the helm's own
    /// hello already send, so a claimless supervisor's peers see an honest
    /// "no identity to report yet" rather than a value this process was
    /// never entitled to produce.
    pub(crate) host_identity: Option<String>,
    pub(crate) sessions: Mutex<HashMap<String, Arc<SessionEntry>>>,
    /// Every live attachment in this supervisor, keyed per (session,
    /// terminal) — see [`AttachmentKey`].
    ///
    /// The lock is what makes the session-scoped takeover ATOMIC: an
    /// attach holds it across both the lease sweep and its own
    /// installation, so no observer can ever see a session whose
    /// terminals are split between two leases (`ControlMsg::Attach`'s
    /// contract). Every other holder — input, resize, pause, delete,
    /// stall, connection teardown — inherits the same guarantee for free,
    /// because a takeover cannot interleave inside any of their
    /// check-then-act pairs.
    pub(crate) attachments: Mutex<HashMap<AttachmentKey, ActiveAttach>>,
    /// The upward request channel of every full-authority connection
    /// currently served — see [`super::agent_relay`] for what travels up
    /// one and why anything does.
    ///
    /// A `Vec`, not a map: it is indexed by nothing (a link is found by
    /// matching an attachment's writer queue against it), it holds one
    /// entry per connected helm, and that is a number a person could count
    /// on one hand. A key would have to be invented purely to have one.
    ///
    /// NOT part of the `attachments`-then-`sessions` lock-ordering rule
    /// this struct's docs state: `helm_link_for_session` releases
    /// `attachments` before taking this, and nothing else takes both.
    pub(crate) helm_links: Mutex<Vec<Arc<super::agent_relay::HelmLink>>>,
    /// Per-terminal barriers for provisional opens and unfinished shutdowns.
    ///
    /// Teardown publishes a `Reaping` entry while it still holds
    /// `attachments`, before allowing any attach to observe the old map entry
    /// as absent. The retained receiver is therefore the per-terminal handoff
    /// boundary: a replacement waits outside the global async mutex, while a
    /// lost reaper leaves `Failed` behind until supervisor restart.
    pub(crate) output_reaps: OutputReapRegistry,
    /// Each tmux session's sink lifecycle and handoff state, keyed by tmux
    /// session name (PLAN_M4.md order-of-work step 5, and
    /// [`super::terminals::SessionSinkHandle`] for what a live sink is).
    ///
    /// A live entry is `Weak`, deliberately: attachments own the sink. A
    /// final release replaces that weak entry with a per-session `Reaping`
    /// state before starting process teardown. A failed reap leaves `Failed`
    /// behind even after every attachment is gone; it is proof that opening
    /// another same-session client would be unsafe, not stale map data.
    /// Candidate-operation barriers sit alongside that registered state: they
    /// are published atomically when a missing sink is reserved for opening
    /// and remain until the open completes without an installable handle or
    /// the resulting installed/competing client is resolved. A runtime task
    /// owns the in-flight open. On success, the request receives a guard and
    /// completes the barrier by installing it; abandoning that guard transfers
    /// both cleanup and completion to its runtime-owned drop reaper. Request
    /// cancellation therefore cannot erase either barrier.
    ///
    /// Keyed by TMUX session name rather than farhelm session id because
    /// the name is what a client attaches to, and because it makes the
    /// restart path fall out correctly: a restart that rebuilds the tmux
    /// session under the same name kills the sink attached to the old one,
    /// and the supervising task reattaches to the new one by name.
    ///
    /// No process operation runs under this lock. Teardown only installs a
    /// reaping state; the detached reaper does the slow work afterward.
    pub(super) sinks: SinkRegistry,
    /// Every attachment upload currently in flight in this supervisor,
    /// keyed by the transfer id minted at `BeginUpload` (PLAN_M4.md item
    /// 4).
    ///
    /// A REGISTRY for one purpose: `DeleteSession` has to find the
    /// transfers belonging to a session it is about to erase, and a
    /// transfer is otherwise reachable only through the connection-local
    /// channel routing that admitted it. Nothing else consults this map —
    /// chunks, commits, and client aborts all arrive on the connection
    /// that owns them and go straight down that route.
    ///
    /// Keyed by a supervisor-minted id rather than by (session, channel):
    /// channel ids are unique only within a connection, and one session
    /// can legitimately have several uploads in flight from several
    /// connections at once. The id doubles as the transfer identifier in
    /// the diagnostic trail SPEC.md's logging section requires, so an
    /// operator reading begin/publish/abort events can follow one transfer
    /// through them.
    ///
    /// LOCK ORDER: never held alongside `sessions`, `attachments`, or
    /// `sinks`. A lifecycle claim may be held while taking it (delete does
    /// exactly that), never the reverse.
    pub(crate) uploads: Mutex<HashMap<u64, UploadHandle>>,
    /// Source of the transfer ids above: monotonic within this process,
    /// which is all an id needs to be — uploads are in-flight state, never
    /// durable, so nothing outlives the process that could collide with a
    /// restarted one's numbering.
    pub(crate) next_transfer: AtomicU64,
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
    pub(crate) pending_snapshots: Mutex<HashMap<String, Vec<u8>>>,
    /// This binary's own path: the launch shim is a subcommand of it.
    farhelm_exe: PathBuf,
    /// [`Self::farhelm_exe`] as a `str`, or `None` when that path is not
    /// UTF-8 — resolved ONCE at construction rather than per launch.
    ///
    /// It exists because of what the conversation hook has to do with the
    /// path: every [`crate::agent_kind::AgentIntegration::hook_argv`]
    /// implementation embeds it in a vendor's own quoting syntax through
    /// `shell_words::quote`, which takes `&str` and nothing else. Doing
    /// the fallible `Path`-to-`str` conversion here means the failure has
    /// exactly one home, and the launch paths get to treat "hookable" as a
    /// plain `Option`. Fallible rather than lossy is the whole reason this
    /// field is an `Option`: `Path::to_str` refuses a non-UTF-8 path
    /// outright instead of substituting replacement characters, so there
    /// is no mangled-but-plausible path to accidentally hand a vendor.
    ///
    /// `None` is a real, if vanishing, state and not a should-never-happen:
    /// a farhelm installed under a non-UTF-8 path simply never has hook
    /// flags injected — [`Supervisor::with_hook_argv`] logs the skip and
    /// every session falls back to the record scan. Nothing else about the
    /// launch changes, because the shim itself is addressed by `PathBuf`
    /// (see [`crate::launch::window_command`]) and has never needed the
    /// path to be text.
    farhelm_exe_str: Option<String>,
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
    pub(crate) admission: Arc<tokio::sync::Semaphore>,
    /// The periodic ticker's OWN bound, deliberately disjoint from
    /// `admission` (PLAN_M6_75.md item 1's review found the shared version
    /// to be a SPEC violation).
    ///
    /// Sharing the request semaphore looked right — the ticker consumes
    /// the same tmux subprocesses the handlers do — and was wrong for a
    /// reason that has nothing to do with tmux: a permit taken by periodic
    /// work is a permit a REQUEST cannot have. With seven slow handlers in
    /// flight the ticker would take the eighth, the next control request
    /// would park inside `handle_control`, and `handle_connection`'s read
    /// loop — the same loop that dispatches keystrokes — would stop
    /// draining. SPEC.md's status rule is absolute that status detection
    /// must never gate or delay interaction with the terminal, so no
    /// sampling policy may be able to reach the input path at all, however
    /// indirectly. A separate semaphore makes that structural rather than
    /// a matter of tuning the permit count.
    ///
    /// One permit, because there is exactly one ticker and its passes are
    /// sequential: what this actually bounds is a future second caller,
    /// and what it BUYS today is a limiter a test can exhaust in order to
    /// prove that request admission is unaffected by a wedged sampler.
    /// Total tmux concurrency therefore becomes one more than
    /// `HANDLER_ADMISSION_PERMITS` rather than exactly it, which is a
    /// rounding error next to the failure mode it removes.
    pub(crate) sampling_admission: Arc<tokio::sync::Semaphore>,
    /// Per-supervisor state purely so integration tests can shorten
    /// these; see [`SupervisorTimeouts`].
    pub(crate) timeouts: SupervisorTimeouts,
    /// Injection points; production builds carry the defaults. Held for
    /// the process lifetime because `serve` reloads sessions again and
    /// must consult the same boot-id source the constructor did — a second
    /// reload that read a different source would classify the same host
    /// two ways.
    pub(crate) seams: SupervisorSeams,
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
    pub(crate) may_record: std::sync::atomic::AtomicBool,
    /// Collapses concurrent creates that share an intent key into one
    /// launch; see [`KeyedLocks`] for why an in-process lock is the whole
    /// mechanism and what it is (and is not) responsible for.
    intent_locks: Arc<KeyedLocks>,
    /// One claim per SESSION, held for the whole of any operation that
    /// changes what is running under it: restart, stop, delete, and — since
    /// PLAN_M4.md item 2 — opening and closing a terminal tab. Rename
    /// (PLAN_M5.md item 3) joins them for a related but distinct reason: it
    /// runs nothing, but it is the one operation that changes a session's
    /// stored metadata, and the claim is what keeps its
    /// durable-then-in-memory write from interleaving with an operation
    /// that republishes or removes the same entry (see
    /// [`Supervisor::rename_session`]).
    ///
    /// A CREATE takes one too, but only on its retry-takeover path and only
    /// for the two steps that reclaim an existing session's identities: the
    /// takeover transaction and the map removal that mirrors it. That span
    /// is the one window in a create where a rename can interleave —
    /// everywhere else the session is not on the map yet, or no longer is,
    /// and rename answers `NotFound` like stop and delete do. The claim is
    /// released well before the launch's tmux work, so a create never holds
    /// one across a subprocess (see `Supervisor::launch_reserved`).
    ///
    /// The map-wide `sessions` mutex cannot do this job — it is released
    /// the moment an entry is cloned out of it, and every one of these
    /// operations then spends seconds in tmux and `/proc` with nothing
    /// holding the session still. What that permits is not merely untidy:
    /// two concurrent restarts each recheck liveness before either has
    /// stopped anything, both conclude "nothing is running", and the
    /// second's marker-keyed kill sweep then reaps the agent the first one
    /// just launched — a kill nobody consented to, arrived at entirely
    /// through legal steps. Stop-vs-restart is the same shape with the
    /// sweep on the other side, and delete-vs-restart resolves to a
    /// half-torn-down session rather than one honest winner.
    ///
    /// LOCK ORDER: lifecycle first, then `attachments`, then `sessions`
    /// (see this struct's lock-discipline docs). Nothing acquires a
    /// lifecycle claim while holding either of the other two.
    pub(crate) lifecycle_locks: Arc<KeyedLocks>,
    /// The home directory the agents' own record trees hang off
    /// (PLAN_M3.md item 8), resolved once at construction from
    /// `SupervisorSeams::agent_home` or `$HOME`.
    ///
    /// `None` disables conversation capture entirely, and that is the
    /// honest behavior rather than a degraded one: with no home there is
    /// no directory to observe, so every integrated session simply stays
    /// uncaptured and takes SPEC.md's fresh-launch fallback. Resolved ONCE
    /// so a supervisor cannot start capturing from a different tree
    /// mid-life, which would let one session's identity be claimed from a
    /// directory a later pass no longer looks at.
    pub(super) agent_home: Option<PathBuf>,
    /// The home directory `~`-prefixed create cwds expand against,
    /// resolved once at construction from [`SupervisorSeams::user_home`]
    /// or `$HOME` (see [`expand_tilde_cwd`] for the contract). Resolved
    /// once for the same reason `agent_home` is: what a `~` means must not
    /// shift mid-life with the daemon's environment.
    user_home: Option<PathBuf>,
    /// See [`SupervisorSeams::capture_window`].
    pub(super) capture_window: CaptureWindowBounds,
    /// Serializes and schedules conversation-capture passes (PLAN_M3.md
    /// item 8, rescheduled by PLAN_M6_75.md item 1). See
    /// [`CaptureCoordination`].
    pub(super) capture: CaptureCoordination,
}

/// The refusal every lifecycle verb returns for an UNRECOGNIZED foreign
/// pane owner — delete, archive, stop, restart, and close-tab all say this
/// same sentence, which is why it is written once.
///
/// It has to carry both halves an operator needs to act: which tmux
/// session actually holds the pane, and why an unrecognized name is what
/// stops the verb rather than merely being logged. The wording keeps the
/// substance of the `bail!` this classification replaced
/// ([`crate::tmux::PaneProbe`]), because that text is what the 2026-08-16
/// incident's journal entries and any search for it will match — the
/// change is that it now appears only for the case that still deserves it.
/// [`Supervisor::known_session_tmux_name`] holds the reasoning for the
/// split.
pub(crate) fn unknown_pane_owner_refusal(pane: &str, owner: &str, expected: &str) -> String {
    format!(
        "pane {pane} belongs to session {owner:?}, not {expected:?}, and {owner:?} is not a \
         session this supervisor knows; refusing to treat it as this session's terminal — the \
         recorded pane may be this session's own live terminal, renamed or moved out from under \
         us, and acting on that guess would touch an agent that is still running"
    )
}

/// Everything `farhelm supervisor run` resolves ONCE at process startup
/// and hands to the supervisor it is about to build.
///
/// A struct rather than a growing parameter list, and every field is
/// REQUIRED — there is deliberately no `Default` and no `..rest` escape.
/// Each of these is an operator override read from the CLI or the
/// environment exactly once, at exactly one place, and a caller allowed to
/// omit one is a launch path that silently ignores it. Spelling them out
/// is what keeps "resolved once, honoured everywhere" checkable by the
/// compiler instead of by review.
///
/// The values themselves land in [`SupervisorSeams`], which is where every
/// launch path reads them from; this type exists only to carry them across
/// the one boundary between the CLI arm and the constructor. See that
/// struct's `tmux_program` and `agent_hooks` fields for what each means and
/// for the environment-hygiene rule they both obey.
pub struct SupervisorStartup {
    /// The resolved `--tmux` / `FARHELM_TMUX` override (see
    /// [`crate::tmux::resolve_tmux_program_from_env`]). Callers with no
    /// opinion pass [`crate::tmux::DEFAULT_TMUX_PROGRAM`].
    pub tmux_program: PathBuf,
    /// The parsed `FARHELM_AGENT_HOOKS` opt-out (see
    /// [`crate::agent_kind::parse_agent_hooks`]). Callers with no opinion
    /// pass [`crate::agent_kind::AgentHooks::default()`], which hooks every
    /// integrated kind.
    pub agent_hooks: crate::agent_kind::AgentHooks,
}

impl Supervisor {
    /// Production constructor: the launch shim is this very process's
    /// binary. Test harnesses must NOT use this — their `current_exe` is
    /// the libtest runner, not farhelm — and use `new_with_exe` instead.
    pub async fn new(state_dir: &Path) -> anyhow::Result<Arc<Supervisor>> {
        Self::new_for_startup(
            state_dir,
            SupervisorStartup {
                tmux_program: PathBuf::from(crate::tmux::DEFAULT_TMUX_PROGRAM),
                agent_hooks: crate::agent_kind::AgentHooks::default(),
            },
        )
        .await
    }

    /// [`Self::new`] with the process-startup overrides named explicitly —
    /// see [`SupervisorStartup`].
    ///
    /// Its own constructor rather than parameters on [`Self::new`] because
    /// the overrides have exactly one caller — `farhelm supervisor run`,
    /// which resolves them once at startup — while everything else wants
    /// the plain defaults.
    pub async fn new_for_startup(
        state_dir: &Path,
        startup: SupervisorStartup,
    ) -> anyhow::Result<Arc<Supervisor>> {
        let SupervisorStartup {
            tmux_program,
            agent_hooks,
        } = startup;
        let exe = std::env::current_exe().context("resolving own binary path")?;
        Self::new_with_seams(
            state_dir,
            exe,
            SupervisorTimeouts::default(),
            SupervisorSeams {
                tmux_program,
                agent_hooks,
                ..SupervisorSeams::default()
            },
        )
        .await
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
        let farhelm_exe = if farhelm_exe.is_absolute() {
            farhelm_exe
        } else {
            std::env::current_dir()
                .context("reading the supervisor's working directory")?
                .join(farhelm_exe)
        };
        // Derived from the ABSOLUTE spelling above, never from the
        // caller's, so the string the hook flags carry is the same path
        // the shim is launched through. See `Supervisor::farhelm_exe_str`
        // for why the conversion is done once here instead of at each
        // launch, and why losing it is a degradation rather than an error.
        let farhelm_exe_str = farhelm_exe.to_str().map(str::to_string);
        if farhelm_exe_str.is_none() {
            warn!(
                exe = %farhelm_exe.display(),
                "this farhelm executable's path is not valid UTF-8, so no launch can carry \
                 conversation hook flags; every session falls back to the record scan"
            );
        }
        // Store one absolute spelling after creation. Every injected
        // socket path derives from this value, so a supervisor started
        // with a relative `--state-dir` cannot hand a tab or agent a path
        // that resolves against that process's unrelated working directory.
        let state_dir = tokio::fs::canonicalize(state_dir)
            .await
            .context("resolving the supervisor state directory")?;
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
        tokio::fs::File::open(&state_dir)
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
        let (ownership, lock_newly_acquired) = match StateDirOwnership::claim(&state_dir)? {
            Some((ownership, newly_acquired)) => (Some(ownership), newly_acquired),
            None => (None, false),
        };
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
        // Gated on `ownership`, unlike the schema migration above sharing
        // the same gate for the same reason: minting a host identity is a
        // durable WRITE, and a claimless process (one that lost the
        // exclusivity race — see `StateDirOwnership`) has no standing to
        // perform one, exactly as it has none to migrate the schema or
        // reconcile session outcomes. An earlier build called
        // `ensure_host_identity` here unconditionally, reasoning that the
        // conditional upsert made it "safe" for a claimless caller too —
        // true for CORRECTNESS (no two processes converge on different
        // identities), but beside the point: a losing racer must perform
        // no durable write at all, whether or not that write is safe to
        // race. A claimless process instead only READS whatever identity
        // already exists (`SessionStore::read_host_identity`), which is
        // `None` on a genuinely fresh install nobody has minted for yet —
        // see `Supervisor::host_identity`'s own docs for what that `None`
        // means and where it propagates.
        let host_identity = if ownership.is_some() {
            Some(store.ensure_host_identity().await?)
        } else {
            store.read_host_identity().await?
        };
        // Threaded from `timeouts` rather than `TmuxDriver::new`'s
        // production default so an integration test's loosened budgets
        // (see `SupervisorTimeouts::tmux_exchange`) actually reach the
        // driver every attach and send-keys call goes through. The tmux
        // binary rides along for the same reason at the other end of the
        // scale: this is the only production driver a supervisor ever
        // builds, so it is the one place the `--tmux` / `FARHELM_TMUX`
        // override has to land for every launch path to honor it.
        let tmux = TmuxDriver::new_with_program(
            &state_dir,
            crate::tmux::TmuxBudgets {
                exchange: timeouts.tmux_exchange,
                pane_list: timeouts.tmux_pane_list,
            },
            seams.tmux_program.clone(),
        );
        tmux.ensure_server().await?;
        // Anything attached to the private server at this instant
        // predates this supervisor, and a control client whose owner
        // died with output in flight can be wedged in tmux's
        // deferred-exit trap forever — see `reap_stale_control_clients`
        // for the whole story. Reaped before any session work so a
        // stale sink cannot sit alongside (or shadow) the fresh one
        // this process is about to bring up. Gated on the lock being
        // NEWLY acquired, which is what makes "everything attached is a
        // dead predecessor's" true: a claimless constructor lost the
        // race to a live owner in another process, and a reused claim
        // means a live incumbent in THIS process — either way the
        // attached clients are someone's healthy, working clients.
        if lock_newly_acquired {
            let reaped = tmux.reap_stale_control_clients().await?;
            if reaped > 0 {
                info!(
                    reaped,
                    "reaped stale control clients left by a dead predecessor"
                );
            }
        }

        // Resolved before the first reload, because that reload already
        // runs a capture pass: a session whose first input landed before a
        // restart must be able to capture on the way back up, not only on
        // the first list afterwards.
        let agent_home = seams
            .agent_home
            .clone()
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
            .filter(|home| !home.as_os_str().is_empty());
        // Same resolution shape as `agent_home`, but no warning on absence:
        // a missing home only matters if a `~` create ever arrives, and
        // that request gets its own clear refusal.
        let user_home = seams
            .user_home
            .clone()
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
            .filter(|home| !home.as_os_str().is_empty());
        if agent_home.is_none() {
            warn!(
                "no HOME for this supervisor, so no agent record directory can be located; \
                 conversation-identity capture is disabled and restart will offer a fresh \
                 launch for every session"
            );
        }
        let capture_window = seams.capture_window;
        let (sessions, may_record) =
            Self::reload_sessions(&state_dir, &store, &tmux, &seams, ownership.is_some()).await?;

        let supervisor = Arc::new(Supervisor {
            state_dir,
            tmux,
            store,
            host_identity,
            sessions: Mutex::new(sessions),
            attachments: Mutex::new(HashMap::new()),
            helm_links: Mutex::new(Vec::new()),
            output_reaps: Arc::new(std::sync::Mutex::new(HashMap::new())),
            sinks: Arc::new(std::sync::Mutex::new(Default::default())),
            uploads: Mutex::new(HashMap::new()),
            next_transfer: AtomicU64::new(1),
            pending_snapshots: Mutex::new(HashMap::new()),
            farhelm_exe,
            farhelm_exe_str,
            admission: Arc::new(tokio::sync::Semaphore::new(HANDLER_ADMISSION_PERMITS)),
            sampling_admission: Arc::new(tokio::sync::Semaphore::new(SAMPLING_ADMISSION_PERMITS)),
            timeouts,
            seams,
            ownership,
            may_record: std::sync::atomic::AtomicBool::new(may_record),
            intent_locks: Arc::new(KeyedLocks::default()),
            lifecycle_locks: Arc::new(KeyedLocks::default()),
            agent_home,
            user_home,
            capture_window,
            capture: CaptureCoordination {
                lock: Mutex::new(()),
                history: std::sync::Mutex::new(CaptureHistory::default()),
            },
        });
        // Capture runs on the reload passes as well as the list path
        // (PLAN_M3.md item 8), and not merely for symmetry: a session whose
        // agent wrote its record while this supervisor was DOWN has no
        // other moment to be noticed, and a session already holding an
        // identity gets its record re-verified before anything is served
        // from it. It runs HERE rather than inside `reload_sessions`
        // because the pass needs the finished supervisor — its seams, its
        // store, and its capture coordination. It is a `Reply`-shaped
        // pass: nothing has swept yet, so it simply runs. (Note for
        // anyone installing a `capture_gate` in a test: THIS is the pass
        // it will meet first.)
        supervisor.capture_now().await;
        Ok(supervisor)
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
                        match read_launch_sentinel(state_dir, &row.id, row.generation).await {
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
                for (id, generation) in rows
                    .iter()
                    .filter(|row| sentinel_overrides.contains_key(&row.id))
                    .map(|row| (row.id.clone(), row.generation))
                    .collect::<Vec<_>>()
                {
                    cleanup_launch_artifacts(state_dir, &id, generation).await;
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
                cleanup_launch_artifacts(state_dir, &row.id, row.generation).await;
                continue;
            }

            // Archive is durable evidence that this session intentionally
            // has no terminal. Do not rediscover a same-named tmux husk as
            // its agent: restart is the only operation allowed to clear the
            // flag and create a new terminal generation.
            if row.archived {
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
                // Marker-led, not positional (PLAN_M4.md item 2): with
                // tabs on the same tmux session, "the lowest pane id"
                // could hand this row a TAB's pane — after which stop
                // would reap that tab and restart would respawn into it.
                // See `agent_pane_from_states` for the preference ladder
                // and the legacy fallback it keeps for sessions created
                // before markers existed.
                agent_pane_from_states(&pane_states, &row.tmux_name, &row.id)
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
                match read_launch_sentinel(state_dir, &row.id, row.generation).await {
                    Ok(Some(detail)) => {
                        transitions.push((
                            row.id.clone(),
                            row.generation,
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
                    Ok(None) => {
                        // No sentinel, and a pane that is present and
                        // DEAD: the shape a launch that never reached the
                        // shim leaves behind.
                        let pane_dead = found.as_ref().is_some_and(|(_, state)| state.dead);
                        if let Some(detail) = wrapper_failure_detail(
                            state_dir,
                            &row.id,
                            row.generation,
                            row.launch_scoped,
                            pane_dead,
                        )
                        .await
                        {
                            transitions.push((
                                row.id.clone(),
                                row.generation,
                                Transition::SentinelError {
                                    detail: detail.clone(),
                                    pane: found.as_ref().map(|(pane, _)| pane.clone()),
                                },
                            ));
                            sentinel_hits.insert(row.id.clone(), detail);
                            if let Some((pane, state)) = found {
                                found_panes.insert(row.id.clone(), (pane, state));
                            }
                            continue;
                        }
                    }
                    Err(e) => {
                        // Loud propagation, not fall-through (item 1): this
                        // row's reconciliation is DEFERRED for this pass —
                        // no `Transition` is proposed for it at all, so a
                        // durable misclassification can never be committed
                        // from unreliable evidence — while the file
                        // survives for a later, repaired pass to read. Its
                        // pane still rides into `found_panes` so a
                        // genuinely alive session keeps reporting a live status
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
                    transitions.push((
                        row.id.clone(),
                        row.generation,
                        Transition::ObservedExit { exit_code: None },
                    ));
                }
                continue;
            };
            if !row.outcome.is_terminal() {
                if state.dead {
                    // A dead pane found BY NAME (the row has none of its
                    // own) is only this launch's evidence when this launch
                    // is the session's FIRST: generation 0 means the pane
                    // can only have come from the create that crashed
                    // before confirming it. On a later generation the same
                    // shape means a restart crashed between opening its
                    // generation and respawning, so the pane still there is
                    // the PREVIOUS run's — recording its exit against this
                    // generation would attribute a death to a launch that
                    // never happened. That row stays `Launching` (listing
                    // as `Unknown`) for a retried restart to resolve, which
                    // is the same "no side effects found is not evidence
                    // the agent ran" rule the no-pane branch above applies.
                    let stale_pane = row.pane.is_empty() && row.generation > 0;
                    if !stale_pane {
                        // The pane outlived its process (remain-on-exit)
                        // and still holds the code — "exited with the code
                        // the surviving dead pane retains". A rediscovered
                        // pane rides the same commit as the outcome it
                        // evidences, so no crash window can leave the pane
                        // recorded under a still-`Running` row.
                        transitions.push((
                            row.id.clone(),
                            row.generation,
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
                    }
                } else if row.outcome != LastOutcome::Running || row.pane != pane {
                    // Live, and either not yet confirmed, confirmed
                    // against a different pane, or carrying a stop intent
                    // whose kill sweep evidently never landed — the last
                    // being the reconciliation that keeps a crashed stop
                    // from annotating a session that is still running. A
                    // LIVE pane found by name after a crashed relaunch is
                    // this generation's after all (the respawn landed
                    // before the crash), which is why the staleness rule
                    // above covers only the dead case.
                    transitions.push((
                        row.id.clone(),
                        row.generation,
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
        // reader while leaving credential-bearing debris behind. (A
        // relaunch can no longer collide with them: launch files are named
        // per GENERATION — `launch::spec_path_for_launch` — so a later
        // launch's paths are simply different ones.) Folded into this same
        // successful arm (item 7) rather than a separate loop afterward:
        // a failed write means nothing durable exists yet, so the files
        // must survive for the next pass to retry against — cleaning them
        // up then would silently convert a real, still-unrecorded failure
        // into "no sentinel found" for good.
        let committed = if may_write && !transitions.is_empty() {
            match store.transition_many(transitions).await {
                Ok(committed) => {
                    for (id, generation) in rows
                        .iter()
                        .filter(|row| sentinel_hits.contains_key(&row.id))
                        .map(|row| (row.id.clone(), row.generation))
                        .collect::<Vec<_>>()
                    {
                        if matches!(committed.get(&id), Some(LastOutcome::Error { .. })) {
                            cleanup_launch_artifacts(state_dir, &id, generation).await;
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
            // Rebuilt from the stored columns rather than re-derived from
            // the invocation: item 7's snapshot is recorded once at create
            // and never re-guessed (`crate::agent_kind`), which is exactly
            // what makes a session's kind survive a supervisor upgrade
            // whose derivation heuristic has changed.
            let snapshot = IntegrationSnapshot {
                kind: row.agent_kind,
                resume_template: row.resume_template,
            };
            // The durable verdict comes back as a CLAIM, not as progress
            // toward one: an ambiguity dominates (and survives precisely so
            // a restart cannot re-decide it on thinner evidence), and a
            // captured identity is re-verified against the record its
            // locator hint names rather than re-derived from the directory
            // — so an append confirms it and a fork's new id never
            // displaces it.
            //
            // `conversation_source` is what separates the two identity
            // cases sharing the `captured_conversation` column: `'hook'`
            // means the agent reported it from inside its own process, so
            // the session comes back as `Reported` and this supervisor
            // neither scans nor re-verifies for it. Anything else means the
            // scan concluded it.
            //
            // Testing ambiguity FIRST is only correct because the store
            // guarantees `capture_ambiguous = 1` and
            // `conversation_source = 'hook'` never coexist on a row:
            // `record_reported_conversation` clears the ambiguity flag as
            // it writes, and `record_capture_ambiguous` is fenced on
            // `conversation_source IS NULL` so it cannot set the flag on a
            // reported row. Were that pair ever allowed to coexist, this
            // branch order would silently downgrade an exact answer to a
            // refusal across every restart.
            let capture = if row.capture_ambiguous {
                CaptureState::Ambiguous { durable: true }
            } else {
                match (
                    row.captured_conversation,
                    row.conversation_source.as_deref(),
                ) {
                    (Some(conversation), Some("hook")) => CaptureState::Reported { conversation },
                    (Some(conversation), _) => CaptureState::Captured {
                        conversation,
                        record: row.captured_record.map(PathBuf::from).unwrap_or_default(),
                        stamp: RecordStamp {
                            len: 0,
                            mtime_unix: None,
                        },
                    },
                    (None, _) => CaptureState::Unclaimed,
                }
            };
            let restart_offer = snapshot.restart_offer(capture.committed_conversation());
            // Derived before `row.id` is moved into the entry's `info`.
            let scope = launch_scope_unit(&row.id, row.generation, row.launch_scoped);
            sessions.insert(
                row.id.clone(),
                Arc::new(SessionEntry {
                    info: SessionInfo {
                        parent: row.parent,
                        archived: row.archived,
                        id: row.id,
                        title: row.title,
                        created_at: row.created_at,
                        // Restored, not re-minted: see the entry's own
                        // `last_activity_at` cell below for why this one
                        // durable value survives a restart when the
                        // sampler's in-memory state deliberately does not.
                        last_activity_at: row.last_activity_at,
                        creation_seq: Some(row.creation_seq),
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
                        // Computed honestly here from the snapshot plus
                        // whatever identity was stored, but — like
                        // `status` — recomputed by `ListSessions` on
                        // every reply (`session_restart_offer`): capture
                        // can upgrade a session from `FreshOnly` to
                        // `Resume` at any moment, and a value frozen at
                        // reload would go stale the first time it did.
                        restart_offer,
                        // Vocabulary only for now (PLAN_M4.md step 4 gives
                        // tabs real rediscovery from tmux); until then this
                        // is the honest "none known" value every reload
                        // reports.
                        tabs: Vec::new(),
                        // The stored snapshot, with a PLACEHOLDER existence
                        // — like `status` and `restart_offer` above, and
                        // for the same reason: existence is a statement
                        // about the catalog at REPLY time, and `entry_info`
                        // re-derives it on every reply that carries this
                        // entry. Nothing reads the value parked here.
                        source_profile: row.source_profile.map(|profile| SourceProfile {
                            id: profile.id,
                            name: profile.name,
                            existence: ProfileExistence::Present,
                        }),
                    },
                    terminal,
                    outcome: Arc::new(std::sync::Mutex::new(outcome)),
                    snapshot,
                    canonical_cwd: row.canonical_cwd,
                    first_input: Arc::new(std::sync::Mutex::new(FirstInput {
                        at: row.first_input_at,
                        // Loaded FROM the database, so by definition
                        // already there.
                        durable: row.first_input_at.is_some(),
                    })),
                    capture: Arc::new(std::sync::Mutex::new(capture)),
                    // Clear on a reload, whatever the previous supervisor
                    // did: `hooked` records that THIS process appended the
                    // hook flags when it spawned the agent, and this
                    // process spawned nothing. Leaving the tripwire armed
                    // from a stored guess would warn about a launch nobody
                    // here can account for.
                    hooked: hook_flag(false),
                    hook_warned: hook_flag(false),
                    // Activity samples are process-local and deliberately
                    // not durable: a reloaded session has been observed by
                    // nobody in THIS process, and inventing a STATUS for
                    // it from a stored screen would claim knowledge of
                    // what happened while the supervisor was down.
                    activity: ActivitySample::unsampled(),
                    // The timestamp beside it IS restored, and the two are
                    // not in tension. What the sampler holds is a claim
                    // about now — this pane looks like this, it has been
                    // quiet this many looks — which nothing in SQLite can
                    // still vouch for after a restart. What this holds is
                    // a claim about a past instant, which stays true
                    // however long the supervisor was down; re-minting it
                    // to "now" would be the actual lie, and dropping it to
                    // zero would tell a recency sort that a session with
                    // years of history has never done anything.
                    last_activity_at: activity_stamp(row.last_activity_at),
                    generation: row.generation,
                    // The SELECTION comes straight back out of the row
                    // rather than from this supervisor's own probe: the
                    // launch that made it may have run under a DIFFERENT
                    // supervisor, on a host whose manager has since changed
                    // — and stop must aim at what was actually created, not
                    // at what would be created now (PLAN_M3.md item 10's
                    // reload interplay). The NAME is derived here, never
                    // stored, so the row cannot name somebody else's unit.
                    scope,
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
        // The cgroup is a FOURTH source of provenance, asked here rather than
        // in the row loop above because it costs a D-Bus round trip per
        // question and only pending reservations have a question worth
        // asking: a live scope with the reserved launch's name can only have
        // been created inside that launch's own tmux window, and it is the
        // only evidence that survives the pane, the sentinel, and the tmux
        // session all being gone while a daemon the launch spawned runs on.
        // Failing to ask (an unreachable manager) leaves the reservation
        // pending, which is the same answer every other unresolved source
        // produces.
        if may_write {
            match store.pending_reservations().await {
                Ok(pending) => {
                    let mut settled: Vec<Settlement> = Vec::new();
                    for reservation in pending {
                        let mut evidence = launched.contains(&reservation.session_id);
                        if !evidence
                            && seams.scopes.available().await
                            && let Some(unit) = crate::scope::unit_name(&reservation.session_id, 0)
                        {
                            evidence = seams.scopes.exists(&unit).await.unwrap_or(false);
                        }
                        if evidence {
                            settled.push(Settlement {
                                intent_key: reservation.intent_key,
                                session_id: reservation.session_id,
                                outcome: ReservationOutcome::Created,
                            });
                        }
                    }
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

    /// One session's DURABLE integration snapshot and captured
    /// conversation identity, read straight from the store.
    ///
    /// Public because it is the seam PLAN_M3.md item 9's restart is built
    /// on — the resume it runs is exactly [`SessionSnapshot::resume_argv`]
    /// — and because the capture tests assert against it. Reading through
    /// the store rather than the in-memory map is deliberate on both
    /// counts: item 9 must resume from what SURVIVED, and a test that
    /// asserted against the mirror would pass even if nothing had ever been
    /// persisted, which is precisely the property under test.
    ///
    /// `None` means no such session.
    pub async fn session_snapshot(&self, id: &str) -> anyhow::Result<Option<SessionSnapshot>> {
        let Some(row) = self.store.session(id).await? else {
            return Ok(None);
        };
        let snapshot = IntegrationSnapshot {
            kind: row.agent_kind,
            resume_template: row.resume_template,
        };
        let captured = row.captured_conversation;
        Ok(Some(SessionSnapshot {
            restart_offer: snapshot.restart_offer(captured.as_deref()),
            resume_argv: captured
                .as_deref()
                .and_then(|conversation| snapshot.filled_resume_argv(conversation)),
            kind: snapshot.kind,
            resume_template: snapshot.resume_template,
            captured_conversation: captured,
            first_input_at: row.first_input_at,
            capture_ambiguous: row.capture_ambiguous,
            canonical_cwd: row.canonical_cwd,
        }))
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
        // The freshly loaded map's own capture pass; see `capture_now`.
        self.capture_now().await;
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
        // The attachments tree's own reconciliation, which needs the same
        // authoritative session set the launch-dir sweep does: staging
        // files a hard crash stranded, directories a delete parked but
        // never discarded, and whole session directories whose session no
        // longer exists (see `attachments::reconcile_at_startup`).
        crate::attachments::reconcile_at_startup(&self.state_dir, &known_sessions).await;
        // This supervisor's own cadence (PLAN_M6_75.md item 1), started
        // last because this is where initialization ENDS: the session map
        // is the one this process will serve, the socket is bound, and the
        // startup reconciliation has already decided what on-disk state
        // was debris. (An earlier version of this comment claimed the
        // ticker had to follow the sweeps to avoid reading files they were
        // unlinking; that was wrong and is worth recording as wrong — the
        // capture pass scans the AGENTS' record roots under `agent_home`,
        // while the sweeps unlink launch specs, snapshots, and tmux config
        // temporaries under the state dir. The two never touch the same
        // file. What the ordering actually buys is that no tick can
        // observe a half-initialized supervisor.)
        //
        // Bound to a name rather than discarded: the handle owns the task,
        // so `let _ = ...` would stop the ticker at the instant it started
        // it. See `ticker`'s module doc for the shutdown contract; here
        // the accept loop below never returns, so the ticker's lifetime is
        // this call's.
        let mut ticker = start_ticker(self);
        info!(socket = %path.display(), "supervisor listening");
        loop {
            let accepted = tokio::select! {
                // A ticker that ENDED while this process is still serving
                // can only mean a panic (nothing else stops it while the
                // handle is held), and a silently absent ticker is the
                // worst shape this feature has: capture would quietly stop
                // advancing for any session nobody happens to poll, with
                // no symptom until a restart offered a fresh launch where
                // a resume was expected. `watch` reports it loudly and
                // then parks forever, so this arm fires at most once.
                // Deliberately NOT fatal and deliberately not restarted:
                // status and capture are both best-effort, and taking a
                // whole host's sessions offline — or spinning up a
                // replacement task that would panic on the same input —
                // would be a worse answer than serving without a ticker
                // and saying so.
                () = ticker.watch() => continue,
                accepted = listener.accept() => accepted,
            };
            match accepted {
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
    /// whole under that key's lock ([`KeyedLocks`]) so concurrent retries
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
    ///
    /// ## Why a RETRY does not re-resolve its request
    ///
    /// A pending retry rebuilds its launch from the row the crashed attempt
    /// committed ([`Supervisor::validate_retry`]) rather than from the
    /// request that arrived, and the reason is PLAN_M6_75.md item 4's
    /// catalog: the request names a profile, and a profile is MUTABLE. An
    /// unchanged retry that re-read the catalog would launch whatever the
    /// profile says NOW — so editing a profile between the crash and the
    /// retry would silently change what an already-accepted intent runs,
    /// and deleting it would turn that intent into a `NotFound` for a
    /// create the supervisor had already accepted and half-performed. The
    /// stored row is what the first attempt actually resolved, and a retry
    /// under the same reservation is the same create.
    pub(crate) async fn create_session(
        &self,
        inputs: CreateInputs<'_>,
        claim: Option<IntentClaim>,
    ) -> anyhow::Result<SessionInfo> {
        // `~` expansion lives at the head of `validate_create`, and its
        // reach is exactly validation's reach: a replay answers from the
        // reservation without expanding anything (expansion reads
        // supervisor state — the resolved home — which a settled intent
        // must not re-consult), a retry with its row intact relaunches
        // from the row's already-expanded cwd (`validate_retry`), and a
        // refused expansion is a validation refusal recorded against the
        // intent key like any other. The intent-key FINGERPRINT is
        // computed by the handler from the client's literal string: a
        // client retrying "~" produces the same fingerprint both times,
        // which is all idempotency needs.
        let Some(claim) = claim else {
            let request = self.validate_create(inputs).await?;
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
                Resolution::Answer(answer) => return *answer,
                Resolution::Relaunch(reservation) => Reserved::Retry(reservation),
                Resolution::Fresh => Reserved::New {
                    claim: claim.clone(),
                    identity: new_session_identity(),
                },
            },
            None => Reserved::New {
                claim: claim.clone(),
                identity: new_session_identity(),
            },
        };
        let validated = match &reserved {
            Reserved::Retry(reservation) => self.validate_retry(inputs, reservation).await,
            _ => self.validate_create(inputs).await,
        };
        let request = match validated {
            Ok(request) => request,
            Err(refusal) => return self.record_refused_create(&reserved, refusal).await,
        };
        self.launch_session(request, reserved).await
    }

    /// [`Supervisor::create_session`] with no snapshot overrides — the
    /// shape every test predating PLAN_M3.md item 7 exercises.
    ///
    /// A test-only wrapper rather than a production convenience: sending
    /// no overrides is what the UI does, but expressing that as a
    /// SEPARATE entry point in production would give the create path two
    /// doors and let a future field be threaded through only one of them.
    /// Tests that DO exercise overrides build `CreateInputs` themselves.
    #[cfg(test)]
    async fn create_session_without_overrides(
        &self,
        cwd: &str,
        invocation: &str,
        title: Option<String>,
        cols: u16,
        rows: u16,
        claim: Option<IntentClaim>,
    ) -> anyhow::Result<SessionInfo> {
        self.create_session(
            CreateInputs {
                cwd,
                parent: None,
                mode: CreateMode::Raw {
                    invocation: invocation.to_string(),
                    agent_kind: None,
                    resume_template: None,
                },
                title,
                cols,
                rows,
            },
            claim,
        )
        .await
    }

    /// Everything checkable before the world is touched: the working
    /// directory is usable, the invocation parses into an argv, the
    /// integration snapshot resolves, and the title is resolved — refused
    /// if the caller spelled a control character into it, defaulted from
    /// the cwd (with control characters sanitized) when they omitted it.
    ///
    /// Split out of `create_session` because the idempotency state machine
    /// must be able to run its reservation lookup WITHOUT it (see that
    /// function's docs on ordering) and then apply it to only the branches
    /// that are about to launch something.
    ///
    /// The snapshot resolution belongs HERE rather than at the launch for
    /// two reasons. It can fail — PLAN_M3.md item 7's one validation
    /// invariant — and every refusal in this function is one a keyed create
    /// records against its intent key and replays verbatim, which is the
    /// contract acceptance 7 states without a validation exception. And it
    /// needs the parsed `argv`, since the kind and the default template
    /// both come from the invocation's FIRST TOKEN rather than from the
    /// invocation string.
    ///
    /// The parsed argv and any resume-template override are additionally
    /// held to the shared executable-argv rule
    /// (`agent_kind::ensure_executable_argv`,
    /// `agent_kind::ensure_resume_template`), which is the same rule profile
    /// writes enforce. A raw create is otherwise the door through which a
    /// command line that names no program — `''` splits into a one-element
    /// argv holding the empty string — reaches tmux. An additional check
    /// refuses a `{cwd}` placeholder as the program
    /// (`agent_kind::ensure_no_cwd_program`): a raw create is the one path
    /// that reaches tmux without a profile in front of it, so it needs the
    /// identical refusal the profile editor applies.
    ///
    /// ## Profile resolution (PLAN_M6_75.md item 4)
    ///
    /// A profile-backed create resolves its profile FIRST, whether the caller
    /// selected its stable id, selected its human-facing name, or omitted a
    /// selector on an authenticated spawn and asked for the host's last-used
    /// profile. Everything after that point is identical to a raw create: the profile's
    /// invocation, kind, and template feed the very same
    /// [`IntegrationSnapshot::resolve`] seam a raw create's overrides do,
    /// so a session created from a profile carries an ordinary immutable
    /// snapshot with no second code path behind it. What the profile adds
    /// is the source-profile identity recorded beside that snapshot.
    ///
    /// An unknown id is `NotFound`: the profile can be deleted between a
    /// picker read and submit, so the caller must refresh and pick again. A
    /// name with zero or multiple exact matches is `InvalidRequest` and names
    /// the available or matching candidates, so `--agent` can be made
    /// unambiguous. A selectorless spawn also returns `InvalidRequest` when
    /// there is no last-used source, or when that source profile has since
    /// been deleted; its remedy is an explicit `--agent <profile-name>`.
    /// None of these paths silently chooses another profile, since that would
    /// launch an agent the user never asked for.
    ///
    /// Each refusal is recorded against a keyed create like every other
    /// precondition, so a retry replays the same answer instead of resolving
    /// against a catalog or host default that may have changed again.
    ///
    /// A method rather than an associated function since PLAN_M6_75.md item
    /// 4: resolving a profile needs the store.
    async fn validate_create(&self, inputs: CreateInputs<'_>) -> anyhow::Result<LaunchRequest> {
        let CreateInputs {
            cwd,
            parent,
            mode,
            title,
            cols,
            rows,
        } = inputs;
        // The profile lookup precedes the cwd check for one reason: it is
        // the only precondition here that reads state this supervisor OWNS,
        // and reporting "no such profile" for a request that also names a
        // vanished directory is the more actionable of the two answers —
        // the client's catalog is stale and every retry against that
        // profile will fail, directory or no directory.
        let (invocation, agent_kind, resume_template, source_profile) = match mode {
            CreateMode::Raw {
                invocation,
                agent_kind,
                resume_template,
            } => (invocation, agent_kind, resume_template, None),
            CreateMode::Profile { profile_id } => {
                let profile_id = profile_id.as_str();
                let profile = self
                    .store
                    .profile(profile_id)
                    .await
                    .context("reading the profile this create names")?
                    .ok_or_else(|| {
                        RequestError::new(
                            ErrorKind::NotFound,
                            format!(
                                "no profile {} exists on this host; it may have been deleted \
                                 since the profile list was read — pick another and try again",
                                truncate_for_error(profile_id)
                            ),
                        )
                    })?;
                (
                    profile.invocation,
                    // A profile's kind is never a guess (it is
                    // `AgentKind::Generic` when the user picked none), so it
                    // is passed as an explicit OVERRIDE rather than left for
                    // basename derivation — a `Generic` profile whose
                    // invocation happens to start with `claude` must stay
                    // generic, because that is what the user chose.
                    Some(profile.agent_kind),
                    // `None` here keeps `Profile::resume_template`'s own
                    // meaning: for an integrated kind the snapshot resolver
                    // derives that kind's default (from THIS invocation's
                    // argv0, so an edited invocation carries its resume with
                    // it), and for a generic profile there is simply no
                    // resume template.
                    profile.resume_template,
                    Some(ProfileSnapshot {
                        id: profile.id,
                        name: profile.name,
                    }),
                )
            }
            CreateMode::ProfileName { profile_name } => {
                let profiles = self
                    .store
                    .profiles()
                    .await
                    .context("reading the profile catalog to resolve this create's name")?;
                let candidates = profiles
                    .iter()
                    .map(|profile| format!("{} ({})", profile.name, profile.id))
                    .collect::<Vec<_>>()
                    .join(", ");
                let mut matches = profiles
                    .into_iter()
                    .filter(|profile| profile.name == profile_name)
                    .collect::<Vec<_>>();
                let profile =
                    match matches.len() {
                        1 => matches.pop().expect("one profile match was counted"),
                        0 => {
                            return Err(RequestError::new(
                            ErrorKind::InvalidRequest,
                            format!(
                                "no profile named {} exists on this host; available profiles: {}",
                                truncate_for_error(&profile_name),
                                if candidates.is_empty() { "none" } else { &candidates }
                            ),
                        )
                        .into());
                        }
                        _ => {
                            let matches = matches
                                .iter()
                                .map(|profile| format!("{} ({})", profile.name, profile.id))
                                .collect::<Vec<_>>()
                                .join(", ");
                            return Err(RequestError::new(
                                ErrorKind::InvalidRequest,
                                format!(
                                    "profile name {} is ambiguous; matching candidates: {matches}",
                                    truncate_for_error(&profile_name)
                                ),
                            )
                            .into());
                        }
                    };
                (
                    profile.invocation,
                    Some(profile.agent_kind),
                    profile.resume_template,
                    Some(ProfileSnapshot {
                        id: profile.id,
                        name: profile.name,
                    }),
                )
            }
            CreateMode::DerivedProfile => {
                let source = self
                    .store
                    .latest_source_profile()
                    .await
                    .context("deriving this host's last-used profile")?
                    .ok_or_else(|| {
                        RequestError::new(
                            ErrorKind::InvalidRequest,
                            "no profile has been used on this host; pass --agent <profile-name>",
                        )
                    })?;
                let profile = self
                    .store
                    .profile(&source.id)
                    .await
                    .context("reading this host's last-used profile")?
                    .ok_or_else(|| {
                        RequestError::new(
                            ErrorKind::InvalidRequest,
                            format!(
                                "the last-used profile {} ({}) no longer exists; pass --agent \
                                 <profile-name> instead of guessing an older default",
                                truncate_for_error(&source.name),
                                truncate_for_error(&source.id)
                            ),
                        )
                    })?;
                (
                    profile.invocation,
                    Some(profile.agent_kind),
                    profile.resume_template,
                    Some(ProfileSnapshot {
                        id: profile.id,
                        name: profile.name,
                    }),
                )
            }
        };
        // `~` expansion happens here, as the first half of the cwd check,
        // in the one function every NEW create flows through — unkeyed,
        // keyed-new, and a retry whose row vanished. A retry with its row
        // intact never comes here and never re-expands: its first
        // attempt's stored cwd is the resolution (see `validate_retry`),
        // so a `HOME` change between the crash and the retry cannot re-aim
        // or reject an already-accepted create. Everything below — the
        // usability check, the derived title, the durable row, the
        // canonical identity — sees only the expanded absolute path.
        let cwd = expand_tilde_cwd(cwd, self.user_home.as_deref())?;
        let cwd = cwd.as_ref();
        let cwd_path = PathBuf::from(cwd);
        ensure_cwd_usable(cwd).await?;
        // The invocation itself stays out of the error: it may carry
        // credentials (`--api-key ...`), and this message travels into
        // the HTTP error body and the helm's stderr/journal. shell-words'
        // own error names the syntax problem. Attached as `.context(...)`
        // (not the root cause) specifically so that diagnostic keeps
        // reaching the user through the `{e:#}` chain — `RequestError` is
        // still findable via `downcast_ref` at this depth (see its docs).
        let argv = shell_words::split(&invocation).context(RequestError::new(
            ErrorKind::InvalidRequest,
            "parsing agent invocation",
        ))?;
        // The SHARED executable-argv rule, not a local emptiness test. An
        // `argv.is_empty()` check on its own accepts `''`, which
        // `shell_words` splits into a one-element argv holding the empty
        // string — a command line that exists and names nothing — and it
        // says nothing about a NUL byte, which truncates an argument
        // silently rather than failing. Profile writes have refused both
        // for a while; this is the same rule reaching the raw path.
        crate::agent_kind::ensure_executable_argv("agent invocation", &argv)
            .map_err(|message| RequestError::new(ErrorKind::InvalidRequest, message))?;
        // A raw create is the boundary that reaches tmux without ever
        // passing through a profile, so it needs the same `{cwd}`-as-PROGRAM
        // refusal the profile editor applies (`store::validate_profile_fields`)
        // — the rule is about the shape of the argv, not about which door it
        // came through.
        crate::agent_kind::ensure_no_cwd_program("agent invocation", &argv)
            .map_err(|message| RequestError::new(ErrorKind::InvalidRequest, message))?;
        // The resume-template OVERRIDE is caller data on exactly the same
        // footing as a profile's template, and it becomes this session's
        // immutable snapshot — so it is held to the same rule here rather
        // than at the restart that would otherwise discover it.
        if let Some(template) = resume_template.as_deref() {
            crate::agent_kind::ensure_resume_template(template)
                .map_err(|message| RequestError::new(ErrorKind::InvalidRequest, message))?;
        }
        // ## Titles must stay printable on one line
        //
        // A title is durable metadata that this supervisor echoes verbatim
        // in every `SessionList` reply, and its consumers are not all
        // DOM-shaped: the helm already writes it through `tracing` at
        // startup (farhelm-helm's "startup session created" line), so a
        // terminal-bound renderer exists TODAY, and a CLI `list` would be
        // another. A terminal is unforgiving of arbitrary bytes — an
        // embedded escape sequence is terminal injection the moment it is
        // printed, and even a bare newline or tab breaks the one-line-label
        // assumption every renderer makes. `char::is_control` sweeps all of
        // that at once: C0 (including \n, \t, ESC), DEL, and the C1 range.
        //
        // The two sources of a title are handled ASYMMETRICALLY, and the
        // asymmetry is the point.
        //
        // An EXPLICIT title is caller data, so it is REFUSED rather than
        // rewritten: silently altering what the caller sent is a worse
        // surprise than a clear error, and nothing legitimate constructs a
        // title this way. Living in `validate_create` rather than at the
        // protocol edge is also what keeps a keyed create honest — every
        // refusal from this function is recorded against the intent key by
        // `record_refused_create`, so the retry replays this answer instead
        // of a fresh one. The check itself is [`ensure_title_printable`],
        // shared with rename (PLAN_M5.md item 3) so the two verbs cannot
        // grow different rules — or different words — for one contract.
        //
        // A DERIVED title is SANITIZED instead. It is server-generated from
        // a directory the caller never chose as a label, and a control
        // character is legal in a path component, so refusing here would
        // make an existing, perfectly usable directory impossible to open a
        // session in — punishing the caller for a name they did not pick.
        // Replaced with U+FFFD rather than deleted so the label still shows
        // that something was there.
        //
        // `cwd` and `invocation` are deliberately not swept: neither is a
        // display label. `cwd` becomes tmux's working-directory argument
        // and `invocation` is parsed into an argv — both are consumed as
        // data by something that already has to accept arbitrary bytes.
        let title = match title {
            Some(explicit) => {
                ensure_title_printable(&explicit)?;
                explicit
            }
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
            None => cwd_path
                .file_name()
                .map(|n| {
                    n.to_str()
                        .expect("cwd arrived as UTF-8 via the protocol; its components are UTF-8")
                })
                .unwrap_or("session")
                .chars()
                .map(|c| if c.is_control() { '\u{FFFD}' } else { c })
                .collect(),
        };
        // Derived from `argv[0]`, the ORIGINAL first token — not from a
        // canonical command name — so `/opt/bin/claude` resumes through
        // `/opt/bin/claude`. `crate::agent_kind` owns every rule here,
        // including the one failure this whole function can produce that
        // is not about the filesystem.
        let snapshot = IntegrationSnapshot::resolve(&argv[0], agent_kind, resume_template)
            .map_err(|e| RequestError::new(ErrorKind::InvalidRequest, e.to_string()))?;
        // The agent will report its own `getcwd()`, which the kernel has
        // already resolved, so correlation has to compare against the
        // resolved spelling or a session created through a symlink could
        // never match its own records. A failure here does NOT fail the
        // create: the directory was just confirmed usable, so this is a
        // race or an exotic filesystem, and the literal path is the honest
        // fallback — it costs capture for that session, never correctness.
        let canonical_cwd = match tokio::fs::canonicalize(&cwd_path).await {
            Ok(resolved) => resolved.to_string_lossy().into_owned(),
            Err(e) => {
                warn!(
                    cwd = %cwd, error = %e,
                    "could not resolve this working directory to a canonical path; \
                     conversation capture may not correlate for this session"
                );
                cwd.to_string()
            }
        };
        Ok(LaunchRequest {
            parent,
            cwd: cwd.to_string(),
            // A create has no prior identity to have verified, and the
            // canonicalization above is deliberately allowed to fail
            // without failing the create — so the caller's own spelling is
            // what tmux gets. See [`LaunchRequest::launch_cwd`].
            launch_cwd: cwd.to_string(),
            invocation,
            argv,
            title,
            cols,
            rows,
            snapshot,
            canonical_cwd,
            source_profile,
        })
    }

    /// Rebuild a PENDING retry's launch from the row its crashed attempt
    /// committed, rather than re-resolving the request that arrived
    /// (PLAN_M6_75.md item 4).
    ///
    /// A retry under an existing reservation is the SAME create: its
    /// identities are already assigned, and by this point the supervisor has
    /// established that the first attempt left no launch behind. What it
    /// must therefore run is what the first attempt resolved — and for a
    /// profile-backed create, that is no longer derivable from the request,
    /// because the catalog it names is mutable. Re-resolving would let an
    /// edit between the two attempts change what an unchanged intent
    /// launches, and a delete turn an accepted create into a `NotFound`.
    /// The row is the record of that resolution, so the row is what this
    /// reads: invocation, integration snapshot, canonical cwd, title, and
    /// the source-profile identity all come back exactly as committed.
    ///
    /// Two things are still checked against the world rather than taken from
    /// the row, and both are about to be used:
    ///
    /// - The working directory must STILL be usable. The retry is about to
    ///   launch into it, and a directory removed since the crash is a
    ///   precondition failure now, not a historical detail.
    /// - The working directory must still be the SAME directory
    ///   ([`ensure_cwd_identity`]). Usable is not enough: a symlink
    ///   repointed between the two attempts leaves a path that stats fine
    ///   and now resolves somewhere else, and this retry would launch there
    ///   while carrying the crashed attempt's `canonical_cwd` — so the
    ///   agent runs in one directory and conversation capture correlates
    ///   against another, silently and for the life of the session. The
    ///   VERIFIED path is what the launch is then aimed at
    ///   ([`LaunchRequest::launch_cwd`]), so the same link cannot be
    ///   repointed again between this check and the tmux call.
    /// - The stored invocation must still be an executable argv
    ///   (`agent_kind::ensure_executable_argv`) whose program is not the
    ///   `{cwd}` placeholder (`agent_kind::ensure_no_cwd_program`), and so
    ///   must the stored resume template. They were once, so this cannot
    ///   fail for a row this build wrote — but the database is a trust
    ///   boundary like any other input (`store`'s own decode says the
    ///   same), and the alternative to checking is handing tmux a vector
    ///   that names no program, or (slot 0 is never substituted) one that
    ///   would exec a program literally named `{cwd}` instead of failing
    ///   loudly on the refusal.
    ///
    /// A retry whose row is GONE falls back to validating the request
    /// normally. That is the honest fallback rather than a failure: there is
    /// no recorded resolution left to preserve, the reservation's identities
    /// are still free to reuse, and the alternative — refusing — would
    /// permanently strand an intent key whose session someone deleted.
    async fn validate_retry(
        &self,
        inputs: CreateInputs<'_>,
        reservation: &Reservation,
    ) -> anyhow::Result<LaunchRequest> {
        let row = self
            .store
            .session(&reservation.session_id)
            .await
            .context("reading the interrupted attempt's session row")?;
        let Some(row) = row else {
            return self.validate_create(inputs).await;
        };
        // The row's stored cwd IS the first attempt's resolution — already
        // expanded if the request said `~` — so the checks run against it,
        // never against the request's literal string. Re-expanding the
        // request here would let a `HOME` change between the crash and the
        // retry reject (or worse, re-aim) an already-accepted create; the
        // fingerprint has already proven the request is the same literal
        // string the first attempt resolved, which is all the request's
        // own cwd is good for.
        ensure_cwd_usable(&row.cwd).await?;
        let verified = ensure_cwd_identity(&row.cwd, row.canonical_cwd.as_deref()).await?;
        let argv = shell_words::split(&row.invocation).context(RequestError::new(
            ErrorKind::InvalidRequest,
            "parsing the interrupted attempt's recorded agent invocation",
        ))?;
        // Same shared rule the raw path applies, for the reason this
        // function's docs give about the database being a trust boundary:
        // `''` and an embedded NUL both survive a round trip through
        // SQLite, and neither is an argv anything can run.
        crate::agent_kind::ensure_executable_argv(
            "the interrupted attempt's recorded agent invocation",
            &argv,
        )
        .map_err(|message| RequestError::new(ErrorKind::InvalidRequest, message))?;
        // `decode_session_row` already refuses a `{cwd}`-first invocation
        // when the row is loaded, so a row read through the store cannot
        // carry one here. Checked again anyway: this is the gate on the
        // vector about to be exec'd, and the repo's pattern for a rule this
        // sharp is to hold both ends of the pipe to it rather than trust
        // that the one enforcing end never changes underneath the other.
        crate::agent_kind::ensure_no_cwd_program(
            "the interrupted attempt's recorded agent invocation",
            &argv,
        )
        .map_err(|message| RequestError::new(ErrorKind::InvalidRequest, message))?;
        if let Some(template) = row.resume_template.as_deref() {
            crate::agent_kind::ensure_resume_template(template)
                .map_err(|message| RequestError::new(ErrorKind::InvalidRequest, message))?;
        }
        Ok(LaunchRequest {
            parent: row.parent,
            // The path `ensure_cwd_identity` just VERIFIED, so the launch
            // cannot be aimed somewhere else by a symlink repointed between
            // that check and the tmux call. `None` only for a row with no
            // recorded identity, where there was nothing to verify.
            launch_cwd: verified.unwrap_or_else(|| row.cwd.clone()),
            invocation: row.invocation,
            argv,
            title: row.title,
            cols: inputs.cols,
            rows: inputs.rows,
            snapshot: IntegrationSnapshot {
                kind: row.agent_kind,
                resume_template: row.resume_template,
            },
            // `None` only for a row written before the column existed, which
            // is necessarily a non-integrated session; the stored cwd is the
            // same honest fallback `validate_create` uses.
            canonical_cwd: row.canonical_cwd.unwrap_or_else(|| row.cwd.clone()),
            cwd: row.cwd,
            source_profile: row.source_profile,
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
            return Resolution::Answer(Box::new(Err(RequestError::new(
                ErrorKind::Conflict,
                format!(
                    "intent key {} was already used for a different create request; \
                     a reused key is a client bug rather than a merge, so this request \
                     is refused — send a new key for a new request",
                    truncate_for_error(&claim.intent_key)
                ),
            )
            .into())));
        }
        if reservation.dedup_scope == DedupScope::SessionLifetime {
            match self
                .store
                .prune_orphaned_bounded_reservation(
                    &reservation.intent_key,
                    &reservation.session_id,
                )
                .await
            {
                Ok(true) => return Resolution::Fresh,
                Ok(false) => {}
                Err(error) => return Resolution::Answer(Box::new(Err(error))),
            }
        }
        match &reservation.outcome {
            // Settled either way: the answer is whatever was recorded, and
            // `answer_from` is the one place that decides what that means
            // so every caller of it agrees (`ReservationOutcome::Failed`'s
            // own docs on why the kind rides along with the message).
            ReservationOutcome::Created | ReservationOutcome::Failed { .. } => {
                self.replay_resolution(&reservation, self.answer_from(&reservation).await)
                    .await
            }
            ReservationOutcome::Pending => {
                match self.reserved_launch_evidence(&reservation).await {
                    LaunchEvidence::Present => {
                        let answer = self.settle_and_replay(&reservation).await;
                        self.replay_resolution(&reservation, answer).await
                    }
                    LaunchEvidence::Absent => Resolution::Relaunch(Box::new(reservation)),
                    // Neither relaunch nor replay: this process cannot tell
                    // which is true, and both wrong answers are permanent (a
                    // duplicate agent, or a success that never ran). The
                    // reservation stays pending, so a later retry — or the next
                    // reload, once whatever failed is readable again — resolves
                    // it against evidence instead of a guess.
                    LaunchEvidence::Unresolved(why) => Resolution::Answer(Box::new(Err(why
                        .context(format!(
                            "cannot tell whether intent key {}'s create ever launched, so it is \
                     neither replayed nor retried; try again once the cause is cleared",
                            truncate_for_error(&claim.intent_key)
                        ))))),
                }
            }
        }
    }

    /// Recheck a failed bounded replay against concurrent deletion.
    ///
    /// The first orphan prune can linearize before deletion while the row
    /// still exists. If replay then finds it gone, deletion either removed
    /// the reservation too or left it eligible for this second atomic
    /// prune. Both cases mean the key is fresh, never a permanent-style
    /// conflict.
    async fn replay_resolution(
        &self,
        reservation: &Reservation,
        answer: anyhow::Result<SessionInfo>,
    ) -> Resolution {
        if answer.is_err() && reservation.dedup_scope == DedupScope::SessionLifetime {
            let pruned = self
                .store
                .prune_orphaned_bounded_reservation(
                    &reservation.intent_key,
                    &reservation.session_id,
                )
                .await;
            match pruned {
                Ok(true) => return Resolution::Fresh,
                Ok(false) => match self.store.reservation(&reservation.intent_key).await {
                    Ok(None) => return Resolution::Fresh,
                    Ok(Some(_)) => {}
                    Err(error) => return Resolution::Answer(Box::new(Err(error))),
                },
                Err(error) => return Resolution::Answer(Box::new(Err(error))),
            }
        }
        Resolution::Answer(Box::new(answer))
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
    /// Four independent sources, any one of which is enough to say a
    /// launch happened:
    ///
    /// - **The durable row.** A recorded pane means something once saw this
    ///   session in tmux. An outcome past `Launching` means the same, with
    ///   ONE exception that is exactly why the pane is checked separately:
    ///   `Interrupted` is written by the reboot conversion, which blankets
    ///   `Launching` rows too — so an interrupted row with no pane is a
    ///   create that may never have launched at all, and treating the
    ///   status alone as provenance would replay a session that never
    ///   existed. A non-NULL `conversation_source` on the same row is a
    ///   third, independent reading of it: only the launch hook writes that
    ///   column, and the hook runs inside the agent process this
    ///   reservation started, so a report is the agent testifying to its
    ///   own existence. It can arrive before the pane is recorded, which is
    ///   the whole reason it is checked rather than being assumed to imply
    ///   one of the others.
    /// - **The launch sentinel.** The shim wrote it, so the shim ran, so
    ///   tmux started something — even when no pane was ever recorded.
    /// - **The launch's cgroup scope**, where this host has a user manager.
    ///   A unit with the reserved generation-0 name can only have been
    ///   created inside the reserved session's own window, and — unlike
    ///   every other source here — it survives the agent, the pane, and the
    ///   tmux session all being gone while a daemon it spawned runs on.
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
                    || row.conversation_source.is_some()
                {
                    return LaunchEvidence::Present;
                }
            }
            Ok(None) => {}
            Err(e) => {
                return LaunchEvidence::Unresolved(e.context("reading the reserved session row"));
            }
        }
        // The launch's cgroup, where this host has one. A scope with the
        // reserved identities' generation-0 name can only have been created
        // by `systemd-run` inside the reserved session's own tmux window, so
        // its existence proves that window ran — and it proves it in the one
        // case every other source misses: a wrapped launch whose agent
        // daemonized something and then died, leaving no pane, no sentinel,
        // and a tmux session already gone, while the daemon runs on inside
        // the cgroup. Relaunching over that would be the duplicate this
        // whole mechanism exists to exclude.
        //
        // Only asked when a manager is actually available, because `exists`
        // on a manager-less host is an ERROR, and reading that as
        // `Unresolved` would wedge every reconciliation in CI.
        if self.seams.scopes.available().await
            && let Some(unit) = crate::scope::unit_name(&reservation.session_id, 0)
        {
            match self.seams.scopes.exists(&unit).await {
                Ok(true) => return LaunchEvidence::Present,
                Ok(false) => {}
                Err(e) => {
                    return LaunchEvidence::Unresolved(
                        e.context("asking the user manager about the reserved launch's scope"),
                    );
                }
            }
        }
        // Generation 0: a reservation's session is one this create is
        // still trying to launch for the FIRST time, so its evidence can
        // only ever be its original launch's (`spec_path_for_launch`).
        match read_launch_sentinel(&self.state_dir, &reservation.session_id, 0).await {
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
    /// (`status: Unknown`, no annotation) — a replay is the same answer, so
    /// it must have the same shape. The restart offer is the one field
    /// computed rather than placeheld, and from the STORED snapshot: it is
    /// a property of the session that already exists, not of this reply,
    /// and a replay landing after capture succeeded should say so rather
    /// than under-report what the original create would report now.
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
            Some(row) => {
                let snapshot = IntegrationSnapshot {
                    kind: row.agent_kind,
                    resume_template: row.resume_template,
                };
                // A placeholder existence, replaced below by
                // `with_derived_source_profile`. Derived NOW rather than
                // replayed, because a replay can land long after the
                // original create and the profile it named may have been
                // renamed or deleted since.
                let source_profile = row.source_profile.map(|snapshotted| SourceProfile {
                    id: snapshotted.id,
                    name: snapshotted.name,
                    existence: ProfileExistence::Present,
                });
                let info = SessionInfo {
                    parent: row.parent,
                    archived: row.archived,
                    restart_offer: snapshot.restart_offer(row.captured_conversation.as_deref()),
                    id: row.id,
                    title: row.title,
                    created_at: row.created_at,
                    // Read from the row like everything else here: a
                    // replay answers from the durable record, and the
                    // session it describes may have been producing output
                    // for hours since the original create.
                    last_activity_at: row.last_activity_at,
                    creation_seq: Some(row.creation_seq),
                    cwd: row.cwd,
                    invocation: row.invocation,
                    status: SessionStatus::Unknown,
                    annotation: None,
                    // Vocabulary only for now — see PLAN_M4.md step 4 for
                    // where tabs get real rediscovery.
                    tabs: Vec::new(),
                    source_profile,
                };
                self.with_derived_source_profile(info).await
            }
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
        request: LaunchRequest,
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
        request: LaunchRequest,
        reserved: &Reserved,
    ) -> anyhow::Result<SessionInfo> {
        let LaunchRequest {
            parent,
            cwd,
            launch_cwd,
            invocation,
            argv,
            title,
            cols,
            rows,
            snapshot,
            canonical_cwd,
            source_profile,
        } = request;
        // Reassigned on the retry-takeover path below, from the value that
        // transaction actually committed: a rename that landed between the
        // crashed attempt and this retry survives in SQLite, and the reply
        // and the published entry built from here have to agree with it.
        let mut title = title;
        let mut creation_seq = 0;
        let mut session_token = None;
        let id = reserved.session_id().to_string();
        let tmux_name = reserved.tmux_name().to_string();
        // Decided ONCE, here, and carried into every durable write below —
        // never re-decided per write. A create's launch is generation 0 by
        // construction (only a restart ever bumps it), so the unit name is
        // fully determined at this point, and committing the selection with
        // the launching row is what makes it survive a crash straddling the
        // launch (PLAN_M3.md items 2 and 10).
        let scoped = self.scope_selected(&id).await;
        // Sampled HERE, after `scope_selected`'s await — not before it —
        // because that call's FIRST invocation anywhere in the process can
        // run the full systemd-availability probe (`scope::ScopeManager::
        // available`'s own docs), which is not instantaneous. A `created_at`
        // read before that probe could predate the row it is about to be
        // written into by however long the probe took, disagreeing with
        // insert order under concurrency (a session that started launching
        // SECOND could still record an EARLIER `created_at` than one that
        // started first but hit a warm cache). Reading it here instead, as
        // close to the durable insert as the shared control flow below
        // allows, keeps the stored value honest.
        //
        // Read ONCE and threaded into both the durable row (whichever of
        // the retry-takeover or fresh-insert branches below runs) and the
        // `SessionInfo` this function returns, so a FRESH-INSERT reply's
        // `created_at` (PLAN_M6.md item 1) is never a second,
        // independently-timed reading of the clock that could drift from
        // what actually landed in SQLite — see `StoredSession::created_at`'s
        // own docs. On the retry-takeover path below this value is only a
        // FALLBACK candidate for `row.created_at`: `restart_pending_launch`
        // preserves the crashed attempt's own original timestamp instead
        // whenever it can find it, and `created_at` is reassigned to that
        // preserved value before this function's own reply is built, so a
        // retry's reply matches the row a concurrent `ListSessions` could
        // already have shown for it (`StoredSession::created_at`'s docs
        // again, for why that matters) — this is also why sampling here,
        // slightly before the retry branch's OWN awaits
        // (`clear_launch_artifacts_fail_closed`, `restart_pending_launch`),
        // is safe: nothing on that path ever keeps this fallback value once
        // a preserved timestamp is found.
        let mut created_at = now_unix();
        let launch_scope = launch_scope_unit(&id, 0, scoped);
        if let Reserved::Retry(reservation) = reserved {
            // Clear the interrupted attempt's leftovers before reusing its
            // identities; `clear_launch_artifacts_fail_closed` carries the
            // full argument for why this is fail-closed, and item 9's
            // restart takes the same step for the same reason.
            if let Err(e) = clear_launch_artifacts_fail_closed(&self.state_dir, &id, 0).await {
                return Err(anyhow::anyhow!(
                    "not relaunching intent key {}: {e}; the intent stays pending, so a \
                     retry can resolve it once the cause is cleared",
                    truncate_for_error(&reservation.intent_key)
                ));
            }
            // The atomic re-check of the decision that got us here: the
            // evidence was gathered a moment ago, and a delete or a
            // late-landing launch since then must win over it. See
            // `SessionStore::restart_pending_launch`.
            // The snapshot rides the RE-inserted row exactly as it does a
            // first insert: a relaunch under the same reservation is the
            // same create, so the session it finally produces must carry
            // the same immutable kind and template it would have had if
            // the first attempt had not crashed. The source-profile
            // snapshot rides along for the same reason — and it is the same
            // profile either way, since the fingerprint binds the profile
            // identity to the intent key.
            let row = StoredSession {
                conversation_source: None,
                id: id.clone(),
                parent: parent.clone(),
                archived: false,
                title: title.clone(),
                created_at,
                // Seeded to creation, never to 0: a session nobody has
                // watched produce output yet has no activity to report,
                // and the honest fallback for a recency sort is "as recent
                // as it is old". `restart_pending_launch` reads the
                // preserved column back out anyway, so this value only
                // survives on the path where no prior row was found.
                last_activity_at: created_at,
                creation_seq: 0,
                cwd: cwd.to_string(),
                invocation: invocation.clone(),
                tmux_name: tmux_name.clone(),
                pane: String::new(),
                outcome: LastOutcome::Launching,
                agent_kind: snapshot.kind,
                resume_template: snapshot.resume_template.clone(),
                canonical_cwd: Some(canonical_cwd.clone()),
                captured_conversation: None,
                captured_record: None,
                capture_ambiguous: false,
                first_input_at: None,
                generation: 0,
                launch_scoped: scoped,
                source_profile: source_profile.clone(),
            };
            // The takeover and the map removal that mirrors it run under
            // this session's LIFECYCLE CLAIM, which is what closes the
            // rename window between them. `rename_session` takes the same
            // claim and then resolves the session through the map, so with
            // the claim held it either commits entirely BEFORE the takeover
            // — where the transaction preserves its title and hands it back
            // — or finds the entry already gone and answers `NotFound`, the
            // same way stop and delete do for the rest of this launch.
            // Without it a rename could land in between: committed to
            // SQLite and to the map, and then erased from the map by the
            // removal below, leaving the durable title and every list this
            // process serves disagreeing until the next reload.
            //
            // The claim is released the moment this block ends, well before
            // the tmux round trips further down. Holding one across a wedged
            // tmux would take this session's stop, restart and delete down
            // with it, and nothing past the map removal needs it: with no
            // entry on the map there is nothing left for a claim-holder to
            // resolve.
            //
            // Nothing inside takes a lifecycle claim of its own (the two
            // replay paths are reached only after this block), so the
            // non-reentrant `KeyedLocks` cannot deadlock here. The
            // intent-key claim `create_session` already holds is nested
            // OUTSIDE this one, and that nesting has no partner in the
            // other direction: `intent_locks` is claimed in exactly one
            // place, and nothing there holds a lifecycle claim first.
            let claim = {
                let _lifecycle = self.lifecycle_locks.claim(&id).await;
                let claim = self
                    .store
                    .restart_pending_launch(row, &reservation.intent_key)
                    .await
                    .context("taking over the interrupted attempt's reservation")?;
                if matches!(claim, RetryClaim::Acquired { .. }) {
                    // The in-memory mirror follows the row it mirrors, and
                    // it is also what serializes this relaunch against
                    // `StopSession` and `DeleteSession`: both resolve the
                    // session through this map, so while a relaunch is in
                    // flight they answer `NotFound` rather than tearing down
                    // a launch that is half-built. The residual window — a
                    // delete that read the map just before this removal — is
                    // closed at the other end, where the confirmation below
                    // finds its row already gone.
                    self.sessions.lock().await.remove(&id);
                }
                claim
            };
            match claim {
                // Overwrites the fallback `now_unix()` reading above with
                // the crashed attempt's PRESERVED value, and the resolved
                // snapshot's title with whatever the row actually carries —
                // see `RetryClaim::Acquired`'s and
                // `StoredSession::created_at`'s own docs for why this reply
                // must agree with whatever a concurrent reload-then-list
                // could already have shown.
                RetryClaim::Acquired {
                    created_at: preserved,
                    creation_seq: preserved_sequence,
                    title: preserved_title,
                    session_token: preserved_token,
                } => {
                    created_at = preserved;
                    creation_seq = preserved_sequence;
                    title = preserved_title;
                    session_token = Some(preserved_token);
                }
                RetryClaim::Resolved(settled) => return self.answer_from(&settled).await,
                RetryClaim::Launched => return self.settle_and_replay(reservation).await,
            }
        }

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
                        conversation_source: None,
                        id: id.clone(),
                        parent: parent.clone(),
                        archived: false,
                        title: title.clone(),
                        created_at,
                        // Nothing has been observed happening in a session
                        // that has not launched, so creation is the only
                        // honest seed — see the retry path's copy above.
                        last_activity_at: created_at,
                        creation_seq: 0,
                        cwd: cwd.to_string(),
                        invocation: invocation.clone(),
                        tmux_name: tmux_name.clone(),
                        // Not known until tmux has created the session —
                        // see `StoredSession::pane`.
                        pane: String::new(),
                        outcome: LastOutcome::Launching,
                        agent_kind: snapshot.kind,
                        resume_template: snapshot.resume_template.clone(),
                        canonical_cwd: Some(canonical_cwd.clone()),
                        // Every capture column is written by its own later,
                        // write-once path: nothing has been captured for a
                        // session that has not launched, nothing has been
                        // typed into it, and no correlation has been
                        // attempted, let alone found ambiguous.
                        captured_conversation: None,
                        captured_record: None,
                        capture_ambiguous: false,
                        first_input_at: None,
                        generation: 0,
                        launch_scoped: scoped,
                        // Written once, with the row, and never rewritten:
                        // SPEC.md's snapshot rule is that a later edit or
                        // delete of the profile leaves this session's record
                        // of what it came from exactly as it is.
                        source_profile: source_profile.clone(),
                    },
                    claim,
                )
                .await
                .context("recording new session in the database")?;
            match claimed {
                Claimed::Ours {
                    session_token: inserted_token,
                    creation_seq: inserted_sequence,
                } => {
                    session_token = Some(inserted_token);
                    creation_seq = inserted_sequence;
                }
                Claimed::TakenBy(winner) => {
                    // Someone else holds this key. Nothing was committed,
                    // so the honest answer is the winner's.
                    return self.answer_from(&winner).await;
                }
            }
        }
        // Deliberately BEFORE the cleanup-bearing paths below: a simulated
        // crash must leave the launching row (and its reservation) exactly
        // as a real one would, with nothing tidied up after it.
        self.simulate_crash(CreateStage::AfterRecord)?;

        let session_token = session_token.ok_or_else(|| {
            anyhow::anyhow!("session {id} committed without returning its spawn credential")
        })?;

        let spawned = self
            .spawn_agent(
                &id,
                &session_token,
                0,
                &tmux_name,
                argv,
                // The very snapshot the entry published below carries, so
                // the kind that chose this launch's hook flags is the kind
                // the session is durably recorded as. Re-deriving it here
                // would be a second classifier for one decision.
                &snapshot,
                // The VERIFIED directory, not the caller's spelling: see
                // [`LaunchRequest::launch_cwd`]. They differ only on the
                // retry path, and only there because that path had an
                // identity to check.
                &launch_cwd,
                cols,
                rows,
                None,
                None,
                launch_scope.as_deref(),
            )
            .await;
        let (pane, spec_path, status_file_path, hooked) = match spawned {
            Ok(Spawned {
                pane,
                spec_path,
                status_path,
                hooked,
            }) => (pane, spec_path, status_path, hooked),
            Err(SpawnFailure::Spec(error)) => {
                // Nothing external happened yet — the spec is the FIRST
                // side effect and it did not land — so the launching row
                // is provably describing nothing and is rolled back. A
                // crash here would leave it instead, which is the case
                // reload reconciles; this path can do better because the
                // process is still alive to know.
                return Err(self.abandon_launching_record(reserved, error).await);
            }
            Err(SpawnFailure::Tmux { spec_path, error }) => {
                // A tmux failure is AMBIGUOUS in a way the spec write is
                // not: `new-session` can fail after the session already
                // exists (a lost reply, a timeout mid-command), so
                // deleting the row on the strength of the error alone
                // would orphan a running agent — no row, no id, nothing
                // left that knows to reap it. Ask tmux instead, and only
                // roll back on a CONFIRMED absence; an ambiguous or failed
                // probe keeps the row, which is the only record anything
                // will ever have of that launch.
                let mut error = error;
                match self.tmux.has_session(&tmux_name).await {
                    Ok(false) => {
                        // Reaped BEFORE the row is discarded, and this is
                        // the ordering the whole rollback rests on:
                        // removing the row is removing the last handle
                        // anything has on this launch, so whatever the
                        // launch managed to start must be provably gone
                        // FIRST. "tmux has no session" is not that proof —
                        // a window that ran far enough to create its scope
                        // and daemonize something leaves exactly this
                        // shape — so the scope and the marker sweep are
                        // both asked. An unconfirmed reap RETAINS the row
                        // (the same rule the ambiguous arms below follow)
                        // rather than orphaning what it could not reach.
                        if let Err(sweep) = reap_process_tree(
                            &self.seams.scopes,
                            launch_scope.as_slice(),
                            None,
                            &id,
                            &SweepTarget::AgentOnly,
                        )
                        .await
                        {
                            return Err(error.context(format!(
                                "and the failed launch's process tree could not be swept \
                                 ({sweep:#}), so session {id} is kept as a launching record \
                                 rather than deleted"
                            )));
                        }
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
            parent,
            archived: false,
            id: id.clone(),
            title,
            // The same value just persisted: a fresh mint for a first-time
            // insert, or — on a retry takeover — the crashed attempt's own
            // PRESERVED value, reassigned from `RetryClaim::Acquired`
            // above. Either way this is never a second, independently
            // read clock value; see `StoredSession::created_at`'s docs for
            // why a retry in particular must not re-mint.
            created_at,
            // Equal to `created_at` for a session that has produced
            // nothing yet, which is what every create reply describes. The
            // two only diverge once the ticker has watched this session's
            // pane change.
            last_activity_at: created_at,
            creation_seq: Some(creation_seq),
            cwd: cwd.to_string(),
            invocation: invocation.clone(),
            // Create-time placeholder, deliberately NOT a live status:
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
            // No run has ended yet, so there is no stop annotation to
            // carry (PLAN_M3.md item 4).
            annotation: None,
            // Computed honestly rather than defaulted, even though nothing
            // can be captured at create time: a session created with an
            // explicit placeholder-free template already has a real
            // fallback to offer, and reporting `FreshOnly` for it would
            // understate what restart could do from the very first reply.
            restart_offer: snapshot.restart_offer(None),
            // A brand-new session has no tabs; real tab creation lands in
            // PLAN_M4.md step 4.
            tabs: Vec::new(),
            // The profile this create resolved, snapshotted once and never
            // rewritten (PLAN_M6_75.md item 4). The existence beside it is
            // a PLACEHOLDER here — like `status` above — because this value
            // is what the published ENTRY carries, and an entry's existence
            // is re-derived by every reply built from it. The reply this
            // function returns derives its own below.
            source_profile: source_profile.map(|profile| SourceProfile {
                id: profile.id,
                name: profile.name,
                existence: ProfileExistence::Present,
            }),
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
            .transition(&id, 0, Transition::ConfirmRunning { pane: pane.clone() })
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
            let reusable = reserved.dedup_scope() == Some(DedupScope::SessionLifetime);
            let mut error = anyhow::Error::new(RequestError::new(
                ErrorKind::Conflict,
                if reusable {
                    format!(
                        "session {} was deleted while it was being created, so the launch was \
                         torn back down; its bounded intent key is reusable",
                        truncate_for_error(&id)
                    )
                } else {
                    format!(
                        "session {} was deleted while it was being created, so the launch was \
                         torn back down; it will not be recreated under the same intent key",
                        truncate_for_error(&id)
                    )
                },
            ));
            // The PROCESS tree first, tmux second. Killing the tmux
            // session ends the pane's own process group and nothing else:
            // the agent's daemonized descendants — and, on a scoped launch,
            // everything in its cgroup — outlive it, and the row that could
            // have found them again is about to be gone for good. A failed
            // reap therefore rides the error rather than being swallowed;
            // there is no row left to retry from, so saying so is all this
            // path can still do for whoever reads the log.
            if let Err(sweep) = reap_process_tree(
                &self.seams.scopes,
                launch_scope.as_slice(),
                None,
                &id,
                &SweepTarget::AgentOnly,
            )
            .await
            {
                warn!(
                    session = %id, error = %format!("{sweep:#}"),
                    "could not reap the process tree of a create that raced a delete; \
                     something it started may still be running unlisted"
                );
                error = error.context(format!(
                    "additionally, the new agent's process tree could not be swept ({sweep:#}); \
                     something it started may still be running with no session record left"
                ));
            }
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
            // Same ordering, same reason as the delete race above: the tmux
            // kill reaches the pane's group and nothing beyond it, and this
            // path may still go on to remove the row.
            if let Err(sweep) = reap_process_tree(
                &self.seams.scopes,
                launch_scope.as_slice(),
                None,
                &id,
                &SweepTarget::AgentOnly,
            )
            .await
            {
                warn!(
                    session = %id, error = %format!("{sweep:#}"),
                    "could not reap the process tree of a create whose confirmation failed"
                );
                result = result.context(format!(
                    "additionally, the new agent's process tree could not be swept ({sweep:#})"
                ));
            }
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
                outcome: Arc::new(std::sync::Mutex::new(LastOutcome::Running)),
                snapshot,
                canonical_cwd: Some(canonical_cwd.clone()),
                // Nothing has been typed into this session yet, so capture
                // has no correlator to key on and correctly stays idle
                // until the input path supplies one.
                first_input: Arc::new(std::sync::Mutex::new(FirstInput {
                    at: None,
                    durable: true,
                })),
                capture: Arc::new(std::sync::Mutex::new(CaptureState::Unclaimed)),
                // What `spawn_agent` actually did to this launch's argv,
                // recorded on the entry that describes that launch. A
                // create publishes exactly once, so this is the only
                // moment the flag can be set for generation 0.
                hooked: hook_flag(hooked),
                hook_warned: hook_flag(false),
                // The agent has printed nothing this supervisor has looked
                // at yet; the ticker's next sample establishes the baseline
                // every later one is compared against.
                activity: ActivitySample::unsampled(),
                // Agrees with the reply going out above and with the row
                // just committed: all three say creation, because nothing
                // has been seen happening here yet.
                last_activity_at: activity_stamp(info.last_activity_at),
                // A create is a session's FIRST launch by definition
                // (`store::StoredSession::generation`); only a restart ever
                // moves this off zero.
                generation: 0,
                // Derived from the same selection the launching row
                // committed above, not from a fresh probe: the entry must
                // describe the launch that happened.
                scope: launch_scope,
            }),
        );
        // Derived HERE rather than reused from the pre-launch lookup, and
        // the gap is real: a launch is a tmux round trip plus two durable
        // writes, and a profile renamed or deleted while it ran would make
        // a `Present` copied from that lookup a stale answer on a reply
        // whose contract (`farhelm_proto::SourceProfile`) is that existence
        // describes the catalog AT REPLY TIME.
        //
        // The failure carries the session ID, and that is the load-bearing
        // part rather than politeness. This runs AFTER the session is
        // durable and published, so the caller is being told a create
        // failed that in fact succeeded; without the id in the message
        // there is nothing in the reply that could ever reach that session
        // again, and the obvious response — retry the create — starts a
        // SECOND agent in the same directory. An unkeyed create has no
        // reservation to reconcile it either, so the id is the only handle
        // that exists.
        let session_id = info.id.clone();
        self.with_derived_source_profile(info).await.with_context(|| {
            format!(
                "session {session_id} WAS created and is running; only describing which profile \
                 it came from failed, so this reply is withheld — attach to or delete that \
                 session rather than creating another",
                session_id = truncate_for_error(&session_id)
            )
        })
    }

    /// Re-derive `info`'s source-profile existence against the catalog as it
    /// stands right now (PLAN_M6_75.md item 5).
    ///
    /// The single-snapshot counterpart to `list_page`'s batched read: one
    /// reply describing one session costs one lookup by id, and a reply
    /// describing a page reads the whole catalog once instead. Both feed the
    /// same rule (`status::source_profile_existence`).
    ///
    /// A raw-created session costs NOTHING — no query is issued at all,
    /// which is what keeps this off the create and restart paths of every
    /// session that names no profile.
    ///
    /// The read is allowed to FAIL the reply, and deliberately so: an
    /// unreadable catalog cannot be degraded into "the profile is gone"
    /// without lying about a specific and alarming thing. Callers on a path
    /// that has already changed the world add the context saying so, since
    /// the failure describes the REPLY rather than the operation.
    async fn with_derived_source_profile(
        &self,
        mut info: SessionInfo,
    ) -> anyhow::Result<SessionInfo> {
        let Some(snapshotted) = info.source_profile else {
            return Ok(info);
        };
        let current = self
            .store
            .profile(&snapshotted.id)
            .await
            .context("reading the profile this session was created from")?;
        info.source_profile = Some(SourceProfile {
            existence: source_profile_existence(
                &snapshotted.name,
                current.map(|profile| profile.name).as_deref(),
            ),
            ..snapshotted
        });
        Ok(info)
    }

    /// Is `owner` the tmux session name of a session this supervisor
    /// currently holds in its map? This is the discriminator every
    /// lifecycle verb applies to a [`PaneProbe::ForeignOwner`], and it
    /// separates two situations the probe itself cannot tell apart but
    /// which have opposite safety profiles.
    ///
    /// **Known owner ⇒ the recorded pane's terminal is provably dead.**
    /// tmux pane ids are never reused within one server generation, so if
    /// the current server handed our recorded id to a session THIS
    /// supervisor launched, the recording must predate the current server
    /// — i.e. the server that owned the recorded pane is gone, and with it
    /// that pane, its terminal, and the agent that ran in it. This is the
    /// 2026-08-16 incident. Proceeding without a pane root and without
    /// stop-consent is then both safe and correct: there is no husk left
    /// to clean up, and any daemonized survivor of the dead run is the
    /// marker sweep's job, which SPEC.md already sanctions without
    /// consent.
    ///
    /// **Unknown owner ⇒ assume it may be OUR live terminal.** A pane
    /// whose session name we do not recognize is most cheaply explained by
    /// an out-of-band `rename-session` or `move-pane` on the private
    /// socket — in which case the recorded pane is still this session's
    /// running agent, merely answering to a different name. Every verb
    /// then has something to lose by proceeding: restart and delete would
    /// kill a live agent without consent, stop would record a plain exit
    /// that never happened, and delete/archive/close-tab would leave the
    /// renamed container, its scrollback, and its tabs behind while
    /// reporting success. So the verbs fail closed there, exactly as they
    /// did before the probe was made classifying.
    ///
    /// The residual blind spot, stated plainly: an external `move-pane`
    /// that moved our LIVE pane into another farhelm session's tmux
    /// session would present as the known-owner case and be treated as a
    /// recycle. Nothing farhelm does can produce that arrangement, and
    /// perfect defense against an operator deliberately rearranging their
    /// own private tmux socket is a non-goal — the fence here is against
    /// accident and against the incident shape, not against the account
    /// owner.
    ///
    /// Matches both the name a session's `Terminal` currently records and
    /// the name minted from its id ([`new_session_identity`]) — they are
    /// the same string in every ordinary state, but an entry caught
    /// between a relaunch's teardown and its republication has no recorded
    /// terminal while its tmux session name is still very much farhelm's.
    /// Either match establishes the same thing the proof needs: farhelm
    /// launched that session, on this server.
    pub(crate) async fn known_session_tmux_name(&self, owner: &str) -> bool {
        self.sessions.lock().await.iter().any(|(id, entry)| {
            format!("fh-{id}") == owner
                || entry
                    .terminal
                    .as_ref()
                    .is_some_and(|terminal| terminal.tmux_name == owner)
        })
    }

    /// Relaunch a session's agent in place — SPEC.md's restart, PLAN_M3.md
    /// item 9. The only relaunch mechanism there is: the resume offered
    /// when opening an interrupted session lands here too.
    ///
    /// ## Serialization
    ///
    /// The whole operation runs under this session's LIFECYCLE CLAIM
    /// ([`Supervisor::lifecycle_locks`]), which stop and delete take too.
    /// Without it, two restarts of one session interleave into a genuinely
    /// dangerous shape rather than a merely untidy one: both recheck
    /// liveness before either has stopped anything, both conclude "not
    /// running", and the second one's kill sweep — which is keyed on the
    /// session's environment marker, not on a pid — reaps the agent the
    /// FIRST one just launched, with no consent asked for killing it. The
    /// claim makes that sequence unconstructible, and makes restart-vs-stop
    /// and restart-vs-delete resolve to exactly one winner with the loser
    /// getting an honest answer instead of a half-torn-down session.
    ///
    /// ## What it refuses, and when
    ///
    /// Every refusal below runs before ANY of this operation's side effects
    /// — before the stop, before the sweep, before the generation — so a
    /// restart refused for one of these reasons leaves the session exactly
    /// as it was, stop annotation included. That is deliberately not the
    /// same claim as "nothing can fail after a change": a restart that gets
    /// past these and then fails has stopped an agent, and item 4's
    /// annotation promise is kept there by restoring the previous outcome
    /// (`SessionStore::abort_relaunch`), not by nothing having happened.
    ///
    /// - **`mode` against the CURRENT offer**, recomputed here rather than
    ///   trusted from the client's cached `SessionInfo`: capture can
    ///   upgrade a session from `FreshOnly` to `Resume` asynchronously
    ///   (item 8), so the offer the user was shown may already be stale.
    ///   A mismatch is a `Conflict` naming what the offer is NOW, which is
    ///   what lets a client refresh and re-present rather than retry blindly
    ///   (`ControlMsg::RestartSession`'s staleness contract). The check is
    ///   made ATOMIC with the relaunch by the capture pass being awaited —
    ///   a `CaptureReason::Reply` pass, so it cannot be answered by a
    ///   sweep that began before this request — and by the
    ///   generation claim being conditional on the same two fields the
    ///   validation read (`SessionStore::begin_relaunch`).
    /// - **A vanished or repointed working directory**, named in the error.
    ///   Existence uses the same check and wording a create uses
    ///   (`ensure_cwd_usable`); identity additionally requires the path to
    ///   still resolve to what it resolved to at create
    ///   (`ensure_cwd_identity`), so a symlink repointed between launches
    ///   cannot silently relaunch a permissive agent somewhere else. The
    ///   RESOLVED path is then what the relaunch is aimed at, rather than
    ///   the session's own spelling of it — otherwise the same link could
    ///   simply be repointed again between this check and the tmux call.
    /// - **A still-running agent without explicit consent.** Liveness is
    ///   RE-probed through the pane here; the client's `status` is only ever
    ///   a hint about whether to show a confirm dialog, never the
    ///   authorization to skip it (see `stop_if_running`'s wire docs). A
    ///   status of `Unknown` — no terminal, or a launch never confirmed —
    ///   is treated as possibly-alive for exactly this reason: the pane, not
    ///   the reported status, is what answers.
    ///
    /// ## What it does, in order
    ///
    /// 1. Stops the agent through the SHARED stop lifecycle
    ///    ([`stop_live_agent`]) when it is alive and the user consented, or
    ///    otherwise runs the marker sweep on its own — either way the prior
    ///    run's descendants (including daemons an already-exited agent left
    ///    behind) die BEFORE the new launch, never alongside it (SPEC.md:
    ///    "Restart reaps any leftover descendants of the prior run before
    ///    relaunching").
    /// 2. Hands off to [`Supervisor::relaunch`] for everything destructive,
    ///    on a supervisor-owned task: from the generation claim through
    ///    republication, the span must not be cancellable by the connection
    ///    that asked for it (see that method's docs).
    ///
    /// The captured conversation identity is RETAINED across a `Resume`
    /// relaunch and cleared for the others, along with the rest of the
    /// per-launch capture state; [`SessionStore::begin_relaunch`] carries
    /// the argument for that split.
    pub(crate) async fn restart_session(
        self: &Arc<Self>,
        session_id: &str,
        mode: RestartMode,
        stop_if_running: bool,
    ) -> anyhow::Result<SessionInfo> {
        // Taken FIRST, and released only when the whole restart is done —
        // including the republication. Lock order is lifecycle →
        // attachments → sessions (see the `Supervisor` struct's docs).
        let lifecycle = self.lifecycle_locks.claim(session_id).await;
        if !self.may_record() {
            // A supervisor with no standing to write cannot open a launch
            // generation, and launching without one is precisely the state
            // item 2's ordering rule exists to make impossible.
            return Err(RequestError::new(
                ErrorKind::Internal,
                "this supervisor is not recording session state (it does not hold the state \
                 directory's claim, or it could not read this host's boot id), so it will not \
                 relaunch a session it cannot durably account for",
            )
            .into());
        }
        // A `Reply` pass, exactly like the list path's: this is about to
        // validate the requested mode against the session's offer, and a
        // pass that began before this request cannot answer for it — a
        // sweep still in flight may be one commit away from changing that
        // offer, and validating against the pre-commit answer is the
        // staleness this whole contract exists to exclude. A restart is a
        // rare, user-initiated operation; waiting out one filesystem scan
        // is free.
        self.capture_now().await;
        let entry = self.sessions.lock().await.get(session_id).cloned();
        let Some(entry) = entry else {
            return Err(RequestError::new(
                ErrorKind::NotFound,
                format!("no such session: {}", truncate_for_error(session_id)),
            )
            .into());
        };
        // Read from the STORE rather than the in-memory mirror: a restart
        // resumes what SURVIVED (`session_snapshot`'s own docs), and the
        // filled resume argv is built there from the durable identity.
        let snapshot = self
            .session_snapshot(session_id)
            .await
            .context("reading this session's durable snapshot for a restart")?;
        let Some(snapshot) = snapshot else {
            return Err(RequestError::new(
                ErrorKind::NotFound,
                format!("no such session: {}", truncate_for_error(session_id)),
            )
            .into());
        };
        let argv = relaunch_argv(mode, &snapshot, &entry.info.invocation)?;
        ensure_cwd_usable(&entry.info.cwd).await?;
        // The VERIFIED path travels with the relaunch rather than being
        // discarded once the comparison passes: `relaunch_into_terminal`
        // hands it to tmux, so a symlink repointed between here and there
        // cannot aim the new agent somewhere nothing checked. `None` only
        // for a row with no recorded identity — see `ensure_cwd_identity`.
        let launch_cwd = ensure_cwd_identity(&entry.info.cwd, snapshot.canonical_cwd.as_deref())
            .await?
            .unwrap_or_else(|| entry.info.cwd.clone());

        // Liveness as it is NOW, from the pane rather than from anything a
        // client cached. A pane query that FAILS is not read as "not
        // running": that would be the one direction that can kill nothing
        // and relaunch beside a live agent.
        let pane_state = match entry.terminal.as_ref() {
            Some(terminal) => match self
                .tmux
                .pane_process(&terminal.tmux_name, &terminal.pane)
                .await
                .context("rechecking whether this session's agent is still running")?
            {
                PaneProbe::Owned(pane) => Some(pane),
                PaneProbe::Gone => None,
                // A RECOGNIZED owner reads exactly as `Gone`, and the
                // consent question below is why that is not merely a
                // convenience. Pane ids are unique within a server
                // generation, so a session farhelm launched holding our
                // recorded id proves the recording predates the current
                // server — this session's agent died with the server that
                // owned its pane. There is therefore no live agent for
                // `stop_if_running` to protect, and the else-branch's
                // survivors hunt (marker-keyed, not pane-keyed) is the
                // no-consent reaping SPEC.md already assigns to a restart
                // following an agent that exited on its own. The second
                // half of `None` matters too: it makes `terminal_survives`
                // false below, so the relaunch builds a fresh terminal
                // instead of a reuse that could not have worked. Logged
                // loudly because reaching it means a tmux server died
                // under a running supervisor.
                //
                // An UNRECOGNIZED owner gets the pre-classification
                // behavior back: the recheck fails and the restart never
                // starts. The recorded pane may be this session's own
                // agent under a renamed tmux session, and a restart that
                // guessed otherwise would kill a live agent without the
                // consent `stop_if_running` exists to obtain.
                PaneProbe::ForeignOwner { owner } => {
                    if !self.known_session_tmux_name(&owner).await {
                        return Err(anyhow::anyhow!(unknown_pane_owner_refusal(
                            &terminal.pane,
                            &owner,
                            &terminal.tmux_name
                        ))
                        .context("rechecking whether this session's agent is still running"));
                    }
                    warn!(
                        session = %session_id, foreign_owner = %owner,
                        "this session's recorded pane now belongs to another tmux session, so \
                         the restart treats its terminal as gone and creates a fresh one"
                    );
                    None
                }
            },
            None => None,
        };
        let alive_pane = pane_state.filter(|pane| !pane.dead);
        // Captured BEFORE the kill, exactly as `StopSession` captures its
        // own snapshot and for the same reason: an alternate-screen app's
        // frame is gone the moment its process is, so the only chance to
        // carry it into the relaunched terminal is now. Skipped entirely
        // when nothing is alive to be on the alternate screen — the
        // dead-pane case is handled by the relaunch plan itself, which can
        // still read a retained alt grid off the dead pane.
        let live_frame = match (entry.terminal.as_ref(), alive_pane) {
            (Some(terminal), Some(_)) => {
                capture_alt_screen_before_stop(self, session_id, terminal).await
            }
            _ => None,
        };
        if let Some(pane) = alive_pane {
            if !stop_if_running {
                return Err(RequestError::new(
                    ErrorKind::Conflict,
                    "this session's agent is still running; restarting it stops the agent and \
                     its whole process tree first, so confirm stopping it and send the restart \
                     again with that consent",
                )
                .into());
            }
            if let Err(failure) = stop_live_agent(self, session_id, &entry, Some(pane.pid)).await {
                match failure {
                    // The tree IS stopped and only the bookkeeping is
                    // behind — and the outcome it failed to write is one
                    // this restart is about to replace with a new
                    // generation anyway. Proceeding is strictly better
                    // than refusing a restart whose only casualty is a
                    // record about to be overwritten.
                    StopFailure::UnrecordedOutcome(e) => warn!(
                        session = %session_id, error = %format!("{e:#}"),
                        "could not record the stop that preceded this restart; the relaunch's \
                         own generation replaces that record regardless"
                    ),
                    // Nothing was killed, or the sweep could not confirm
                    // it: relaunching now would risk exactly the
                    // "alongside, not after" the SPEC forbids.
                    failure => {
                        return Err(RequestError::new(
                            ErrorKind::Internal,
                            format!("not relaunching: {}", failure.message()),
                        )
                        .into());
                    }
                }
            }
        } else {
            // SPEC.md: an agent exiting on its own does not trigger a hunt
            // for daemonized survivors — the session's next restart does.
            // This is that hunt, and it runs before the new launch rather
            // than beside it. The PRIOR run's scope (`entry.scope`, still
            // the pre-relaunch generation's here) is what those survivors
            // would be in — a scope outlives its main process for exactly
            // as long as something it spawned is still alive.
            // `AgentOnly` for the same reason `stop_live_agent` uses it:
            // SPEC.md says restart touches the agent terminal only, so a
            // hunt for the PRIOR run's survivors must not reap a tab's
            // shell that has been happily running across the restart.
            reap_process_tree(
                &self.seams.scopes,
                entry.scope.as_slice(),
                None,
                session_id,
                &SweepTarget::AgentOnly,
            )
            .await
            .context("reaping the prior run's leftover descendants before relaunching")?;
        }

        // Everything from here on is destructive, and runs on a task this
        // SUPERVISOR owns rather than the connection's. See
        // `Supervisor::relaunch` for why cancellation must not reach it;
        // awaiting the handle is itself cancellable, so a client that
        // disconnects mid-restart loses only its reply.
        let sup = Arc::clone(self);
        let entry_for_task = Arc::clone(&entry);
        // "Survives" means the recorded pane is still THIS session's to
        // respawn into. A recognized foreign owner already collapsed to
        // `None` above precisely so it lands here as "did not survive" —
        // not because reuse would be dangerous (it could not even happen:
        // `relaunch_in_pane` targets `pane_in_session(session, pane)`, and
        // tmux rejects a mismatched session/pane pairing atomically, see
        // that method's docs), but because reuse would be a GUARANTEED
        // failure. Saying "did not survive" is what lets a fresh terminal
        // be created instead; the driver's session-paired target remains
        // the cross-session safety barrier either way.
        let terminal_survives = pane_state.is_some();
        let relaunch = tokio::spawn(async move {
            // The claim MOVES into the task with the work it protects.
            // Holding it in the caller instead would release it the moment
            // a disconnecting client cancelled the await below — while
            // this task ran on — and the next restart, stop, or delete
            // would walk straight into the span this claim exists to keep
            // them out of.
            let _lifecycle = lifecycle;
            sup.relaunch(
                &entry_for_task,
                &snapshot,
                mode,
                argv,
                launch_cwd,
                terminal_survives,
                live_frame,
            )
            .await
        });
        match relaunch.await {
            Ok(result) => result,
            Err(join) => Err(anyhow::anyhow!(
                "the relaunch task for session {} did not complete: {join}",
                truncate_for_error(session_id)
            )),
        }
    }

    /// The destructive half of [`Supervisor::restart_session`]: open the
    /// new launch generation, take the session off the map, relaunch, and
    /// republish — or put everything back.
    ///
    /// ## Why this runs on a supervisor-owned task
    ///
    /// From the generation claim to the republication, this span leaves the
    /// session in states nothing else can resolve: a durable `Launching`
    /// row, no in-memory entry (so stop and delete answer `NotFound`), and
    /// possibly a freshly spawned agent. The connection's request handlers
    /// are tracked in a `JoinSet` that `handle_connection` ABORTS at
    /// shutdown (`HANDLER_SHUTDOWN_TIMEOUT`), so a client that disconnects
    /// at the wrong moment could otherwise cancel this mid-span and strand
    /// exactly that state — an agent running under a session no longer in
    /// the map, with the entry never restored. Spawning here means
    /// cancellation can only ever reach the AWAIT in the caller, never the
    /// work: the task holds its own `Arc<Supervisor>` and runs to
    /// completion regardless of who is still listening.
    ///
    /// ## Restoring on failure
    ///
    /// A failure that is DEFINITIVE about having changed nothing outside
    /// this process puts the previous run's outcome back
    /// ([`SessionStore::abort_relaunch`]) — that is what makes item 4's
    /// "only a successful restart clears the annotation" true rather than
    /// merely intended. A failure that is AMBIGUOUS about a spawned agent
    /// leaves the `Launching` row alone for reload to reconcile: restoring
    /// "exited, stopped by user" over a session that may be running would
    /// be a worse lie than an honest "unknown".
    ///
    /// A failure that arrives AFTER the new generation was published
    /// ([`RelaunchDisposition::Published`]) restores nothing at all. The
    /// restoration below rebuilds the entry from the PRE-restart one, so
    /// running it over a completed publication would replace a live terminal
    /// with the one it superseded — a fresh-terminal restart would lose its
    /// new pane to a reply-building error, which no failure to describe a
    /// session has any standing to do.
    #[allow(clippy::too_many_arguments)]
    async fn relaunch(
        self: &Arc<Self>,
        entry: &Arc<SessionEntry>,
        snapshot: &SessionSnapshot,
        mode: RestartMode,
        argv: Vec<String>,
        launch_cwd: String,
        terminal_survives: bool,
        live_frame: Option<Vec<u8>>,
    ) -> anyhow::Result<SessionInfo> {
        let id = entry.info.id.clone();
        // A relaunch that is not resuming a captured identity opens a FRESH
        // capture window: `first_input_at` and the correlation verdict
        // belong to one run, not to the session (see
        // `SessionStore::begin_relaunch`). `Resume` keeps them, because
        // reverifying the identity it is resuming is exactly what the
        // capture pass must go on doing.
        let reset_capture = mode != RestartMode::Resume;
        let claim = self
            .store
            .begin_relaunch(
                &id,
                crate::store::OfferBasis {
                    captured_conversation: snapshot.captured_conversation.clone(),
                    capture_ambiguous: snapshot.capture_ambiguous,
                },
                reset_capture,
                // Re-evaluated here rather than inherited from the run
                // being replaced (PLAN_M3.md item 10): the selection is a
                // fact about a LAUNCH, and a restart is a new launch — on a
                // host that has gained or lost its user manager since, the
                // previous run's answer is simply the wrong one.
                self.scope_selected(&id).await,
            )
            .await
            .context("opening a new launch generation for this restart")?;
        let claim = match claim {
            crate::store::RelaunchDecision::Claimed(claim) => claim,
            crate::store::RelaunchDecision::OfferChanged => {
                return Err(RequestError::new(
                    ErrorKind::Conflict,
                    "this session's restart offer changed while the restart was being \
                     prepared (its conversation identity was just captured, or its \
                     correlation was just found ambiguous); nothing was relaunched — refresh \
                     the session and re-present the offer",
                )
                .into());
            }
            crate::store::RelaunchDecision::Gone => {
                return Err(RequestError::new(
                    ErrorKind::Conflict,
                    format!(
                        "session {} was deleted while its restart was being prepared, so \
                         nothing was relaunched",
                        truncate_for_error(&id)
                    ),
                )
                .into());
            }
        };
        // Off the map for the duration. The LIFECYCLE CLAIM the caller
        // holds is what actually keeps stop and delete out of this window
        // — they queue behind it rather than seeing a missing entry — and
        // this removal is the belt to that suspenders: nothing can install
        // an attachment on, or tear down, a session whose terminal is
        // being replaced. The visible cost is that a `ListSessions`
        // landing inside this window omits the session entirely: accepted,
        // because the window is a couple of tmux round trips long, and
        // publishing an entry whose pane is mid-respawn would be worse
        // than briefly publishing none.
        self.sessions.lock().await.remove(&id);
        // Whatever is attached is attached to the PREVIOUS run: the pane is
        // about to be respawned under it (or replaced outright), so the
        // client is told to reattach rather than left watching a stream
        // whose meaning changed underneath it.
        let relaunched = match self.detach_for_restart(&id).await {
            Ok(()) => {
                self.relaunch_into_terminal(
                    entry,
                    claim.generation,
                    launch_scope_unit(&id, claim.generation, claim.scoped),
                    argv,
                    launch_cwd,
                    terminal_survives,
                    live_frame,
                    reset_capture,
                )
                .await
            }
            Err(error) => Err(RelaunchFailure::definitive(
                error.context("detaching the previous run's terminal output"),
            )),
        };
        match relaunched {
            Ok(info) => Ok(info),
            // The relaunch got as far as publishing its new generation and
            // then failed to describe it. Everything below would UNDO that
            // publication — republishing the pre-restart entry, terminal
            // included — so it is skipped outright; see
            // [`RelaunchDisposition::Published`].
            Err(failure) if failure.disposition == RelaunchDisposition::Published => {
                Err(failure.error)
            }
            Err(failure) => {
                // Definitive from here on means exactly one thing, since
                // the published case returned above.
                let definitive = failure.disposition == RelaunchDisposition::Definitive;
                // Which selection the re-published entry describes follows
                // the same rule the outcome does: on a DEFINITIVE failure
                // `abort_relaunch` just put the previous run's selection
                // back in the row, so the entry must agree with it; on an
                // ambiguous one the new generation's stands, because an
                // agent may genuinely be running in its scope.
                //
                // Note what the derivation then names on the definitive
                // path: the ABANDONED generation's unit, which by
                // construction was never created (that is what makes the
                // failure definitive), so a later stop finds no unit and
                // falls through to the sweep. That is not a lost kill — the
                // PREVIOUS run's own scope was already reaped before this
                // generation was ever opened, by the stop or the leftover
                // reap the restart performs first.
                let scope = launch_scope_unit(
                    &id,
                    claim.generation,
                    if definitive {
                        claim.prior.scoped
                    } else {
                        claim.scoped
                    },
                );
                if definitive {
                    // Nothing outside this process changed, so the previous
                    // run's outcome is still the truth about this session —
                    // annotation, exit code and all.
                    match self
                        .store
                        .abort_relaunch(&id, claim.generation, &claim.prior)
                        .await
                    {
                        Ok(_) => {
                            *entry.outcome.lock().expect("outcome mutex poisoned") =
                                claim.prior.outcome.clone();
                        }
                        Err(e) => warn!(
                            session = %id, error = %format!("{e:#}"),
                            "could not restore the outcome this failed restart replaced; the \
                             session lists as unknown until it is restarted again"
                        ),
                    }
                } else {
                    // An agent may be running under the new generation.
                    // `Launching` is the honest record for that, and reload
                    // reconciles it against what it can actually find.
                    *entry.outcome.lock().expect("outcome mutex poisoned") = LastOutcome::Launching;
                }
                // The entry goes back — with the generation it now has, so
                // nothing published under it can write against a
                // generation the store has moved past — UNLESS the session
                // is gone, which is one of the ways a relaunch fails
                // (a delete committed while this was in flight). Putting an
                // entry back for a deleted row would resurrect the session
                // in the list with nothing durable behind it, and every
                // later operation on it would fail in a more confusing
                // place than this one.
                let still_exists = match self.store.session(&id).await {
                    Ok(row) => row.is_some(),
                    // Unknown is treated as "still there": losing a live
                    // session from the map costs its terminal, its stop and
                    // its delete, while keeping a doomed entry costs one
                    // confusing row until the next reload.
                    Err(e) => {
                        warn!(
                            session = %id, error = %format!("{e:#}"),
                            "could not confirm whether this session still exists after a \
                             failed restart; keeping its entry"
                        );
                        true
                    }
                };
                if still_exists {
                    let mut recovered_info = entry.info.clone();
                    // An ambiguous launch may be running. Keeping the
                    // archived flag would hide the only row that can be
                    // used to stop, inspect, or restart it.
                    recovered_info.archived =
                        recovered_archive_flag(definitive, claim.prior.archived);
                    self.sessions.lock().await.insert(
                        id.clone(),
                        relaunched_entry(
                            entry,
                            recovered_info,
                            entry.terminal.clone(),
                            claim.generation,
                            scope,
                            entry
                                .outcome
                                .lock()
                                .expect("outcome mutex poisoned")
                                .clone(),
                            reset_capture,
                        ),
                    );
                }
                Err(failure.error)
            }
        }
    }

    /// The launch itself, once the generation is claimed and the session is
    /// off the map: publish this launch's spec, hand it to tmux (into the
    /// surviving pane, or a fresh session), confirm it durably, and
    /// republish the entry.
    ///
    /// `launch_cwd` is the directory the caller VERIFIED against this
    /// session's recorded identity ([`ensure_cwd_identity`]), which is what
    /// tmux is given — never `entry.info.cwd`, whose symlinks could be
    /// repointed between that check and this launch.
    ///
    /// `terminal_survives` is the caller's own pane probe: `true` means the
    /// pane still exists (alive or dead-but-retained) and the relaunch
    /// respawns into it, keeping the prior run above in scrollback
    /// (SPEC.md); `false` means the terminal is gone — an interrupted
    /// session, or one whose tmux server died — and a fresh one is built.
    ///
    /// Every failure says whether it is DEFINITIVE (nothing outside this
    /// process changed) so the caller knows whether the previous outcome
    /// can be restored; see [`RelaunchFailure`].
    #[allow(clippy::too_many_arguments)]
    async fn relaunch_into_terminal(
        &self,
        entry: &SessionEntry,
        generation: i64,
        scope: Option<String>,
        argv: Vec<String>,
        launch_cwd: String,
        terminal_survives: bool,
        live_frame: Option<Vec<u8>>,
        reset_capture: bool,
    ) -> Result<SessionInfo, RelaunchFailure> {
        let id = entry.info.id.clone();
        let terminal = entry.terminal.as_ref();
        // The session's tabs, captured BEFORE anything destructive and
        // carried through to the reply. A restart touches the agent
        // terminal alone (SPEC.md), so the tabs it does not touch are
        // exactly these; rediscovering them AFTERWARDS and reporting `[]`
        // on a query failure would have a reply claim a session lost its
        // tabs when the restart never went near them. The lifecycle claim
        // this runs under is what makes "before" and "after" the same
        // list. A failure to read it refuses the restart outright, which
        // is safe here precisely because nothing has happened yet.
        let tabs = match terminal {
            Some(terminal) => self.session_tabs(terminal).await.map_err(|e| {
                RelaunchFailure::definitive(
                    e.context("reading this session's terminal tabs before relaunching it"),
                )
            })?,
            // No terminal means no tmux session, and therefore no windows
            // for a tab to be.
            None => Vec::new(),
        };
        let tmux_name = match terminal {
            Some(terminal) => terminal.tmux_name.clone(),
            // A session whose terminal did not survive keeps the tmux name
            // its row was created with — read back rather than re-derived,
            // because the name is part of the session's durable identity
            // (create reservations reconcile against exactly this string)
            // and a relaunch is the same session, not a new one.
            None => match self.store.session(&id).await {
                Ok(Some(row)) => row.tmux_name,
                Ok(None) => {
                    return Err(RelaunchFailure::definitive(anyhow::anyhow!(
                        "session {} vanished between opening its launch generation and \
                         relaunching it",
                        truncate_for_error(&id)
                    )));
                }
                Err(e) => {
                    return Err(RelaunchFailure::definitive(
                        e.context("reading this session's tmux name for a relaunch"),
                    ));
                }
            },
        };
        // Read every fallible durable launch input before removing a
        // snapshot, killing a tmux husk, or shrinking a reusable pane. A
        // credential read failure must leave the previous terminal exactly
        // as it was rather than strand the temporary relaunch geometry.
        let session_token = match self.store.session_token(&id).await {
            Ok(Some(token)) => token,
            Ok(None) => {
                return Err(RelaunchFailure::definitive(anyhow::anyhow!(
                    "session {} vanished before its spawn credential could be read",
                    truncate_for_error(&id)
                )));
            }
            Err(error) => {
                return Err(RelaunchFailure::definitive(
                    error.context("reading this session's spawn credential for a relaunch"),
                ));
            }
        };
        // A snapshot stored for the PREVIOUS run must not survive into this
        // one: `Attach`'s dead-pane replay would otherwise show the old
        // run's last screen as if it were the new run's, the moment the new
        // agent exits. Fail-closed for the same reason the launch artifacts
        // are — a file this process could not remove is one a later replay
        // will happily read.
        if let Err(e) = remove_fail_closed(
            &snapshot_path(&self.state_dir, &id),
            "the previous run's alt-screen snapshot",
        )
        .await
        {
            return Err(RelaunchFailure::definitive(anyhow::anyhow!(
                "not relaunching session {}: {e}",
                truncate_for_error(&id)
            )));
        }
        let reuse = terminal.filter(|_| terminal_survives);
        if reuse.is_none() {
            // The pane this session knew is gone, but a tmux session under
            // its name can still exist (a pane killed on its own, a server
            // this supervisor lost track of). `new-session` would refuse
            // the duplicate name, so the husk is torn down first — this is
            // the same "the terminal is gone" case delete would clean up,
            // reached from the other direction.
            if let Err(e) = self.tmux.kill_session(&tmux_name).await {
                return Err(RelaunchFailure::definitive(e.context(
                    "clearing this session's leftover tmux session before relaunching",
                )));
            }
        }
        // The prior run's last screen, for the cases tmux's own respawn
        // cannot carry across (see `TmuxDriver::plan_pane_relaunch`). A
        // frame captured before the kill wins over one read off the dead
        // pane now: it is the same content, but taken while the app was
        // still there to have it.
        let plan = match reuse {
            Some(terminal) => {
                self.tmux
                    .plan_pane_relaunch(
                        &terminal.tmux_name,
                        &terminal.pane,
                        MAX_ALT_SCREEN_SNAPSHOT_BYTES,
                    )
                    .await
            }
            None => crate::tmux::PaneRelaunchPlan {
                restore: None,
                carry_over: None,
            },
        };
        let preamble = live_frame.or(plan.carry_over);
        let spawned = self
            .spawn_agent(
                &id,
                &session_token,
                generation,
                &tmux_name,
                argv,
                // The session's own immutable snapshot: a restart is a new
                // generation of the SAME session, so the kind that decides
                // the hook flags is the one the session was created with,
                // never re-derived from this relaunch's argv (which for a
                // resume is `claude --resume <id>`, not the original
                // invocation at all).
                &entry.snapshot,
                // The path `restart_session` VERIFIED against this
                // session's recorded identity, not `entry.info.cwd` — see
                // `ensure_cwd_identity` for the check-then-repoint window
                // that closes.
                &launch_cwd,
                RELAUNCH_COLS,
                RELAUNCH_ROWS,
                reuse,
                preamble,
                scope.as_deref(),
            )
            .await;
        // Restored whatever the spawn did: this window was shrunk by the
        // plan above, and leaving it one row tall because the launch failed
        // would be a second, unrelated injury.
        //
        // Addressed through the pane, like every window-scoped command
        // since tabs (PLAN_M4.md item 2): a bare session target names the
        // session's CURRENT window, which a tab can be. `plan.restore` is
        // only ever `Some` on the reuse path, where `reuse` names the very
        // pane whose window `plan_pane_relaunch` shrank — and `respawn-pane`
        // keeps that pane id across the relaunch, so it is still the right
        // handle here.
        if let Some((cols, rows)) = plan.restore
            && let Some(reused) = reuse
            && let Err(e) = self
                .tmux
                .resize_window(&tmux_name, &reused.pane, cols, rows)
                .await
        {
            warn!(
                session = %id, error = %format!("{e:#}"),
                "could not restore this window's size after a relaunch; the next attach's \
                 own resize will correct it"
            );
        }
        let Spawned {
            pane,
            spec_path,
            hooked,
            ..
        } = match spawned {
            Ok(spawned) => spawned,
            Err(SpawnFailure::Spec(error)) => {
                return Err(RelaunchFailure::definitive(
                    error.context("publishing this restart's launch spec"),
                ));
            }
            Err(SpawnFailure::Tmux { spec_path, error }) => {
                return Err(self
                    .unwind_failed_relaunch(
                        &id,
                        &tmux_name,
                        scope.as_deref(),
                        reuse,
                        &spec_path,
                        error,
                    )
                    .await);
            }
        };
        // Launch confirmed: the pane exists, so the durable record moves
        // from launching to running and gains the pane it could not know
        // before — the same transition a create commits, for the same
        // reason, and fenced on this launch's own generation so a racing
        // observer cannot have moved it first.
        let confirmed = self
            .store
            .transition(
                &id,
                generation,
                Transition::ConfirmRunning { pane: pane.clone() },
            )
            .await;
        let confirmed = match confirmed {
            Ok(confirmed) => confirmed,
            Err(e) => {
                // The agent IS running and this process could not record
                // it. Publishing the new terminal as `Launching` keeps the
                // session reachable — attachable, stoppable, deletable —
                // which is strictly better than reporting a failure that
                // leaves an untracked agent behind; the next list or reload
                // confirms what this write could not.
                warn!(
                    session = %id, error = %format!("{e:#}"),
                    "could not confirm this relaunch durably; publishing it as launching so \
                     the agent stays reachable"
                );
                self.publish_relaunched(
                    entry,
                    generation,
                    Relaunched {
                        terminal: Terminal {
                            tmux_name: tmux_name.clone(),
                            pane: pane.clone(),
                        },
                        scope: scope.clone(),
                        outcome: LastOutcome::Launching,
                        reset_capture,
                        hooked,
                        tabs: tabs.clone(),
                    },
                )
                .await;
                // PUBLISHED, not merely ambiguous: the entry above is on
                // the map with the new terminal and generation, and the
                // generic recovery would immediately replace it with the
                // pre-restart one — throwing away the very reachability
                // this branch just went out of its way to preserve.
                return Err(RelaunchFailure::published(e.context(
                    "confirming the relaunch in the database; the agent is running and the \
                     session lists as unknown until the next observation",
                )));
            }
        };
        match confirmed {
            Some(LastOutcome::Running) => {}
            // The row is GONE: a delete resolved this session's entry
            // before the removal above and committed while the relaunch was
            // mid-flight. The delete wins — it was a deliberate action
            // against a session that existed when it was issued — so this
            // relaunch tears its own work back down rather than leaving an
            // agent running with no row that knows about it.
            None => {
                let mut teardown = Vec::new();
                if let Err(e) = reap_process_tree(
                    &self.seams.scopes,
                    scope.as_slice(),
                    None,
                    &id,
                    &SweepTarget::AgentOnly,
                )
                .await
                {
                    teardown.push(format!("the new agent's process tree ({e:#})"));
                }
                if let Err(e) = self.tmux.kill_session(&tmux_name).await {
                    teardown.push(format!("its tmux session {tmux_name} ({e:#})"));
                }
                for (path, what) in [
                    (&spec_path, "launch spec"),
                    (
                        &crate::launch::status_path_for_spec(&spec_path),
                        "launch sentinel",
                    ),
                ] {
                    if let Err(e) = remove_fail_closed(path, what).await {
                        teardown.push(e);
                    }
                }
                let message = if teardown.is_empty() {
                    format!(
                        "session {} was deleted while it was being relaunched, so the new \
                         agent was torn back down",
                        truncate_for_error(&id)
                    )
                } else {
                    // Never "was torn back down" over a failure: a caller
                    // reading that would believe nothing survives, and
                    // something does.
                    format!(
                        "session {} was deleted while it was being relaunched, and the new \
                         agent could NOT be fully torn back down: {}",
                        truncate_for_error(&id),
                        teardown.join("; ")
                    )
                };
                return Err(RelaunchFailure::ambiguous(
                    RequestError::new(ErrorKind::Conflict, message).into(),
                ));
            }
            // Anything else means another writer moved this generation's
            // outcome between the spawn and this line — a lost race rather
            // than a success. Published as what actually committed, never
            // as a fabricated `Running`.
            Some(other) => {
                warn!(
                    session = %id, outcome = ?other,
                    "this relaunch's confirmation lost a race; publishing what committed"
                );
                let info = self
                    .publish_relaunched(
                        entry,
                        generation,
                        Relaunched {
                            terminal: Terminal {
                                tmux_name: tmux_name.clone(),
                                pane: pane.clone(),
                            },
                            scope,
                            outcome: other,
                            reset_capture,
                            hooked,
                            tabs,
                        },
                    )
                    .await;
                return self.restart_reply(info).await;
            }
        }

        info!(session = %id, tmux = %tmux_name, %pane, reused = reuse.is_some(), "session restarted");
        let info = self
            .publish_relaunched(
                entry,
                generation,
                Relaunched {
                    terminal: Terminal { tmux_name, pane },
                    scope,
                    outcome: LastOutcome::Running,
                    reset_capture,
                    hooked,
                    tabs,
                },
            )
            .await;
        self.restart_reply(info).await
    }

    /// Unwind a relaunch whose tmux command failed — but only once it is
    /// established that the launch did NOT take.
    ///
    /// tmux failures are ambiguous in a way the spec write is not: a
    /// respawn (or a `new-session`) can fail after tmux has already applied
    /// it, so removing this launch's spec on the strength of the error
    /// alone would leave a REAL agent running with the shim's own spec
    /// deleted underneath it — which the shim then reports as a launch
    /// failure, converting a live session into a fabricated exec error.
    /// This probes first, exactly as the create path does, and only cleans
    /// up on confirmed absence.
    ///
    /// `scope` is the FAILED launch's own cgroup unit, not the previous
    /// run's: `systemd-run` can have created the scope and placed processes
    /// in it before whatever made tmux fail, so the sweep this performs
    /// must be able to reach them.
    async fn unwind_failed_relaunch(
        &self,
        id: &str,
        tmux_name: &str,
        scope: Option<&str>,
        reuse: Option<&Terminal>,
        spec_path: &Path,
        error: anyhow::Error,
    ) -> RelaunchFailure {
        let error = error.context(format!(
            "relaunching session {} in tmux",
            truncate_for_error(id)
        ));
        // For a reused pane the question is "is a process running in it
        // now"; for a fresh terminal it is "does the session exist". Either
        // answer being unavailable is itself ambiguity — hence the inner
        // `Option`: `None` is "tmux answered, but not about our pane".
        let applied = match reuse {
            Some(terminal) => self
                .tmux
                .pane_process(&terminal.tmux_name, &terminal.pane)
                .await
                .map(|probe| match probe {
                    PaneProbe::Owned(pane) => Some(!pane.dead),
                    PaneProbe::Gone => Some(false),
                    // Ambiguous for EITHER kind of foreign owner, which is
                    // why this is the one site that does not ask
                    // `known_session_tmux_name` which kind it has. The
                    // lifecycle verbs ask "is this session's agent
                    // running", and a recognized owner answers that no;
                    // here the question is "did the respawn we just issued
                    // into THIS pane apply", and nothing about the owner's
                    // identity answers it — the respawn may have landed
                    // before whatever rearranged ownership, whichever
                    // rearrangement it was. Unwinding on a guess would
                    // delete a live agent's spec out from under it, so
                    // this stays conservative and keeps the launching
                    // record.
                    PaneProbe::ForeignOwner { .. } => None,
                }),
            None => self.tmux.has_session(tmux_name).await.map(Some),
        };
        match applied {
            Ok(Some(false)) => {}
            Ok(Some(true)) => {
                return RelaunchFailure::ambiguous(error.context(
                    "tmux reports a live process for this session despite the failure, so the \
                     launch may have taken; it is kept as a launching record rather than \
                     unwound",
                ));
            }
            Ok(None) => {
                return RelaunchFailure::ambiguous(error.context(
                    "the pane this relaunch reused now belongs to a different tmux session, so \
                     whether the launch took cannot be established; it is kept as a launching \
                     record rather than unwound",
                ));
            }
            Err(probe) => {
                return RelaunchFailure::ambiguous(error.context(format!(
                    "could not determine whether the relaunch took ({probe:#}), so it is kept \
                     as a launching record rather than unwound"
                )));
            }
        }
        // Confirmed absent: nothing is running under this launch, so its
        // spec — which holds the agent's full command line, credentials
        // included — must not be left for nothing to consume. A removal
        // that itself fails is reported rather than swallowed; the file is
        // this launch's only leftover, and silence about it is what turns
        // hygiene into a leak.
        let mut error = error;
        if let Err(cleanup) = remove_fail_closed(spec_path, "the failed relaunch's spec").await {
            error = error.context(cleanup);
        }
        // The marker sweep, not just tmux: a launch that got far enough to
        // start the login shell can have left descendants even though tmux
        // now reports nothing running.
        if let Err(sweep) = reap_process_tree(
            &self.seams.scopes,
            scope.map(str::to_string).as_slice(),
            None,
            id,
            &SweepTarget::AgentOnly,
        )
        .await
        {
            return RelaunchFailure::ambiguous(error.context(format!(
                "and the failed launch's process tree could not be swept ({sweep:#})"
            )));
        }
        RelaunchFailure::definitive(error)
    }

    /// Finish a successful relaunch's reply by deriving its source-profile
    /// existence against the catalog as it stands now (PLAN_M6_75.md item
    /// 4).
    ///
    /// Separate from [`Supervisor::publish_relaunched`] because publication
    /// must not be able to fail: the new entry is already on the map and the
    /// agent is already running by the time this reads anything. So a failed
    /// catalog read is reported as a PUBLISHED relaunch failure whose
    /// message says the restart itself succeeded — the same shape, and the
    /// same reasoning, as the confirm-write failure above it: the caller is
    /// told what it must not assume, and the next list describes the session
    /// correctly regardless.
    ///
    /// [`RelaunchDisposition::Published`] rather than `ambiguous`, and the
    /// difference is not bookkeeping. An ambiguous failure runs the generic
    /// recovery, which republishes the entry built from the PRE-restart one
    /// — so a catalog read failing here used to overwrite the new terminal
    /// with the terminal the restart had just replaced, and report a
    /// `Launching` outcome over an agent that was confirmed running. On a
    /// fresh-terminal restart that loses the live terminal entirely, which
    /// contradicts SPEC.md's restart guarantee for a reason that has nothing
    /// to do with the restart.
    async fn restart_reply(&self, info: SessionInfo) -> Result<SessionInfo, RelaunchFailure> {
        let session_id = info.id.clone();
        self.with_derived_source_profile(info).await.map_err(|e| {
            RelaunchFailure::published(e.context(format!(
                "the restart of session {session_id} SUCCEEDED and its new generation is \
                 published and running; only describing which profile it was created from \
                 failed, so this reply is withheld — the next list reports the restarted \
                 session normally, and restarting again would kill the agent this one started",
                session_id = truncate_for_error(&session_id)
            )))
        })
    }

    /// Put a relaunched session back on the map under its NEW generation,
    /// and build the reply that describes it.
    ///
    /// A new `SessionEntry` rather than a mutated one, which is the whole
    /// mechanism behind [`SessionEntry::generation`]: anything still
    /// holding the previous `Arc` is holding a description of the previous
    /// run, and every durable write it attempts is fenced out by the
    /// generation it carries.
    async fn publish_relaunched(
        &self,
        entry: &SessionEntry,
        generation: i64,
        result: Relaunched,
    ) -> SessionInfo {
        let Relaunched {
            terminal,
            scope,
            outcome,
            reset_capture,
            hooked,
            tabs,
        } = result;
        let restart_offer = if reset_capture {
            // The new window has captured nothing yet, so the only offer
            // this session can honestly make is what its snapshot alone
            // supports — which is also how a stale ambiguity stops being
            // reported the moment the relaunch clears it.
            entry.snapshot.restart_offer(None)
        } else {
            entry.snapshot.restart_offer(
                entry
                    .capture
                    .lock()
                    .expect("capture mutex poisoned")
                    .committed_conversation(),
            )
        };
        let info = SessionInfo {
            parent: entry.info.parent.clone(),
            archived: false,
            id: entry.info.id.clone(),
            title: entry.info.title.clone(),
            // A restart is a new LAUNCH GENERATION of the same session, not
            // a new session — carried forward from the entry being
            // replaced, never re-derived from "now".
            created_at: entry.info.created_at,
            // From the entry's LIVE cell rather than its `info` copy: the
            // sampler may have advanced it many times since this entry was
            // built, and a restart reply that reported the build-time
            // value would walk the session backwards in a recency sort
            // until the next sample corrected it. Carried across the
            // generation boundary on purpose, like `created_at` above and
            // unlike everything below that describes the RUN — the new
            // entry shares this very cell (`relaunched_entry`), so this
            // read and every later reply answer from one place.
            last_activity_at: entry
                .last_activity_at
                .load(std::sync::atomic::Ordering::Relaxed),
            creation_seq: entry.info.creation_seq,
            cwd: entry.info.cwd.clone(),
            invocation: entry.info.invocation.clone(),
            // Deliberately not a fabricated live status: the pane exists, but
            // whether the agent's own `exec` inside it succeeds is a
            // separate question this reply cannot answer. `ListSessions`
            // computes the real status, and the UI refreshes after a
            // restart for exactly that reason.
            status: SessionStatus::Unknown,
            // Cleared with the new generation: the annotation described how
            // the PREVIOUS run ended (item 4).
            annotation: None,
            restart_offer,
            // Captured before the relaunch by `relaunch_into_terminal`,
            // never rediscovered here: see that capture's own comment for
            // why a post-restart query must not be allowed to report `[]`
            // for a session whose tabs the restart never touched.
            tabs,
            // A restart is a new launch generation of the SAME session, so
            // what it was created from is carried forward from the entry
            // being replaced — never dropped, and never re-resolved. Losing
            // it here would have been invisible until the next reload put
            // it back, and in the meantime a profile-created session would
            // have looked raw-created to every client (PLAN_M6_75.md item
            // 4's snapshot rule, which a restart has no standing to
            // rewrite). The existence rides along as the same placeholder
            // every entry carries; the reply built from this derives its
            // own.
            source_profile: entry.info.source_profile.clone(),
        };
        let published = relaunched_entry(
            entry,
            info.clone(),
            Some(terminal),
            generation,
            scope,
            outcome,
            reset_capture,
        );
        // Raised on the NEW entry, whose cell `relaunched_entry` has just
        // minted `false` — this launch's own injection is the ONLY thing
        // that may raise it, whatever the previous launch did. The
        // conditional is therefore not a merge with an inherited value; it
        // simply leaves an un-injected launch's cell alone. Raising it on
        // `entry` instead would arm the tripwire on the abandoned
        // generation, which nothing evaluates any more.
        if hooked {
            published
                .hooked
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.sessions
            .lock()
            .await
            .insert(entry.info.id.clone(), published);
        info
    }

    /// The input control client's pid for one live attachment, for tests.
    ///
    /// Killing that process is the only deterministic way to exercise the
    /// connection handler's input-failure teardown. `tab_id = None` selects the
    /// agent terminal; `Some(id)` selects that tab. Production has no caller.
    pub async fn attachment_input_client_pid(
        &self,
        session_id: &str,
        tab_id: Option<&str>,
    ) -> Option<u32> {
        let terminal = match tab_id {
            Some(id) => TerminalId::Tab(id.to_string()),
            None => TerminalId::Agent,
        };
        self.attachments
            .lock()
            .await
            .get(&AttachmentKey::new(session_id, terminal))
            .and_then(|attachment| attachment.input.pid())
    }

    /// Tear down whatever is attached to a session being relaunched,
    /// telling the client why.
    ///
    /// A restart replaces the process behind the pane and can replace the
    /// pane itself, so an attachment that survived it would either be
    /// streaming a terminal whose meaning silently changed or, in the
    /// fresh-terminal case, one that no longer exists at all. Detaching
    /// both cases identically keeps the client's rule simple: after a
    /// restart, reattach. The replay a reattach performs is also what puts
    /// the reused pane's scrollback — the prior run's output — back on the
    /// client's screen.
    ///
    /// Scoped to the AGENT terminal, and that scope is a contract, not an
    /// artifact of there being only one terminal today: SPEC.md has
    /// restart touch the agent terminal alone, so a session's tabs — and
    /// therefore their attachments — must survive it untouched
    /// (PLAN_M4.md item 2). A session-wide sweep here would detach a tab
    /// whose shell the restart never went near.
    async fn detach_for_restart(&self, session_id: &str) -> anyhow::Result<()> {
        let key = AttachmentKey::new(session_id, TerminalId::Agent);
        let mut attachments = self.attachments.lock().await;
        let attachment = attachments.remove(&key);
        let Some(attachment) = attachment else {
            return Ok(());
        };
        // Stop before destructuring so the forwarder retains its cleanup tail;
        // publishing while the map lock is still held prevents a replacement
        // from observing neither the old entry nor its reap barrier. `..` then
        // drops this attachment's input client and pause sender.
        self.begin_forwarder_shutdown(key.clone(), &attachment);
        drop(attachments);
        let ActiveAttach {
            channel,
            notify,
            forwarder,
            sink,
            ..
        } = attachment;
        // Stopped before the notice, exactly as the delete handler does, so
        // the forwarder cannot race its own "terminal ended" detach against
        // this truthful one.
        let cleanup = self
            .record_forwarder_join(key.clone(), forwarder.await)
            .err();
        let reaping = self.has_output_reap_for_key(&key);
        drop(sink);
        notify_detached(&notify, channel, "session restarted".to_string());
        if let Some(cleanup) = cleanup {
            anyhow::bail!("{cleanup}");
        }
        if reaping {
            anyhow::bail!("the previous run's terminal-output client is still being cleaned up");
        }
        Ok(())
    }

    /// The shell every launch of this supervisor runs through — the seam
    /// (`SupervisorSeams::launch_shell`) when one is installed, the real
    /// `$SHELL`/passwd chain otherwise.
    ///
    /// Resolved PER LAUNCH rather than once at construction, which is
    /// SPEC.md's environment contract rather than an implementation
    /// detail: "the environment is evaluated at each launch", so a user who
    /// changes their login shell sees it on the next agent launch or tab
    /// open without restarting the supervisor.
    async fn launch_shell(&self) -> String {
        match &self.seams.launch_shell {
            Some(shell) => shell.clone(),
            None => resolve_shell().await,
        }
    }

    /// One session's tabs, rediscovered from tmux, in creation order.
    ///
    /// Fallible on purpose, and the two failure shapes are not the same
    /// answer. tmux definitively reporting that there is no server (or no
    /// panes) is `Ok(vec![])` — the session genuinely has no tabs, which
    /// is exactly what a rebooted or archived session looks like. Any
    /// OTHER query failure is an `Err`: "we could not ask" is not "there
    /// are none", and a caller that flattened the two would publish an
    /// empty tab strip for a session whose tabs are alive and attached.
    /// [`crate::tmux::TmuxDriver::is_definitively_empty`] is what draws
    /// the line, against tmux's own stderr rather than a rendered string.
    ///
    /// Its own tmux round trip, unlike the `ListSessions` path (which
    /// reuses the pane-state map it already fetched for liveness), because
    /// a single-session reply has no such map to share.
    ///
    /// Dead tabs are omitted, exactly as the `ListSessions` path omits
    /// them (`status::entry_info`): a reaped-any-tick-now corpse is not a
    /// tab a REPLY should offer. Teardown must not use this — it
    /// enumerates per-tab scopes and a hidden corpse still owns one; see
    /// [`Self::session_tabs_including_dead`].
    pub(crate) async fn session_tabs(&self, terminal: &Terminal) -> anyhow::Result<Vec<TabInfo>> {
        let states = self.tmux.pane_states().await?;
        Ok(tabs_from_pane_states(states.values(), &terminal.tmux_name)
            .into_iter()
            .filter(|tab| !tab.dead)
            .map(|tab| TabInfo { id: tab.id })
            .collect())
    }

    /// [`Self::session_tabs`] with dead tabs INCLUDED — the teardown
    /// spelling. Archive and delete enumerate per-tab cgroup scopes from
    /// this list, and a tab whose shell exited a moment ago still owns a
    /// scope (its daemonized children live there until something stops
    /// it); the reply-facing filter would skip exactly that scope in the
    /// death-to-reap window. The unit GLOB swept afterwards usually
    /// catches strays anyway, but the explicit list must not depend on it
    /// — the glob is the belt for units tmux can no longer name at all.
    pub(crate) async fn session_tabs_including_dead(
        &self,
        terminal: &Terminal,
    ) -> anyhow::Result<Vec<TabInfo>> {
        let states = self.tmux.pane_states().await?;
        Ok(tabs_from_pane_states(states.values(), &terminal.tmux_name)
            .into_iter()
            .map(|tab| TabInfo { id: tab.id })
            .collect())
    }

    /// Rename a session: the durable row and the in-memory entry that must
    /// land with it, returning the entry now published (PLAN_M5.md item 3).
    ///
    /// The caller builds the reply from what comes back, and deliberately
    /// OUTSIDE this call — see `session_info_now`, which needs a tmux round
    /// trip this function must not still be holding a claim across.
    ///
    /// ## Why the write is two-part
    ///
    /// Not a belt-and-braces duplication — the durable row alone would be
    /// INVISIBLE. `ListSessions` is served from in-memory `SessionEntry`
    /// values that are immutable once created and never re-read from SQLite
    /// mid-process, so a store-only rename would keep showing the old title
    /// in every list reply until the supervisor restarted. See
    /// [`renamed_entry`] for the rebuild, which is the half that makes the
    /// new title visible immediately — and for why it SHARES the entry's
    /// mutable cells rather than copying their values.
    ///
    /// ## Why the lifecycle claim, and why it is released here
    ///
    /// Rename is the first operation that changes a session's stored
    /// METADATA, and its two halves — the row and the republished entry —
    /// cannot be made atomic by either mutex: the `sessions` guard is
    /// released the moment the entry is cloned out of it, and the store
    /// write is an await that must happen between the two. Held under the
    /// session's claim ([`Supervisor::lifecycle_locks`]) the three
    /// interleavings that matter all resolve: two concurrent renames become
    /// last-write-wins with the store and the map agreeing on the SAME
    /// winner (rather than each ending up with a different one), and
    /// neither a delete nor a restart can slip between the row update and
    /// the map install.
    ///
    /// The claim ends WITH THE COMMIT, before any reply is built. A claim
    /// is exclusive against stop, delete and restart, so holding one across
    /// the reply's tmux probe would let a wedged tmux block this session's
    /// teardown for as long as it stayed wedged — and the reply needs
    /// nothing the claim protects.
    ///
    /// ## Why the commit runs on a supervisor-owned task, and why it takes
    /// the permit with it
    ///
    /// A client that disconnects mid-request cancels this future. Between
    /// the committed row and the map install that is exactly the window
    /// that must not be interrupted: the durable title would be the new one
    /// while every list reply from this still-running process served the
    /// old, until a restart. The two-part write therefore runs on a task
    /// this supervisor owns and this function merely awaits — dropping the
    /// awaiting future abandons the `JoinHandle`, never the work. The reply
    /// build afterwards is deliberately left cancellable: it changes
    /// nothing.
    ///
    /// `permit` is the request's ONE admission slot, and it is a parameter
    /// rather than something acquired here because ownership is the whole
    /// point. The task outlives its awaiter by construction, so a permit
    /// released when the awaiter is cancelled would be capacity handed back
    /// while the work still holds a lifecycle claim and a database write —
    /// and, since nothing else tracks these tasks, a client that
    /// disconnected after every rename could otherwise accumulate them
    /// without bound, each pinning a title and an `Arc<Supervisor>`. Moving
    /// the permit in makes the bound structural instead: at most
    /// `HANDLER_ADMISSION_PERMITS` commits can be in flight at once.
    ///
    /// The commit hands the permit back with its outcome — SUCCEEDED OR
    /// FAILED — so the caller holds the same slot through whichever reply
    /// it sends. Two separate reasons, and both matter.
    ///
    /// One slot per request, rather than a second one for the reply, is
    /// what keeps this from deadlocking: a rename that acquired one permit
    /// and then waited for another would wedge outright once
    /// `HANDLER_ADMISSION_PERMITS` renames were in flight, every one of
    /// them holding a slot nothing can release while waiting for a slot
    /// nobody will free.
    ///
    /// And the slot has to span the FAILURE reply too, not just the
    /// success one. Every reply here awaits the connection's bounded
    /// writer queue, so a client that is not reading parks whichever task
    /// is sending — which is precisely what admission bounds for every
    /// other handler. Freeing the slot at a failed commit would let a peer
    /// flooding refusable renames (a nonexistent session id, say) reclaim
    /// capacity per request while its error-reply tasks piled up against
    /// the queue it is not draining: the same unbounded accumulation this
    /// design closes, reintroduced through the error path. The permit is
    /// therefore returned in both arms and dropped only when the reply has
    /// been handed over.
    ///
    /// `None` comes back only when the task itself died (a panic), which
    /// took its permit with it.
    ///
    /// Everything outside the title is left alone, which SPEC.md's rename
    /// is: tmux session names are internal identifiers no user sees, and
    /// the create-idempotency fingerprint records the CREATE request as it
    /// was sent, so a later rename must not disturb an intent key's replay.
    /// Attachments are untouched for the same reason — nothing about a
    /// terminal stream depends on the session's label — and the run's own
    /// state (outcome, first input, capture progress) is not copied but
    /// SHARED with the entry this replaces, so an attachment's input path
    /// and an in-flight capture pass keep writing where the published entry
    /// reads.
    pub(crate) async fn rename_session(
        self: &Arc<Self>,
        session_id: &str,
        title: String,
        permit: tokio::sync::OwnedSemaphorePermit,
    ) -> (
        Result<Arc<SessionEntry>, RequestError>,
        Option<tokio::sync::OwnedSemaphorePermit>,
    ) {
        let sup = Arc::clone(self);
        let id = session_id.to_string();
        let commit = tokio::spawn(async move {
            // The admission slot belongs to this task while the commit
            // runs and then travels onward with the OUTCOME, whichever it
            // is; the lifecycle claim belongs to the inner block and is
            // released when it ends. See this function's docs for why the
            // awaiter's cancellation may release neither early, and why
            // the slot has to span the failure reply too.
            let outcome = async {
                let _lifecycle = sup.lifecycle_locks.claim(&id).await;
                let not_found = || {
                    RequestError::new(
                        ErrorKind::NotFound,
                        format!("no such session: {}", truncate_for_error(&id)),
                    )
                };
                let Some(entry) = sup.sessions.lock().await.get(&id).cloned() else {
                    return Err(not_found());
                };
                // Durable first, in-memory second — the same crash ordering
                // every other write in this file takes: a crash between the
                // two leaves a renamed row that the next reload reads, while
                // the reverse order would leave a title that exists only
                // until the process ends.
                let updated = sup
                    .store
                    .set_session_title(&id, &title)
                    .await
                    .map_err(|e| {
                        RequestError::new(
                            error_kind(&e),
                            format!("renaming session {}: {e:#}", truncate_for_error(&id)),
                        )
                    })?;
                if !updated {
                    // The map had an entry but the row is gone. A delete
                    // cannot have done it under this claim, so what remains
                    // is a row removed by something that takes no claim — a
                    // create rollback for an id this map should no longer be
                    // holding, or a database edited underneath a running
                    // supervisor. Either way the durable truth is that there
                    // is no session here, and `NotFound` reports it rather
                    // than confirming a rename of nothing.
                    return Err(not_found());
                }
                let renamed = renamed_entry(&entry, title);
                {
                    let mut sessions = sup.sessions.lock().await;
                    // Identity-checked rather than a blind insert. Under the
                    // claim no other writer should be able to have replaced
                    // this entry, so the check is defensive: if that ever
                    // stops holding, losing the in-memory half is
                    // recoverable — the durable title is committed and the
                    // next reload picks it up — while overwriting whatever a
                    // claim-less writer published in the meantime would not
                    // be.
                    if let Some(slot) = sessions.get_mut(&id)
                        && Arc::ptr_eq(slot, &entry)
                    {
                        *slot = Arc::clone(&renamed);
                    }
                }
                Ok(renamed)
            }
            .await;
            (outcome, permit)
        });
        // The commit task panicking is reported rather than propagated:
        // letting the panic through here would amount to a connection with
        // no reply on it. Its permit died with it, hence `None`.
        match commit.await {
            Ok((outcome, permit)) => (outcome, Some(permit)),
            Err(e) => (
                Err(RequestError::new(
                    ErrorKind::Internal,
                    format!(
                        "renaming session {} failed unexpectedly: {e}",
                        truncate_for_error(session_id)
                    ),
                )),
                None,
            ),
        }
    }

    /// Open a terminal tab: a new tmux window on the session's tmux
    /// session, running the user's login shell in the session's working
    /// directory (PLAN_M4.md item 2, `ControlMsg::OpenTab`'s contract).
    ///
    /// ## Ordering, and what each step buys
    ///
    /// 1. **Resolve the session's agent terminal.** Not because a tab
    ///    needs it, but because it is where the tmux session name lives —
    ///    and its absence IS the restart-first refusal: a session whose
    ///    terminals a reboot or archive erased has no tmux session to add
    ///    a window to, and building a tab-only substrate for an agent-less
    ///    session is not a state this system has.
    /// 2. **Check the working directory.** The same `ensure_cwd_usable`
    ///    precondition restart makes, so the two refusals read the same
    ///    (M3's error shape, reused unchanged per PLAN_M4.md item 1).
    /// 3. **Mint the id and create the window.** The id is minted BEFORE
    ///    the window so it can go into the window's environment as the tab
    ///    marker (`FARHELM_TAB_ID`) in the very same `new-window` — every
    ///    process the shell ever forks then carries it, which is what
    ///    close reaps by.
    /// 4. **Mark the window.** This is the tab's only record (tabs are not
    ///    durable metadata), so a failure here is a FAILED OPEN with the
    ///    window cleaned up — the alternative is a live shell nothing can
    ///    ever find again, list, or close.
    /// 5. **Check the pane is alive.** SPEC.md's every-failed-operation
    ///    rule: a shell already dead by reply time is a refused open
    ///    carrying the pane's last words, not a successful open holding a
    ///    corpse. A shell that starts and later exits is a different thing
    ///    entirely: an established tab, which the listing paths hide and
    ///    the ticker reaps (SPEC.md's automatic tab reap).
    ///
    /// Nothing here WRITES to supervisor.db or the sessions map. That is
    /// the point: rediscovery from window markers is the honest
    /// implementation of "tabs are not durable metadata", and a tab that
    /// survives a supervisor restart does so by the same mechanism the
    /// agent terminal does — tmux outliving the supervisor.
    ///
    /// ## Two safety properties, both structural
    ///
    /// It takes the session's LIFECYCLE claim, for the same reason
    /// `StopSession` and `DeleteSession` do (see
    /// `Supervisor::lifecycle_locks`). Without it a delete could complete
    /// its process-tree sweep, and this could then start a shell in the
    /// tmux session the delete is about to tear down — leaving that
    /// shell's daemonized children alive with no row left to reap them
    /// from. Serialized, both orders are correct: an open that wins is
    /// swept by the delete that follows, and an open that loses finds no
    /// session at all.
    ///
    /// And everything from the first side effect onward runs on a
    /// SUPERVISOR-OWNED task. A client that disconnects mid-open cancels
    /// the await, not the work: a cancellation between `new-window` and
    /// the marking would otherwise strand a live, unmarked, unfindable
    /// shell forever.
    pub(crate) async fn open_tab(
        self: &Arc<Self>,
        session_id: &str,
    ) -> Result<TabInfo, RequestError> {
        let lifecycle = self.lifecycle_locks.claim(session_id).await;
        let entry = self.sessions.lock().await.get(session_id).cloned();
        let Some(entry) = entry else {
            return Err(RequestError::new(
                ErrorKind::NotFound,
                format!("no such session: {}", truncate_for_error(session_id)),
            ));
        };
        let restart_first = || {
            RequestError::new(
                ErrorKind::Conflict,
                format!(
                    "session {} has no terminals to add a tab to (a reboot or an archive ended \
                     them); restart the session first",
                    truncate_for_error(session_id)
                ),
            )
        };
        let Some(agent) = entry.terminal.clone() else {
            return Err(restart_first());
        };
        // The tmux session can also be gone WITHOUT the entry knowing —
        // this process may have been serving since before an external
        // `kill-session`, or since before the whole tmux server died. Both
        // are the same fact for a caller ("there is no terminal substrate
        // here") and get the same restart-first advice; a raw tmux
        // diagnostic would be honest and useless. Anything else propagates
        // as itself, because "we could not ask" is not "it is gone".
        match self.tmux.has_session(&agent.tmux_name).await {
            Ok(true) => {}
            Ok(false) => return Err(restart_first()),
            Err(e) if self.tmux.is_definitively_empty(&e) => return Err(restart_first()),
            Err(e) => return Err(RequestError::new(error_kind(&e), format!("{e:#}"))),
        }
        ensure_cwd_usable(&entry.info.cwd)
            .await
            .map_err(|e| RequestError::new(error_kind(&e), format!("{e:#}")))?;
        let session_token = self
            .store
            .session_token(session_id)
            .await
            .map_err(|e| RequestError::new(ErrorKind::Internal, format!("{e:#}")))?
            .ok_or_else(|| {
                RequestError::new(
                    ErrorKind::NotFound,
                    format!("no such session: {}", truncate_for_error(session_id)),
                )
            })?;

        // Everything past here is destructive or owes cleanup, so it runs
        // on a task this supervisor owns and the lifecycle claim moves
        // into it. Awaiting the handle is itself cancellable; the work is
        // not.
        let sup = Arc::clone(self);
        let session_id = session_id.to_string();
        let cwd = entry.info.cwd.clone();
        let task = tokio::spawn(async move {
            let _lifecycle = lifecycle;
            sup.open_tab_window(&session_id, &session_token, &agent, &cwd)
                .await
        });
        match task.await {
            Ok(result) => result,
            Err(join) => Err(RequestError::new(
                ErrorKind::Internal,
                format!("the terminal-tab open task failed: {join}"),
            )),
        }
    }

    /// [`Self::open_tab`]'s side-effecting half, split out so the whole of
    /// it runs on one supervisor-owned task; see that method for the
    /// ordering argument.
    fn tab_environment(
        &self,
        session_id: &str,
        session_token: &str,
        tab_id: &str,
    ) -> Vec<(String, String)> {
        let mut env = self.seams.launch_env.clone();
        // The id, token, and socket are one spawn authority. Keeping their
        // construction together prevents a tab-only change from restoring
        // the pre-upgrade state where the id existed but spawn still failed.
        env.push((
            crate::launch::SESSION_ID_ENV_VAR.to_string(),
            session_id.to_string(),
        ));
        env.push((
            crate::launch::TAB_ID_ENV_VAR.to_string(),
            tab_id.to_string(),
        ));
        env.push((
            crate::launch::SESSION_TOKEN_ENV_VAR.to_string(),
            session_token.to_string(),
        ));
        env.push((
            crate::launch::SUPERVISOR_SOCK_ENV_VAR.to_string(),
            Self::socket_path(&self.state_dir)
                .to_string_lossy()
                .into_owned(),
        ));
        env
    }

    /// Open and publish a shell window after its complete environment is fixed.
    async fn open_tab_window(
        &self,
        session_id: &str,
        session_token: &str,
        agent: &Terminal,
        cwd: &str,
    ) -> Result<TabInfo, RequestError> {
        let tab_id = uuid::Uuid::new_v4().to_string();
        // The scope, decided per OPEN the way an agent's is decided per
        // launch — and, unlike the agent's, never recorded: a tab has no
        // durable row to record it in, so `close_tab` re-derives the same
        // name from the same two ids and lets `exists` settle whether
        // there is anything there.
        let unit = crate::scope::tab_unit_name(session_id, &tab_id);
        let scope_prefix = match unit.as_deref() {
            Some(unit) => match self.seams.scopes.launch_prefix(unit).await {
                Some(prefix) => prefix,
                // Loud for the agent path's reason: the tab still opens
                // (never worse than a host with no manager at all), but
                // close falls back to the marker sweep, and a silent
                // downgrade of a containment guarantee is exactly what
                // this project does not do.
                None => {
                    warn!(
                        session = %session_id, tab = %tab_id, unit,
                        "this host's systemd user manager is not usable, so this tab opens \
                         without a cgroup scope and its close falls back to the process-tree \
                         sweep"
                    );
                    Vec::new()
                }
            },
            None => Vec::new(),
        };
        let shell = self.launch_shell().await;
        let env = self.tab_environment(session_id, session_token, &tab_id);
        let window_cmd = crate::launch::tab_window_command(&shell, scope_prefix);
        let (window, pane) = self
            .tmux
            .new_window(&agent.tmux_name, cwd, &env, &window_cmd)
            .await
            .map_err(|e| {
                RequestError::new(
                    error_kind(&e),
                    format!("could not open a terminal tab for session {session_id}: {e:#}"),
                )
            })?;
        let terminal = Terminal {
            tmux_name: agent.tmux_name.clone(),
            pane,
        };

        // Size the new window like the AGENT's before anything can attach
        // to it (BUGS_BURNDOWN.md issue 4). `new-window` inherits the
        // session default size — not the agent window's, since resize is
        // per-window (PLAN_M4.md item 3) — so without this, a tab's first
        // attach almost always carries a REAL resize, and the shell's
        // SIGWINCH repaint can race the attach's snapshot capture: the
        // captured content and the separately sampled cursor can disagree,
        // and the leading explanation for issue 4's reported symptom (a
        // stale duplicate of the prompt beside the live one; never
        // reproduced under instrumentation, so the mechanism is inferred)
        // is exactly that torn frame. The agent window's size is the best
        // available stand-in for the island about to attach (same client,
        // same pane area). Best effort on the size read and the resize
        // alike: a failure of either only restores the old
        // resize-at-attach behavior, so it is not worth failing the open.
        match self.tmux.window_size(&agent.tmux_name, &agent.pane).await {
            Ok(Some((cols, rows))) => {
                if let Err(e) = self
                    .tmux
                    .resize_window(&terminal.tmux_name, &terminal.pane, cols, rows)
                    .await
                {
                    debug!(
                        session = %session_id, tab = %tab_id, error = %format!("{e:#}"),
                        "could not pre-size the new tab window; the first attach will resize it"
                    );
                }
            }
            Ok(None) => {}
            Err(e) => {
                debug!(
                    session = %session_id, tab = %tab_id, error = %format!("{e:#}"),
                    "could not read the agent window's size; the first attach will resize the tab"
                );
            }
        }

        // The seam stands in for the marking itself when installed: what
        // it exists to reach is the state AFTER this call fails — a live,
        // unmarked, unfindable window — which no other input can produce.
        let marked = match &self.seams.tab_open_fault {
            Some(fault) => fault(TabOpenStage::BeforeMarking),
            None => Ok(()),
        };
        let marked = match marked {
            Ok(()) => {
                self.tmux
                    .mark_window(
                        &terminal.tmux_name,
                        &terminal.pane,
                        TAB_WINDOW_OPTION,
                        &tab_id,
                    )
                    .await
            }
            Err(e) => Err(e),
        };
        if let Err(e) = marked {
            // An unmarked window is invisible to rediscovery, so leaving
            // it would strand a live shell nothing can list or close.
            return Err(self
                .discard_failed_tab_window(
                    session_id,
                    &terminal,
                    &tab_id,
                    format!("could not mark the new terminal tab's window: {e:#}"),
                )
                .await);
        }

        // The dead-at-reply refusal (`ControlMsg::OpenTab`'s contract).
        //
        // Bounded settle rather than one instantaneous read, and the
        // difference is the promise itself. A shell that cannot start dies
        // in microseconds, but tmux marks the pane dead only once it has
        // reaped the child and drained the pty — measured at 4–18 ms on an
        // idle host (tmux 3.7b), and load only widens that. A single check
        // fired the moment `new-window` returns therefore races tmux's own
        // bookkeeping, and losing that race means reporting a SUCCESSFUL
        // open holding a corpse, which is exactly what this refusal
        // exists to prevent. "Already dead when the open would reply" is a
        // statement about when the supervisor chooses to reply, so it
        // chooses to reply late enough for the answer to mean something.
        //
        // Bounded on the other side too: a healthy open pays the whole
        // window, so it is sized for a hand-driven operation rather than a
        // hot path. The settle ends early the moment the pane IS dead, so
        // the failing case stays fast.
        //
        // The gate is the test-only handle on the "already dead" side of
        // that window: the window and its marker exist here, so a waiting
        // test can watch tmux for the pane's death and only then let the
        // settle run. See [`TabSettleGate`]; production installs none.
        if let Some(gate) = &self.seams.tab_settle_gate {
            gate().await;
        }
        match self.settled_tab_pane(&terminal).await {
            Ok(Some(state)) if !state.dead => {}
            Ok(state) => {
                // Captured BEFORE the window is destroyed — it is the
                // whole point of the refusal — and the three outcomes stay
                // distinguishable: what the shell said, that it said
                // nothing, and that we could not look.
                //
                // Only when the pane still EXISTS, though: a capture
                // addressed at a vanished pane falls back to the session's
                // current one (see `tmux::pane_in_session`), which would
                // quote the agent terminal's screen into this refusal.
                let (gone, detail) = match state {
                    None => (" (its pane was already gone)", String::new()),
                    Some(_) => (
                        "",
                        match self.tab_pane_last_words(&terminal).await {
                            Ok(words) if !words.trim().is_empty() => {
                                format!(": {}", words.trim())
                            }
                            Ok(_) => " and printed nothing".to_string(),
                            Err(e) => format!(", and its output could not be read ({e:#})"),
                        },
                    ),
                };
                return Err(self
                    .discard_failed_tab_window(
                        session_id,
                        &terminal,
                        &tab_id,
                        format!(
                            "the terminal tab's shell ({shell}) was already dead when the tab \
                             opened{gone}{detail}"
                        ),
                    )
                    .await);
            }
            Err(e) => {
                return Err(self
                    .discard_failed_tab_window(
                        session_id,
                        &terminal,
                        &tab_id,
                        format!("could not confirm the new terminal tab's shell is running: {e:#}"),
                    )
                    .await);
            }
        }

        info!(
            session = %session_id, tab = %tab_id, tmux = %terminal.tmux_name, %window,
            pane = %terminal.pane, scoped = unit.is_some(),
            "terminal tab opened"
        );
        Ok(TabInfo { id: tab_id })
    }

    /// Close a terminal tab: reap its shell and everything that shell left
    /// behind, then drop its window (`ControlMsg::CloseTab`'s contract).
    ///
    /// ## The ordering, and why it is not the obvious one
    ///
    /// Never kill-window-first (PLAN_M4.md item 2). The window's pane is
    /// what anchors the descendant walk — a PPID closure needs a live root
    /// — so killing it up front would orphan the walk and leave only the
    /// weaker marker scan, which a process that scrubbed its environment
    /// escapes. So: reap while the pane is alive, THEN drop the window,
    /// THEN sweep once more.
    ///
    /// That last pass is not redundant with the first. Between them the
    /// window dies, and `kill-window` is itself a kill — it SIGHUPs the
    /// pane's process group — so it is the step that ends anything the
    /// first pass had no way to reach (a tab whose pane was already dead
    /// has no root to walk from at all, and its shell's children may only
    /// die with the pty). The second pass is what turns `TabClosed` into
    /// the honest statement `SessionStopped` is: the reply is sent only
    /// once nothing of this tab is left running, with the same
    /// no-systemd-manager blind spot `kill_process_tree` documents.
    ///
    /// ## The attachment goes last, and unconditionally
    ///
    /// Once the window is gone, any client still attached to it is
    /// streaming a terminal that does not exist, so the detach happens
    /// even when the second sweep fails and this reports an error. A close
    /// that returned an error while leaving a live attachment on a dead
    /// window would leave the user with a frozen terminal and a retry that
    /// answers `NotFound`.
    ///
    /// An unknown tab id is `NotFound` (`resolve_terminal`'s answer, so
    /// close and attach agree on what "that tab is gone" means). A tab
    /// whose shell already exited still closes successfully — like
    /// `StopSession`, "make sure nothing is running" already holds, and
    /// the window is dropped either way.
    pub(crate) async fn close_tab(
        self: &Arc<Self>,
        session_id: &str,
        tab_id: &str,
    ) -> Result<(), RequestError> {
        // The session's lifecycle claim, held for the whole close — see
        // `open_tab` for the argument, which applies symmetrically here:
        // a close interleaved with a delete would have two sweeps and two
        // teardowns racing over the same window. It is also what makes
        // `Attach`'s own tab revalidation meaningful (see that handler).
        let lifecycle = self.lifecycle_locks.claim(session_id).await;
        let entry = self.sessions.lock().await.get(session_id).cloned();
        let Some(entry) = entry else {
            return Err(RequestError::new(
                ErrorKind::NotFound,
                format!("no such session: {}", truncate_for_error(session_id)),
            ));
        };
        // The close-capable resolver: an exited-but-unreaped tab is exactly
        // what this path (the user's close, and the ticker's reap through
        // it) must still be able to find and destroy, while attach and
        // resize already treat it as gone.
        let terminal =
            resolve_terminal_for_close(self, &entry, &TerminalId::Tab(tab_id.to_string())).await?;

        // Supervisor-owned, like the open: a client disconnecting between
        // the first sweep and the window kill must not leave a half-reaped
        // tab with no owner to finish it.
        let sup = Arc::clone(self);
        let session_id = session_id.to_string();
        let tab_id = tab_id.to_string();
        let task = tokio::spawn(async move {
            let _lifecycle = lifecycle;
            sup.close_tab_window(&session_id, &terminal, &tab_id).await
        });
        match task.await {
            Ok(result) => result,
            Err(join) => Err(RequestError::new(
                ErrorKind::Internal,
                format!("the terminal-tab close task failed: {join}"),
            )),
        }
    }

    /// [`Self::close_tab`]'s side-effecting half, split out so the whole
    /// of it runs on one supervisor-owned task.
    async fn close_tab_window(
        &self,
        session_id: &str,
        terminal: &Terminal,
        tab_id: &str,
    ) -> Result<(), RequestError> {
        self.reap_tab_tree(session_id, terminal, tab_id, TabReapAnchor::PaneIfLive)
            .await
            .map_err(|e| {
                RequestError::new(
                    ErrorKind::Internal,
                    format!("closing terminal tab {}: {e:#}", truncate_for_error(tab_id)),
                )
            })?;
        self.tmux
            .kill_window(&terminal.tmux_name, &terminal.pane)
            .await
            .map_err(|e| {
                RequestError::new(
                    error_kind(&e),
                    format!(
                        "terminal tab {}'s processes were reaped but its window could not be \
                         removed: {e:#}",
                        truncate_for_error(tab_id)
                    ),
                )
            })?;
        // The re-enumeration, marker-only: the pane that would have
        // supplied a root died with the window a moment ago, and asking
        // tmux about a pane it no longer has would resolve to a SIBLING
        // TAB's pane — see `reap_tab_tree`'s `anchor` docs.
        let survivors = self
            .reap_tab_tree(session_id, terminal, tab_id, TabReapAnchor::MarkerOnly)
            .await;
        // Unconditional, and before reporting either error: the window is
        // already gone, so its viewer must receive the terminal verdict even
        // when process or output-client cleanup remains unconfirmed.
        let detach = self.detach_closed_tab(session_id, tab_id).await;
        tab_close_cleanup_result(tab_id, survivors, detach)?;
        info!(session = %session_id, tab = %tab_id, "terminal tab closed");
        Ok(())
    }

    /// Tear down whatever attachment a just-closed tab still had.
    ///
    /// Scoped to the ONE terminal, unlike `DeleteSession`'s session-wide
    /// sweep: closing a tab says nothing about the session's other
    /// terminals, and the client keeps its lease on them (PLAN_M4.md item
    /// 3 — the lease groups a session's channels, and this removes one of
    /// them rather than dissolving the group).
    ///
    /// Stops and awaits the forwarder before notifying, like every other
    /// teardown here, so it cannot race its own end-of-stream `Detached`
    /// against this truthful one. The forwarder may transfer process reaping
    /// to the published per-terminal barrier; in that case the notice still
    /// goes out, but replacement attach remains fail-closed behind the barrier.
    ///
    /// This is also the ONLY thing that ends a tab's attachment while its
    /// session lives, and that is worth knowing: a tab's forwarder holds a
    /// control client attached to the tmux SESSION, so losing the tab's
    /// WINDOW does not end that client the way losing the session would —
    /// the stream simply goes quiet. Every path the product itself offers
    /// is covered (a close comes through here; a reboot or archive takes
    /// the whole tmux session, which does end the client). A window killed
    /// by hand, directly against the private tmux server, is the residual:
    /// its viewer sees a terminal that stops updating until it detaches or
    /// reattaches. Detecting it would mean teaching the output stream
    /// tmux's window-close notifications, which no product path needs.
    async fn detach_closed_tab(&self, session_id: &str, tab_id: &str) -> Result<(), RequestError> {
        let key = AttachmentKey::new(session_id, TerminalId::Tab(tab_id.to_string()));
        let mut attachments = self.attachments.lock().await;
        let attachment = attachments.remove(&key);
        let Some(attachment) = attachment else {
            return Ok(());
        };
        // Install the handoff barrier before releasing the map lock, so a
        // concurrent attach cannot slip into the removal-to-publication gap.
        self.begin_forwarder_shutdown(key.clone(), &attachment);
        drop(attachments);
        // `..` drops this attachment's input client (killing its
        // no-output control client) and its pause sender.
        let ActiveAttach {
            channel,
            notify,
            forwarder,
            sink,
            ..
        } = attachment;
        let cleanup = self
            .record_forwarder_join(key.clone(), forwarder.await)
            .err();
        let reaping = self.has_output_reap_for_key(&key);
        drop(sink);
        notify_detached(&notify, channel, "terminal tab closed".to_string());
        if let Some(cleanup) = cleanup {
            return Err(RequestError::new(ErrorKind::Internal, cleanup.to_string()));
        }
        if reaping {
            return Err(RequestError::new(
                ErrorKind::Internal,
                "the closed tab's terminal-output client is still being cleaned up",
            ));
        }
        Ok(())
    }

    /// Tear down a window an `OpenTab` decided not to keep, and turn
    /// `because` into the error that open reports.
    ///
    /// Reaps before killing the window, for the same reason `close_tab`
    /// does (the live pane anchors the descendant walk) — and it is not
    /// merely symmetry: the failure that brings us here can be the MARKING
    /// step, by which point the shell has been running for a round trip
    /// and may already have forked.
    ///
    /// Cleanup failures are AGGREGATED INTO the returned error rather than
    /// logged past it. The open has failed either way, but "the tab was
    /// removed again" and "a shell you cannot see is still running" are
    /// very different things to tell a user, and the second one has to
    /// name the tab id — it is the only handle left for cleaning it up by
    /// hand, since an unmarked or unlisted window is exactly what
    /// rediscovery cannot see.
    async fn discard_failed_tab_window(
        &self,
        session_id: &str,
        terminal: &Terminal,
        tab_id: &str,
        because: String,
    ) -> RequestError {
        let mut left_behind: Vec<String> = Vec::new();
        if let Err(e) = self
            .reap_tab_tree(session_id, terminal, tab_id, TabReapAnchor::PaneIfLive)
            .await
        {
            left_behind.push(format!("its processes ({e:#})"));
        }
        if let Err(e) = self
            .tmux
            .kill_window(&terminal.tmux_name, &terminal.pane)
            .await
        {
            left_behind.push(format!("its window ({e:#})"));
        }
        if left_behind.is_empty() {
            return RequestError::new(
                ErrorKind::Internal,
                format!("{because}, so the tab was removed again"),
            );
        }
        warn!(
            session = %session_id, tab = %tab_id, pane = %terminal.pane,
            "a failed terminal-tab open could not be fully unwound"
        );
        RequestError::new(
            ErrorKind::Internal,
            format!(
                "{because}, and it could not be cleaned up: {} of terminal tab {} may still \
                 exist (pane {})",
                left_behind.join(" and "),
                truncate_for_error(tab_id),
                terminal.pane
            ),
        )
    }

    /// Watch a freshly-created tab pane until it is either confirmed dead
    /// or has survived [`TAB_LAUNCH_SETTLE`] — see the `OpenTab` handler's
    /// own comment for why the open waits at all.
    ///
    /// Returns the LAST observation, so the caller's `None`/`dead`
    /// branches read exactly as they would from a bare `pane_process`
    /// call. A query error ends the settle immediately: an open that
    /// cannot ask tmux anything must not spend the whole window
    /// re-asking, and the caller refuses on it either way.
    ///
    /// A FOREIGN owner is an error here whoever the owner is — the one
    /// call site that does not consult
    /// [`Supervisor::known_session_tmux_name`] at all. This pane was
    /// created by this process moments ago, in this session, so ANY other
    /// owner is an invariant violation rather than the staleness the
    /// lifecycle verbs classify: a recycled id cannot explain it either,
    /// because the recycle story needs a server generation to have passed
    /// since the pane was recorded, and none has. Erroring routes it into
    /// the caller's refusal-and-unwind path instead of letting the open
    /// report success over a pane it does not own.
    async fn settled_tab_pane(
        &self,
        terminal: &Terminal,
    ) -> anyhow::Result<Option<crate::tmux::PaneProcess>> {
        let deadline = tokio::time::Instant::now() + TAB_LAUNCH_SETTLE;
        loop {
            let observed = match self
                .tmux
                .pane_process(&terminal.tmux_name, &terminal.pane)
                .await?
            {
                PaneProbe::Owned(state) => Some(state),
                PaneProbe::Gone => None,
                PaneProbe::ForeignOwner { owner } => {
                    anyhow::bail!(
                        "pane {} belongs to session {owner:?}, not {:?}; refusing to treat it \
                         as this tab's terminal (a stale or reused pane id would otherwise \
                         cross-wire two sessions' process trees)",
                        terminal.pane,
                        terminal.tmux_name
                    );
                }
            };
            let alive = observed.is_some_and(|state| !state.dead);
            if !alive || tokio::time::Instant::now() >= deadline {
                return Ok(observed);
            }
            tokio::time::sleep(TAB_LAUNCH_SETTLE_STEP).await;
        }
    }

    /// The last thing a failed tab shell printed, for the refused open's
    /// error detail.
    ///
    /// Thin over [`crate::tmux::TmuxDriver::capture_pane_text`], which
    /// does the work worth knowing about: it reads SCROLLBACK rather than
    /// the visible grid (a dead pane's grid is replaced by tmux's own
    /// "Pane is dead" banner, which would restate the exit code and lose
    /// the shell's actual complaint), keeps the TAIL when the text is over
    /// cap, and pairs the pane with its session so a stale pane id cannot
    /// quote a stranger's terminal into this session's error.
    ///
    /// The `Err` is passed through rather than flattened into an empty
    /// capture, because the refusal distinguishes all three outcomes: what
    /// the shell said, that it said nothing, and that we could not look.
    /// A cap far below the snapshot cap, because this ends up inside a
    /// protocol error message rather than a replay frame.
    async fn tab_pane_last_words(&self, terminal: &Terminal) -> anyhow::Result<String> {
        const MAX_LAST_WORDS: usize = 4 * 1024;
        self.tmux
            .capture_pane_text(&terminal.tmux_name, &terminal.pane, MAX_LAST_WORDS)
            .await
            .inspect_err(|e| {
                debug!(
                    pane = %terminal.pane, error = %format!("{e:#}"),
                    "could not capture a failed tab pane's output for the refusal's detail"
                );
            })
    }

    /// Reap one tab's process tree: its own cgroup scope where one exists,
    /// then the marker sweep keyed on that tab's marker.
    ///
    /// Shared by `close_tab` and by the failed-open unwind so both reap
    /// identically. The pane's own pid is looked up here rather than
    /// passed in, and the answers are NOT the same: a live pane is
    /// the PPID closure's root, a CONFIRMED-absent or dead pane means
    /// there is no root to walk from (a dead pane's remembered pid may
    /// already be recycled), a pane owned by a DIFFERENT session splits by
    /// whether that owner is a session this supervisor knows (no root, or
    /// a refusal — see the match arm below), and a pane query that FAILED
    /// propagates — falling back to the marker scan there would let a
    /// close report success having quietly swept less than it claims.
    ///
    /// `anchor` says whether to look the pane up at all, and the caller
    /// that says `MarkerOnly` is not being lazy: after `kill-window` the
    /// pane is gone BY CONSTRUCTION, and asking tmux about it again is not
    /// merely pointless but unsafe — a `=session:.%pane` target for a
    /// vanished pane falls back to the session's CURRENT pane (audited;
    /// see `tmux::pane_in_session`), so the second sweep would anchor its
    /// descendant walk on a SIBLING TAB and reap it. That is not
    /// hypothetical: it is the bug this parameter exists to make
    /// unexpressible.
    ///
    /// The scope unit is DERIVED (see `scope::tab_unit_name`), never
    /// stored, and it is derived even when this supervisor's own
    /// availability probe says there is no user manager: the scope may
    /// predate this supervisor, or the probe may have run while the
    /// manager was briefly unreachable, and `kill_scope`'s own existence
    /// check is what settles whether there is anything there. Naming a
    /// unit that does not exist costs one query; skipping one that does
    /// costs the containment guarantee.
    async fn reap_tab_tree(
        &self,
        session_id: &str,
        terminal: &Terminal,
        tab_id: &str,
        anchor: TabReapAnchor,
    ) -> anyhow::Result<()> {
        let root_pid = match anchor {
            TabReapAnchor::PaneIfLive => match self
                .tmux
                .pane_process(&terminal.tmux_name, &terminal.pane)
                .await
                .context("reading a terminal tab's pane process before reaping it")?
            {
                PaneProbe::Owned(state) if !state.dead => Some(state.pid),
                PaneProbe::Owned(_) | PaneProbe::Gone => None,
                // A RECOGNIZED foreign owner gives no root to walk from —
                // the recorded pane died with a previous tmux server, and
                // walking the stranger that inherited its id would reap
                // THEIR tab — so the close falls back to the marker sweep
                // and the tab's own scope, exactly as the confirmed-absent
                // case does.
                //
                // An UNRECOGNIZED owner is an `Err`, which is the same
                // fail-closed direction the failed-query case takes and
                // for a stronger reason here: the pane may be this tab's
                // own live shell under a renamed tmux session, so a close
                // that swept by marker alone would report the tab removed
                // while its window, pane, and shell went on living. The
                // failed-open unwind inherits that too — it reports an
                // incomplete cleanup instead of claiming removal.
                PaneProbe::ForeignOwner { owner } => {
                    if !self.known_session_tmux_name(&owner).await {
                        anyhow::bail!(unknown_pane_owner_refusal(
                            &terminal.pane,
                            &owner,
                            &terminal.tmux_name
                        ));
                    }
                    None
                }
            },
            TabReapAnchor::MarkerOnly => None,
        };
        let units: Vec<String> = crate::scope::tab_unit_name(session_id, tab_id)
            .into_iter()
            .collect();
        // `session_id` is carried purely for the log lines inside the
        // sweep; `SweepTarget::Tab` is what actually selects processes
        // here, by the session marker AND this tab's own id.
        reap_process_tree(
            &self.seams.scopes,
            &units,
            root_pid,
            session_id,
            &SweepTarget::Tab(tab_id.to_string()),
        )
        .await
    }

    /// Whether a launch of `id` may run in its own cgroup scope
    /// (PLAN_M3.md item 10) — the SELECTION, recorded durably per launch.
    ///
    /// Two independent conditions, both of which must hold: this host has a
    /// usable systemd user manager, and this session's id can safely name a
    /// unit at all (`scope::unit_name` — a non-UUID id, which only a
    /// hand-edited or foreign database can produce, is refused rather than
    /// sanitized into a name that might collide with another session's).
    /// Nameability does not depend on the generation, so this answers for
    /// every launch of the session.
    ///
    /// Per LAUNCH, never per session and never per supervisor: the answer is
    /// recorded with the row it describes, so two launches of the same
    /// session on either side of a supervisor restart can honestly differ.
    /// The underlying availability probe is what is cached
    /// (`scope::ScopeManager::available`); this call is the cheap read of
    /// that verdict.
    async fn scope_selected(&self, id: &str) -> bool {
        if !self.seams.scopes.available().await {
            debug!(
                session = %id,
                "no systemd user manager; this launch relies on the process-tree sweep alone"
            );
            return false;
        }
        if crate::scope::unit_name(id, 0).is_none() {
            warn!(
                session = %id,
                "this session's id cannot safely name a systemd unit, so its launches rely on \
                 the process-tree sweep alone"
            );
            return false;
        }
        true
    }

    /// Append this launch's conversation-reporting hook flags to `argv`,
    /// or explain in the log why they were left off (plan §2.3).
    ///
    /// The hook is what makes a session's conversation identity EXACT
    /// rather than inferred: the agent itself reports the id it is
    /// actually using, which is the only thing that survives a `/clear`.
    /// Everything this function can refuse to do is a fallback to the
    /// record scan, never a failed launch — the scan is the no-hook path
    /// SPEC.md keeps promising, and a launch that cannot be hooked is
    /// still a perfectly good launch.
    ///
    /// Returns the argv to spawn and whether it was hooked; see
    /// [`with_hook_argv_using`] for the refusal list, the order it applies
    /// in, and what the bool is for.
    ///
    /// ## Placement in the launch
    ///
    /// Called from [`Supervisor::spawn_agent`], which both launch paths
    /// funnel through — so the create path and the restart path cannot
    /// disagree about whether a launch is hooked, and a resumed launch
    /// (`relaunch_argv`'s `claude --resume <id>` shape) is hooked exactly
    /// like a fresh one.
    ///
    /// Deliberately AFTER each path's own argv validation (create's at
    /// `validate_create`, restart's inside `relaunch_argv`), and the
    /// appended tail is deliberately NOT re-validated: it is a fixed set
    /// of non-empty literals this crate wrote from a path it resolved
    /// itself. Re-running the user-input checks over it would be checking
    /// this crate's own output for user mistakes, and the only outcomes
    /// available would be a false refusal or a no-op.
    fn with_hook_argv(
        &self,
        argv: Vec<String>,
        snapshot: &IntegrationSnapshot,
        session: &str,
    ) -> (Vec<String>, bool) {
        with_hook_argv_using(
            argv,
            snapshot,
            &self.seams.agent_hooks,
            self.farhelm_exe_str.as_deref(),
            session,
        )
    }

    /// Publish one launch's spec and start its window command in tmux —
    /// the side-effecting half of a launch, shared by create (PLAN_M3.md
    /// items 2/6) and restart (item 9).
    ///
    /// Shared deliberately, and this is the seam that keeps a relaunch from
    /// becoming a second, subtly different launch implementation: the spec
    /// contents, its 0600 publication, the shim path, the login-shell
    /// window command, the `{cwd}` placeholder fill, and the tmux
    /// invocation are all decided in exactly one place. A relaunch that
    /// built its own would be free to drift on any of them — a missing
    /// session-id marker (silently breaking the kill sweep), a different
    /// shell resolution (breaking SPEC.md's environment contract), a spec
    /// written world-readable, a wrapper handed a directory that is not
    /// the one the pane got.
    ///
    /// `reuse` is the ONLY difference between the two callers: `None`
    /// creates a fresh tmux session (create, and a restart whose terminal
    /// is gone), `Some(pane)` respawns into an existing pane so the prior
    /// run stays in scrollback (see [`TmuxDriver::relaunch_in_pane`]).
    ///
    /// `snapshot` is here for one reason: this is where argv becomes a
    /// process, so it is where the conversation hook has to be appended
    /// ([`Supervisor::with_hook_argv`]). Threading it in — rather than
    /// injecting in each caller — is the same argument the rest of this
    /// function rests on: two callers doing it themselves is two places
    /// for the decision to drift. Whether the hook actually went on is
    /// reported back through [`Spawned::hooked`], because the entry that
    /// records it does not exist yet on the create path.
    ///
    /// `scope` is the cgroup unit this launch was ALREADY recorded as
    /// running in (PLAN_M3.md item 10) — the caller decides and commits it
    /// durably first, so this function only spends it. It reaches the agent
    /// through the window command, never through the spec, because the spec
    /// is read by the shim and the scope has to exist before the shim runs.
    ///
    /// Failures are classified rather than flattened, because the two
    /// classes cannot be unwound the same way: a spec that never landed
    /// proves nothing external happened, while a tmux failure is AMBIGUOUS
    /// (the session can exist despite the error) and the caller must probe
    /// before deciding anything. Both carry the spec path so a caller
    /// unwinding can remove the credential-bearing file it left behind.
    #[allow(clippy::too_many_arguments)]
    async fn spawn_agent(
        &self,
        id: &str,
        session_token: &str,
        generation: i64,
        tmux_name: &str,
        argv: Vec<String>,
        snapshot: &IntegrationSnapshot,
        cwd: &str,
        cols: u16,
        rows: u16,
        reuse: Option<&Terminal>,
        preamble: Option<Vec<u8>>,
        scope: Option<&str>,
    ) -> Result<Spawned, SpawnFailure> {
        // Placeholder substitution is the FIRST transformation and hook
        // injection the second, so the injected tail — literals this crate
        // wrote, never re-validated — is never itself a substitution
        // target. `cwd` here is the directory tmux gets for this launch,
        // which is the whole reason the fill lives in this function and
        // not at the split sites: it is the one value that makes "the
        // wrapper's directory" and "the pane's directory" the same string
        // on every path. The log names the cwd and never the argv, which
        // users do put credentials into (see `validate_create`).
        let argv = if crate::agent_kind::has_cwd_placeholder(&argv) {
            info!(session = %id, cwd, "substituting the working directory into the agent command line");
            crate::agent_kind::fill_cwd(argv, cwd)
        } else {
            argv
        };
        // The last thing done to the argv before it is written down —
        // after the fill above, and nothing after it: the spec on disk is
        // what the shim execs, so the flags have to be in it, and putting
        // the injection here is what lets the shim stay ignorant of hooks
        // entirely.
        let (argv, hooked) = self.with_hook_argv(argv, snapshot, id);
        let spec_path = crate::launch::spec_path_for_launch(&self.state_dir, id, generation);
        // Derived the SAME way the shim derives it from its own copy of
        // `spec_path` (`launch::status_path_for_spec`) — never computed
        // independently here — so the two sides can never disagree about
        // where a launch failure gets recorded, including for the failure
        // classes (missing/malformed spec) where the shim never gets to
        // read this struct's own `status_file` field at all.
        let status_path = crate::launch::status_path_for_spec(&spec_path);
        let spec = LaunchSpec {
            argv,
            status_file: status_path.clone(),
            // The kill machinery's environment-marker sweep (see
            // `kill_process_tree`) is keyed on this exact value reaching
            // the agent's process and everything it forks.
            session_id: id.to_string(),
            session_token: session_token.to_string(),
            supervisor_sock: Self::socket_path(&self.state_dir),
            farhelm_bin_dir: self
                .farhelm_exe
                .parent()
                .expect("an absolute farhelm executable has a parent directory")
                .to_path_buf(),
            // Only ever set by a restart reusing a terminal whose visible
            // frame tmux cannot carry across the respawn itself; see
            // `LaunchSpec::preamble`.
            preamble: preamble.unwrap_or_default(),
        };
        // Serialized before the write so the (practically impossible)
        // encoding failure shares the write's rollback path rather than
        // returning past it and stranding the launching row.
        let spec_bytes = match serde_json::to_vec(&spec) {
            Ok(bytes) => bytes,
            Err(e) => {
                return Err(SpawnFailure::Spec(
                    anyhow::Error::new(e).context("encoding launch spec"),
                ));
            }
        };
        // 0600 from the first byte: the spec holds the full agent command
        // line, which users do put credentials into (`--api-key ...`).
        // Mode is set at open, not chmod-after-write — a write-then-chmod
        // leaves a window where the default umask exposes the contents.
        // A failed write cleans up too: a partial spec (disk full after
        // create) would otherwise strand a credential prefix on disk
        // until the next supervisor restart's sweep. The write PUBLISHES BY
        // HARD LINK and therefore refuses to replace an existing file
        // (`files::write_private_file_sync`) — which is exactly the check
        // that makes generation-scoped naming load-bearing rather than
        // cosmetic: every launch writes a path no launch has used before,
        // so this can only fail on a genuine collision, never on a
        // predecessor's leftovers.
        if let Err(e) = crate::write_private_file(&spec_path, &spec_bytes).await {
            return Err(SpawnFailure::Spec(
                anyhow::Error::new(e).context("writing launch spec"),
            ));
        }

        let shell = self.launch_shell().await;
        // The scope wrapper, or nothing at all. Note the asymmetry with the
        // rest of this function: `scope` is DECIDED and RECORDED durably by
        // the caller, before the launching row commits, and only consumed
        // here — the selection has to precede the side effect it describes
        // (item 2's ordering rule), or a crash mid-launch would leave a row
        // that cannot say what stop should do.
        //
        // A selected scope whose prefix cannot be built is a contradiction
        // this supervisor can only have reached by losing its user manager
        // between the selection and now. Launching UNWRAPPED is the right
        // answer to it — never worse than M2 — and it is loud, because the
        // row will go on claiming a scope that no longer exists.
        let scope_prefix = match scope {
            Some(unit) => match self.seams.scopes.launch_prefix(unit).await {
                Some(prefix) => prefix,
                None => {
                    warn!(
                        session = %id, unit,
                        "this launch selected a cgroup scope but the user manager is no longer \
                         usable; launching without one, so stop falls back to the \
                         process-tree sweep"
                    );
                    Vec::new()
                }
            },
            None => Vec::new(),
        };
        let cmd = window_command(&shell, &self.farhelm_exe, &spec_path, scope_prefix);
        let started = match reuse {
            Some(terminal) => self
                .tmux
                .relaunch_in_pane(
                    &terminal.tmux_name,
                    &terminal.pane,
                    cwd,
                    &self.seams.launch_env,
                    &cmd,
                )
                .await
                .map(|()| terminal.pane.clone()),
            None => self
                .tmux
                .create_session(tmux_name, cwd, cols, rows, &self.seams.launch_env, &cmd)
                .await
                .map_err(|e| e.context("creating the session's tmux session")),
        };
        match started {
            Ok(pane) => {
                // Mark the agent's window (PLAN_M4.md item 2). Only on the
                // fresh-session path: a REUSED pane's window was marked
                // when its session was created, and `respawn-pane` keeps
                // the window (and so its options) intact.
                //
                // After the launch rather than before it, because the
                // window does not exist until then — and FATAL, not best
                // effort. The marker is what a pane-less reload uses to
                // tell this session's agent window from its tabs
                // (`agent_pane_from_states`), so an unmarked agent window
                // is a session that can later recover onto a tab's pane
                // and reap it. `SpawnFailure::Tmux` is the right shape for
                // it too: the tmux session genuinely exists at this point,
                // which is exactly what that variant's unwind expects.
                if reuse.is_none()
                    && let Err(error) = self
                        .tmux
                        .mark_window(tmux_name, &pane, AGENT_WINDOW_OPTION, id)
                        .await
                {
                    return Err(SpawnFailure::Tmux {
                        spec_path,
                        error: error.context(
                            "marking the session's agent window, without which a later reload \
                             could not tell it apart from a terminal tab",
                        ),
                    });
                }
                Ok(Spawned {
                    pane,
                    spec_path,
                    status_path,
                    hooked,
                })
            }
            Err(error) => Err(SpawnFailure::Tmux { spec_path, error }),
        }
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
    pub(crate) fn may_record(&self) -> bool {
        self.may_record.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Record the conversation identity a session's own agent reported
    /// from inside its process, through the launch hook.
    ///
    /// This is the authoritative answer to the question the capture scan
    /// can only infer: the agent names its own conversation, so the result
    /// dominates every scan-derived verdict (see [`CaptureState`]'s
    /// ladder). A second report for the same launch REPLACES the first —
    /// `/clear` and `/new` start a new conversation inside a running
    /// process, and the id they retire is exactly the one that must not be
    /// resumed again.
    ///
    /// ## The durable write decides what is claimed
    ///
    /// Modelled on `super::capture::commit_capture`: the in-memory
    /// [`CaptureState::Reported`] is entered only AFTER the store write
    /// has landed, because `committed_conversation` — and therefore
    /// `farhelm_proto::RestartOffer::Resume` — promises a restart that
    /// there is a stored id for it to fill in. A failed write is logged
    /// and changes nothing in memory, with no retry list: neither vendor
    /// re-fires the hook, so that report is simply lost, and the scan is
    /// still running for this session precisely because it never reached
    /// `Reported`. A store that cannot write is a supervisor in trouble,
    /// not a state to engineer a queue around.
    ///
    /// The same "no retry" rule covers a failed REPLACEMENT, and there it
    /// is worth naming what it costs (plan §2.5): when a `/clear` report
    /// cannot be written, the PREVIOUS identity keeps standing, durably
    /// and in memory, so the session goes on offering to resume a
    /// conversation the user has discarded until the next report or
    /// relaunch. That is deliberately not repaired here. A retry queue for
    /// this write would have to survive the process to be worth anything,
    /// which means durable state describing an intention rather than a
    /// fact — and the failure it would serve is a store this supervisor
    /// can no longer write to at all.
    ///
    /// ## The publication gap
    ///
    /// A report can arrive before the session has an in-memory entry.
    /// Claude's hook fires at process start, which lands inside two real
    /// windows: a create between reserving the row and publishing the
    /// entry, and a relaunch between opening the new generation and
    /// publishing the replacement. `NotFound` there would throw away a
    /// perfectly good report at exactly the moment the vendor's own event
    /// says it is most reliable.
    ///
    /// So the durable ROW is the fallback authority for the generation:
    /// it exists from reservation onwards, which is strictly earlier than
    /// any hook can run. The write goes through and the in-memory advance
    /// is simply skipped, because there is no cell to advance yet.
    ///
    /// ### Why the memory that appears afterwards converges
    ///
    /// Worth spelling out, because the naive reading — "the entry that
    /// shows up next will be built from the row" — is NOT what happens and
    /// a reviewer who checks will find it does not. The create path mints
    /// its entry with [`CaptureState::Unclaimed`] unconditionally, and
    /// `relaunched_entry` either mints `Unclaimed` or clones the previous
    /// entry's cell; neither reads these columns. Only a supervisor RELOAD
    /// derives the capture state from the row. So for a create-gap report,
    /// the durable row says `Reported` while memory says `Unclaimed`, and
    /// they stay that way for a while. That divergence is accepted rather
    /// than repaired, on three legs:
    ///
    /// - **Nothing can spoil the row in the meantime.** Both scan writers
    ///   are fenced against exactly this shape:
    ///   [`crate::store::SessionStore::record_captured_conversation`]
    ///   demands `captured_conversation IS NULL` and
    ///   [`crate::store::SessionStore::record_capture_ambiguous`] demands
    ///   `conversation_source IS NULL`. A hook-written row fails both, so
    ///   an `Unclaimed` mirror cannot talk a pass into overwriting the
    ///   reported identity or into recording an ambiguity over it.
    /// - **Everything that ACTS on the identity reads the row, not the
    ///   mirror.** `session_snapshot` builds `resume_argv` and the restart
    ///   mode validation from `captured_conversation` in SQLite, so a
    ///   restart landing inside the divergence resumes the reported
    ///   conversation regardless of what memory holds.
    /// - **The mirror catches up on its own.** The next capture pass that
    ///   reaches a commit has its write-once UPDATE refused by the fence
    ///   above and reads the row back; `commit_capture` then advances the
    ///   entry to the id it read — the reported one — with an empty record
    ///   path that re-verification fills in. A reload does the same thing
    ///   more directly.
    ///
    /// What the divergence costs is one thing only: `session_restart_offer`
    /// reads the ENTRY, so `ListSessions` advertises `FreshOnly` for the
    /// interval. That is an understatement of what the session can do, and
    /// an understatement is the safe direction — it never offers a resume of
    /// a conversation the agent is not in. Nothing is lost by not advancing
    /// memory here; something would be lost by refusing the report.
    ///
    /// ## It takes NO lifecycle claim, deliberately
    ///
    /// Unlike every neighbouring session operation. `restart_session`
    /// holds a session's lifecycle claim for the WHOLE restart, and
    /// Claude's hook fires at the new agent process's startup — squarely
    /// inside that window. Waiting on the claim would queue the report
    /// behind the tail of the restart that caused it and blow the hook's
    /// own 2 s budget under load, which the vendor surfaces as a hook
    /// error in the user's terminal and never retries.
    ///
    /// The generation fence in the store is what protects the write
    /// instead, and its one accepted gap is worth stating: a session's
    /// token survives a relaunch, so a hook process from the PREVIOUS
    /// generation that somehow outlived the restart's kill sweep would
    /// still pass the credential check at the connection. It cannot do
    /// damage, because `record_reported_conversation` is fenced on the
    /// generation this entry was published with and the stale process's
    /// generation is gone; the accepted case is exactly the one the sweep
    /// already makes unreachable, since it kills the whole process tree —
    /// the hook included — before the new generation exists.
    pub(crate) async fn report_conversation(
        &self,
        id: &str,
        conversation: String,
        source: String,
    ) -> Result<(), RequestError> {
        let entry = self.sessions.lock().await.get(id).cloned();
        // The same gate every scan write honours. A supervisor that is not
        // recording is shutting down or has been replaced, and a report it
        // accepted would be a conclusion it has no standing to draw. It is
        // not parked for later either: queuing a write past this gate is
        // the precise thing `may_record` exists to prevent.
        if !self.may_record() {
            return Err(RequestError::new(
                ErrorKind::Internal,
                "the supervisor is not recording right now",
            ));
        }
        // Which LAUNCH this report is about. The entry is the authority
        // when there is one; the row stands in during a publication gap
        // (see the docs above), and it can do so because the reservation
        // commits it before anything is spawned. Only when NEITHER exists
        // is this genuinely a report for a session that is gone.
        let generation = match &entry {
            Some(entry) => entry.generation,
            None => match self.store.session(id).await {
                Ok(Some(row)) => row.generation,
                Ok(None) => {
                    return Err(RequestError::new(
                        ErrorKind::NotFound,
                        format!("no such session: {}", truncate_for_error(id)),
                    ));
                }
                Err(e) => {
                    warn!(
                        session = %id, conversation = %conversation, source = %source,
                        error = %format!("{e:#}"),
                        "could not read the session row a report arrived for before its \
                         entry was published; the report is discarded"
                    );
                    return Err(RequestError::new(
                        ErrorKind::Internal,
                        "the reported conversation identity could not be recorded",
                    ));
                }
            },
        };
        // The injected failure STANDS IN for the store call rather than
        // preceding it, so a test can exercise this function's own failure
        // path without a store that is genuinely broken.
        let injected = self
            .seams
            .capture_store_fault
            .as_ref()
            .map(|fault| fault(super::capture::CaptureWrite::Report, id));
        let written = match injected {
            Some(Err(e)) => Err(e),
            _ => {
                self.store
                    .record_reported_conversation(id, generation, &conversation)
                    .await
            }
        };
        match written {
            Ok(true) => {}
            // The row moved out from under this report: the session was
            // relaunched (a new generation) or deleted between the entry
            // read above and the write. `Conflict` rather than `Internal`
            // because nothing malfunctioned — the report is simply about a
            // launch that no longer exists, and the surviving launch's own
            // hook is the one entitled to speak for it.
            Ok(false) => {
                warn!(
                    session = %id, conversation = %conversation, source = %source,
                    generation,
                    "this session's agent reported a conversation identity for a launch \
                     that is no longer current; the report is discarded"
                );
                return Err(RequestError::new(
                    ErrorKind::Conflict,
                    "this session has moved on to another launch",
                ));
            }
            Err(e) => {
                warn!(
                    session = %id, conversation = %conversation, source = %source,
                    error = %format!("{e:#}"),
                    "could not record the conversation identity this session's agent \
                     reported; nothing will retry it and the scan remains the fallback"
                );
                return Err(RequestError::new(
                    ErrorKind::Internal,
                    "the reported conversation identity could not be recorded",
                ));
            }
        }
        // The publication gap: the row now holds the report, and there is
        // no in-memory cell to mirror it into. The entry that appears next
        // will NOT be built from these columns — see the "why the memory
        // that appears afterwards converges" section above for the three
        // things that make that divergence harmless and self-correcting.
        let Some(entry) = entry else {
            info!(
                session = %id, conversation = %conversation, source = %source, generation,
                "this session's agent reported a conversation identity before its entry \
                 was published; the durable row carries it until the entry appears"
            );
            return Ok(());
        };
        // Read the outgoing claim under the same lock that installs the
        // new one, so the line below describes the transition that
        // actually happened rather than one a racing pass has since
        // changed.
        let displaced = {
            let mut state = entry.capture.lock().expect("capture mutex poisoned");
            let previous = state.committed_conversation().map(str::to_string);
            state.advance(CaptureState::Reported {
                conversation: conversation.clone(),
            });
            previous.filter(|was| was != &conversation)
        };
        info!(
            session = %id, conversation = %conversation, source = %source,
            "recorded the conversation identity this session's agent reported"
        );
        // Only a DIFFERENT id is worth a second line. A scan claim that
        // agrees with the report is the ordinary Codex sequence — the
        // record usually lands before the first-prompt hook fires — and
        // logging it would bury the case that matters underneath it: an
        // agent that started a new conversation before its first prompt,
        // whose earlier id must stop being offered.
        if let Some(was) = displaced {
            info!(
                session = %id, was = %was, now = %conversation,
                "this session's agent reported a conversation identity that replaces the \
                 one previously claimed for it"
            );
        }
        Ok(())
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
    pub(crate) async fn record(
        &self,
        session: &str,
        entry: &SessionEntry,
        transition: Transition,
    ) -> anyhow::Result<()> {
        if !self.may_record() {
            return Ok(());
        }
        // Fenced on the entry's own generation: an observation made against
        // a run a restart has since replaced describes something that is no
        // longer true, and the store drops it rather than letting it land
        // on the new run (see `SessionEntry::generation`).
        if let Some(committed) = self
            .store
            .transition(session, entry.generation, transition)
            .await?
        {
            *entry.outcome.lock().expect("outcome mutex poisoned") = committed;
        }
        Ok(())
    }
}

/// Preserve every cleanup failure after a tab's window is irreversibly gone.
///
/// A retry cannot recover the killed window, so dropping either the process
/// sweep error or the output-client error would discard the only actionable
/// account of what may still be alive.
fn tab_close_cleanup_result(
    tab_id: &str,
    survivors: anyhow::Result<()>,
    detach: Result<(), RequestError>,
) -> Result<(), RequestError> {
    match (survivors, detach) {
        (Ok(()), Ok(())) => Ok(()),
        (Ok(()), Err(detach)) => Err(detach),
        (Err(survivors), Ok(())) => Err(RequestError::new(
            ErrorKind::Internal,
            format!(
                "terminal tab {}'s window is gone but survivors of its shell could not be \
                 confirmed reaped: {survivors:#}",
                truncate_for_error(tab_id)
            ),
        )),
        (Err(survivors), Err(detach)) => Err(RequestError::new(
            ErrorKind::Internal,
            format!(
                "terminal tab {}'s window is gone, but process cleanup failed ({survivors:#}) \
                 and terminal-output cleanup also failed ({})",
                truncate_for_error(tab_id),
                detach.message
            ),
        )),
    }
}

/// `farhelm supervisor run` in one call: build a supervisor on `state_dir`
/// and serve its socket until the process dies. Returns only on a fatal
/// error — a successful supervisor never returns.
///
/// `startup` carries the already-resolved process-startup overrides (see
/// [`SupervisorStartup`]), and every one of them is REQUIRED rather than
/// optional: this is the whole supervisor entry point, so a caller that
/// could omit one is a launch path that silently ignores `--tmux`,
/// `FARHELM_TMUX`, or `FARHELM_AGENT_HOOKS`.
pub async fn run(state_dir: &Path, startup: SupervisorStartup) -> anyhow::Result<()> {
    let sup = Supervisor::new_for_startup(state_dir, startup).await?;
    sup.serve().await
}

/// Connect to a running supervisor's socket (used by `internal stdio`).
/// Fails if no supervisor is listening; this deliberately does not start
/// one, because discovery-first (SPEC.md) means an absent supervisor is
/// the caller's decision to make, not a side effect of dialing.
pub async fn connect(state_dir: &Path) -> anyhow::Result<UnixStream> {
    let path = Supervisor::socket_path(state_dir);
    UnixStream::connect(&path).await.map_err(|e| {
        // `ConnectionRefused` (nothing is listening on the socket file)
        // and `NotFound` (no socket file at all) are the shapes a plain
        // "no supervisor here" takes at this layer. Naming that directly —
        // and the fix — is what turns a raw "Connection refused (os error
        // 111)" into something an operator can act on without knowing
        // this is a unix-domain-socket dial at all.
        //
        // Linux also answers `ConnectionRefused` for a path that EXISTS
        // but is not a socket, so a stale regular file squatting the name
        // lands in this branch too — and the remedy still holds, because
        // `Supervisor::serve` unlinks whatever it finds at the socket path
        // before binding. (A DIRECTORY there refuses connections the same
        // way and is not fixed by the remedy; it is a strange enough state
        // that the bind error the remedy then produces is the clearer
        // place to learn about it.) Other kinds — permission denied, a
        // non-directory component in the path — keep the generic context:
        // they need the raw error, not a remedy that would not apply.
        //
        // `--state-dir` is always spelled out: `internal stdio` is
        // normally reached over ssh with a state dir the remote's default
        // would not match, and a remedy that silently drops it starts a
        // supervisor in the wrong place. The quoting is for spaces, so the
        // line survives a paste into a shell; `to_string_lossy` is no more
        // lossy than the `Path::display` already used for the socket path
        // beside it, and this whole string is advice for a human.
        let context = if matches!(
            e.kind(),
            std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound
        ) {
            format!(
                "supervisor does not appear to be running (socket {} is not accepting \
                 connections); start it with `farhelm supervisor run --state-dir {}`",
                path.display(),
                shell_words::quote(&state_dir.to_string_lossy()),
            )
        } else {
            format!("connecting to supervisor socket {}", path.display())
        };
        anyhow::Error::new(e).context(context)
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::connection::{CONNECTION_WRITER_QUEUE, ConnectionCtx};
    use super::super::handlers::handle_control;
    use super::super::status::session_status;
    use super::super::uploads::UploadRoute;
    use super::*;
    use farhelm_proto::{ControlMsg, Frame};
    use tokio::sync::mpsc;

    /// An empty upload routing map, for the many tests that drive
    /// `handle_control` directly with no transfer in flight.
    ///
    /// A helper rather than a local per test because it is always the
    /// same empty map and always for the same reason: these tests are
    /// about some other message entirely, and the routes exist only
    /// because a connection's read loop owns both maps (see
    /// [`UploadRoute`]).
    pub(crate) fn no_uploads() -> HashMap<u32, UploadRoute> {
        HashMap::new()
    }

    /// The `~` expansion contract, form by form (BUGS_BURNDOWN.md issue 1,
    /// SPEC.md's `~`-resolves-on-the-target-host rule).
    ///
    /// Pinned as a unit test because the whole point of the function is
    /// the edge inventory: bare `~` is home itself (not `home/`), `~/` is
    /// also home rather than a trailing-slash artifact, `~user` is a
    /// refusal and not a mangled expansion, and everything not starting
    /// with `~` — including a relative path that `ensure_cwd_usable` will
    /// refuse later — passes through untouched. The degenerate homes each
    /// pin an error CLASSIFICATION, not just an error, because the kind
    /// decides who goes hunting: a MISSING home is `InvalidRequest` (the
    /// caller can answer it by sending an absolute path), a relative or
    /// non-UTF-8 home is `Internal` (host misconfiguration no different
    /// request can route around), and `~user` is `InvalidRequest` caller
    /// misuse. All of them name the actual problem in the message.
    #[test]
    fn tilde_expansion_covers_every_form() {
        let kind_of = |err: &anyhow::Error| {
            err.downcast_ref::<RequestError>()
                .expect("expansion refusals carry a RequestError")
                .kind
        };
        let home = Path::new("/home/someone");
        let expand = |cwd: &str| expand_tilde_cwd(cwd, Some(home)).map(|c| c.into_owned());
        assert_eq!(expand("~").unwrap(), "/home/someone");
        assert_eq!(expand("~/").unwrap(), "/home/someone");
        assert_eq!(expand("~/ws/repo").unwrap(), "/home/someone/ws/repo");
        assert_eq!(expand("/abs/path").unwrap(), "/abs/path");
        assert_eq!(expand("relative/path").unwrap(), "relative/path");

        // A home with a trailing slash must not produce a double slash.
        assert_eq!(
            expand_tilde_cwd("~/x", Some(Path::new("/home/someone/")))
                .unwrap()
                .into_owned(),
            "/home/someone/x"
        );

        let user_form = expand("~other").unwrap_err();
        assert_eq!(kind_of(&user_form), ErrorKind::InvalidRequest);
        let user_form = user_form.to_string();
        assert!(
            user_form.contains("~other") && user_form.contains("not"),
            "a ~user refusal must name the path and say it is unsupported: {user_form}"
        );
        // `~other/sub` is the same refusal, not a partial expansion.
        assert!(expand("~other/sub").is_err());

        let no_home = expand_tilde_cwd("~", None).unwrap_err();
        assert_eq!(
            kind_of(&no_home),
            ErrorKind::InvalidRequest,
            "no home is answerable by the caller (send an absolute path), not a server fault"
        );
        let no_home = no_home.to_string();
        assert!(
            no_home.contains("home"),
            "a no-home refusal must name the missing home, not claim the path is relative: \
             {no_home}"
        );

        let relative_home = expand_tilde_cwd("~/x", Some(Path::new("home"))).unwrap_err();
        assert_eq!(
            kind_of(&relative_home),
            ErrorKind::Internal,
            "a relative home is host misconfiguration, not a caller mistake"
        );
        assert!(
            relative_home.to_string().contains("home"),
            "a relative-home refusal must name the home: {relative_home:#}"
        );

        // A non-UTF-8 home cannot become part of a `String`-typed cwd; the
        // refusal is the host's problem and the DIAGNOSTIC is part of the
        // contract — a generic internal error would strip the operator's
        // only pointer at what to fix.
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let raw = std::ffi::OsStr::from_bytes(b"/home/\xff");
            let non_utf8 = expand_tilde_cwd("~", Some(Path::new(raw))).unwrap_err();
            assert_eq!(kind_of(&non_utf8), ErrorKind::Internal);
            let message = non_utf8.to_string();
            assert!(
                message.contains("home") && message.contains("not valid UTF-8"),
                "the refusal must name the home and the encoding problem: {message}"
            );
        }
    }

    /// Tab close reports both kinds of cleanup it could not confirm.
    ///
    /// The window is already gone when these failures are combined, so a
    /// second close cannot recover whichever diagnostic the first response
    /// discards. Keeping both is the only way to tell an operator about a
    /// surviving process and an unresolved output client at the same time.
    #[test]
    fn tab_close_preserves_process_and_output_cleanup_failures() {
        let error = tab_close_cleanup_result(
            "tab-1",
            Err(anyhow::anyhow!("injected survivor")),
            Err(RequestError::new(
                ErrorKind::Internal,
                "injected output cleanup",
            )),
        )
        .expect_err("both failed cleanup paths must fail the close");

        assert!(
            error.message.contains("injected survivor"),
            "the process-sweep failure must survive aggregation: {error:?}"
        );
        assert!(
            error.message.contains("injected output cleanup"),
            "the output-client failure must survive aggregation: {error:?}"
        );
    }

    /// A supervisor state directory that also kills the private tmux
    /// server rooted inside it when it goes out of scope.
    ///
    /// Drop-based, and the drop is the whole point: a test that fails an
    /// assertion never reaches an explicit teardown, and every test here
    /// that creates a session starts a tmux server that nothing else will
    /// ever stop. Measured before this existed: one `cargo test -p
    /// farhelm-supervisor --lib` run left 24 tmux servers behind, and they
    /// accumulate across runs until the host is visibly degraded (~2,000
    /// were found on the development machine).
    ///
    /// Wraps the `TempDir` rather than sitting beside it so the ordering is
    /// not a rule each test has to remember: the server is killed in this
    /// type's own `Drop`, which runs before the inner `TempDir`'s and
    /// therefore before the directory holding the server's socket is
    /// removed. Its `path()` mirrors `TempDir::path`, so call sites read
    /// exactly as they did.
    pub(crate) struct StateDir(tempfile::TempDir);

    impl StateDir {
        pub(crate) fn new() -> StateDir {
            StateDir(tempfile::tempdir().expect("state dir"))
        }

        pub(crate) fn path(&self) -> &std::path::Path {
            self.0.path()
        }
    }

    /// How long [`StateDir`]'s teardown waits for `tmux kill-server` before
    /// giving up on it.
    ///
    /// `Drop` cannot await, so this is a real blocking wait on a test
    /// thread — and an unbounded one would let a single wedged tmux hang
    /// the whole suite with no output and no clue why. A `kill-server` that
    /// has not returned in this long is not going to; the server it was
    /// aimed at leaks, which is a bounded cost, unlike the hang.
    const KILL_SERVER_DEADLINE: Duration = Duration::from_secs(10);

    impl Drop for StateDir {
        fn drop(&mut self) {
            // Best-effort: the common case is a test that never started a
            // server at all, and `kill-server` on an absent socket is an
            // error worth ignoring rather than reporting.
            let Ok(mut child) = std::process::Command::new("tmux")
                .arg("-S")
                .arg(self.0.path().join("tmux.sock"))
                .arg("kill-server")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
            else {
                return;
            };
            let deadline = std::time::Instant::now() + KILL_SERVER_DEADLINE;
            loop {
                match child.try_wait() {
                    Ok(Some(_)) | Err(_) => return,
                    Ok(None) if std::time::Instant::now() >= deadline => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return;
                    }
                    Ok(None) => std::thread::sleep(Duration::from_millis(20)),
                }
            }
        }
    }

    /// A snapshot shaped exactly as `session_snapshot` would build one for
    /// a session with `offer` — used by the mode/offer matrix below, which
    /// is about the PAIRING rules rather than about how an offer is
    /// derived (`IntegrationSnapshot::restart_offer` owns that, and its own
    /// tests cover it).
    fn snapshot_offering(offer: RestartOffer) -> SessionSnapshot {
        let (kind, resume_template, captured_conversation, resume_argv) = match offer {
            RestartOffer::FreshOnly => (AgentKind::Generic, None, None, None),
            RestartOffer::Resume => (
                AgentKind::Claude,
                Some(vec![
                    "claude".to_string(),
                    "--resume".to_string(),
                    "{conversation}".to_string(),
                ]),
                Some("conv-1".to_string()),
                Some(vec![
                    "claude".to_string(),
                    "--resume".to_string(),
                    "conv-1".to_string(),
                ]),
            ),
            RestartOffer::FallbackTemplate => (
                AgentKind::Generic,
                Some(vec!["agent".to_string(), "--continue".to_string()]),
                None,
                None,
            ),
        };
        SessionSnapshot {
            kind,
            resume_template,
            captured_conversation,
            restart_offer: offer,
            resume_argv,
            first_input_at: None,
            capture_ambiguous: false,
            canonical_cwd: None,
        }
    }

    /// The whole mode/offer matrix, in one place: exactly one mode is legal
    /// per offer, and every other pairing is a `Conflict` rather than a
    /// best-effort substitution.
    ///
    /// The diagonal matters as much as the off-diagonal. `Fresh` against a
    /// `Resume` offer is the one a well-meaning client is most likely to
    /// send ("the user just wants a restart"), and SPEC.md is explicit that
    /// v1 has no such downgrade — for a clean conversation you create a new
    /// session. `Resume` against `FreshOnly` is the mirror image and the
    /// more dangerous one: honoring it could only mean running a
    /// `{conversation}` template with nothing to fill it.
    #[test]
    fn each_restart_offer_accepts_exactly_one_mode() {
        for offer in [
            RestartOffer::FreshOnly,
            RestartOffer::Resume,
            RestartOffer::FallbackTemplate,
        ] {
            let snapshot = snapshot_offering(offer);
            for mode in [
                RestartMode::Fresh,
                RestartMode::Resume,
                RestartMode::FallbackTemplate,
            ] {
                let result = relaunch_argv(mode, &snapshot, "agent --flag");
                let legal = matches!(
                    (offer, mode),
                    (RestartOffer::FreshOnly, RestartMode::Fresh)
                        | (RestartOffer::Resume, RestartMode::Resume)
                        | (
                            RestartOffer::FallbackTemplate,
                            RestartMode::FallbackTemplate
                        )
                );
                match result {
                    Ok(argv) => assert!(
                        legal,
                        "mode {mode:?} must not be accepted for offer {offer:?}, got {argv:?}"
                    ),
                    Err(e) => {
                        assert!(
                            !legal,
                            "mode {mode:?} must be accepted for offer {offer:?}: {e:#}"
                        );
                        assert_eq!(
                            error_kind(&e),
                            ErrorKind::Conflict,
                            "a mismatched mode is a staleness conflict, not a bad request"
                        );
                    }
                }
            }
        }
    }

    /// The refusal has to NAME the current offer, because the client's
    /// prescribed response is to refresh and re-present it (the wire
    /// vocabulary's staleness contract) — an unqualified "conflict" would
    /// leave it with nothing to show the user.
    #[test]
    fn a_mismatched_mode_names_the_current_offer() {
        let err = relaunch_argv(
            RestartMode::Fresh,
            &snapshot_offering(RestartOffer::Resume),
            "agent",
        )
        .expect_err("fresh is not legal against a resume offer");
        let message = format!("{err:#}");
        assert!(
            message.contains("resum"),
            "the refusal must name the current offer: {message}"
        );
    }

    /// Command construction per mode, including the property the whole
    /// resume promise rests on: the conversation id arrives in its OWN argv
    /// element, substituted rather than spliced, and no placeholder ever
    /// survives into something that gets executed.
    #[test]
    fn each_mode_builds_its_own_command() {
        let resumed = relaunch_argv(
            RestartMode::Resume,
            &snapshot_offering(RestartOffer::Resume),
            "claude --dangerously-skip-permissions",
        )
        .expect("resume is legal against a resume offer");
        assert_eq!(resumed, vec!["claude", "--resume", "conv-1"]);
        assert!(
            !resumed.iter().any(|e| e.contains("{conversation}")),
            "a placeholder must never reach a command line: {resumed:?}"
        );

        let fallback = relaunch_argv(
            RestartMode::FallbackTemplate,
            &snapshot_offering(RestartOffer::FallbackTemplate),
            "agent --launch-only",
        )
        .expect("the fallback template is legal against its own offer");
        assert_eq!(
            fallback,
            vec!["agent", "--continue"],
            "the configured template runs verbatim, never the launch invocation"
        );

        let fresh = relaunch_argv(
            RestartMode::Fresh,
            &snapshot_offering(RestartOffer::FreshOnly),
            "agent 'one arg' --flag",
        )
        .expect("fresh is legal against a fresh-only offer");
        assert_eq!(
            fresh,
            vec!["agent", "one arg", "--flag"],
            "a fresh relaunch re-parses the session's own invocation, quoting included"
        );
    }

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

    /// PLAN_M3.md item 2's "unsupported host" branch: a host with no
    /// `/proc/sys/kernel/random/boot_id` at all (simulated by pointing at
    /// a path that plain does not exist) must come back `Ok(None)`, never
    /// an `Err` — see [`BootIdSource`]'s docs for why the two outcomes
    /// drive opposite reload behavior and must not be collapsed.
    #[test]
    fn read_boot_id_from_a_missing_path_is_ok_none() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("boot_id");
        assert_eq!(read_boot_id_from(&missing).unwrap(), None);
    }

    /// An empty (or whitespace-only) boot-id file is not a usable id;
    /// pinned as `Ok(None)` rather than `Ok(Some(""))`, matching the
    /// production reasoning inline in `read_boot_id_from`: storing an
    /// empty string would make a later REAL id look like a reboot on no
    /// actual evidence.
    #[test]
    fn read_boot_id_from_an_empty_or_blank_file_is_ok_none() {
        let tmp = tempfile::tempdir().unwrap();
        let empty = tmp.path().join("boot_id");
        std::fs::write(&empty, b"").unwrap();
        assert_eq!(read_boot_id_from(&empty).unwrap(), None);

        let blank = tmp.path().join("boot_id_blank");
        std::fs::write(&blank, b"  \n\t\n").unwrap();
        assert_eq!(read_boot_id_from(&blank).unwrap(), None);
    }

    /// The ordinary case: a real boot-id line, trimmed of the trailing
    /// newline `/proc` files carry, comes back as `Ok(Some(<trimmed>))`.
    #[test]
    fn read_boot_id_from_a_normal_file_returns_the_trimmed_id() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("boot_id");
        std::fs::write(&path, b"1234abcd-56ef-78ab-90cd-ef1234567890\n").unwrap();
        assert_eq!(
            read_boot_id_from(&path).unwrap(),
            Some("1234abcd-56ef-78ab-90cd-ef1234567890".to_string())
        );
    }

    /// The `Err` branch, distinct from `Ok(None)`: a path that exists but
    /// cannot be read as a file must fail loudly rather than being
    /// silently treated as "unsupported host" — reload degrades instead
    /// of guessing (see `BootIdSource`'s docs). A directory is the
    /// portable way to force that read error without `chmod` tricks,
    /// which break when the test runs as root (repo rule).
    #[test]
    fn read_boot_id_from_an_unreadable_path_is_err() {
        let tmp = tempfile::tempdir().unwrap();
        let as_dir = tmp.path().join("boot_id");
        std::fs::create_dir(&as_dir).unwrap();
        assert!(read_boot_id_from(&as_dir).is_err());
    }

    /// The Mac boot id must be present, non-empty, and STABLE across two
    /// reads — stability within one boot is the property that made
    /// `kern.bootsessionuuid` the right source over PLAN_M3.md's presumed
    /// `kern.boottime`, which the kernel rewrites when NTP steps the
    /// clock. Two reads disagreeing here would mean reboot classification
    /// can fire without a reboot, mass-interrupting live sessions on an
    /// ordinary supervisor restart.
    #[cfg(target_os = "macos")]
    #[test]
    fn the_mac_boot_session_uuid_is_present_and_stable() {
        let first = read_host_boot_id()
            .expect("reading kern.bootsessionuuid must not error")
            .expect("modern macOS publishes kern.bootsessionuuid");
        let second = read_host_boot_id().unwrap().unwrap();
        assert_eq!(first, second, "the id must be stable within one boot");
        assert!(
            !first.contains('\0'),
            "the sysctl's trailing NUL must have been trimmed: {first:?}"
        );
    }

    /// A dummy launch-shim path for tests that never create a session:
    /// `Supervisor::new_with_exe` never touches this path itself (only
    /// `create_session` does, via `window_command`), so a nonexistent
    /// file is fine wherever a test's request is rejected — by the
    /// `CREATE_FIELD_CAP` guard, say — before any side effect happens.
    pub(crate) fn dummy_exe() -> PathBuf {
        PathBuf::from("/nonexistent/farhelm")
    }

    /// A session entry with the given terminal and recorded outcome, for
    /// the entry-replacement tests below and the classification tests in
    /// `service::status` — which are about how those two inputs combine,
    /// and need no tmux, no store, and no session at all.
    pub(crate) fn entry_with(terminal: Option<Terminal>, outcome: LastOutcome) -> SessionEntry {
        SessionEntry {
            info: SessionInfo {
                parent: None,
                archived: false,
                id: "s1".to_string(),
                title: "t".to_string(),
                created_at: 1_700_000_000,
                last_activity_at: 1_700_000_000,
                creation_seq: None,
                cwd: "/tmp".to_string(),
                invocation: "agent".to_string(),
                status: SessionStatus::default(),
                annotation: None,
                restart_offer: RestartOffer::default(),
                tabs: Vec::new(),
                source_profile: None,
            },
            terminal,
            outcome: Arc::new(std::sync::Mutex::new(outcome)),
            snapshot: IntegrationSnapshot {
                kind: AgentKind::Generic,
                resume_template: None,
            },
            canonical_cwd: None,
            first_input: Arc::new(std::sync::Mutex::new(FirstInput {
                at: None,
                durable: true,
            })),
            capture: Arc::new(std::sync::Mutex::new(CaptureState::Unclaimed)),
            hooked: hook_flag(false),
            hook_warned: hook_flag(false),
            activity: ActivitySample::unsampled(),
            last_activity_at: crate::service::core::activity_stamp(1_700_000_000),
            generation: 0,
            scope: None,
        }
    }

    /// A title-only replacement SHARES the run's mutable cells with the
    /// entry it replaces, rather than copying their values.
    ///
    /// The property every writer that resolved an entry BEFORE the rename
    /// depends on: an attachment's input path writes the first-input
    /// anchor through the entry its route pinned, a capture pass advances
    /// state it gathered earlier, and a list pass commits an outcome it
    /// observed a moment ago. With copies, each of those lands in an
    /// object nothing reads again — silently, and for capture
    /// permanently, since a window that never opens never closes.
    ///
    /// Asserted by mutating the OLD entry and observing the published one,
    /// which is the direction the bug actually takes: the writers hold the
    /// old entry and the readers use the new.
    #[test]
    fn a_renamed_entry_shares_the_runs_cells_with_the_entry_it_replaces() {
        let old = entry_with(Some(a_terminal()), LastOutcome::Running);
        let renamed = renamed_entry(&old, "new title".to_string());
        assert_eq!(renamed.info.title, "new title");
        assert_eq!(old.info.title, "t", "the replaced entry is left untouched");

        *old.outcome.lock().unwrap() = LastOutcome::Interrupted;
        assert_eq!(
            *renamed.outcome.lock().unwrap(),
            LastOutcome::Interrupted,
            "an outcome recorded through the old entry must be what the published one reports"
        );

        old.first_input.lock().unwrap().at = Some(1_700_000_000);
        assert_eq!(
            renamed.first_input.lock().unwrap().at,
            Some(1_700_000_000),
            "the first-input anchor is written through whichever entry the input path pinned"
        );

        *old.capture.lock().unwrap() = CaptureState::Provisional {
            conversation: "conv-x".to_string(),
        };
        assert!(
            matches!(
                &*renamed.capture.lock().unwrap(),
                CaptureState::Provisional { conversation } if conversation == "conv-x"
            ),
            "capture progress must not be split in two by a rename"
        );

        // The activity sample joins the same rule (PLAN_M6_75.md item 1).
        // A rename describes the SAME run, so a session renamed between
        // two ticks must not lose its baseline and read as freshly
        // discovered — which, once the classifier consumes this, would
        // show up as a status that resets every time somebody edits a
        // title.
        old.activity.lock().unwrap().observe("screen".to_string());
        assert_eq!(
            renamed.activity.lock().unwrap().samples,
            1,
            "a sample taken through the old entry must be what the published one reports"
        );

        // The two hook-diagnostic flags are the newest cells here and the
        // easiest to get wrong, because their sibling rule under RELAUNCH
        // is the opposite one: `relaunched_entry` mints both fresh on every
        // generation. A rename is not a generation, so copying the values
        // instead of sharing the cells would split the tripwire's latch in
        // two — `with_hook_argv` raises `hooked` on the entry it launched,
        // and a rename landing between the launch and the horizon would
        // leave the published entry claiming an unhooked launch (no warning
        // ever, for a hook that really is broken), or leave `hook_warned`
        // unspent so the tripwire re-warns once per tick forever.
        //
        // `Arc::ptr_eq` as well as a value check for these two: they are
        // `AtomicBool`s written with `Relaxed` stores from the pass, so a
        // copied-value implementation could still pass a mutate-and-observe
        // assertion the moment somebody "fixed" it by re-copying, whereas
        // pointer identity is the property the writers actually rely on.
        assert!(
            Arc::ptr_eq(&old.hooked, &renamed.hooked),
            "the hook-injection flag must be the SAME cell across a rename"
        );
        assert!(
            Arc::ptr_eq(&old.hook_warned, &renamed.hook_warned),
            "the tripwire latch must be the SAME cell across a rename"
        );
        old.hooked.store(true, std::sync::atomic::Ordering::Relaxed);
        old.hook_warned
            .store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(
            renamed.hooked.load(std::sync::atomic::Ordering::Relaxed),
            "a launch flagged as hooked through the old entry must stay hooked once renamed"
        );
        assert!(
            renamed
                .hook_warned
                .load(std::sync::atomic::Ordering::Relaxed),
            "a tripwire warning already spent must not be spendable again after a rename"
        );
    }

    /// A RELAUNCH does the opposite, and must keep doing it: the new
    /// generation gets fresh cells, so a pass still holding the previous
    /// entry cannot write its late conclusion onto the launch that
    /// replaced it.
    ///
    /// The mirror image of the test above, and the reason those cells are
    /// `Arc`ed rather than simply shared everywhere: the two replacement
    /// paths need opposite things, and a "simplification" that made both
    /// share would reintroduce exactly the cross-generation contamination
    /// the generation fence exists to prevent. Carried-over VALUES are not
    /// the same as a shared cell — this asserts the value came across and
    /// the identity did not.
    ///
    /// `last_activity_at` is asserted here too, and it is the deliberate
    /// EXCEPTION: that cell is session-scoped, so a relaunch shares the
    /// very `Arc`. Pinned alongside its opposites so the two rules are
    /// read together — a future reader tidying this entry's cells into one
    /// consistent policy would otherwise pick a policy that is wrong for
    /// one of them either way.
    #[test]
    fn a_relaunched_entry_gets_fresh_cells_even_when_it_carries_the_values_over() {
        let old = entry_with(Some(a_terminal()), LastOutcome::Running);
        old.first_input.lock().unwrap().at = Some(1_700_000_000);
        *old.capture.lock().unwrap() = CaptureState::Provisional {
            conversation: "conv-old".to_string(),
        };

        let relaunched = relaunched_entry(
            &old,
            old.info.clone(),
            old.terminal.clone(),
            old.generation + 1,
            None,
            LastOutcome::Launching,
            false,
        );
        assert_eq!(
            relaunched.first_input.lock().unwrap().at,
            Some(1_700_000_000),
            "test premise: a relaunch that keeps its capture window carries the anchor over"
        );

        // The previous run's observers write on: none of it may reach the
        // entry describing the new one.
        *old.outcome.lock().unwrap() = LastOutcome::Interrupted;
        old.first_input.lock().unwrap().at = Some(1_800_000_000);
        *old.capture.lock().unwrap() = CaptureState::UncapturedFinal;
        assert_eq!(*relaunched.outcome.lock().unwrap(), LastOutcome::Launching);
        assert_eq!(
            relaunched.first_input.lock().unwrap().at,
            Some(1_700_000_000)
        );
        assert!(matches!(
            &*relaunched.capture.lock().unwrap(),
            CaptureState::Provisional { conversation } if conversation == "conv-old"
        ));

        // And the exception, by identity rather than by value: a late
        // observation written through the old entry has to reach the
        // published one, because "when was output last seen here" is a
        // fact about the session and not about either run.
        assert!(
            Arc::ptr_eq(&old.last_activity_at, &relaunched.last_activity_at),
            "the activity stamp is session-scoped and must be the SAME cell across a relaunch"
        );
        old.last_activity_at
            .store(1_900_000_000, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(
            relaunched
                .last_activity_at
                .load(std::sync::atomic::Ordering::Relaxed),
            1_900_000_000
        );
    }

    /// The other half of the same rule: a relaunch that OPENED a fresh
    /// capture window discards whatever the previous run concluded, from
    /// every state including [`CaptureState::Reported`].
    ///
    /// `Reported` is the state most tempting to special-case here, because
    /// it is the only one the ladder treats as authoritative rather than
    /// inferred — and special-casing it would be a bug. The authority is
    /// over the RUN that reported it: a non-Resume relaunch starts a
    /// different agent process, and the conversation the old process was in
    /// says nothing about the new one. Carrying it over would leave the
    /// session advertising a resume of a conversation the fresh launch is
    /// not in, and — because nothing but another report may displace
    /// `Reported` — the scan could never correct it either. The durable
    /// half of the same decision is `begin_relaunch` clearing
    /// `conversation_source` along with the captured columns.
    ///
    /// Asserted across every variant rather than just `Reported` so the
    /// rule reads as "the reset is unconditional", which is the property
    /// that has to survive the next variant somebody adds.
    #[test]
    fn a_relaunch_that_reopens_the_capture_window_discards_every_prior_verdict() {
        for prior in [
            CaptureState::Provisional {
                conversation: "conv-prov".to_string(),
            },
            // The retry state, included because it is the one variant that
            // carries an outstanding INTENTION rather than a conclusion,
            // and because nothing downstream would catch it. A pending
            // claim carried into the published entry is retried by the very
            // next pass, and that retry reads the generation off the entry
            // it is holding — the NEW one — so the store's generation fence
            // sees a current write and lets it through. The previous run's
            // conversation would land durably on the fresh launch. This
            // reset is the only thing standing in the way, which is exactly
            // why the variant is asserted here rather than assumed safe.
            CaptureState::PendingCommit {
                conversation: "conv-pending".to_string(),
                record: PathBuf::from("/tmp/pending.jsonl"),
                stamp: RecordStamp {
                    len: 0,
                    mtime_unix: None,
                },
            },
            CaptureState::UncapturedFinal,
            CaptureState::Captured {
                conversation: "conv-scan".to_string(),
                record: PathBuf::from("/tmp/record.jsonl"),
                stamp: RecordStamp {
                    len: 0,
                    mtime_unix: None,
                },
            },
            CaptureState::Ambiguous { durable: true },
            CaptureState::Reported {
                conversation: "conv-reported".to_string(),
            },
        ] {
            let old = entry_with(Some(a_terminal()), LastOutcome::Running);
            old.first_input.lock().unwrap().at = Some(1_700_000_000);
            *old.capture.lock().unwrap() = prior.clone();

            let relaunched = relaunched_entry(
                &old,
                old.info.clone(),
                old.terminal.clone(),
                old.generation + 1,
                None,
                LastOutcome::Launching,
                true,
            );
            assert!(
                matches!(
                    &*relaunched.capture.lock().unwrap(),
                    CaptureState::Unclaimed
                ),
                "a reopened capture window must start from Unclaimed, whatever the previous \
                 run concluded (was {prior:?})"
            );
            assert_eq!(
                relaunched.first_input.lock().unwrap().at,
                None,
                "and the anchor the verdict was derived from goes with it"
            );
        }
    }

    /// A RESUME relaunch keeps a reported identity, which is the whole
    /// point of resuming: the new process is asked to re-enter the very
    /// conversation the previous one named.
    ///
    /// The complement of the test above, and worth its own assertion rather
    /// than being read off the general "values carry over" rule, because
    /// `Reported` is the state a reader is most likely to assume must be
    /// re-earned. Dropping it here would not merely lose a diagnostic: the
    /// entry's capture state is what `session_restart_offer` computes the
    /// Resume offer from, so a session that had just been resumed would
    /// immediately stop offering to resume — and, with no record locator to
    /// re-verify from, the scan could not put it back either.
    #[test]
    fn a_resume_relaunch_keeps_the_identity_the_agent_reported() {
        let old = entry_with(Some(a_terminal()), LastOutcome::Running);
        old.first_input.lock().unwrap().at = Some(1_700_000_000);
        *old.capture.lock().unwrap() = CaptureState::Reported {
            conversation: "conv-reported".to_string(),
        };

        let relaunched = relaunched_entry(
            &old,
            old.info.clone(),
            old.terminal.clone(),
            old.generation + 1,
            None,
            LastOutcome::Launching,
            false,
        );
        assert!(
            matches!(
                &*relaunched.capture.lock().unwrap(),
                CaptureState::Reported { conversation } if conversation == "conv-reported"
            ),
            "a resume must carry the reported identity onto the launch that resumes it"
        );
        assert!(
            !Arc::ptr_eq(&old.capture, &relaunched.capture),
            "carried by VALUE, not by cell: a pass still holding the previous entry must \
             not be able to write its late verdict onto this generation"
        );
    }

    /// The hook tripwire cells are minted fresh on EVERY relaunch, a
    /// Resume included — the one pair here that does not follow the
    /// capture window.
    ///
    /// The distinction is subtle enough to be worth a test of its own,
    /// because the neighbouring rule is so nearly right: a Resume keeps its
    /// capture state, so keeping the "hooked" flag beside it looks
    /// consistent. It is not. `hooked` answers "was THIS launch's argv
    /// injected", and two launches of one session can genuinely differ — a
    /// resume template carrying a `--`, a kind newly excluded, a
    /// `farhelm_exe` that stopped resolving. Carrying `true` across would
    /// arm the tripwire for a launch that never had a hook, producing a
    /// warning about a report that was never going to come.
    ///
    /// `hook_warned` carries the sharper half: a warning already spent
    /// belongs to the launch that earned it, and inheriting it would
    /// silence the wire for the next broken launch — exactly the restart a
    /// user performs when trying to fix the thing the warning is about.
    #[test]
    fn a_relaunch_mints_fresh_hook_cells_even_when_it_keeps_the_capture_window() {
        let ordering = std::sync::atomic::Ordering::Relaxed;
        for reset_capture in [true, false] {
            let old = entry_with(Some(a_terminal()), LastOutcome::Running);
            old.hooked.store(true, ordering);
            old.hook_warned.store(true, ordering);

            let relaunched = relaunched_entry(
                &old,
                old.info.clone(),
                old.terminal.clone(),
                old.generation + 1,
                None,
                LastOutcome::Launching,
                reset_capture,
            );
            assert!(
                !relaunched.hooked.load(ordering),
                "a relaunch this process has not injected yet is not hooked \
                 (reset_capture = {reset_capture})"
            );
            assert!(
                !relaunched.hook_warned.load(ordering),
                "and the previous launch's warning must not silence this one's \
                 (reset_capture = {reset_capture})"
            );
            assert!(
                !Arc::ptr_eq(&old.hooked, &relaunched.hooked),
                "fresh CELLS too, so a late writer cannot reach across the generation"
            );
        }
    }

    /// The activity sample is the one cell a relaunch resets outright
    /// rather than carrying over, whatever the capture window decided.
    ///
    /// Separate from the test above because the RULE is different, not
    /// merely the field: `first_input`/`capture` carry their VALUES across
    /// a relaunch that kept its window, while a sample describes a process
    /// that no longer exists. Inheriting it would let the previous
    /// generation's sampled tail and its unchanged-sample streak contaminate
    /// the replacement — the new pane classified `Idle` because the OLD one
    /// stopped changing, or sharpened to `Waiting` from a dialog the dead
    /// run was showing, on evidence gathered from a process that is gone.
    /// That cross-generation contamination is exactly what the fence exists
    /// to prevent.
    #[test]
    fn a_relaunch_resets_the_activity_sample_rather_than_inheriting_the_dead_runs_screen() {
        let old = entry_with(Some(a_terminal()), LastOutcome::Running);
        old.activity
            .lock()
            .unwrap()
            .observe("previous run's screen".to_string());

        let relaunched = relaunched_entry(
            &old,
            old.info.clone(),
            old.terminal.clone(),
            old.generation + 1,
            None,
            LastOutcome::Launching,
            // `false` deliberately: even a relaunch that KEEPS its capture
            // window — the case that carries the anchor over — must still
            // start from an unsampled screen.
            false,
        );
        {
            let fresh = relaunched.activity.lock().unwrap();
            assert_eq!(
                fresh.samples, 0,
                "the new generation has been seen by nobody"
            );
            assert_eq!(fresh.tail, None);
        }

        old.activity
            .lock()
            .unwrap()
            .observe("a late sample of the dead run".to_string());
        assert_eq!(
            relaunched.activity.lock().unwrap().samples,
            0,
            "a sampler still holding the previous entry must not reach the launch that \
             replaced it"
        );
    }

    /// The one terminal the entry-replacement tests above and
    /// `service::status`'s classification tests use.
    pub(crate) fn a_terminal() -> Terminal {
        Terminal {
            tmux_name: "fh-1".to_string(),
            pane: "%0".to_string(),
        }
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
        let state = StateDir::new();
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
                .create_session(name, "/tmp", 80, 24, &[], &argv)
                .await
                .expect("create a tmux session directly");
        }
        // Deliberately unequal to `created_at`, and deliberately in the
        // past: it is what the restore assertion below distinguishes from
        // both a re-mint and a reset.
        let stored_activity_at = now_unix() - 3_600;
        for (id, tmux_name) in [("live", "fh-live"), ("dead", "fh-dead")] {
            sup.store
                .insert_session(
                    StoredSession {
                        conversation_source: None,
                        id: id.to_string(),
                        parent: None,
                        archived: false,
                        title: id.to_string(),
                        created_at: now_unix(),
                        last_activity_at: stored_activity_at,
                        creation_seq: 0,
                        cwd: "/tmp".to_string(),
                        invocation: "agent".to_string(),
                        tmux_name: tmux_name.to_string(),
                        pane: String::new(),
                        outcome: LastOutcome::Launching,
                        agent_kind: farhelm_proto::AgentKind::Generic,
                        resume_template: None,
                        canonical_cwd: None,
                        captured_conversation: None,
                        captured_record: None,
                        capture_ambiguous: false,
                        first_input_at: None,
                        generation: 0,
                        launch_scoped: false,
                        source_profile: None,
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
        // Activity samples are process-local (PLAN_M6_75.md item 1): a
        // reload adopts sessions this process has never watched, so every
        // reloaded entry must start unsampled rather than claiming a
        // recency for a stretch of time nobody was looking at. Asserted on
        // the real reload path because that is the only place the rule can
        // be got wrong.
        //
        // The activity TIMESTAMP is the deliberate counterexample beside
        // it, asserted here so the two rules are read together rather than
        // one being "fixed" into the other later: that value is a fact
        // about a past instant, which a supervisor being down does not
        // falsify, so a reload restores it verbatim. Re-minting it would
        // claim this process saw something it did not; dropping it to zero
        // would tell a most-recently-active sort that a long-lived session
        // has never done anything.
        for (id, entry) in &sessions {
            let sample = entry.activity.lock().unwrap();
            assert_eq!(
                sample.samples, 0,
                "session {id} came back from the store, so nothing has observed its pane yet"
            );
            assert_eq!(sample.unchanged_streak, 0);
            assert_eq!(
                entry
                    .last_activity_at
                    .load(std::sync::atomic::Ordering::Relaxed),
                stored_activity_at,
                "session {id} must come back with the activity time the row recorded"
            );
            assert_eq!(
                entry.info.last_activity_at, stored_activity_at,
                "and the entry's own wire snapshot must agree with its cell"
            );
        }
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

    /// PLAN_M3.md item 10's other reload contract, and the one host-
    /// independent tests never reached before this: `reload_sessions`
    /// derives each entry's in-memory `scope` from the STORED
    /// `launch_scoped` column (`launch_scope_unit`, called at the row-to-
    /// entry conversion), not from a fresh systemd probe. A row that
    /// recorded a scoped launch must therefore come back out of reload
    /// still naming that generation's unit — with no systemd user manager
    /// involved anywhere in this test, since the derivation is a pure
    /// function of the row and the scope manager is never consulted at
    /// reload time — only later, when a stop, delete, or restart reaps the
    /// prior run through its scope (and a restart additionally re-probes
    /// availability for the launch it is about to make).
    ///
    /// Driven against a real tmux server, like the sibling reload tests
    /// above, because `reload_sessions` is one pass and this is a property
    /// of that pass rather than of `launch_scope_unit` in isolation
    /// (already a one-line pure function with nothing else worth pinning
    /// on its own).
    #[tokio::test]
    async fn reload_carries_forward_the_scope_a_stored_launch_recorded() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");

        let scoped_id = uuid::Uuid::new_v4().to_string();
        sup.tmux
            .create_session(
                "fh-scoped",
                "/tmp",
                80,
                24,
                &[],
                &["sh".to_string(), "-c".to_string(), "sleep 300".to_string()],
            )
            .await
            .expect("create a tmux session directly");
        sup.store
            .insert_session(
                StoredSession {
                    conversation_source: None,
                    id: scoped_id.clone(),
                    parent: None,
                    archived: false,
                    title: "scoped".to_string(),
                    created_at: now_unix(),
                    last_activity_at: now_unix(),
                    creation_seq: 0,
                    cwd: "/tmp".to_string(),
                    invocation: "agent".to_string(),
                    tmux_name: "fh-scoped".to_string(),
                    pane: String::new(),
                    outcome: LastOutcome::Launching,
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    resume_template: None,
                    canonical_cwd: None,
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: true,
                    source_profile: None,
                },
                None,
            )
            .await
            .expect("insert a scoped launching row");

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
            sessions[&scoped_id].scope,
            crate::scope::unit_name(&scoped_id, 0),
            "reload must re-derive the unit from the stored generation and \
             launch_scoped flag rather than leaving the entry unscoped"
        );
    }

    /// Reload reads `conversation_source` as the discriminator between the
    /// two writers that share the `captured_conversation` column, so an
    /// identity the AGENT reported comes back as
    /// [`CaptureState::Reported`] and one the SCAN concluded comes back as
    /// `Captured`.
    ///
    /// The distinction survives a restart or it is not a distinction. A
    /// reported session must not be re-scanned, must not be re-verified
    /// against a record file, and must keep dominating any later ambiguity
    /// the scan would otherwise reach for — all of which follow from the
    /// state it reloads into and none of which follow from the id alone.
    /// Reloading a reported row as `Captured` would put the session back
    /// under scan authority and reopen exactly the `/clear` hole the report
    /// exists to close.
    ///
    /// Both rows are asserted in one pass because the risk is a mapping
    /// that ignores the new column entirely: a test that only planted a
    /// hook row would still pass against code that returned `Reported` for
    /// every captured id.
    ///
    /// Driven through the real `reload_sessions`, like its siblings above,
    /// because the row-to-state mapping only exists inside that pass.
    #[tokio::test]
    async fn reload_distinguishes_a_reported_identity_from_a_scanned_one() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");

        let reported_id = uuid::Uuid::new_v4().to_string();
        let scanned_id = uuid::Uuid::new_v4().to_string();
        for (id, source) in [
            (&reported_id, Some("hook".to_string())),
            (&scanned_id, None),
        ] {
            sup.store
                .insert_session(
                    StoredSession {
                        conversation_source: source,
                        id: id.clone(),
                        parent: None,
                        archived: false,
                        title: "identity".to_string(),
                        created_at: now_unix(),
                        last_activity_at: now_unix(),
                        creation_seq: 0,
                        cwd: "/tmp".to_string(),
                        invocation: "claude".to_string(),
                        tmux_name: format!("fh-{id}"),
                        pane: String::new(),
                        outcome: LastOutcome::Exited {
                            exit_code: Some(0),
                            annotation: None,
                        },
                        agent_kind: farhelm_proto::AgentKind::Claude,
                        // Reload refuses an integrated kind whose template
                        // carries no `{conversation}` element, so the row
                        // has to be one a create would actually have
                        // accepted.
                        resume_template: Some(vec![
                            "claude".to_string(),
                            "--resume".to_string(),
                            crate::agent_kind::CONVERSATION_PLACEHOLDER.to_string(),
                        ]),
                        canonical_cwd: Some("/tmp".to_string()),
                        captured_conversation: Some(format!("conv-{id}")),
                        captured_record: None,
                        capture_ambiguous: false,
                        first_input_at: Some(now_unix()),
                        generation: 0,
                        launch_scoped: false,
                        source_profile: None,
                    },
                    None,
                )
                .await
                .expect("insert a row carrying a captured identity");
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

        let reported = sessions[&reported_id].capture.lock().unwrap().clone();
        assert!(
            matches!(
                &reported,
                CaptureState::Reported { conversation } if *conversation == format!("conv-{reported_id}")
            ),
            "a row whose identity came from the agent's own hook must reload as Reported, \
             not back under the scan's authority: {reported:?}"
        );
        let scanned = sessions[&scanned_id].capture.lock().unwrap().clone();
        assert!(
            matches!(
                &scanned,
                CaptureState::Captured { conversation, .. }
                    if *conversation == format!("conv-{scanned_id}")
            ),
            "a row with no conversation source is still the scan's claim: {scanned:?}"
        );

        // Both are committed identities whatever produced them, so both
        // owe the user a Resume offer after the restart.
        assert_eq!(
            reported.committed_conversation(),
            Some(format!("conv-{reported_id}").as_str())
        );
        assert_eq!(
            scanned.committed_conversation(),
            Some(format!("conv-{scanned_id}").as_str())
        );
    }

    /// Deleting a session removes its conversation-hook trace, at the path
    /// [`hook_log_path`] names — and removes that one only.
    ///
    /// The trace is per-session and unbounded in lifetime: one line per
    /// hook run, appended for as long as the session exists. Nothing else
    /// ever removes it — the hook only appends, and the startup
    /// reconciliation that sweeps the attachments tree knows nothing about
    /// this directory — so if delete misses it, every session farhelm has
    /// ever run leaves a file behind forever, each naming a conversation
    /// id of a session the user believes is gone.
    ///
    /// Scope, stated so nobody reads a guarantee that is not here: the
    /// fixture plants its file through the DELETER's own helper, so this
    /// says nothing whatever about the writer. That writer lives in
    /// another crate and derives its path independently, from the socket's
    /// parent directory; whether the two agree on a file is a cross-crate
    /// contract this test cannot observe even in principle, and the e2e
    /// suite is what holds it. Using the deleter's helper is deliberate
    /// anyway — a hand-spelled path here would make this a test of the
    /// layout rather than of the removal, and it would still not reach the
    /// writer.
    ///
    /// A neighbouring file in the same directory is planted to pin the
    /// scope: delete removes ONE session's trace, not the directory, and
    /// certainly not a sibling session's history.
    #[tokio::test]
    async fn deleting_a_session_removes_its_conversation_hook_trace() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let doomed = "hooked-session";
        let bystander = "other-session";
        sup.store
            .insert_session(
                StoredSession {
                    conversation_source: None,
                    id: doomed.to_string(),
                    parent: None,
                    archived: false,
                    title: "hooked".to_string(),
                    created_at: now_unix(),
                    last_activity_at: now_unix(),
                    creation_seq: 0,
                    cwd: "/tmp".to_string(),
                    invocation: "claude".to_string(),
                    tmux_name: format!("fh-{doomed}"),
                    pane: String::new(),
                    outcome: LastOutcome::Exited {
                        exit_code: Some(0),
                        annotation: None,
                    },
                    agent_kind: farhelm_proto::AgentKind::Claude,
                    resume_template: None,
                    canonical_cwd: Some("/tmp".to_string()),
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                    source_profile: None,
                },
                None,
            )
            .await
            .expect("seed the session being deleted");

        let trace = hook_log_path(state.path(), doomed);
        let survivor = hook_log_path(state.path(), bystander);
        std::fs::create_dir_all(trace.parent().expect("the trace has a parent directory"))
            .expect("hook-log directory");
        std::fs::write(&trace, "1700000000 acked\n").expect("plant the doomed trace");
        std::fs::write(&survivor, "1700000000 acked\n").expect("plant the bystander's trace");

        // No terminal, so the teardown skips tmux entirely: what is under
        // test is the file cleanup that runs after the row is gone, and a
        // real pane would only add a tmux server to the test.
        let mut entry = entry_with(None, LastOutcome::Running);
        entry.info.id = doomed.to_string();
        // `TeardownError` carries no `Debug`, so the assertion says what
        // was expected rather than unwrapping.
        assert!(
            sup.teardown_session(&entry, doomed).await.is_ok(),
            "a terminal-less session tears down without tmux"
        );

        assert!(
            !trace.exists(),
            "the deleted session's hook trace must be gone: {}",
            trace.display()
        );
        assert!(
            survivor.exists(),
            "another session's trace must be untouched: {}",
            survivor.display()
        );
    }

    /// A session that never had a hook trace still deletes cleanly: the
    /// removal is reached, finds nothing, and the delete reports success
    /// with the row gone.
    ///
    /// This is the ordinary case by a wide margin — nothing writes a trace
    /// unless the launch was hooked. Note what a mishandled `NotFound`
    /// would and would not cost, because the earlier framing of this test
    /// overstated it: the removal runs AFTER the row is already deleted and
    /// is log-only, so no handling of it could make a delete fail. What the
    /// explicit `NotFound` arm buys is warning suppression — without it
    /// every unhooked delete would emit "could not remove a deleted
    /// session's conversation-hook trace", the kind of routine false alarm
    /// that trains an operator to stop reading this subsystem's warnings,
    /// including the ones that mean something. That log line is not
    /// observable from here without a tracing fixture; what this test
    /// holds is the surrounding claim, that the missing file is reached on
    /// the success path rather than short-circuited before it.
    #[tokio::test]
    async fn deleting_a_session_with_no_hook_trace_is_not_an_error() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let id = "unhooked-session";
        sup.store
            .insert_session(
                StoredSession {
                    conversation_source: None,
                    id: id.to_string(),
                    parent: None,
                    archived: false,
                    title: "unhooked".to_string(),
                    created_at: now_unix(),
                    last_activity_at: now_unix(),
                    creation_seq: 0,
                    cwd: "/tmp".to_string(),
                    invocation: "agent".to_string(),
                    tmux_name: format!("fh-{id}"),
                    pane: String::new(),
                    outcome: LastOutcome::Exited {
                        exit_code: Some(0),
                        annotation: None,
                    },
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    resume_template: None,
                    canonical_cwd: None,
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                    source_profile: None,
                },
                None,
            )
            .await
            .expect("seed the session being deleted");

        let mut entry = entry_with(None, LastOutcome::Running);
        entry.info.id = id.to_string();
        assert!(
            sup.teardown_session(&entry, id).await.is_ok(),
            "a missing hook trace is the ordinary case, not a failure"
        );
        assert!(sup.store.session(id).await.unwrap().is_none());
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
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");

        let mut panes = HashMap::new();
        for (id, command) in [("landed", "exit 0"), ("never-landed", "sleep 300")] {
            let tmux_name = format!("fh-{id}");
            let argv = ["sh".to_string(), "-c".to_string(), command.to_string()];
            let pane = sup
                .tmux
                .create_session(&tmux_name, "/tmp", 80, 24, &[], &argv)
                .await
                .expect("create a tmux session directly");
            sup.store
                .insert_session(
                    StoredSession {
                        conversation_source: None,
                        id: id.to_string(),
                        parent: None,
                        archived: false,
                        title: id.to_string(),
                        created_at: now_unix(),
                        last_activity_at: now_unix(),
                        creation_seq: 0,
                        cwd: "/tmp".to_string(),
                        invocation: "agent".to_string(),
                        tmux_name,
                        pane: pane.clone(),
                        outcome: LastOutcome::Running,
                        agent_kind: farhelm_proto::AgentKind::Generic,
                        resume_template: None,
                        canonical_cwd: None,
                        captured_conversation: None,
                        captured_record: None,
                        capture_ambiguous: false,
                        first_input_at: None,
                        generation: 0,
                        launch_scoped: false,
                        source_profile: None,
                    },
                    None,
                )
                .await
                .expect("insert");
            // Through the real transition, so the fixture is the state a
            // real interrupted stop leaves behind rather than a hand-made
            // approximation of it.
            sup.store
                .transition(id, 0, Transition::StopRequested)
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
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        sup.store
            .insert_session(
                StoredSession {
                    conversation_source: None,
                    id: "s1".to_string(),
                    parent: None,
                    archived: false,
                    title: "t".to_string(),
                    created_at: now_unix(),
                    last_activity_at: now_unix(),
                    creation_seq: 0,
                    cwd: "/tmp".to_string(),
                    invocation: "agent".to_string(),
                    tmux_name: "fh-1".to_string(),
                    pane: "%0".to_string(),
                    outcome: LastOutcome::Running,
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    resume_template: None,
                    canonical_cwd: None,
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                    source_profile: None,
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
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
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
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        sup.store
            .insert_session(
                StoredSession {
                    conversation_source: None,
                    id: "s1".to_string(),
                    parent: None,
                    archived: false,
                    title: "t".to_string(),
                    created_at: now_unix(),
                    last_activity_at: now_unix(),
                    creation_seq: 0,
                    cwd: "/tmp".to_string(),
                    invocation: "agent".to_string(),
                    tmux_name: "fh-1".to_string(),
                    pane: "%0".to_string(),
                    outcome: LastOutcome::Running,
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    resume_template: None,
                    canonical_cwd: None,
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                    source_profile: None,
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
        let state = StateDir::new();
        let db_path = state.path().join("supervisor.db");
        let store = SessionStore::open(&db_path, true).await.expect("store");
        store
            .insert_session(
                StoredSession {
                    conversation_source: None,
                    id: "s1".to_string(),
                    parent: None,
                    archived: false,
                    title: "t".to_string(),
                    created_at: now_unix(),
                    last_activity_at: now_unix(),
                    creation_seq: 0,
                    cwd: "/tmp".to_string(),
                    invocation: "agent".to_string(),
                    tmux_name: "fh-does-not-exist".to_string(),
                    pane: "%0".to_string(),
                    outcome: LastOutcome::Running,
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    resume_template: None,
                    canonical_cwd: None,
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                    source_profile: None,
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
        // Theme D of the M6.75 review-swarm batch: a claimless construction
        // must perform NO durable write, and minting a host identity is one
        // — exactly like the reconciliation checked below, just quieter,
        // since nothing here observes tmux or sessions to notice it. Before
        // the fix, `ensure_host_identity` ran unconditionally regardless of
        // `ownership`, so a losing racer against a genuinely fresh install
        // would durably mint an identity it had no standing to write.
        assert_eq!(
            candidate.host_identity, None,
            "a claimless construction must not mint an identity in its own in-memory copy either"
        );

        let store = SessionStore::open(&db_path, true).await.expect("store");
        assert_eq!(
            store
                .read_host_identity()
                .await
                .expect("reading back host identity"),
            None,
            "a claimless construction must leave supervisor_meta.host_identity NULL, \
             not mint one behind the incumbent's back"
        );
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
        let state = StateDir::new();
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
        let state = StateDir::new();
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
                parent: None,
                profile_name: None,
                cwd: "/".to_string(),
                invocation: Some("agent".to_string()),
                profile_id: None,
                title: None,
                cols: 80,
                rows: 24,
                intent_key: None,
                agent_kind: None,
                resume_template: None,
            },
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
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
    ///
    /// The MODE cases (PLAN_M6_75.md item 3) are the newest ones with
    /// teeth, and the reason they are here rather than only in a handler
    /// test: the fingerprint is the ONLY thing standing between a retried
    /// intent key and a create that launches something other than what the
    /// first attempt did. A retry that flips raw-to-profile, or names a
    /// different profile, must land on a different fingerprint and be
    /// refused as a key reuse.
    #[test]
    fn the_create_fingerprint_covers_every_session_shaping_field() {
        let base = raw_fingerprint("/work", "agent --flag", Some("t"), None, None);
        let cases = [
            (
                raw_fingerprint("/other", "agent --flag", Some("t"), None, None),
                "cwd",
            ),
            (
                raw_fingerprint("/work", "agent --other", Some("t"), None, None),
                "invocation",
            ),
            (
                raw_fingerprint("/work", "agent --flag", Some("other"), None, None),
                "title",
            ),
            (
                raw_fingerprint("/work", "agent --flag", None, None, None),
                "an omitted title",
            ),
            (
                raw_fingerprint(
                    "/work",
                    "agent --flag",
                    Some("t"),
                    Some(AgentKind::Claude),
                    None,
                ),
                "the agent-kind override",
            ),
            (
                raw_fingerprint(
                    "/work",
                    "agent --flag",
                    Some("t"),
                    None,
                    Some(&["claude", "{conversation}"]),
                ),
                "the resume-template override",
            ),
            (
                create_fingerprint(
                    None,
                    "/work",
                    &CreateMode::Profile {
                        profile_id: "prof-1".to_string(),
                    },
                    Some("t"),
                ),
                "the create MODE",
            ),
            (
                create_fingerprint(
                    Some("parent-1"),
                    "/work",
                    &CreateMode::Raw {
                        invocation: "agent --flag".to_string(),
                        agent_kind: None,
                        resume_template: None,
                    },
                    Some("t"),
                ),
                "the parent",
            ),
            (
                create_fingerprint(
                    None,
                    "/work",
                    &CreateMode::ProfileName {
                        profile_name: "Claude Code".to_string(),
                    },
                    Some("t"),
                ),
                "the profile-name selector",
            ),
        ];
        for (fingerprint, what) in cases {
            assert_ne!(fingerprint, base, "{what} must change the fingerprint");
        }
        assert_eq!(
            raw_fingerprint("/work", "agent --flag", Some("t"), None, None),
            base,
            "the same request must fingerprint identically every time"
        );
        // Adjacent fields cannot bleed into one another: a delimiter-joined
        // encoding would make these two requests indistinguishable.
        assert_ne!(
            raw_fingerprint("/a", "bc", None, None, None),
            raw_fingerprint("/ab", "c", None, None, None),
        );
        // Distinct override VALUES are distinguished, not merely the
        // presence of an override: two integrated kinds are two different
        // requests, and so are two templates of the same length.
        assert_ne!(
            raw_fingerprint("/work", "a", None, Some(AgentKind::Claude), None),
            raw_fingerprint("/work", "a", None, Some(AgentKind::Codex), None),
        );
        assert_ne!(
            raw_fingerprint("/work", "a", None, None, Some(&["x"])),
            raw_fingerprint("/work", "a", None, None, Some(&["y"])),
        );
        // Two profile-mode creates that differ only in WHICH profile are
        // two different requests: same key, different profile, refused —
        // never a replay of whichever one happened to run first.
        assert_ne!(
            profile_fingerprint("/work", "prof-1", None),
            profile_fingerprint("/work", "prof-2", None),
        );
        assert_ne!(
            create_fingerprint(
                Some("parent-1"),
                "/work",
                &CreateMode::Raw {
                    invocation: "agent".to_string(),
                    agent_kind: None,
                    resume_template: None,
                },
                None,
            ),
            create_fingerprint(
                Some("parent-2"),
                "/work",
                &CreateMode::Raw {
                    invocation: "agent".to_string(),
                    agent_kind: None,
                    resume_template: None,
                },
                None,
            ),
            "same key with a different parent must conflict"
        );
        assert_ne!(
            create_fingerprint(
                Some("parent-1"),
                "/work",
                &CreateMode::Profile {
                    profile_id: "prof-1".to_string(),
                },
                None,
            ),
            create_fingerprint(
                Some("parent-2"),
                "/work",
                &CreateMode::Profile {
                    profile_id: "prof-1".to_string(),
                },
                None,
            ),
            "a profile-id create's parent must change its fingerprint"
        );
        assert_ne!(
            create_fingerprint(
                Some("parent-1"),
                "/work",
                &CreateMode::ProfileName {
                    profile_name: "Claude Code".to_string(),
                },
                None,
            ),
            create_fingerprint(
                Some("parent-2"),
                "/work",
                &CreateMode::ProfileName {
                    profile_name: "Claude Code".to_string(),
                },
                None,
            ),
            "a profile-name create's parent must change its fingerprint"
        );
    }

    /// [`create_fingerprint`] of a RAW-mode request, spelled as the fields
    /// a caller actually sends.
    ///
    /// The mode enum makes an unrepresentable-state bug impossible but a
    /// literal verbose, and these tests are about the ENCODING rather than
    /// about constructing modes; the two helpers keep each case one line so
    /// the field being varied is the visible thing.
    fn raw_fingerprint(
        cwd: &str,
        invocation: &str,
        title: Option<&str>,
        agent_kind: Option<AgentKind>,
        resume_template: Option<&[&str]>,
    ) -> String {
        create_fingerprint(
            None,
            cwd,
            &CreateMode::Raw {
                invocation: invocation.to_string(),
                agent_kind,
                resume_template: resume_template
                    .map(|template| template.iter().map(ToString::to_string).collect()),
            },
            title,
        )
    }

    /// [`create_fingerprint`] of a PROFILE-mode request. See
    /// [`raw_fingerprint`].
    fn profile_fingerprint(cwd: &str, profile_id: &str, title: Option<&str>) -> String {
        create_fingerprint(
            None,
            cwd,
            &CreateMode::Profile {
                profile_id: profile_id.to_string(),
            },
            title,
        )
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
    ///
    /// The RAW strings below are the ones pre-M6.75 supervisors already
    /// wrote, and they must never change again (PLAN_M6_75.md item 3, and
    /// `create_fingerprint`'s own "frozen" section): a reservation is a
    /// permanent tombstone, so an encoding change turns every key a
    /// supervisor has ever seen into a `Conflict` on its next identical
    /// retry — permanently, for that key. Version 10 therefore gave the
    /// PROFILE mode a separate encoding rather than extending this one.
    #[test]
    fn the_persisted_fingerprint_encoding_is_pinned() {
        assert_eq!(
            raw_fingerprint(
                "/work",
                "claude --flag",
                Some("title"),
                Some(AgentKind::Claude),
                Some(&["claude", "{conversation}"]),
            ),
            r#"["/work","claude --flag","title","claude",["claude","{conversation}"]]"#
        );
        assert_eq!(
            raw_fingerprint("/work", "agent", None, None, None),
            r#"["/work","agent",null,null,null]"#
        );
        // The profile mode's own encoding: discriminated and SHORTER, so it
        // is distinguishable from a raw fingerprint by shape alone — no
        // `cwd`, title or profile id can make the two collide.
        assert_eq!(
            profile_fingerprint("/work", "prof-7", Some("title")),
            r#"["profile","/work","title","prof-7"]"#
        );
        assert_eq!(
            create_fingerprint(
                Some("parent-1"),
                "/work",
                &CreateMode::Raw {
                    invocation: "agent".to_string(),
                    agent_kind: None,
                    resume_template: None,
                },
                None,
            ),
            r#"["parented_raw","parent-1","/work","agent",null,null,null]"#
        );
        assert_eq!(
            create_fingerprint(
                None,
                "/work",
                &CreateMode::ProfileName {
                    profile_name: "Claude Code".to_string(),
                },
                None,
            ),
            r#"["profile_name",null,"/work",null,"Claude Code"]"#
        );
    }

    /// The upgrade property the frozen encoding exists for, stated as the
    /// only thing that actually matters: a fingerprint a v9 supervisor
    /// wrote must still be produced, byte for byte, by the same request
    /// today.
    ///
    /// Written against a HARD-CODED legacy string rather than against
    /// `create_fingerprint`'s current output on both sides, because the
    /// latter would pass under any encoding change at all — including one
    /// that broke every install in the field. This literal is the fixture;
    /// its counterpart in the field is a row in somebody's SQLite file that
    /// nothing will ever rewrite.
    #[test]
    fn a_v9_fingerprint_is_reproduced_exactly_by_the_same_request_today() {
        // Exactly what `create_fingerprint(cwd, invocation, title,
        // agent_kind, resume_template)` produced when those were five
        // separate parameters and the profile mode did not exist.
        const V9_RAW: &str =
            r#"["/work","claude --flag","title","claude",["claude","{conversation}"]]"#;
        assert_eq!(
            raw_fingerprint(
                "/work",
                "claude --flag",
                Some("title"),
                Some(AgentKind::Claude),
                Some(&["claude", "{conversation}"]),
            ),
            V9_RAW,
            "a raw create whose key was claimed before the upgrade must still match its own \
             tombstone, or every such key conflicts forever"
        );
    }

    /// The fingerprint a v9 supervisor stored for
    /// `create_session_without_overrides("/", "agent", None, ..)` — the
    /// simplest keyed create this module's tests make, spelled as the bytes
    /// that are actually sitting in upgraded installs' reservation tables.
    ///
    /// A literal rather than a call to `create_fingerprint`: the point of
    /// the two tests below is that a v10 binary agrees with a string it did
    /// not produce, and computing both sides would prove only that the
    /// function agrees with itself.
    const V9_STORED_FINGERPRINT: &str = r#"["/","agent",null,null,null]"#;

    /// A SETTLED reservation written before the upgrade still replays.
    ///
    /// This is the failure mode that would have been permanent: a
    /// reservation is a tombstone nothing prunes, so a client retrying an
    /// identical create with a key it claimed on the old binary would be
    /// told `Conflict` — "this key already means something else" — for as
    /// long as that database exists, with the session it created sitting
    /// right there unreachable through its own intent key.
    ///
    /// The first create plants the legacy fingerprint the way a v9
    /// supervisor did (the claim is caller-supplied, so the test can write
    /// the exact bytes); the retry computes its own the way this build
    /// does. Replaying to the SAME session id is the whole assertion.
    ///
    /// The activity stamp is advanced between the two creates and checked
    /// on the reply, because a replay is the one create path that answers
    /// entirely from the DURABLE record rather than from what it just did.
    /// The session it describes may have been producing output for hours
    /// since the original create — a retry after a network partition can
    /// arrive arbitrarily late — so a replay that reported creation time,
    /// or re-derived the field from `created_at`, would hand a client a
    /// session that has visibly moved backwards in its own list. Equal
    /// values on the two sides would let that pass unnoticed.
    #[tokio::test]
    async fn a_settled_v9_reservation_replays_instead_of_conflicting() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");

        let original = sup
            .create_session_without_overrides(
                "/",
                "agent",
                None,
                80,
                24,
                Some(IntentClaim {
                    intent_key: "v9-key".to_string(),
                    fingerprint: V9_STORED_FINGERPRINT.to_string(),
                    dedup_scope: DedupScope::Permanent,
                }),
            )
            .await
            .expect("the pre-upgrade create");
        assert_eq!(
            original.last_activity_at, original.created_at,
            "a session nothing has watched yet reports its creation time"
        );

        // Ten minutes of observed output, the way the ticker would have
        // recorded it while the client was away.
        let observed_at = original.created_at + 600;
        sup.store
            .record_activity(&original.id, observed_at)
            .await
            .expect("advance the durable stamp");

        let replayed = sup
            .create_session_without_overrides(
                "/",
                "agent",
                None,
                80,
                24,
                Some(IntentClaim {
                    intent_key: "v9-key".to_string(),
                    fingerprint: raw_fingerprint("/", "agent", None, None, None),
                    dedup_scope: DedupScope::Permanent,
                }),
            )
            .await
            .expect("an identical retry across the upgrade must replay, not conflict");
        assert_eq!(
            replayed.id, original.id,
            "the retry must answer with the session the key already made"
        );
        assert_eq!(
            replayed.last_activity_at, observed_at,
            "a replay answers from the record as it stands, not from the create it is \
             replaying"
        );
        assert_eq!(
            sup.store.load_all().await.expect("load").len(),
            1,
            "and must not have launched a second agent for the same intent"
        );
    }

    /// The other half of the same upgrade: a PENDING reservation — a create
    /// the old binary claimed but never settled (a crash, a kill) — must
    /// still be recognized as this request's own, so the retry reconciles
    /// it under the reserved identity instead of being refused as a reuse.
    ///
    /// Worth pinning separately from the settled case because the two take
    /// different paths through `resolve_reservation`, and the fingerprint
    /// check happens BEFORE either — a mismatch would short-circuit both
    /// with `Conflict` and leave the reserved identity stranded forever,
    /// which is worse than the settled case: there is not even a session to
    /// point at.
    #[tokio::test]
    async fn a_pending_v9_reservation_is_reconciled_instead_of_conflicting() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        sup.store
            .insert_session(
                StoredSession {
                    conversation_source: None,
                    id: "stranded".to_string(),
                    parent: None,
                    archived: false,
                    title: "stranded".to_string(),
                    created_at: now_unix(),
                    last_activity_at: now_unix(),
                    creation_seq: 0,
                    cwd: "/".to_string(),
                    invocation: "agent".to_string(),
                    tmux_name: "fh-stranded".to_string(),
                    pane: String::new(),
                    outcome: LastOutcome::Launching,
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    resume_template: None,
                    canonical_cwd: None,
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                    source_profile: None,
                },
                Some(IntentClaim {
                    intent_key: "v9-key".to_string(),
                    fingerprint: V9_STORED_FINGERPRINT.to_string(),
                    dedup_scope: DedupScope::Permanent,
                }),
            )
            .await
            .expect("plant a pre-upgrade pending claim");

        let session = sup
            .create_session_without_overrides(
                "/",
                "agent",
                None,
                80,
                24,
                Some(IntentClaim {
                    intent_key: "v9-key".to_string(),
                    fingerprint: raw_fingerprint("/", "agent", None, None, None),
                    dedup_scope: DedupScope::Permanent,
                }),
            )
            .await
            .expect("the retry must reconcile the pre-upgrade claim, not conflict with it");
        assert_eq!(
            session.id, "stranded",
            "the reserved identity is what makes this a reconciliation rather than a new create"
        );
    }

    /// SPEC.md owes the user an explanation whenever it offers the
    /// fallback instead of a resume, so the two ambiguity diagnostics are
    /// part of the contract rather than debug output. This pins the
    /// overlapping-windows one; its sibling — two records inside one
    /// window — is `agent_kind::choose`'s own payload and is pinned there.
    ///
    /// Asserted on the message-BUILDING function rather than by capturing
    /// the `tracing` event, deliberately and with a cost: capturing events
    /// needs a subscriber, and this crate carries no subscriber
    /// dependency, so a capture harness would mean hand-rolling one in
    /// test code. Extracting the message into a named function instead is
    /// what makes it assertable at all — the emission itself is exercised
    /// by the e2e ambiguity tests, which prove the behavior the message
    /// describes.
    #[test]
    fn the_overlap_diagnostic_explains_the_refusal_it_accompanies() {
        let reason = overlapping_windows_reason("sess-a", "sess-b", "/work/repo");
        for needle in [
            "sess-a",
            "sess-b",
            "/work/repo",
            // Not just the ids: a log line that named two sessions without
            // saying what follows from the collision would leave the user
            // no better off than the silent fallback.
            "conversation identity captured for this launch",
        ] {
            assert!(reason.contains(needle), "{needle:?} missing from: {reason}");
        }
    }

    /// A brand-new session's activity stamp equals its creation time in
    /// all FOUR places the value lives, before anything has watched it.
    ///
    /// The seed is the field's contract at rest, and it is written down
    /// four separate times by four separate pieces of code: the row the
    /// create commits, the `SessionInfo` the reply carries, the entry's own
    /// frozen `info`, and the atomic cell the ticker later advances. Any
    /// one of them starting at 0 instead would be invisible until a
    /// most-recently-active sort existed to read it, and would then park
    /// that session at the epoch — indistinguishable from a real
    /// timestamp, with nothing to correct it until the session next
    /// produced output. Asserting them together is what makes "they agree"
    /// a property rather than four coincidences.
    ///
    /// No ticker runs here (`serve` is what starts one), so nothing can
    /// have moved the value: what this observes is purely what creation
    /// planted.
    #[tokio::test]
    async fn a_created_session_dates_its_activity_to_creation_everywhere() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let created = sup
            .create_session_without_overrides("/", "agent", None, 80, 24, None)
            .await
            .expect("create");

        assert_eq!(
            created.last_activity_at, created.created_at,
            "the reply the client actually reads"
        );
        assert_eq!(
            sup.store
                .session(&created.id)
                .await
                .expect("read the row")
                .expect("a create commits a row")
                .last_activity_at,
            created.created_at,
            "the durable row a later supervisor reloads from"
        );
        let entry = sup
            .sessions
            .lock()
            .await
            .get(&created.id)
            .cloned()
            .expect("the created session is on the map");
        assert_eq!(
            entry.info.last_activity_at, created.created_at,
            "the entry's frozen wire snapshot"
        );
        assert_eq!(
            entry
                .last_activity_at
                .load(std::sync::atomic::Ordering::Relaxed),
            created.created_at,
            "and the live cell every later reply is answered from"
        );
    }

    /// The snapshot is item 7's IMMUTABLE record, and immutability is only
    /// worth anything if the value that lands is the resolved one. This
    /// drives a real create through the store and asserts what came back
    /// out of it: derivation from the first token, the default template
    /// built from that same token, and the honest restart offer that
    /// follows from having no captured identity yet.
    ///
    /// It also pins the negative half — the validation invariant refuses
    /// BEFORE anything is written — because a refusal that still left a
    /// row behind would be far worse than one that never happened.
    #[tokio::test]
    async fn a_creates_snapshot_is_resolved_once_and_stored_with_the_session() {
        let state = StateDir::new();
        let work = tempfile::tempdir().expect("workdir");
        let cwd = work.path().to_string_lossy().to_string();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");

        // `/opt/bin/claude` does not exist and never runs: the launch is
        // expected to fail somewhere past validation, which is fine — what
        // this test is about is what validation RESOLVED, and the launching
        // row is committed before any of that.
        let created = sup
            .create_session(
                CreateInputs {
                    cwd: &cwd,
                    parent: None,
                    mode: CreateMode::Raw {
                        invocation: "/opt/bin/claude --dangerously-skip-permissions".to_string(),
                        agent_kind: None,
                        resume_template: None,
                    },
                    title: Some("t".to_string()),
                    cols: 80,
                    rows: 24,
                },
                None,
            )
            .await;
        let id = match &created {
            Ok(info) => info.id.clone(),
            Err(e) => panic!("the create should reach a launch: {e:#}"),
        };
        assert_eq!(
            created.as_ref().unwrap().restart_offer,
            RestartOffer::FreshOnly,
            "nothing can be captured at create time, and the derived template needs an id"
        );
        let snapshot = sup
            .session_snapshot(&id)
            .await
            .expect("reading the snapshot")
            .expect("the session exists");
        assert_eq!(snapshot.kind, AgentKind::Claude);
        assert_eq!(
            snapshot.resume_template.as_deref().unwrap(),
            [
                "/opt/bin/claude",
                "--resume",
                crate::agent_kind::CONVERSATION_PLACEHOLDER
            ],
            "the template is built from the ORIGINAL first token, not a bare command name"
        );
        assert_eq!(snapshot.captured_conversation, None);
        assert_eq!(snapshot.resume_argv, None);
        assert_eq!(snapshot.first_input_at, None);

        // The validation invariant, refused before anything is stored.
        let before = sup.store.load_all().await.expect("load").len();
        let refused = sup
            .create_session(
                CreateInputs {
                    cwd: &cwd,
                    parent: None,
                    mode: CreateMode::Raw {
                        invocation: "claude".to_string(),
                        agent_kind: None,
                        resume_template: Some(vec!["claude".to_string(), "--continue".to_string()]),
                    },
                    title: None,
                    cols: 80,
                    rows: 24,
                },
                None,
            )
            .await
            .expect_err("a placeholder-free template on an integrated kind is refused");
        assert_eq!(error_kind(&refused), ErrorKind::InvalidRequest);
        assert_eq!(
            sup.store.load_all().await.expect("load").len(),
            before,
            "a refused create must not leave a row behind"
        );
    }

    /// Name resolution chooses the whole matching profile from one catalog
    /// snapshot and returns useful candidates for both exact-match failures.
    #[tokio::test]
    async fn profile_name_resolution_is_exact_and_refuses_missing_or_ambiguous_names() {
        let state = StateDir::new();
        let work = tempfile::tempdir().expect("workdir");
        let cwd = work.path().to_string_lossy().to_string();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let profiles = sup.store.profiles().await.expect("starter catalog");
        assert_eq!(
            profiles.len(),
            4,
            "the fresh catalog supplies four candidates"
        );
        let chosen = profiles
            .iter()
            .find(|profile| profile.id == "starter-codex")
            .expect("Codex starter");
        let inputs = |name: &str| CreateInputs {
            cwd: &cwd,
            parent: Some("parent-1".to_string()),
            mode: CreateMode::ProfileName {
                profile_name: name.to_string(),
            },
            title: None,
            cols: 80,
            rows: 24,
        };

        let resolved = sup
            .validate_create(inputs(&chosen.name))
            .await
            .expect("an exact unique name resolves");
        let source = resolved
            .source_profile
            .expect("profile resolution records its source");
        assert_eq!(source.id, chosen.id);
        assert_eq!(source.name, chosen.name);
        assert_eq!(resolved.invocation, chosen.invocation);

        for (id, name) in [("exact-a", "Exact Name"), ("exact-b", "Exact Name")] {
            sup.store
                .insert_profile_with_id(farhelm_proto::Profile {
                    id: id.to_string(),
                    name: name.to_string(),
                    invocation: "agent".to_string(),
                    agent_kind: AgentKind::Generic,
                    resume_template: None,
                })
                .await
                .expect("insert duplicate-name fixture");
        }

        let ambiguous = match sup.validate_create(inputs("Exact Name")).await {
            Err(error) => error,
            Ok(_) => panic!("two exact matches are ambiguous"),
        };
        assert_eq!(error_kind(&ambiguous), ErrorKind::InvalidRequest);
        let ambiguous = format!("{ambiguous:#}");
        assert!(ambiguous.contains("exact-a") && ambiguous.contains("exact-b"));

        let missing = match sup.validate_create(inputs("exact name")).await {
            Err(error) => error,
            Ok(_) => panic!("name matching is exact, including case"),
        };
        assert_eq!(error_kind(&missing), ErrorKind::InvalidRequest);
        let missing = format!("{missing:#}");
        assert!(
            missing.contains("available profiles") && missing.contains("starter-claude"),
            "a missing exact name lists candidates the caller can send: {missing}"
        );
    }

    /// A raw create is the one path that reaches tmux with no profile in
    /// front of it — its invocation comes straight from the HTTP API's
    /// `CreateSession` (or a test), never from `farhelm spawn`, which only
    /// selects or derives a profile and never carries a raw invocation. So
    /// `{cwd}` as the invocation's PROGRAM has to be refused here too, in
    /// the identical wording the profile editor gives
    /// (`agent_kind::ensure_no_cwd_program`) — otherwise a wrapper-shaped
    /// invocation given directly to the raw-create path would slip past
    /// the one boundary a profile write goes through.
    #[tokio::test]
    async fn a_raw_create_refuses_a_cwd_placeholder_as_the_program() {
        let state = StateDir::new();
        let work = tempfile::tempdir().expect("workdir");
        let cwd = work.path().to_string_lossy().to_string();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let refusal = match sup
            .validate_create(CreateInputs {
                cwd: &cwd,
                parent: None,
                mode: CreateMode::Raw {
                    invocation: format!("{} claude", crate::agent_kind::CWD_PLACEHOLDER),
                    agent_kind: None,
                    resume_template: None,
                },
                title: None,
                cols: 80,
                rows: 24,
            })
            .await
        {
            Err(error) => error,
            Ok(_) => panic!("a `{{cwd}}`-first invocation must not validate"),
        };
        assert_eq!(error_kind(&refusal), ErrorKind::InvalidRequest);
        let refusal = format!("{refusal:#}");
        // Pin the FULL shared wording, not one fragment: an editor user
        // needs the placeholder name to recognize what tripped the
        // refusal, "PROGRAM" to understand why, and the remedy to know
        // what to do about it. A test that checked only one substring
        // could pass while any of the other two silently regressed away.
        assert!(
            refusal.contains(crate::agent_kind::CWD_PLACEHOLDER)
                && refusal.contains("PROGRAM")
                && refusal.contains("belongs in an argument slot"),
            "the refusal uses the shared placeholder-as-program wording: {refusal}"
        );
    }

    /// `{cwd}` is legal anywhere past the program slot, and by design
    /// filling it is spawn-time's job (`agent_kind::fill_cwd`, meant to be
    /// called only from `Supervisor::spawn_agent`, once the launch cwd is
    /// known) rather than validation's — so a wrapper invocation must
    /// validate successfully with the placeholder still literally present
    /// in the resulting `LaunchRequest`. `spawn_agent` does not wire up the
    /// fill in this commit (that lands in the next commit of the stack);
    /// this test pins the half already in place: "argv shape is
    /// acceptable" is checked here and must not eagerly substitute the
    /// placeholder validation is not responsible for.
    #[tokio::test]
    async fn a_raw_create_accepts_a_cwd_placeholder_in_an_argument_slot() {
        let state = StateDir::new();
        let work = tempfile::tempdir().expect("workdir");
        let cwd = work.path().to_string_lossy().to_string();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let resolved = sup
            .validate_create(CreateInputs {
                cwd: &cwd,
                parent: None,
                mode: CreateMode::Raw {
                    invocation: format!("wrapper {} claude", crate::agent_kind::CWD_PLACEHOLDER),
                    agent_kind: None,
                    resume_template: None,
                },
                title: None,
                cols: 80,
                rows: 24,
            })
            .await
            .expect("a `{cwd}` in an argument slot is a valid invocation");
        assert_eq!(
            resolved.argv,
            ["wrapper", crate::agent_kind::CWD_PLACEHOLDER, "claude"],
            "validation must not substitute the placeholder itself"
        );
    }

    /// Default derivation names the newest profile-backed session and never
    /// walks backward when that profile has since been removed.
    #[tokio::test]
    async fn derived_profile_refuses_a_gone_newest_profile_without_walkback() {
        let state = StateDir::new();
        let work = tempfile::tempdir().expect("workdir");
        let cwd = work.path().to_string_lossy().to_string();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let starter = sup
            .store
            .profile("starter-claude")
            .await
            .unwrap()
            .expect("starter profile");
        let gone = match sup
            .store
            .create_profile(
                "Gone Agent".to_string(),
                "agent".to_string(),
                AgentKind::Generic,
                None,
            )
            .await
            .expect("create profile")
        {
            crate::store::ProfileCreation::Created(profile) => profile,
            other => panic!("profile creation must succeed: {other:?}"),
        };
        const SEEDED_CREATED_AT: i64 = 1_700_000_000;
        let source =
            |id: &str, title: &str, profile: crate::store::ProfileSnapshot| StoredSession {
                conversation_source: None,
                id: id.to_string(),
                parent: None,
                archived: false,
                title: title.to_string(),
                created_at: SEEDED_CREATED_AT,
                last_activity_at: SEEDED_CREATED_AT,
                creation_seq: 0,
                cwd: cwd.clone(),
                invocation: "agent".to_string(),
                tmux_name: format!("fh-{id}"),
                pane: "%0".to_string(),
                outcome: LastOutcome::Running,
                agent_kind: AgentKind::Generic,
                resume_template: None,
                canonical_cwd: None,
                captured_conversation: None,
                captured_record: None,
                capture_ambiguous: false,
                first_input_at: None,
                generation: 0,
                launch_scoped: false,
                source_profile: Some(profile),
            };
        sup.store
            .insert_session(
                source(
                    "older",
                    "older",
                    crate::store::ProfileSnapshot {
                        id: starter.id,
                        name: starter.name,
                    },
                ),
                None,
            )
            .await
            .expect("seed older source");
        sup.store
            .insert_session(
                source(
                    "newest",
                    "newest",
                    crate::store::ProfileSnapshot {
                        id: gone.id.clone(),
                        name: gone.name.clone(),
                    },
                ),
                None,
            )
            .await
            .expect("seed same-second newer source");
        sup.store
            .delete_profile(&gone.id)
            .await
            .expect("delete profile");

        let refusal = match sup
            .validate_create(CreateInputs {
                cwd: &cwd,
                parent: None,
                mode: CreateMode::DerivedProfile,
                title: None,
                cols: 80,
                rows: 24,
            })
            .await
        {
            Err(error) => error,
            Ok(_) => panic!("a gone newest profile is a precondition failure"),
        };
        assert_eq!(error_kind(&refusal), ErrorKind::InvalidRequest);
        let refusal = format!("{refusal:#}");
        assert!(
            refusal.contains("Gone Agent") && refusal.contains("--agent"),
            "the refusal names the lost choice and the explicit remedy: {refusal}"
        );
    }

    /// A host with no profile-backed history cannot guess what spawn should
    /// run, and the refusal leaves no session behind.
    #[tokio::test]
    async fn derived_profile_without_source_history_is_a_clean_precondition_failure() {
        let state = StateDir::new();
        let work = tempfile::tempdir().expect("workdir");
        let cwd = work.path().to_string_lossy().to_string();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let refusal = match sup
            .validate_create(CreateInputs {
                cwd: &cwd,
                parent: None,
                mode: CreateMode::DerivedProfile,
                title: None,
                cols: 80,
                rows: 24,
            })
            .await
        {
            Err(error) => error,
            Ok(_) => panic!("a profile cannot be derived from no profile-backed sessions"),
        };
        assert_eq!(error_kind(&refusal), ErrorKind::InvalidRequest);
        assert!(format!("{refusal:#}").contains("--agent"));
        assert!(sup.store.load_all().await.unwrap().is_empty());
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
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        let launch_dir = state.path().join("launch");
        let request = |req_id: u64, agent_kind: Option<AgentKind>| ControlMsg::CreateSession {
            req_id,
            parent: None,
            profile_name: None,
            cwd: "/".to_string(),
            invocation: Some("agent".to_string()),
            profile_id: None,
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
        handle_control(
            &sup,
            request(1, None),
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;
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
        handle_control(
            &sup,
            request(2, None),
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;
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
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
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
    /// about each other's in-flight launch (see [`KeyedLocks`] for why
    /// that specific ambiguity is what it exists to remove), one that
    /// pruned too eagerly would hand a waiter a DIFFERENT mutex for the
    /// same key, and one that never pruned would grow a map entry per key
    /// this process has ever seen.
    #[tokio::test]
    async fn intent_locks_exclude_hand_off_and_prune() {
        let locks = Arc::new(KeyedLocks::default());
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
        let state = StateDir::new();
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
            .create_session_without_overrides(
                "/",
                "agent",
                None,
                80,
                24,
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint: raw_fingerprint("/", "agent", None, None, None),
                    dedup_scope: DedupScope::Permanent,
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
            .create_session_without_overrides(
                "/",
                "agent",
                None,
                80,
                24,
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint: raw_fingerprint("/", "agent", None, None, None),
                    dedup_scope: DedupScope::Permanent,
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
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let work = state.path().join("appears-later");
        let cwd = work.to_string_lossy().to_string();
        let claim = |fingerprint_cwd: &str, invocation: &str| IntentClaim {
            intent_key: "one-intent".to_string(),
            fingerprint: raw_fingerprint(fingerprint_cwd, invocation, None, None, None),
            dedup_scope: DedupScope::Permanent,
        };

        let first = sup
            .create_session_without_overrides(
                &cwd,
                "agent",
                None,
                80,
                24,
                Some(claim(&cwd, "agent")),
            )
            .await
            .expect_err("the working directory does not exist yet");
        assert_eq!(error_kind(&first), ErrorKind::InvalidRequest);
        assert!(
            format!("{first:#}").contains("does not exist"),
            "the refusal must name what was wrong: {first:#}"
        );

        std::fs::create_dir(&work).expect("the directory the user was about to create");
        let replay = sup
            .create_session_without_overrides(
                &cwd,
                "agent",
                None,
                80,
                24,
                Some(claim(&cwd, "agent")),
            )
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
            .create_session_without_overrides(
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
        let state = StateDir::new();
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
        let fingerprint = raw_fingerprint("/", "agent", None, None, None);
        sup.store
            .insert_session(
                StoredSession {
                    conversation_source: None,
                    id: "stranded".to_string(),
                    parent: None,
                    archived: false,
                    title: "stranded".to_string(),
                    created_at: now_unix(),
                    last_activity_at: now_unix(),
                    creation_seq: 0,
                    cwd: "/".to_string(),
                    invocation: "agent".to_string(),
                    tmux_name: "fh-stranded".to_string(),
                    pane: String::new(),
                    outcome: LastOutcome::Launching,
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    resume_template: None,
                    canonical_cwd: None,
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                    source_profile: None,
                },
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint: fingerprint.clone(),
                    dedup_scope: DedupScope::Permanent,
                }),
            )
            .await
            .expect("seed");
        sup.sessions.lock().await.insert(
            "stranded".to_string(),
            Arc::new(entry_with(None, LastOutcome::Launching)),
        );

        let session = sup
            .create_session_without_overrides(
                "/",
                "agent",
                None,
                80,
                24,
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint,
                    dedup_scope: DedupScope::Permanent,
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
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let fingerprint = raw_fingerprint("/", "agent", None, None, None);
        sup.store
            .insert_session(
                StoredSession {
                    conversation_source: None,
                    id: "ended".to_string(),
                    parent: None,
                    archived: false,
                    title: "ended".to_string(),
                    created_at: now_unix(),
                    last_activity_at: now_unix(),
                    creation_seq: 0,
                    cwd: "/".to_string(),
                    invocation: "agent".to_string(),
                    tmux_name: "fh-ended".to_string(),
                    pane: String::new(),
                    outcome: LastOutcome::Launching,
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    resume_template: None,
                    canonical_cwd: None,
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                    source_profile: None,
                },
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint: fingerprint.clone(),
                    dedup_scope: DedupScope::Permanent,
                }),
            )
            .await
            .expect("seed a pending reservation");
        sup.store
            .transition("ended", 0, Transition::ObservedExit { exit_code: Some(1) })
            .await
            .expect("the session ran and finished");

        let replayed = sup
            .create_session_without_overrides(
                "/",
                "agent",
                None,
                80,
                24,
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint,
                    dedup_scope: DedupScope::Permanent,
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

    /// An EXPLICIT title carrying a control character is refused, and
    /// nothing about the request survives the refusal.
    ///
    /// Titles are durable metadata this supervisor echoes verbatim into
    /// every `SessionList` reply, and the renderers are not all
    /// escape-immune: the helm already logs a title through `tracing` when
    /// it opens its startup session (farhelm-helm's "startup session
    /// created" line), so a terminal-bound consumer exists today, and a CLI
    /// `list` would be another. There an embedded escape sequence is
    /// terminal injection while a bare newline breaks the one-line-label
    /// assumption every renderer makes. Manual testing against the real
    /// binary confirmed this was previously accepted and echoed verbatim
    /// (an ANSI OSC sequence and a newline both went in and came back out
    /// untouched).
    ///
    /// The fixtures are chosen so no single one can carry the test: ESC
    /// appears ALONE in one of them, because an OSC fixture bundles ESC
    /// with BEL and would still pass against an implementation that missed
    /// ESC entirely. DEL and a C1 byte are there because `char::is_control`
    /// covers three disjoint ranges and a hand-rolled ASCII-only check
    /// would pass the first two cases. A normal multi-script Unicode title
    /// — emoji, CJK, spaces — is NOT a control character and must still be
    /// accepted, so the boundary is pinned on both sides rather than only
    /// on the refusal.
    #[tokio::test]
    async fn a_title_with_control_characters_is_rejected_before_anything_is_stored() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();

        for (req_id, title) in [
            (1u64, "escape \u{1b}]0;evil\u{7} here".to_string()),
            (2, "line one\nline two".to_string()),
            // ESC on its own: the OSC fixture above hides an
            // implementation that only recognizes BEL.
            (3, "bare escape \u{1b} here".to_string()),
            (4, "delete \u{7f} here".to_string()),
            (5, "c1 control \u{9b} here".to_string()),
        ] {
            handle_control(
                &sup,
                ControlMsg::CreateSession {
                    req_id,
                    parent: None,
                    profile_name: None,
                    cwd: "/".to_string(),
                    invocation: Some("agent".to_string()),
                    profile_id: None,
                    title: Some(title.clone()),
                    cols: 80,
                    rows: 24,
                    intent_key: None,
                    agent_kind: None,
                    resume_template: None,
                },
                ConnectionCtx {
                    tx: &tx,
                    priority: &tx,
                    input_routes: &mut input_routes,
                    upload_routes: &mut no_uploads(),
                    tasks: &mut tasks,
                },
            )
            .await;
            let frame = rx.try_recv().expect("a reply must have been sent");
            let decoded: ControlMsg = serde_json::from_slice(&frame.body).expect("decode");
            let ControlMsg::Error { kind, message, .. } = decoded else {
                panic!("a title with a control character must be refused: {decoded:?} ({title:?})");
            };
            assert_eq!(kind, ErrorKind::InvalidRequest, "for {title:?}");
            assert!(
                message.contains("control characters"),
                "the refusal must name what was wrong: {message}"
            );
        }
        assert!(
            sup.store.load_all().await.expect("load").is_empty(),
            "a refused request must not have reached the store at all"
        );
        assert!(
            sup.sessions.lock().await.is_empty(),
            "a refused request must not have created a session either"
        );

        // A title with no control characters — including non-ASCII script
        // mixes that are easy to conflate with "unusual" — is accepted and
        // judged on its merits, same as the intent-key boundary test above.
        handle_control(
            &sup,
            ControlMsg::CreateSession {
                req_id: 6,
                parent: None,
                profile_name: None,
                cwd: "/".to_string(),
                invocation: Some("agent".to_string()),
                profile_id: None,
                title: Some("🚀 デモ project — a normal title".to_string()),
                cols: 80,
                rows: 24,
                intent_key: None,
                agent_kind: None,
                resume_template: None,
            },
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;
        let frame = rx.try_recv().expect("a reply must have been sent");
        let decoded: ControlMsg = serde_json::from_slice(&frame.body).expect("decode");
        assert!(
            matches!(decoded, ControlMsg::SessionCreated { .. }),
            "a title with only ordinary Unicode must be accepted: {decoded:?}"
        );
    }

    /// A title DERIVED from the cwd is sanitized rather than refused.
    ///
    /// The asymmetry with the test above is the whole point, and it is easy
    /// to "simplify" away: a control character is legal in a path
    /// component, so refusing a derived title would make an existing,
    /// perfectly usable directory impossible to open a session in over a
    /// label the caller never chose. The caller omitted the title precisely
    /// to let the server pick one, so the server fixes it up — to U+FFFD
    /// rather than to nothing, so the label still shows something was
    /// removed — and the create succeeds.
    #[tokio::test]
    async fn a_title_derived_from_a_control_character_cwd_is_sanitized_not_refused() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();

        // A real directory, because `validate_create` insists the cwd
        // exists before it ever looks at the title.
        let work = tempfile::tempdir().expect("work dir");
        let evil = work.path().join("evil\u{1b}name");
        std::fs::create_dir(&evil).expect("a control character is legal in a path component");

        handle_control(
            &sup,
            ControlMsg::CreateSession {
                req_id: 1,
                parent: None,
                profile_name: None,
                cwd: evil.to_str().expect("tempdir paths are UTF-8").to_string(),
                invocation: Some("agent".to_string()),
                profile_id: None,
                title: None,
                cols: 80,
                rows: 24,
                intent_key: None,
                agent_kind: None,
                resume_template: None,
            },
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;
        let frame = rx.try_recv().expect("a reply must have been sent");
        let decoded: ControlMsg = serde_json::from_slice(&frame.body).expect("decode");
        let ControlMsg::SessionCreated { session, .. } = decoded else {
            panic!("a legal directory must remain usable as a cwd: {decoded:?}");
        };
        assert!(
            session.title.contains('\u{FFFD}'),
            "the replacement must be visible in the label: {:?}",
            session.title
        );
        assert!(
            !session.title.chars().any(char::is_control),
            "nothing control-shaped may survive derivation: {:?}",
            session.title
        );

        // The STORED title, not merely the echoed one: the reply is
        // built from the same value that was persisted, and a future
        // sanitize-on-the-way-out would pass the assertion above while
        // still writing the raw bytes to disk.
        let stored = sup.store.load_all().await.expect("load");
        let [row] = stored.as_slice() else {
            panic!("exactly one session must have been created: {stored:?}");
        };
        assert_eq!(row.title, session.title);
    }

    /// A keyed create refused for its title behaves like every other keyed
    /// refusal: the retry replays it, and a corrected title is a key reuse.
    ///
    /// This is what makes the check's PLACEMENT load-bearing rather than
    /// cosmetic. Refusing at the protocol edge would answer before the
    /// reservation lookup, so the refusal would never be recorded and a
    /// retry would re-derive it — and, worse, a pre-existing SUCCESSFUL
    /// reservation whose title predates this rule would stop replaying its
    /// session. Living in `validate_create` puts the refusal on the path
    /// `record_refused_create` owns, which is what the first two requests
    /// here prove. The third pins the other half of the contract: fixing
    /// the title makes it a DIFFERENT request under the same key
    /// (`create_fingerprint` binds the title), and a reused key is a client
    /// bug rather than a merge — so it is a `Conflict`, not a belated
    /// success. Modelled on
    /// `a_failed_create_replays_its_error_and_a_changed_override_conflicts`.
    #[tokio::test]
    async fn a_keyed_title_refusal_replays_and_a_corrected_title_conflicts() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();
        let request = |req_id: u64, title: &str| ControlMsg::CreateSession {
            req_id,
            parent: None,
            profile_name: None,
            cwd: "/".to_string(),
            invocation: Some("agent".to_string()),
            profile_id: None,
            title: Some(title.to_string()),
            cols: 80,
            rows: 24,
            intent_key: Some("one-intent".to_string()),
            agent_kind: None,
            resume_template: None,
        };
        let reply = |rx: &mut mpsc::Receiver<Frame>| {
            let frame = rx.try_recv().expect("a reply must have been sent");
            serde_json::from_slice::<ControlMsg>(&frame.body).expect("decode")
        };

        handle_control(
            &sup,
            request(1, "bad \u{1b} title"),
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;
        let ControlMsg::Error {
            message: first_message,
            kind: first_kind,
            ..
        } = reply(&mut rx)
        else {
            panic!("a control character in an explicit title must be refused");
        };
        assert_eq!(first_kind, ErrorKind::InvalidRequest);
        assert!(
            first_message.contains("control characters"),
            "the refusal must name what was wrong: {first_message}"
        );

        // Identical request, identical key: the RECORDED answer, replayed
        // rather than recomputed.
        handle_control(
            &sup,
            request(2, "bad \u{1b} title"),
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;
        let ControlMsg::Error { message, kind, .. } = reply(&mut rx) else {
            panic!("a replayed failure must still be an error");
        };
        assert_eq!(message, first_message, "the replay must be the same answer");
        assert_eq!(kind, first_kind);

        // Same key, a title that would have been fine on its own: the key
        // is spent, and a different fingerprint under it is a reuse.
        handle_control(
            &sup,
            request(3, "good title"),
            ConnectionCtx {
                tx: &tx,
                priority: &tx,
                input_routes: &mut input_routes,
                upload_routes: &mut no_uploads(),
                tasks: &mut tasks,
            },
        )
        .await;
        let ControlMsg::Error { message, kind, .. } = reply(&mut rx) else {
            panic!("a key reused for a different request must be refused");
        };
        assert_eq!(
            kind,
            ErrorKind::Conflict,
            "correcting the title does not un-spend the key: {message}"
        );
        assert!(
            message.contains("one-intent") && message.contains("different create request"),
            "the refusal must name the key and say why: {message}"
        );

        assert!(
            sup.sessions.lock().await.is_empty(),
            "none of the three attempts may have created a session"
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
        let state = StateDir::new();
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
            .create_session_without_overrides(
                &work.path().to_string_lossy(),
                "sh -c 'sleep 300'",
                None,
                80,
                24,
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint: raw_fingerprint(
                        &work.path().to_string_lossy(),
                        "sh -c 'sleep 300'",
                        None,
                        None,
                        None,
                    ),
                    dedup_scope: DedupScope::Permanent,
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
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let fingerprint = raw_fingerprint("/", "agent", None, None, None);
        sup.store
            .insert_session(
                StoredSession {
                    conversation_source: None,
                    id: "stranded".to_string(),
                    parent: None,
                    archived: false,
                    title: "stranded".to_string(),
                    created_at: now_unix(),
                    last_activity_at: now_unix(),
                    creation_seq: 0,
                    cwd: "/".to_string(),
                    invocation: "agent".to_string(),
                    tmux_name: "fh-stranded".to_string(),
                    pane: String::new(),
                    outcome: LastOutcome::Launching,
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    resume_template: None,
                    canonical_cwd: None,
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                    source_profile: None,
                },
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint: fingerprint.clone(),
                    dedup_scope: DedupScope::Permanent,
                }),
            )
            .await
            .expect("seed a pending reservation");
        let spec = crate::launch::spec_path_for_launch(state.path(), "stranded", 0);
        std::fs::create_dir(crate::launch::status_path_for_spec(&spec))
            .expect("plant an unreadable sentinel");

        let error = sup
            .create_session_without_overrides(
                "/",
                "agent",
                None,
                80,
                24,
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint,
                    dedup_scope: DedupScope::Permanent,
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
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let fingerprint = raw_fingerprint("/", "agent", None, None, None);
        sup.store
            .insert_session(
                StoredSession {
                    conversation_source: None,
                    id: "stranded".to_string(),
                    parent: None,
                    archived: false,
                    title: "stranded".to_string(),
                    created_at: now_unix(),
                    last_activity_at: now_unix(),
                    creation_seq: 0,
                    cwd: "/".to_string(),
                    invocation: "agent".to_string(),
                    tmux_name: "fh-stranded".to_string(),
                    pane: String::new(),
                    outcome: LastOutcome::Launching,
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    resume_template: None,
                    canonical_cwd: None,
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                    source_profile: None,
                },
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint: fingerprint.clone(),
                    dedup_scope: DedupScope::Permanent,
                }),
            )
            .await
            .expect("seed a pending reservation");
        // A leftover spec (but no sentinel — a sentinel would itself be
        // evidence the launch happened) in a directory that refuses
        // unlinking.
        let spec = crate::launch::spec_path_for_launch(state.path(), "stranded", 0);
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
            .create_session_without_overrides(
                "/",
                "agent",
                None,
                80,
                24,
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint,
                    dedup_scope: DedupScope::Permanent,
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
        let state = StateDir::new();
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
            .create_session_without_overrides(
                "/",
                "agent",
                None,
                80,
                24,
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint: raw_fingerprint("/", "agent", None, None, None),
                    dedup_scope: DedupScope::Permanent,
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

    /// "No supervisor is running here" is the single most common way this
    /// dial fails, and the raw kernel text for it ("No such file or
    /// directory", "Connection refused (os error 111)") tells an operator
    /// nothing about unix sockets, this state dir, or what to run. Both
    /// shapes are pinned because they arise from genuinely different
    /// states — a host where a supervisor was never started, and one where
    /// it died and left its socket file behind — reaching this code as
    /// different `ErrorKind`s that a narrowed match could easily drop.
    ///
    /// `--state-dir` is asserted specifically: `internal stdio` is reached
    /// over ssh with a state dir that is usually not the remote default,
    /// so a remedy printed without it starts a supervisor somewhere the
    /// caller will still not find. The io error must survive in the chain
    /// as well — the friendly context is an addition, not a replacement
    /// for the errno a bug report needs.
    #[tokio::test]
    async fn connect_names_the_socket_and_state_dir_when_nothing_listens() {
        let dir = tempfile::tempdir().expect("state dir");
        let socket = Supervisor::socket_path(dir.path());

        // Never started: no socket file at all (`NotFound`).
        let missing = connect(dir.path()).await.unwrap_err();
        // Died and left the file behind: `ConnectionRefused`. Neither std
        // nor tokio unlinks a bound unix socket on drop, which is what
        // makes this reproduce the stale-socket state exactly.
        let listener = UnixListener::bind(&socket).expect("bind");
        drop(listener);
        assert!(socket.exists(), "the stale socket file must remain");
        // Retried, briefly, and the reason is a real race rather than
        // flakiness insurance: this is a multi-threaded test binary whose
        // other tests spawn processes constantly, and a `fork` that lands
        // between the `bind` above and its `drop` gives the child a
        // duplicate of the listening descriptor. Close-on-exec closes it
        // the instant that child execs, but until then the socket still
        // has a listener and `connect` legitimately SUCCEEDS. Waiting out
        // that window costs nothing; asserting on the first answer made
        // the whole suite fail about one run in three.
        // Bounded, so a genuine regression fails loudly instead of hanging.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let refused = loop {
            match connect(dir.path()).await {
                Err(error) => break error,
                Ok(stream) => {
                    drop(stream);
                    assert!(
                        std::time::Instant::now() < deadline,
                        "a socket whose listener is gone kept accepting connections"
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            }
        };

        for (err, expected_kind) in [
            (missing, std::io::ErrorKind::NotFound),
            (refused, std::io::ErrorKind::ConnectionRefused),
        ] {
            let rendered = format!("{err:#}");
            assert!(
                rendered.contains(&socket.display().to_string()),
                "must name the socket it could not reach: {rendered}"
            );
            assert!(
                rendered.contains("farhelm supervisor run --state-dir"),
                "must suggest starting a supervisor on THIS state dir: {rendered}"
            );
            assert!(
                rendered.contains(&dir.path().display().to_string()),
                "the suggested command must name the state dir: {rendered}"
            );
            let io = err
                .chain()
                .find_map(|cause| cause.downcast_ref::<std::io::Error>())
                .expect("the underlying io error must stay in the chain");
            assert_eq!(io.kind(), expected_kind);
        }
    }

    /// A failure that is NOT "nothing is listening" must keep the generic
    /// context and the raw error, because the remedy would be a wrong
    /// answer: no amount of `farhelm supervisor run` fixes a path whose
    /// parent is a regular file, and printing it would send the operator
    /// down the wrong trail.
    ///
    /// A non-directory component is used rather than the more obvious
    /// "point at a directory" — Linux answers `ConnectionRefused` for ANY
    /// existing non-socket path, so a directory takes the remedy branch
    /// (deliberately, see `connect`) and would not exercise this one.
    #[tokio::test]
    async fn connect_keeps_the_generic_context_for_other_failures() {
        let dir = tempfile::tempdir().expect("state dir");
        let not_a_dir = dir.path().join("regular-file");
        std::fs::write(&not_a_dir, b"not a state dir").expect("write");

        let err = connect(&not_a_dir).await.unwrap_err();
        let rendered = format!("{err:#}");
        assert!(
            !rendered.contains("farhelm supervisor run"),
            "an unrelated dial failure must not gain a guessed remedy: {rendered}"
        );
        assert!(
            rendered.contains("connecting to supervisor socket")
                && rendered.contains(&not_a_dir.display().to_string()),
            "the generic context must still name the path: {rendered}"
        );
        let io = err
            .chain()
            .find_map(|cause| cause.downcast_ref::<std::io::Error>())
            .expect("the underlying io error must stay in the chain");
        assert_ne!(io.kind(), std::io::ErrorKind::ConnectionRefused);
    }

    /// A profile-backed create whose profile is RENAMED while the launch is
    /// in flight reports the rename (PLAN_M6_75.md item 5).
    ///
    /// `SourceProfile`'s contract is that existence describes the catalog
    /// AT REPLY TIME, and a create is the one path where that is easy to
    /// get wrong: the profile was looked up before the launch, so copying
    /// `Present` from that lookup costs nothing and looks right. The gap it
    /// ignores is real — a launch is a tmux round trip plus two durable
    /// writes — and the reply would then assert something about the catalog
    /// that stopped being true while it worked.
    ///
    /// Forced deterministically through the create-lifecycle seam rather
    /// than by racing a spawned task: the rename is performed from inside
    /// the `DuringLaunch` stage, which is exactly the window between the
    /// pre-launch resolution and the reply.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_profile_renamed_during_the_launch_is_reported_by_the_create_reply() {
        let state = StateDir::new();
        let db = state.path().join("supervisor.db");
        let sup = {
            let db = db.clone();
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
                        // is what a concurrent profile edit would have.
                        let db = db.clone();
                        tokio::task::block_in_place(move || {
                            tokio::runtime::Handle::current().block_on(async move {
                                let store = SessionStore::open(&db, false).await?;
                                let profile = store
                                    .profile("starter-claude")
                                    .await?
                                    .expect("the starter is what the create named");
                                store
                                    .update_profile(farhelm_proto::Profile {
                                        name: "Renamed mid-launch".to_string(),
                                        ..profile
                                    })
                                    .await?;
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

        let session = sup
            .create_session(
                CreateInputs {
                    cwd: "/",
                    parent: None,
                    mode: CreateMode::Profile {
                        profile_id: "starter-claude".to_string(),
                    },
                    title: None,
                    cols: 80,
                    rows: 24,
                },
                None,
            )
            .await
            .expect("the create itself succeeds; only the catalog moved under it");
        let source = session
            .source_profile
            .expect("a profile-backed create records what it came from");
        assert_eq!(
            source.name, "claude",
            "the SNAPSHOT is what the user picked, and no later edit rewrites it"
        );
        assert_eq!(
            source.existence,
            ProfileExistence::Renamed,
            "existence describes the catalog when the reply was built, not when the profile was \
             resolved"
        );
    }

    /// A pending retry launches what the FIRST attempt resolved, even after
    /// its profile has been edited out from under it (PLAN_M6_75.md item
    /// 4).
    ///
    /// A profile is mutable and an accepted intent is not. Re-resolving the
    /// request on the retry would mean that editing a profile between a
    /// crash and its retry silently changes what an unchanged intent
    /// launches — the same key, the same request, a different agent — and
    /// there is no moment at which the client could have asked for that.
    /// The row the crashed attempt committed is the record of what was
    /// resolved, so the row is what the retry runs.
    ///
    /// The stranded row is planted directly, which is what a crash between
    /// the claim and the launch leaves behind: a reservation, a `Launching`
    /// row with no pane, and no tmux session anywhere.
    #[tokio::test]
    async fn a_pending_retry_runs_the_original_profile_resolution_not_the_edited_one() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let fingerprint = create_fingerprint(
            None,
            "/",
            &CreateMode::Profile {
                profile_id: "starter-claude".to_string(),
            },
            None,
        );
        sup.store
            .insert_session(
                StoredSession {
                    conversation_source: None,
                    id: "stranded".to_string(),
                    parent: None,
                    archived: false,
                    title: "stranded".to_string(),
                    created_at: now_unix(),
                    last_activity_at: now_unix(),
                    creation_seq: 0,
                    cwd: "/".to_string(),
                    invocation: "claude --original".to_string(),
                    tmux_name: "fh-stranded".to_string(),
                    pane: String::new(),
                    outcome: LastOutcome::Launching,
                    agent_kind: farhelm_proto::AgentKind::Claude,
                    resume_template: Some(vec![
                        "claude".to_string(),
                        "--resume".to_string(),
                        crate::agent_kind::CONVERSATION_PLACEHOLDER.to_string(),
                    ]),
                    canonical_cwd: Some("/".to_string()),
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                    source_profile: Some(ProfileSnapshot {
                        id: "starter-claude".to_string(),
                        name: "claude".to_string(),
                    }),
                },
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint: fingerprint.clone(),
                    dedup_scope: DedupScope::Permanent,
                }),
            )
            .await
            .expect("seed the crashed attempt");

        // The catalog moves between the two attempts: a rename AND a
        // changed invocation, so re-resolution would be visible in the
        // launched command line rather than only in a label.
        sup.store
            .update_profile(farhelm_proto::Profile {
                id: "starter-claude".to_string(),
                name: "Edited".to_string(),
                invocation: "claude --edited".to_string(),
                agent_kind: farhelm_proto::AgentKind::Claude,
                resume_template: None,
            })
            .await
            .expect("edit")
            .expect("the profile is there to edit");

        let session = sup
            .create_session(
                CreateInputs {
                    cwd: "/",
                    parent: None,
                    mode: CreateMode::Profile {
                        profile_id: "starter-claude".to_string(),
                    },
                    title: None,
                    cols: 80,
                    rows: 24,
                },
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint,
                    dedup_scope: DedupScope::Permanent,
                }),
            )
            .await
            .expect("the retry performs the create under the reserved identity");
        assert_eq!(session.id, "stranded");
        assert_eq!(
            session.invocation, "claude --original",
            "the retry runs what the first attempt resolved, not what the catalog says now"
        );
        let source = session.source_profile.expect("the snapshot rides the row");
        assert_eq!(source.name, "claude", "and so does the name it recorded");
        assert_eq!(
            source.existence,
            ProfileExistence::Renamed,
            "while existence is still derived fresh, as on every other reply"
        );
    }

    /// The same retry, with its profile DELETED between the attempts.
    ///
    /// The failure this excludes is worse than the edited case: deleting a
    /// profile would turn an already-accepted create — one the supervisor
    /// had committed a row for and told nobody about — into a permanent
    /// `NotFound` for its own intent key. The unknown-profile precondition
    /// is about a create that has not happened yet; this one already has.
    #[tokio::test]
    async fn a_pending_retry_survives_its_profile_being_deleted_between_attempts() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let fingerprint = create_fingerprint(
            None,
            "/",
            &CreateMode::Profile {
                profile_id: "starter-codex".to_string(),
            },
            None,
        );
        sup.store
            .insert_session(
                StoredSession {
                    conversation_source: None,
                    id: "stranded".to_string(),
                    parent: None,
                    archived: false,
                    title: "stranded".to_string(),
                    created_at: now_unix(),
                    last_activity_at: now_unix(),
                    creation_seq: 0,
                    cwd: "/".to_string(),
                    invocation: "codex".to_string(),
                    tmux_name: "fh-stranded".to_string(),
                    pane: String::new(),
                    outcome: LastOutcome::Launching,
                    agent_kind: farhelm_proto::AgentKind::Codex,
                    resume_template: Some(vec![
                        "codex".to_string(),
                        "resume".to_string(),
                        crate::agent_kind::CONVERSATION_PLACEHOLDER.to_string(),
                    ]),
                    canonical_cwd: Some("/".to_string()),
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                    source_profile: Some(ProfileSnapshot {
                        id: "starter-codex".to_string(),
                        name: "codex".to_string(),
                    }),
                },
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint: fingerprint.clone(),
                    dedup_scope: DedupScope::Permanent,
                }),
            )
            .await
            .expect("seed the crashed attempt");
        assert!(
            sup.store
                .delete_profile("starter-codex")
                .await
                .expect("delete")
        );

        let session = sup
            .create_session(
                CreateInputs {
                    cwd: "/",
                    parent: None,
                    mode: CreateMode::Profile {
                        profile_id: "starter-codex".to_string(),
                    },
                    title: None,
                    cols: 80,
                    rows: 24,
                },
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint,
                    dedup_scope: DedupScope::Permanent,
                }),
            )
            .await
            .expect("a create the supervisor already accepted must not become NotFound");
        assert_eq!(session.id, "stranded");
        assert_eq!(session.invocation, "codex");
        let source = session.source_profile.expect("the snapshot rides the row");
        assert_eq!(source.name, "codex");
        assert_eq!(
            source.existence,
            ProfileExistence::Deleted,
            "the session still says what it came from, and the reply says that profile is gone"
        );
    }

    /// A RESTART keeps the session's source profile, and re-derives its
    /// existence (PLAN_M6_75.md item 4).
    ///
    /// A restart is a new launch generation of the same session, so what it
    /// was created from does not change — but the restart publishes a NEW
    /// entry, and an entry built without the snapshot loses it for every
    /// reply until the next supervisor reload puts it back. That is a
    /// SPEC.md violation with a long fuse: a profile-created session would
    /// simply look raw-created, and nothing would ever fail.
    ///
    /// The rename is applied between the create and the restart, so the
    /// reply cannot be passing by echoing a placeholder: the snapshotted
    /// name and the derived existence disagree with each other, and both
    /// have to be right.
    #[tokio::test]
    async fn a_restart_keeps_the_source_profile_and_re_derives_its_existence() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let cwd = state.path().to_string_lossy().to_string();
        let created = sup
            .create_session(
                CreateInputs {
                    cwd: &cwd,
                    parent: None,
                    mode: CreateMode::Profile {
                        profile_id: "starter-claude".to_string(),
                    },
                    title: None,
                    cols: 80,
                    rows: 24,
                },
                None,
            )
            .await
            .expect("create from the starter profile");

        sup.store
            .update_profile(farhelm_proto::Profile {
                id: "starter-claude".to_string(),
                name: "Claude, renamed".to_string(),
                invocation: "claude".to_string(),
                agent_kind: farhelm_proto::AgentKind::Claude,
                resume_template: None,
            })
            .await
            .expect("rename")
            .expect("the profile is there to rename");

        let restarted = sup
            .restart_session(&created.id, RestartMode::Fresh, true)
            .await
            .expect("restart");
        let source = restarted
            .source_profile
            .expect("a restart never changes what a session was created from");
        assert_eq!(source.id, "starter-claude");
        assert_eq!(
            source.name, "claude",
            "the snapshot is what the session recorded at creation"
        );
        assert_eq!(
            source.existence,
            ProfileExistence::Renamed,
            "and the existence beside it is derived for THIS reply"
        );

        // The republished ENTRY carries it too, which is what every later
        // reply is built from — a restart that returned the right reply
        // while dropping the entry's copy would look correct exactly once.
        let entry = sup
            .sessions
            .lock()
            .await
            .get(&created.id)
            .cloned()
            .expect("the restarted session is back on the map");
        assert_eq!(
            entry
                .info
                .source_profile
                .as_ref()
                .map(|profile| profile.id.as_str()),
            Some("starter-claude")
        );
    }

    /// A sampler still holding the PRE-restart entry advances the value
    /// every later reply reads — because the activity cell is one per
    /// session, not one per launch.
    ///
    /// The bug this pins is a lost update with no symptom at the moment it
    /// happens. `sample_pass` clones an entry, awaits a `capture-pane`
    /// round trip, and only then dates what it saw; a restart landing in
    /// that window republishes a new entry under the same id. With a
    /// per-launch cell the late write would advance the ABANDONED entry
    /// while the same sampler's durable write advanced the row, leaving
    /// this supervisor answering every reply from a value older than its
    /// own database — and staying wrong until some later change crossed
    /// the quantum, which for a session that just went quiet may be never.
    /// Restarting a session is also exactly when a user is watching the
    /// list, so the stale answer lands where it is most visible.
    ///
    /// The barrier is what makes the ordering a fact rather than a hope:
    /// the late writer is released only after `restart_session` has
    /// returned and published the replacement. The value it writes is
    /// chosen rather than sampled, so the assertion can be an equality —
    /// what is under test is WHERE a late write lands, and the sampler's
    /// own quantization and durable write have their own tests in
    /// `service::ticker` and `store`.
    #[tokio::test]
    async fn a_late_activity_write_from_a_pre_restart_entry_reaches_the_live_reply() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let cwd = state.path().to_string_lossy().to_string();
        let created = sup
            .create_session(
                CreateInputs {
                    cwd: &cwd,
                    parent: None,
                    mode: CreateMode::Raw {
                        invocation: "agent".to_string(),
                        agent_kind: Some(AgentKind::Generic),
                        resume_template: None,
                    },
                    title: None,
                    cols: 80,
                    rows: 24,
                },
                None,
            )
            .await
            .expect("create");

        // The handle a sampling pass would be holding across its await.
        let sampled = sup
            .sessions
            .lock()
            .await
            .get(&created.id)
            .cloned()
            .expect("the created session is on the map");

        let observed_at = created.created_at + 600;
        let released = Arc::new(tokio::sync::Barrier::new(2));
        let late_write = {
            let released = Arc::clone(&released);
            tokio::spawn(async move {
                released.wait().await;
                // The one line `ticker::note_activity` runs once it has
                // decided the observation may move the value.
                sampled
                    .last_activity_at
                    .store(observed_at, std::sync::atomic::Ordering::Relaxed);
            })
        };

        sup.restart_session(&created.id, RestartMode::Fresh, true)
            .await
            .expect("restart");
        released.wait().await;
        late_write.await.expect("the late writer finished");

        let live = sup
            .sessions
            .lock()
            .await
            .get(&created.id)
            .cloned()
            .expect("the restarted session is back on the map");
        assert_eq!(
            crate::service::status::entry_info(
                &live,
                &HashMap::new(),
                None,
                &crate::store::ProfileNames::new(),
            )
            .last_activity_at,
            observed_at,
            "the replacement entry must read the same cell the pre-restart entry was written \
             through; a per-launch cell would report the build-time value instead"
        );
    }

    /// The source-profile snapshot survives a supervisor RESTART, and its
    /// existence is derived fresh on the way back up (PLAN_M6_75.md item
    /// 4).
    ///
    /// The columns are only worth having if reload reads them: a session
    /// created from a profile, on a supervisor that then goes away, must
    /// come back still knowing what it came from. Nothing else in the tree
    /// covers the reload wiring — the create path builds its entry from the
    /// request rather than from the row, so a reload that dropped these two
    /// columns would pass every create-side test.
    ///
    /// The profile is DELETED while the supervisor is down, which is both
    /// the harder case and the honest one: it proves the reloaded snapshot
    /// is the session's own record rather than something re-derived from a
    /// catalog that no longer contains it.
    #[tokio::test]
    async fn a_reloaded_session_keeps_its_source_profile_and_derives_existence_freshly() {
        let state = StateDir::new();
        let cwd = state.path().to_string_lossy().to_string();
        let created = {
            let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
                .await
                .expect("supervisor");
            let created = sup
                .create_session(
                    CreateInputs {
                        cwd: &cwd,
                        parent: None,
                        mode: CreateMode::Profile {
                            profile_id: "starter-claude".to_string(),
                        },
                        title: None,
                        cols: 80,
                        rows: 24,
                    },
                    None,
                )
                .await
                .expect("create from the starter profile");
            sup.store
                .delete_profile("starter-claude")
                .await
                .expect("delete the profile out from under the session");
            created
        };

        let reloaded = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("a second supervisor over the same state directory");
        let entry = reloaded
            .sessions
            .lock()
            .await
            .get(&created.id)
            .cloned()
            .expect("the session is reloaded");
        let snapshot = entry
            .info
            .source_profile
            .as_ref()
            .expect("the stored snapshot survives the process that wrote it");
        assert_eq!(snapshot.id, "starter-claude");
        assert_eq!(snapshot.name, "claude");

        // Through a real reply, so the derivation is exercised rather than
        // the raw column: the profile is gone, and the reply must say so.
        let page = crate::service::listing::list_page(
            &reloaded,
            crate::service::listing::ListQuery {
                cursor: None,
                limit: 10,
            },
        )
        .await
        .expect("list");
        let listed = page
            .sessions
            .iter()
            .find(|session| session.id == created.id)
            .expect("the reloaded session lists");
        assert_eq!(
            listed
                .source_profile
                .as_ref()
                .map(|profile| profile.existence),
            Some(ProfileExistence::Deleted),
            "existence is derived at reply time, never reloaded from the row"
        );
    }

    /// A retry whose working directory was REPOINTED between the attempts
    /// is refused rather than launched somewhere else (PLAN_M6_75.md item
    /// 4).
    ///
    /// The path still stats fine — that is the whole difficulty. A symlink
    /// repointed between the crash and the retry leaves `ensure_cwd_usable`
    /// perfectly satisfied while the directory underneath is somebody
    /// else's, and the retry carries the crashed attempt's `canonical_cwd`
    /// forward: so the agent would run in the NEW target while conversation
    /// capture correlated against the OLD one. Nothing fails, no log line
    /// appears, and the session simply never captures its conversation —
    /// or, where the old target is another live project, correlates against
    /// records that are not its own.
    ///
    /// The refusal names both paths, because "working directory does not
    /// exist" would send the user looking for a typo when the directory is
    /// right there.
    #[tokio::test]
    async fn a_retry_whose_directory_was_repointed_is_refused_rather_than_relaunched() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let original = state.path().join("original");
        let replacement = state.path().join("replacement");
        std::fs::create_dir(&original).expect("original target");
        std::fs::create_dir(&replacement).expect("replacement target");
        let link = state.path().join("work");
        std::os::unix::fs::symlink(&original, &link).expect("symlink");
        let cwd = link.to_string_lossy().to_string();

        let fingerprint = raw_fingerprint(&cwd, "agent", None, None, None);
        sup.store
            .insert_session(
                StoredSession {
                    conversation_source: None,
                    id: "stranded".to_string(),
                    parent: None,
                    archived: false,
                    title: "stranded".to_string(),
                    created_at: now_unix(),
                    last_activity_at: now_unix(),
                    creation_seq: 0,
                    cwd: cwd.clone(),
                    invocation: "agent".to_string(),
                    tmux_name: "fh-stranded".to_string(),
                    pane: String::new(),
                    outcome: LastOutcome::Launching,
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    resume_template: None,
                    // What the crashed attempt resolved, and what capture
                    // would go on correlating against.
                    canonical_cwd: Some(
                        std::fs::canonicalize(&original)
                            .expect("canonicalize")
                            .to_string_lossy()
                            .into_owned(),
                    ),
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                    source_profile: None,
                },
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint: fingerprint.clone(),
                    dedup_scope: DedupScope::Permanent,
                }),
            )
            .await
            .expect("seed the crashed attempt");

        // The repoint, between the two attempts.
        std::fs::remove_file(&link).expect("unlink");
        std::os::unix::fs::symlink(&replacement, &link).expect("re-link");

        let refusal = sup
            .create_session_without_overrides(
                &cwd,
                "agent",
                None,
                80,
                24,
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint,
                    dedup_scope: DedupScope::Permanent,
                }),
            )
            .await
            .expect_err("a retry must not relaunch into a directory that is no longer the one");
        assert_eq!(error_kind(&refusal), ErrorKind::InvalidRequest);
        let rendered = format!("{refusal:#}");
        assert!(
            rendered.contains(&replacement.to_string_lossy().to_string())
                && rendered.contains(&original.to_string_lossy().to_string()),
            "the refusal must name where the path resolves NOW and where the session was \
             created, or it reads as a missing directory: {rendered}"
        );
        assert!(
            sup.sessions.lock().await.is_empty(),
            "and nothing may have been launched"
        );
    }

    /// A catalog read that fails AFTER the session is created still tells
    /// the caller which session exists (PLAN_M6_75.md item 5).
    ///
    /// The reply is withheld — an unreadable catalog cannot be degraded
    /// into "the profile is gone" — but the create itself has already
    /// happened by then: the row is committed, the tmux session is running,
    /// and the entry is on the map. So the error is the ONLY thing the
    /// caller will ever see about a session that exists, and without the id
    /// in it there is no handle anywhere: an unkeyed create has no
    /// reservation to reconcile, the caller cannot attach, cannot delete,
    /// and the obvious response — retry the create — starts a second agent
    /// in the same directory.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_catalog_failure_after_the_launch_still_names_the_session_it_created() {
        let state = StateDir::new();
        let db = state.path().join("supervisor.db");
        let sup = {
            let db = db.clone();
            Supervisor::new_with_seams(
                state.path(),
                dummy_exe(),
                SupervisorTimeouts::default(),
                SupervisorSeams {
                    create_crash: Some(Arc::new(move |stage| {
                        if stage != CreateStage::DuringLaunch {
                            return Ok(());
                        }
                        // After the profile was resolved and the launch
                        // happened; before the reply is assembled.
                        let db = db.clone();
                        tokio::task::block_in_place(move || {
                            tokio::runtime::Handle::current().block_on(async move {
                                SessionStore::open(&db, false)
                                    .await?
                                    .drop_profile_catalog_for_test()
                                    .await;
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

        let failure = sup
            .create_session(
                CreateInputs {
                    cwd: "/",
                    parent: None,
                    mode: CreateMode::Profile {
                        profile_id: "starter-claude".to_string(),
                    },
                    title: None,
                    cols: 80,
                    rows: 24,
                },
                None,
            )
            .await
            .expect_err("an unreadable catalog withholds the reply");
        let rendered = format!("{failure:#}");

        let published = sup.sessions.lock().await;
        let (id, _) = published
            .iter()
            .next()
            .expect("the session was created and published before the catalog was read");
        assert!(
            rendered.contains(id.as_str()),
            "the failure must name the session that exists, or nothing ever can: {rendered}"
        );
        assert!(
            rendered.contains("WAS created"),
            "and must say plainly that it exists, so the caller does not retry into a \
             duplicate: {rendered}"
        );
        assert!(
            sup.store
                .session(id)
                .await
                .expect("read")
                .is_some_and(|row| row.outcome == LastOutcome::Running),
            "the session must be durable and confirmed, not rolled back with the reply"
        );
    }

    /// The same for a RESTART: the reply is withheld, and the new
    /// generation — terminal included — stays published.
    ///
    /// Worse than the create case if it were silent, because the obvious
    /// response to a failed restart is to restart again — which would kill
    /// the agent this restart just started and launch a third. The message
    /// therefore names the session AND says the restart succeeded.
    ///
    /// ## Why the terminal is asserted, and why the restart is a
    /// fresh-terminal one
    ///
    /// The bug this pins was not the message. A reply-build failure used to
    /// be classified as an AMBIGUOUS relaunch failure, which runs the
    /// generic recovery — and that recovery republishes an entry built from
    /// the PRE-restart one, terminal and all. So a catalog read failing
    /// after `publish_relaunched` had already installed the new generation
    /// overwrote it: the map ended up pointing at the terminal the restart
    /// had just replaced, with a `Launching` outcome, while the agent the
    /// restart actually started ran in a terminal nothing referenced. That
    /// contradicts SPEC.md's restart guarantee for a reason that has nothing
    /// to do with restarting.
    ///
    /// The generation alone could not catch it — the recovery republishes
    /// under `claim.generation` too, so `generation == 1` was true either
    /// way. What separates them is the TERMINAL, and it only differs when
    /// the restart builds a fresh one: a reused pane keeps its id across
    /// `respawn-pane`, so the terminal is identical whichever entry wins.
    /// Killing the tmux session first is what makes this the fresh-terminal
    /// path, and the new pane is then probed through tmux to prove it is
    /// live rather than merely different.
    #[tokio::test]
    async fn a_catalog_failure_after_a_restart_keeps_the_new_terminal_published() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let cwd = state.path().to_string_lossy().to_string();
        let created = sup
            .create_session(
                CreateInputs {
                    cwd: &cwd,
                    parent: None,
                    mode: CreateMode::Profile {
                        profile_id: "starter-claude".to_string(),
                    },
                    title: None,
                    cols: 80,
                    rows: 24,
                },
                None,
            )
            .await
            .expect("create from the starter profile");
        let before = sup
            .sessions
            .lock()
            .await
            .get(&created.id)
            .cloned()
            .expect("the create publishes an entry")
            .terminal
            .clone()
            .expect("with a terminal");

        // The terminal goes away, which is what an interrupted session (or
        // a tmux server that died) leaves behind — and is what makes the
        // restart below build a FRESH terminal with a pane id of its own.
        sup.tmux
            .kill_session(&before.tmux_name)
            .await
            .expect("kill the session's terminal");

        // Unreadable from here on: the restart's own work does not touch
        // the catalog, so only the reply-building read fails.
        sup.store.drop_profile_catalog_for_test().await;
        let failure = sup
            .restart_session(&created.id, RestartMode::Fresh, true)
            .await
            .expect_err("an unreadable catalog withholds the restart's reply too");
        let rendered = format!("{failure:#}");
        assert!(
            rendered.contains(&created.id) && rendered.contains("SUCCEEDED"),
            "the failure must name the session and say the restart happened, so nobody \
             restarts it again: {rendered}"
        );

        let entry = sup
            .sessions
            .lock()
            .await
            .get(&created.id)
            .cloned()
            .expect("the restarted session stays published despite the withheld reply");
        assert_eq!(
            entry.generation, 1,
            "and it is the NEW generation that is published, not the one the restart replaced"
        );
        assert_eq!(
            *entry.outcome.lock().expect("outcome mutex poisoned"),
            LastOutcome::Running,
            "the restart confirmed its launch, so a failure to DESCRIBE the session must not \
             walk that back to Launching"
        );
        let published = entry.terminal.clone().expect("a restarted session has one");
        assert_ne!(
            published.pane, before.pane,
            "the published terminal must be the one the restart created, not the pane it \
             replaced — a recovery that republished the pre-restart entry would leave the live \
             terminal unreferenced"
        );
        // `Owned`, not merely "not gone": the pane must both exist AND be
        // this session's, which is the whole point of the map pointing at
        // it.
        assert!(
            matches!(
                sup.tmux
                    .pane_process(&published.tmux_name, &published.pane)
                    .await
                    .expect("probe the published pane"),
                PaneProbe::Owned(_)
            ),
            "and it must be attachable: tmux has to know the pane the map points at"
        );
    }

    /// A retry adopts the title its TAKEOVER preserved, not the one its
    /// snapshot was resolved with — in the reply, in SQLite, and in the
    /// entry the very next list is served from.
    ///
    /// A rename is the one field a user can change after creation, and a
    /// retry's takeover is a delete-and-reinsert. The store keeps the
    /// renamed title (`SessionStore::restart_pending_launch` has its own
    /// test for that), but keeping it there is only half the fix: the caller
    /// resolved its `LaunchRequest` before the race and would otherwise
    /// build both its reply and the replacement `SessionEntry` from the
    /// stale label — so the durable row and every list this process serves
    /// disagree until the next reload, with the user's rename apparently
    /// reverted.
    ///
    /// The race is constructed by calling the launch directly with a
    /// deliberately STALE snapshot title against a row that already carries
    /// the new one, which is exactly the state a rename landing between
    /// `validate_retry`'s row read and the takeover's commit produces. Doing
    /// it this way rather than through a seam is what makes it
    /// deterministic; the serialization that keeps a rename from landing
    /// LATER (between the takeover and the map removal) is the lifecycle
    /// claim `launch_reserved` holds across both.
    #[tokio::test]
    async fn a_retry_publishes_the_title_its_takeover_preserved() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let cwd = state.path().to_string_lossy().to_string();
        let stranded = StoredSession {
            conversation_source: None,
            id: "stranded".to_string(),
            parent: None,
            archived: false,
            title: "as created".to_string(),
            created_at: now_unix(),
            last_activity_at: now_unix(),
            creation_seq: 0,
            cwd: cwd.clone(),
            invocation: "agent".to_string(),
            tmux_name: "fh-stranded".to_string(),
            pane: String::new(),
            outcome: LastOutcome::Launching,
            agent_kind: farhelm_proto::AgentKind::Generic,
            resume_template: None,
            canonical_cwd: Some(cwd.clone()),
            captured_conversation: None,
            captured_record: None,
            capture_ambiguous: false,
            first_input_at: None,
            generation: 0,
            launch_scoped: false,
            source_profile: None,
        };
        sup.store
            .insert_session(
                stranded.clone(),
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint: "fp".to_string(),
                    dedup_scope: DedupScope::Permanent,
                }),
            )
            .await
            .expect("seed the crashed attempt");
        sup.store
            .set_session_title("stranded", "as renamed")
            .await
            .expect("the user renames the stranded session");

        let reservation = Reservation {
            intent_key: "key".to_string(),
            fingerprint: "fp".to_string(),
            session_id: "stranded".to_string(),
            tmux_name: "fh-stranded".to_string(),
            dedup_scope: DedupScope::Permanent,
            outcome: ReservationOutcome::Pending,
        };
        let reply = sup
            .launch_reserved(
                LaunchRequest {
                    parent: None,
                    cwd: cwd.clone(),
                    launch_cwd: cwd.clone(),
                    invocation: "agent".to_string(),
                    argv: vec!["agent".to_string()],
                    // The STALE label: what the retry resolved before the
                    // rename it knows nothing about.
                    title: "as created".to_string(),
                    cols: 80,
                    rows: 24,
                    snapshot: IntegrationSnapshot {
                        kind: farhelm_proto::AgentKind::Generic,
                        resume_template: None,
                    },
                    canonical_cwd: cwd.clone(),
                    source_profile: None,
                },
                &Reserved::Retry(Box::new(reservation)),
            )
            .await
            .expect("the retry performs the create under the reserved identity");

        assert_eq!(
            reply.title, "as renamed",
            "the reply must carry the title the takeover committed, not the one it was built \
             from"
        );
        assert_eq!(
            sup.store
                .session("stranded")
                .await
                .expect("read")
                .expect("the takeover leaves a row")
                .title,
            "as renamed",
            "premise: the durable row keeps the rename"
        );
        assert_eq!(
            sup.sessions
                .lock()
                .await
                .get("stranded")
                .expect("the retry publishes an entry")
                .info
                .title,
            "as renamed",
            "and the entry the next list is served from must agree with the row, or the rename \
             looks reverted until the supervisor restarts"
        );
    }

    /// An unexecutable argv is refused on EVERY path that can produce one,
    /// not only at a profile write.
    ///
    /// `''` is the case that motivates this: it parses to a one-element argv
    /// holding the empty string, so an `argv.is_empty()` test — which is
    /// what the raw create and the pending retry each had — sees a perfectly
    /// good command line that names nothing. Profile CRUD has refused it for
    /// a while, so the same command line was accepted or refused depending
    /// on which door it came through.
    ///
    /// All three doors, because each reads its argv from a different place:
    /// the request (raw create), the crashed attempt's row (retry), and the
    /// session's stored invocation (restart). The restart half is asserted
    /// against `relaunch_argv` directly, which is where that path's argv is
    /// built and the only place a stored one is checked before it becomes an
    /// exec.
    #[tokio::test]
    async fn an_argv_naming_no_program_is_refused_on_every_launch_path() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let cwd = state.path().to_string_lossy().to_string();

        let raw = sup
            .create_session_without_overrides(&cwd, "''", None, 80, 24, None)
            .await
            .expect_err("a raw create must not launch a command line that names no program");
        assert_eq!(error_kind(&raw), ErrorKind::InvalidRequest);
        assert!(
            format!("{raw:#}").contains("names no program"),
            "the refusal must say what is wrong: {raw:#}"
        );

        // The retry path reads its argv from the crashed attempt's ROW,
        // which a build with looser rules could have written.
        let fingerprint = raw_fingerprint(&cwd, "''", None, None, None);
        sup.store
            .insert_session(
                StoredSession {
                    conversation_source: None,
                    id: "stranded".to_string(),
                    parent: None,
                    archived: false,
                    title: "stranded".to_string(),
                    created_at: now_unix(),
                    last_activity_at: now_unix(),
                    creation_seq: 0,
                    cwd: cwd.clone(),
                    invocation: "''".to_string(),
                    tmux_name: "fh-stranded".to_string(),
                    pane: String::new(),
                    outcome: LastOutcome::Launching,
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    resume_template: None,
                    canonical_cwd: Some(cwd.clone()),
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                    source_profile: None,
                },
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint: fingerprint.clone(),
                    dedup_scope: DedupScope::Permanent,
                }),
            )
            .await
            .expect("seed the crashed attempt");
        let retry = sup
            .create_session_without_overrides(
                &cwd,
                "''",
                None,
                80,
                24,
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint,
                    dedup_scope: DedupScope::Permanent,
                }),
            )
            .await
            .expect_err("a retry must not relaunch a recorded argv that names no program");
        assert!(
            format!("{retry:#}").contains("names no program"),
            "the retry's refusal must say what is wrong too: {retry:#}"
        );
        assert!(
            sup.sessions.lock().await.is_empty(),
            "and neither refusal may have launched anything"
        );

        // The restart path, at the point its argv is built from durable
        // columns. Both the fresh-launch invocation and a stored fallback
        // template can carry the shape.
        let fresh = relaunch_argv(
            RestartMode::Fresh,
            &snapshot_offering(RestartOffer::FreshOnly),
            "''",
        )
        .expect_err("a stored invocation that names no program is not a restart command");
        assert_eq!(error_kind(&fresh), ErrorKind::InvalidRequest);
        assert!(format!("{fresh:#}").contains("names no program"));

        let empty_template = SessionSnapshot {
            resume_template: Some(vec![String::new(), "--continue".to_string()]),
            ..snapshot_offering(RestartOffer::FallbackTemplate)
        };
        let fallback = relaunch_argv(RestartMode::FallbackTemplate, &empty_template, "agent")
            .expect_err("nor is a stored fallback template whose program slot is empty");
        assert!(format!("{fallback:#}").contains("names no program"));
    }

    /// A relaunch whose directory identity cannot be CHECKED is refused, and
    /// the launch it does allow is aimed at the path it verified.
    ///
    /// Both halves were the same hole. The check used to proceed with a
    /// warning when canonicalization failed, which inverts the threat model:
    /// the scenario being defended against is a path whose meaning changed,
    /// and a path that stats fine but will not resolve is exactly that shape
    /// — so the one input most deserving refusal was the one waved through.
    /// And a successful comparison used to be discarded, with the ORIGINAL
    /// symlinked path handed to tmux afterwards, leaving a
    /// time-of-check/time-of-use window several awaits and a subprocess
    /// wide.
    ///
    /// A session with no recorded identity is untouched by either rule,
    /// which is asserted too: rows predating the column would otherwise
    /// become unrestartable forever.
    #[tokio::test]
    async fn a_relaunch_refuses_a_directory_identity_it_cannot_confirm() {
        let state = StateDir::new();
        let target = state.path().join("target");
        std::fs::create_dir(&target).expect("target");
        let canonical = std::fs::canonicalize(&target)
            .expect("canonicalize")
            .to_string_lossy()
            .into_owned();
        let link = state.path().join("work");
        std::os::unix::fs::symlink(&target, &link).expect("symlink");
        let link_path = link.to_string_lossy().into_owned();

        assert_eq!(
            ensure_cwd_identity(&link_path, Some(&canonical))
                .await
                .expect("a matching identity is not a refusal"),
            Some(canonical.clone()),
            "the RESOLVED path comes back, so the launch can be aimed at it rather than at a \
             link that can still be repointed"
        );

        // The link now points at nothing: the directory itself is gone, so
        // canonicalization fails while the session's recorded identity says
        // exactly what it should have resolved to.
        std::fs::remove_dir(&target).expect("remove the link's target");
        let refusal = ensure_cwd_identity(&link_path, Some(&canonical))
            .await
            .expect_err("an unresolvable path cannot be confirmed, so it must not be launched in");
        assert_eq!(error_kind(&refusal), ErrorKind::InvalidRequest);
        let rendered = format!("{refusal:#}");
        assert!(
            rendered.contains(&canonical) && rendered.contains(&link_path),
            "the refusal must name both the path and what it was supposed to be: {rendered}"
        );

        assert_eq!(
            ensure_cwd_identity(&link_path, None)
                .await
                .expect("a session with no recorded identity has nothing to confirm"),
            None,
            "and it must stay restartable, or every row predating the column is stranded"
        );
    }

    /// A retry launches into the directory it VERIFIED, even when the
    /// symlink it was given is repointed after the check and before tmux
    /// sees it.
    ///
    /// This is the time-of-check/time-of-use window `ensure_cwd_identity`
    /// closes, forced open deterministically: the create-lifecycle seam runs
    /// at `AfterRecord`, which is after validation and before the launch's
    /// first external side effect, and repoints the link from there. A build
    /// that passed the original path through would put the agent in the
    /// attacker's directory — with the permissive flags agents are commonly
    /// launched with — while the session went on recording, and correlating
    /// against, the directory it thought it had checked.
    ///
    /// The agent itself reports where it landed, because that is the only
    /// answer that matters: a stub shim writes its own `pwd` and then sits
    /// there, so the assertion is on the working directory the launched
    /// process actually inherited rather than on anything the supervisor
    /// says about itself.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_retry_launches_into_the_directory_it_verified_not_a_repointed_link() {
        let state = StateDir::new();
        let original = state.path().join("original");
        let replacement = state.path().join("replacement");
        std::fs::create_dir(&original).expect("original target");
        std::fs::create_dir(&replacement).expect("replacement target");
        let canonical_original = std::fs::canonicalize(&original)
            .expect("canonicalize")
            .to_string_lossy()
            .into_owned();
        let link = state.path().join("work");
        std::os::unix::fs::symlink(&original, &link).expect("symlink");
        let cwd = link.to_string_lossy().into_owned();

        // A stub standing in for the launch shim: it records the directory
        // it was started in and then stays alive, so the pane survives long
        // enough for the create to confirm it.
        let landed = state.path().join("landed");
        let shim = state.path().join("stub-shim");
        std::fs::write(
            &shim,
            format!(
                "#!/bin/sh\npwd -P > {}\nexec sleep 300\n",
                landed.to_string_lossy()
            ),
        )
        .expect("write the stub shim");
        std::fs::set_permissions(&shim, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .expect("make the stub executable");

        let sup = {
            let link = link.clone();
            let replacement = replacement.clone();
            Supervisor::new_with_seams(
                state.path(),
                shim,
                SupervisorTimeouts::default(),
                SupervisorSeams {
                    // `/bin/sh` rather than the user's shell: the stub
                    // ignores the `-l -i -c` it is handed, and pinning the
                    // shell keeps a login profile out of the picture.
                    launch_shell: Some("/bin/sh".to_string()),
                    create_crash: Some(Arc::new(move |stage| {
                        if stage != CreateStage::AfterRecord {
                            return Ok(());
                        }
                        // Validation has run and nothing external exists
                        // yet: exactly the window a repoint would have to
                        // land in to win.
                        std::fs::remove_file(&link)?;
                        std::os::unix::fs::symlink(&replacement, &link)?;
                        Ok(())
                    })),
                    ..SupervisorSeams::default()
                },
            )
            .await
            .expect("supervisor")
        };

        let fingerprint = raw_fingerprint(&cwd, "agent", None, None, None);
        sup.store
            .insert_session(
                StoredSession {
                    conversation_source: None,
                    id: "stranded".to_string(),
                    parent: None,
                    archived: false,
                    title: "stranded".to_string(),
                    created_at: now_unix(),
                    last_activity_at: now_unix(),
                    creation_seq: 0,
                    cwd: cwd.clone(),
                    invocation: "agent".to_string(),
                    tmux_name: "fh-stranded".to_string(),
                    pane: String::new(),
                    outcome: LastOutcome::Launching,
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    resume_template: None,
                    canonical_cwd: Some(canonical_original.clone()),
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                    source_profile: None,
                },
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint: fingerprint.clone(),
                    dedup_scope: DedupScope::Permanent,
                }),
            )
            .await
            .expect("seed the crashed attempt");

        sup.create_session_without_overrides(
            &cwd,
            "agent",
            None,
            80,
            24,
            Some(IntentClaim {
                intent_key: "key".to_string(),
                fingerprint,
                dedup_scope: DedupScope::Permanent,
            }),
        )
        .await
        .expect("the retry launches: the repoint happened after the check, not before it");

        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let reported = loop {
            if let Ok(reported) = std::fs::read_to_string(&landed) {
                break reported;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the stub shim never reported the directory it started in"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        };
        assert_eq!(
            reported.trim(),
            canonical_original,
            "the agent must have started in the directory the identity check verified, not in \
             whatever the link was repointed at afterwards"
        );
    }

    /// Every shell tab receives the same spawn authority as its owning agent.
    #[tokio::test]
    async fn a_tab_environment_carries_the_complete_spawn_contract() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let env = sup
            .tab_environment("session-7", "private-token", "tab-9")
            .into_iter()
            .collect::<HashMap<_, _>>();
        assert_eq!(
            env.get(crate::launch::SESSION_ID_ENV_VAR)
                .map(String::as_str),
            Some("session-7")
        );
        assert_eq!(
            env.get(crate::launch::SESSION_TOKEN_ENV_VAR)
                .map(String::as_str),
            Some("private-token")
        );
        let socket = Supervisor::socket_path(&sup.state_dir);
        assert_eq!(
            env.get(crate::launch::SUPERVISOR_SOCK_ENV_VAR)
                .map(String::as_str),
            Some(socket.to_string_lossy().as_ref())
        );
        assert!(socket.is_absolute());
    }

    // -----------------------------------------------------------------
    // Conversation hook injection (plan §2.3).
    //
    // The decision function is pure, so the cases below drive
    // `with_hook_argv_using` directly rather than standing a supervisor up
    // per refusal. The last test in the section is the one that proves the
    // wiring: that the decision actually reaches the launch spec on disk
    // and the entry's tripwire flag.
    // -----------------------------------------------------------------

    /// A snapshot for `kind`, as the create path would have resolved one.
    ///
    /// Built through [`IntegrationSnapshot::resolve`] rather than by
    /// struct literal so these tests exercise the same snapshot shape a
    /// real session carries — including the derived resume template, whose
    /// absence would be a different (and silently passing) fixture.
    fn hook_snapshot(kind: AgentKind) -> IntegrationSnapshot {
        let argv0 = match kind {
            AgentKind::Claude => "claude",
            AgentKind::Codex => "codex",
            AgentKind::Generic => "agent",
        };
        IntegrationSnapshot::resolve(argv0, Some(kind), None).expect("a derived template resolves")
    }

    /// The exact tail each integrated kind expects for `exe`, straight
    /// from the integration itself.
    ///
    /// Asserting against this rather than against a literal `--settings`
    /// string is deliberate: this test is about WHETHER and WHERE the tail
    /// is appended, and the tail's own content is pinned by
    /// `agent_kind`'s quoting tests. A literal here would be a second
    /// copy of the vendor's flag syntax to keep in sync.
    fn expected_hook_tail(kind: AgentKind, exe: &str) -> Vec<String> {
        crate::agent_kind::integration_for(kind)
            .expect("an integrated kind")
            .hook_argv(exe)
    }

    /// The injection appends each integrated kind's OWN hook flags after
    /// the user's argv, and reports the launch as hooked.
    ///
    /// This is the whole mechanism plan §2.3 exists for: without the tail
    /// on the command line the agent never reports its conversation id, and
    /// `/clear` keeps silently breaking restart-resume. The two kinds are
    /// asserted separately because they inject completely different flags
    /// (Claude a `--settings` JSON, Codex a trust bypass plus two `-c`
    /// TOML values), and a bug that appended one kind's tail for both would
    /// otherwise pass.
    ///
    /// The user's own argv must survive UNCHANGED and in order: everything
    /// downstream — including the shim's `exec` — treats the leading
    /// elements as the invocation the operator asked for.
    #[test]
    fn with_hook_argv_appends_each_integrated_kinds_own_flags() {
        for kind in [AgentKind::Claude, AgentKind::Codex] {
            let argv = vec!["agent".to_string(), "--flag".to_string()];
            let (hooked_argv, hooked) = with_hook_argv_using(
                argv.clone(),
                &hook_snapshot(kind),
                &crate::agent_kind::AgentHooks::All,
                Some("/opt/farhelm"),
                "session-1",
            );
            assert!(hooked, "{kind:?}: an integrated kind must be hooked");
            let tail = expected_hook_tail(kind, "/opt/farhelm");
            assert_eq!(
                hooked_argv,
                [argv.clone(), tail].concat(),
                "{kind:?}: the user's argv must be preserved verbatim with the kind's own \
                 hook flags appended after it"
            );
        }
    }

    /// A `Generic` session is left alone, and SILENTLY.
    ///
    /// A kind with no integration has no hook to append — injecting vendor
    /// flags into an unknown command line would corrupt the launch — and
    /// un-integrated sessions are the ordinary case, so a log line per
    /// launch would drown the skips that actually mean identity capture
    /// degraded.
    ///
    /// Only the RETURN is asserted here; this suite captures no tracing
    /// output, so the silence half of the contract is documented at
    /// [`with_hook_argv_using`] rather than tested. The opt-out is
    /// nonetheless set to `None` — the value that would log `disabled by
    /// FARHELM_AGENT_HOOKS` if the policy check ran ahead of the
    /// integration check — so the fixture at least matches the case the
    /// ordering rule is about.
    #[test]
    fn with_hook_argv_leaves_a_generic_session_untouched() {
        let argv = vec!["agent".to_string()];
        let (result, hooked) = with_hook_argv_using(
            argv.clone(),
            &hook_snapshot(AgentKind::Generic),
            &crate::agent_kind::AgentHooks::None,
            Some("/opt/farhelm"),
            "session-1",
        );
        assert!(!hooked);
        assert_eq!(result, argv);
    }

    /// A bare `--` in the user's invocation suppresses injection (plan D4).
    ///
    /// Both vendors accept a trailing positional prompt, and `--` ends
    /// option parsing: flags appended past one are not configuration at
    /// all, they are prompt TEXT typed at the agent. Falling back to the
    /// record scan is the only safe answer, and it is asserted for both
    /// integrated kinds because the rule is about argv shape, not vendor.
    #[test]
    fn with_hook_argv_refuses_an_invocation_containing_a_bare_double_dash() {
        for kind in [AgentKind::Claude, AgentKind::Codex] {
            let argv = vec![
                "agent".to_string(),
                "--".to_string(),
                "write me a haiku".to_string(),
            ];
            let (result, hooked) = with_hook_argv_using(
                argv.clone(),
                &hook_snapshot(kind),
                &crate::agent_kind::AgentHooks::All,
                Some("/opt/farhelm"),
                "session-1",
            );
            assert!(!hooked, "{kind:?}: a bare -- must suppress injection");
            assert_eq!(result, argv, "{kind:?}: the argv must be untouched");
        }
        // A `--`-looking element that is not the separator is not the
        // separator: the check is equality, not a prefix match, or every
        // ordinary long flag would disable hooking.
        let argv = vec!["agent".to_string(), "--verbose".to_string()];
        let (_, hooked) = with_hook_argv_using(
            argv,
            &hook_snapshot(AgentKind::Claude),
            &crate::agent_kind::AgentHooks::All,
            Some("/opt/farhelm"),
            "session-1",
        );
        assert!(hooked, "an ordinary long flag must not read as a bare --");
    }

    /// A user's own `--settings` wins over ours, and ONLY on Claude
    /// (plan D3).
    ///
    /// Claude Code honours the LAST `--settings` flag only, so appending
    /// ours after the operator's would silently discard theirs — trading a
    /// configuration they chose for an identity improvement they did not
    /// ask for. Both spellings are refused, because `--settings x` and
    /// `--settings=x` are the same flag to the vendor and a check that saw
    /// only one would leak past the other.
    ///
    /// Codex is the control case and it matters: its injection uses no
    /// `--settings` at all, so a check that skipped for every kind would
    /// disable Codex hooking over a flag that cannot collide with
    /// anything.
    #[test]
    fn with_hook_argv_yields_to_a_users_own_settings_flag_on_claude_only() {
        for user_flag in [
            vec!["--settings".to_string(), "/my/settings.json".to_string()],
            vec!["--settings=/my/settings.json".to_string()],
        ] {
            let argv = [vec!["agent".to_string()], user_flag.clone()].concat();
            let (result, hooked) = with_hook_argv_using(
                argv.clone(),
                &hook_snapshot(AgentKind::Claude),
                &crate::agent_kind::AgentHooks::All,
                Some("/opt/farhelm"),
                "session-1",
            );
            assert!(
                !hooked,
                "claude must not append a second --settings over {user_flag:?}"
            );
            assert_eq!(result, argv);

            let (result, hooked) = with_hook_argv_using(
                argv.clone(),
                &hook_snapshot(AgentKind::Codex),
                &crate::agent_kind::AgentHooks::All,
                Some("/opt/farhelm"),
                "session-1",
            );
            assert!(
                hooked,
                "codex injects no --settings, so {user_flag:?} cannot collide with it"
            );
            assert_eq!(
                result,
                [argv, expected_hook_tail(AgentKind::Codex, "/opt/farhelm")].concat()
            );
        }
    }

    /// An invocation that already steers Codex's hook configuration is left
    /// alone, in every spelling that steering comes in.
    ///
    /// Two distinct hazards share this one skip. Appending a SECOND
    /// `--dangerously-bypass-hook-trust` puts a repeated flag in front of a
    /// vendor parser whose tolerance for that is unverified, and a rejected
    /// argument is not a degraded launch but a failed one — the single
    /// outcome injection may never cause. A `-c` override writing the
    /// `hooks.`/`features.hooks` tables, meanwhile, is a user configuring
    /// the exact tables our tail writes; whose value survives the merge is
    /// the vendor's business, and quietly appending ours makes farhelm a
    /// participant in a configuration argument it has no stake in.
    ///
    /// Both `-c value` and `-cvalue` are checked because they are one flag
    /// to the vendor, exactly as with Claude's two `--settings` spellings.
    /// The control cases are the point of the second half: an unrelated
    /// `-c` override still gets hooked (the rule is about the hook tables,
    /// not about configuring anything at all), and the whole rule is
    /// Codex-only — a Claude invocation carrying `-c hooks.…` means
    /// something else entirely, since Claude's injection uses no `-c`.
    #[test]
    fn with_hook_argv_yields_to_an_invocation_that_already_configures_codex_hooks() {
        for user_flags in [
            vec!["--dangerously-bypass-hook-trust".to_string()],
            vec!["-c".to_string(), "hooks.SessionStart=[]".to_string()],
            vec!["-chooks.SessionStart=[]".to_string()],
            vec!["-c".to_string(), "features.hooks=true".to_string()],
            vec!["-cfeatures.hooks=true".to_string()],
        ] {
            let argv = [vec!["agent".to_string()], user_flags.clone()].concat();
            let (result, hooked) = with_hook_argv_using(
                argv.clone(),
                &hook_snapshot(AgentKind::Codex),
                &crate::agent_kind::AgentHooks::All,
                Some("/opt/farhelm"),
                "session-1",
            );
            assert!(
                !hooked,
                "codex must not append its tail over {user_flags:?}"
            );
            assert_eq!(result, argv, "the argv must be untouched");

            // Claude is the control: the same elements carry no meaning
            // for its injection, so refusing there would cost hooking for
            // no reason at all.
            let (_, hooked) = with_hook_argv_using(
                argv,
                &hook_snapshot(AgentKind::Claude),
                &crate::agent_kind::AgentHooks::All,
                Some("/opt/farhelm"),
                "session-1",
            );
            assert!(
                hooked,
                "claude injects no -c and no bypass flag, so {user_flags:?} cannot collide"
            );
        }

        // An override that configures something else entirely is not this
        // rule's business — a check on `-c` alone would disable hooking for
        // every Codex user who has ever pinned a model.
        for unrelated in [
            vec!["-c".to_string(), "model=\"gpt-5-codex\"".to_string()],
            vec!["-cmodel=\"gpt-5-codex\"".to_string()],
        ] {
            let argv = [vec!["agent".to_string()], unrelated.clone()].concat();
            let (result, hooked) = with_hook_argv_using(
                argv.clone(),
                &hook_snapshot(AgentKind::Codex),
                &crate::agent_kind::AgentHooks::All,
                Some("/opt/farhelm"),
                "session-1",
            );
            assert!(hooked, "{unrelated:?} does not touch the hook tables");
            assert_eq!(
                result,
                [argv, expected_hook_tail(AgentKind::Codex, "/opt/farhelm")].concat()
            );
        }
    }

    /// The `FARHELM_AGENT_HOOKS` opt-out actually suppresses injection, in
    /// both of its narrowing forms (plan D5).
    ///
    /// This is the escape hatch the plan promises an operator who does not
    /// want the flags — Codex prints a trust-bypass warning on every launch
    /// (D2), and this is how they turn that off — so a value that parsed
    /// correctly but did not reach the launch would be a silent broken
    /// promise. `Only` is checked in BOTH directions from one policy value,
    /// because a bug that ignored the list entirely would still pass a test
    /// that only asserted the allowed kind.
    #[test]
    fn with_hook_argv_honours_the_agent_hooks_opt_out() {
        let argv = vec!["agent".to_string()];
        for kind in [AgentKind::Claude, AgentKind::Codex] {
            let (result, hooked) = with_hook_argv_using(
                argv.clone(),
                &hook_snapshot(kind),
                &crate::agent_kind::AgentHooks::None,
                Some("/opt/farhelm"),
                "session-1",
            );
            assert!(!hooked, "{kind:?}: `none` must hook nothing");
            assert_eq!(result, argv);
        }

        let codex_only = crate::agent_kind::AgentHooks::Only(vec![AgentKind::Codex]);
        let (result, hooked) = with_hook_argv_using(
            argv.clone(),
            &hook_snapshot(AgentKind::Codex),
            &codex_only,
            Some("/opt/farhelm"),
            "session-1",
        );
        assert!(hooked, "a listed kind must still be hooked");
        assert_eq!(
            result,
            [
                argv.clone(),
                expected_hook_tail(AgentKind::Codex, "/opt/farhelm")
            ]
            .concat()
        );
        let (result, hooked) = with_hook_argv_using(
            argv.clone(),
            &hook_snapshot(AgentKind::Claude),
            &codex_only,
            Some("/opt/farhelm"),
            "session-1",
        );
        assert!(!hooked, "a kind absent from the list must not be hooked");
        assert_eq!(result, argv);
    }

    /// A farhelm binary whose path is not UTF-8 launches un-hooked rather
    /// than failing the launch.
    ///
    /// The hook command has to be embedded in a vendor's shell-quoting
    /// syntax, which is `&str`-only, so a path that cannot be text has no
    /// hook to offer. The contract this pins is the DEGRADATION: capture
    /// falls back to the record scan and everything else about the launch
    /// is unchanged, because the shim itself is addressed by path and has
    /// never needed the name to be text.
    #[test]
    fn with_hook_argv_cannot_hook_a_non_utf8_farhelm_path() {
        let argv = vec!["agent".to_string()];
        let (result, hooked) = with_hook_argv_using(
            argv.clone(),
            &hook_snapshot(AgentKind::Claude),
            &crate::agent_kind::AgentHooks::All,
            None,
            "session-1",
        );
        assert!(!hooked);
        assert_eq!(result, argv);
    }

    /// The opt-out a supervisor was STARTED with reaches the launch
    /// decision — the one link `parse_agent_hooks`'s own tests cannot
    /// cover.
    ///
    /// `farhelm supervisor run` reads `FARHELM_AGENT_HOOKS` once and hands
    /// the parsed value to [`Supervisor::new_for_startup`]; everything
    /// after that consults the seam. A value that parsed correctly and then
    /// failed to land in [`SupervisorSeams`] would leave the escape hatch
    /// silently inoperative — the operator sets the variable, restarts, and
    /// Codex still prints its trust warning at every launch. Both
    /// directions are asserted from the same fixture, because a bug that
    /// dropped the field entirely would still pass the `None` half alone.
    ///
    /// It stops at [`Supervisor::with_hook_argv`] rather than driving a
    /// real create: the decision is what the constructor is on the hook
    /// for, and a create would drag tmux into a test about a struct field.
    #[tokio::test]
    async fn a_startup_opt_out_reaches_the_launch_decision() {
        let state = StateDir::new();
        let startup = |agent_hooks| SupervisorStartup {
            tmux_program: PathBuf::from(crate::tmux::DEFAULT_TMUX_PROGRAM),
            agent_hooks,
        };
        let argv = vec!["claude".to_string()];

        let opted_out =
            Supervisor::new_for_startup(state.path(), startup(crate::agent_kind::AgentHooks::None))
                .await
                .expect("supervisor");
        let (result, hooked) =
            opted_out.with_hook_argv(argv.clone(), &hook_snapshot(AgentKind::Claude), "session-1");
        assert!(
            !hooked,
            "a supervisor started with `none` must hook nothing"
        );
        assert_eq!(result, argv);

        // The same construction with the default value, to prove the
        // refusal above came from the startup value rather than from
        // anything else this constructor happens to resolve — its own
        // `current_exe`, for one, which is the libtest runner here.
        let default = Supervisor::new_for_startup(
            state.path(),
            startup(crate::agent_kind::AgentHooks::default()),
        )
        .await
        .expect("supervisor");
        let (_, hooked) =
            default.with_hook_argv(argv, &hook_snapshot(AgentKind::Claude), "session-1");
        assert!(hooked, "the default startup value must still hook");
    }

    /// End to end through a real create: a Claude session's launch spec on
    /// disk carries the hook flags, and its entry records the launch as
    /// hooked.
    ///
    /// The pure tests above prove the DECISION; this one proves the
    /// WIRING, which is the part with somewhere to go wrong — the snapshot
    /// has to be threaded into `spawn_agent`, the injection has to happen
    /// before the spec is serialized (the spec is what the shim execs, so
    /// flags added afterwards would never reach the agent), and the
    /// returned bool has to land on the entry the create publishes rather
    /// than being dropped.
    ///
    /// It reads the spec off disk rather than trusting an in-memory value
    /// for the same reason: the file IS the interface to the launch. The
    /// spec survives to be read because this supervisor's shim path is
    /// `dummy_exe`, so nothing ever consumes and unlinks it.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_claude_create_carries_the_hook_flags_into_its_launch_spec() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let info = sup
            .create_session_without_overrides("/", "claude", None, 80, 24, None)
            .await
            .expect("create");

        let spec_path = crate::launch::spec_path_for_launch(&sup.state_dir, &info.id, 0);
        let spec: LaunchSpec = serde_json::from_slice(
            &std::fs::read(&spec_path).expect("the launch spec must still be on disk"),
        )
        .expect("the launch spec must parse");
        let expected = expected_hook_tail(
            AgentKind::Claude,
            dummy_exe().to_str().expect("a UTF-8 dummy exe"),
        );
        assert_eq!(
            spec.argv,
            [vec!["claude".to_string()], expected].concat(),
            "the spec the shim execs must be the user's invocation followed by the hook flags"
        );

        assert!(
            sup.sessions
                .lock()
                .await
                .get(&info.id)
                .expect("the created session must be on the map")
                .hooked
                .load(std::sync::atomic::Ordering::Relaxed),
            "a hooked launch must raise the entry flag the liveness tripwire reads"
        );
    }
}
