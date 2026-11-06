//! Starting a create's agent in a terminal host, behind the switch
//! [`super::SupervisorSeams::new_terminals`] (on in every shipped
//! supervisor, since `farhelm supervisor run` defaults to hosts; an
//! in-process harness turns it on by choice).
//!
//! A host launch is the tmux launch with a different terminal around the
//! same command. Everything up to the command is shared
//! ([`Supervisor::prepare_launch`]: the spec, the hook policy, the login
//! shell and the shim's command line, with the `systemd-run --user --scope`
//! wrapper inside it exactly as a tmux pane runs it); this module only
//! starts that command as a host's child, and answers the questions the
//! create's launch identity asks of a host:
//!
//! - **What the child gets** ([`host_child_env`]): the supervisor's own
//!   environment with the variables of Farhelm's terminal set over it
//!   (`TERM_PROGRAM=farhelm` among them) and nothing that names a tmux
//!   server or pane. The host itself starts with an
//!   empty environment and only the socket on its command line
//!   (`crate::hosts::spawn`), so the session's markers and credential,
//!   which reach the child through this environment and the launch spec,
//!   are never on the host: Stop's marker sweep never finds it, and a
//!   process that lives for weeks keeps no credential readable by others.
//! - **Which launch a host belongs to**: its tag, which names the session
//!   and the launch generation, and, until the launch is confirmed (which
//!   records the socket's name in the row's `pane`, the plan's D-14), the
//!   launch record below.
//! - **The launch record** ([`LaunchHostRecord`]): before the host's first
//!   stage is given the spec it starts the host from, the launch's
//!   artifacts record which socket the host will bind and which process the
//!   first stage is, pinned to its start identity. A supervisor that dies
//!   after that leaves a first stage that can still start the host on its
//!   own; the record is how a retry of the same create finds and fences it
//!   ([`Supervisor::host_launch_evidence`]). A supervisor that dies before
//!   it leaves a first stage that sees its stdin end without a spec and
//!   starts nothing.
//! - **What a failed start may have left** ([`HostLeft`]), from the first
//!   stage's exit status and the adoption, and how such a host is ended
//!   ([`Supervisor::end_launch_host`]).
//!
//! The host's child stays in the supervisor's own control group, as a tmux
//! pane does under the tmux server: a host gets no systemd scope of its
//! own. Its lifetime is the unit's (`KillMode=process` keeps it across
//! supervisor restarts, as it keeps tmux), and the agent inside keeps its
//! per-launch scope from the wrapper in its command.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use terminal_host::protocol::Size;
use tracing::warn;

use super::{
    FarhelmArgsMode, LaunchEvidence, LaunchesClosed, PreparedLaunch, ShellSource, Supervisor,
};
use crate::agent_kind::IntegrationSnapshot;
use crate::hosts::{
    AdoptError, Adoption, ChildSpec, ClientDeadlines, ClientError, ConnectError,
    DEFAULT_JOURNAL_CAP, GoneHost, HostConnection, HostName, HostOutcome, HostProcess, HostRole,
    HostTag, HostTerminal, HostTerminalOptions, Liveness, SpawnError, StopLimits, Termination,
    terminate_verified,
};
use crate::launch::LaunchArtifacts;
use crate::service::terminals::Terminal;
use terminal_host::protocol::Info;

// ---- The child's environment -------------------------------------------------------

/// `TERM` for a host's child: the terminal type the supervisor's emulator
/// implements (`farhelm_terminal_emulator`).
pub(crate) const HOST_TERM: &str = "xterm-256color";

/// `COLORTERM` for a host's child: the emulator draws 24-bit color, which
/// programs only use when this says so (`TERM` cannot).
const HOST_COLORTERM: &str = "truecolor";

/// `TERM_PROGRAM` for a host's child: the terminal is Farhelm's own, and
/// says so. Programs that adapt to the terminal they run in read this
/// variable, so it names the terminal the child really runs in rather than
/// one it does not (the old terminals' tmux, or whatever terminal the
/// supervisor was started from).
pub(crate) const HOST_TERM_PROGRAM: &str = "farhelm";

/// `TERM_PROGRAM_VERSION` for a host's child: this build's version, the
/// same one the emulator reports when a program asks for the terminal's
/// version (XTVERSION, `farhelm_terminal_emulator::VERSION_REPORT`), so that
/// the two ways a program can learn the terminal's version agree.
pub(crate) const HOST_TERM_PROGRAM_VERSION: &str = farhelm_proto::BUILD_VERSION;

/// What `PATH` is when neither the supervisor nor the launch environment
/// has one, so that the login shell and the launch shim can still find the
/// system's programs.
const DEFAULT_PATH: &str = "/usr/bin:/bin";

/// Variables a host's child never inherits from the supervisor, because
/// they describe something the child is not part of.
///
/// - `TMUX` and `TMUX_PANE` name a tmux server and a pane, and a host's
///   child is in neither. A supervisor started from inside the user's own
///   tmux has them; a child that inherited them would send a plain `tmux`
///   command to that server and believe it runs in one of its panes.
/// - `LISTEN_PID`, `LISTEN_FDS`, `LISTEN_FDNAMES`: systemd's socket
///   activation hand-off, which names descriptors given to the supervisor
///   process, not to the child.
///
/// The terminal's own variables (`TERM`, `COLORTERM`, `TERM_PROGRAM`,
/// `TERM_PROGRAM_VERSION`) are not here because [`host_child_env`] always
/// sets them, which replaces whatever the supervisor's terminal left.
const NOT_INHERITED: [&str; 5] = [
    "TMUX",
    "TMUX_PANE",
    "LISTEN_PID",
    "LISTEN_FDS",
    "LISTEN_FDNAMES",
];

/// The complete environment of a host's child.
///
/// The child gets the supervisor's own environment (`base`), which is what
/// SPEC.md's "as if the user had SSHed in" contract builds on, with
/// `launch_env` over it, and then the variables that describe the terminal
/// over both: [`HOST_TERM`], [`HOST_COLORTERM`], [`HOST_TERM_PROGRAM`] and
/// [`HOST_TERM_PROGRAM_VERSION`]. These are always set, never inherited, so
/// that a supervisor started from some other terminal (an iTerm, the user's
/// own tmux) passes none of that terminal's claims to the child. Then
/// `SHELL` (`shell`, the user's login shell) and `PWD` (`cwd`), and `PATH`
/// when nothing gave one ([`DEFAULT_PATH`]). The [`NOT_INHERITED`]
/// variables are removed, also when `launch_env` sets them.
///
/// Nothing here claims tmux: a host's child runs in no tmux, and a login
/// shell's startup file that starts tmux when `TMUX` is unset runs as it
/// would in any terminal outside tmux.
///
/// Sorted by name, so the same inputs give the same spec.
pub(crate) fn host_child_env(
    base: impl IntoIterator<Item = (OsString, OsString)>,
    launch_env: &[(String, String)],
    shell: &str,
    cwd: &str,
) -> Vec<(OsString, OsString)> {
    let mut env: BTreeMap<OsString, OsString> = base.into_iter().collect();
    for (name, value) in launch_env {
        env.insert(name.into(), value.into());
    }
    for name in NOT_INHERITED {
        env.remove(std::ffi::OsStr::new(name));
    }
    env.insert("TERM".into(), HOST_TERM.into());
    env.insert("COLORTERM".into(), HOST_COLORTERM.into());
    env.insert("TERM_PROGRAM".into(), HOST_TERM_PROGRAM.into());
    env.insert(
        "TERM_PROGRAM_VERSION".into(),
        HOST_TERM_PROGRAM_VERSION.into(),
    );
    env.entry("PATH".into())
        .or_insert_with(|| DEFAULT_PATH.into());
    env.insert("SHELL".into(), shell.into());
    env.insert("PWD".into(), cwd.into());
    env.into_iter().collect()
}

// ---- The launch record -------------------------------------------------------------

/// How long a retry's evidence waits for an interrupted start's recorded
/// first stage to finish before ending it with the verified signal.
///
/// A healthy first stage only forks the host and waits for it to bind and
/// start its child, well under a second. Ending one that is merely slow is
/// harmless: a second stage already forked goes on to bind the socket and
/// start the child, but then finds its readiness report unread and ends
/// both again (`crates/terminal-host/SPEC.md`, "Start and readiness"),
/// which the evidence waits for ([`STRAY_WAIT`]) before relaunching.
const STARTER_WAIT: Duration = Duration::from_secs(5);

/// How long a retry's evidence waits, once the recorded first stage is
/// gone and nothing answers at the recorded socket, for every process
/// whose command line still names that socket to go: a second stage forked
/// before its first stage ended, which may still bind the socket and start
/// the child before it notices and ends itself. One that does not go in
/// time leaves the answer unresolved.
const STRAY_WAIT: Duration = Duration::from_secs(5);

/// How often that wait looks at the first stage again.
const STARTER_POLL: Duration = Duration::from_millis(50);

/// How long a launch's runtime gets to confirm its host ended through the
/// protocol ([`HostTerminal::shutdown`]: its reply and shutdown deadlines,
/// and the wait for the host's process) before the host is ended by its
/// socket instead. The runtime takes a shutdown up only between its other
/// work, which a snapshot encode at full scrollback can hold for seconds.
const RUNTIME_SHUTDOWN_BOUND: Duration = Duration::from_secs(15);

/// The launch record's first line, which names its format.
const RECORD_HEADER: &str = "farhelm launch host 1";

/// Which socket a launch's host binds, and which first-stage process may
/// still start it: written to the launch's artifacts
/// ([`LaunchArtifacts::host`]) before that process is given the spec.
///
/// The record is what a later supervisor fences a start that outlived its
/// supervisor by (module docs, "The launch record"). It stays while the
/// launch is unconfirmed and its host may run; the rollback paths remove it
/// only once the host is confirmed ended, and a retry's takeover only once
/// the evidence found the first stage gone and nothing at the socket. The
/// cleanup after an agent's launch error leaves it: the error settles the
/// agent, not the host, which may still run with nothing else naming it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LaunchHostRecord {
    /// The socket's name in the state directory's socket directory.
    pub(crate) socket: HostName,
    /// The first stage, pinned to its start identity.
    pub(crate) starter: HostProcess,
}

impl LaunchHostRecord {
    /// The record's bytes: three lines, the header, `socket <name>` and
    /// `starter <pid> <start>`.
    pub(crate) fn encode(&self) -> String {
        format!(
            "{RECORD_HEADER}\nsocket {}\nstarter {} {}\n",
            self.socket, self.starter.pid, self.starter.start
        )
    }

    /// Read a record [`Self::encode`] wrote, refusing anything else.
    pub(crate) fn decode(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        if lines.next() != Some(RECORD_HEADER) {
            return Err("it is not a launch host record this build reads".into());
        }
        let socket = lines
            .next()
            .and_then(|line| line.strip_prefix("socket "))
            .and_then(HostName::parse)
            .ok_or("it names no socket")?;
        let (pid, start) = lines
            .next()
            .and_then(|line| line.strip_prefix("starter "))
            .and_then(|rest| rest.split_once(' '))
            .and_then(|(pid, start)| Some((pid.parse().ok()?, start.parse().ok()?)))
            .ok_or("it names no starter")?;
        if lines.next().is_some() {
            return Err("it has more than three lines".into());
        }
        Ok(Self {
            socket,
            starter: HostProcess { pid, start },
        })
    }
}

/// The launch record at `path`: `Ok(None)` when there is none, an error
/// when one is there and cannot be read, which says nothing about whether
/// the launch started a host.
pub(crate) async fn read_launch_record(path: &Path) -> anyhow::Result<Option<LaunchHostRecord>> {
    match tokio::fs::read_to_string(path).await {
        Ok(text) => LaunchHostRecord::decode(&text)
            .map(Some)
            .map_err(|why| anyhow::anyhow!("the launch host record {}: {why}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(anyhow::Error::new(error)
            .context(format!("reading the launch host record {}", path.display()))),
    }
}

/// What [`Supervisor::settle_launch_record`] found at a launch record's
/// socket, once the recorded first stage can start nothing more.
pub(crate) enum AtRecordedSocket {
    /// Nothing serves the socket and no process names it: no host runs
    /// there, and none can start.
    Gone,
    /// A host answered with its `Info`, over the connection that now owns
    /// it (boxed, being much the largest variant). Whose host it is, its
    /// tag says.
    Answered {
        connection: Box<HostConnection>,
        info: Info,
    },
    /// Something may serve the socket but did not take the connection
    /// usefully: busy, another protocol, no answer, or a process that still
    /// names the socket without accepting. Its peer, when one is known, is
    /// what the verified signal may end.
    NoAnswer(ConnectError),
    /// A host took the connection and did not report its `Info`.
    NoInfo {
        error: ClientError,
        peer: Option<HostProcess>,
    },
}

// ---- Starting a host ---------------------------------------------------------------

/// A launch's agent started in a host, a create's or a Restart's: what
/// [`Supervisor::spawn_agent_in_host`] produced.
pub(super) struct SpawnedInHost {
    /// The runtime adopted for the new host, reporting to the supervisor's
    /// liveness notifier.
    pub(super) host: HostTerminal,
    /// See `Spawned::artifacts`.
    pub(super) artifacts: LaunchArtifacts,
    /// See `Spawned::hooked`.
    pub(super) hooked: bool,
    /// [`Supervisor::host_scan`] held shared since just before the host's
    /// first stage started: the caller keeps it until the host's entry is
    /// on the session map, or releases it before giving the host up.
    pub(super) spawning: tokio::sync::OwnedRwLockReadGuard<()>,
}

/// Why [`Supervisor::spawn_agent_in_host`] did not start the agent.
pub(super) enum HostSpawnFailure {
    /// The spec never landed, so nothing was started (`prepare_launch`).
    Spec(anyhow::Error),
    /// The spec landed and the host did not start, or started and could not
    /// be adopted. `left` says whether a host may remain; the shim may never
    /// run to consume the spec, so the artifacts are the caller's to remove
    /// once it knows.
    Host {
        artifacts: LaunchArtifacts,
        error: anyhow::Error,
        left: HostLeft,
    },
    /// The start was interrupted on purpose ([`super::HostStartInterrupt`]),
    /// as a supervisor crash would interrupt it: the caller returns at once
    /// with no cleanup, leaving the row, the record and the first stage.
    #[cfg(any(test, feature = "test-seams"))]
    Interrupted(anyhow::Error),
}

/// What a failed host start may have left running, from what the start
/// itself could tell.
pub(super) enum HostLeft {
    /// No host: the first stage refused (and a refusing first stage leaves
    /// nothing, `crates/terminal-host/SPEC.md`, "Start and readiness"), the
    /// spawn confirmed an unfinished start gone, nothing was attempted, the
    /// first stage was never given its spec, or the host ended before it
    /// could be adopted. The child may have run for a moment; the caller's
    /// process sweep is for that.
    Nothing,
    /// A host may still run at `socket`: an unfinished start the spawn could
    /// not confirm ended, or a host that reported ready but cannot be used.
    /// Whatever answers there is this launch's own, since its socket name
    /// was drawn fresh for it.
    MayRemain { socket: PathBuf },
}

impl Supervisor {
    /// The program this supervisor starts hosts from: its own executable
    /// ([`crate::hosts::own_host_program`]), or the test seam's
    /// replacement for an in-process supervisor.
    pub(super) fn host_program(&self) -> PathBuf {
        #[cfg(any(test, feature = "test-seams"))]
        {
            if let Some(program) = &self.seams.host_program {
                return program.clone();
            }
        }
        crate::hosts::own_host_program(&self.farhelm_exe)
    }

    /// `SHELL` for a host's child: the user's login shell. That is the
    /// launch's own shell in production. A supervisor whose launch shell is
    /// a test seam (a stand-in that records what it was given, say) still
    /// gives the user's login shell, so that the stand-in changes only how
    /// the launch starts, not which shell the child's programs see as the
    /// user's.
    pub(super) async fn host_shell_variable(&self, launch_shell: &str) -> String {
        match &self.seams.launch_shell {
            Some(_) => crate::launch::resolve_shell().await,
            None => launch_shell.to_string(),
        }
    }

    /// Start one launch's agent in a new terminal host, tagged with its
    /// session and launch generation, and adopt the host: the host
    /// counterpart of [`Self::spawn_agent`]'s new tmux session.
    ///
    /// The host's child runs exactly the command a tmux pane would
    /// ([`Self::prepare_launch`]), in `cwd`, at `cols` x `rows` (clamped
    /// as tmux clamps them), with the environment of [`host_child_env`]
    /// and the default journal cap. `initial_snapshot` seeds its terminal:
    /// a create has none, and a Restart gives the old terminal's history
    /// (`host_restart`). The host is started from this supervisor's own program
    /// ([`Self::host_program`]) at a socket name drawn here; before its
    /// first stage is given the spec, the launch record (module docs) is
    /// written, and a start whose first stage cannot be pinned or whose
    /// record cannot be written is ended there, with nothing started. The
    /// host is then adopted as freshly spawned, reporting to the
    /// supervisor's liveness notifier, before this returns.
    ///
    /// Takes [`Self::host_scan`] shared just before the first stage starts
    /// and hands the guard over with the host ([`SpawnedInHost::spawning`]);
    /// a failure releases it before returning, so the caller may end what
    /// was left ([`Self::end_launch_host`] takes the lock exclusively).
    #[allow(clippy::too_many_arguments)]
    #[allow(
        clippy::result_large_err,
        reason = "one launch per call, never a hot path; the failure hands the caller the \
                  launch's artifact owner by value, for cleanup once it knows what is left"
    )]
    pub(super) async fn spawn_agent_in_host(
        &self,
        id: &str,
        session_token: &str,
        generation: i64,
        argv: Vec<String>,
        shell_source: Option<ShellSource<'_>>,
        snapshot: &IntegrationSnapshot,
        args_mode: FarhelmArgsMode,
        cwd: &str,
        cols: u16,
        rows: u16,
        scope: Option<&str>,
        preparation: Option<crate::launch::CheckoutPreparation>,
        initial_prompt: Option<(farhelm_proto::LaunchHarness, &str)>,
        container_config: Option<&farhelm_proto::ContainerConfig>,
        initial_snapshot: Option<Vec<u8>>,
    ) -> Result<SpawnedInHost, HostSpawnFailure> {
        let PreparedLaunch {
            cwd,
            artifacts,
            shell,
            cmd,
            hooked,
            hook_skip,
        } = self
            .prepare_launch(
                id,
                session_token,
                generation,
                argv,
                shell_source,
                snapshot,
                args_mode,
                cwd,
                scope,
                preparation,
                initial_prompt,
                container_config,
            )
            .await
            .map_err(HostSpawnFailure::Spec)?;
        // Counted from before the host starts until it is tracked below (or
        // this launch fails), as for a tmux window: a stop in between still
        // waits for it, and once the stop's wait has ended nothing starts.
        let _starting = match self.starting_launch() {
            Ok(starting) => starting,
            Err(LaunchesClosed) => {
                return Err(HostSpawnFailure::Host {
                    artifacts,
                    error: anyhow::anyhow!(
                        "Farhelm's supervisor is stopping, so no terminal host was started"
                    ),
                    left: HostLeft::Nothing,
                });
            }
        };
        let shell_variable = self.host_shell_variable(&shell).await;
        let env = host_child_env(
            std::env::vars_os(),
            &self.seams.launch_env,
            &shell_variable,
            &cwd,
        );
        let child = ChildSpec {
            argv: cmd.into_iter().map(OsString::from).collect(),
            cwd: PathBuf::from(&cwd),
            env,
            size: Size {
                cols: cols.clamp(1, 10_000),
                rows: rows.clamp(1, 10_000),
            },
            journal_cap: DEFAULT_JOURNAL_CAP,
            initial_snapshot,
        };
        let tag = HostTag {
            session_id: id.to_string(),
            created_ns: crate::hosts::tag::creation_stamp(),
            role: HostRole::Agent { generation },
        };
        // Every failure from here to the end of the start says nothing
        // started: the first stage is killed (by dropping it) before it is
        // given its spec.
        let not_started = |artifacts, error: anyhow::Error| HostSpawnFailure::Host {
            artifacts,
            error: error.context("starting the session's terminal host"),
            left: HostLeft::Nothing,
        };
        let name = match crate::hosts::free_host_name(&self.state_dir) {
            Ok(name) => name,
            Err(error) => return Err(not_started(artifacts, error.into())),
        };
        let spawning = self.host_scan.clone().read_owned().await;
        let starting = match crate::hosts::start_host(
            &self.host_program(),
            &self.state_dir,
            name,
            child,
            &tag,
        ) {
            Ok(starting) => starting,
            Err(error) => return Err(not_started(artifacts, error.into())),
        };
        let Some(starter) = starting.starter() else {
            return Err(not_started(
                artifacts,
                anyhow::anyhow!("its first stage's process could not be read"),
            ));
        };
        let record = LaunchHostRecord {
            socket: starting.name().clone(),
            starter,
        };
        if let Err(error) =
            crate::write_private_file(artifacts.host(), record.encode().as_bytes()).await
        {
            return Err(not_started(
                artifacts,
                anyhow::Error::new(error).context("recording the launch's terminal host"),
            ));
        }
        #[cfg(any(test, feature = "test-seams"))]
        {
            if let Some(interrupt) = self.seams.faults.host_start_interrupt()
                && let Err(error) = interrupt()
            {
                if let Err(delivery) = starting.deliver_and_abandon_for_test().await {
                    warn!(session = %id, error = %delivery,
                        "the interrupted start's spec was not delivered");
                }
                return Err(HostSpawnFailure::Interrupted(
                    error.context(super::SimulatedCrash),
                ));
            }
        }
        let spawned = match starting.finish().await {
            Ok(spawned) => spawned,
            Err(error) => {
                let left = match &error {
                    SpawnError::MayRemain { socket, .. } => HostLeft::MayRemain {
                        socket: socket.clone(),
                    },
                    SpawnError::StateDir(_)
                    | SpawnError::HostsDir(_)
                    | SpawnError::Tag(_)
                    | SpawnError::Name(_)
                    | SpawnError::Start { .. }
                    | SpawnError::Refused { .. }
                    | SpawnError::Abandoned { .. } => HostLeft::Nothing,
                };
                return Err(HostSpawnFailure::Host {
                    artifacts,
                    error: anyhow::Error::new(error)
                        .context("starting the session's terminal host"),
                    left,
                });
            }
        };
        let mut options = HostTerminalOptions {
            notifier: Some(self.host_liveness.clone()),
            ..Default::default()
        };
        if let Some(adjust) = self.seams.faults.host_terminal_options() {
            adjust(&mut options);
        }
        let adopted = HostTerminal::adopt(&spawned.socket, Adoption::Fresh, options).await;
        let host = match adopted {
            Ok(host) => host,
            Err(error) => {
                let left = match &error {
                    AdoptError::Gone(_) => HostLeft::Nothing,
                    AdoptError::Unavailable { .. } => HostLeft::MayRemain {
                        socket: spawned.socket.clone(),
                    },
                };
                return Err(HostSpawnFailure::Host {
                    artifacts,
                    error: anyhow::Error::new(error)
                        .context("connecting to the session's new terminal host"),
                    left,
                });
            }
        };
        // Told once the launch is known to be running, as for tmux.
        if let Some(skip) = hook_skip {
            self.notify_session(
                id,
                generation,
                super::super::notifications::NotificationKind::HookNotAdded,
                &skip.text(),
            )
            .await;
        }
        self.track_pending_hold(artifacts.spec(), Terminal::Host(host.clone()));
        Ok(SpawnedInHost {
            host,
            artifacts,
            hooked,
            spawning,
        })
    }

    /// End a host this supervisor started for `session_id`, or a host its
    /// runtime holds for a session being deleted, confirm it ended, and
    /// remove the socket file it left.
    ///
    /// Through its runtime's `Shutdown` first, when `host` is that runtime,
    /// within [`RUNTIME_SHUTDOWN_BOUND`]. When that does not confirm the
    /// end (the bound ran out, the runtime had already ended as running,
    /// exited or unavailable, or an earlier caller stopped it), the runtime
    /// is stopped ([`HostTerminal::abort`], so that it does not take the host
    /// back) and the process it adopted ([`HostTerminal::adopted_process`],
    /// fixed at adoption) is ended with the verified signal, without
    /// connecting to the socket again: whatever serves the socket now may
    /// have replaced the adopted host (renamed over its path), and is never
    /// shut down or signalled for it. A replacement's socket file is left in
    /// place ([`Self::remove_ended_socket`] removes only a socket nothing
    /// serves).
    ///
    /// With no adopted process (no runtime, as for a start that may have
    /// left a host the spawn could not hand over, or a platform that named
    /// no peer), the socket is connected to, and only a host whose tag names
    /// `session_id` is shut down, over that same connection; a host that
    /// takes the connection but cannot be read is ended by the verified
    /// signal to the process the connection named, which checks that its
    /// command line names this socket. A host with another tag, or one whose
    /// tag cannot be read, is never shut down, and nothing at all with no
    /// process known is signalled: both are errors. Every wait here is
    /// bounded.
    ///
    /// Ending the host closes its pty, which hangs up the child; the
    /// caller's process sweep is what reaches descendants that outlive the
    /// hangup. `Err` carries why the host may still run.
    ///
    /// The caller must not hold [`Self::host_scan`]: removing the socket a
    /// signal left takes it exclusively.
    pub(crate) async fn end_launch_host(
        &self,
        host: Option<&HostTerminal>,
        socket: &Path,
        session_id: &str,
    ) -> Result<(), String> {
        if let Some(host) = host {
            match tokio::time::timeout(RUNTIME_SHUTDOWN_BOUND, host.shutdown()).await {
                Ok(Ok(report)) if report.ended => {
                    self.remove_ended_socket(socket).await;
                    return Ok(());
                }
                Ok(Err(Liveness::Gone)) => {
                    self.remove_ended_socket(socket).await;
                    return Ok(());
                }
                Ok(Ok(report)) => warn!(
                    socket = %socket.display(), goodbye = report.goodbye,
                    "a terminal host was not confirmed ended after Shutdown; ending the \
                     process its runtime adopted"
                ),
                Ok(Err(liveness)) => warn!(
                    socket = %socket.display(), ?liveness,
                    "a terminal host can no longer be ended through its runtime; ending the \
                     process its runtime adopted"
                ),
                Err(_) => warn!(
                    socket = %socket.display(), bound = ?RUNTIME_SHUTDOWN_BOUND,
                    "a terminal host runtime did not finish its Shutdown in time; ending the \
                     process it adopted"
                ),
            }
            host.abort();
            if let Some(adopted) = host.adopted_process() {
                return self.end_adopted_process(adopted, socket).await;
            }
        }
        self.end_host_tagged_for(socket, session_id).await
    }

    /// End `adopted`, the process a runtime adopted as the host on `socket`,
    /// with the verified signal and nothing else (see
    /// [`Self::end_launch_host`]): ended once that process is seen gone,
    /// whether by the signal or before it. Its socket file is removed only if
    /// nothing serves it now.
    pub(super) async fn end_adopted_process(
        &self,
        adopted: HostProcess,
        socket: &Path,
    ) -> Result<(), String> {
        match terminate_verified(&adopted, socket).await {
            Ok(Termination::AlreadyGone | Termination::Terminated | Termination::Killed) => {
                self.remove_ended_socket(socket).await;
                Ok(())
            }
            Ok(Termination::StillRunning) => Err(format!(
                "the terminal host's process {} survived SIGTERM and SIGKILL",
                adopted.pid
            )),
            Err(error) => Err(format!(
                "the terminal host's process {} could not be ended: {error}",
                adopted.pid
            )),
        }
    }

    /// End the host at `socket` with no adopted process to go by (see
    /// [`Self::end_launch_host`]): `Shutdown` only to a host whose tag
    /// names `session_id`, over the connection that read the tag; the
    /// verified signal to the connection's process for one that cannot be
    /// read; gone when nothing serves the socket.
    async fn end_host_tagged_for(&self, socket: &Path, session_id: &str) -> Result<(), String> {
        let limits = StopLimits::default();
        let outcome = match HostConnection::connect_with(socket, limits.client).await {
            Err(ConnectError::Gone(gone)) => {
                HostOutcome::Stopped(crate::hosts::Stopped::Gone(gone))
            }
            Err(error @ (ConnectError::Busy { peer } | ConnectError::Unavailable { peer, .. })) => {
                match peer {
                    Some(peer) => {
                        crate::hosts::stop::signal(&peer, socket, &error.to_string(), limits.client)
                            .await
                    }
                    None => {
                        HostOutcome::Unconfirmed(format!("{error}, and its process is unknown"))
                    }
                }
            }
            Ok(mut connection) => match connection.info().await {
                Ok(info) => match HostTag::decode(&info.tag) {
                    Ok(tag) if tag.session_id == session_id => {
                        crate::hosts::stop::shut_down(connection, socket, limits).await
                    }
                    Ok(tag) => HostOutcome::Unconfirmed(format!(
                        "it is served by a host for another terminal ({tag:?}), which was left \
                         alone"
                    )),
                    Err(unreadable) => HostOutcome::Unconfirmed(format!(
                        "it is served by a host whose tag cannot be read ({unreadable}), which \
                         was left alone"
                    )),
                },
                Err(error) => match connection.peer() {
                    Some(peer) => {
                        crate::hosts::stop::signal(&peer, socket, &error.to_string(), limits.client)
                            .await
                    }
                    None => HostOutcome::Unconfirmed(format!(
                        "it did not report itself ({error}), and its process is unknown"
                    )),
                },
            },
        };
        match outcome {
            HostOutcome::Stopped(_) => {
                self.remove_ended_socket(socket).await;
                Ok(())
            }
            HostOutcome::Unconfirmed(why) => Err(why),
        }
    }

    /// Remove the socket file of a host just confirmed ended, if one is left
    /// (a host ended by a signal leaves it), so that a later look at the
    /// socket finds nothing there at once rather than after the refusal
    /// window. Through the registry's re-verifying removal, which judges
    /// the socket gone again before it unlinks anything, under
    /// [`Self::host_scan`] held exclusively, as that removal requires (no
    /// spawn in the directory meanwhile). Best effort: a file it cannot
    /// remove is only logged, since the host is gone either way.
    pub(crate) async fn remove_ended_socket(&self, socket: &Path) {
        let _no_spawns = self.host_scan.write().await;
        remove_ended_socket_file(socket).await;
    }

    /// Whether session `session_id`'s first launch (generation 0) started a
    /// terminal host, for a retried create whose row does not show it
    /// launched: the last source of `reserved_launch_evidence` for a host
    /// row, and the host counterpart of asking tmux for the reserved session.
    ///
    /// The bias is the evidence's: relaunching needs positive evidence of
    /// absence, so anything that cannot be settled is unresolved. In order:
    ///
    /// - **The session's own published host.** A create whose confirmation
    ///   failed and whose host could not be confirmed ended keeps its row
    ///   and publishes that host, with the row still showing no launch. A
    ///   host on the session's entry is the launch (`Present`), whether or
    ///   not its runtime still answers; nothing here connects to it.
    /// - **The launch record** ([`LaunchHostRecord`]). None means no host
    ///   start ever got as far as being given a spec (the record is written
    ///   first), so `Absent`; one that cannot be read is unresolved.
    /// - **Its first stage.** While the recorded process still runs (a start
    ///   that outlived the supervisor that made it), it may yet start the
    ///   host: it is waited for up to [`STARTER_WAIT`], then ended with the
    ///   verified signal, which checks that it is still that process and
    ///   still names the recorded socket. A first stage that cannot be
    ///   confirmed gone leaves the answer unresolved.
    /// - **The recorded socket.** A host there whose tag names this
    ///   session's first launch is `Present`; anything else that answers
    ///   (unreadable, another tag) is unresolved. Nothing there is not yet
    ///   absence: a second stage the first stage forked before it ended can
    ///   still bind the socket and start the child (the host binds and starts
    ///   its child before it reports ready, and only then finds its first
    ///   stage gone and ends both). Every such process names the socket on
    ///   its command line, as a fork of the first stage, so the socket is
    ///   looked at again until no live process names it (the client's own
    ///   rule: a socket is never gone while one does), for up to
    ///   [`STRAY_WAIT`]: then `Absent`, and unresolved when one stays or the
    ///   process table cannot be searched.
    ///
    /// Connecting takes over the host at the socket. No runtime of this
    /// supervisor holds it (a published one returned above, and the same
    /// session's create is excluded by its intent key); the connection ends
    /// when this returns, which leaves the host as it was.
    pub(super) async fn host_launch_evidence(&self, session_id: &str) -> LaunchEvidence {
        let published = self.sessions.lock().await.get(session_id).cloned();
        if let Some(entry) = published {
            match &entry.terminal {
                Some(Terminal::Host(_)) => return LaunchEvidence::Present,
                Some(Terminal::Tmux(_)) | None => {}
            }
        }
        let artifacts = LaunchArtifacts::for_launch(&self.state_dir, session_id, 0);
        let record = match read_launch_record(artifacts.host()).await {
            Ok(Some(record)) => record,
            Ok(None) => return LaunchEvidence::Absent,
            Err(error) => return LaunchEvidence::Unresolved(error),
        };
        let socket = match self.recorded_socket(&record) {
            Ok(socket) => socket,
            Err(error) => return LaunchEvidence::Unresolved(error),
        };
        let info = match self.settle_launch_record(&record, &socket).await {
            Err(why) => {
                return LaunchEvidence::Unresolved(anyhow::anyhow!(
                    "the interrupted launch's terminal host start (process {}) could not be \
                     confirmed ended: {why}",
                    record.starter.pid
                ));
            }
            Ok(AtRecordedSocket::Gone) => return LaunchEvidence::Absent,
            Ok(AtRecordedSocket::NoAnswer(error)) => {
                return LaunchEvidence::Unresolved(anyhow::Error::new(error).context(format!(
                    "the interrupted launch's terminal host at {} cannot be read, or a \
                         process that may yet start it still names its socket",
                    socket.display()
                )));
            }
            Ok(AtRecordedSocket::NoInfo { error, .. }) => {
                return LaunchEvidence::Unresolved(anyhow::Error::new(error).context(format!(
                    "the interrupted launch's terminal host at {} did not report itself",
                    socket.display()
                )));
            }
            Ok(AtRecordedSocket::Answered { info, .. }) => info,
        };
        match HostTag::decode(&info.tag) {
            Ok(tag)
                if tag.session_id == session_id
                    && tag.role == (HostRole::Agent { generation: 0 }) =>
            {
                LaunchEvidence::Present
            }
            Ok(tag) => LaunchEvidence::Unresolved(anyhow::anyhow!(
                "the interrupted launch's terminal host socket {} is served by a host for \
                 another terminal ({tag:?})",
                socket.display()
            )),
            Err(unreadable) => {
                LaunchEvidence::Unresolved(anyhow::Error::new(unreadable).context(format!(
                    "the interrupted launch's terminal host at {} carries a tag this build \
                     cannot read",
                    socket.display()
                )))
            }
        }
    }

    /// The full path of the socket `record` names, in this supervisor's
    /// socket directory, spelled absolute as the spawn spells it (which the
    /// verified signal compares byte for byte).
    pub(crate) fn recorded_socket(&self, record: &LaunchHostRecord) -> anyhow::Result<PathBuf> {
        self.named_socket(&record.socket)
    }

    /// The full path of the socket called `name` in this supervisor's
    /// socket directory, spelled as [`Self::recorded_socket`] spells a
    /// record's, so that Delete finds a placeholder's socket and a record's
    /// equal exactly when they are one socket.
    pub(crate) fn named_socket(&self, name: &crate::hosts::HostName) -> anyhow::Result<PathBuf> {
        let state_dir = std::path::absolute(&self.state_dir)
            .map_err(|error| anyhow::Error::new(error).context("resolving the state directory"))?;
        Ok(name.socket_in(&crate::hosts::hosts_dir(&state_dir)))
    }

    /// Settle what a launch record's start left at its socket, for a caller
    /// that holds no runtime for it: a retried create
    /// ([`Self::host_launch_evidence`]) and Delete (`service::session_hosts`);
    /// a reload settles a tab host's record the same way, through
    /// [`settle_launch_record_with`].
    ///
    /// First the recorded first stage is fenced ([`fence_starter`]:
    /// waited for, then ended with the verified signal), so that it cannot
    /// start a host after this answers; `Err` with why when it may still run.
    /// Then the socket is connected to (which takes over any host there). A
    /// socket nothing serves is not yet [`AtRecordedSocket::Gone`] while a
    /// live process still names it on its command line: a second stage the
    /// first stage forked before it ended still binds the socket and starts
    /// the child before its readiness report finds the first stage gone and
    /// it ends both, so the socket is looked at again for up to
    /// [`STRAY_WAIT`] while such a process remains, and one that stays is
    /// [`AtRecordedSocket::NoAnswer`] with that process as its peer.
    pub(crate) async fn settle_launch_record(
        &self,
        record: &LaunchHostRecord,
        socket: &Path,
    ) -> Result<AtRecordedSocket, String> {
        settle_launch_record_with(record, socket, &self.seams.faults).await
    }
}

/// [`Supervisor::settle_launch_record`] with the test hooks given as
/// `faults` rather than read from a supervisor: for a reload, which settles a
/// tab host's interrupted start before it accepts that the start left
/// nothing (`host_tabs`, "After a restart"). The constructor's reload runs
/// before any supervisor exists; `serve`'s reload calls it the same way.
pub(crate) async fn settle_launch_record_with(
    record: &LaunchHostRecord,
    socket: &Path,
    faults: &super::FaultHooks,
) -> Result<AtRecordedSocket, String> {
    fence_starter(record.starter, socket, faults).await?;
    // The client judges a socket gone only while no live process names
    // it (`hosts::client`, "Never gone while a host process serves the
    // socket"); one that does, with nothing listening yet, is
    // "not accepting", which is what a second stage still on its way
    // to binding looks like.
    let strays_until = tokio::time::Instant::now() + STRAY_WAIT;
    let mut announced = false;
    let mut connection = loop {
        match HostConnection::connect(socket).await {
            Ok(connection) => break connection,
            Err(ConnectError::Gone(_)) => return Ok(AtRecordedSocket::Gone),
            Err(ConnectError::Unavailable {
                reason: crate::hosts::Unavailable::NotAccepting,
                peer,
            }) if tokio::time::Instant::now() < strays_until => {
                tracing::debug!(socket = %socket.display(), ?peer,
                        "a process still names a recorded host socket; waiting");
                if !announced {
                    announced = true;
                    if let Some(waiting) = faults.launch_stray_waiting() {
                        waiting();
                    }
                }
                tokio::time::sleep(STARTER_POLL).await;
            }
            Err(error) => return Ok(AtRecordedSocket::NoAnswer(error)),
        }
    };
    match connection.info().await {
        Ok(info) => Ok(AtRecordedSocket::Answered {
            connection: Box::new(connection),
            info,
        }),
        Err(error) => Ok(AtRecordedSocket::NoInfo {
            error,
            peer: connection.peer(),
        }),
    }
}

/// Make sure a recorded first stage can fork nothing more: `Ok` once it is
/// gone, by itself within [`STARTER_WAIT`] or by the verified signal after
/// it; `Err` with why it may still run.
async fn fence_starter(
    starter: HostProcess,
    socket: &Path,
    faults: &super::FaultHooks,
) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + STARTER_WAIT;
    let mut announced = false;
    loop {
        if !starter.is_running()? {
            return Ok(());
        }
        if !announced {
            announced = true;
            if let Some(waiting) = faults.launch_starter_waiting() {
                waiting();
            }
        }
        if tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(STARTER_POLL).await;
    }
    match terminate_verified(&starter, socket).await {
        Ok(Termination::AlreadyGone | Termination::Terminated | Termination::Killed) => Ok(()),
        Ok(Termination::StillRunning) => Err("it survived SIGTERM and SIGKILL".into()),
        Err(error) => Err(error.to_string()),
    }
}

/// [`Supervisor::remove_ended_socket`] without taking the lock it takes:
/// for a caller that already keeps host spawns out of the socket directory
/// (holding `host_scan` exclusively, or running before any supervisor
/// exists, as the constructor's reload does).
pub(super) async fn remove_ended_socket_file(socket: &Path) {
    let gone = match GoneHost::ended_at(socket, ClientDeadlines::default()) {
        Ok(Some(gone)) => gone,
        Ok(None) => return,
        Err(error) => {
            warn!(socket = %socket.display(), %error,
                "could not look at an ended terminal host's socket file");
            return;
        }
    };
    match gone.remove_socket().await {
        crate::hosts::SocketRemoval::Removed | crate::hosts::SocketRemoval::AlreadyGone => {}
        other => warn!(socket = %socket.display(), outcome = ?other,
            "left an ended terminal host's socket file in place"),
    }
}

#[cfg(test)]
mod tests {
    //! The child's environment and the launch record, as pure functions of
    //! their inputs. That a real agent launched in a host gets this
    //! environment, and that the record fences a start that outlived its
    //! supervisor, is shown against real hosts in
    //! `crates/farhelm/tests/e2e/host_creates.rs`.

    use super::*;

    fn env(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
        pairs
            .iter()
            .map(|(name, value)| (OsString::from(name), OsString::from(value)))
            .collect()
    }

    fn get<'a>(env: &'a [(OsString, OsString)], name: &str) -> Option<&'a str> {
        env.iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.to_str().unwrap())
    }

    /// The supervisor's environment reaches the child, with Farhelm's
    /// terminal variables set over it and an outer tmux's or terminal's
    /// claims replaced or removed.
    ///
    /// Why: the child's environment is SPEC.md's "as if the user had
    /// SSHed in" contract, so the supervisor's own variables must pass
    /// through; and a new terminal must not claim to be tmux or the
    /// terminal the supervisor happened to start from, since programs adapt
    /// to what these variables say. Spec: from a supervisor environment
    /// holding `HOME`, `PATH`, a `FARHELM_` variable, an outer tmux's
    /// `TMUX`/`TMUX_PANE`, an outer terminal's `TERM`, `TERM_PROGRAM`,
    /// `TERM_PROGRAM_VERSION` and `COLORTERM`, and systemd's `LISTEN_*`, the
    /// child gets `HOME`, `PATH` and the `FARHELM_` variable unchanged,
    /// `TERM=xterm-256color`, `COLORTERM=truecolor`, `TERM_PROGRAM=farhelm`,
    /// `TERM_PROGRAM_VERSION` set to this build's version, `SHELL` and `PWD`
    /// from the arguments, and no `TMUX`, `TMUX_PANE` or `LISTEN_*`; the
    /// result is sorted by name.
    #[test]
    fn the_child_gets_the_supervisors_environment_in_farhelms_terminal() {
        let child = host_child_env(
            env(&[
                ("HOME", "/home/someone"),
                ("PATH", "/opt/bin:/usr/bin"),
                ("FARHELM_TMUX", "/opt/tmux"),
                ("TMUX", "/tmp/tmux-1000/default,42,0"),
                ("TMUX_PANE", "%3"),
                ("TERM", "screen"),
                ("TERM_PROGRAM", "iTerm.app"),
                ("TERM_PROGRAM_VERSION", "3.5"),
                ("COLORTERM", "24bit"),
                ("LISTEN_PID", "1"),
                ("LISTEN_FDS", "1"),
                ("LISTEN_FDNAMES", "x"),
            ]),
            &[],
            "/bin/zsh",
            "/work",
        );
        assert_eq!(
            child,
            env(&[
                ("COLORTERM", "truecolor"),
                ("FARHELM_TMUX", "/opt/tmux"),
                ("HOME", "/home/someone"),
                ("PATH", "/opt/bin:/usr/bin"),
                ("PWD", "/work"),
                ("SHELL", "/bin/zsh"),
                ("TERM", "xterm-256color"),
                ("TERM_PROGRAM", "farhelm"),
                ("TERM_PROGRAM_VERSION", farhelm_proto::BUILD_VERSION),
            ])
        );
    }

    /// `TERM_PROGRAM_VERSION` is the version the emulator reports to a
    /// program that asks the terminal for its version.
    ///
    /// Why: a program can learn the terminal's version from the environment
    /// or by asking the terminal (XTVERSION); two different answers would
    /// make one of them a lie. Spec: the emulator's XTVERSION answer is
    /// `DCS > | Farhelm <TERM_PROGRAM_VERSION> ST`.
    #[test]
    fn the_version_is_the_one_the_emulator_reports() {
        assert_eq!(
            farhelm_terminal_emulator::VERSION_REPORT,
            format!("\x1bP>|Farhelm {HOST_TERM_PROGRAM_VERSION}\x1b\\")
        );
    }

    /// The launch environment applies over the supervisor's own, under the
    /// terminal's variables, and cannot bring back what a child never
    /// inherits.
    ///
    /// Why: the launch environment carries a tab's markers and credential
    /// and, in tests, a launch's own `HOME`; it must win over the
    /// supervisor's values, but the terminal's description is not the
    /// launch's to change. Spec: a launch environment setting `HOME`,
    /// `PATH`, `TERM`, `TERM_PROGRAM`, `TERM_PROGRAM_VERSION`, `SHELL` and
    /// `TMUX` gives the child its `HOME` and `PATH`, the terminal's `TERM`,
    /// `TERM_PROGRAM` and `TERM_PROGRAM_VERSION`, `SHELL` from the argument
    /// and no `TMUX`.
    #[test]
    fn the_launch_environment_applies_under_the_terminals_variables() {
        let child = host_child_env(
            env(&[("HOME", "/home/someone"), ("PATH", "/usr/bin")]),
            &[
                ("HOME".into(), "/fixture/home".into()),
                ("PATH".into(), "/fixture/bin".into()),
                ("TERM".into(), "dumb".into()),
                ("TERM_PROGRAM".into(), "fixture".into()),
                ("TERM_PROGRAM_VERSION".into(), "0.0.1".into()),
                ("SHELL".into(), "/fixture/sh".into()),
                ("TMUX".into(), "/fixture/tmux,1,0".into()),
            ],
            "/bin/bash",
            "/",
        );
        assert_eq!(get(&child, "HOME"), Some("/fixture/home"));
        assert_eq!(get(&child, "PATH"), Some("/fixture/bin"));
        assert_eq!(get(&child, "TERM"), Some(HOST_TERM));
        assert_eq!(get(&child, "TERM_PROGRAM"), Some(HOST_TERM_PROGRAM));
        assert_eq!(
            get(&child, "TERM_PROGRAM_VERSION"),
            Some(HOST_TERM_PROGRAM_VERSION)
        );
        assert_eq!(get(&child, "SHELL"), Some("/bin/bash"));
        assert_eq!(get(&child, "TMUX"), None);
    }

    /// A supervisor without `PATH` gives the child a minimal default one,
    /// unless the launch environment gives one.
    ///
    /// Why: a login shell started with no `PATH` cannot find the programs
    /// its startup files run. Spec: no
    /// `PATH` anywhere gives `/usr/bin:/bin`; a launch environment `PATH`
    /// with none in the supervisor's is kept.
    #[test]
    fn without_a_path_the_child_gets_a_default() {
        let child = host_child_env(env(&[]), &[], "/bin/sh", "/");
        assert_eq!(get(&child, "PATH"), Some(DEFAULT_PATH));
        let child = host_child_env(
            env(&[]),
            &[("PATH".into(), "/fixture/bin".into())],
            "/bin/sh",
            "/",
        );
        assert_eq!(get(&child, "PATH"), Some("/fixture/bin"));
    }

    /// The launch record reads back what was written, and refuses anything
    /// it did not write.
    ///
    /// Why: the record is what fences a start that outlived its supervisor;
    /// a damaged one read as some other socket or process would point the
    /// retry's verified signal at the wrong thing, and one read as absent
    /// would let the retry relaunch beside a running host. Spec: an encoded
    /// record decodes to itself; a record with another header, a socket
    /// name that is not a host name (a path, the wrong length), a starter
    /// that is not two numbers, a missing line or an extra line is refused.
    #[test]
    fn the_launch_record_round_trips_and_refuses_anything_else() {
        let record = LaunchHostRecord {
            socket: HostName::parse("abcdefgh").unwrap(),
            starter: HostProcess {
                pid: 4242,
                start: 987_654_321,
            },
        };
        let text = record.encode();
        assert_eq!(LaunchHostRecord::decode(&text), Ok(record));
        for damaged in [
            text.replace(RECORD_HEADER, "farhelm launch host 2"),
            text.replace("abcdefgh", "../../etc"),
            text.replace("abcdefgh", "abcdefg"),
            text.replace("4242 987654321", "4242"),
            text.replace("4242 ", "x "),
            text.lines().take(2).collect::<Vec<_>>().join("\n"),
            format!("{text}extra\n"),
            String::new(),
        ] {
            assert!(
                LaunchHostRecord::decode(&damaged).is_err(),
                "accepted {damaged:?}"
            );
        }
    }
}
