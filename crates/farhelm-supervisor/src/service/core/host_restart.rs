//! Restarting an agent into a new terminal host (the plan's PR 21, the
//! goal's "A restart is a new host, and keeps the history").
//!
//! A terminal host runs one child for its whole life and has no message to
//! start another, so a Restart of an agent whose terminal is a host never
//! reuses that host: it starts a new one for the new launch generation,
//! through the same start a create uses (`host_launch`: the launch record,
//! the `host_scan` fence, adoption), and ends the old host once the new
//! launch is confirmed. A session with no terminal (its host ended, or its
//! old tmux terminal was ended at startup, `old_terminals`) restarts here
//! too, into a new, empty host. Restart's own frame is unchanged ([`Supervisor::relaunch`]: the
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
//! visible frame. A new host has neither by itself, so it is seeded with an initial snapshot holding the old
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
//! gone, or it had no terminal) restarts with an empty terminal; so does one
//! whose host ended while
//! the supervisor ran (its runtime has ended, and only a running runtime
//! gives a seed: the main session's DECISION, recorded in SPEC_impl).
//!
//! ## Viewers
//!
//! A viewer attached to the old host was detached before the relaunch
//! (`Supervisor::detach_for_restart`, "session restarted"), and attaches
//! again to whatever the session's terminal is then: the
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
//! ## A replacement that fails
//!
//! A replacement that does not start is unwound like a create's
//! ([`Supervisor::unwind_failed_host_relaunch`]): a host the start may
//! have left is ended, the launch's files and process tree are cleaned up,
//! and the failure is definitive, so the previous run's record and terminal
//! come back (under the orderly stop's fence, as any publication). Only a
//! replacement host that cannot be confirmed ended keeps the launching
//! record; the session then holds no
//! terminal, the old runtime is stopped, and the hosts of both generations
//! stay reachable by their launch records (Delete), and by their tags (the
//! next startup).

use std::path::Path;
use std::sync::Arc;
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
    /// and published; the old host still runs.
    BeforeOldHostEnded,
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

/// What a Restart's new host starts with: the old terminal's history seed,
/// read before the generation is claimed ([`Supervisor::restart_seed`]),
/// and the old terminal's size.
pub(super) struct RestartSeed {
    /// The initial snapshot; `None` when there is no old history to keep (a
    /// host's emulator lost), which starts the new terminal empty.
    blob: Option<Vec<u8>>,
    cols: u16,
    rows: u16,
}

impl Supervisor {
    /// `relaunch_into_terminal`'s work: start the new launch of session
    /// `entry` in a new terminal host seeded with the old terminal's
    /// history, confirm and publish it, then end the old host, `replacing`
    /// (module docs).
    ///
    /// `replacing` is the session's host runtime when it has one, or none (a
    /// session that holds no terminal: its host was found gone, a create was
    /// kept as a launching record, or its old tmux terminal was ended at
    /// startup), which restarts with an empty terminal unless `seed` says
    /// otherwise. The other arguments are
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
        replacing: Option<&HostTerminal>,
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
        let RestartSeed { blob, cols, rows } = seed.unwrap_or(RestartSeed {
            blob: None,
            cols: RELAUNCH_COLS,
            rows: RELAUNCH_ROWS,
        });
        // Read before anything starts: a credential read failure leaves the
        // previous terminal as it was.
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
                // The session's own snapshot: the kind the session was
                // created with decides the hook flags.
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
        let terminal = host.clone();
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
                        replacing,
                        &artifacts,
                    )
                    .await);
            }
            Err(error) => {
                // The agent runs and this process could not record it: the
                // new host is published as launching so the session stays
                // reachable. The old terminal is
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
            if let Some(old) = replacing {
                old.abort();
            }
            return Err(RelaunchFailure::published(crash));
        }
        self.end_replaced(&id, replacing).await;
        self.restart_reply(info).await
    }

    /// End the host a published restart of `session_id` replaced
    /// ([`Self::end_superseded_host`]). A failure is logged and leaves the
    /// restart a success: an old host is left to Delete and the next start.
    async fn end_replaced(&self, session_id: &str, replacing: Option<&HostTerminal>) {
        if let Some(old) = replacing {
            self.end_superseded_host(session_id, old).await;
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
            blob,
            cols: size.cols,
            rows: size.rows,
        })
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
        held: Option<&HostTerminal>,
    ) -> anyhow::Result<Vec<(u32, u64)>> {
        let (held, held_generation) = match held {
            Some(host) => (
                host.socket().file_name().map(ToOwned::to_owned),
                match host.tag().map(|tag| &tag.role) {
                    Some(crate::hosts::HostRole::Agent { generation }) => Some(*generation),
                    Some(crate::hosts::HostRole::Tab { .. }) | None => None,
                },
            ),
            None => (None, None),
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
                HostRestartStage::Confirmation => error,
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
