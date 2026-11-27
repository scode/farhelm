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
//! come back (under the orderly stop's fence, as any publication). An
//! unconfirmed host, scope or process sweep keeps the launching row. The
//! session then holds no terminal, the old runtime is stopped, and both
//! generations' hosts stay reachable through their records or tags.

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use farhelm_proto::{ErrorKind, SessionInfo, SessionLaunch, TabInfo};
use tracing::{info, warn};

use super::host_launch::{AtRecordedSocket, HostLeft, HostSpawnFailure, SpawnedInHost};
use super::{
    FarhelmArgsMode, RELAUNCH_COLS, RELAUNCH_ROWS, RelaunchFailure, Relaunched, RequestError,
    SessionEntry, ShellSource, SimulatedCrash, Supervisor, truncate_for_error,
};
use crate::agent_kind::IntegrationSnapshot;
use crate::hosts::HostTerminal;
use crate::launch::LaunchArtifacts;
use crate::service::sweep::{
    ScopeKillFailure, ScopeUnits, SweepTarget, reap_process_tree,
    reap_process_tree_with_podman_deadline, reap_recorded_scope,
};
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

/// What a Restart's new host starts with: the previous terminal's history seed,
/// read before the generation is claimed ([`Supervisor::restart_seed`]),
/// and the previous terminal's size.
pub(super) struct RestartSeed {
    /// The initial snapshot; `None` when there is no old history to keep (a
    /// host's emulator lost), which starts the new terminal empty.
    blob: Option<Vec<u8>>,
    cols: u16,
    rows: u16,
}

/// Evidence from settling recorded agent starts, carried into the process sweep.
/// A gone current host's record still distinguishes an unresolved launch from one that never started.
#[derive(Default)]
pub(super) struct SettledAgentLaunches {
    pub(super) roots: Vec<(u32, u64)>,
    pub(super) gone_current_records: Vec<PathBuf>,
}

/// Retire a settled gone host's launch record once its remaining cleanup is confirmed.
pub(super) async fn retire_gone_launch_records(records: impl IntoIterator<Item = PathBuf>) {
    for path in records {
        if let Err(error) = tokio::fs::remove_file(&path).await
            && error.kind() != std::io::ErrorKind::NotFound
        {
            warn!(path = %path.display(), %error, "could not remove the settled launch record");
        }
    }
}

impl Supervisor {
    /// `relaunch_into_terminal`'s work: start the new launch of session
    /// `entry` in a new terminal host seeded with the previous terminal's
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
    /// scope, the argv and which of the launch's commands it is (the resume
    /// command, or the start command when Claude saved no conversation), the
    /// verified working directory, and the tab
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
        phase: crate::agent_kind::LaunchPhase,
        launch_cwd: String,
        tabs: Vec<TabInfo>,
        seed: Option<RestartSeed>,
        setup_persistent_hooks: bool,
        extra_instructions: Option<&str>,
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
                // The phase picks a command launch's shell source to match
                // `argv`: its resume command text, or its start command text
                // for a restart that starts fresh.
                ShellSource::of(launch, phase),
                // The session's own snapshot: the kind the session was
                // created with decides the hook flags.
                snapshot,
                FarhelmArgsMode::of(launch, phase),
                &launch_cwd,
                cols,
                rows,
                scope.as_deref(),
                // A restart never prepares a checkout or types an initial prompt.
                None,
                None,
                blob,
                setup_persistent_hooks,
                extra_instructions,
            )
            .await;
        let SpawnedInHost {
            host,
            artifacts,
            hooked,
            persistent_hook_setup,
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
                        generation,
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
                // reachable. The previous terminal is
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
                    launch,
                    snapshot,
                    generation,
                    Relaunched {
                        terminal,
                        scope,
                        outcome: LastOutcome::Launching,
                        hooked,
                        persistent_hook_setup,
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
                launch,
                snapshot,
                generation,
                Relaunched {
                    terminal,
                    scope,
                    outcome,
                    hooked,
                    persistent_hook_setup,
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
                "the previous terminal had not settled when the restart read its history; the \
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
                    "the previous terminal's host is gone, so the restarted terminal starts empty");
                None
            }
            None => {
                warn!(session = %id,
                    "the previous terminal's history could not be read, so the restarted terminal \
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
    ///   left) is removed, except for the current generation's record, which
    ///   the caller retires only after confirming its process sweep;
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
        entry: &SessionEntry,
    ) -> anyhow::Result<SettledAgentLaunches> {
        let session_id = entry.info.id.as_str();
        let (held, held_generation) = match entry.terminal.as_ref() {
            Some(host) => (
                host.socket().file_name().map(ToOwned::to_owned),
                match host.tag().map(|tag| &tag.role) {
                    Some(crate::hosts::HostRole::Agent { generation }) => Some(*generation),
                    Some(crate::hosts::HostRole::Tab { .. }) | None => None,
                },
            ),
            None => (None, None),
        };
        let mut settled = SettledAgentLaunches::default();
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
                    if generation == Some(entry.generation) {
                        settled.gone_current_records.push(path);
                    } else {
                        retire_gone_launch_records([path]).await;
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
                                settled.roots.push((root.pid, root.start));
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
        Ok(settled)
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
    /// process tree reaped and its unconsumed artifacts removed
    /// (`scope` is the failed launch's own unit, which the scope wrapper
    /// may have created). A definitive failure lets the previous run's record
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
        let unresolved_scope = |error| RelaunchFailure {
            scope_cleanup_unconfirmed: scope.is_some(),
            ..RelaunchFailure::ambiguous(error)
        };
        if let HostLeft::MayRemain { socket } = left {
            let ended = match self.end_launch_host(None, &socket, id).await {
                Ok(()) => {
                    // Shutdown can confirm one process ended while a second stage
                    // that has yet to bind can still start. Fence the record before
                    // an apparently empty scope can settle this launch.
                    self.confirm_failed_restart_host_gone(artifacts).await
                }
                Err(error) => Err(anyhow::Error::msg(error)),
            };
            if let Err(why) = ended {
                return unresolved_scope(error.context(format!(
                    "a terminal host may still be running at {} ({why:#}), so the launch is kept as a \
                     launching record rather than unwound",
                    socket.display()
                )));
            }
        }
        let mut error = artifacts
            .remove_unconsumed_relaunch_fail_closed(error)
            .await;
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
            ScopeKillFailure::Refuse,
        )
        .await
        {
            return unresolved_scope(error.context(format!(
                "and the failed launch's process tree could not be swept ({sweep:#})"
            )));
        }
        // Retain the failed generation's launch record while its recorded
        // scope could still hold descendants. A later cleanup needs both.
        if !artifacts.remove_host_record_best_effort().await {
            error =
                error.context("and the failed relaunch's launch host record could not be removed");
        }
        if let Err(crash) = self.host_restart_fault(HostRestartStage::ReplacementUnwound) {
            return RelaunchFailure::ambiguous(crash);
        }
        RelaunchFailure::definitive(error)
    }

    /// Tear down a host relaunch whose session's row is gone (a delete
    /// committed while it ran): its agent's process tree, its new host, and
    /// the old host it was replacing, with their launch records once each
    /// is confirmed ended, and the launch's files. The replacement's record stays
    /// with any unresolved scope cleanup so startup can fence the recorded start.
    /// With the row gone, the error must name what cleanup could not confirm.
    async fn tear_down_relaunch_deleted_meanwhile(
        &self,
        id: &str,
        generation: i64,
        scope: Option<&str>,
        new: &HostTerminal,
        old: Option<&HostTerminal>,
        artifacts: &LaunchArtifacts,
    ) -> RelaunchFailure {
        let mut teardown = Vec::new();
        // The tree first: ending a host hangs up its child and nothing
        // beyond it.
        let swept = reap_process_tree(
            &self.seams.scopes,
            ScopeUnits::recorded(scope.map(str::to_string)),
            None,
            id,
            &SweepTarget::AgentOnly,
            ScopeKillFailure::Refuse,
        )
        .await;
        let tree_ended = swept.is_ok();
        if let Err(error) = swept {
            teardown.push(format!("the new agent's process tree ({error:#})"));
        }
        if scope.is_some()
            && !tree_ended
            && let Err(error) = self
                .store
                .record_pending_scope_cleanup(id, generation)
                .await
        {
            warn!(session = %id, generation, error = %format!("{error:#}"),
                "could not save the failed restart's scope cleanup for the next start");
            teardown.push(format!("saving the new agent's scope cleanup ({error:#})"));
        }
        match self.end_launch_host(Some(new), new.socket(), id).await {
            Ok(()) if tree_ended => {
                artifacts.remove_host_record_best_effort().await;
            }
            Ok(()) => {}
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

    /// Finish the scope cleanups that failed Restarts of deleted sessions left in
    /// `supervisor.db`, before serving and even when no session rows remain.
    ///
    /// Each obligation names one launch generation, whose launch record (when one is left)
    /// is settled first ([`Self::settle_failed_restart_host`]). Older obligations may have
    /// no record because the previous supervisor removed it after confirming the host gone.
    /// A record's starter must be fenced before an empty scope can satisfy the obligation,
    /// since a start still under way could create the scope after the sweep looked.
    ///
    /// A replacement host that still answers at the recorded socket is the other shape a
    /// failed teardown leaves (its sweep and its host's end both failed). Such a host has
    /// already started its agent, so once its tag shows it is exactly this launch's, the
    /// scope is swept and the host is shut down over the connection the settle opened.
    /// Its record and the obligation are retired only once both the sweep and the host's
    /// end are confirmed. A host with any other tag, or one that cannot be read, is left
    /// alone with the record and the obligation, as is anything the settle cannot tell.
    pub(super) async fn retry_pending_scope_cleanups(&self, deadline: tokio::time::Instant) {
        // Leave enough of the remaining startup budget for the ordinary signals and confirmation after the
        // optional Podman stop. An answering replacement host is shut down in that same remainder, after
        // the sweep; `hosts::stop::shut_down` can take around 25 seconds at worst (its `Shutdown`, the
        // end wait, then the verified signal's graces). The deadline cutting it short is safe: the record
        // and the obligation are only retired after a confirmed end, so the next start tries again.
        let now = tokio::time::Instant::now();
        let podman_deadline = now
            + deadline
                .saturating_duration_since(now)
                .saturating_sub(Duration::from_secs(10));
        let work = async {
            let pending = match self.store.pending_scope_cleanups().await {
                Ok(pending) => pending,
                Err(error) => {
                    warn!(error = %format!("{error:#}"), "could not read pending restart scope cleanups");
                    return;
                }
            };
            if pending.is_empty() {
                return;
            }
            let known = match self.store.known_session_ids().await {
                Ok(known) => known,
                Err(error) => {
                    warn!(error = %format!("{error:#}"), "could not identify sessions to protect during restart scope cleanup");
                    return;
                }
            };
            let held = self.held_terminals().await;
            for (id, generation) in pending {
                if let Some(unit) = crate::scope::unit_name(&id, generation) {
                    let restored = known.contains(&id);
                    if restored {
                        match self.store.session(&id).await {
                            Ok(Some(row)) if row.generation == generation => {
                                warn!(session = %id, generation,
                                    "the restored session still owns the pending restart scope");
                                continue;
                            }
                            Ok(_) => {}
                            Err(error) => {
                                warn!(session = %id, generation, error = %format!("{error:#}"),
                                    "could not check which generation the restored session owns");
                                continue;
                            }
                        }
                    }
                    let artifacts = LaunchArtifacts::for_launch(&self.state_dir, &id, generation);
                    let answering = match self
                        .settle_failed_restart_host(&artifacts, &held.sockets)
                        .await
                    {
                        Ok(FailedRestartHost::Gone) => None,
                        // Only a host tagged for exactly this session's launch generation
                        // is the replacement the obligation was left for. Anything else at
                        // the recorded socket says nothing about whether that replacement's
                        // scope can still fill, so the host, the record and the obligation
                        // are all left for a later start, untouched.
                        Ok(FailedRestartHost::Answered {
                            socket,
                            connection,
                            info,
                        }) => match crate::hosts::HostTag::decode(&info.tag) {
                            Ok(tag)
                                if tag.session_id == id
                                    && tag.role
                                        == (crate::hosts::HostRole::Agent { generation }) =>
                            {
                                Some((socket, connection))
                            }
                            Ok(tag) => {
                                warn!(session = %id, generation, socket = %socket.display(), ?tag,
                                    "the failed restart's recorded socket serves a host for another \
                                     terminal; leaving it and the restart scope cleanup in place");
                                continue;
                            }
                            Err(unreadable) => {
                                warn!(session = %id, generation, socket = %socket.display(),
                                    %unreadable,
                                    "the failed restart's recorded host carries a tag this build \
                                     cannot read; leaving it and the restart scope cleanup in place");
                                continue;
                            }
                        },
                        Err(error) => {
                            warn!(session = %id, generation, error = %format!("{error:#}"),
                                "restart scope cleanup is waiting for its recorded host start");
                            continue;
                        }
                    };
                    let sweep = if restored {
                        reap_recorded_scope(&self.seams.scopes, unit, &id, podman_deadline).await
                    } else {
                        reap_process_tree_with_podman_deadline(
                            &self.seams.scopes,
                            ScopeUnits::recorded(Some(unit)),
                            None,
                            &id,
                            &SweepTarget::AgentOnly,
                            ScopeKillFailure::Refuse,
                            Some(podman_deadline),
                        )
                        .await
                    };
                    let swept = match sweep {
                        Ok(_) => true,
                        Err(error) => {
                            warn!(session = %id, generation, error = %format!("{error:#}"),
                                "restart scope cleanup is still unconfirmed");
                            false
                        }
                    };
                    // The answering host is ended even when the sweep failed. No session's
                    // terminal holds it, so only this retry, or a Delete of a session that
                    // reuses the ID (which settles every launch record of that ID), would
                    // ever end it, and its hangup reaches the agent the sweep could not
                    // confirm gone. With the host gone the next
                    // start settles the record as gone and retries the sweep alone, the
                    // shape this retry already handles; the record and the obligation stay
                    // until both are confirmed.
                    if let Some((socket, connection)) = answering {
                        match crate::hosts::stop::shut_down(
                            *connection,
                            &socket,
                            crate::hosts::StopLimits::default(),
                        )
                        .await
                        {
                            crate::hosts::HostOutcome::Stopped(how) => {
                                info!(session = %id, generation, socket = %socket.display(),
                                    how = %how, "ended a failed restart's terminal host");
                                self.remove_ended_socket(&socket).await;
                            }
                            crate::hosts::HostOutcome::Unconfirmed(why) => {
                                warn!(session = %id, generation, socket = %socket.display(), %why,
                                    "a failed restart's terminal host may still run; its restart \
                                     scope cleanup stays for the next start");
                                continue;
                            }
                        }
                    }
                    if !swept {
                        continue;
                    }
                    if !artifacts.remove_host_record_best_effort().await {
                        continue;
                    }
                } else {
                    warn!(session = ?id, generation, "discarding pending restart cleanup with an invalid session id");
                }
                if let Err(error) = self
                    .store
                    .retire_pending_scope_cleanup(&id, generation)
                    .await
                {
                    warn!(session = ?id, generation, error = %format!("{error:#}"),
                        "could not retire pending restart scope cleanup");
                }
            }
        };
        if tokio::time::timeout_at(deadline, work).await.is_err() {
            warn!(
                "restart scope cleanup ran out of startup time; remaining obligations stay for the next start"
            );
        }
    }

    /// Confirm that a failed Restart's replacement left no host running, for the unwind
    /// of a replacement whose start failed ([`Self::unwind_failed_host_relaunch`]).
    ///
    /// A host that answers at the recorded socket is an error here, never something to
    /// end: an unwind that found its replacement live must keep the Restart ambiguous
    /// rather than restore the previous terminal over a running agent. No held sockets
    /// are checked: the recorded socket is the one this Restart's own failed start drew.
    async fn confirm_failed_restart_host_gone(
        &self,
        artifacts: &LaunchArtifacts,
    ) -> anyhow::Result<()> {
        match self
            .settle_failed_restart_host(artifacts, &HashSet::new())
            .await?
        {
            FailedRestartHost::Gone => Ok(()),
            // The connection drops here, which leaves the host as it was.
            FailedRestartHost::Answered { socket, .. } => {
                anyhow::bail!("the recorded host at {} is still running", socket.display())
            }
        }
    }

    /// Settle a failed Restart's exact launch record before cleanup can trust a vacant
    /// scope: its first stage fenced, then what answers at its socket
    /// ([`Self::settle_launch_record`]).
    ///
    /// [`FailedRestartHost::Gone`] when there is no record (or an empty one) or nothing
    /// serves its socket, whose leftover file is then removed. A host that answers comes
    /// back with the connection that now owns it, for the caller to judge by its tag. An
    /// error for everything that cannot be told: a record that is not a regular file or
    /// cannot be read, a first stage that may still run, something at the socket that
    /// does not answer usefully, and a socket in `held_sockets`. A live terminal holding
    /// the socket may be part of a restored session, and must stay connected and alone.
    ///
    /// This runs both for an ordinary failed Restart unwinding with its row still present
    /// and for a deleted session's obligation at startup: it can wait for a still-running
    /// first stage, end it with the verified signal and remove its socket before the
    /// caller removes the record.
    async fn settle_failed_restart_host(
        &self,
        artifacts: &LaunchArtifacts,
        held_sockets: &HashSet<OsString>,
    ) -> anyhow::Result<FailedRestartHost> {
        let path = artifacts.host();
        match tokio::fs::symlink_metadata(path).await {
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) => anyhow::bail!("the failed restart's launch record is not a regular file"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(FailedRestartHost::Gone);
            }
            Err(error) => {
                return Err(anyhow::Error::new(error)
                    .context("checking the failed restart's launch record"));
            }
        }
        let Some(record) = super::read_launch_record(path).await? else {
            return Ok(FailedRestartHost::Gone);
        };
        crate::hosts::existing_hosts_dir(&std::path::absolute(&self.state_dir)?)?;
        let socket = self.recorded_socket(&record)?;
        if socket
            .file_name()
            .is_some_and(|name| held_sockets.contains(name))
        {
            anyhow::bail!("the recorded socket is still held by a terminal");
        }
        match self.settle_launch_record(&record, &socket).await {
            Ok(AtRecordedSocket::Gone) => {
                self.remove_ended_socket(&socket).await;
                Ok(FailedRestartHost::Gone)
            }
            Ok(AtRecordedSocket::Answered { connection, info }) => {
                Ok(FailedRestartHost::Answered {
                    socket,
                    connection,
                    info,
                })
            }
            Ok(AtRecordedSocket::NoAnswer(error)) => Err(error.into()),
            Ok(AtRecordedSocket::NoInfo { error, .. }) => Err(error.into()),
            Err(error) => Err(anyhow::Error::msg(error)),
        }
    }
}

/// What a failed Restart's launch record left, once settled
/// ([`Supervisor::settle_failed_restart_host`]).
enum FailedRestartHost {
    /// No host runs at the recorded socket and none can start there: the record is
    /// missing or empty, or its first stage is fenced and nothing serves the socket.
    Gone,
    /// A host answered at the recorded socket, which no terminal on the map holds. Its
    /// first stage is fenced, and a host that answers has already started its one child,
    /// so it can start nothing more. Whose host it is, `info`'s tag says; `connection`
    /// owns the host now (boxed, as in [`AtRecordedSocket::Answered`]), and dropping it
    /// leaves the host as it was.
    Answered {
        socket: PathBuf,
        connection: Box<crate::hosts::HostConnection>,
        info: terminal_host::protocol::Info,
    },
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::super::tests::{StateDir, dummy_exe};
    use super::super::{RelaunchDisposition, SupervisorSeams, SupervisorTimeouts};
    use super::*;

    /// The portable sweep cannot confirm a scrubbed process in a recorded scope.
    /// Restart keeps the failed generation when the scope manager is unavailable.
    #[farhelm_testtrace::test]
    async fn failed_relaunch_keeps_its_record_when_its_scope_cannot_be_confirmed() {
        let state = StateDir::new();
        let scopes = crate::scope::ScopeManager::fake_reprobing(false, false, Arc::new(|_| {}));
        let seams = SupervisorSeams {
            scopes: Arc::new(scopes),
            ..SupervisorSeams::default()
        };
        let sup = Supervisor::new_with_seams(
            state.path(),
            dummy_exe(),
            SupervisorTimeouts::default(),
            seams,
        )
        .await
        .unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let scope = crate::scope::unit_name(&id, 1).unwrap();
        let artifacts = LaunchArtifacts::for_launch(state.path(), &id, 1);
        let record = super::super::host_launch::LaunchHostRecord {
            socket: crate::hosts::free_host_name(state.path()).unwrap(),
            starter: crate::hosts::HostProcess {
                pid: std::process::id(),
                start: 0,
            },
        };
        std::fs::write(artifacts.host(), record.encode())
            .expect("failed generation's record exists");
        let failure = sup
            .unwind_failed_host_relaunch(
                &id,
                Some(&scope),
                HostLeft::Nothing,
                &artifacts,
                anyhow::anyhow!("start failed"),
            )
            .await;
        assert_eq!(failure.disposition, RelaunchDisposition::Ambiguous);
        assert!(failure.scope_cleanup_unconfirmed);
        assert!(format!("{:#}", failure.error).contains(&scope));
        assert!(artifacts.host().exists());
    }

    /// A replacement whose row disappeared has no stored scope flag. The launch record only names its host,
    /// so the retry must come from the separate obligation even when there are no sessions to rediscover.
    #[cfg(target_os = "linux")]
    #[farhelm_testtrace::test]
    async fn a_deleted_restarts_scope_is_retried_at_startup_after_its_host_is_gone() {
        use super::super::host_launch::LaunchHostRecord;
        use super::super::tests::serve_until_answering;
        use crate::scope::{ScopeManager, ScopeOp};

        for unrelated_session in [false, true] {
            let state = StateDir::new();
            let id = uuid::Uuid::new_v4().to_string();
            let generation = 3;
            let unit = crate::scope::unit_name(&id, generation).unwrap();
            let name =
                crate::hosts::free_host_name(state.path()).expect("allocate host socket name");
            let socket = name.socket_in(&crate::hosts::hosts_dir(state.path()));
            let artifacts = LaunchArtifacts::for_launch(state.path(), &id, generation);
            let sup = supervisor_with_scopes(
                &state,
                ScopeManager::fake_reprobing(false, false, Arc::new(|_| {})),
            )
            .await;
            store_launching_row(&sup, &id, generation, true, None).await;
            let row = sup
                .store
                .session(&id)
                .await
                .unwrap()
                .expect("replacement launch row");
            assert!(row.launch_scoped);
            assert_eq!(row.generation, generation);
            let other_id = uuid::Uuid::new_v4().to_string();
            if unrelated_session {
                store_launching_row(&sup, &other_id, 0, true, None).await;
            }
            sup.store
                .delete_session(&id, None)
                .await
                .expect("delete during restart");
            assert!(sup.store.session(&id).await.unwrap().is_none());
            let record = LaunchHostRecord {
                socket: name,
                starter: crate::hosts::HostProcess {
                    pid: std::process::id(),
                    start: 0,
                },
            };
            std::fs::write(artifacts.host(), record.encode())
                .expect("replacement's launch record exists");
            assert!(!socket.exists(), "the replacement host is already gone");

            let failure = sup
                .tear_down_relaunch_deleted_meanwhile(
                    &id,
                    generation,
                    Some(&unit),
                    &HostTerminal::gone(&socket, None),
                    None,
                    &artifacts,
                )
                .await;
            assert!(format!("{:#}", failure.error).contains(&unit));
            assert!(
                artifacts.host().exists(),
                "the host record is still needed while the scope is unresolved"
            );
            let obligation = [(id.clone(), generation)];
            assert_eq!(
                sup.store.pending_scope_cleanups().await.unwrap(),
                obligation
            );
            drop(sup);

            let sup = supervisor_with_scopes(
                &state,
                ScopeManager::fake_reprobing(false, false, Arc::new(|_| {})),
            )
            .await;
            let serving = serve_until_answering(&sup).await;
            assert_eq!(
                sup.store.pending_scope_cleanups().await.unwrap(),
                obligation
            );
            assert!(
                artifacts.host().exists(),
                "another failed sweep keeps the record"
            );
            serving.abort();
            assert!(serving.await.unwrap_err().is_cancelled());
            drop(sup);

            let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
            let sink = Arc::clone(&observed);
            let sup = supervisor_with_scopes(
                &state,
                ScopeManager::fake_vanishing_after_signal(
                    "SIGTERM",
                    Arc::new(move |op| {
                        sink.lock().unwrap().push(op.clone());
                    }),
                ),
            )
            .await;
            // Constructing a supervisor alone does not claim the right to serve or consume this obligation.
            assert_eq!(
                sup.store.pending_scope_cleanups().await.unwrap(),
                obligation
            );
            let serving = serve_until_answering(&sup).await;
            assert!(sup.store.pending_scope_cleanups().await.unwrap().is_empty());
            assert!(
                !artifacts.host().exists(),
                "successful cleanup retires the record"
            );
            {
                let operations = observed.lock().unwrap();
                let signal = ScopeOp::Kill {
                    unit: unit.clone(),
                    signal: "SIGTERM".into(),
                };
                let at = operations
                    .iter()
                    .position(|op| op == &signal)
                    .expect("startup signals the recorded generation");
                assert!(operations[..at].contains(&ScopeOp::Exists(unit.clone())));
                assert!(operations[at + 1..].contains(&ScopeOp::Exists(unit.clone())));
                assert!(
                    operations.iter().all(|op| match op {
                        ScopeOp::Kill {
                            unit: signalled, ..
                        } => signalled == &unit,
                        _ => true,
                    }),
                    "startup leaves unrelated scopes alone: {operations:?}"
                );
            }
            assert!(sup.store.session(&id).await.unwrap().is_none());
            assert_eq!(
                sup.store.session(&other_id).await.unwrap().is_some(),
                unrelated_session
            );
            serving.abort();
            assert!(serving.await.unwrap_err().is_cancelled());
        }
    }

    /// A failed first stage can leave a scope even after the session's row is gone.
    /// Ordinary Delete waits for the lifecycle claim, so remove the row at the start seam.
    #[cfg(target_os = "linux")]
    #[farhelm_testtrace::test(flavor = "multi_thread")]
    async fn a_failed_replacement_keeps_scope_cleanup_when_its_row_is_deleted() {
        use super::super::{FaultHooks, RelaunchPlan};
        use crate::scope::ScopeManager;
        use std::os::unix::fs::PermissionsExt;

        let state = StateDir::new();
        let id = uuid::Uuid::new_v4().to_string();
        let sup = supervisor_with_scopes(&state, ScopeManager::disabled()).await;
        store_launching_row(&sup, &id, 0, false, None).await;
        drop(sup);

        let program = state.path().join("refusing-host");
        std::fs::write(&program, "#!/bin/sh\ncat >/dev/null\nexit 1\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        let db_path = state.path().join("supervisor.db");
        let delete_id = id.clone();
        let on_start = move || {
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    let store = crate::store::SessionStore::open(&db_path, false).await?;
                    let row = store.session(&delete_id).await?.expect("replacement row");
                    assert_eq!(row.generation, 1);
                    assert!(row.launch_scoped);
                    store.delete_session(&delete_id, None).await?;
                    Ok(())
                })
            })
        };
        let sup = Supervisor::new_with_seams(
            state.path(),
            dummy_exe(),
            SupervisorTimeouts::default(),
            SupervisorSeams {
                host_program: Some(program),
                scopes: Arc::new(ScopeManager::fake_failing_kills(Arc::new(|_| {}))),
                faults: FaultHooks {
                    host_start_interrupt: Some(Arc::new(on_start)),
                    ..FaultHooks::default()
                },
                ..SupervisorSeams::default()
            },
        )
        .await
        .unwrap();
        let entry = sup.sessions.lock().await[&id].clone();
        let snapshot = sup.session_snapshot(&id).await.unwrap().unwrap();
        let _lifecycle = sup.lifecycle_locks.claim(&id).await;
        let error = sup
            .relaunch(
                &entry,
                &snapshot,
                RelaunchPlan {
                    extra_instructions: None,
                    setup_persistent_hooks: false,
                    clear_capture: false,
                    argv: vec!["sh".into(), "-c".into(), "exit 0".into()],
                    phase: crate::agent_kind::LaunchPhase::Resume,
                    launch_cwd: state.path().to_string_lossy().into_owned(),
                    restart_with: None,
                    seed: None,
                },
            )
            .await
            .expect_err("the replacement failed before confirmation");
        assert!(
            format!("{error:#}").contains("process tree could not be swept"),
            "{error:#}"
        );
        assert!(sup.store.session(&id).await.unwrap().is_none());
        assert_eq!(
            sup.store.pending_scope_cleanups().await.unwrap(),
            [(id.clone(), 1)]
        );
        assert!(
            LaunchArtifacts::for_launch(state.path(), &id, 1)
                .host()
                .is_file()
        );
    }

    /// A host starter can create a scope after an early check found it absent. The startup
    /// deadline preserves both pieces of evidence until a later attempt can fence that starter.
    #[cfg(target_os = "linux")]
    #[farhelm_testtrace::test]
    async fn pending_restart_cleanup_checks_its_starter_before_its_scope() {
        use super::super::{FaultHooks, host_launch::LaunchHostRecord};
        use crate::scope::{ScopeManager, ScopeOp};
        use std::sync::atomic::{AtomicBool, Ordering};
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

        let state = StateDir::new();
        let id = uuid::Uuid::new_v4().to_string();
        let name = crate::hosts::free_host_name(state.path()).unwrap();
        let socket = name.socket_in(&crate::hosts::hosts_dir(state.path()));
        let mut child = tokio::process::Command::new("/usr/bin/python3")
            .args([
                "-u",
                "-c",
                "import os, socket, sys\nprint('ready')\nassert input() == 'start'\ns = socket.socket(socket.AF_UNIX)\ns.bind(sys.argv[1])\ns.listen()\nprint('bound')\nassert input() == 'stop'\ns.close()\nos.unlink(sys.argv[1])",
            ])
            .arg(&socket)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut gate = child.stdin.take().unwrap();
        let mut progress = BufReader::new(child.stdout.take().unwrap()).lines();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), progress.next_line())
                .await
                .unwrap()
                .unwrap()
                .as_deref(),
            Some("ready"),
        );
        let starter = crate::hosts::HostProcess::current(child.id().unwrap()).unwrap();
        let artifacts = LaunchArtifacts::for_launch(state.path(), &id, 1);
        let unit = crate::scope::unit_name(&id, 1).unwrap();
        let scope_ops = Arc::new(std::sync::Mutex::new(Vec::new()));
        let track_scope_ops: crate::scope::ScopeOpSink = Arc::new({
            let scope_ops = Arc::clone(&scope_ops);
            move |op| scope_ops.lock().unwrap().push(op.clone())
        });
        let absent_scope = Arc::new(ScopeManager::fake_vanishing(
            0,
            Arc::clone(&track_scope_ops),
        ));
        let waited_for_starter = Arc::new(AtomicBool::new(false));
        let sup = Supervisor::new_with_seams(
            state.path(),
            dummy_exe(),
            SupervisorTimeouts::default(),
            SupervisorSeams {
                scopes: Arc::clone(&absent_scope),
                faults: FaultHooks {
                    launch_starter_waiting: Some(Arc::new({
                        let waited_for_starter = Arc::clone(&waited_for_starter);
                        move || waited_for_starter.store(true, Ordering::Relaxed)
                    })),
                    ..FaultHooks::default()
                },
                ..SupervisorSeams::default()
            },
        )
        .await
        .unwrap();
        std::fs::write(
            artifacts.host(),
            LaunchHostRecord {
                socket: name,
                starter,
            }
            .encode(),
        )
        .unwrap();
        sup.store
            .record_pending_scope_cleanup(&id, 1)
            .await
            .unwrap();
        assert!(starter.is_running().unwrap());
        assert!(!socket.exists());
        assert!(absent_scope.available().await);
        assert!(!absent_scope.exists(&unit).await.unwrap());
        scope_ops.lock().unwrap().clear();

        sup.retry_pending_scope_cleanups(tokio::time::Instant::now() + Duration::from_secs(2))
            .await;
        assert!(
            waited_for_starter.load(Ordering::Relaxed),
            "the retry reached the starter fence"
        );
        assert!(starter.is_running().unwrap());
        assert!(
            !scope_ops
                .lock()
                .unwrap()
                .iter()
                .any(|op| matches!(op, ScopeOp::Exists(_)))
        );
        assert!(artifacts.host().is_file());
        assert_eq!(
            sup.store.pending_scope_cleanups().await.unwrap(),
            [(id.clone(), 1)]
        );

        gate.write_all(b"start\n").await.unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), progress.next_line())
                .await
                .unwrap()
                .unwrap()
                .as_deref(),
            Some("bound"),
        );
        assert!(
            socket.exists(),
            "the held starter could still bring up a host"
        );
        // An orderly host end removes its socket. The scope can still outlive
        // the host, so the next startup sees a live scope at the same identity.
        gate.write_all(b"stop\n").await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_secs(5), child.wait())
                .await
                .unwrap()
                .unwrap()
                .success()
        );
        assert!(!starter.is_running().unwrap());
        assert!(!socket.exists());
        drop(sup);
        let scopes = ScopeManager::fake_vanishing_after_signal("SIGTERM", track_scope_ops);
        let sup = supervisor_with_scopes(&state, scopes).await;
        sup.retry_pending_scope_cleanups(tokio::time::Instant::now() + Duration::from_secs(5))
            .await;
        {
            let operations = scope_ops.lock().unwrap();
            let signal = ScopeOp::Kill {
                unit: unit.clone(),
                signal: "SIGTERM".into(),
            };
            let sent = operations
                .iter()
                .position(|op| op == &signal)
                .expect("retry signalled the replacement's scope");
            assert!(operations[sent + 1..].contains(&ScopeOp::Exists(unit)));
        }
        assert!(!artifacts.host().exists());
        assert!(sup.store.pending_scope_cleanups().await.unwrap().is_empty());
    }

    /// Restart cleanup must spare the process of a restored session even when only its
    /// create reservation remains. The old scope has disappeared, so cleanup would
    /// otherwise fall back to the session marker shared by both launches.
    #[cfg(target_os = "linux")]
    #[farhelm_testtrace::test]
    async fn pending_restart_cleanup_leaves_the_restored_sessions_process_running() {
        use super::super::tests::serve_until_answering;
        use crate::store::{DedupScope, IntentClaim};
        use std::process::Stdio;
        use tokio::io::{AsyncBufReadExt, BufReader};

        for reservation_only in [false, true] {
            let state = StateDir::new();
            let id = uuid::Uuid::new_v4().to_string();
            let scopes = crate::scope::ScopeManager::fake_vanishing(0, Arc::new(|_| {}));
            let sup = supervisor_with_scopes(&state, scopes).await;
            let claim = reservation_only.then(|| IntentClaim {
                intent_key: "restored-start".into(),
                fingerprint: "restored-start".into(),
                dedup_scope: DedupScope::Permanent,
            });
            store_launching_row(&sup, &id, 2, false, claim).await;
            if reservation_only {
                sup.store.delete_session(&id, None).await.unwrap();
                assert!(sup.store.session(&id).await.unwrap().is_none());
                assert_eq!(
                    sup.store.known_session_ids().await.unwrap(),
                    HashSet::from([id.clone()]),
                    "the reservation alone owns the ID",
                );
            }
            sup.store
                .record_pending_scope_cleanup(&id, 1)
                .await
                .unwrap();
            // Wait until the child is executing before reading its marker from /proc.
            // The open input pipe keeps that same process around for the cleanup check.
            let mut new_agent = tokio::process::Command::new("/bin/sh")
                .args(["-c", "printf 'ready\\n'; read -r unused"])
                .env(crate::launch::SESSION_ID_ENV_VAR, &id)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .unwrap();
            let mut progress = BufReader::new(new_agent.stdout.take().unwrap()).lines();
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(5), progress.next_line())
                    .await
                    .expect("the marked child did not report readiness")
                    .unwrap()
                    .as_deref(),
                Some("ready"),
            );
            let pid = new_agent.id().unwrap();
            let environs = std::fs::read(format!("/proc/{pid}/environ")).unwrap();
            let marker = format!("{}={id}", crate::launch::SESSION_ID_ENV_VAR);
            assert!(
                environs
                    .split(|&byte| byte == 0)
                    .any(|item| item == marker.as_bytes())
            );
            assert!(new_agent.try_wait().unwrap().is_none());

            let serving = serve_until_answering(&sup).await;
            assert!(sup.store.pending_scope_cleanups().await.unwrap().is_empty());
            assert_eq!(
                sup.store
                    .session(&id)
                    .await
                    .unwrap()
                    .map(|row| row.generation),
                if reservation_only { None } else { Some(2) },
            );
            assert!(sup.store.known_session_ids().await.unwrap().contains(&id));
            assert!(
                new_agent.try_wait().unwrap().is_none(),
                "the restored session stays alive"
            );
            new_agent.kill().await.unwrap();
            serving.abort();
            assert!(serving.await.unwrap_err().is_cancelled());
        }
    }

    // ---- A failed restart's replacement host that still answers ------------------------

    /// A deleted restart's replacement host that still answers is cleaned up at the next
    /// start: its scope swept, the host shut down, and its record and obligation retired.
    ///
    /// Why: when a Restart loses its row mid-relaunch and neither its new agent's scope
    /// sweep nor the end of its new host succeeds, the host keeps answering at its
    /// recorded socket. Startup host reclaim leaves that record and socket to the pending
    /// cleanup, so if the cleanup treated an answering host as a reason to wait, nothing
    /// would ever end the host, sweep the scope or retire the obligation (the bug this
    /// pins). Spec: with a real host tagged for the session's pending generation answering
    /// at the recorded socket, the scope present, and the record and obligation in place
    /// (premises checked after the supervisor restarts), serving sends the scope SIGTERM
    /// and checks it again, ends the host's process, removes its socket, and leaves no
    /// record and no obligation.
    #[cfg(target_os = "linux")]
    #[farhelm_testtrace::test]
    async fn a_deleted_restarts_answering_replacement_is_ended_and_its_cleanup_retired_at_startup()
    {
        use super::super::tests::serve_until_answering;
        use crate::scope::{ScopeManager, ScopeOp};

        let state = StateDir::new();
        let id = uuid::Uuid::new_v4().to_string();
        let generation = 2;
        let unit = crate::scope::unit_name(&id, generation).unwrap();
        let artifacts = LaunchArtifacts::for_launch(state.path(), &id, generation);
        let sup = supervisor_with_scopes(
            &state,
            ScopeManager::fake_reprobing(false, false, Arc::new(|_| {})),
        )
        .await;
        let (socket, host) = plant_answering_replacement(&sup, &state, &id, generation).await;
        drop(sup);

        let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = Arc::clone(&observed);
        let sup = supervisor_with_scopes(
            &state,
            ScopeManager::fake_vanishing_after_signal(
                "SIGTERM",
                Arc::new(move |op| sink.lock().unwrap().push(op.clone())),
            ),
        )
        .await;
        // The restart and its reload left everything the cleanup needs in place.
        assert_replacement_answers(&socket, &id, generation).await;
        assert!(host.is_running().unwrap());
        // The fake answers existence only once its availability probe has run.
        assert!(sup.seams.scopes.available().await);
        assert!(sup.seams.scopes.exists(&unit).await.unwrap());
        assert!(artifacts.host().is_file());
        assert_eq!(
            sup.store.pending_scope_cleanups().await.unwrap(),
            [(id.clone(), generation)]
        );
        observed.lock().unwrap().clear();

        let serving = serve_until_answering(&sup).await;
        {
            let operations = observed.lock().unwrap();
            let signal = ScopeOp::Kill {
                unit: unit.clone(),
                signal: "SIGTERM".into(),
            };
            let sent = operations
                .iter()
                .position(|op| op == &signal)
                .expect("startup signalled the replacement's scope");
            assert!(operations[sent + 1..].contains(&ScopeOp::Exists(unit.clone())));
        }
        assert!(
            !host.is_running().unwrap(),
            "the answering replacement host was ended"
        );
        assert!(!socket.exists(), "the ended host's socket was removed");
        assert!(!artifacts.host().exists(), "the launch record was retired");
        assert!(sup.store.pending_scope_cleanups().await.unwrap().is_empty());
        serving.abort();
        assert!(serving.await.unwrap_err().is_cancelled());
    }

    /// An answering replacement is ended even when its scope sweep fails, and its record
    /// and obligation stay until a later start confirms the sweep.
    ///
    /// Why: no session's terminal holds the host, so only this retry (or the Delete of a
    /// session that reuses the ID) would end it; ending it moves the leftover to the shape
    /// the gone-host retry already handles. Retiring
    /// the record or the obligation on the host's end alone would drop the only evidence
    /// that the scope may still hold the agent. Spec: with the answering host planted and
    /// the scope manager unavailable, the retry ends the host's process and removes its
    /// socket, and keeps the record and the obligation; a later retry with a scope that
    /// retires on SIGTERM signals it and retires both.
    #[cfg(target_os = "linux")]
    #[farhelm_testtrace::test]
    async fn an_answering_replacement_is_ended_even_when_its_scope_sweep_fails() {
        use crate::scope::{ScopeManager, ScopeOp};

        let state = StateDir::new();
        let id = uuid::Uuid::new_v4().to_string();
        let generation = 1;
        let unit = crate::scope::unit_name(&id, generation).unwrap();
        let artifacts = LaunchArtifacts::for_launch(state.path(), &id, generation);
        let sup = supervisor_with_scopes(
            &state,
            ScopeManager::fake_reprobing(false, false, Arc::new(|_| {})),
        )
        .await;
        let (socket, host) = plant_answering_replacement(&sup, &state, &id, generation).await;
        assert!(!sup.seams.scopes.available().await);

        sup.retry_pending_scope_cleanups(tokio::time::Instant::now() + Duration::from_secs(30))
            .await;
        assert!(!host.is_running().unwrap(), "the host was ended anyway");
        assert!(!socket.exists());
        assert!(
            artifacts.host().is_file(),
            "an unconfirmed sweep keeps the record"
        );
        assert_eq!(
            sup.store.pending_scope_cleanups().await.unwrap(),
            [(id.clone(), generation)]
        );
        drop(sup);

        let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = Arc::clone(&observed);
        let sup = supervisor_with_scopes(
            &state,
            ScopeManager::fake_vanishing_after_signal(
                "SIGTERM",
                Arc::new(move |op| sink.lock().unwrap().push(op.clone())),
            ),
        )
        .await;
        sup.retry_pending_scope_cleanups(tokio::time::Instant::now() + Duration::from_secs(30))
            .await;
        assert!(observed.lock().unwrap().contains(&ScopeOp::Kill {
            unit,
            signal: "SIGTERM".into(),
        }));
        assert!(!artifacts.host().exists());
        assert!(sup.store.pending_scope_cleanups().await.unwrap().is_empty());
    }

    /// A host at the recorded socket whose tag cannot be read, names another launch
    /// generation of the session, or names another session, is left alone with the record
    /// and the obligation.
    ///
    /// Why: only a host tagged for exactly the pending launch is the replacement the
    /// obligation was left for. Anything else says nothing about whether that launch's
    /// scope can still fill, and ending a host on the strength of its socket name alone
    /// could end someone else's terminal. Each half of the tag match (session and
    /// generation) has a case of its own, so dropping either fails this test. Spec: for a
    /// stand-in host at the recorded socket carrying unreadable tag bytes, one tagged for
    /// the same session's next generation, and one tagged for another session at the
    /// same generation (premises: each tag decodes as the case says, and the retry's
    /// settle connects to the host), the retry sends it no `Shutdown`, touches no scope,
    /// and keeps the record and the obligation.
    #[cfg(target_os = "linux")]
    #[farhelm_testtrace::test]
    async fn pending_restart_cleanup_leaves_a_host_tagged_for_another_launch_alone() {
        use super::super::host_launch::LaunchHostRecord;
        use crate::hosts::terminal::test_host::{TestHostSpec, start_test_host};
        use crate::scope::{ScopeManager, ScopeOp};

        /// The three ways a host at the recorded socket can fail to be this launch's.
        #[derive(Debug, Clone, Copy)]
        enum Mismatch {
            Unreadable,
            OtherGeneration,
            OtherSession,
        }

        let generation = 3;
        for mismatch in [
            Mismatch::Unreadable,
            Mismatch::OtherGeneration,
            Mismatch::OtherSession,
        ] {
            let state = StateDir::new();
            let id = uuid::Uuid::new_v4().to_string();
            let tagged = |session_id: String, generation: i64| {
                crate::hosts::HostTag {
                    session_id,
                    created_ns: 1,
                    role: crate::hosts::HostRole::Agent { generation },
                }
                .encode()
                .unwrap()
            };
            let tag = match mismatch {
                Mismatch::Unreadable => b"not a host tag".to_vec(),
                Mismatch::OtherGeneration => tagged(id.clone(), generation + 1),
                Mismatch::OtherSession => tagged(uuid::Uuid::new_v4().to_string(), generation),
            };
            match (mismatch, crate::hosts::HostTag::decode(&tag)) {
                (Mismatch::Unreadable, Err(_)) => {}
                (Mismatch::OtherGeneration, Ok(decoded)) => {
                    assert_eq!(decoded.session_id, id);
                    assert_ne!(decoded.role, crate::hosts::HostRole::Agent { generation });
                }
                (Mismatch::OtherSession, Ok(decoded)) => {
                    assert_ne!(decoded.session_id, id);
                    assert_eq!(decoded.role, crate::hosts::HostRole::Agent { generation });
                }
                (mismatch, decoded) => panic!("fixture: {mismatch:?} decoded as {decoded:?}"),
            }
            let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
            let sink = Arc::clone(&observed);
            let sup = supervisor_with_scopes(
                &state,
                ScopeManager::fake_vanishing_after_signal(
                    "SIGTERM",
                    Arc::new(move |op| sink.lock().unwrap().push(op.clone())),
                ),
            )
            .await;
            let name = crate::hosts::free_host_name(state.path()).unwrap();
            let stand_in = start_test_host(TestHostSpec {
                socket: Some(name.socket_in(&crate::hosts::hosts_dir(state.path()))),
                tag,
                ..TestHostSpec::default()
            });
            let artifacts = LaunchArtifacts::for_launch(state.path(), &id, generation);
            std::fs::write(
                artifacts.host(),
                LaunchHostRecord {
                    socket: name,
                    starter: ended_starter(),
                }
                .encode(),
            )
            .unwrap();
            sup.store
                .record_pending_scope_cleanup(&id, generation)
                .await
                .unwrap();
            assert_eq!(stand_in.connections(), 0);

            sup.retry_pending_scope_cleanups(tokio::time::Instant::now() + Duration::from_secs(30))
                .await;
            assert!(
                stand_in.connections() >= 1,
                "the retry reached the host at the recorded socket"
            );
            assert!(!stand_in.shutdown_seen(), "the host was not shut down");
            {
                // An availability probe is not a sweep; a check or a signal would be.
                let operations = observed.lock().unwrap();
                assert!(
                    operations.iter().all(|op| matches!(op, ScopeOp::Probe)),
                    "no scope was looked at or signalled: {operations:?}"
                );
            }
            assert!(artifacts.host().is_file());
            assert_eq!(
                sup.store.pending_scope_cleanups().await.unwrap(),
                [(id.clone(), generation)]
            );
            stand_in.close().await;
        }
    }

    /// A restored session's live terminal at the recorded socket is never shut down, even
    /// when its tag names exactly the pending launch.
    ///
    /// Why: the tag check cannot tell this case apart, so the held-socket guard is the
    /// only thing protecting it. A session that reuses the deleted ID can launch at the
    /// pending generation G (overwriting that generation's launch record with its own
    /// host's), then have a Restart to G+1 fail definitively: its row is then at G+1,
    /// past the skip for a row that owns the pending generation, while its terminal is
    /// still the generation-G host tagged exactly for the pending launch. Spec: with a
    /// row at G+1, a terminal on the map that is a real host tagged for the session's
    /// agent at G, and G's launch record naming that host's socket (premises checked),
    /// the retry leaves the host's process running at its socket, touches no scope, and
    /// keeps the record and the obligation.
    ///
    /// A real host rather than a stand-in, because a stand-in serves one client at a time:
    /// the retry's own connection would time out behind the runtime's, and the test would
    /// pass with the guard removed (checked by removing it).
    #[cfg(target_os = "linux")]
    #[farhelm_testtrace::test]
    async fn pending_restart_cleanup_leaves_a_restored_sessions_held_terminal_alone() {
        use super::super::tests::entry_with;
        use crate::scope::{ScopeManager, ScopeOp};

        let state = StateDir::new();
        let id = uuid::Uuid::new_v4().to_string();
        let generation = 2;
        let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = Arc::clone(&observed);
        let sup = supervisor_with_scopes(
            &state,
            ScopeManager::fake_vanishing_after_signal(
                "SIGTERM",
                Arc::new(move |op| sink.lock().unwrap().push(op.clone())),
            ),
        )
        .await;
        let (socket, host) = plant_answering_replacement(&sup, &state, &id, generation).await;
        store_launching_row(&sup, &id, generation + 1, true, None).await;
        let terminal = HostTerminal::adopt(
            &socket,
            crate::hosts::Adoption::Fresh,
            crate::hosts::HostTerminalOptions::default(),
        )
        .await
        .expect("adopt the restored session's terminal");
        let mut entry = entry_with(Some(terminal.clone()), LastOutcome::Running);
        entry.info.id = id.clone();
        sup.sessions
            .lock()
            .await
            .insert(id.clone(), Arc::new(entry));
        let artifacts = LaunchArtifacts::for_launch(state.path(), &id, generation);
        // The restored row is past the pending generation, so only the guard can spare it.
        assert_eq!(
            sup.store
                .session(&id)
                .await
                .unwrap()
                .map(|row| row.generation),
            Some(generation + 1)
        );
        assert!(
            sup.held_terminals()
                .await
                .sockets
                .contains(socket.file_name().unwrap()),
            "the restored session's terminal holds the recorded socket"
        );
        assert!(host.is_running().unwrap());
        assert!(sup.store.known_session_ids().await.unwrap().contains(&id));

        sup.retry_pending_scope_cleanups(tokio::time::Instant::now() + Duration::from_secs(30))
            .await;
        assert!(host.is_running().unwrap(), "the held host was not ended");
        assert!(socket.exists(), "the held host still serves its socket");
        {
            let operations = observed.lock().unwrap();
            assert!(
                operations.iter().all(|op| matches!(op, ScopeOp::Probe)),
                "no scope was looked at or signalled: {operations:?}"
            );
        }
        assert!(artifacts.host().is_file());
        assert_eq!(
            sup.store.pending_scope_cleanups().await.unwrap(),
            [(id.clone(), generation)]
        );
        // The state directory's teardown ends the host itself.
        sup.sessions.lock().await.remove(&id);
        terminal.abort();
    }

    /// Start a real terminal host for `id`'s launch `generation` from this test binary,
    /// and record it as that launch's host with the generation's scope cleanup pending:
    /// what a Restart leaves when its row was deleted mid-relaunch and both the sweep of
    /// its new agent's scope and the end of its new host failed. The host's child is a
    /// `sleep` that the host's end hangs up. Returns the host's socket and its process,
    /// with the premise that the host answers with that tag checked. The held-terminal
    /// test reuses it for a restored session's own host at that generation, which leaves
    /// the same record and obligation behind.
    #[cfg(target_os = "linux")]
    async fn plant_answering_replacement(
        sup: &Supervisor,
        state: &StateDir,
        id: &str,
        generation: i64,
    ) -> (PathBuf, crate::hosts::HostProcess) {
        use super::super::host_launch::LaunchHostRecord;

        let child = crate::hosts::ChildSpec {
            argv: vec!["/bin/sh".into(), "-c".into(), "exec sleep 600".into()],
            cwd: state.path().to_path_buf(),
            env: vec![("PATH".into(), "/usr/bin:/bin".into())],
            size: terminal_host::protocol::Size { cols: 80, rows: 24 },
            journal_cap: crate::hosts::DEFAULT_JOURNAL_CAP,
            initial_snapshot: None,
        };
        let tag = crate::hosts::HostTag {
            session_id: id.to_string(),
            created_ns: crate::hosts::tag::creation_stamp(),
            role: crate::hosts::HostRole::Agent { generation },
        };
        let spawned = crate::hosts::spawn_host(
            &crate::hosts::test_program::unit_test_host_program(),
            state.path(),
            child,
            &tag,
        )
        .await
        .expect("start the replacement's terminal host");
        std::fs::write(
            LaunchArtifacts::for_launch(state.path(), id, generation).host(),
            LaunchHostRecord {
                socket: spawned.name,
                starter: ended_starter(),
            }
            .encode(),
        )
        .expect("write the replacement's launch record");
        sup.store
            .record_pending_scope_cleanup(id, generation)
            .await
            .expect("record the replacement's scope cleanup");
        let host = assert_replacement_answers(&spawned.socket, id, generation).await;
        assert!(host.is_running().unwrap());
        (spawned.socket, host)
    }

    /// Assert that a host answers at `socket` with a tag naming `id`'s agent of launch
    /// `generation`, and return its process. The connection ends here, which leaves the
    /// host as it was.
    #[cfg(target_os = "linux")]
    async fn assert_replacement_answers(
        socket: &Path,
        id: &str,
        generation: i64,
    ) -> crate::hosts::HostProcess {
        let mut connection = crate::hosts::HostConnection::connect(socket)
            .await
            .expect("the replacement host answers");
        let info = connection
            .info()
            .await
            .expect("the replacement reports itself");
        let tag = crate::hosts::HostTag::decode(&info.tag).expect("a readable tag");
        assert_eq!(tag.session_id, id);
        assert_eq!(tag.role, crate::hosts::HostRole::Agent { generation });
        connection.peer().expect("the host's process is known")
    }

    /// A first stage that can fork nothing more: this process's pid with a start time it
    /// never had, which the starter fence reads as a process that has ended.
    #[cfg(target_os = "linux")]
    fn ended_starter() -> crate::hosts::HostProcess {
        crate::hosts::HostProcess {
            pid: std::process::id(),
            start: 0,
        }
    }

    /// Give each start a new manager observation against the same isolated database.
    #[cfg(target_os = "linux")]
    async fn supervisor_with_scopes(
        state: &StateDir,
        scopes: crate::scope::ScopeManager,
    ) -> Arc<Supervisor> {
        Supervisor::new_with_seams(
            state.path(),
            dummy_exe(),
            SupervisorTimeouts::default(),
            SupervisorSeams {
                scopes: Arc::new(scopes),
                ..SupervisorSeams::default()
            },
        )
        .await
        .expect("open supervisor")
    }

    /// Plant an unconfirmed host row. A claim lets the fixture leave the ID known after removing the row.
    #[cfg(target_os = "linux")]
    async fn store_launching_row(
        sup: &Supervisor,
        id: &str,
        generation: i64,
        scoped: bool,
        claim: Option<crate::store::IntentClaim>,
    ) {
        sup.store
            .insert_session(
                crate::store::StoredSession {
                    id: id.to_string(),
                    parent: None,
                    title: id.to_string(),
                    created_at: 1,
                    last_activity_at: 1,
                    last_work_started_at: 0,
                    creation_seq: 0,
                    cwd: "/tmp".to_string(),
                    canonical_cwd: None,
                    launch: SessionLaunch::Legacy {
                        invocation: "agent".to_string(),
                        agent_kind: farhelm_proto::AgentKind::Generic,
                        resume_template: None,
                    },
                    terminal_address: String::new(),
                    terminal_kind: crate::store::TerminalKind::Host,
                    outcome: LastOutcome::Launching,
                    captured_conversation: None,
                    generation,
                    launch_scoped: scoped,
                    launch_boot_ended: false,
                    launch_hooked: false,
                    handoff_marker_expected: false,
                    omp_launch_program: None,
                    omp_reporter_asset: None,
                    capture_ownership_version: 0,
                    conversation_source: None,
                    persistent_hook_setup: Default::default(),
                    setup_persistent_hooks_on_create: false,
                },
                claim,
            )
            .await
            .expect("insert launching row");
    }
}
