//! The system-facing half of provisioning executes the same action vocabulary
//! for a local host or through the user's SSH access.

use super::plan::{DirectorySpec, PayloadArch, PayloadKind, ProvisioningTarget};
use crate::manager::peer_text;
use async_trait::async_trait;
use farhelm_proto::ControlMsg;
use farhelm_proto::io::{ClosedBeforeHello, FrameReader, FrameWriter, VersionSkew, handshake};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const PROBE_TIMEOUT: Duration = Duration::from_secs(15);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(60);
pub(super) const MAX_CHILD_STREAM_BYTES: usize = 64 * 1024;
const POSITIVE_ABSENCE_EXIT: i32 = 75;
pub(super) const REMOTE_PROBE_MARKER: &str = "farhelm-probe-command-started-v1";
pub(super) const REMOTE_RESOLVED_PREFIX: &str = "farhelm-probe-resolved-v1:";
pub(super) const REACH_RECORD_MARKER: &str = "farhelm-reach-v1";
pub(super) const PAYLOAD_COPY_BUFFER: usize = 64 * 1024;

/// Probe-time transport plus the binary and state directory the stdio proxy
/// must use before any install plan exists.
#[derive(Debug, Clone)]
pub(super) struct ProbeTarget {
    pub(super) transport: ProvisioningTarget,
    pub(super) probe_farhelm: PathBuf,
    pub(super) probe_state_dir: Option<PathBuf>,
}

/// What completing the protocol hello proves about discovery.
#[derive(Debug)]
pub(super) enum ProbeObservation {
    Supervisor {
        build_version: String,
        host_identity: Option<String>,
        dial_farhelm: PathBuf,
        dial_state_dir: Option<PathBuf>,
    },
    /// A supervisor answered the hello but speaks a DIFFERENT protocol
    /// version, so the exchange stopped at the version refusal. Presence is
    /// PROVEN — only a live supervisor sends a hello at all — and the skew
    /// payload carries its build, but nothing else a completed hello would
    /// have: in particular no host identity, because the refusal happens
    /// before any further exchange. Consumers that need identity must
    /// decide explicitly what an unverifiable-but-present peer means for
    /// them.
    ///
    /// This variant is what makes UPDATE work on the hosts it exists for:
    /// a host left behind by a protocol bump is exactly the one the panel's
    /// update action must reach, and before this variant existed the skew
    /// refusal was misclassified as a transport failure ("the supervisor
    /// probe closed before hello completion"), making a skewed host
    /// un-updatable — found 2026-09-01 on the first real cross-protocol
    /// update attempt (protocol 12 host, protocol 14 helm).
    SkewedSupervisor {
        /// The peer's own build version, from the skew payload — what the
        /// hosts panel shows and what update logs name.
        peer_build: String,
        dial_farhelm: PathBuf,
        dial_state_dir: Option<PathBuf>,
    },
    Absent,
}

/// Result of applying provisioning's positive-absence probe to the local row.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum LocalSupervisorDiscovery {
    /// A supervisor completed the protocol hello and should be reused as-is.
    Answering,
    /// Every probe failure was one the convergence classifier proves absent.
    Absent,
}

/// Discover the reserved local supervisor without installing or registering it.
///
/// Desktop startup uses the same hello and positive-absence classifier as the
/// shipped provisioning workflow. An answering supervisor is therefore an
/// ownership boundary: callers must reuse it rather than start a rival process.
pub async fn discover_local_supervisor(
    farhelm: &Path,
    state_dir: &Path,
) -> anyhow::Result<LocalSupervisorDiscovery> {
    let backend = SystemBackend::new(state_dir.to_path_buf());
    let target = ProbeTarget {
        transport: ProvisioningTarget::Local,
        probe_farhelm: farhelm.to_path_buf(),
        probe_state_dir: Some(state_dir.to_path_buf()),
    };
    match backend.probe(&target).await.map_err(anyhow::Error::new)? {
        ProbeObservation::Supervisor { .. } => Ok(LocalSupervisorDiscovery::Answering),
        // A skewed supervisor is still a supervisor OWNING the socket and
        // the state directory: starting a rival because we cannot talk to
        // it would be strictly worse than reusing it and letting the
        // connection manager surface the skew as the per-host state it
        // already has for exactly this situation.
        ProbeObservation::SkewedSupervisor { .. } => Ok(LocalSupervisorDiscovery::Answering),
        ProbeObservation::Absent => Ok(LocalSupervisorDiscovery::Absent),
    }
}

/// Host facts needed to select payloads and render absolute install paths,
/// including the unit directory the running user manager actually searches.
#[derive(Debug, Clone)]
pub(super) struct Reach {
    pub(super) home: PathBuf,
    pub(super) user_unit_dir: PathBuf,
    pub(super) arch: PayloadArch,
    /// The `/etc/os-release` `ID` field, verbatim, empty when the host has
    /// no such file.
    ///
    /// Informational only: nothing in this struct's construction branches
    /// on it, because it predicts none of the capabilities provisioning
    /// actually needs (a payload architecture, a usable systemd user
    /// manager, a resolvable unit directory, an acceptable tmux). It is
    /// carried through purely so the plan the user confirms can name the
    /// host it inspected — see `ProvisioningPlan::confirmation`.
    pub(super) distro_id: String,
    pub(super) needs_tmux: bool,
    /// The host's OWN tmux executable, absolute, when it cleared the
    /// floor — `None` whenever `needs_tmux` is set.
    ///
    /// The whole executable, not just its directory, because the plan has
    /// to be able to name it: an accepted host tmux is pinned into the
    /// unit as `FARHELM_TMUX`, so a leftover private tmux in Farhelm's own
    /// lib directory cannot shadow the binary provisioning approved. The
    /// directory this used to carry is still derived from it for the
    /// unit's PATH, which serves the different purpose of letting a
    /// user-manager process find tmux at all.
    pub(super) host_tmux: Option<PathBuf>,
}

/// Supported hosts continue to a plan; every other host keeps the manual
/// supervisor path without turning platform mismatch into a setup failure.
#[derive(Debug, Clone)]
pub(super) enum ReachOutcome {
    Supported(Reach),
    Manual(String),
}

/// Idempotent actions distinguish useful no-ops and optional degradation
/// from ordinary completion in the progress record.
#[derive(Debug)]
pub(super) enum ActionOutcome {
    Completed,
    Skipped(String),
    Degraded(String),
}

/// A backend failure preserves the host's stderr separately so the REST
/// progress record can escape it before retention.
#[derive(Debug)]
pub(super) struct BackendFailure {
    pub(super) context: String,
    stderr: String,
}

impl std::fmt::Display for BackendFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.rendered())
    }
}

impl std::error::Error for BackendFailure {}

impl BackendFailure {
    pub(super) fn new(context: impl Into<String>, stderr: impl Into<String>) -> Self {
        Self {
            context: context.into(),
            stderr: stderr.into(),
        }
    }

    pub(super) fn rendered(&self) -> String {
        if self.stderr.is_empty() {
            self.context.clone()
        } else {
            format!("{}: host stderr {}", self.context, peer_text(&self.stderr))
        }
    }
}

/// The system-facing half of provisioning. Tests replace it with a recorder
/// for failure taxonomy and linger behavior; real transport tests use the
/// production implementation with only linger faked.
#[async_trait]
pub(super) trait ProvisioningBackend: Send + Sync {
    async fn probe(&self, target: &ProbeTarget) -> Result<ProbeObservation, BackendFailure>;
    async fn inspect(&self, target: &ProbeTarget) -> Result<ReachOutcome, BackendFailure>;
    async fn ensure_directories(
        &self,
        target: &ProvisioningTarget,
        directories: &[DirectorySpec],
    ) -> Result<ActionOutcome, BackendFailure>;
    async fn install_path(
        &self,
        target: &ProvisioningTarget,
        kind: PayloadKind,
        payload: &PreparedPayload,
        destination: &Path,
        temporary: &Path,
        mode: u32,
    ) -> Result<ActionOutcome, BackendFailure>;
    async fn install_bytes(
        &self,
        target: &ProvisioningTarget,
        content: &[u8],
        destination: &Path,
        temporary: &Path,
        mode: u32,
    ) -> Result<ActionOutcome, BackendFailure>;
    async fn daemon_reload(
        &self,
        target: &ProvisioningTarget,
    ) -> Result<ActionOutcome, BackendFailure>;
    async fn enable_now(
        &self,
        target: &ProvisioningTarget,
        unit: &str,
        unit_path: &Path,
    ) -> Result<ActionOutcome, BackendFailure>;
    async fn enable_linger(
        &self,
        target: &ProvisioningTarget,
    ) -> Result<ActionOutcome, BackendFailure>;
    async fn restart(
        &self,
        target: &ProvisioningTarget,
        unit: &str,
    ) -> Result<ActionOutcome, BackendFailure>;

    /// Read one unit file out of THIS machine's systemd user directory,
    /// or `None` when no such file exists.
    ///
    /// Takes no target on purpose: the only question it answers is about
    /// the helm's own machine, where `farhelm helm setup` — not the hosts
    /// panel — owns the units (D9). The local row consults it before
    /// probing so a supervisor unit setup wrote can never be overwritten
    /// from the panel.
    ///
    /// `None` means one thing only: no such file. Every other outcome —
    /// an unreadable file, contents that are not UTF-8, a unit directory
    /// that cannot be located — is an error, because the caller reads
    /// `None` as "there is nothing here to protect" and would go on to
    /// install over whatever it could not read.
    async fn read_user_unit(&self, name: &str) -> Result<Option<String>, BackendFailure>;

    /// Let a deliberately injected backend complete attachment without a
    /// real manager transport. Production returns `None`, preserving the
    /// manager-owned attach path below.
    async fn injected_attach(
        &self,
        _target: &ProvisioningTarget,
    ) -> Result<Option<ActionOutcome>, BackendFailure> {
        Ok(None)
    }
}

/// Optional linger is the one host action real transport tests must not run
/// against the developer's account.
#[derive(Clone)]
pub(super) enum LingerBehavior {
    Real,
    #[cfg(test)]
    Simulated(Result<(), String>),
}

/// Production process and file operations for both supported transports.
pub(super) struct SystemBackend {
    pub(super) control_dir: PathBuf,
    pub(super) linger: LingerBehavior,
    pub(super) launcher: Arc<dyn CommandLauncher>,
    pub(super) runtime_units: bool,
    #[cfg(test)]
    pub(super) fail_before_rename: bool,
}

/// Process-creation seam that tests use to exercise production classifiers
/// without invoking a real SSH endpoint.
pub(super) trait CommandLauncher: Send + Sync {
    fn spawn(
        &self,
        command: &mut tokio::process::Command,
    ) -> std::io::Result<tokio::process::Child>;
}

/// Ordinary launcher: all lifecycle and stream policy stays with the caller.
pub(super) struct SystemLauncher;

impl CommandLauncher for SystemLauncher {
    fn spawn(
        &self,
        command: &mut tokio::process::Command,
    ) -> std::io::Result<tokio::process::Child> {
        command.spawn()
    }
}

/// Put a command and any helpers it spawns in a disposable process group.
/// A shell timeout must stop the actual mutator, not only its waiting shell.
pub(super) fn isolate_process_group(command: &mut tokio::process::Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.as_std_mut().process_group(0);
    }
}

impl SystemBackend {
    /// Production uses the real optional linger action.
    pub(super) fn new(control_dir: PathBuf) -> Self {
        Self {
            control_dir,
            linger: LingerBehavior::Real,
            launcher: Arc::new(SystemLauncher),
            runtime_units: false,
            #[cfg(test)]
            fail_before_rename: false,
        }
    }

    /// Real transport tests replace only linger; every install and systemd
    /// action around it still reaches the host. Runtime-only unit links keep
    /// the fixture's unit file out of the user's persistent configuration.
    #[cfg(test)]
    pub(super) fn with_simulated_linger(
        control_dir: PathBuf,
        result: Result<(), String>,
        runtime_units: bool,
    ) -> Self {
        Self {
            control_dir,
            linger: LingerBehavior::Simulated(result),
            launcher: Arc::new(SystemLauncher),
            runtime_units,
            fail_before_rename: false,
        }
    }

    /// Build a remote shell command on the same option-safe prefix used by
    /// steady-state supervisor connections.
    fn ssh_command(
        &self,
        destination: &str,
        remote_command: String,
    ) -> anyhow::Result<tokio::process::Command> {
        let mut command = tokio::process::Command::new("ssh");
        command.args(crate::ssh::ssh_base_args(
            destination,
            &self.control_dir.join("ssh-cm-%C"),
        )?);
        // ssh concatenates its trailing argv and reparses it remotely. Keep
        // the complete `sh -c` invocation in one shell-quoted string so the
        // script cannot absorb words from a destination or path.
        command.arg(format!("sh -c {}", shell_words::quote(&remote_command)));
        Ok(command)
    }

    /// Run one bounded shell script locally or through SSH, retaining output
    /// for structured parsing and escaped diagnostics.
    async fn run_shell(
        &self,
        target: &ProvisioningTarget,
        script: &str,
        timeout: Duration,
    ) -> Result<CommandResult, BackendFailure> {
        let mut command = match target {
            ProvisioningTarget::Local => {
                let mut command = tokio::process::Command::new("sh");
                command.args(["-c", script]);
                command
            }
            ProvisioningTarget::Ssh { destination } => self
                .ssh_command(destination, script.to_string())
                .map_err(|error| {
                    BackendFailure::new("building the ssh command", error.to_string())
                })?,
        };
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        isolate_process_group(&mut command);
        let child = self
            .launcher
            .spawn(&mut command)
            .map_err(|error| BackendFailure::new("spawning the host command", error.to_string()))?;
        capture_child(child, timeout, "the host command").await
    }

    /// Require a zero exit status while preserving the host's stderr.
    pub(super) async fn require_shell(
        &self,
        target: &ProvisioningTarget,
        script: &str,
        context: &str,
    ) -> Result<CommandResult, BackendFailure> {
        let output = self.run_shell(target, script, COMMAND_TIMEOUT).await?;
        if output.code == Some(0) {
            Ok(output)
        } else {
            let status = output.failure_status();
            Err(BackendFailure::new(
                context,
                if output.stderr.is_empty() {
                    status
                } else {
                    format!("{status}; {}", output.stderr)
                },
            ))
        }
    }

    /// Read an installed artifact's SHA-256 and mode. Only a verified missing
    /// path is `None`; command, permission, and I/O failures remain errors.
    pub(super) async fn metadata_on_target(
        &self,
        target: &ProvisioningTarget,
        path: &Path,
    ) -> Result<Option<TargetMetadata>, BackendFailure> {
        match target {
            ProvisioningTarget::Local => match tokio::fs::File::open(path).await {
                Ok(mut file) => {
                    use std::os::unix::fs::PermissionsExt;
                    let metadata = file.metadata().await.map_err(|error| {
                        BackendFailure::new(
                            format!("reading metadata for {}", path.display()),
                            error.to_string(),
                        )
                    })?;
                    let mut digest = Sha256::new();
                    let mut buffer = vec![0_u8; PAYLOAD_COPY_BUFFER];
                    loop {
                        let read = file.read(&mut buffer).await.map_err(|error| {
                            BackendFailure::new(
                                format!("reading {} for its hash", path.display()),
                                error.to_string(),
                            )
                        })?;
                        if read == 0 {
                            break;
                        }
                        digest.update(&buffer[..read]);
                    }
                    Ok(Some(TargetMetadata {
                        hash: format!("{:x}", digest.finalize()),
                        mode: metadata.permissions().mode() & 0o7777,
                    }))
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(BackendFailure::new(
                    format!("reading {} for its hash", path.display()),
                    error.to_string(),
                )),
            },
            ProvisioningTarget::Ssh { .. } => {
                let path = shell_path(path)?;
                let output = self
                    .run_shell(
                        target,
                        &format!(
                            "if [ ! -e {path} ]; then exit 44; fi; \
                             sha256sum -- {path} && stat -c '%a' -- {path}"
                        ),
                        COMMAND_TIMEOUT,
                    )
                    .await?;
                if output.code == Some(44) {
                    return Ok(None);
                }
                if output.code != Some(0) {
                    return Err(BackendFailure::new(
                        "inspecting the remote artifact",
                        output.stderr,
                    ));
                }
                let text = String::from_utf8(output.stdout).map_err(|error| {
                    BackendFailure::new("reading remote artifact metadata", error.to_string())
                })?;
                let mut fields = text.split_whitespace();
                let hash = fields.next().unwrap_or_default();
                if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                    return Err(BackendFailure::new(
                        "reading the remote hash",
                        format!("sha256sum returned malformed output {text:?}"),
                    ));
                }
                let mode = fields
                    .last()
                    .and_then(|mode| u32::from_str_radix(mode, 8).ok())
                    .ok_or_else(|| {
                        BackendFailure::new(
                            "reading the remote mode",
                            format!("stat returned malformed output {text:?}"),
                        )
                    })?;
                Ok(Some(TargetMetadata {
                    hash: hash.to_ascii_lowercase(),
                    mode,
                }))
            }
        }
    }

    /// Repair an installed artifact's mode without retransferring bytes.
    async fn set_target_mode(
        &self,
        target: &ProvisioningTarget,
        path: &Path,
        mode: u32,
    ) -> Result<(), BackendFailure> {
        match target {
            ProvisioningTarget::Local => set_mode(path, mode).await,
            ProvisioningTarget::Ssh { .. } => {
                self.require_shell(
                    target,
                    &format!("chmod {mode:o} -- {}", shell_path(path)?),
                    "repairing installed permissions",
                )
                .await?;
                Ok(())
            }
        }
    }

    /// Remove exactly this run's temporary path on either transport.
    async fn remove_temporary(
        &self,
        target: &ProvisioningTarget,
        path: &Path,
    ) -> Result<(), BackendFailure> {
        match target {
            ProvisioningTarget::Local => match tokio::fs::remove_file(path).await {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(BackendFailure::new(
                    format!("removing temporary file {}", path.display()),
                    error.to_string(),
                )),
            },
            ProvisioningTarget::Ssh { .. } => {
                self.require_shell(
                    target,
                    &format!("rm -f -- {}", shell_path(path)?),
                    "removing the remote temporary file",
                )
                .await?;
                Ok(())
            }
        }
    }

    /// Converge bytes and mode through a same-directory atomic replacement.
    /// Any failed copy, transfer, chmod, or rename removes the temporary while
    /// preserving the previously installed path.
    async fn install_source(
        &self,
        target: &ProvisioningTarget,
        source: &Path,
        source_hash: &str,
        install: InstallDestination<'_>,
    ) -> Result<ActionOutcome, BackendFailure> {
        let InstallDestination {
            path: destination,
            temporary,
            mode,
            description,
        } = install;
        if let Some(installed) = self.metadata_on_target(target, destination).await?
            && installed.hash == source_hash
        {
            let repaired = installed.mode != mode;
            if repaired {
                self.set_target_mode(target, destination, mode).await?;
            }
            return Ok(ActionOutcome::Skipped(format!(
                "{} already has the requested {description}{}",
                destination.display(),
                if repaired {
                    format!("; repaired mode to {mode:04o}")
                } else {
                    String::new()
                }
            )));
        }

        self.remove_temporary(target, temporary).await?;
        let install: Result<(), BackendFailure> = match target {
            ProvisioningTarget::Local => {
                async {
                    let mut input = tokio::fs::File::open(source).await.map_err(|error| {
                        BackendFailure::new(
                            format!("opening staged {description} {}", source.display()),
                            error.to_string(),
                        )
                    })?;
                    let mut options = tokio::fs::OpenOptions::new();
                    options.write(true).create_new(true);
                    #[cfg(unix)]
                    {
                        options.custom_flags(libc::O_NOFOLLOW).mode(mode);
                    }
                    let mut output = options.open(temporary).await.map_err(|error| {
                        BackendFailure::new(
                            format!("creating temporary {description} {}", temporary.display()),
                            error.to_string(),
                        )
                    })?;
                    tokio::io::copy(&mut input, &mut output)
                        .await
                        .map_err(|error| {
                            BackendFailure::new(
                                format!("copying {description} to {}", temporary.display()),
                                error.to_string(),
                            )
                        })?;
                    output.flush().await.map_err(|error| {
                        BackendFailure::new(
                            format!("flushing temporary {description} {}", temporary.display()),
                            error.to_string(),
                        )
                    })?;
                    drop(output);
                    set_mode(temporary, mode).await?;
                    #[cfg(test)]
                    if self.fail_before_rename {
                        return Err(BackendFailure::new(
                            "planted failure before atomic rename",
                            "",
                        ));
                    }
                    tokio::fs::rename(temporary, destination)
                        .await
                        .map_err(|error| {
                            BackendFailure::new(
                                format!("atomically installing {}", destination.display()),
                                error.to_string(),
                            )
                        })?;
                    Ok(())
                }
                .await
            }
            ProvisioningTarget::Ssh {
                destination: ssh_destination,
            } => {
                async {
                    self.sftp_put(ssh_destination, source, temporary).await?;
                    self.require_shell(
                        target,
                        &format!(
                            "actual=$(sha256sum -- {}) || exit; \
                             [ \"${{actual%% *}}\" = {} ] || {{ \
                               printf '%s\\n' 'uploaded payload digest mismatch' >&2; exit 76; \
                             }}; chmod {mode:o} -- {} && mv -f -- {} {}",
                            shell_path(temporary)?,
                            shell_words::quote(source_hash),
                            shell_path(temporary)?,
                            shell_path(temporary)?,
                            shell_path(destination)?
                        ),
                        &format!("finishing the atomic {description} install"),
                    )
                    .await?;
                    Ok(())
                }
                .await
            }
        };
        if let Err(primary) = install {
            if let Err(cleanup) = self.remove_temporary(target, temporary).await {
                return Err(BackendFailure::new(
                    format!("{}; temporary cleanup also failed", primary.context),
                    format!("{}; {}", primary.stderr, cleanup.rendered()),
                ));
            }
            return Err(primary);
        }
        Ok(ActionOutcome::Completed)
    }

    /// Transfer one local file to one absolute remote temporary path.
    async fn sftp_put(
        &self,
        destination: &str,
        source: &Path,
        remote: &Path,
    ) -> Result<(), BackendFailure> {
        let mut command = tokio::process::Command::new("sftp");
        command.args(["-b", "-"]);
        command.args(
            crate::ssh::ssh_base_args(destination, &self.control_dir.join("ssh-cm-%C")).map_err(
                |error| BackendFailure::new("building the sftp command", error.to_string()),
            )?,
        );
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        isolate_process_group(&mut command);
        let mut child = self
            .launcher
            .spawn(&mut command)
            .map_err(|error| BackendFailure::new("spawning sftp", error.to_string()))?;
        let batch = format!("put {} {}\n", sftp_path(source)?, sftp_path(remote)?);
        let mut stdin = child.stdin.take().expect("piped sftp stdin");
        use tokio::io::AsyncWriteExt;
        if let Err(error) = stdin.write_all(batch.as_bytes()).await {
            terminate_child(&mut child).await;
            return Err(BackendFailure::new(
                "writing the sftp batch",
                error.to_string(),
            ));
        }
        drop(stdin);
        let output = capture_child(child, TRANSFER_TIMEOUT, "the sftp transfer").await?;
        if output.code != Some(0) {
            return Err(BackendFailure::new(
                format!("transferring {} with sftp", source.display()),
                output.stderr,
            ));
        }
        Ok(())
    }

    /// Start the stdio proxy without interpreting its bytes. The caller owns
    /// hello completion and the positive-absence exit taxonomy.
    async fn spawn_probe(
        &self,
        target: &ProbeTarget,
    ) -> Result<(tokio::process::Child, bool), BackendFailure> {
        let mut command = match &target.transport {
            ProvisioningTarget::Local => {
                let mut command = tokio::process::Command::new(&target.probe_farhelm);
                command.args(["internal", "stdio"]);
                if let Some(state) = &target.probe_state_dir {
                    command.arg("--state-dir").arg(state);
                }
                command
            }
            ProvisioningTarget::Ssh { destination } => {
                let farhelm = shell_path(&target.probe_farhelm)?;
                let state = target
                    .probe_state_dir
                    .as_ref()
                    .map(|path| shell_path(path))
                    .transpose()?;
                let state_arg = state
                    .map(|path| format!(" --state-dir {path}"))
                    .unwrap_or_default();
                let script = format!(
                    "printf '%s\\n' {marker} >&2; resolved=''; \
                     if command -v {farhelm} >/dev/null 2>&1; then resolved=$(command -v {farhelm}); \
                     elif [ {farhelm} = farhelm ] && [ -x \"$HOME/.local/lib/farhelm/farhelm\" ]; \
                     then resolved=\"$HOME/.local/lib/farhelm/farhelm\"; fi; \
                     if [ -n \"$resolved\" ]; then printf '%s%s\\n' {resolved_prefix} \"$resolved\" >&2; \
                     exec \"$resolved\" internal stdio{state_arg}; fi; exit {POSITIVE_ABSENCE_EXIT}",
                    marker = shell_words::quote(REMOTE_PROBE_MARKER),
                    resolved_prefix = shell_words::quote(REMOTE_RESOLVED_PREFIX),
                );
                self.ssh_command(destination, script).map_err(|error| {
                    BackendFailure::new("building the ssh probe", error.to_string())
                })?
            }
        };
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        isolate_process_group(&mut command);
        let child = self.launcher.spawn(&mut command).map_err(|error| {
            BackendFailure::new("spawning the supervisor probe", error.to_string())
        })?;
        Ok((
            child,
            matches!(target.transport, ProvisioningTarget::Ssh { .. }),
        ))
    }
}

/// Bounded captured result of one local or remote host shell.
#[derive(Debug)]
pub(super) struct CommandResult {
    code: Option<i32>,
    signal: Option<i32>,
    stdout: Vec<u8>,
    stderr: String,
}

impl CommandResult {
    /// Describe every unsuccessful termination without relying on the host
    /// command to have written a diagnostic of its own.
    fn failure_status(&self) -> String {
        match (self.code, self.signal) {
            (Some(code), _) => format!("exit status {code}"),
            (None, Some(signal)) => format!("terminated by signal {signal}"),
            (None, None) => "terminated without an exit status or signal".to_string(),
        }
    }
}

/// Content and permission facts required for an idempotent skip decision.
#[derive(Debug)]
pub(super) struct TargetMetadata {
    pub(super) hash: String,
    mode: u32,
}

/// One payload snapshot whose digest and install bytes come from the same
/// read. The temporary file stays alive until every plan action is done.
pub(super) struct PreparedPayload {
    file: tempfile::NamedTempFile,
    hash: String,
}

impl PreparedPayload {
    /// Expose only the private snapshot, never the caller-controlled source
    /// path that may change after preflight.
    pub(super) fn path(&self) -> &Path {
        self.file.path()
    }
}

/// Copy a payload through a fixed-size buffer while computing the digest of
/// those exact bytes. Later installation reads only this private snapshot,
/// so replacing the source path cannot change what the plan installs.
pub(super) async fn stage_payload(source: &Path) -> Result<PreparedPayload, BackendFailure> {
    let mut input = tokio::fs::File::open(source).await.map_err(|error| {
        BackendFailure::new(
            format!("opening provisioning payload {}", source.display()),
            error.to_string(),
        )
    })?;
    let file = tempfile::NamedTempFile::new().map_err(|error| {
        BackendFailure::new("creating the validated payload snapshot", error.to_string())
    })?;
    let mut output = tokio::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(file.path())
        .await
        .map_err(|error| {
            BackendFailure::new("opening the validated payload snapshot", error.to_string())
        })?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; PAYLOAD_COPY_BUFFER];
    loop {
        let read = input.read(&mut buffer).await.map_err(|error| {
            BackendFailure::new(
                format!("reading provisioning payload {}", source.display()),
                error.to_string(),
            )
        })?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
        output.write_all(&buffer[..read]).await.map_err(|error| {
            BackendFailure::new("staging the validated payload bytes", error.to_string())
        })?;
    }
    output.flush().await.map_err(|error| {
        BackendFailure::new("flushing the validated payload snapshot", error.to_string())
    })?;
    Ok(PreparedPayload {
        file,
        hash: format!("{:x}", digest.finalize()),
    })
}

/// Final and temporary coordinates plus convergence policy for one artifact.
struct InstallDestination<'a> {
    path: &'a Path,
    temporary: &'a Path,
    mode: u32,
    description: &'a str,
}

/// Why a child stream stopped before EOF. The retained prefix is bounded and
/// safe to include in an escaped peer diagnostic.
pub(super) struct DrainFailure {
    stream: &'static str,
    detail: String,
    prefix: Vec<u8>,
}

/// Probe stderr keeps a bounded diagnostic prefix while scanning every byte
/// for the wrapper records that decide remote absence and resolved dialing.
#[derive(Default)]
pub(super) struct ProbeStderr {
    pub(super) prefix: Vec<u8>,
    pub(super) command_started: bool,
    resolved_farhelm: Option<Vec<u8>>,
}

/// Count handshake bytes without changing framing. Positive absence emits no
/// protocol stdout at all; any byte seen before an I/O failure makes the
/// stream malformed or truncated rather than absent.
struct CountingReader<R> {
    inner: R,
    bytes: Arc<std::sync::atomic::AtomicUsize>,
}

impl<R> tokio::io::AsyncRead for CountingReader<R>
where
    R: tokio::io::AsyncRead + Unpin,
{
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
        buffer: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let before = buffer.filled().len();
        let result = std::pin::Pin::new(&mut self.inner).poll_read(context, buffer);
        if matches!(result, std::task::Poll::Ready(Ok(()))) {
            self.bytes.fetch_add(
                buffer.filled().len().saturating_sub(before),
                std::sync::atomic::Ordering::Relaxed,
            );
        }
        result
    }
}

impl ProbeStderr {
    /// Recognize only complete wrapper records while the byte stream is
    /// drained; diagnostic truncation must not hide a later control record.
    fn observe_line(&mut self, line: &[u8]) {
        if line == REMOTE_PROBE_MARKER.as_bytes() {
            self.command_started = true;
        }
        if let Some(path) = line.strip_prefix(REMOTE_RESOLVED_PREFIX.as_bytes())
            && !path.is_empty()
        {
            self.resolved_farhelm = Some(path.to_vec());
        }
    }

    /// Render the bounded prefix retained for a safe peer diagnostic.
    fn diagnostic(&self) -> String {
        String::from_utf8_lossy(&self.prefix).into_owned()
    }
}

/// Drain probe stderr to EOF even after its diagnostic budget is full.
/// Marker recognition is line-framed and therefore independent of where a
/// long SSH banner falls relative to the retained prefix.
pub(super) async fn drain_probe_stderr<R>(
    mut stream: R,
    signal: tokio::sync::mpsc::UnboundedSender<DrainFailure>,
) -> ProbeStderr
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut result = ProbeStderr::default();
    let mut line = Vec::new();
    let mut line_overflow = false;
    let mut buffer = [0_u8; 4096];
    loop {
        match stream.read(&mut buffer).await {
            Ok(0) => {
                if !line_overflow {
                    result.observe_line(&line);
                }
                return result;
            }
            Ok(read) => {
                let remaining = MAX_CHILD_STREAM_BYTES.saturating_sub(result.prefix.len());
                result
                    .prefix
                    .extend_from_slice(&buffer[..read.min(remaining)]);
                for byte in &buffer[..read] {
                    if *byte == b'\n' {
                        if !line_overflow {
                            result.observe_line(&line);
                        }
                        line.clear();
                        line_overflow = false;
                    } else if line.len() < MAX_CHILD_STREAM_BYTES {
                        line.push(*byte);
                    } else {
                        line_overflow = true;
                    }
                }
            }
            Err(error) => {
                let _ = signal.send(DrainFailure {
                    stream: "stderr",
                    detail: error.to_string(),
                    prefix: result.prefix.clone(),
                });
                return result;
            }
        }
    }
}

/// Drain a child pipe concurrently, signalling the owner when peer output
/// exceeds the memory budget or the pipe itself fails.
async fn drain_capped<R>(
    mut stream: R,
    name: &'static str,
    signal: tokio::sync::mpsc::UnboundedSender<DrainFailure>,
) -> Vec<u8>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut kept = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        match stream.read(&mut buffer).await {
            Ok(0) => return kept,
            Ok(read) if kept.len() + read <= MAX_CHILD_STREAM_BYTES => {
                kept.extend_from_slice(&buffer[..read]);
            }
            Ok(read) => {
                let remaining = MAX_CHILD_STREAM_BYTES.saturating_sub(kept.len());
                kept.extend_from_slice(&buffer[..read.min(remaining)]);
                let _ = signal.send(DrainFailure {
                    stream: name,
                    detail: format!("exceeded the {MAX_CHILD_STREAM_BYTES}-byte limit"),
                    prefix: kept.clone(),
                });
                return kept;
            }
            Err(error) => {
                let _ = signal.send(DrainFailure {
                    stream: name,
                    detail: error.to_string(),
                    prefix: kept.clone(),
                });
                return kept;
            }
        }
    }
}

/// Wait for one child while concurrently draining bounded stdout and stderr.
/// Every timeout or stream failure kills and reaps before control returns.
/// The caller must isolate the command with [`isolate_process_group`] before
/// spawning so descendants cannot outlive a timed-out shell.
pub(super) async fn capture_child(
    mut child: tokio::process::Child,
    timeout: Duration,
    context: &str,
) -> Result<CommandResult, BackendFailure> {
    let stdout = child.stdout.take().expect("captured child stdout");
    let stderr = child.stderr.take().expect("captured child stderr");
    let (signal_tx, mut signal_rx) = tokio::sync::mpsc::unbounded_channel();
    let _signal_guard = signal_tx.clone();
    let stdout_task = tokio::spawn(drain_capped(stdout, "stdout", signal_tx.clone()));
    let stderr_task = tokio::spawn(drain_capped(stderr, "stderr", signal_tx));
    let deadline = tokio::time::Instant::now() + timeout;

    let status = tokio::select! {
        result = child.wait() => match result {
            Ok(status) => status,
            Err(error) => {
                terminate_child(&mut child).await;
                let _ = stdout_task.await;
                let _ = stderr_task.await;
                return Err(BackendFailure::new(
                    format!("waiting for {context}"),
                    error.to_string(),
                ));
            }
        },
        failure = signal_rx.recv() => {
            let failure = failure.expect("stream drain signal sender disappeared");
            terminate_child(&mut child).await;
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            return Err(BackendFailure::new(
                format!("reading {context} {} ({})", failure.stream, failure.detail),
                String::from_utf8_lossy(&failure.prefix),
            ));
        }
        _ = tokio::time::sleep_until(deadline) => {
            terminate_child(&mut child).await;
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            return Err(BackendFailure::new(format!("{context} timed out"), ""));
        }
    };
    let stdout = stdout_task.await.map_err(|error| {
        BackendFailure::new(format!("joining {context} stdout drain"), error.to_string())
    })?;
    let stderr = stderr_task.await.map_err(|error| {
        BackendFailure::new(format!("joining {context} stderr drain"), error.to_string())
    })?;
    if let Ok(failure) = signal_rx.try_recv() {
        return Err(BackendFailure::new(
            format!("reading {context} {} ({})", failure.stream, failure.detail),
            String::from_utf8_lossy(&failure.prefix),
        ));
    }
    #[cfg(unix)]
    let signal = {
        use std::os::unix::process::ExitStatusExt;
        status.signal()
    };
    #[cfg(not(unix))]
    let signal = None;
    Ok(CommandResult {
        code: status.code(),
        signal,
        stdout,
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
    })
}

/// Kill a child and every helper in its isolated process group, then reap
/// the direct child before releasing a provisioning lock.
async fn terminate_child(child: &mut tokio::process::Child) {
    #[cfg(unix)]
    if let Some(pid) = child.id() {
        // SAFETY: every production caller invokes `isolate_process_group`
        // before spawn, so the negative pid names only this child's group.
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
    }
    let _ = child.kill().await;
    let _ = child.wait().await;
}

/// Terminate a protocol probe and retain its bounded stderr prefix.
///
/// Probe stdout belongs to the framed handshake, so this is the common cleanup
/// half for the independently drained diagnostic stream.
async fn stop_probe(
    child: &mut tokio::process::Child,
    stderr_task: tokio::task::JoinHandle<ProbeStderr>,
) -> ProbeStderr {
    terminate_child(child).await;
    stderr_task.await.unwrap_or_default()
}

/// Only transport failures are eligible for positive-absence exit
/// classification. A decoded but malformed protocol reply remains a protocol
/// error even if the child happens to exit with the reserved status later.
fn handshake_io_failure(error: &std::io::Error) -> bool {
    ClosedBeforeHello::is_cause_of(error) || error.kind() != std::io::ErrorKind::InvalidData
}

#[async_trait]
impl ProvisioningBackend for SystemBackend {
    async fn probe(&self, target: &ProbeTarget) -> Result<ProbeObservation, BackendFailure> {
        let (mut child, remote) = self.spawn_probe(target).await?;
        let stdout = child.stdout.take().expect("piped probe stdout");
        let stdin = child.stdin.take().expect("piped probe stdin");
        let stderr = child.stderr.take().expect("piped probe stderr");
        let (stderr_signal_tx, mut stderr_signal_rx) = tokio::sync::mpsc::unbounded_channel();
        let _stderr_signal_guard = stderr_signal_tx.clone();
        let stderr_task = tokio::spawn(drain_probe_stderr(stderr, stderr_signal_tx));
        let stdout_bytes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut reader = FrameReader::new(CountingReader {
            inner: stdout,
            bytes: Arc::clone(&stdout_bytes),
        });
        let mut writer = FrameWriter::new(stdin);
        let handshake_result = tokio::select! {
            result = tokio::time::timeout(
                PROBE_TIMEOUT,
                handshake(&mut reader, &mut writer, "helm"),
            ) => result,
            failure = stderr_signal_rx.recv() => {
                let failure = failure.expect("probe stderr signal sender disappeared");
                let stderr = stop_probe(&mut child, stderr_task).await;
                return Err(BackendFailure::new(
                    format!("reading supervisor probe {} ({})", failure.stream, failure.detail),
                    if stderr.prefix.is_empty() {
                        String::from_utf8_lossy(&failure.prefix).into_owned()
                    } else {
                        stderr.diagnostic()
                    },
                ));
            }
        };
        match handshake_result {
            Ok(Ok(ControlMsg::Hello {
                build_version,
                host_identity,
                ..
            })) => {
                let stderr = stop_probe(&mut child, stderr_task).await;
                if let Ok(failure) = stderr_signal_rx.try_recv() {
                    return Err(BackendFailure::new(
                        format!(
                            "reading supervisor probe {} ({})",
                            failure.stream, failure.detail
                        ),
                        String::from_utf8_lossy(&failure.prefix),
                    ));
                }
                let dial_farhelm = if remote {
                    stderr
                        .resolved_farhelm
                        .as_deref()
                        .map(bytes_path)
                        .transpose()
                        .map_err(|error| {
                            BackendFailure::new(
                                "the remote probe reported an unusable resolved binary",
                                error.to_string(),
                            )
                        })?
                        .ok_or_else(|| {
                            BackendFailure::new(
                                "the remote probe answered without reporting its resolved binary",
                                stderr.diagnostic(),
                            )
                        })?
                } else {
                    target.probe_farhelm.clone()
                };
                path_text(&dial_farhelm)?;
                Ok(ProbeObservation::Supervisor {
                    build_version,
                    host_identity,
                    dial_farhelm,
                    dial_state_dir: target.probe_state_dir.clone(),
                })
            }
            // A version-skew refusal must be recognized BEFORE the
            // transport-failure classification below: `handshake` returns
            // it as an `io::Error` of kind `Other`, which
            // `handshake_io_failure` would happily claim — and did, until
            // 2026-09-01, when that misclassification surfaced as "closed
            // before hello completion with exit status 0" on the first
            // cross-protocol UPDATE attempt. The hello DID complete; the
            // peer was refused for speaking another protocol, which is a
            // positive presence observation, not a failure.
            Ok(Err(error)) if VersionSkew::cause_of(&error).is_some() => {
                let peer_build = VersionSkew::cause_of(&error)
                    .expect("guard established the skew payload")
                    .peer_build
                    .clone();
                let stderr = stop_probe(&mut child, stderr_task).await;
                // Same post-stop drain-failure check as the completed-hello
                // arm above, for the same reason: a stderr READ failure is a
                // broken probe transport whichever hello came back, and the
                // two sibling arms must not drift on it.
                if let Ok(failure) = stderr_signal_rx.try_recv() {
                    return Err(BackendFailure::new(
                        format!(
                            "reading supervisor probe {} ({})",
                            failure.stream, failure.detail
                        ),
                        String::from_utf8_lossy(&failure.prefix),
                    ));
                }
                // The dial coordinates resolve exactly as in the completed-
                // hello arm above: the remote script printed its resolved
                // binary to stderr before exec'ing it, and the local target
                // named the binary directly.
                let dial_farhelm = if remote {
                    stderr
                        .resolved_farhelm
                        .as_deref()
                        .map(bytes_path)
                        .transpose()
                        .map_err(|error| {
                            BackendFailure::new(
                                "the remote probe reported an unusable resolved binary",
                                error.to_string(),
                            )
                        })?
                        .ok_or_else(|| {
                            BackendFailure::new(
                                "the remote probe answered without reporting its resolved binary",
                                stderr.diagnostic(),
                            )
                        })?
                } else {
                    target.probe_farhelm.clone()
                };
                path_text(&dial_farhelm)?;
                Ok(ProbeObservation::SkewedSupervisor {
                    peer_build,
                    dial_farhelm,
                    dial_state_dir: target.probe_state_dir.clone(),
                })
            }
            Ok(Err(error)) if handshake_io_failure(&error) => {
                let status = tokio::select! {
                    result = child.wait() => match result {
                        Ok(status) => status,
                        Err(wait_error) => {
                            let stderr = stop_probe(&mut child, stderr_task).await;
                            return Err(BackendFailure::new(
                                "waiting for the failed supervisor probe",
                                if stderr.prefix.is_empty() {
                                    wait_error.to_string()
                                } else {
                                    stderr.diagnostic()
                                },
                            ));
                        }
                    },
                    failure = stderr_signal_rx.recv() => {
                        let failure = failure.expect("probe stderr signal sender disappeared");
                        let stderr = stop_probe(&mut child, stderr_task).await;
                        return Err(BackendFailure::new(
                            format!("reading supervisor probe {} ({})", failure.stream, failure.detail),
                            if stderr.prefix.is_empty() {
                                String::from_utf8_lossy(&failure.prefix).into_owned()
                            } else {
                                stderr.diagnostic()
                            },
                        ));
                    },
                    _ = tokio::time::sleep(Duration::from_secs(2)) => {
                        let stderr = stop_probe(&mut child, stderr_task).await;
                        return Err(BackendFailure::new(
                            "the supervisor probe closed without a hello but did not exit",
                            stderr.diagnostic(),
                        ));
                    }
                };
                // A child can close stdin after writing stdout, so the
                // handshake writer may report BrokenPipe before its reader
                // has been polled. Drain one frame after exit to distinguish
                // genuinely empty positive absence from buffered malformed
                // protocol bytes without trusting task scheduling order.
                let stdout_empty = if stdout_bytes.load(std::sync::atomic::Ordering::Relaxed) == 0 {
                    tokio::time::timeout(Duration::from_secs(2), reader.read_frame())
                        .await
                        .is_ok()
                        && stdout_bytes.load(std::sync::atomic::Ordering::Relaxed) == 0
                } else {
                    false
                };
                let stderr = stderr_task.await.unwrap_or_default();
                if let Ok(failure) = stderr_signal_rx.try_recv() {
                    return Err(BackendFailure::new(
                        format!(
                            "reading supervisor probe {} ({})",
                            failure.stream, failure.detail
                        ),
                        String::from_utf8_lossy(&failure.prefix),
                    ));
                }
                if status.code() == Some(POSITIVE_ABSENCE_EXIT)
                    && stdout_empty
                    && (!remote || stderr.command_started)
                {
                    return Ok(ProbeObservation::Absent);
                }
                Err(BackendFailure::new(
                    format!(
                        "the supervisor probe closed before hello completion with exit status {status}"
                    ),
                    stderr.diagnostic(),
                ))
            }
            result => {
                let message = match result {
                    Ok(Ok(other)) => {
                        format!("the supervisor probe returned a malformed hello: {other:?}")
                    }
                    Err(_) => "the supervisor probe timed out".to_string(),
                    Ok(Err(error)) => {
                        format!("the supervisor probe hello failed: {error:#}")
                    }
                };
                let stderr = stop_probe(&mut child, stderr_task).await;
                Err(BackendFailure::new(message, stderr.diagnostic()))
            }
        }
    }

    async fn inspect(&self, target: &ProbeTarget) -> Result<ReachOutcome, BackendFailure> {
        // Unit discovery must read the user manager's environment, not only
        // the login shell's. Those environments can disagree about
        // XDG_CONFIG_HOME, and writing to the shell's directory would leave
        // a valid-looking unit that this manager never searches.
        let script = "if [ -r /etc/os-release ]; then . /etc/os-release; fi; \
                      printf '%s\\0%s\\0%s\\0' 'farhelm-reach-v1' \"${ID-}\" \"${HOME-}\"; \
                      uname -m | tr -d '\\n'; printf '\\0'; \
                      if command -v tmux >/dev/null 2>&1; then command -v tmux | tr -d '\\n'; fi; \
                      printf '\\0'; \
                      if command -v tmux >/dev/null 2>&1; then tmux -V | tr -d '\\n'; fi; \
                      printf '\\0'; \
                      manager=unavailable; unit_dir=''; \
                      if manager_env=$(systemctl --user show-environment 2>/dev/null); then \
                        manager=usable; \
                        xdg=$(printf '%s\\n' \"$manager_env\" | sed -n 's/^XDG_CONFIG_HOME=//p' | head -n 1); \
                        if [ -n \"$xdg\" ]; then \
                          case $xdg in /*) unit_dir=$xdg/systemd/user ;; *) manager=unsupported-xdg ;; esac; \
                        else unit_dir=$HOME/.config/systemd/user; fi; \
                      fi; \
                      printf '%s\\0%s\\0' \"$manager\" \"$unit_dir\"";
        let output = self
            .run_shell(&target.transport, script, COMMAND_TIMEOUT)
            .await?;
        if output.code != Some(0) {
            return Err(BackendFailure::new(
                "the provisioning reach check failed",
                output.stderr,
            ));
        }
        parse_reach_output(&output.stdout)
    }

    async fn ensure_directories(
        &self,
        target: &ProvisioningTarget,
        directories: &[DirectorySpec],
    ) -> Result<ActionOutcome, BackendFailure> {
        match target {
            ProvisioningTarget::Local => {
                for directory in directories {
                    tokio::fs::create_dir_all(&directory.path)
                        .await
                        .map_err(|error| {
                            BackendFailure::new(
                                format!("creating directory {}", directory.path.display()),
                                error.to_string(),
                            )
                        })?;
                    set_mode(&directory.path, directory.mode).await?;
                }
            }
            ProvisioningTarget::Ssh { .. } => {
                let commands = directories
                    .iter()
                    .map(|directory| {
                        Ok(format!(
                            "install -d -m {:o} -- {}",
                            directory.mode,
                            shell_path(&directory.path)?
                        ))
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .join(" && ");
                self.require_shell(target, &commands, "creating provisioning directories")
                    .await?;
            }
        }
        Ok(ActionOutcome::Completed)
    }

    async fn install_path(
        &self,
        target: &ProvisioningTarget,
        _kind: PayloadKind,
        payload: &PreparedPayload,
        destination: &Path,
        temporary: &Path,
        mode: u32,
    ) -> Result<ActionOutcome, BackendFailure> {
        self.install_source(
            target,
            payload.path(),
            &payload.hash,
            InstallDestination {
                path: destination,
                temporary,
                mode,
                description: "payload",
            },
        )
        .await
    }

    async fn install_bytes(
        &self,
        target: &ProvisioningTarget,
        content: &[u8],
        destination: &Path,
        temporary: &Path,
        mode: u32,
    ) -> Result<ActionOutcome, BackendFailure> {
        use std::io::Write;
        let mut file = tempfile::NamedTempFile::new().map_err(|error| {
            BackendFailure::new("creating the local unit transfer file", error.to_string())
        })?;
        file.write_all(content).map_err(|error| {
            BackendFailure::new("writing the local unit transfer file", error.to_string())
        })?;
        self.install_source(
            target,
            file.path(),
            &hex_sha256(content),
            InstallDestination {
                path: destination,
                temporary,
                mode,
                description: "unit content",
            },
        )
        .await
    }

    async fn daemon_reload(
        &self,
        target: &ProvisioningTarget,
    ) -> Result<ActionOutcome, BackendFailure> {
        self.require_shell(
            target,
            "systemctl --user daemon-reload",
            "reloading the systemd user manager",
        )
        .await?;
        Ok(ActionOutcome::Completed)
    }

    async fn enable_now(
        &self,
        target: &ProvisioningTarget,
        unit: &str,
        unit_path: &Path,
    ) -> Result<ActionOutcome, BackendFailure> {
        let enable_target = if self.runtime_units {
            shell_path(unit_path)?
        } else {
            shell_words::quote(unit).into_owned()
        };
        let runtime = if self.runtime_units { " --runtime" } else { "" };
        self.require_shell(
            target,
            &format!("systemctl --user{runtime} enable --now -- {enable_target}"),
            "enabling and starting the supervisor unit",
        )
        .await?;
        Ok(ActionOutcome::Completed)
    }

    async fn enable_linger(
        &self,
        target: &ProvisioningTarget,
    ) -> Result<ActionOutcome, BackendFailure> {
        match &self.linger {
            LingerBehavior::Real => {
                let output = self
                    .run_shell(
                        target,
                        "LC_ALL=C loginctl --no-ask-password enable-linger \"$(id -un)\"",
                        COMMAND_TIMEOUT,
                    )
                    .await?;
                if output.code == Some(0) {
                    return Ok(ActionOutcome::Completed);
                }
                if linger_was_refused(output.code, &output.stderr) {
                    return Ok(ActionOutcome::Degraded(
                        "linger was refused; starts at login, not at boot".to_string(),
                    ));
                }
                Err(BackendFailure::new("enabling linger", output.stderr))
            }
            #[cfg(test)]
            LingerBehavior::Simulated(Ok(())) => Ok(ActionOutcome::Completed),
            #[cfg(test)]
            LingerBehavior::Simulated(Err(message)) => Ok(ActionOutcome::Degraded(format!(
                "linger was refused ({message}); starts at login, not at boot"
            ))),
        }
    }

    async fn restart(
        &self,
        target: &ProvisioningTarget,
        unit: &str,
    ) -> Result<ActionOutcome, BackendFailure> {
        let unit = shell_words::quote(unit);
        self.require_shell(
            target,
            &format!("systemctl --user restart -- {unit}"),
            "restarting the supervisor unit",
        )
        .await?;
        Ok(ActionOutcome::Completed)
    }

    /// The helm's own process environment is the authority here, which is
    /// the one place in this file where that is true rather than a
    /// shortcut: this method asks about the machine the helm itself runs
    /// on, so the manager that would load the unit is the helm's own user
    /// manager. Remote hosts are asked over SSH by `inspect` instead, and
    /// the derivation is shared with it through
    /// [`crate::units::user_unit_dir`].
    ///
    /// Everything short of a confirmed absence is an error. A missing
    /// `HOME` with no absolute `XDG_CONFIG_HOME`, a file this process may
    /// not read, contents that are not UTF-8 — none of those mean "no
    /// protected unit exists", and answering `None` for them would let
    /// the panel install over a unit it merely failed to inspect.
    async fn read_user_unit(&self, name: &str) -> Result<Option<String>, BackendFailure> {
        let home = std::env::var_os("HOME").filter(|value| !value.is_empty());
        let directory = crate::units::user_unit_dir_for(
            std::env::var_os("XDG_CONFIG_HOME").as_deref(),
            home.as_ref().map(Path::new),
        )
        .map_err(|error| BackendFailure::new(format!("{error:#}"), ""))?;
        let path = directory.join(name);
        match tokio::fs::read(&path).await {
            Ok(bytes) => String::from_utf8(bytes).map(Some).map_err(|_| {
                BackendFailure::new(
                    format!(
                        "the unit file {} is not valid UTF-8, so it cannot be checked for \
                         ownership",
                        path.display()
                    ),
                    "",
                )
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(BackendFailure::new(
                format!("reading the unit file {}", path.display()),
                error.to_string(),
            )),
        }
    }
}

/// Encode a path as one remote shell word, refusing bytes that cannot cross
/// SSH's text command boundary without changing meaning.
pub(super) fn shell_path(path: &Path) -> Result<String, BackendFailure> {
    Ok(shell_words::quote(&path_text(path)?).into_owned())
}

/// Preserve a path exactly at every text-only SSH, registry, and systemd
/// boundary. Rejecting before confirmation is safer than displaying one path
/// and later mutating a lossy approximation of it.
///
/// Provisioning's failure type over [`crate::units::path_text`], which owns
/// the rule so that unit rendering and remote command lines cannot come to
/// disagree about which paths are representable. No host ran anything, so
/// there is no host stderr to carry.
pub(super) fn path_text(path: &Path) -> Result<String, BackendFailure> {
    crate::units::path_text(path).map_err(|error| BackendFailure::new(format!("{error:#}"), ""))
}

/// Encode one batch-mode sftp path. Sftp has its own quoting grammar and
/// cannot reuse shell quoting safely.
pub(super) fn sftp_path(path: &Path) -> Result<String, BackendFailure> {
    let text = path.to_str().ok_or_else(|| {
        BackendFailure::new(
            format!("path {} is not valid UTF-8", path.to_string_lossy()),
            "sftp batch paths are text",
        )
    })?;
    if text.contains('\0') || text.contains('\n') || text.contains('\r') {
        return Err(BackendFailure::new(
            format!("path {text:?} cannot be represented in an sftp batch"),
            "",
        ));
    }
    Ok(format!(
        "\"{}\"",
        text.replace('\\', "\\\\").replace('"', "\\\"")
    ))
}

/// Set the final mode on a temporary file before its atomic rename.
#[cfg(unix)]
pub(super) async fn set_mode(path: &Path, mode: u32) -> Result<(), BackendFailure> {
    use std::os::unix::fs::PermissionsExt;
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .await
        .map_err(|error| {
            BackendFailure::new(
                format!("setting permissions on {}", path.display()),
                error.to_string(),
            )
        })
}

/// Non-Unix builds do not carry Unix executable mode bits.
#[cfg(not(unix))]
pub(super) async fn set_mode(_path: &Path, _mode: u32) -> Result<(), BackendFailure> {
    Ok(())
}

/// Preserve native Unix HOME bytes until a later text-only boundary rejects
/// them explicitly.
#[cfg(unix)]
fn bytes_path(bytes: &[u8]) -> anyhow::Result<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    Ok(PathBuf::from(std::ffi::OsString::from_vec(bytes.to_vec())))
}

/// Non-Unix paths in command output must decode as UTF-8.
#[cfg(not(unix))]
fn bytes_path(bytes: &[u8]) -> anyhow::Result<PathBuf> {
    Ok(PathBuf::from(std::str::from_utf8(bytes)?))
}

/// Lowercase digest spelling shared with `sha256sum` output.
pub(super) fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        write!(&mut out, "{byte:02x}").expect("writing to String cannot fail");
    }
    out
}

/// Whether a host's own `tmux -V` output clears the supervisor's version
/// floor, deciding whether provisioning installs Farhelm's private build.
///
/// The comparison is imported from the supervisor rather than reimplemented
/// so the two can never disagree. Divergence here is not a cosmetic bug: a
/// laxer test would accept a host tmux, skip the payload, and hand the
/// remote supervisor a binary it then refuses at startup — provisioning
/// would report success on a host that cannot start.
///
/// Conservative in the same direction the supervisor is: anything
/// unparseable — including the empty string a host with no tmux reports —
/// requests the private payload rather than assuming compatibility.
pub(super) fn tmux_meets_floor(output: &str) -> bool {
    farhelm_supervisor::tmux::parse_tmux_version(output)
        .is_ok_and(|version| version >= farhelm_supervisor::tmux::TMUX_FLOOR)
}

pub(super) fn linger_was_refused(code: Option<i32>, stderr: &str) -> bool {
    if code == Some(0) {
        return false;
    }
    let lower = stderr.to_ascii_lowercase();
    [
        "permission denied",
        "access denied",
        "authentication is required",
        "interactive authentication required",
        "not authorized",
    ]
    .iter()
    .any(|message| lower.contains(message))
}

/// Turn the reach probe's NUL-delimited record into a support decision.
///
/// Nothing here gates on the distro ID. Every real requirement is a
/// capability the probe measured directly: a payload architecture, a
/// usable systemd user manager (with a resolvable, absolute unit
/// directory), and — checked further down — an acceptable tmux or the
/// ability to install one. The ID is parsed and carried into `Reach`
/// purely so the confirmation plan can name the host it inspected; an
/// empty ID (no `/etc/os-release` at all) is not a reason to refuse a
/// host whose manager is otherwise usable, so it flows through as an
/// empty string rather than a rejection.
pub(super) fn parse_reach_output(output: &[u8]) -> Result<ReachOutcome, BackendFailure> {
    let fields: Vec<&[u8]> = output.split(|byte| *byte == 0).collect();
    if fields.len() != 9 || fields[0] != REACH_RECORD_MARKER.as_bytes() || !fields[8].is_empty() {
        return Err(BackendFailure::new(
            "the provisioning reach check returned malformed output",
            String::from_utf8_lossy(output),
        ));
    }
    let distro_id = String::from_utf8_lossy(fields[1]).into_owned();
    let arch_text = String::from_utf8_lossy(fields[3]);
    let arch = match arch_text.as_ref() {
        "x86_64" => PayloadArch::X86_64,
        "aarch64" | "arm64" => PayloadArch::Aarch64,
        other => {
            return Ok(ReachOutcome::Manual(format!(
                "automatic provisioning has no payload for architecture {other:?}. Run the supervisor manually."
            )));
        }
    };
    if fields[6] != b"usable" {
        let reason = if fields[6] == b"unsupported-xdg" {
            "the systemd user manager reports a relative XDG_CONFIG_HOME, so Farhelm cannot determine its unit directory"
        } else {
            "automatic provisioning requires a usable systemd user manager"
        };
        return Ok(ReachOutcome::Manual(format!(
            "{reason}; run the supervisor manually on this host."
        )));
    }
    let home = bytes_path(fields[2]).map_err(|error| {
        BackendFailure::new("the host reported an unusable HOME", error.to_string())
    })?;
    if home.as_os_str().is_empty() || !home.is_absolute() {
        return Err(BackendFailure::new(
            "the host reported an unusable HOME",
            format!("expected an absolute path, got {}", home.display()),
        ));
    }
    if let Err(error) = path_text(&home) {
        return Ok(ReachOutcome::Manual(format!(
            "automatic provisioning cannot represent this host's HOME at its text-only plan boundary ({error}); run the supervisor manually with explicit paths."
        )));
    }
    let user_unit_dir = bytes_path(fields[7]).map_err(|error| {
        BackendFailure::new(
            "the host reported an unusable systemd user unit directory",
            error.to_string(),
        )
    })?;
    if user_unit_dir.as_os_str().is_empty()
        || !user_unit_dir.is_absolute()
        || path_text(&user_unit_dir).is_err()
    {
        return Ok(ReachOutcome::Manual(format!(
            "automatic provisioning cannot use the systemd user unit directory {}; run the supervisor manually with explicit paths.",
            user_unit_dir.display()
        )));
    }
    let tmux_path = bytes_path(fields[4]).map_err(|error| {
        BackendFailure::new("the host reported an unusable tmux path", error.to_string())
    })?;
    let tmux = String::from_utf8_lossy(fields[5]);
    let tmux_ok =
        tmux_meets_floor(&tmux) && tmux_path.is_absolute() && path_text(&tmux_path).is_ok();
    Ok(ReachOutcome::Supported(Reach {
        home,
        user_unit_dir,
        arch,
        distro_id,
        needs_tmux: !tmux_ok,
        host_tmux: tmux_ok.then_some(tmux_path),
    }))
}
