//! Starting a create's agent in a terminal host, behind the internal switch
//! [`super::SupervisorSeams::new_terminals`].
//!
//! A host launch is the tmux launch with a different terminal around the
//! same command. Everything up to the command is shared
//! ([`Supervisor::prepare_launch`]: the spec, the hook policy, the login
//! shell and the shim's command line, with the `systemd-run --user --scope`
//! wrapper inside it exactly as a tmux pane runs it); this module only
//! starts that command as a host's child, and answers the questions the
//! create's launch identity asks of a host:
//!
//! - **What the child gets** ([`host_child_env`]): the environment a tmux
//!   pane's command gets today (the plan's D-13), with the startup-file
//!   protection described there (D-12). The host itself starts with an
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
    AdoptError, Adoption, ChildSpec, ClientDeadlines, ConnectError, DEFAULT_JOURNAL_CAP, GoneHost,
    HostConnection, HostName, HostOutcome, HostProcess, HostRole, HostTag, HostTerminal,
    HostTerminalOptions, Liveness, SpawnError, StopLimits, Termination, terminate_verified,
};
use crate::launch::LaunchArtifacts;
use crate::service::terminals::Terminal;

// ---- The child's environment -------------------------------------------------------

/// `TERM` for a host's child: what tmux sets in a pane from the private
/// server's `default-terminal` (`crate::tmux`'s server configuration), and
/// what the emulator interprets.
pub(crate) const HOST_TERM: &str = "xterm-256color";

/// The value a host's child gets for `TMUX`, standing in for the private
/// server's own while the agent's login shell runs its startup files (the
/// plan's D-12).
///
/// In a tmux pane the login shell's startup files run with tmux's `TMUX`
/// set, and the launch shim removes it (`crate::launch::PRIVATE_TMUX_ENV_VARS`)
/// only afterwards, before the agent starts. A startup file that starts or
/// attaches tmux when `TMUX` is unset (a common way to get tmux in every
/// terminal) is therefore held back while an agent starts, and so it must
/// be in a host, which has no tmux of its own: otherwise it would put the
/// agent's whole launch chain inside the user's own tmux, and the agent in a
/// terminal Farhelm does not drive. The same strip removes this placeholder,
/// so the agent and everything it starts see neither.
///
/// It names a socket path that cannot exist (`/dev/null` is not a
/// directory), so a startup file that runs a tmux command because `TMUX` is
/// set reaches no server: tmux prints that it cannot connect, on the
/// terminal before the agent draws. In a tmux pane the same command
/// reached Farhelm's private server silently.
pub(crate) const STARTUP_TMUX_PLACEHOLDER: &str = "/dev/null/farhelm-terminal-host,0,0";

/// What `PATH` is when neither the supervisor nor the launch environment
/// has one: tmux's own default for a pane (`_PATH_DEFPATH`).
const DEFAULT_PATH: &str = "/usr/bin:/bin";

/// What tmux sets `TERM_PROGRAM` to in every pane, and so a host's child
/// gets too (see [`host_child_env`]).
const PANE_TERM_PROGRAM: &str = "tmux";

/// Variables a host's child never inherits from the supervisor.
///
/// - `TMUX_PANE` names a pane, and a host's child is in none. (A supervisor
///   started from inside the user's own tmux has it; tmux overrides it for
///   its panes, so a host's child must not see the outer one either.)
/// - `TERM_PROGRAM_VERSION`, which [`host_child_env`] sets again from the
///   private server's version when that is known; a supervisor started from
///   a terminal application carries that application's, which no pane ever
///   sees.
/// - `LISTEN_PID`, `LISTEN_FDS`, `LISTEN_FDNAMES`: systemd's socket
///   activation hand-off, which tmux clears for its panes too.
///
/// `TMUX` and `TERM_PROGRAM` are not here because they are always
/// overwritten, with the startup placeholder and tmux's own name.
const NOT_INHERITED: [&str; 5] = [
    "TMUX_PANE",
    "TERM_PROGRAM_VERSION",
    "LISTEN_PID",
    "LISTEN_FDS",
    "LISTEN_FDNAMES",
];

/// The complete environment of a host's child: what tmux 3.7c gives a new
/// pane's command (`environ_for_session` and `spawn_pane` in its source)
/// when the supervisor creates a session, with the startup placeholder for
/// `TMUX` and no `TMUX_PANE` (the plan's D-12 and D-13).
///
/// In tmux's order, which decides who wins a name: the supervisor's own
/// environment (`base`; a pane gets the server's, which the supervisor
/// started with its own, and the session's `update-environment` copies of
/// the supervisor's), then `launch_env` (`-e` on `new-session`), then the
/// terminal's own variables (which therefore override `launch_env`, as
/// tmux's do): `TERM`, `TERM_PROGRAM=tmux`, `TERM_PROGRAM_VERSION` (the
/// private server's version, `tmux_version`; left unset when it could not
/// be asked) and `COLORTERM=truecolor`; then the `TMUX` placeholder,
/// `PATH` (the supervisor's, which tmux takes from the client that created
/// the session; else the launch environment's; else tmux's default),
/// `SHELL` (`shell`: tmux's `default-shell`) and `PWD` (`cwd`).
///
/// `TERM_PROGRAM` names tmux although no tmux is involved. That keeps what
/// agents read exactly what they read in a pane, which the screen readers
/// and agent fixtures were made under; tmux's passthrough envelope, which a
/// program that believes it runs in tmux may use, is off in Farhelm's tmux
/// and dropped by the emulator alike, so it reaches neither terminal.
///
/// Sorted by name, so the same inputs give the same spec.
pub(crate) fn host_child_env(
    base: impl IntoIterator<Item = (OsString, OsString)>,
    launch_env: &[(String, String)],
    tmux_version: Option<&str>,
    shell: &str,
    cwd: &str,
) -> Vec<(OsString, OsString)> {
    let mut env: BTreeMap<OsString, OsString> = base.into_iter().collect();
    let client_path = env.get(std::ffi::OsStr::new("PATH")).cloned();
    for (name, value) in launch_env {
        env.insert(name.into(), value.into());
    }
    for name in NOT_INHERITED {
        env.remove(std::ffi::OsStr::new(name));
    }
    env.insert("TERM".into(), HOST_TERM.into());
    env.insert("TERM_PROGRAM".into(), PANE_TERM_PROGRAM.into());
    if let Some(version) = tmux_version {
        env.insert("TERM_PROGRAM_VERSION".into(), version.into());
    }
    env.insert("COLORTERM".into(), "truecolor".into());
    env.insert("TMUX".into(), STARTUP_TMUX_PLACEHOLDER.into());
    match client_path {
        Some(path) => {
            env.insert("PATH".into(), path);
        }
        None => {
            env.entry("PATH".into())
                .or_insert_with(|| DEFAULT_PATH.into());
        }
    }
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
async fn read_launch_record(path: &Path) -> anyhow::Result<Option<LaunchHostRecord>> {
    match tokio::fs::read_to_string(path).await {
        Ok(text) => LaunchHostRecord::decode(&text)
            .map(Some)
            .map_err(|why| anyhow::anyhow!("the launch host record {}: {why}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(anyhow::Error::new(error)
            .context(format!("reading the launch host record {}", path.display()))),
    }
}

// ---- Starting a host ---------------------------------------------------------------

/// A create's agent started in a host: what [`Supervisor::spawn_agent_in_host`]
/// produced.
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
    fn host_program(&self) -> PathBuf {
        #[cfg(any(test, feature = "test-seams"))]
        {
            if let Some(program) = &self.seams.host_program {
                return program.clone();
            }
        }
        crate::hosts::own_host_program(&self.farhelm_exe)
    }

    /// `SHELL` for a host's child, as tmux's `default-shell` would give it:
    /// the user's login shell. That is the launch's own shell in
    /// production; a supervisor whose launch shell is a test seam still
    /// gives the user's login shell, as tmux does whatever command a pane
    /// runs, so a test can compare the two kinds' environments.
    async fn host_shell_variable(&self, launch_shell: &str) -> String {
        match &self.seams.launch_shell {
            Some(_) => crate::launch::resolve_shell().await,
            None => launch_shell.to_string(),
        }
    }

    /// The private tmux server's version, which tmux gives every pane as
    /// `TERM_PROGRAM_VERSION`; `None`, logged, when tmux cannot be asked,
    /// which costs a host's child only that variable.
    async fn pane_term_program_version(&self, id: &str) -> Option<String> {
        match self.tmux.server_version().await {
            Ok(version) => Some(version),
            Err(error) => {
                warn!(session = %id, error = %format!("{error:#}"),
                    "could not ask tmux for its version; the terminal host's child gets no \
                     TERM_PROGRAM_VERSION");
                None
            }
        }
    }

    /// Start one launch's agent in a new terminal host, tagged with its
    /// session and launch generation, and adopt the host: the host
    /// counterpart of [`Self::spawn_agent`]'s new tmux session.
    ///
    /// The host's child runs exactly the command a tmux pane would
    /// ([`Self::prepare_launch`]), in `cwd`, at `cols` x `rows` (clamped
    /// as tmux clamps them), with the environment of [`host_child_env`]
    /// and the default journal cap. `initial_snapshot` seeds its terminal;
    /// a create has none, and Restart's later move to hosts is what it is
    /// for. The host is started from this supervisor's own program
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
        let tmux_version = self.pane_term_program_version(id).await;
        let env = host_child_env(
            std::env::vars_os(),
            &self.seams.launch_env,
            tmux_version.as_deref(),
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

    /// End a host this supervisor started for a launch that is not being
    /// published, confirm it ended, and remove the socket file it left.
    ///
    /// Through its runtime's `Shutdown` first, when `host` is that runtime,
    /// within [`RUNTIME_SHUTDOWN_BOUND`]; when that does not confirm the end,
    /// the runtime is stopped ([`HostTerminal::abort`], so that it does not
    /// take the host back mid-shutdown) and the host is ended by its socket,
    /// the way `farhelm internal stop-terminal-hosts` ends one
    /// ([`crate::hosts::end_host_at`]: `Shutdown` with take-over, then the
    /// verified SIGTERM for a host the protocol cannot end), within that
    /// command's per-host limits. Every wait here is bounded.
    ///
    /// Ending the host closes its pty, which hangs up the child; the
    /// caller's process sweep is what reaches descendants that outlive the
    /// hangup. `Err` carries why the host may still run.
    ///
    /// The caller must not hold [`Self::host_scan`]: removing the socket a
    /// signal left takes it exclusively.
    pub(super) async fn end_launch_host(
        &self,
        host: Option<&HostTerminal>,
        socket: &Path,
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
                    "a launch's terminal host was not confirmed ended after Shutdown; \
                     ending it by its socket"
                ),
                Ok(Err(liveness)) => warn!(
                    socket = %socket.display(), ?liveness,
                    "a launch's terminal host can no longer be reached through its runtime; \
                     ending it by its socket"
                ),
                Err(_) => warn!(
                    socket = %socket.display(), bound = ?RUNTIME_SHUTDOWN_BOUND,
                    "a launch's terminal host runtime did not finish its Shutdown in time; \
                     ending the host by its socket"
                ),
            }
            host.abort();
        }
        match crate::hosts::end_host_at(socket, StopLimits::default()).await {
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
    async fn remove_ended_socket(&self, socket: &Path) {
        let _no_spawns = self.host_scan.write().await;
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
        let socket = match std::path::absolute(&self.state_dir) {
            Ok(state_dir) => record
                .socket
                .socket_in(&crate::hosts::hosts_dir(&state_dir)),
            Err(error) => {
                return LaunchEvidence::Unresolved(
                    anyhow::Error::new(error).context("resolving the state directory"),
                );
            }
        };
        if let Err(why) = self.fence_starter(record.starter, &socket).await {
            return LaunchEvidence::Unresolved(anyhow::anyhow!(
                "the interrupted launch's terminal host start (process {}) could not be \
                 confirmed ended: {why}",
                record.starter.pid
            ));
        }
        // The client judges a socket gone only while no live process names
        // it (`hosts::client`, "Never gone while a host process serves the
        // socket"); one that does, with nothing listening yet, is
        // "not accepting", which is what a second stage still on its way
        // to binding looks like.
        let strays_until = tokio::time::Instant::now() + STRAY_WAIT;
        let mut announced = false;
        let mut connection = loop {
            match HostConnection::connect(&socket).await {
                Ok(connection) => break connection,
                Err(ConnectError::Gone(_)) => return LaunchEvidence::Absent,
                Err(ConnectError::Unavailable {
                    reason: crate::hosts::Unavailable::NotAccepting,
                    peer,
                }) if tokio::time::Instant::now() < strays_until => {
                    tracing::debug!(socket = %socket.display(), ?peer,
                        "a process still names the interrupted launch's host socket; waiting");
                    if !announced {
                        announced = true;
                        if let Some(waiting) = self.seams.faults.launch_stray_waiting() {
                            waiting();
                        }
                    }
                    tokio::time::sleep(STARTER_POLL).await;
                }
                Err(error) => {
                    return LaunchEvidence::Unresolved(anyhow::Error::new(error).context(format!(
                        "the interrupted launch's terminal host at {} cannot be read, or a \
                             process that may yet start it still names its socket",
                        socket.display()
                    )));
                }
            }
        };
        let info = match connection.info().await {
            Ok(info) => info,
            Err(error) => {
                return LaunchEvidence::Unresolved(anyhow::Error::new(error).context(format!(
                    "the interrupted launch's terminal host at {} did not report itself",
                    socket.display()
                )));
            }
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

    /// Make sure a recorded first stage can fork nothing more: `Ok` once it
    /// is gone, by itself within [`STARTER_WAIT`] or by the verified signal
    /// after it; `Err` with why it may still run.
    async fn fence_starter(&self, starter: HostProcess, socket: &Path) -> Result<(), String> {
        let deadline = tokio::time::Instant::now() + STARTER_WAIT;
        let mut announced = false;
        loop {
            if !starter.is_running()? {
                return Ok(());
            }
            if !announced {
                announced = true;
                if let Some(waiting) = self.seams.faults.launch_starter_waiting() {
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
}

#[cfg(test)]
mod tests {
    //! The child's environment and the launch record, as pure functions of
    //! their inputs. That the environment matches what a real tmux pane's
    //! command gets, and that the record fences a start that outlived its
    //! supervisor, is shown against a real tmux and real hosts in
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

    /// The supervisor's environment reaches the child, with the terminal's
    /// own variables set as a tmux pane's are, and an outer tmux's or
    /// terminal's claims replaced or removed.
    ///
    /// Why: the child's environment is SPEC.md's "as if the user had
    /// SSHed in" contract, which a tmux pane delivered by passing the
    /// supervisor's environment through; a host passes nothing it is not
    /// given. Spec: from a supervisor environment holding `HOME`, `PATH`,
    /// a `FARHELM_` variable, an outer tmux's `TMUX`/`TMUX_PANE`, an outer
    /// terminal's `TERM`, `TERM_PROGRAM`, `TERM_PROGRAM_VERSION` and
    /// `COLORTERM`, and systemd's `LISTEN_*`, with the private server's
    /// version `3.7c`, the child gets `HOME`, `PATH` and the `FARHELM_`
    /// variable unchanged, `TERM=xterm-256color`, `TERM_PROGRAM=tmux`,
    /// `TERM_PROGRAM_VERSION=3.7c`, `COLORTERM=truecolor`, the startup
    /// placeholder for `TMUX`, `SHELL` and `PWD` from the arguments, and no
    /// `TMUX_PANE` or `LISTEN_*`; the result is sorted by name.
    #[test]
    fn the_child_gets_the_supervisors_environment_shaped_like_a_panes() {
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
            Some("3.7c"),
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
                ("TERM_PROGRAM", "tmux"),
                ("TERM_PROGRAM_VERSION", "3.7c"),
                ("TMUX", STARTUP_TMUX_PLACEHOLDER),
            ])
        );
    }

    /// Without the private server's version, the child gets no
    /// `TERM_PROGRAM_VERSION` at all, never an outer terminal's.
    ///
    /// Why: a version that names another program than `TERM_PROGRAM` is a
    /// claim no pane ever made. Spec: a supervisor environment with
    /// `TERM_PROGRAM_VERSION=3.5` and no tmux version gives a child with
    /// `TERM_PROGRAM=tmux` and no `TERM_PROGRAM_VERSION`.
    #[test]
    fn without_tmuxs_version_the_child_gets_none() {
        let child = host_child_env(
            env(&[("TERM_PROGRAM_VERSION", "3.5")]),
            &[],
            None,
            "/bin/sh",
            "/",
        );
        assert_eq!(get(&child, "TERM_PROGRAM"), Some("tmux"));
        assert_eq!(get(&child, "TERM_PROGRAM_VERSION"), None);
    }

    /// The launch environment applies as tmux's `-e` does: over the
    /// supervisor's own, under the terminal's own variables.
    ///
    /// Why: the launch environment seam is how tests give a launch its own
    /// `HOME`; a host that ordered it differently from tmux would make the
    /// same test see different environments per kind. Spec: a launch
    /// environment setting `HOME`, `TERM`, `TERM_PROGRAM`, `PATH`, `SHELL`
    /// and `TMUX` gives the child its `HOME`, but `TERM` and `TERM_PROGRAM`
    /// from the terminal, `PATH` from the supervisor, `SHELL` from the
    /// argument and the `TMUX` placeholder.
    #[test]
    fn the_launch_environment_applies_where_tmux_applies_it() {
        let child = host_child_env(
            env(&[("HOME", "/home/someone"), ("PATH", "/usr/bin")]),
            &[
                ("HOME".into(), "/fixture/home".into()),
                ("TERM".into(), "dumb".into()),
                ("TERM_PROGRAM".into(), "fixture".into()),
                ("PATH".into(), "/fixture/bin".into()),
                ("SHELL".into(), "/fixture/sh".into()),
                ("TMUX".into(), "/fixture/tmux,1,0".into()),
            ],
            Some("3.7c"),
            "/bin/bash",
            "/",
        );
        assert_eq!(get(&child, "HOME"), Some("/fixture/home"));
        assert_eq!(get(&child, "TERM"), Some(HOST_TERM));
        assert_eq!(get(&child, "TERM_PROGRAM"), Some("tmux"));
        assert_eq!(get(&child, "PATH"), Some("/usr/bin"));
        assert_eq!(get(&child, "SHELL"), Some("/bin/bash"));
        assert_eq!(get(&child, "TMUX"), Some(STARTUP_TMUX_PLACEHOLDER));
    }

    /// A supervisor without `PATH` gives the child tmux's default one,
    /// unless the launch environment gives one.
    ///
    /// Why: a login shell started with no `PATH` cannot find the programs
    /// its startup files run; tmux falls back the same way. Spec: no
    /// `PATH` anywhere gives `/usr/bin:/bin`; a launch environment `PATH`
    /// with none in the supervisor's is kept.
    #[test]
    fn without_a_path_the_child_gets_tmuxs_default() {
        let child = host_child_env(env(&[]), &[], None, "/bin/sh", "/");
        assert_eq!(get(&child, "PATH"), Some(DEFAULT_PATH));
        let child = host_child_env(
            env(&[]),
            &[("PATH".into(), "/fixture/bin".into())],
            None,
            "/bin/sh",
            "/",
        );
        assert_eq!(get(&child, "PATH"), Some("/fixture/bin"));
    }

    /// The startup placeholder names a path no socket can ever be bound at,
    /// and is in the shape tmux reads `TMUX` in.
    ///
    /// Why: a startup file that runs a tmux command because `TMUX` is set
    /// must reach no server; a placeholder at a path someone could bind
    /// would hand its commands to whoever did. Spec: the placeholder's
    /// socket part (before the first comma) lies under `/dev/null`, which
    /// is not a directory, so nothing exists under it; and it carries the
    /// two comma-separated fields tmux's own value has after the socket.
    #[test]
    fn the_startup_placeholder_names_no_possible_socket() {
        let (socket, rest) = STARTUP_TMUX_PLACEHOLDER
            .split_once(',')
            .expect("socket, pid and session index");
        assert_eq!(rest.split(',').count(), 2);
        let socket = Path::new(socket);
        assert_eq!(socket.parent(), Some(Path::new("/dev/null")));
        assert!(
            !Path::new("/dev/null").is_dir(),
            "premise: /dev/null is not a directory"
        );
        assert!(std::fs::symlink_metadata(socket).is_err());
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
