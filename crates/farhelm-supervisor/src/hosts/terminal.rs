//! A host terminal's runtime: the supervisor's side of one terminal that runs
//! in a terminal host, as one task that owns the host's connection and the
//! terminal's emulator.
//!
//! A terminal host (`crates/terminal-host/`) keeps the pty, the program and a
//! journal of raw output, and interprets nothing. Everything a tmux pane
//! does for a terminal beyond that happens here: the program's output is fed
//! to the supervisor's emulator (`farhelm_terminal_emulator`), its terminal
//! queries are answered through the host's `Input`, its synchronized updates
//! are flushed on their deadline, and the child's life is followed from the
//! host's records. [`HostTerminal`] is the handle the rest of the supervisor
//! holds; [`HostTerminal::adopt`] connects to a host and starts the task.
//!
//! - **One owner.** Every connection to a host takes it over (the client's
//!   module docs, the plan's D-11), so a second runtime for the same host
//!   would cut the first one off, and each would take it back in turn.
//!   Exactly one runtime per host may exist in a supervisor. When its
//!   connection ends anyway it connects again to learn what happened: a host
//!   that is gone or
//!   unavailable ends the runtime ([`Liveness::Gone`],
//!   [`Liveness::Unavailable`]); a host that answers is read again from the
//!   position already processed, so nothing is fed twice. A connection that
//!   keeps ending ([`RECONNECT_LIMIT`] times within [`RECONNECT_WINDOW`])
//!   makes the terminal unavailable rather than fight whoever keeps taking
//!   it.
//! - **Adoption** reads the host from the start (`Read(0,0)`): the stored
//!   snapshot record, if any, restores the emulator, and the tail after it is
//!   fed. Which of that is replay depends on how the runtime came to the host
//!   ([`Adoption`]). A host this supervisor has just spawned has had no other
//!   reader, so all of its output is live. For a host that ran before,
//!   output journaled before adoption has already been seen by an earlier
//!   supervisor, and is fed without side effects; [`REPLAYED`] is the one
//!   place that decides what replay may do, and the replay policy that
//!   answers queries the program asked while no supervisor was attached
//!   (the plan's D-7, split at the host's `delivered_end`) replaces it. A replay ends
//!   with any open synchronized update flushed, so that an update the replay
//!   opened is not drawn by the first live feed with live side effects.
//! - **The clock is the caller's** ([`Clock`]). The emulator never reads the
//!   time itself: each feed is stamped with the clock's `now`, and the task
//!   wakes at the open update's deadline on the same clock. Production uses
//!   tokio's clock; tests advance a manual one instead of waiting 150 ms.
//! - **Answers** to the program's queries are written to it as `Input`, in
//!   the order it asked, within the client's input window. Notices for the
//!   user (bells, clipboard writes, titles) are not delivered: a viewer's
//!   grid stream will carry them, and reads the title from the emulator.
//! - **Input** from callers is sent in order with the answers, each `Input`
//!   whole, and is never resent: input whose connection ended before the
//!   host acknowledged it is reported as [`InputFate::Unknown`], since some
//!   of it may have reached the program. Nothing is written after such a
//!   hole that belongs before it, by an input epoch: every input carries
//!   the epoch it was asked for in, a lost connection that lost caller
//!   input moves the epoch on (once, before any of its input is reported
//!   unknown), and an input of an older epoch is reported
//!   [`InputFate::NotSent`] wherever it still is (waiting for the window,
//!   queued in the task's channel, or still waiting for room there). What
//!   the epoch is depends on how a caller sends:
//!   - [`HostTerminal::send_input`] stamps each input with the epoch at the
//!     moment it is called. One-shot input (a key, a prompt line) needs no
//!     more: an input asked for after a loss is written, and one asked for
//!     before it is not, even if it was still waiting to be queued. A
//!     caller that sends several parts this way and does not check each
//!     receipt before the next can have a later part written after an
//!     earlier part was lost.
//!   - An [`InputSequence`] ([`HostTerminal::input_sequence`]) fixes its
//!     epoch when it is made, and stamps every part sent through it with
//!     that epoch. So once any input is lost with a connection, every part
//!     of the sequence asked for afterwards is not sent either, whether or
//!     not the caller looked at a receipt: a paste sent through one never
//!     arrives as a suffix with its middle missing. A caller starts a new
//!     sequence for the next paste.
//!
//!   Answers are not dropped this way: each is complete in itself. While the
//!   window is full, the task stops taking input from callers, whose sends
//!   then wait; it keeps reading the host and acting on resizes and
//!   shutdown, so a program that stops reading its terminal holds back only
//!   input.
//! - **Resizes** ([`HostTerminal::resize`]) are coalesced: the latest size
//!   asked for is sent, once. On a new connection it is sent again whenever
//!   the host's `Info` reports another size, since a resize written to a
//!   connection that then ended may never have been acted on (a host drops
//!   what a displaced client sent and it had not acted on yet). A repeated
//!   resize is harmless; a lost one leaves the program at the old size. The
//!   emulator takes a new size only from the host's resize record, at its
//!   offset in the output.
//! - **Liveness** ([`Liveness`]) follows the host's records, and only moves
//!   forward: running, then exited, then gone or unavailable. A terminal is
//!   exited only once the stream's exit record has been processed, which the
//!   host sends after the child's drained output, so whoever acts on the
//!   exit reads the final screen. The child's root
//!   ([`HostStatus::child_root`]) is withdrawn sooner: it is the child's
//!   pid only while the host reports the child alive and the process under
//!   that pid still has the start identity pinned at adoption, re-read every
//!   time a root is asked for, since the host reaps the child some time
//!   before its exit record arrives. Every change to exited, gone or
//!   unavailable is also sent to a [`LivenessNotifier`], if one was given,
//!   so that one task can wait for the exits of every host terminal at
//!   once.
//! - **Adapters** map a host terminal's state onto the types the supervisor
//!   already decides with for tmux panes: [`HostStatus::probe`] gives a
//!   [`PaneProbe`], and [`HostStatus::exit_code`] and `HostStatus::anchor`
//!   the exit code and hook anchor a pane would give. Status evidence is the
//!   exception: `service::status::PaneEvidence` borrows a tmux `PaneState`,
//!   which a host terminal could only supply by fabricating its tmux fields
//!   (session name, window, ordinals, markers), so [`HostStatus::evidence`]
//!   gives the same three classes with what their consumers read
//!   ([`HostEvidence`]), as the interim seam until `PaneEvidence` itself
//!   carries owned liveness facts.
//! - **Unavailable is final for a runtime, not for a host.** A runtime that
//!   ends unavailable leaves its host alone and never tries it again. Some
//!   reasons are lasting (a protocol or snapshot format this build does not
//!   know); others can pass (a connect that timed out on a loaded machine,
//!   a host that was briefly stopped, a child that could not be pinned, a
//!   connection that kept ending). Whoever owns the runtimes adopts such a
//!   host again, on a backoff, with a new runtime.
//!
//! Four things later steps build on are here as seams, and say so where
//! they are: where a snapshot is taken (`Runtime::on_output` feeds each read
//! whole, and a clean boundary within it, with the processed
//! [`HostStatus::position`], is a checkpoint's place); where the host's
//! `delivered_end` will decide what replay answers ([`REPLAYED`] and
//! `Replay`); a gap in the journal or a snapshot that needs a redraw, after
//! which the program will be asked to redraw (`Runtime::on_gap`,
//! `Runtime::on_snapshot`); and the notices for the user, which `Answers`
//! drops and a viewer's grid stream will deliver. None of them is done yet:
//! no snapshot is taken, replay answers nothing, a gap is logged and
//! continued from, and notices go nowhere.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex, PoisonError, Weak};
use std::time::{Duration, Instant};

use farhelm_terminal_emulator::{Effects, Emulator, FeedMode, Modes, Notice, SnapshotError};
use terminal_host::protocol::{ChildExit, Info, MAX_INPUT, Position, Size};
use tokio::sync::{mpsc, oneshot, watch};
use tracing::{info, warn};

use super::client::{
    ClientDeadlines, ClientError, ConnectError, Gone, HostConnection, HostEvent, InputCounts,
    InputTicket, Unavailable,
};
use super::process::HostProcess;
use crate::procs::{self, PaneAnchor, ProcessIdentity, ProcessState};
use crate::tmux::{PaneProbe, PaneProcess};

// ---- Limits --------------------------------------------------------------------

/// Lines of history each host terminal keeps by default: the same as a tmux
/// pane's `history-limit`, so a terminal keeps as much scrollback in a host
/// as it does in tmux.
pub const DEFAULT_SCROLLBACK_LINES: usize = crate::tmux::HISTORY_LIMIT as usize;

/// How many `send_input` calls may wait in the task's queue. Each holds at
/// most `MAX_INPUT` bytes, so this bounds what callers can queue to 1 MiB
/// beyond the input window; past it their sends wait.
const INPUT_QUEUE: usize = 16;

/// The most answer bytes waiting for room in the input window. A program
/// that floods queries and never reads its terminal fills the window, and
/// answers past this are dropped rather than held: the program is not
/// reading them, and holding them would let one program grow the shared
/// supervisor without bound.
pub const MAX_PENDING_ANSWERS: usize = 64 * 1024;

/// How many times adoption reads the child's process while the host says it
/// runs, before giving up on pinning it. The read fails only in the moment
/// between the child's exit and the host reaping it (a zombie has no start
/// identity left), which the host ends at once on `SIGCHLD`.
const PIN_ATTEMPTS: usize = 10;

/// Pause between attempts to pin the child.
const PIN_RETRY: Duration = Duration::from_millis(20);

/// How many reconnects within [`RECONNECT_WINDOW`] a runtime makes before it
/// gives the host up as unavailable. A connection ends on its own only when
/// the host ends, stops answering, or is taken over; a host that answers
/// again and again only to be taken over each time has another owner, and
/// fighting it would cut off both.
pub const RECONNECT_LIMIT: usize = 3;

/// See [`RECONNECT_LIMIT`].
pub const RECONNECT_WINDOW: Duration = Duration::from_secs(60);

/// How often a shutdown re-reads the host's process while waiting for it to
/// end.
const END_POLL: Duration = Duration::from_millis(20);

/// What replayed output may do: the side effects of output an earlier
/// supervisor already received. Nothing is answered or reported.
///
/// This is the seam for the replay policy (D-7). It treats everything
/// journaled before adoption as seen, the conservative half of that policy:
/// a query the program asked while no supervisor was attached goes
/// unanswered (the program waits out its own timeout), and nothing is ever
/// answered twice (a second answer would arrive at a shell prompt as typed
/// garbage). Splitting the replay at the host's `delivered_end`, to answer
/// what no supervisor saw as [`FeedMode::ReplayUnseen`], is the policy's
/// other half.
pub const REPLAYED: FeedMode = FeedMode::ReplaySeen;

// ---- The clock -----------------------------------------------------------------

/// The time a host terminal's emulator runs on: what each feed is stamped
/// with, and what the task waits on for an open synchronized update's
/// deadline.
#[derive(Debug, Clone, Default)]
pub enum Clock {
    /// tokio's clock, which is the system's outside tests.
    #[default]
    System,
    /// A clock that moves only when a test advances it.
    #[cfg(any(test, feature = "test-seams"))]
    Manual(ManualClock),
}

impl Clock {
    /// The time now.
    pub fn now(&self) -> Instant {
        match self {
            Clock::System => tokio::time::Instant::now().into_std(),
            #[cfg(any(test, feature = "test-seams"))]
            Clock::Manual(clock) => clock.now(),
        }
    }

    /// Wait until the clock reads `deadline` or later.
    async fn sleep_until(&self, deadline: Instant) {
        match self {
            Clock::System => tokio::time::sleep_until(deadline.into()).await,
            #[cfg(any(test, feature = "test-seams"))]
            Clock::Manual(clock) => clock.wait_until(deadline).await,
        }
    }
}

/// A clock for tests: it starts at the real time it was made and moves only
/// by [`ManualClock::advance`], so a test steps a terminal past a deadline
/// instead of sleeping until it. Clones share one time.
#[cfg(any(test, feature = "test-seams"))]
#[derive(Debug, Clone)]
pub struct ManualClock {
    now: Arc<watch::Sender<Instant>>,
}

#[cfg(any(test, feature = "test-seams"))]
impl Default for ManualClock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(test, feature = "test-seams"))]
impl ManualClock {
    /// A clock reading the real time now, until it is advanced.
    pub fn new() -> Self {
        Self {
            now: Arc::new(watch::Sender::new(Instant::now())),
        }
    }

    /// The time it reads.
    pub fn now(&self) -> Instant {
        *self.now.borrow()
    }

    /// Move the clock forward by `by`, waking whatever waits for a time it
    /// now reaches.
    pub fn advance(&self, by: Duration) {
        self.now.send_modify(|now| *now += by);
    }

    async fn wait_until(&self, deadline: Instant) {
        let mut now = self.now.subscribe();
        // The sender lives as long as `self`, so the wait ends only by
        // reaching the deadline.
        let _ = now.wait_for(|now| *now >= deadline).await;
    }
}

// ---- Liveness and status ---------------------------------------------------------

/// The child process a kill or a process-tree sweep may start from: its pid,
/// pinned to its kernel start identity, so that a pid reused after the child
/// was reaped is never mistaken for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProcessRoot {
    pub pid: u32,
    pub start: u64,
}

/// How a host terminal's program is, as far as its host says.
///
/// It only moves forward: `Running` with a pinned child, `Running` without
/// one, `Exited`, and then `Gone` or `Unavailable`, which end the runtime
/// (see `Liveness::rank`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Liveness {
    /// The exit record has not been processed, so as far as the terminal's
    /// screen goes, the program runs.
    ///
    /// `child` is the child's process as pinned at adoption, from which
    /// Stop's and Delete's kill and sweep start, but only through
    /// [`HostStatus::child_root`], which checks that it still runs. It is
    /// `None` once a host's `Info` said its child was reaped (at adoption or
    /// on a reconnect) and its exit record, which follows the child's
    /// drained output, has not been processed yet.
    Running { child: Option<ProcessRoot> },
    /// The stream's exit record has been processed: the host reaped its
    /// child, which ended this way, and every output before the exit has
    /// been fed. The host keeps running and keeps the final screen, until it
    /// is shut down.
    Exited { exit: ChildExit },
    /// The host has ended: its socket is missing or refuses every
    /// connection, or it confirmed a shutdown.
    Gone,
    /// The host may still run, but this runtime cannot use it, and leaves
    /// it alone. `peer` is its process when one is known, for the verified
    /// SIGTERM (`super::process::terminate_verified`) that is the only way
    /// to end it.
    Unavailable {
        reason: Unavailability,
        peer: Option<HostProcess>,
    },
}

impl Liveness {
    /// How far along the one-way order this is. A runtime only ever moves to
    /// a higher rank: a root once withdrawn never comes back, an exit is
    /// never undone, and an ended runtime stays ended.
    fn rank(&self) -> u8 {
        match self {
            Liveness::Running { child: Some(_) } => 0,
            Liveness::Running { child: None } => 1,
            Liveness::Exited { .. } => 2,
            Liveness::Gone | Liveness::Unavailable { .. } => 3,
        }
    }

    /// Whether this is announced to a [`LivenessNotifier`].
    fn is_announced(&self) -> bool {
        self.rank() >= 2
    }
}

/// Why a host terminal is unavailable.
///
/// Some reasons can pass, and the host should be adopted again later (see
/// the module docs, "Unavailable is final for a runtime, not for a host"):
/// `Connect` with a timeout, a host no longer taking connections or one
/// closing them before it answered, `Busy`, `Request`, `ChildUnpinned` and
/// `Unstable`. The others say this build cannot use the host at all.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Unavailability {
    /// Connecting found a host this supervisor cannot use.
    #[error(transparent)]
    Connect(Unavailable),
    /// The host stayed busy with another client.
    #[error("the terminal host stayed busy")]
    Busy,
    /// A request the adoption needed failed.
    #[error("the terminal host did not answer a request: {0}")]
    Request(ClientError),
    /// The host's snapshot record does not restore: a format this build
    /// does not know (a newer supervisor's, after a downgrade), or a blob
    /// that does not decode. The terminal's state is never guessed at.
    #[error("the terminal host's snapshot cannot be restored: {0}")]
    Snapshot(SnapshotError),
    /// The host says its child runs, but the child's process could not be
    /// read, so there is no root to kill it by.
    #[error("the terminal host's child {pid} could not be read")]
    ChildUnpinned { pid: u32 },
    /// The connection kept ending ([`RECONNECT_LIMIT`]).
    #[error("the connection to the terminal host kept ending")]
    Unstable,
    /// A connect found another process than the adopted host serving its
    /// socket: whatever the runtime held about the host's journal is not
    /// that process's. Socket names are random, so this means something
    /// unusual happened to the host. The process found is left alone and
    /// never named; the liveness's peer is the adopted host's process, which
    /// may have ended.
    #[error("another process now serves the terminal host's socket")]
    Replaced,
    /// The host acknowledged a shutdown, or did not, but its process did
    /// not end in time.
    #[error("the terminal host did not end after Shutdown")]
    ShutdownUnconfirmed,
}

/// Everything a host terminal's handle can read without asking its task:
/// its liveness and how far its emulator has got.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostStatus {
    pub liveness: Liveness,
    /// The child's pid as the host reports it. Once the child has exited,
    /// the pid may belong to another process.
    pub child_pid: u32,
    /// The child's start identity, when it was read while the host still
    /// had the child unreaped: what tells the child from a process that
    /// reuses its pid, re-checked for every root and kept after the exit for
    /// attribution (`HostStatus::anchor`). `None` for a child that exited
    /// before the runtime could read it.
    pub child_start: Option<u64>,
    /// The position in the host's journal the emulator has processed: the
    /// position a snapshot of it now would carry, and where a new
    /// connection resumes reading.
    pub position: Position,
    /// Counts every change of the emulator's state (output, resizes,
    /// flushes, restores), so that a reader can wait for the screen to
    /// change.
    pub revision: u64,
    /// Whether the emulator has processed everything the host had journaled
    /// when the runtime adopted it, its stored snapshot included: the host
    /// is asked for `Info` right after `Read`, and every record it sent
    /// before that reply (the snapshot record among them) must have been
    /// processed too, so a host whose only state is an initial snapshot is
    /// not caught up until the snapshot is restored.
    pub caught_up: bool,
}

/// A host terminal's state in the three classes the supervisor's status
/// evidence has for a tmux pane (`service::status::PaneEvidence`), with
/// what each class carries there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostEvidence {
    /// The host answers for its child: whether it has exited and its exit
    /// code, as `PaneState::dead` and `PaneState::exit_code` give them for a
    /// pane.
    Owned { dead: bool, exit_code: Option<i32> },
    /// The host has ended, and with it the terminal.
    Absent,
    /// The host may run but cannot be read (D-16): neither liveness nor an
    /// exit may be concluded from it, as for a pane under a name no session
    /// answers to.
    Unattributed,
}

impl HostStatus {
    /// The root that a kill or sweep of the child's process tree may start
    /// from: the child, while the host reports it alive and the process
    /// under its pid still has the start identity pinned at adoption, which
    /// this reads from the process table each time. Never once the child is
    /// reaped, whose pid may since belong to anything, nor while a zombie,
    /// which needs no kill and whose children have been reparented; and not
    /// when the process table cannot be read, since a root must be certain.
    pub fn child_root(&self) -> Option<ProcessRoot> {
        match self.pinned_child() {
            Some(Ok(root)) => Some(root),
            Some(Err(_)) | None => None,
        }
    }

    /// The pinned child checked against the process table: `Ok` while it
    /// still runs, `Err` with why the check could not be made, `None` when
    /// there is no running child to check (none pinned, reaped, or no
    /// longer the same process).
    fn pinned_child(&self) -> Option<Result<ProcessRoot, String>> {
        let Liveness::Running { child: Some(root) } = &self.liveness else {
            return None;
        };
        match procs::read_process(root.pid) {
            Ok(Some((_, start, ProcessState::Running))) if start == root.start => Some(Ok(*root)),
            Ok(_) => None,
            Err(why) => Some(Err(why)),
        }
    }

    /// What `TmuxDriver::pane_process` would answer for a pane in this
    /// state, for the verbs that decide on that answer (Stop, Restart,
    /// Delete, prompt delivery, the tab reap).
    ///
    /// A child that [`Self::child_root`] gives is an owned, live process. A
    /// child without a root (reaped, or the exit recorded) is an owned, dead
    /// one with no pid, so nothing signals a reaped pid. This can say dead
    /// a little before [`Self::evidence`] does, which waits for the exit
    /// record so that the exit is recorded with the final screen. A host
    /// that has ended is gone. An unavailable host, or a process table that
    /// cannot be read, is an error: the verbs treat an error as "could not
    /// tell" and refuse. It has no `ForeignOwner` answer: a host terminal's
    /// owner is its tag, not a name that could be recycled.
    ///
    /// `PaneProcess` carries a pid only. A caller that kills or sweeps from
    /// it must take the root with its start identity from
    /// [`Self::child_root`] rather than read an identity from the bare pid
    /// again, which could be a process that took the pid since.
    pub fn probe(&self) -> anyhow::Result<PaneProbe> {
        match &self.liveness {
            Liveness::Running { .. } => match self.pinned_child() {
                Some(Ok(root)) => Ok(PaneProbe::Owned(PaneProcess {
                    pid: Some(root.pid),
                    dead: false,
                })),
                Some(Err(why)) => Err(anyhow::anyhow!(
                    "the terminal host's child could not be checked: {why}"
                )),
                None => Ok(PaneProbe::Owned(PaneProcess {
                    pid: None,
                    dead: true,
                })),
            },
            Liveness::Exited { .. } => Ok(PaneProbe::Owned(PaneProcess {
                pid: None,
                dead: true,
            })),
            Liveness::Gone => Ok(PaneProbe::Gone),
            Liveness::Unavailable { reason, .. } => Err(anyhow::anyhow!(
                "the terminal host is unavailable: {reason}"
            )),
        }
    }

    /// This state as status evidence ([`HostEvidence`]): running until the
    /// exit record has been processed, so that no exit is recorded before
    /// the output it follows has reached the screen.
    pub fn evidence(&self) -> HostEvidence {
        match &self.liveness {
            Liveness::Running { .. } => HostEvidence::Owned {
                dead: false,
                exit_code: None,
            },
            Liveness::Exited { exit } => HostEvidence::Owned {
                dead: true,
                exit_code: exit_code(*exit),
            },
            Liveness::Gone => HostEvidence::Absent,
            Liveness::Unavailable { .. } => HostEvidence::Unattributed,
        }
    }

    /// The child's exit code, as a dead pane reports it: the status it
    /// exited with, `None` while it runs and for a death by signal (which a
    /// pane cannot express as a code either).
    pub fn exit_code(&self) -> Option<i32> {
        match &self.liveness {
            Liveness::Exited { exit } => exit_code(*exit),
            Liveness::Running { .. } | Liveness::Gone | Liveness::Unavailable { .. } => None,
        }
    }

    /// Where a hook report's recorded process chain must end to be
    /// attributed to this terminal: the child, as `procs::PaneAnchor`
    /// describes a pane's process. Kept after the child exits, like a dead
    /// pane's, because a report written before the exit may still be
    /// waiting to be applied; with the start identity read while the child
    /// lived, so a pid reused since cannot match. A child that had already
    /// exited when the runtime adopted its host was never pinned, and is
    /// anchored by its pid alone, as tmux anchors a dead pane. `None`
    /// without a child pid, and for a host that cannot be read.
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "the hook anchor's switch point is its first caller"
        )
    )]
    pub(crate) fn anchor(&self) -> Option<PaneAnchor> {
        match &self.liveness {
            Liveness::Unavailable { .. } => None,
            Liveness::Running { .. } | Liveness::Exited { .. } | Liveness::Gone => {
                (self.child_pid > 1).then_some(PaneAnchor {
                    pid: self.child_pid,
                    start: self.child_start,
                })
            }
        }
    }
}

/// A child exit as a pane's exit code (see [`HostStatus::exit_code`]).
fn exit_code(exit: ChildExit) -> Option<i32> {
    match exit {
        ChildExit::Code(code) => Some(code),
        ChildExit::Signal(_) => None,
    }
}

// ---- The notifier ----------------------------------------------------------------

/// Where host terminals announce becoming exited, gone or unavailable: one
/// channel shared by every runtime, so that a single task (the ticker, the
/// tab reap) learns of every exit without polling each terminal. An exit is
/// announced once its record has been processed, so the screen it leaves is
/// final.
///
/// Unbounded, and bounded all the same: a runtime sends at most twice
/// (exited, then gone or unavailable).
#[derive(Debug, Clone)]
pub struct LivenessNotifier(mpsc::UnboundedSender<LivenessChange>);

/// One host terminal's new liveness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LivenessChange {
    /// The host's socket, which names it.
    pub socket: PathBuf,
    pub liveness: Liveness,
}

impl LivenessNotifier {
    /// A notifier and the receiver its changes arrive at.
    pub fn channel() -> (Self, mpsc::UnboundedReceiver<LivenessChange>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        (Self(sender), receiver)
    }
}

// ---- Adoption --------------------------------------------------------------------

/// How a runtime comes to its host, which decides what of the host's journal
/// is replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adoption {
    /// This supervisor has just spawned the host and nothing else has read
    /// it: all of its output is live, including what it wrote before the
    /// runtime connected, so the program's first queries are answered.
    ///
    /// The host's `delivered_end` checks the claim: a host that has sent
    /// output to some client before was read by someone, and its output up
    /// to there is replay all the same (a retried create that adopts a host
    /// an earlier supervisor already read, say).
    Fresh,
    /// The host ran before this runtime: what it journaled before adoption
    /// is replay ([`REPLAYED`]), and only what follows is live.
    Recovered,
}

/// What a runtime is made with.
#[derive(Debug, Clone)]
pub struct HostTerminalOptions {
    /// Lines of history the emulator keeps.
    pub scrollback_lines: usize,
    /// The connection's deadlines.
    pub deadlines: ClientDeadlines,
    /// The emulator's clock.
    pub clock: Clock,
    /// Where to announce liveness changes.
    pub notifier: Option<LivenessNotifier>,
}

impl Default for HostTerminalOptions {
    fn default() -> Self {
        Self {
            scrollback_lines: DEFAULT_SCROLLBACK_LINES,
            deadlines: ClientDeadlines::default(),
            clock: Clock::default(),
            notifier: None,
        }
    }
}

/// Why no runtime was started for a host.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AdoptError {
    /// Nothing listens there any more.
    #[error("no terminal host: {0}")]
    Gone(Gone),
    /// Something answers that cannot be used. It is left alone; `peer`, when
    /// known, is what may end it.
    #[error("the terminal host is unavailable: {reason}")]
    Unavailable {
        reason: Unavailability,
        peer: Option<HostProcess>,
    },
}

impl From<ConnectError> for AdoptError {
    fn from(error: ConnectError) -> Self {
        match error {
            ConnectError::Gone(gone) => AdoptError::Gone(gone),
            ConnectError::Busy { peer } => AdoptError::Unavailable {
                reason: Unavailability::Busy,
                peer,
            },
            ConnectError::Unavailable { reason, peer } => AdoptError::Unavailable {
                reason: Unavailability::Connect(reason),
                peer,
            },
        }
    }
}

// ---- The handle ------------------------------------------------------------------

/// A terminal that runs in a terminal host, as the rest of the supervisor
/// holds it. Clones share one runtime; the runtime's task ends when the last
/// clone is dropped (closing its connection and leaving the host running),
/// or when the host ends.
///
/// Two handles are equal when they share one runtime: a new runtime for the
/// same host is a new terminal handle, even though it names the same socket.
#[derive(Clone)]
pub struct HostTerminal {
    inner: Arc<Inner>,
}

/// What every clone of one handle shares. Dropping it drops the senders,
/// which is how the task learns that nobody holds the terminal any more.
struct Inner {
    socket: PathBuf,
    emulator: Arc<StdMutex<Emulator>>,
    status: watch::Receiver<HostStatus>,
    input: mpsc::Sender<InputRequest>,
    control: mpsc::Sender<Control>,
    size: watch::Sender<Option<Size>>,
    /// The input epoch, shared with the runtime: how many connections ended
    /// with caller input lost. A request carries the epoch it was asked for
    /// in (at the call, or the sequence's), stamped before it waited for room
    /// in the queue, and one from an older epoch is not sent (module docs,
    /// "Input"): a caller blocked on a full queue when input was lost is
    /// behind the hole too.
    epoch: Arc<AtomicU64>,
}

impl PartialEq for HostTerminal {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for HostTerminal {}

impl std::fmt::Debug for HostTerminal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostTerminal")
            .field("socket", &self.inner.socket)
            .field("liveness", &self.inner.status.borrow().liveness)
            .finish_non_exhaustive()
    }
}

/// What became of one [`HostTerminal::send_input`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFate {
    /// Every byte reached the pty.
    Written,
    /// Some or all of it did not reach the pty: nothing reads the terminal
    /// any more, or a write to it failed (`crates/terminal-host/SPEC.md`,
    /// "Input"). Bytes written before the rest was discarded did reach it.
    Discarded,
    /// It was sent, and the connection ended before the host said what
    /// became of it. Some of it may have reached the program, so it is not
    /// sent again, and must not be by the caller either.
    Unknown,
    /// It was never sent: the terminal ended first.
    NotSent,
}

/// The fate of one input, once it is known.
#[derive(Debug)]
pub struct InputReceipt(oneshot::Receiver<InputFate>);

/// The parts of one multi-part input (a paste), from
/// [`HostTerminal::input_sequence`]: every part carries the input epoch the
/// sequence was made in, so once any input is lost with a connection, the
/// parts asked for afterwards are [`InputFate::NotSent`] rather than written
/// after the hole, whether or not the caller waited for a receipt in
/// between. A sequence is for one paste; the next one starts a new sequence.
///
/// It holds the terminal weakly: a sequence kept after every
/// [`HostTerminal`] handle is gone does not keep the runtime (and its
/// connection) alive, and its parts are then refused as ended.
#[derive(Debug, Clone)]
pub struct InputSequence {
    inner: Weak<Inner>,
    epoch: u64,
}

impl InputSequence {
    /// Queue one part, as [`HostTerminal::send_input`] does, in this
    /// sequence's epoch.
    pub async fn send(&self, data: &[u8]) -> Result<InputReceipt, InputRefused> {
        let inner = self.inner.upgrade().ok_or(InputRefused::Ended)?;
        HostTerminal { inner }.send_in_epoch(data, self.epoch).await
    }
}

impl InputReceipt {
    /// Wait for the input's fate. A runtime that ended without saying is
    /// [`InputFate::Unknown`], which never invites a resend.
    ///
    /// The wait has no bound of its own: an input is resolved when the host
    /// acknowledges it, and a program that stops reading its terminal over a
    /// healthy connection holds its input unacknowledged for as long as it
    /// does. A caller that cannot wait that long bounds the wait itself; to
    /// that caller, an input whose wait ran out is unknown.
    pub async fn fate(self) -> InputFate {
        self.0.await.unwrap_or(InputFate::Unknown)
    }
}

/// Why [`HostTerminal::send_input`] took nothing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InputRefused {
    /// The input is empty or longer than `MAX_INPUT`. A caller splits
    /// longer input itself, never inside a key's encoding.
    #[error("an input must be 1 to MAX_INPUT bytes")]
    Invalid,
    /// The terminal's runtime has ended.
    #[error("the terminal has ended")]
    Ended,
}

/// Why [`HostTerminal::resize`] took nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ResizeRefused {
    /// A size outside what a host accepts (1 to `MAX_DIMENSION` each way).
    #[error("a terminal size must be 1 to MAX_DIMENSION each way")]
    Invalid,
    /// The terminal's runtime has ended.
    #[error("the terminal has ended")]
    Ended,
}

/// How a [`HostTerminal::shutdown`] went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShutdownReport {
    /// Whether the host acknowledged the shutdown.
    pub goodbye: bool,
    /// Whether the host was confirmed ended: its process gone (or, without
    /// a known process, its socket file). When it was not, `peer` is what
    /// may still end it.
    pub ended: bool,
    /// The host's process, from the connection.
    pub peer: Option<HostProcess>,
}

/// One caller's input, on its way to the task.
struct InputRequest {
    data: Vec<u8>,
    fate: oneshot::Sender<InputFate>,
    /// The input epoch when `send_input` was called (`Inner::epoch`).
    epoch: u64,
}

/// A request to the task that is never held back behind input.
enum Control {
    Shutdown(oneshot::Sender<ShutdownReport>),
}

impl HostTerminal {
    /// Connect to the host at `socket`, take it over, and start its runtime.
    ///
    /// Before returning it pins the child (reading the host's `Info`, the
    /// child's process, and `Info` again, so that the identity read is the
    /// child's: a pid is not reused before the host reaps it), starts
    /// reading the host's journal from the start, and makes two more `Info`
    /// round trips, one after the other, by the second of whose replies the
    /// stream's first record has arrived (`crates/terminal-host/SPEC.md`,
    /// "Reading", on the order of replies and records). When that record
    /// is a snapshot record it is restored here, so a host whose stored
    /// snapshot cannot be restored is an [`AdoptError`], never a terminal
    /// that looks usable for a moment. It does not wait for the rest of the
    /// journal; [`HostStatus::caught_up`] says when it has been processed.
    pub async fn adopt(
        socket: &Path,
        adoption: Adoption,
        options: HostTerminalOptions,
    ) -> Result<HostTerminal, AdoptError> {
        let mut conn = HostConnection::connect_with(socket, options.deadlines).await?;
        let peer = conn.peer();
        let unavailable = |reason| AdoptError::Unavailable { reason, peer };
        let (child, info) = pin_child(&mut conn)
            .await
            .map_err(|error| unavailable(Unavailability::Request(error)))?;
        // A child the host has already reaped is exited only once the
        // stream's exit record is processed, after the output it left.
        let (liveness, child_start) = match child {
            Child::Running(root) => (Liveness::Running { child: Some(root) }, Some(root.start)),
            Child::Reaped => (Liveness::Running { child: None }, None),
            Child::Unpinned => {
                return Err(unavailable(Unavailability::ChildUnpinned {
                    pid: info.child_pid,
                }));
            }
        };
        conn.start_read(Position::default())
            .await
            .map_err(|error| unavailable(Unavailability::Request(error)))?;
        // The fence: two `Info` round trips after `Read`, the second sent
        // only once the first is answered. A version-1 host's `Read` only
        // sets the stream's cursor (`handle`, crates/terminal-host/src/
        // host.rs); records move from the cursor into the outgoing queue
        // only at the end of a loop iteration (`refill_outgoing`, after
        // `act_on_buffered` in `Host::step`), and a reply is written only at
        // the start of a later one (`write_client` comes before the client
        // is read and acted on). So the first reply can overtake the
        // stream's first record, when `Read` and `Info` are acted on in one
        // iteration; but that iteration ends by queuing the first record
        // (the snapshot record, when there is one), and the second `Info`,
        // sent after the first reply was written, is acted on in a later
        // iteration and answered behind it. Once the second reply is read,
        // the first record (when the stream held one at `Read`) has been
        // decoded and is queued in the client.
        // The host's SPEC states this order ("Reading") for exactly the
        // exchange made here, so every host keeps it for this adoption:
        // every reply the adoption asked for before `Read` has been received
        // (the pinning's `Info`s), it has sent no input (so no
        // acknowledgement is owed), and after `Read` it sends one `Info`,
        // waits for its reply, and then sends one more.
        for _ in 0..2 {
            conn.info()
                .await
                .map_err(|error| unavailable(Unavailability::Request(error)))?;
        }
        let fence = conn.queued_events();

        let adopted_end = Position {
            offset: info.end,
            resizes: info.resizes,
        };
        let replay = match adoption {
            Adoption::Fresh if info.delivered_end > 0 => {
                warn!(
                    socket = %socket.display(),
                    delivered_end = info.delivered_end,
                    "a terminal host adopted as fresh has sent output to a client before; \
                     replaying that output without answers"
                );
                Replay::until(Position {
                    offset: info.delivered_end,
                    resizes: 0,
                })
            }
            Adoption::Fresh => Replay::finished(),
            Adoption::Recovered => Replay::until(adopted_end),
        };
        let emulator = Arc::new(StdMutex::new(Emulator::new(
            emulator_size(info.initial_size),
            options.scrollback_lines,
        )));
        let initial = HostStatus {
            liveness: liveness.clone(),
            child_pid: info.child_pid,
            child_start,
            position: Position::default(),
            revision: 0,
            caught_up: fence == 0 && adopted_end == Position::default(),
        };
        let (status_tx, status_rx) = watch::channel(initial);
        let (input_tx, input_rx) = mpsc::channel(INPUT_QUEUE);
        let (control_tx, control_rx) = mpsc::channel(1);
        let (size_tx, size_rx) = watch::channel(None);
        let epoch = Arc::new(AtomicU64::new(0));

        let mut runtime = Runtime {
            socket: socket.to_path_buf(),
            deadlines: options.deadlines,
            clock: options.clock,
            notifier: options.notifier,
            scrollback_lines: options.scrollback_lines,
            emulator: emulator.clone(),
            status: status_tx,
            input_rx,
            control_rx,
            size_rx,
            size_open: true,
            size_wanted: None,
            size_requested: None,
            peer,
            position: Position::default(),
            have_state: false,
            replay,
            adopted_end,
            pending: VecDeque::new(),
            pending_answer_bytes: 0,
            in_flight: VecDeque::new(),
            counts: InputCounts::default(),
            reconnects: VecDeque::new(),
            fence,
            epoch: epoch.clone(),
        };
        if fence > 0 {
            // Queued already, so this does not wait.
            let first = conn
                .next_event()
                .await
                .map_err(|error| unavailable(Unavailability::Request(error)))?;
            if let Turn::Unavailable(reason) = runtime.on_event(first) {
                return Err(unavailable(reason));
            }
        }
        tokio::spawn(runtime.run(conn));
        Ok(HostTerminal {
            inner: Arc::new(Inner {
                socket: socket.to_path_buf(),
                emulator,
                status: status_rx,
                input: input_tx,
                control: control_tx,
                size: size_tx,
                epoch,
            }),
        })
    }

    /// The host's socket, which names the host.
    pub fn socket(&self) -> &Path {
        &self.inner.socket
    }

    /// The terminal's state now.
    pub fn status(&self) -> HostStatus {
        self.inner.status.borrow().clone()
    }

    /// The terminal's state, to wait on for changes.
    pub fn subscribe(&self) -> watch::Receiver<HostStatus> {
        self.inner.status.clone()
    }

    /// The program's liveness now.
    pub fn liveness(&self) -> Liveness {
        self.inner.status.borrow().liveness.clone()
    }

    /// Read the emulator: for what the accessors below do not cover (the
    /// grid and its history, a Restart seed). `read` holds the emulator's
    /// lock, which the task needs to feed output, so it must be quick.
    pub fn read<R>(&self, read: impl FnOnce(&Emulator) -> R) -> R {
        let emulator = self
            .inner
            .emulator
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        read(&emulator)
    }

    /// The last `max_bytes` of the visible screen as plain text, shaped
    /// like tmux's capture (`Emulator::screen_tail`): what status detection
    /// reads.
    pub fn screen_tail(&self, max_bytes: usize) -> String {
        self.read(|emulator| emulator.screen_tail(max_bytes))
    }

    /// The title the program last set.
    pub fn title(&self) -> String {
        self.read(|emulator| emulator.title().to_owned())
    }

    /// The program's terminal modes now: what input encoding and prompt
    /// delivery need (bracketed paste, mouse, cursor keys).
    pub fn modes(&self) -> Modes {
        self.read(Emulator::modes)
    }

    /// The terminal's size as its emulator has it: the size of the latest
    /// resize record processed, which a resize just asked for may not have
    /// reached yet.
    pub fn size(&self) -> Size {
        let size = self.read(Emulator::size);
        Size {
            cols: size.columns,
            rows: size.lines,
        }
    }

    /// Queue `data` (1 to `MAX_INPUT` bytes) as one `Input` to the program,
    /// in order with earlier input and the terminal's own answers.
    ///
    /// Waits while the task's queue is full, which it is while the program
    /// does not read its terminal; the wait is what holds a paste back.
    /// Dropped while waiting, it sent nothing. Once it returns, the input is
    /// the task's: the receipt says what became of it, and dropping the
    /// receipt does not take it back.
    ///
    /// The input carries the epoch of the moment this is called (module
    /// docs, "Input"): for one-shot input. The parts of one paste go through
    /// an [`InputSequence`] instead.
    pub async fn send_input(&self, data: &[u8]) -> Result<InputReceipt, InputRefused> {
        let epoch = self.inner.epoch.load(Ordering::SeqCst);
        self.send_in_epoch(data, epoch).await
    }

    /// A sequence of inputs that belong together, such as the parts of one
    /// paste: once any input is lost with a connection, none of its parts
    /// asked for afterwards is written (module docs, "Input").
    pub fn input_sequence(&self) -> InputSequence {
        InputSequence {
            inner: Arc::downgrade(&self.inner),
            epoch: self.inner.epoch.load(Ordering::SeqCst),
        }
    }

    /// [`Self::send_input`], stamped with `epoch`.
    async fn send_in_epoch(&self, data: &[u8], epoch: u64) -> Result<InputReceipt, InputRefused> {
        if data.is_empty() || data.len() > MAX_INPUT {
            return Err(InputRefused::Invalid);
        }
        let (fate, receipt) = oneshot::channel();
        let request = InputRequest {
            data: data.to_vec(),
            fate,
            epoch,
        };
        self.inner
            .input
            .send(request)
            .await
            .map_err(|_| InputRefused::Ended)?;
        Ok(InputReceipt(receipt))
    }

    /// Ask for the terminal to be `size`. Returns at once; the emulator takes
    /// the size when the host's resize record arrives ([`Self::size`]).
    /// Asking again before it has been sent replaces the earlier size.
    pub fn resize(&self, size: Size) -> Result<(), ResizeRefused> {
        if !size.is_valid() {
            return Err(ResizeRefused::Invalid);
        }
        // The runtime holds the only receiver; it is gone once it ended.
        if self.inner.size.is_closed() {
            return Err(ResizeRefused::Ended);
        }
        self.inner.size.send_replace(Some(size));
        Ok(())
    }

    /// End the host through its protocol (`Shutdown`, which closes the pty,
    /// giving the child its hangup) and confirm that it ended.
    ///
    /// The task takes the request up once it is done with what it is doing:
    /// a request to the host in progress (bounded by the reply deadline) or
    /// a reconnect (up to [`RECONNECT_LIMIT`] connects). From then on it
    /// takes the protocol's shutdown (the connection's shutdown deadline);
    /// when the connection turns out to have ended before the host answered
    /// (noticed or not), one connect and a second shutdown on it, so that a
    /// healthy host is not reported unconfirmed for want of a connection;
    /// and the wait for the host's process to go (the shutdown deadline
    /// again).
    ///
    /// Input not yet sent is not sent; input in flight is unknown. The
    /// runtime ends either way, [`Liveness::Gone`] when the host was
    /// confirmed ended and unavailable when it was not. A runtime that had
    /// already ended answers with its final liveness instead, whose peer
    /// (for an unavailable host) is what may still end the host.
    pub async fn shutdown(&self) -> Result<ShutdownReport, Liveness> {
        let (reply, report) = oneshot::channel();
        if self
            .inner
            .control
            .send(Control::Shutdown(reply))
            .await
            .is_err()
        {
            return Err(self.liveness());
        }
        report.await.map_err(|_| self.liveness())
    }

    // ---- Adapters, from the state now (see `HostStatus`) ----

    /// See [`HostStatus::child_root`].
    pub fn child_root(&self) -> Option<ProcessRoot> {
        self.inner.status.borrow().child_root()
    }

    /// See [`HostStatus::probe`].
    pub fn probe(&self) -> anyhow::Result<PaneProbe> {
        self.inner.status.borrow().probe()
    }

    /// See [`HostStatus::evidence`].
    pub fn evidence(&self) -> HostEvidence {
        self.inner.status.borrow().evidence()
    }

    /// See [`HostStatus::exit_code`].
    pub fn exit_code(&self) -> Option<i32> {
        self.inner.status.borrow().exit_code()
    }

    /// See `HostStatus::anchor`.
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "the hook anchor's switch point is its first caller"
        )
    )]
    pub(crate) fn anchor(&self) -> Option<PaneAnchor> {
        self.inner.status.borrow().anchor()
    }
}

/// A host's size in the emulator's terms.
fn emulator_size(size: Size) -> farhelm_terminal_emulator::Size {
    farhelm_terminal_emulator::Size {
        columns: size.cols,
        lines: size.rows,
    }
}

/// What reading the host's child found.
enum Child {
    /// It runs, pinned.
    Running(ProcessRoot),
    /// The host reaped it. How it ended is the exit record's to say, after
    /// the output it left.
    Reaped,
    /// The host says it runs, and its process could not be read.
    Unpinned,
}

/// Find the host's child and pin it: `Info`, the child's identity, `Info`
/// again. An identity read between two answers that both say the child runs
/// is the child's own, since its pid cannot be reused until the host reaps
/// it. Returns the last `Info` read.
async fn pin_child(conn: &mut HostConnection) -> Result<(Child, Info), ClientError> {
    let mut info = conn.info().await?;
    for _ in 0..PIN_ATTEMPTS {
        if info.child_exit.is_some() {
            return Ok((Child::Reaped, info));
        }
        // A pid of 0 or 1 is never a child to pin: signalling either would
        // reach a process group or init.
        let identity = (info.child_pid > 1)
            .then(|| ProcessIdentity::read(info.child_pid))
            .flatten();
        let after = conn.info().await?;
        match (identity, after.child_exit) {
            (_, Some(_)) => return Ok((Child::Reaped, after)),
            (Some(identity), None) => {
                let root = ProcessRoot {
                    pid: identity.pid,
                    start: identity.start,
                };
                return Ok((Child::Running(root), after));
            }
            (None, None) => {
                info = after;
                tokio::time::sleep(PIN_RETRY).await;
            }
        }
    }
    Ok((Child::Unpinned, info))
}

// ---- The runtime task ------------------------------------------------------------

/// What of the journal is replay: everything before `end`, until it has all
/// been processed. The replay policy's seam: it splits the replay again at
/// the host's `delivered_end`, read with the adoption's `Info`.
#[derive(Debug, Clone, Copy)]
struct Replay {
    end: Position,
    finished: bool,
}

impl Replay {
    /// No replay: everything is live.
    fn finished() -> Self {
        Self {
            end: Position::default(),
            finished: true,
        }
    }

    /// Everything before `end` is replay.
    fn until(end: Position) -> Self {
        Self {
            end,
            finished: end == Position::default(),
        }
    }

    /// The mode for the effects of processing a record now.
    fn mode(&self) -> FeedMode {
        if self.finished {
            FeedMode::Live
        } else {
            REPLAYED
        }
    }
}

/// Whether `a` lies at or after `b` in the journal (both of its counts).
fn reached(a: Position, b: Position) -> bool {
    a.offset >= b.offset && a.resizes >= b.resizes
}

/// Whether `a` lies strictly after `b`: later output, or as much output and
/// more resizes.
fn after(a: Position, b: Position) -> bool {
    a.offset > b.offset || (a.offset == b.offset && a.resizes > b.resizes)
}

/// Collects a feed's answers, one entry per answer, and drops its notices
/// (see the module docs for why).
#[derive(Default)]
struct Answers(Vec<Vec<u8>>);

impl Effects for Answers {
    fn reply(&mut self, bytes: &[u8]) {
        self.0.push(bytes.to_vec());
    }

    fn notice(&mut self, _notice: Notice) {}
}

/// Input waiting for room in the window: a caller's, or the terminal's own
/// answers.
struct Outgoing {
    data: Vec<u8>,
    /// The caller's receipt; `None` for answers.
    fate: Option<oneshot::Sender<InputFate>>,
}

/// Input sent and not yet acknowledged.
struct InFlight {
    ticket: InputTicket,
    fate: Option<oneshot::Sender<InputFate>>,
}

/// Resolve a receipt, if there is one.
fn resolve(fate: Option<oneshot::Sender<InputFate>>, outcome: InputFate) {
    if let Some(fate) = fate {
        // A receipt nobody waits for is fine to drop.
        let _ = fate.send(outcome);
    }
}

/// What one turn of the task's loop ended with.
enum Turn {
    Continue,
    /// Every handle is gone: stop, leaving the host running.
    Stop,
    Shutdown(oneshot::Sender<ShutdownReport>),
    /// The host cannot be used any more.
    Unavailable(Unavailability),
}

/// What the task woke for.
enum Wake {
    Event(Result<HostEvent, ClientError>),
    Input(Option<InputRequest>),
    Size(bool),
    Control(Option<Control>),
    SyncDue,
}

/// The task's state: everything but the connection, which [`Runtime::run`]
/// owns so that a shutdown can consume it.
struct Runtime {
    socket: PathBuf,
    deadlines: ClientDeadlines,
    clock: Clock,
    notifier: Option<LivenessNotifier>,
    scrollback_lines: usize,
    emulator: Arc<StdMutex<Emulator>>,
    status: watch::Sender<HostStatus>,
    input_rx: mpsc::Receiver<InputRequest>,
    control_rx: mpsc::Receiver<Control>,
    size_rx: watch::Receiver<Option<Size>>,
    /// Whether the size channel still has a sender.
    size_open: bool,
    /// A size asked for and not yet sent on the current connection.
    size_wanted: Option<Size>,
    /// The latest size asked for, sent or not: what a new connection
    /// checks the host's size against.
    size_requested: Option<Size>,
    /// The host's process, from the latest connection.
    peer: Option<HostProcess>,
    /// The position processed (also in the status, for readers).
    position: Position,
    /// Whether the emulator holds the host's state yet: false until the
    /// first record of the first stream, so that an initial snapshot record
    /// at (0, 0) is restored rather than taken for one already held.
    have_state: bool,
    replay: Replay,
    adopted_end: Position,
    pending: VecDeque<Outgoing>,
    /// The bytes of answers in `pending` ([`MAX_PENDING_ANSWERS`]).
    pending_answer_bytes: usize,
    in_flight: VecDeque<InFlight>,
    /// The current connection's acknowledged input totals.
    counts: InputCounts,
    /// When recent reconnects happened ([`RECONNECT_LIMIT`]).
    reconnects: VecDeque<tokio::time::Instant>,
    /// The input epoch (`Inner::epoch`).
    epoch: Arc<AtomicU64>,
    /// How many of the adoption's first events, read before its fence
    /// `Info` reply, are still to be processed; caught up only at 0.
    fence: usize,
}

impl Runtime {
    /// The task: turns until the handles are gone, the host ends or cannot
    /// be used, or a shutdown, reconnecting when the connection ends.
    async fn run(mut self, mut conn: HostConnection) {
        loop {
            match self.turn(&mut conn).await {
                Ok(Turn::Continue) => {}
                Ok(Turn::Stop) => break,
                Ok(Turn::Shutdown(reply)) => {
                    self.shut_down(conn, reply).await;
                    return;
                }
                Ok(Turn::Unavailable(reason)) => {
                    let peer = conn.peer();
                    self.set_liveness(Liveness::Unavailable { reason, peer });
                    break;
                }
                Err(error) => match self.reconnect(error).await {
                    Some(new) => conn = new,
                    None => break,
                },
            }
        }
        self.finish();
    }

    /// Send what waits, then wait for one thing to happen and act on it.
    ///
    /// Requests to the host (input, a resize) are made here, outside the
    /// `select!`, so that none is ever dropped halfway (the client's module
    /// docs on cancellation); the `select!` holds only cancel-safe waits.
    async fn turn(&mut self, conn: &mut HostConnection) -> Result<Turn, ClientError> {
        self.send_pending(conn).await?;
        if let Some(size) = self.size_wanted {
            match conn.resize(size).await {
                // Sizes are checked by the handle; one the client refuses
                // anyway is dropped rather than retried forever.
                Ok(()) | Err(ClientError::InvalidRequest(_)) => self.size_wanted = None,
                // Kept, to be sent on the next connection.
                Err(error) => return Err(error),
            }
        }
        let deadline = self.lock().sync_deadline();
        let clock = self.clock.clone();
        let sync_due = async {
            match deadline {
                Some(deadline) => clock.sleep_until(deadline).await,
                None => std::future::pending().await,
            }
        };
        // Callers' input is taken only while nothing waits for the window,
        // so that a program not reading its terminal holds it back in the
        // callers' sends rather than here.
        let accepting = self.pending.is_empty();
        let wake = tokio::select! {
            event = conn.next_event() => Wake::Event(event),
            request = self.input_rx.recv(), if accepting => Wake::Input(request),
            changed = self.size_rx.changed(), if self.size_open => Wake::Size(changed.is_ok()),
            control = self.control_rx.recv() => Wake::Control(control),
            () = sync_due => Wake::SyncDue,
        };
        Ok(match wake {
            Wake::Event(event) => self.on_event(event?),
            Wake::Input(Some(request)) if request.epoch < self.epoch.load(Ordering::SeqCst) => {
                // Made before caller input was lost: behind the hole.
                let _ = request.fate.send(InputFate::NotSent);
                Turn::Continue
            }
            Wake::Input(Some(request)) => {
                self.pending.push_back(Outgoing {
                    data: request.data,
                    fate: Some(request.fate),
                });
                Turn::Continue
            }
            Wake::Input(None) | Wake::Control(None) => Turn::Stop,
            Wake::Size(open) => {
                if open {
                    self.size_wanted = *self.size_rx.borrow_and_update();
                    self.size_requested = self.size_wanted.or(self.size_requested);
                } else {
                    self.size_open = false;
                }
                Turn::Continue
            }
            Wake::Control(Some(Control::Shutdown(reply))) => Turn::Shutdown(reply),
            Wake::SyncDue => {
                let now = self.clock.now();
                let mode = self.replay.mode();
                let mut answers = Answers::default();
                let flushed = self.lock().flush_sync_if_due(now, mode, &mut answers);
                if flushed {
                    self.queue_answers(answers);
                    self.publish();
                }
                Turn::Continue
            }
        })
    }

    /// Send waiting input, in order, until the window is full.
    ///
    /// An `Input` the client queued counts as sent even when the call then
    /// fails, and its fate is unknown: the client's `sent` count tells, and
    /// such an input is resolved as unknown rather than left to be sent
    /// again on the next connection.
    async fn send_pending(&mut self, conn: &mut HostConnection) -> Result<(), ClientError> {
        while let Some(next) = self.pending.front() {
            let before = conn.input_counts().sent;
            let sent = conn.send_input(&next.data).await;
            let queued = conn.input_counts().sent > before;
            match sent {
                Ok(ticket) => {
                    let next = self.pop_pending();
                    self.in_flight.push_back(InFlight {
                        ticket,
                        fate: next.fate,
                    });
                }
                Err(ClientError::InputWindowFull { .. }) => return Ok(()),
                Err(error) if queued => {
                    // Sent, as far as anyone can tell: it is the reconnect's
                    // to report unknown, together with the rest of what the
                    // connection lost and after the one drop of what
                    // followed it (`reconnect`).
                    let lost = self.pop_pending();
                    self.in_flight.push_back(InFlight {
                        ticket: InputTicket {
                            end: conn.input_counts().sent,
                        },
                        fate: lost.fate,
                    });
                    return Err(error);
                }
                Err(ClientError::InvalidRequest(why)) => {
                    // The handle checks lengths, and answers are cut to fit,
                    // so this does not happen; dropping it keeps the queue
                    // moving if it ever does.
                    warn!(socket = %self.socket.display(), why, "dropping input the client refused");
                    resolve(self.pop_pending().fate, InputFate::NotSent);
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    /// Take the oldest waiting input, keeping the answer count right.
    fn pop_pending(&mut self) -> Outgoing {
        let next = self.pending.pop_front().expect("input is waiting");
        if next.fate.is_none() {
            self.pending_answer_bytes -= next.data.len();
        }
        next
    }

    /// Queue a feed's answers, packed into as few `Input`s as fit
    /// `MAX_INPUT` without cutting an answer (only an answer longer than
    /// that is cut), within [`MAX_PENDING_ANSWERS`].
    fn queue_answers(&mut self, answers: Answers) {
        let mut dropped = 0usize;
        for answer in answers.0 {
            if self.pending_answer_bytes + answer.len() > MAX_PENDING_ANSWERS {
                dropped += 1;
                continue;
            }
            self.pending_answer_bytes += answer.len();
            for piece in answer.chunks(MAX_INPUT) {
                match self.pending.back_mut() {
                    Some(last)
                        if last.fate.is_none() && last.data.len() + piece.len() <= MAX_INPUT =>
                    {
                        last.data.extend_from_slice(piece);
                    }
                    _ => self.pending.push_back(Outgoing {
                        data: piece.to_vec(),
                        fate: None,
                    }),
                }
            }
        }
        if dropped > 0 {
            warn!(
                socket = %self.socket.display(),
                dropped,
                "dropping answers to terminal queries: the program is not reading its terminal"
            );
        }
    }

    /// Act on one record or acknowledgement from the host.
    fn on_event(&mut self, event: HostEvent) -> Turn {
        self.fence = self.fence.saturating_sub(1);
        // Acknowledgements are not records: they say nothing about the
        // emulator's state, so only a record ends the first stream's start.
        let first = !self.have_state && !matches!(event, HostEvent::InputAcknowledged(_));
        if first {
            self.have_state = true;
        }
        match event {
            HostEvent::Snapshot { position, blob } => {
                if let Err(error) = self.on_snapshot(first, position, &blob) {
                    return Turn::Unavailable(Unavailability::Snapshot(error));
                }
            }
            HostEvent::Output { offset, data } => self.on_output(offset, &data),
            HostEvent::Resized {
                offset,
                index,
                size,
            } => self.on_resized(offset, index, size),
            HostEvent::Gap {
                from,
                to,
                resizes,
                size,
            } => self.on_gap(from, to, resizes, size),
            HostEvent::Exited { offset, exit } => self.on_exited(offset, exit),
            HostEvent::InputAcknowledged(counts) => self.acknowledge(counts),
        }
        Turn::Continue
    }

    /// Restore the emulator from a snapshot record, unless it already holds
    /// that state or a later one: a stream resumed after a reconnect sends
    /// the record again when its position cannot be continued from
    /// (`crates/terminal-host/SPEC.md`, "Reading"), and what follows it then
    /// continues from the emulator's own position.
    fn on_snapshot(
        &mut self,
        first: bool,
        position: Position,
        blob: &[u8],
    ) -> Result<(), SnapshotError> {
        if !first && !after(position, self.position) {
            return Ok(());
        }
        let restored = Emulator::restore(blob, self.scrollback_lines)?;
        if restored.screens_dropped || restored.width_table_changed.is_some() {
            // The program should be asked to redraw, as after a gap; that
            // request belongs with the gap handling, which is not here yet.
            info!(
                socket = %self.socket.display(),
                screens_dropped = restored.screens_dropped,
                width_table_changed = restored.width_table_changed.is_some(),
                "restored a terminal snapshot that needs a redraw"
            );
        }
        *self.lock() = restored.emulator;
        self.position = position;
        self.after_record();
        Ok(())
    }

    /// Feed output, skipping what was already processed, with the part
    /// before the replay's end fed as replay and the rest live.
    fn on_output(&mut self, offset: u64, data: &[u8]) {
        let mut offset = offset;
        let mut data = data;
        if offset < self.position.offset {
            let skip = usize::try_from(self.position.offset - offset)
                .unwrap_or(usize::MAX)
                .min(data.len());
            data = &data[skip..];
            offset += skip as u64;
        }
        if data.is_empty() {
            return;
        }
        if offset > self.position.offset {
            // The host sends a gap record for anything it dropped, so this is
            // a host breaking its contract; carry on from where it resumed.
            warn!(
                socket = %self.socket.display(),
                expected = self.position.offset,
                offset,
                "terminal output resumed past where it was processed"
            );
        }
        let replayed = if self.replay.finished {
            0
        } else {
            usize::try_from(self.replay.end.offset.saturating_sub(offset))
                .unwrap_or(usize::MAX)
                .min(data.len())
        };
        let (before, live) = data.split_at(replayed);
        // The checkpoint seam: a snapshot is taken here, where the emulator
        // is clean after a read's split candidate or its end
        // (`farhelm_terminal_emulator::snapshot_split_candidate`), at the
        // position processed so far.
        let now = self.clock.now();
        let mut answers = Answers::default();
        if !before.is_empty() {
            self.lock().feed(before, now, REPLAYED, &mut answers);
            self.position.offset = offset + before.len() as u64;
            self.finish_replay_if_reached(&mut answers);
        }
        if !live.is_empty() {
            // Output past the replay's end was journaled after adoption, so
            // the replay is over even if a record before it is missing.
            self.finish_replay(&mut answers);
            self.lock().feed(live, now, FeedMode::Live, &mut answers);
        }
        self.position.offset = offset + data.len() as u64;
        self.queue_answers(answers);
        self.after_record();
    }

    /// Apply a resize record, unless it was already applied.
    fn on_resized(&mut self, offset: u64, index: u64, size: Size) {
        if index < self.position.resizes {
            return;
        }
        let mode = self.replay.mode();
        let mut answers = Answers::default();
        self.lock().resize(emulator_size(size), mode, &mut answers);
        self.position = Position {
            offset: offset.max(self.position.offset),
            resizes: index + 1,
        };
        self.queue_answers(answers);
        self.after_record();
    }

    /// Continue past output the host dropped, keeping the emulator: a gap
    /// never resets a healthy terminal, it only means the bytes in it were
    /// never interpreted.
    fn on_gap(&mut self, from: u64, to: u64, resizes: u64, size: Size) {
        let resumes = Position {
            offset: to,
            resizes,
        };
        if !after(resumes, self.position) {
            return;
        }
        // Asking the program to redraw, and telling the viewer that history
        // was lost, belong here once the journal is checkpointed.
        warn!(
            socket = %self.socket.display(),
            from, to, "terminal output was trimmed before it was processed"
        );
        let mode = self.replay.mode();
        let mut answers = Answers::default();
        {
            let mut emulator = self.lock();
            if emulator.size() != emulator_size(size) {
                emulator.resize(emulator_size(size), mode, &mut answers);
            }
        }
        self.position = Position {
            offset: to.max(self.position.offset),
            resizes: resizes.max(self.position.resizes),
        };
        self.queue_answers(answers);
        self.after_record();
    }

    /// The child exited: draw any update it left open (it can no longer end
    /// it), and record the exit. Output after it (from descendants that keep
    /// the pty) is fed as before.
    fn on_exited(&mut self, offset: u64, exit: ChildExit) {
        let mode = self.replay.mode();
        let mut answers = Answers::default();
        self.lock().flush_sync(mode, &mut answers);
        self.queue_answers(answers);
        self.publish();
        info!(socket = %self.socket.display(), offset, ?exit, "a host terminal's program exited");
        // The host sends the record once per stream, so a resumed stream
        // repeats it; moving forward only makes the repeat a no-op.
        self.set_liveness(Liveness::Exited { exit });
    }

    /// Resolve the inputs an acknowledgement covers. Within one
    /// acknowledgement the host's totals do not say which input each
    /// written or discarded byte belonged to, so the newly resolved inputs
    /// are counted as written first, in order: exact when one
    /// acknowledgement resolves one input, as it usually does, and when the
    /// pty lost its reader partway through several.
    fn acknowledge(&mut self, counts: InputCounts) {
        let resolved_before = self.counts.written + self.counts.discarded;
        let written = counts.written.saturating_sub(self.counts.written);
        while let Some(front) = self.in_flight.front()
            && counts.resolved(front.ticket)
        {
            let front = self.in_flight.pop_front().expect("just looked");
            let outcome = if front.ticket.end - resolved_before <= written {
                InputFate::Written
            } else {
                InputFate::Discarded
            };
            resolve(front.fate, outcome);
        }
        self.counts = counts;
    }

    /// End the replay if its last record has been processed.
    fn finish_replay_if_reached(&mut self, answers: &mut Answers) {
        if !self.replay.finished && reached(self.position, self.replay.end) {
            self.finish_replay(answers);
        }
    }

    /// End the replay: draw an update it left open, with replay's effects,
    /// so that the first live feed does not end it with live ones (the
    /// emulator's `FeedMode` docs).
    fn finish_replay(&mut self, answers: &mut Answers) {
        if self.replay.finished {
            return;
        }
        self.lock().flush_sync(REPLAYED, answers);
        self.replay.finished = true;
    }

    /// Bookkeeping after any record that moved the position.
    fn after_record(&mut self) {
        let mut answers = Answers::default();
        self.finish_replay_if_reached(&mut answers);
        self.queue_answers(answers);
        self.publish();
    }

    /// Tell readers that the emulator changed.
    fn publish(&self) {
        let position = self.position;
        let caught_up = self.fence == 0 && reached(position, self.adopted_end);
        self.status.send_modify(|status| {
            status.position = position;
            status.revision += 1;
            status.caught_up = caught_up;
        });
    }

    /// Move to `liveness`, only forward (`Liveness::rank`): a withdrawn root
    /// never comes back, a terminal that exited never runs again, and one
    /// that ended stays ended. Announces every move to exited, gone or
    /// unavailable.
    fn set_liveness(&self, liveness: Liveness) {
        let changed = self.status.send_if_modified(|status| {
            let allowed = liveness.rank() > status.liveness.rank();
            if allowed {
                status.liveness = liveness.clone();
            }
            allowed
        });
        if changed {
            self.notify(&liveness);
        }
    }

    /// Announce `liveness` to the notifier, unless it is running.
    fn notify(&self, liveness: &Liveness) {
        if !liveness.is_announced() {
            return;
        }
        if let Some(notifier) = &self.notifier {
            // A notifier whose receiver is gone has nobody to tell.
            let _ = notifier.0.send(LivenessChange {
                socket: self.socket.clone(),
                liveness: liveness.clone(),
            });
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Emulator> {
        self.emulator.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The connection ended with `error`: learn what became of the host by
    /// connecting again, and resume reading where processing stopped if it
    /// answers. `None` when the runtime ends instead (the host gone or
    /// unavailable, the connection unstable, or nobody holding the terminal
    /// any more).
    async fn reconnect(&mut self, error: ClientError) -> Option<HostConnection> {
        // Input the old connection sent and the host never acknowledged
        // went with it, and may have been written. What callers sent after
        // it is dropped first, so that input a caller sends on learning of
        // the loss comes after the drop and is written.
        if self.in_flight.iter().any(|lost| lost.fate.is_some()) {
            self.drop_caller_input();
        }
        for lost in self.in_flight.drain(..) {
            resolve(lost.fate, InputFate::Unknown);
        }
        self.counts = InputCounts::default();
        let mut error = error;
        loop {
            // A runtime nobody holds does not take its host back: the
            // connection may have ended because a new runtime took over.
            if self.control_rx.is_closed() {
                return None;
            }
            let now = tokio::time::Instant::now();
            self.reconnects
                .retain(|at| now.duration_since(*at) < RECONNECT_WINDOW);
            if self.reconnects.len() >= RECONNECT_LIMIT {
                warn!(socket = %self.socket.display(), %error, "giving up on a terminal host whose connection keeps ending");
                self.set_liveness(Liveness::Unavailable {
                    reason: Unavailability::Unstable,
                    peer: self.peer,
                });
                return None;
            }
            self.reconnects.push_back(now);
            warn!(socket = %self.socket.display(), %error, "the connection to a terminal host ended; reconnecting");
            let mut conn = match HostConnection::connect_with(&self.socket, self.deadlines).await {
                Ok(conn) => conn,
                Err(failure) => {
                    let liveness = match AdoptError::from(failure) {
                        AdoptError::Gone(_) => Liveness::Gone,
                        AdoptError::Unavailable { reason, peer } => match self.adopted_peer(peer) {
                            Ok(peer) => Liveness::Unavailable { reason, peer },
                            Err(adopted) => Liveness::Unavailable {
                                reason: Unavailability::Replaced,
                                peer: Some(adopted),
                            },
                        },
                    };
                    self.set_liveness(liveness);
                    return None;
                }
            };
            match self.adopted_peer(conn.peer()) {
                Ok(peer) => self.peer = peer,
                Err(adopted) => {
                    self.set_liveness(Liveness::Unavailable {
                        reason: Unavailability::Replaced,
                        peer: Some(adopted),
                    });
                    return None;
                }
            }
            match self.resume(&mut conn).await {
                Ok(()) => return Some(conn),
                Err(failure) => error = failure,
            }
        }
    }

    /// Bring a new connection up to where the old one was: the child's
    /// state (it may have exited meanwhile), the stream from the processed
    /// position, and the size asked for. A size not yet sent stays in
    /// `size_wanted`; one sent on the old connection goes again when the
    /// host reports another size, since the old connection may have ended
    /// before the host acted on it.
    async fn resume(&mut self, conn: &mut HostConnection) -> Result<(), ClientError> {
        let (child, info) = pin_child(conn).await?;
        match child {
            // Only the root is withdrawn: the exit itself is the stream's
            // exit record's to announce, after the output it follows.
            Child::Reaped => self.set_liveness(Liveness::Running { child: None }),
            // The same child as before: the host runs one, and it still
            // runs, so the root pinned at adoption stands.
            Child::Running(_) | Child::Unpinned => {}
        }
        if let Some(size) = self.size_requested
            && size != info.size
        {
            self.size_wanted = Some(size);
        }
        conn.start_read(self.position).await
    }

    /// End the input epoch, once per lost connection, and report the caller
    /// input waiting for the window as not sent, keeping the answers: what
    /// follows caller input that was lost with a connection is never written
    /// after the hole (module docs, "Input").
    ///
    /// Requests in the task's channel, and those still waiting for room in
    /// it, are left there: each is reported not sent when the task receives
    /// it in its turn (`Wake::Input` refuses an older epoch), and one that
    /// already carries the new epoch is sent. Draining the channel here
    /// instead would free its slots for other callers to refill, with nothing
    /// to bound how many requests the drain took in or how long it delayed
    /// the reconnect; left to the turn, the channel keeps its bound.
    fn drop_caller_input(&mut self) {
        self.epoch.fetch_add(1, Ordering::SeqCst);
        let mut kept = VecDeque::with_capacity(self.pending.len());
        for waiting in self.pending.drain(..) {
            match waiting.fate {
                Some(fate) => {
                    let _ = fate.send(InputFate::NotSent);
                }
                None => kept.push_back(waiting),
            }
        }
        self.pending = kept;
    }

    /// The process to name as the host after a connect found `found`: the
    /// adopted host's, or `found` when the adoption learned none; `Err` with
    /// the adopted process when `found` is another one. So nothing the
    /// runtime reports (a liveness, a shutdown report) ever names a process
    /// it did not adopt as what may end its host.
    fn adopted_peer(&self, found: Option<HostProcess>) -> Result<Option<HostProcess>, HostProcess> {
        match (self.peer, found) {
            (Some(adopted), Some(found)) if adopted != found => {
                warn!(socket = %self.socket.display(), ?adopted, ?found, "another process serves a terminal host's socket");
                Err(adopted)
            }
            (adopted, found) => Ok(adopted.or(found)),
        }
    }

    /// Shut the host down and confirm that it ended (see
    /// [`HostTerminal::shutdown`]).
    async fn shut_down(mut self, conn: HostConnection, reply: oneshot::Sender<ShutdownReport>) {
        let mut peer = conn.peer().or(self.peer);
        // The host discards pending input at `Shutdown` without saying.
        for lost in self.in_flight.drain(..) {
            resolve(lost.fate, InputFate::Unknown);
        }
        // A connection can have ended without the task having noticed yet
        // (an end of stream not read, a takeover): `Shutdown` then fails
        // with `Closed`, or goes out with no `Goodbye`. Either way one new
        // connection tells a host that is gone from one that never read
        // the request, and carries the request to the latter.
        let first = if conn.ended().is_some() {
            Err(ClientError::Closed)
        } else {
            conn.shutdown().await
        };
        let (goodbye, ended) = match first {
            Ok(outcome) if outcome.goodbye => (
                true,
                wait_for_end(&self.socket, peer, self.deadlines.shutdown).await,
            ),
            Ok(_) | Err(ClientError::Closed) => self.shut_down_anew(&mut peer).await,
            Err(error) => {
                warn!(socket = %self.socket.display(), %error, "a terminal host's shutdown failed");
                (
                    false,
                    wait_for_end(&self.socket, peer, self.deadlines.shutdown).await,
                )
            }
        };
        self.set_liveness(if ended {
            Liveness::Gone
        } else {
            Liveness::Unavailable {
                reason: Unavailability::ShutdownUnconfirmed,
                peer,
            }
        });
        let _ = reply.send(ShutdownReport {
            goodbye,
            ended,
            peer,
        });
        self.finish();
    }

    /// The second try of a shutdown whose connection ended before the host
    /// answered it: connect once more and send `Shutdown` there. Says
    /// whether the host acknowledged and whether it was confirmed ended,
    /// and updates `peer` to what may still end it.
    ///
    /// A socket that is gone confirms the end. Another process serving the
    /// socket than the adopted host (as `reconnect` checks) is not shut
    /// down: the terminal becomes unavailable as `Replaced`, and only the
    /// adopted host's own process is waited for.
    async fn shut_down_anew(&self, peer: &mut Option<HostProcess>) -> (bool, bool) {
        let limit = self.deadlines.shutdown;
        match HostConnection::connect_with(&self.socket, self.deadlines).await {
            Err(ConnectError::Gone(_)) => (false, true),
            Err(
                ConnectError::Busy { peer: found } | ConnectError::Unavailable { peer: found, .. },
            ) => {
                match self.adopted_peer(found) {
                    Ok(found) => *peer = found,
                    Err(adopted) => self.replaced(peer, adopted),
                }
                (false, wait_for_end(&self.socket, *peer, limit).await)
            }
            Ok(conn) => {
                match self.adopted_peer(conn.peer()) {
                    Ok(found) => *peer = found,
                    Err(adopted) => {
                        self.replaced(peer, adopted);
                        return (false, wait_for_end(&self.socket, *peer, limit).await);
                    }
                }
                let goodbye = match conn.shutdown().await {
                    Ok(outcome) => outcome.goodbye,
                    Err(error) => {
                        warn!(socket = %self.socket.display(), %error, "a terminal host's shutdown failed");
                        false
                    }
                };
                (goodbye, wait_for_end(&self.socket, *peer, limit).await)
            }
        }
    }

    /// A shutdown's connect found another process at the socket: the
    /// terminal is unavailable as `Replaced`, naming the adopted host, and
    /// only that process is waited for.
    fn replaced(&self, peer: &mut Option<HostProcess>, adopted: HostProcess) {
        self.set_liveness(Liveness::Unavailable {
            reason: Unavailability::Replaced,
            peer: Some(adopted),
        });
        *peer = Some(adopted);
    }

    /// Settle every input still held when the runtime ends: what was never
    /// sent is not sent, and what is in flight is unknown.
    fn finish(mut self) {
        for lost in self.in_flight.drain(..) {
            resolve(lost.fate, InputFate::Unknown);
        }
        for waiting in self.pending.drain(..) {
            resolve(waiting.fate, InputFate::NotSent);
        }
        self.input_rx.close();
        while let Ok(request) = self.input_rx.try_recv() {
            let _ = request.fate.send(InputFate::NotSent);
        }
    }
}

/// Wait up to `limit` for a host that was told to shut down to end: its
/// process gone (or replaced by another under its pid), or, when its process
/// is not known, its socket file gone. Says whether it ended. A process
/// table that cannot be read proves nothing, and the wait goes on.
async fn wait_for_end(socket: &Path, peer: Option<HostProcess>, limit: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + limit;
    loop {
        let ended = match peer {
            Some(peer) => peer.is_running() == Ok(false),
            None => matches!(
                std::fs::symlink_metadata(socket),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound
            ),
        };
        if ended {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(END_POLL).await;
    }
}

#[cfg(test)]
mod tests;
