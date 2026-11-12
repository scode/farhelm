//! Restarting an agent into a new terminal host (the plan's PR 21, the
//! goal's "A restart is a new host, and keeps the history", and its "Old to
//! new": a tmux session's Restart).
//!
//! A terminal host runs one child for its whole life and has no message to
//! start another, so a Restart of an agent whose terminal is a host never
//! reuses that host: it starts a new one for the new launch generation,
//! through the same start a create uses (`host_launch`: the launch record,
//! the `host_scan` fence, adoption), and ends the old host once the new
//! launch is confirmed. A Restart of an agent in tmux goes here too, and
//! that is how an old session moves to a new terminal (below, "From a tmux
//! pane"). Restart's own frame is unchanged ([`Supervisor::relaunch`]: the
//! stop first, the generation claim, the session off the map, the viewer
//! detached, the republication or the restore); this module is where
//! `relaunch_into_terminal` starts every new launch.
//!
//! ## Before anything is stopped
//!
//! Under the session's lifecycle claim, once the consent check has passed
//! (so that a refusal touches nothing) and before the agent is stopped:
//! [`Supervisor::settle_outstanding_launches`] settles the session's launch
//! records after the current host's generation (all of them when the
//! session holds no host). A host start that outlived the request or
//! supervisor that made it can still start an agent nothing else would reach
//! (its first stage holds the whole spec, and nothing it will start carries a
//! marker yet), so its first stage is fenced as a retried create fences its
//! own, and a program found running in such a host is rooted into the stop of
//! the agent. One that cannot be settled refuses the Restart with nothing
//! stopped. Earlier records belong to hosts a confirmed later launch replaced,
//! whose programs the Restart that replaced them stopped; they are left to a
//! later start and to Delete, so a wedged one never holds a Restart up. The
//! settle can wait seconds while the agent's input is not held, so the
//! consent check is asked again after it, right before the stop.
//!
//! ## What the new host starts with
//!
//! SPEC.md ("Lifecycle operations") promises that Restart keeps whatever
//! scrollback the terminal retained and does not repaint the previous run's
//! visible frame. A tmux pane keeps both properties by itself
//! (`respawn-pane` keeps history and clears the grid). A new host has
//! neither, so it is seeded with an initial snapshot holding the old
//! emulator's history above a blank screen in default modes, at the old
//! terminal's size (`farhelm_terminal_emulator::Emulator::history_seed`).
//! The seed is read after the agent is stopped and before the generation is
//! claimed ([`Supervisor::restart_seed`]), so a crash or an orderly stop while
//! it is read leaves the session as it was: once the old runtime has caught
//! up with its host's journal and processed the exit record (bounded; a host
//! that is gone has nothing more to come), so it holds the old program's last
//! output; encoded on the blocking pool, cancelled by an orderly stop, and
//! fitted to the host's `MAX_SNAPSHOT` by leaving out the oldest history
//! (marked trimmed). A session with no old runtime to read (its host was
//! gone, or it had no terminal) restarts with an empty terminal, as a tmux
//! session whose terminal was gone does; so does one whose host ended while
//! the supervisor ran (its runtime has ended, and only a running runtime
//! gives a seed: the main session's DECISION, recorded in SPEC_impl).
//!
//! ## Viewers
//!
//! A viewer attached to the old host was detached before the relaunch
//! (`Supervisor::detach_for_restart`, "session restarted"), exactly as for
//! tmux, and attaches again to whatever the session's terminal is then: the
//! new host's grid stream, whose history is the seed. Nothing streams from
//! the old host after the restart.
//!
//! ## Ending the old host, and what a failure leaves
//!
//! The old host is ended ([`Supervisor::end_launch_host`]: its runtime's
//! `Shutdown`, else the verified signal to the process the runtime adopted;
//! only on a platform that named no peer, so that no process was adopted, a
//! host at its socket whose tag names this session) once the new launch is
//! published, and its launch records are removed once it is confirmed ended.
//! That is the success path and the one where only the confirmation's write
//! failed (the new host is then published as launching). Ending it before the
//! reply keeps the session's Delete out meanwhile; the reply can wait for a
//! wedged old host up to the runtime's 15-second `Shutdown` bound plus the
//! verified signal's graces. Until then the old host is the session's terminal:
//! a restart that fails definitively puts it back, with its screen and history,
//! and the row's restored `terminal_address` keeps naming it. A crash leaves
//! the next start enough to converge (`host_handover`): before the replacement
//! has a launch record, the old host is taken back as the row's terminal by the
//! first start that can read it (one that cannot adopts nothing and leaves it
//! for the next); after the new launch started, the reload confirms it and ends
//! the old host as superseded. A failed end is logged and left to that same
//! rule or to the session's Delete, which finds the host by its launch record:
//! the restart itself has succeeded.
//!
//! ## From a tmux pane
//!
//! A Restart of a session whose agent is in tmux (or of a tmux row whose
//! session holds no terminal) starts the new launch in a host like any
//! other, so every Restart ends in a host. The
//! session's agent is stopped in tmux as for any Restart (its pane stays,
//! dead, under `remain-on-exit`), and its pane's history is read for the seed
//! ([`Supervisor::tmux_restart_seed`]): `capture-pane` of the newest rows
//! above the visible screen that a seed can hold at the pane's size (never
//! the screen, which is the previous frame), printed into a scratch emulator
//! at the pane's size and scrollback in tmux capture mode (each line's style
//! carried into the next, as tmux writes styles only where they change, and
//! line drawing shifted to DEC Special Graphics, as tmux assumes) and
//! encoded as a host's seed is, so the new host starts at the pane's size
//! with the old scrollback above a blank screen in default modes. Two things
//! are not reconstructed (the main session's DECISION): a hyperlink spanning
//! rows keeps only its first row, since tmux's capture closes a link at each
//! row's end and does not reopen it; and tmux writes a tab as a literal tab,
//! which the new terminal expands at default tab stops, so custom tab stops
//! are not reproduced. The generation claim writes the host kind into the
//! row with the new generation (`SessionStore::begin_relaunch_named`), since
//! a host can start for it from then on and a supervisor that dies before
//! the confirmation must look for the launch by its tag, as for a create.
//! The agent resumes its conversation through its ordinary resume command,
//! as in tmux.
//!
//! Once the new launch is published, the old agent window is removed by its
//! recorded pane (identity-checked in tmux, so a pane that is not this
//! session's any more is left alone), and the row's `terminal_address` names
//! the host's socket, so nothing records the old pane. The window is removed
//! only when the new host started with lines of its history
//! ([`RestartSeed::holds_history`]; a seed can exist and hold none): otherwise
//! it may hold the only copy, and is left to the session's Delete. The
//! session's tmux tabs stay in its tmux session, which the row still names,
//! until they are closed; new tabs are hosts. Nothing about the agent then
//! depends on that tmux session: when its last tab closes, tmux ends it, and
//! the agent's liveness, Stop, Restart and Delete ask the host alone.
//!
//! What a failure leaves, beside what it leaves for a host agent:
//!
//! - **A replacement that does not start** is unwound definitively, as
//!   below, and the abort restores the tmux kind with the old pane
//!   (`PriorRun::terminal_kind`): the session is the tmux session it was,
//!   its stopped agent's window and history kept, and a later Restart tries
//!   the move again. One that cannot be confirmed ended keeps the host row
//!   launching with no terminal (both kinds' leftovers stay for Delete: the
//!   host by its record, the old window in the session's tmux session).
//! - **A history that cannot be read** (a capture that runs out of
//!   [`TMUX_SEED_CAPTURE_TIME`] twice, the second time for a quarter of the
//!   rows; an answer that will not parse; a pane whose size keeps changing
//!   while it is read; an encode that fails) refuses the Restart before the
//!   claim, as an orderly stop does: the history is not given up for a
//!   failure a retry may not have. Since such a failure can repeat for a
//!   given pane, the refusal says that Clone or Replace also move the
//!   session ([`SEED_REFUSAL_WAY_OUT`]). A pane not this session's any more,
//!   or whose size leaves no room for a line of history in a seed, gives a
//!   seed without history and keeps its window. A capture over
//!   [`TMUX_SEED_CAPTURE_LIMIT`] keeps its newest lines and marks the history
//!   trimmed.
//! - **An orderly stop** that begins while the seed is made cancels it and
//!   refuses the Restart, with the agent stopped and the row still a tmux row
//!   naming its pane. One that begins later is a host restart's: the new
//!   launch goes on, and is the next supervisor's to adopt by its tag.
//! - **A crash** before the generation claim leaves the stopped tmux
//!   session as it was (what a refusal before the claim leaves), which the
//!   next supervisor's Restart moves. After the claim and before the new
//!   host's launch record, the row is a host row with no host and no record,
//!   so the next start finds no terminal for it (there is no earlier host to
//!   fall back on) and leaves it launching (listing as unknown); its next
//!   Restart starts an empty host, and the old window, dead, stays in the
//!   tmux session until Delete. After the new host started, the next start
//!   adopts it by its tag and confirms it, as for a host agent; the old
//!   window, whose pane nothing recorded any more, also stays until Delete.
//!   Such a window shows nowhere (its agent marker keeps it out of the tab
//!   strip) and holds no program.
//! - **A Delete** that won the race leaves the new host ended as for a host
//!   agent, and the tmux side to Delete's own tmux teardown.
//!
//! ## A replacement that fails
//!
//! A replacement that does not start is unwound like a create's
//! ([`Supervisor::unwind_failed_host_relaunch`]): a host the start may
//! have left is ended, the launch's files and process tree are cleaned up,
//! and the failure is definitive, so the previous run's record and terminal
//! come back (under the orderly stop's fence, as any publication). Only a
//! replacement host that cannot be confirmed ended keeps the launching
//! record, as an ambiguous tmux failure does; the session then holds no
//! terminal, the old runtime is stopped, and the hosts of both generations
//! stay reachable by their launch records (Delete), and by their tags (the
//! next startup).

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use farhelm_proto::{ErrorKind, SessionInfo, SessionLaunch, TabInfo};
use tracing::{info, warn};

use super::host_launch::{HostLeft, HostSpawnFailure, SpawnedInHost};
use super::{
    FarhelmArgsMode, RELAUNCH_COLS, RELAUNCH_ROWS, RelaunchFailure, Relaunched, RequestError,
    SessionEntry, ShellSource, SimulatedCrash, Supervisor, truncate_for_error,
};
use crate::agent_kind::IntegrationSnapshot;
use crate::hosts::HostTerminal;
use crate::launch::LaunchArtifacts;
use crate::service::sweep::{ScopeKillFailure, ScopeUnits, SweepTarget, reap_process_tree};
use crate::service::terminals::{Terminal, TmuxTerminal};
use crate::store::{LastOutcome, Transition};

/// How long the restart waits for the old host to have settled before it
/// encodes the seed: its runtime caught up with the host's journal (a host
/// recovered at a supervisor's start replays its journal on its own), and
/// its exit record processed (the record comes after the old program's
/// drained output, so the program's last words are in). A host that is gone
/// is settled at once. When the bound runs out the seed is taken from what
/// the emulator holds then, and the restarted terminal lacks whatever had
/// not been processed; that is logged.
const SEED_SETTLE: Duration = Duration::from_secs(5);

/// The points inside a host agent's Restart at which a test can make it
/// fail ([`HostRestartFault`]).
///
/// All but [`Self::Confirmation`] are points where a real supervisor crash
/// leaves both generations' state in a shape the next startup must converge
/// from (the plan's interruptions at replacement and at the old host's
/// shutdown, and the reviews' claim window).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostRestartStage {
    /// The generation is claimed (the row is `Launching` at the new generation
    /// with no `terminal_address`), and nothing of the new launch exists yet:
    /// no spec, no launch record.
    GenerationClaimed,
    /// The new host runs and is adopted, its launch record written; the row
    /// is still `Launching` for the new generation with no `terminal_address`.
    ReplacementStarted,
    /// Not a crash: an error here stands for the confirmation's database
    /// write failing, and the restart goes on as it does then.
    Confirmation,
    /// A replacement that did not start has been cleaned up (its host
    /// ended, its spec and launch record removed, its process tree swept),
    /// and the previous run's record is not yet restored.
    ReplacementUnwound,
    /// The new launch is confirmed (the row names the new host's socket)
    /// and published; the old host still runs (for a move from tmux, the
    /// old agent window is still there).
    BeforeOldHostEnded,
    /// Not a crash: for a move from tmux, an error here stands for the old
    /// agent window's removal failing, and the restart goes on as it does
    /// then (logged, the window left for Delete).
    OldWindowRemoval,
}

/// A failure injected at one of [`HostRestartStage`]'s points, for tests.
///
/// At a crash stage, returning an error makes the restart return at once
/// with no cleanup, as a supervisor killed there would: before publication
/// the failure is ambiguous (nothing durable is restored, both hosts keep
/// running), after it the restart reports a published failure and leaves the
/// old host running. A test then drops the supervisor and starts the next one
/// on the same state directory. At [`HostRestartStage::Confirmation`] the
/// error is the confirmation write's. Returning `Ok` lets the restart go on,
/// which a test uses to do something at that point instead, such as deleting
/// the session's row to race a Delete. Production installs none.
pub type HostRestartFault = Arc<dyn Fn(HostRestartStage) -> anyhow::Result<()> + Send + Sync>;

/// The hard bound on how much of a tmux pane's history capture a Restart
/// into a host keeps ([`Supervisor::tmux_restart_seed`]): twice the host's
/// `MAX_SNAPSHOT`, newest lines first.
///
/// What is captured is bounded by rows (only those a seed holds at the pane's
/// size, `seed_history_capacity`), which at ordinary widths is the whole
/// 12,000-line scrollback, so this is reached by dense true-color histories
/// at ordinary widths too: a review measured 41.7 MB for 11,251 rows of 200
/// columns in per-cell true color. The seed's encoder would leave most of
/// such rows out anyway: a seed holds at most `MAX_SNAPSHOT` of encoded
/// history, and the encoding (styled text, one escape sequence per change of
/// style) is about as dense as tmux's capture. Over it, the newest whole lines are kept
/// and the seed says its history was trimmed; a style or line-drawing shift
/// set in a line left out then shows only from its next change. The
/// capture is compacted once it holds twice this, so the supervisor holds up
/// to about 64 MiB of it at once, beside tmux's own copy in its server.
const TMUX_SEED_CAPTURE_LIMIT: usize = 2 * terminal_host::protocol::MAX_SNAPSHOT;

/// How long a Restart into a host waits for tmux to capture the pane's
/// history (`TmuxDriver::capture_pane_history`), instead of the 2 s the
/// status captures get.
///
/// A whole styled history is a capture tmux builds in its server before it
/// writes any of it: a review measured 2.5 s for 11,251 rows of 200 columns
/// in per-cell true color on a quiet Linux machine, past the status
/// captures' budget, and a loaded machine or a slower Mac takes longer. A
/// capture that runs out of time is tried once more for fewer rows and then
/// refuses the Restart (the history is not given up), so the bound is set
/// well above that, at eight times the measurement. It bounds this
/// Restart's wait, not tmux's work: tmux renders the capture inside its
/// server, serving no other client meanwhile, and killing the capture's
/// client (at this bound, an orderly stop, or a request that goes away)
/// does not cut that short, so other terminals' tmux calls can stall for as
/// long as the capture takes. The rows taken bound that.
const TMUX_SEED_CAPTURE_TIME: Duration = Duration::from_secs(20);

/// The share of the rows a capture that ran out of time is tried again
/// for: the newest quarter. A capture's time grows with its rows, so a
/// quarter finishes where the whole nearly did, and a pane too slow to
/// capture whole moves with its newest history rather than not at all.
const TMUX_SEED_RETRY_SHARE: usize = 4;

/// What a refused move from tmux tells the user to do when retrying does not
/// help: the history read can keep failing for a given pane (tmux too slow
/// for it, say), and Restart takes the same path every time.
const SEED_REFUSAL_WAY_OUT: &str = "If this keeps happening, Clone or Replace also start the \
    session's agent in a new terminal (without this history), or Delete removes the session.";

/// A failure a test can make a move's history read from tmux have
/// (`FaultHooks::tmux_seed_fault`), in place of the read itself, so that it
/// goes through the same handling as a real one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TmuxSeedFault {
    /// The read fails (an answer that will not parse, say).
    Fails,
    /// The capture runs out of time.
    TimesOut,
}

/// What a Restart's new host starts with: the old terminal's history seed,
/// read before the generation is claimed ([`Supervisor::restart_seed`] for a
/// host, [`Supervisor::tmux_restart_seed`] for a tmux pane), and the old
/// terminal's size.
pub(super) struct RestartSeed {
    /// The initial snapshot; `None` when there is no old history to keep
    /// (a host's emulator lost, a tmux pane not this session's any more or
    /// too large to seed), which starts the new terminal empty.
    blob: Option<Vec<u8>>,
    /// Whether the snapshot holds lines of the old history (a snapshot can
    /// exist and hold none). Read for a move from tmux only; a host seed
    /// sets it with its snapshot.
    history: bool,
    cols: u16,
    rows: u16,
}

impl RestartSeed {
    /// Whether the new terminal starts with lines of the old one's history.
    /// A move from tmux removes the old agent window only then: otherwise
    /// that window may hold the only copy of the history.
    pub(super) fn holds_history(&self) -> bool {
        self.blob.is_some() && self.history
    }
}

/// The terminal a Restart's new host replaces, which the restart ends once
/// the new launch is published ([`Supervisor::relaunch_into_host`]).
#[derive(Clone, Copy)]
pub(super) enum Replacing<'a> {
    /// The session's previous host: a host agent's Restart.
    Host(&'a HostTerminal),
    /// The old agent window of a tmux session that this Restart moves into
    /// a host: the session's recorded pane, which the Restart's probe found
    /// still this session's, and whose program it stopped. Removed with its
    /// window once the new launch is published (module docs, "From a tmux
    /// pane").
    TmuxWindow(&'a TmuxTerminal),
    /// Nothing: a host row whose session holds no terminal, or a tmux
    /// session whose recorded pane is gone (or another session's now).
    Nothing,
}

impl<'a> Replacing<'a> {
    /// The previous host, when this replaces one.
    fn host(self) -> Option<&'a HostTerminal> {
        match self {
            Replacing::Host(host) => Some(host),
            Replacing::TmuxWindow(_) | Replacing::Nothing => None,
        }
    }
}

/// Sets a cancel flag when dropped: a seed's encode on the blocking pool,
/// which nothing interrupts from outside, then gives up within a row
/// whatever ends the future that waits for it (a request cancelled, the
/// process's runtime dropping its tasks at exit).
struct CancelOnDrop(Arc<AtomicBool>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

impl Supervisor {
    /// The host arm of `relaunch_into_terminal`: start the new launch of
    /// session `entry` in a new terminal host seeded with the old terminal's
    /// history, confirm and publish it, then end the old terminal,
    /// `replacing` (module docs).
    ///
    /// `replacing` is the session's host runtime when it has one, the old
    /// agent window of a tmux session this Restart moves into a host, or
    /// nothing (a host row whose session holds no terminal: its host was
    /// found gone, or a create was kept as a launching record; a tmux
    /// session whose pane is gone), which restarts with an empty terminal
    /// unless `seed` says otherwise. The other arguments are
    /// `relaunch_into_terminal`'s:
    /// the selected launch and its snapshot, the claimed generation and its
    /// scope, the resume argv, the verified working directory, and the tab
    /// strip read before anything happened, which a new agent host leaves
    /// as it was. `seed` is `old`'s history and size, read before the
    /// generation was claimed ([`Self::restart_seed`]); without one the new
    /// terminal starts empty at the default relaunch size.
    ///
    /// Failures say what the caller may restore, as every relaunch failure
    /// does ([`RelaunchFailure`]).
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn relaunch_into_host(
        &self,
        entry: &SessionEntry,
        replacing: Replacing<'_>,
        launch: &SessionLaunch,
        snapshot: &IntegrationSnapshot,
        generation: i64,
        scope: Option<String>,
        argv: Vec<String>,
        launch_cwd: String,
        tabs: Vec<TabInfo>,
        seed: Option<RestartSeed>,
    ) -> Result<SessionInfo, RelaunchFailure> {
        let id = entry.info.id.clone();
        if let Err(crash) = self.host_restart_fault(HostRestartStage::GenerationClaimed) {
            return Err(RelaunchFailure::ambiguous(crash));
        }
        let RestartSeed {
            blob, cols, rows, ..
        } = seed.unwrap_or(RestartSeed {
            blob: None,
            history: false,
            cols: RELAUNCH_COLS,
            rows: RELAUNCH_ROWS,
        });
        // Read before anything starts, as the tmux arms read it: a
        // credential read failure leaves the previous terminal as it was.
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
        let spawned = self
            .spawn_agent_in_host(
                &id,
                &session_token,
                generation,
                argv,
                ShellSource::of(launch, crate::agent_kind::LaunchPhase::Resume),
                // The session's own snapshot, as for tmux: the kind the
                // session was created with decides the hook flags.
                snapshot,
                FarhelmArgsMode::of(launch, crate::agent_kind::LaunchPhase::Resume),
                &launch_cwd,
                cols,
                rows,
                scope.as_deref(),
                // A restart never prepares a checkout or types an initial prompt.
                None,
                None,
                blob,
            )
            .await;
        let SpawnedInHost {
            host,
            artifacts,
            hooked,
            spawning,
        } = match spawned {
            Ok(spawned) => spawned,
            Err(HostSpawnFailure::Spec(error)) => {
                return Err(RelaunchFailure::definitive(
                    error.context("publishing this restart's launch spec"),
                ));
            }
            // A simulated crash inside the start: return as a killed
            // supervisor would, restoring nothing durable.
            #[cfg(any(test, feature = "test-seams"))]
            Err(HostSpawnFailure::Interrupted(error)) => {
                return Err(RelaunchFailure::ambiguous(error));
            }
            Err(HostSpawnFailure::Host {
                artifacts,
                error,
                left,
            }) => {
                return Err(self
                    .unwind_failed_host_relaunch(&id, scope.as_deref(), left, &artifacts, error)
                    .await);
            }
        };
        if let Err(crash) = self.host_restart_fault(HostRestartStage::ReplacementStarted) {
            // As a dying supervisor's connections close: the new host keeps
            // running, and so does the old one, whose runtime the caller's
            // recovery stops.
            host.abort();
            return Err(RelaunchFailure::ambiguous(crash));
        }

        // Confirmed as a create confirms its host: the row moves to running
        // with the new socket's name in `terminal_address` (the plan's D-14),
        // fenced on this launch's generation.
        let terminal = Terminal::Host(host.clone());
        let address = terminal.recorded_address().to_string();
        let confirmed = match self.host_restart_fault(HostRestartStage::Confirmation) {
            Err(failed) => Err(failed),
            Ok(()) => {
                self.store
                    .transition(&id, generation, Transition::ConfirmRunning { address })
                    .await
            }
        };
        let outcome = match confirmed {
            Ok(Some(outcome)) => outcome,
            Ok(None) => {
                drop(spawning);
                return Err(self
                    .tear_down_relaunch_deleted_meanwhile(
                        &id,
                        scope.as_deref(),
                        &host,
                        replacing.host(),
                        &artifacts,
                    )
                    .await);
            }
            Err(error) => {
                // The agent runs and this process could not record it: the
                // new host is published as launching so the session stays
                // reachable (the tmux arm's reasoning). The old terminal is
                // ended all the same: its history is in the new host's seed
                // and its program was stopped, and leaving an old host to the
                // next start would leave it running for good if the new host
                // ended first (the row would then never show a confirmed
                // later launch).
                warn!(
                    session = %id, error = %format!("{error:#}"),
                    "could not confirm this relaunch durably; publishing it as launching so the \
                     agent stays reachable"
                );
                self.publish_relaunched(
                    entry,
                    generation,
                    Relaunched {
                        terminal,
                        scope,
                        outcome: LastOutcome::Launching,
                        hooked,
                        tabs,
                    },
                )
                .await;
                drop(spawning);
                self.end_replaced(&id, replacing).await;
                return Err(RelaunchFailure::published(error.context(
                    "confirming the relaunch in the database; the agent is running and the \
                     session lists as unknown until the next observation",
                )));
            }
        };
        if outcome != LastOutcome::Running {
            // Another writer moved this generation's outcome first: published
            // as what committed, never as a fabricated `Running`.
            warn!(
                session = %id, outcome = ?outcome,
                "this relaunch's confirmation lost a race; publishing what committed"
            );
        }
        info!(session = %id, socket = %host.socket().display(), "session restarted in a new terminal host");
        let info = self
            .publish_relaunched(
                entry,
                generation,
                Relaunched {
                    terminal,
                    scope,
                    outcome,
                    hooked,
                    tabs,
                },
            )
            .await;
        // The new host is on the session map now, where anything that must
        // not take it over finds it; released before the old host is ended,
        // which may take the lock exclusively.
        drop(spawning);
        if let Err(crash) = self.host_restart_fault(HostRestartStage::BeforeOldHostEnded) {
            if let Some(old) = replacing.host() {
                old.abort();
            }
            return Err(RelaunchFailure::published(crash));
        }
        self.end_replaced(&id, replacing).await;
        self.restart_reply(info).await
    }

    /// End the terminal a published restart of `session_id` replaced: the
    /// previous host ([`Self::end_superseded_host`]), or the old tmux agent
    /// window of a session this Restart moved into a host. A failure is
    /// logged and leaves the restart a success: an old host is left to
    /// Delete and the next start; an old window holds no program (the
    /// Restart stopped it) and no tab (its agent marker keeps it out of the
    /// tab strip), and goes with the session's tmux session at Delete.
    async fn end_replaced(&self, session_id: &str, replacing: Replacing<'_>) {
        match replacing {
            Replacing::Host(old) => self.end_superseded_host(session_id, old).await,
            Replacing::TmuxWindow(old) => {
                // Identity-checked in tmux itself (`TmuxDriver::kill_window`):
                // a pane that is no longer this session's is left alone.
                let removed = match self.host_restart_fault(HostRestartStage::OldWindowRemoval) {
                    Err(failed) => Err(failed),
                    Ok(()) => self.tmux.kill_window(&old.tmux_name, &old.pane).await,
                };
                if let Err(error) = removed {
                    warn!(
                        session = %session_id, pane = %old.pane, error = %format!("{error:#}"),
                        "the session now runs in a terminal host, but its old tmux agent window \
                         could not be removed; it holds no program and goes with the session's \
                         Delete"
                    );
                }
            }
            Replacing::Nothing => {}
        }
    }

    /// The Restart seed of `old`, the host terminal a Restart of session
    /// `id` replaces, and its size: its history above a blank screen in
    /// default modes (module docs), read before the generation is claimed so
    /// that a crash meanwhile leaves the session exactly as it was.
    ///
    /// Waits up to [`SEED_SETTLE`] for the runtime to have settled (caught
    /// up with its journal, and the exit record processed, or the host
    /// gone), then asks the runtime for the seed (`HostTerminal::history_seed`,
    /// encoded on the blocking pool). A host that is gone gives none: its
    /// runtime has ended, and the new terminal starts empty, as after a
    /// supervisor restart (the main session's DECISION, logged with the
    /// reasons in SPEC_impl's "Restarting a host agent"). The old runtime is
    /// still on the session map while the seed is encoded, so an orderly stop
    /// that begins meanwhile cancels the encode as it closes every runtime;
    /// a seed missing while the supervisor is stopping refuses the Restart,
    /// having stopped the agent and changed nothing else. A seed missing for
    /// any other reason (an encoder that failed, an emulator lost) starts
    /// the new terminal empty, logged, rather than refuse over its history.
    pub(super) async fn restart_seed(
        &self,
        id: &str,
        old: &HostTerminal,
    ) -> anyhow::Result<RestartSeed> {
        let settled = |host: &HostTerminal| {
            let status = host.status();
            match status.liveness {
                crate::hosts::Liveness::Gone | crate::hosts::Liveness::Unavailable { .. } => true,
                crate::hosts::Liveness::Running { .. } | crate::hosts::Liveness::Exited { .. } => {
                    status.caught_up
                        && matches!(
                            status.evidence(),
                            crate::hosts::HostEvidence::Owned { dead: true, .. }
                        )
                }
            }
        };
        let mut status = old.subscribe();
        let in_time = tokio::time::timeout(SEED_SETTLE, async {
            while !settled(old) {
                if status.changed().await.is_err() {
                    break;
                }
            }
        })
        .await;
        if in_time.is_err() && !settled(old) {
            warn!(session = %id, bound = ?SEED_SETTLE, status = ?old.status(),
                "the old terminal had not settled when the restart read its history; the \
                 restarted terminal keeps what had been processed by then");
        }
        let size = old.size();
        let gone = matches!(old.liveness(), crate::hosts::Liveness::Gone);
        let seed = if gone {
            None
        } else {
            old.history_seed(terminal_host::protocol::MAX_SNAPSHOT)
                .await
        };
        let blob = match seed {
            Some(seed) => {
                if seed.dropped_history_lines > 0 {
                    info!(session = %id, dropped = seed.dropped_history_lines,
                        "the restart's history seed left out the oldest lines to fit");
                }
                Some(seed.blob)
            }
            None if self.stopping.load(std::sync::atomic::Ordering::SeqCst) => {
                return Err(RequestError::new(
                    ErrorKind::Internal,
                    "the supervisor is stopping, so this session's agent, now stopped, was not \
                     restarted; restart it again once Farhelm is running",
                )
                .into());
            }
            None if gone => {
                info!(session = %id,
                    "the old terminal's host is gone, so the restarted terminal starts empty");
                None
            }
            None => {
                warn!(session = %id,
                    "the old terminal's history could not be read, so the restarted terminal \
                     starts empty");
                None
            }
        };
        Ok(RestartSeed {
            history: blob.is_some(),
            blob,
            cols: size.cols,
            rows: size.rows,
        })
    }

    /// The Restart seed of `old`, the tmux pane a Restart of session `id`
    /// moves into a terminal host, and the pane's size: its history above a
    /// blank screen in default modes (module docs, "From a tmux pane"), read
    /// after the agent was stopped and before the generation is claimed, as
    /// [`Self::restart_seed`] reads a host's.
    ///
    /// The pane's size decides how many history rows a seed can hold
    /// (`farhelm_terminal_emulator::seed_history_capacity`: what a restore
    /// keeps within its cell budget, at most the scrollback cap); only those
    /// newest rows are read from tmux
    /// ([`crate::tmux::TmuxDriver::capture_pane_history`], within
    /// [`TMUX_SEED_CAPTURE_TIME`] and [`TMUX_SEED_CAPTURE_LIMIT`]) and
    /// printed into a scratch emulator of the pane's size and that
    /// scrollback, whose history is encoded as the seed
    /// (`farhelm_terminal_emulator::history_seed_from_lines_until`, in tmux
    /// capture mode), fitted to `MAX_SNAPSHOT`, on the blocking pool.
    /// Terminal output stays in memory throughout.
    ///
    /// The tmux pane is the only copy of the history, and the move removes
    /// it once the new launch is published, so a history that cannot be read
    /// refuses the Restart rather than start the new terminal empty (unlike
    /// a host's seed, whose emulator a retry cannot bring back): an answer
    /// that will not parse, a pane that keeps changing size while it is
    /// read, an encode that fails. A capture that runs out of time is tried
    /// once more for a quarter of the rows ([`TMUX_SEED_RETRY_SHARE`]; the
    /// history is marked trimmed when that leaves rows out), so a pane too
    /// slow to capture whole still moves with its newest history; only a
    /// second timeout refuses. The
    /// agent is stopped by then and the row is still the tmux row naming its
    /// pane, so a retry starts from the same place, and the refusal says what
    /// else moves the session if it keeps failing ([`SEED_REFUSAL_WAY_OUT`]).
    /// An orderly stop that begins meanwhile ([`Supervisor::stop_began`])
    /// refuses it the same way and stops waiting for the capture (its tmux
    /// client is killed) or cancels the encode, so the stop never waits for
    /// either; a request that goes away does the same.
    ///
    /// A seed without history comes back, never a refusal, for a pane that
    /// is not this session's any more (it holds nothing of this session's)
    /// and for a pane whose size leaves no room for history in a seed
    /// (`seed_history_capacity` refuses it, or allows no line: screens at or
    /// near the restore budget, which a host runtime could hardly hold
    /// either). The new terminal then starts empty, at the default size, and
    /// the old window is left in place ([`RestartSeed::holds_history`]).
    pub(super) async fn tmux_restart_seed(
        &self,
        id: &str,
        old: &TmuxTerminal,
    ) -> anyhow::Result<RestartSeed> {
        let cancelled = Arc::new(AtomicBool::new(false));
        let _cancel_on_drop = CancelOnDrop(Arc::clone(&cancelled));
        let refused = |why: String| {
            anyhow::Error::new(RequestError::new(
                ErrorKind::Internal,
                format!(
                    "this session's agent, now stopped, was not restarted, because {why}; its \
                     terminal and history are as they were, so restart it again. \
                     {SEED_REFUSAL_WAY_OUT}"
                ),
            ))
        };
        // The rows a seed holds at a size, `None` when it holds none.
        let capacity = |cols: u16, rows: u16| {
            farhelm_terminal_emulator::seed_history_capacity(
                farhelm_terminal_emulator::Size {
                    columns: cols,
                    lines: rows,
                },
                crate::hosts::terminal::DEFAULT_SCROLLBACK_LINES,
            )
            .filter(|lines| *lines > 0)
        };
        let without_history = RestartSeed {
            blob: None,
            history: false,
            cols: RELAUNCH_COLS,
            rows: RELAUNCH_ROWS,
        };
        let seeding = async {
            if let Some(gate) = self.seams.faults.tmux_seed_gate() {
                gate().await;
            }
            let mut read = Err(crate::tmux::PaneHistoryError::NotThisSessions);
            for share in [1, TMUX_SEED_RETRY_SHARE] {
                read = match self.seams.faults.tmux_seed_fault().map(|fault| fault()) {
                    Some(Err(TmuxSeedFault::Fails)) => Err(crate::tmux::PaneHistoryError::Failed(
                        anyhow::anyhow!("a read failure the test seam injected"),
                    )),
                    Some(Err(TmuxSeedFault::TimesOut)) => {
                        Err(crate::tmux::PaneHistoryError::TimedOut(anyhow::anyhow!(
                            "a capture timeout the test seam injected"
                        )))
                    }
                    Some(Ok(())) | None => {
                        self.tmux
                            .capture_pane_history(
                                &old.tmux_name,
                                &old.pane,
                                |cols, rows| {
                                    capacity(cols, rows).map_or(0, |lines| lines / share) as u64
                                },
                                TMUX_SEED_CAPTURE_LIMIT,
                                TMUX_SEED_CAPTURE_TIME,
                            )
                            .await
                    }
                };
                match &read {
                    Err(crate::tmux::PaneHistoryError::TimedOut(error)) if share == 1 => {
                        warn!(session = %id, error = %format!("{error:#}"),
                            "capturing the tmux pane's history ran out of time; trying once more \
                             for its newest quarter");
                    }
                    _ => break,
                }
            }
            let history = match read {
                Ok(history) => history,
                Err(crate::tmux::PaneHistoryError::NotThisSessions) => {
                    warn!(session = %id, pane = %old.pane,
                        "the session's tmux pane is no longer its own, so its new terminal host \
                         starts empty");
                    return Ok(without_history);
                }
                Err(
                    crate::tmux::PaneHistoryError::Failed(error)
                    | crate::tmux::PaneHistoryError::TimedOut(error),
                ) => {
                    return Err(refused(format!(
                        "its tmux pane's history could not be read ({error:#})"
                    )));
                }
            };
            let (cols, rows) = (history.cols, history.rows);
            let Some(scrollback) = capacity(cols, rows) else {
                warn!(session = %id, cols, rows,
                    "the session's tmux pane is too large for a terminal host to hold its \
                     history, so its new terminal host starts empty at the default size");
                return Ok(without_history);
            };
            if history.cut {
                info!(session = %id, rows = scrollback,
                    "the tmux pane's history was longer than this seed takes; its oldest \
                     lines are left out");
            }
            let cancel = farhelm_terminal_emulator::Cancel::new(Arc::clone(&cancelled), None);
            let encoded = tokio::task::spawn_blocking(move || {
                farhelm_terminal_emulator::history_seed_from_lines_until(
                    farhelm_terminal_emulator::Size {
                        columns: cols,
                        lines: rows,
                    },
                    scrollback,
                    history.lines(),
                    terminal_host::protocol::MAX_SNAPSHOT,
                    farhelm_terminal_emulator::LineStyles::Carried,
                    history.cut,
                    &cancel,
                )
            })
            .await;
            match encoded {
                Ok(Ok(seed)) => {
                    if seed.snapshot.dropped_history_lines > 0 {
                        info!(session = %id, dropped = seed.snapshot.dropped_history_lines,
                            "the restart's history seed left out the oldest lines to fit");
                    }
                    Ok(RestartSeed {
                        blob: Some(seed.snapshot.blob),
                        history: seed.history_lines > 0,
                        cols,
                        rows,
                    })
                }
                // Only this function's own cancel fires it, once it stops
                // waiting; refused like any seed that could not be made.
                Ok(Err(stopped)) => Err(refused(format!(
                    "its tmux pane's history seed stopped early ({stopped})"
                ))),
                Err(error) => Err(refused(format!(
                    "its tmux pane's history seed failed ({error})"
                ))),
            }
        };
        // Enabled before the flag is read, so a stop that begins between the
        // two still wakes this.
        let stop = self.stop_began.notified();
        tokio::pin!(stop);
        stop.as_mut().enable();
        let stopping = || {
            RequestError::new(
                ErrorKind::Internal,
                "the supervisor is stopping, so this session's agent, now stopped, was not \
                 restarted; restart it again once Farhelm is running",
            )
        };
        if self.stopping.load(Ordering::SeqCst) {
            return Err(stopping().into());
        }
        tokio::select! {
            seed = seeding => seed,
            () = stop => {
                cancelled.store(true, Ordering::SeqCst);
                info!(session = %id,
                    "an orderly stop began while the restart read the tmux pane's history; \
                     refusing the restart");
                Err(stopping().into())
            }
        }
    }

    /// Settle every launch of session `session_id`'s agent that its launch
    /// records say may have started a host after the host its terminal holds
    /// (`held`: those of later launch generations than its tag's, or every
    /// one when it holds no host or its tag cannot be read; never a tab's
    /// record, whose host the Restart leaves as it is), before a Restart
    /// stops the agent and claims a generation: the sweep roots of programs
    /// found running in such hosts, which the caller roots into its stop of
    /// the agent, or why the restart is refused.
    ///
    /// A host start can outlive the request (or the supervisor) that made it:
    /// its recorded first stage still holds the whole spec, its socket not
    /// yet bound, and nothing the restart's sweep can reach carries a marker
    /// or a scope yet. Released after the next launch started, it would start
    /// a second agent on the same conversation. So each record is settled as a
    /// retried create settles its own ([`Supervisor::settle_launch_record`]):
    /// its first stage is waited for and then ended with the verified signal,
    /// and a second stage forked before that is waited out. Then:
    ///
    /// - nothing at its socket: the record (and a socket file a killed host
    ///   left) is removed;
    /// - a host tagged for this session: its program, when it runs, is pinned
    ///   over the connection as a sweep root; the host is left as it was
    ///   (an earlier generation's host is ended at a later start as
    ///   superseded, or by Delete through its record);
    /// - a host tagged for another session: not this session's, left alone;
    /// - anything that cannot be told (a first stage not confirmed ended, a
    ///   record that cannot be read, a host that does not answer usefully or
    ///   whose running program cannot be pinned): an error, which refuses
    ///   the Restart before anything is stopped. It is worth retrying, since
    ///   most of these pass.
    ///
    /// Runs under the session's lifecycle claim, which keeps the session's
    /// own creates, Restarts, Deletes and re-adoption away from its hosts.
    /// Costs one directory listing when the session has no records beside
    /// its current host's.
    pub(super) async fn settle_outstanding_launches(
        &self,
        session_id: &str,
        held: Option<&Terminal>,
    ) -> anyhow::Result<Vec<(u32, u64)>> {
        let (held, held_generation) = match held {
            Some(Terminal::Host(host)) => (
                host.socket().file_name().map(ToOwned::to_owned),
                match host.tag().map(|tag| &tag.role) {
                    Some(crate::hosts::HostRole::Agent { generation }) => Some(*generation),
                    Some(crate::hosts::HostRole::Tab { .. }) | None => None,
                },
            ),
            Some(Terminal::Tmux(_)) | None => (None, None),
        };
        let mut roots = Vec::new();
        for path in self.launch_records_of(session_id).await? {
            // Only the agent's launches after the held host's. An earlier one
            // belongs to a host a confirmed later launch replaced, whose
            // program the Restart that replaced it stopped, and which is ended
            // at a later start or by Delete: settling it here would refuse
            // every Restart for as long as such a host stays wedged. A tab's
            // record names a tab host, which runs no agent and which the
            // agent's Restart leaves as it is (`host_tabs`).
            let generation = match path
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(crate::launch::parse_launch_file_name)
            {
                Some((_, crate::launch::LaunchFileOf::Tab(_))) => continue,
                Some((_, crate::launch::LaunchFileOf::Launch(generation))) => Some(generation),
                None => None,
            };
            if let (Some(held), Some(generation)) = (held_generation, generation)
                && generation <= held
            {
                continue;
            }
            let refused = |why: String| {
                anyhow::Error::new(RequestError::new(
                    ErrorKind::Conflict,
                    format!(
                        "an earlier start of this session's terminal host (recorded in {}) could \
                         not be settled, so nothing was stopped or restarted; try again: {why}",
                        path.display()
                    ),
                ))
            };
            let record = match super::read_launch_record(&path).await {
                Ok(Some(record)) => record,
                Ok(None) => continue,
                Err(error) => return Err(refused(format!("{error:#}"))),
            };
            let socket = self.recorded_socket(&record)?;
            if socket.file_name() == held.as_deref() {
                continue;
            }
            match self
                .settle_launch_record(&record, &socket)
                .await
                .map_err(refused)?
            {
                super::AtRecordedSocket::Gone => {
                    self.remove_ended_socket(&socket).await;
                    if let Err(error) = tokio::fs::remove_file(&path).await
                        && error.kind() != std::io::ErrorKind::NotFound
                    {
                        warn!(path = %path.display(), %error,
                            "could not remove the launch record of a start that left nothing");
                    }
                }
                super::AtRecordedSocket::Answered {
                    mut connection,
                    info,
                } => match crate::hosts::HostTag::decode(&info.tag) {
                    Ok(tag) if tag.session_id == session_id => {
                        let (child, info) = crate::hosts::terminal::pin_child(&mut connection)
                            .await
                            .map_err(|error| refused(error.to_string()))?;
                        match child {
                            crate::hosts::terminal::Child::Running(root) => {
                                info!(session = %session_id, socket = %socket.display(),
                                    "an earlier start of this session's terminal host runs a \
                                     program; stopping it before the restart");
                                roots.push((root.pid, root.start));
                            }
                            crate::hosts::terminal::Child::Reaped => {}
                            crate::hosts::terminal::Child::Unpinned => {
                                return Err(refused(format!(
                                    "its program {} runs but could not be read",
                                    info.child_pid
                                )));
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(unreadable) => return Err(refused(unreadable.to_string())),
                },
                super::AtRecordedSocket::NoAnswer(error) => return Err(refused(error.to_string())),
                super::AtRecordedSocket::NoInfo { error, .. } => {
                    return Err(refused(error.to_string()));
                }
            }
        }
        Ok(roots)
    }

    /// The installed [`HostRestartFault`]'s answer for `stage`, as a
    /// simulated crash's (or, at [`HostRestartStage::Confirmation`], a failed
    /// write's) error, or `Ok` when none is installed (always in
    /// production).
    fn host_restart_fault(&self, stage: HostRestartStage) -> anyhow::Result<()> {
        match self.seams.faults.host_restart_fault() {
            Some(fault) => fault(stage).map_err(|error| match stage {
                HostRestartStage::Confirmation | HostRestartStage::OldWindowRemoval => error,
                HostRestartStage::GenerationClaimed
                | HostRestartStage::ReplacementStarted
                | HostRestartStage::ReplacementUnwound
                | HostRestartStage::BeforeOldHostEnded => error.context(SimulatedCrash),
            }),
            None => Ok(()),
        }
    }

    /// End `old`, the host a confirmed restart of `session_id` replaced,
    /// and remove the launch records naming it once it is confirmed ended.
    /// A host that cannot be confirmed ended is logged and left with its
    /// records: the session's Delete settles it by them, and the next
    /// startup ends it as superseded. Its runtime is stopped either way.
    async fn end_superseded_host(&self, session_id: &str, old: &HostTerminal) {
        if old.is_placeholder() {
            // A startup placeholder holds no runtime; Restart refuses an
            // unreadable host before it gets here. Its records stay for
            // Delete and the next startup.
            return;
        }
        match self
            .end_launch_host(Some(old), old.socket(), session_id)
            .await
        {
            Ok(()) => {
                self.remove_records_naming(session_id, old.socket()).await;
            }
            Err(why) => {
                old.abort();
                warn!(
                    session = %session_id, socket = %old.socket().display(), %why,
                    "the restart's previous terminal host may still run; its session's Delete or \
                     the next supervisor start ends it"
                );
            }
        }
    }

    /// Remove every launch record of `session_id` that names the socket at
    /// `socket`: once that host is confirmed ended, nothing is left for a
    /// record of it to fence. Every generation's record is read, since the
    /// one naming a host need not be the session's current generation's (a
    /// restart that failed definitively keeps the earlier host as the
    /// session's terminal under the new generation). Best effort: a record
    /// that cannot be read or removed is only logged, and its socket, which
    /// no host serves now, settles as gone at Delete.
    pub(super) async fn remove_records_naming(&self, session_id: &str, socket: &Path) {
        let records = match self.launch_records_of(session_id).await {
            Ok(records) => records,
            Err(error) => {
                warn!(session = %session_id, error = %format!("{error:#}"),
                    "could not list the launch records of an ended terminal host");
                return;
            }
        };
        let name = socket.file_name().and_then(|name| name.to_str());
        for path in records {
            match super::read_launch_record(&path).await {
                Ok(Some(record)) if Some(record.socket.to_string().as_str()) == name => {
                    if let Err(error) = tokio::fs::remove_file(&path).await
                        && error.kind() != std::io::ErrorKind::NotFound
                    {
                        warn!(path = %path.display(), %error,
                            "could not remove an ended terminal host's launch record");
                    }
                }
                Ok(_) => {}
                Err(error) => {
                    warn!(path = %path.display(), error = %format!("{error:#}"),
                        "could not read a launch record while removing an ended host's");
                }
            }
        }
    }

    /// Unwind a host relaunch whose start failed, once it is established
    /// that nothing of it runs, deciding from what the start itself could
    /// tell ([`HostLeft`]).
    ///
    /// A start that left nothing, or a host it may have left that is then
    /// confirmed ended ([`Supervisor::end_launch_host`], by the protocol
    /// or the verified signal, never a host another tag names), has its
    /// unconsumed spec, waypoint and launch record removed and its process
    /// tree reaped (`scope` is the failed launch's own unit, which the scope
    /// wrapper may have created): definitive, so the previous run's record
    /// and terminal come back. A host that cannot be confirmed ended, or a
    /// reap that cannot be confirmed, keeps the launching record and the
    /// launch record, ambiguous: an agent may run under the new
    /// generation.
    async fn unwind_failed_host_relaunch(
        &self,
        id: &str,
        scope: Option<&str>,
        left: HostLeft,
        artifacts: &LaunchArtifacts,
        error: anyhow::Error,
    ) -> RelaunchFailure {
        let error = error.context(format!(
            "relaunching session {} in a new terminal host",
            truncate_for_error(id)
        ));
        if let HostLeft::MayRemain { socket } = left
            && let Err(why) = self.end_launch_host(None, &socket, id).await
        {
            return RelaunchFailure::ambiguous(error.context(format!(
                "a terminal host may still be running at {} ({why}), so the launch is kept as a \
                 launching record rather than unwound",
                socket.display()
            )));
        }
        let mut error = artifacts
            .remove_unconsumed_relaunch_fail_closed(error)
            .await;
        if !artifacts.remove_host_record_best_effort().await {
            error =
                error.context("and the failed relaunch's launch host record could not be removed");
        }
        // The start's child may have run for a moment (a host that ended
        // before it could be adopted). Reap through its confirmed scope
        // when sufficient, otherwise through the portable sweep; ending
        // the host alone does not establish that its descendants are gone.
        if let Err(sweep) = reap_process_tree(
            &self.seams.scopes,
            ScopeUnits::recorded(scope.map(str::to_string)),
            None,
            id,
            &SweepTarget::AgentOnly,
            ScopeKillFailure::Warn,
        )
        .await
        {
            return RelaunchFailure::ambiguous(error.context(format!(
                "and the failed launch's process tree could not be swept ({sweep:#})"
            )));
        }
        if let Err(crash) = self.host_restart_fault(HostRestartStage::ReplacementUnwound) {
            return RelaunchFailure::ambiguous(crash);
        }
        RelaunchFailure::definitive(error)
    }

    /// Tear down a host relaunch whose session's row is gone (a delete
    /// committed while it ran): its agent's process tree, its new host, and
    /// the old host it was replacing, with their launch records once each
    /// is confirmed ended, and the launch's files. Nothing is left for a
    /// record to fence once the row is gone, so a host that cannot be
    /// confirmed ended is named in the error rather than kept.
    async fn tear_down_relaunch_deleted_meanwhile(
        &self,
        id: &str,
        scope: Option<&str>,
        new: &HostTerminal,
        old: Option<&HostTerminal>,
        artifacts: &LaunchArtifacts,
    ) -> RelaunchFailure {
        let mut teardown = Vec::new();
        // The tree first: ending a host hangs up its child and nothing
        // beyond it.
        if let Err(error) = reap_process_tree(
            &self.seams.scopes,
            ScopeUnits::recorded(scope.map(str::to_string)),
            None,
            id,
            &SweepTarget::AgentOnly,
            ScopeKillFailure::Warn,
        )
        .await
        {
            teardown.push(format!("the new agent's process tree ({error:#})"));
        }
        match self.end_launch_host(Some(new), new.socket(), id).await {
            Ok(()) => {
                artifacts.remove_host_record_best_effort().await;
            }
            Err(why) => teardown.push(format!(
                "its new terminal host at {} ({why})",
                new.socket().display()
            )),
        }
        if let Some(old) = old.filter(|old| !old.is_placeholder()) {
            match self.end_launch_host(Some(old), old.socket(), id).await {
                Ok(()) => self.remove_records_naming(id, old.socket()).await,
                Err(why) => teardown.push(format!(
                    "its previous terminal host at {} ({why})",
                    old.socket().display()
                )),
            }
        }
        artifacts
            .remove_deleted_relaunch_fail_closed(&mut teardown)
            .await;
        let message = if teardown.is_empty() {
            format!(
                "session {} was deleted while it was being relaunched, so the new agent was torn \
                 back down",
                truncate_for_error(id)
            )
        } else {
            // Never "torn back down" over a failure: something survives.
            format!(
                "session {} was deleted while it was being relaunched, and the new agent could \
                 NOT be fully torn back down: {}",
                truncate_for_error(id),
                teardown.join("; ")
            )
        };
        RelaunchFailure::ambiguous(RequestError::new(ErrorKind::Conflict, message).into())
    }
}
