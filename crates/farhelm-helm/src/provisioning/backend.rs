//! The system-facing half of provisioning executes the same action vocabulary
//! for a local host or through the user's SSH access.

use super::plan::{DirectorySpec, PayloadKind, PayloadTarget, ProvisioningTarget};
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
use tracing::warn;

const PROBE_TIMEOUT: Duration = Duration::from_secs(15);
pub(super) const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
/// The uninstall stop step's bound: [`COMMAND_TIMEOUT`] for its systemd,
/// tmux and cleanup commands, plus the time this build's
/// `farhelm internal stop-terminal-hosts` takes at most between them, and ten
/// seconds for starting that program and its runtime on a loaded host. The
/// remote `farhelm` may come from another release, whose bound is not part of
/// the command's stable contract; the ten seconds are also the margin for
/// that.
const STOP_SUPERVISOR_TIMEOUT: Duration = COMMAND_TIMEOUT
    .saturating_add(farhelm_supervisor::hosts::stop::COMMAND_BUDGET)
    .saturating_add(Duration::from_secs(10));
/// `case` patterns, for a remote shell, matching exactly the diagnostics the
/// supported `stat` implementations (GNU, uutils, BusyBox) print in the C
/// locale for a path that does not exist. Only these excuse treating a lookup
/// as absence: every other failure, permission and I/O errors included, says
/// nothing about whether the path is there.
const STAT_MISSING_PATH_PATTERNS: &str = "'stat: cannot stat'*': No such file or directory'|\
     'stat: cannot stat'*': No such file or directory (os error 2)'|\
     'stat: can'\"'\"'t stat'*': No such file or directory'";
// A transfer has no observable byte progress until the remote temporary exists.
// Once it does, only verified growth of that file renews the idle deadline.
const TRANSFER_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const TRANSFER_PROGRESS_POLL: Duration = Duration::from_secs(2);
// Once a child has exited or been killed, its pipes reach EOF unless another
// process inherited them and outlived it (a local ssh helper, say). Expiry
// fails the command on steps every install takes, so this exceeds the probe's
// 2 s: a ControlPersist check saw EOF about 0.5 s after exit, a thin margin
// under load, and a helper that lingers briefly used to cost only a delay.
// Ten seconds still ends an indefinite wait and frees the planning slot.
pub(super) const CAPTURE_PIPE_GRACE: Duration = Duration::from_secs(10);
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
        /// The host supervisor's database schema version, when it reports one.
        /// Update refuses a host whose schema is above the one this helm's
        /// supervisor opens; `None` (an older supervisor) is never refused.
        database_schema: Option<i64>,
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
        /// The database schema the skewed supervisor reported, if any, for
        /// the same Update refusal as [`ProbeObservation::Supervisor`]'s.
        database_schema: Option<i64>,
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
/// including the native service-file directory and, on a Mac, launchd domain.
#[derive(Debug, Clone)]
pub(super) struct Reach {
    pub(super) home: PathBuf,
    pub(super) user_unit_dir: PathBuf,
    pub(super) arch: PayloadTarget,
    /// The Linux `/etc/os-release` `ID`, empty when absent, or `macOS`.
    ///
    /// Informational only: nothing in this struct's construction branches
    /// on it, because it predicts none of the capabilities provisioning
    /// actually needs (the native payload, service manager and commands). It is
    /// carried through purely so the plan text can name the host it
    /// inspected — see `ProvisioningPlan::confirmation`.
    pub(super) distro_id: String,
    pub(super) needs_tmux: bool,
    /// The inspected per-user launchd domain on a Mac. Kept in frozen plan
    /// actions so execution cannot switch the service to a different user.
    pub(super) launchd_domain: Option<String>,
    /// The host's OWN tmux executable, absolute, when it cleared the
    /// floor — `None` whenever `needs_tmux` is set, and always on a Mac.
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

/// Supported hosts continue to a plan; refusals retain a reason for the panel.
/// A Mac owned by its desktop app is refused without a competing-start remedy.
#[derive(Debug, Clone)]
pub(super) enum ReachOutcome {
    Supported(Reach),
    Manual(String),
    /// The host's supervisor unit carries `farhelm helm setup`'s managed-by
    /// marker, so setup owns it there and the panel acts on it for no
    /// operation. Kept apart from `Manual` because the refusal's remedy
    /// depends on the operation; see
    /// `ProvisioningOperation::setup_managed_refusal`.
    SetupManaged,
}

/// What [`ProvisioningBackend::inspect_uninstall`] found on the host.
///
/// Canonical paths resolve symlinks on the host, then normalize a second
/// name for the login's home only when both full paths name the same file.
/// Every uninstall guard consumes that spelling: the unit must be the file
/// removed, the running binary must be inside the removed lib folder, and
/// the state directory must stay outside it, which is also checked in the
/// spelling from before normalization and by file identity
/// ([`HostPath::under_removed`], which can only refuse). Aliases of other
/// folders remain distinct, per SPEC.md's narrow uninstall exception.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UninstallInspection {
    /// One entry per asked path, in the order asked.
    pub(super) paths: Vec<HostPath>,
    /// The state directory a supervisor started without `--state-dir`
    /// picks on this host (`$XDG_STATE_HOME/farhelm` when that is absolute,
    /// `~/.local/state/farhelm` otherwise, the supervisor's own rule), as
    /// the same ssh environment the probe dials through resolves it.
    pub(super) default_state_dir: HostPath,
    /// The unit's `ActiveState` (`active`, `inactive`, `failed`, ...).
    /// systemd reports `inactive` for a unit it does not know.
    pub(super) unit_active_state: String,
    /// Where systemd loaded the unit from (`FragmentPath`), with its
    /// canonical form, or `None` when systemd knows no file for it. A
    /// unit loaded from anywhere but the plan's unit path is not the one
    /// uninstall would remove, and "no unit file left" says nothing about
    /// it.
    pub(super) unit_fragment: Option<HostPath>,
    /// Mount points at or below an asked path's two spellings, octal escapes
    /// decoded as `readlink -f` would print them.
    pub(super) mounts: Vec<PathBuf>,
}

/// One path as the host sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HostPath {
    pub(super) path: PathBuf,
    /// Whether anything is at the path (a dangling symlink counts).
    pub(super) exists: bool,
    /// Whether the path itself is a symlink (`[ -L ]`), dangling or not.
    /// Only the last component counts: a path under a symlinked parent is
    /// not one, because removing it reaches the real directory. Uninstall
    /// refuses a lib directory that is one, since `rm -rf` of the link
    /// would remove only the link and leave the installation behind.
    pub(super) symlink: bool,
    /// The host's `readlink -f` answer, with a verified alternate home
    /// prefix rewritten to the login home's canonical spelling. No other
    /// folder aliases are folded. `None` means the host could not resolve
    /// the path (a missing parent, a symlink loop).
    pub(super) canonical: Option<PathBuf>,
    /// The host's `readlink -f` answer before home normalization, `None`
    /// exactly when `canonical` is; `inspect_uninstall` says why
    /// containment checks need it.
    pub(super) raw: Option<PathBuf>,
    /// Whether `raw`, or a folder above it, is the same file (`-ef`) as
    /// the folder uninstall would remove: containment no spelling shows,
    /// such as state and lib reached through different binds of the home.
    /// It only ever adds a refusal, never accepts an alias.
    pub(super) under_removed: bool,
}

impl UninstallInspection {
    /// Whether the supervisor unit is running or changing state, as opposed
    /// to stopped or unknown to systemd.
    pub(super) fn unit_running(&self) -> bool {
        !matches!(self.unit_active_state.as_str(), "inactive" | "failed")
    }
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
pub(crate) struct BackendFailure {
    pub(crate) context: String,
    stderr: String,
}

/// Internal refusal identity, assigned from the final guard rather than peer text.
pub(super) const DESTINATION_CHANGED: &str = "destination changed before replacement";

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

    pub(crate) fn rendered(&self) -> String {
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
    /// Remove only the Farhelm-owned token path, with absence already clean.
    /// No other GitHub authentication or user configuration is removed.
    async fn remove_github_token(
        &self,
        _target: &ProvisioningTarget,
        _path: &Path,
    ) -> Result<(), BackendFailure> {
        Err(BackendFailure::new(
            "this backend cannot remove the managed GitHub token",
            "",
        ))
    }

    /// Configure gh's own github.com helper without receiving a token. False
    /// means gh is missing; the token file can still serve new child processes.
    async fn setup_github_git(&self, _target: &ProvisioningTarget) -> Result<bool, BackendFailure> {
        Err(BackendFailure::new(
            "this backend cannot configure the GitHub helper",
            "",
        ))
    }
    /// Recover only the login home for legacy host bindings. Unlike sandbox
    /// preparation, this must never start services or alter the host.
    async fn host_home(&self, _target: &ProvisioningTarget) -> Result<PathBuf, BackendFailure> {
        Err(BackendFailure::new(
            "this backend cannot read the sandbox home",
            "",
        ))
    }
    /// Wait for the gateway after the API reports Running, without changing
    /// services or files. Prepared clones must be reachable before admission
    /// and credential delivery; unsupported backends refuse that boundary.
    async fn wait_tensorlake_ssh(
        &self,
        _target: &ProvisioningTarget,
    ) -> Result<(), BackendFailure> {
        Err(BackendFailure::new(
            "this backend cannot wait for Tensorlake SSH",
            "",
        ))
    }

    /// Prepare a newly created sandbox's user manager and return its home.
    /// Unsupported backends fail explicitly rather than skipping preparation.
    async fn prepare_tensorlake_sandbox(
        &self,
        _target: &ProvisioningTarget,
    ) -> Result<PathBuf, BackendFailure> {
        Err(BackendFailure::new(
            "this backend cannot prepare Tensorlake sandboxes",
            "",
        ))
    }

    /// Read at most 1 MiB, distinguishing an absent file from an unreadable
    /// one. Implementations must never quote the contents in their errors.
    async fn read_small_file(
        &self,
        _target: &ProvisioningTarget,
        _path: &Path,
    ) -> Result<Option<Vec<u8>>, BackendFailure> {
        Err(BackendFailure::new(
            "this backend cannot read agent settings",
            "",
        ))
    }

    /// Refuse a copied supervisor state before starting it under another
    /// host. Reruns of the same host may keep its own state.
    async fn refuse_copied_supervisor(
        &self,
        _target: &ProvisioningTarget,
        _state_dir: &Path,
    ) -> Result<(), BackendFailure> {
        Err(BackendFailure::new(
            "this backend cannot check copied supervisor state",
            "",
        ))
    }
    /// Leave the installed supervisor enabled for clone boot, but stop all
    /// of its processes and remove its entire planned state before capture.
    /// Success also verifies state absence and clears machine-id. Injected
    /// backends must opt in explicitly; ordinary setup never needs this proof.
    async fn finish_snapshot_install(
        &self,
        _target: &ProvisioningTarget,
        _unit: &str,
        _state_dir: &Path,
    ) -> Result<(), BackendFailure> {
        Err(BackendFailure::new(
            "this backend cannot verify a prepared snapshot installation",
            "",
        ))
    }

    async fn probe(&self, target: &ProbeTarget) -> Result<ProbeObservation, BackendFailure>;
    async fn inspect(&self, target: &ProbeTarget) -> Result<ReachOutcome, BackendFailure>;
    async fn ensure_directories(
        &self,
        target: &ProvisioningTarget,
        directories: &[DirectorySpec],
    ) -> Result<ActionOutcome, BackendFailure>;
    /// Transfer a remote binary into its nonce temporary before installation.
    /// Local plans have no upload action and must not call this method.
    async fn upload_path(
        &self,
        target: &ProvisioningTarget,
        kind: PayloadKind,
        payload: &PreparedPayload,
        destination: &Path,
        temporary: &Path,
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
    /// Install bytes without an old-generation condition, retaining ordinary
    /// provisioning convergence and mode repair for non-settings callers.
    async fn install_bytes(
        &self,
        target: &ProvisioningTarget,
        content: &[u8],
        destination: &Path,
        temporary: &Path,
        mode: u32,
    ) -> Result<ActionOutcome, BackendFailure> {
        self.install_bytes_checked(target, content, destination, temporary, mode, None)
            .await
    }
    /// `None` is unconditional; `Some(None)` requires absence, and `Some(Some(hash))`
    /// requires those old bytes at the final rename boundary. A refusal preserves the file.
    async fn install_bytes_checked(
        &self,
        target: &ProvisioningTarget,
        content: &[u8],
        destination: &Path,
        temporary: &Path,
        mode: u32,
        expected: Option<Option<&str>>,
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

    /// Start or refresh only the frozen per-user launchd registration.
    async fn launch_agent(
        &self,
        _target: &ProvisioningTarget,
        _domain: &str,
        _label: &str,
        _plist: &Path,
        _refresh: bool,
    ) -> Result<ActionOutcome, BackendFailure> {
        Err(BackendFailure::new(
            "this backend does not support launchd",
            "",
        ))
    }

    /// End a launchd supervisor and confirm its owned terminal hosts ended.
    async fn stop_launch_agent(
        &self,
        _target: &ProvisioningTarget,
        _domain: &str,
        _label: &str,
        _farhelm: &Path,
        _state: &Path,
    ) -> Result<ActionOutcome, BackendFailure> {
        Err(BackendFailure::new(
            "this backend does not support launchd",
            "",
        ))
    }

    /// Read, without changing anything, what UNINSTALL needs to know about
    /// the installation on the target: which of `paths` exist, whether each
    /// is itself a symlink, and where they really lead, the state directory
    /// a supervisor started without `--state-dir` would use there, and the
    /// supervisor unit's run state and loaded unit-file location. Each path
    /// also says whether it lies in `removed`, the folder the uninstall
    /// would delete, by file identity rather than spelling
    /// ([`HostPath::under_removed`]). See [`UninstallInspection`].
    /// `launchd_domain` selects a Mac job label in `unit`; `None` keeps the
    /// existing Linux unit inspection. The domain is frozen by host inspection,
    /// never inferred from the session which later executes the uninstall.
    async fn inspect_uninstall(
        &self,
        target: &ProvisioningTarget,
        unit: &str,
        launchd_domain: Option<&str>,
        paths: &[&Path],
        removed: &Path,
    ) -> Result<UninstallInspection, BackendFailure>;
    /// Disable the supervisor unit without stopping it.
    async fn disable(
        &self,
        target: &ProvisioningTarget,
        unit: &str,
    ) -> Result<ActionOutcome, BackendFailure>;
    /// Delete the supervisor unit file, refusing one that carries setup's
    /// managed-by marker; an absent file is a skip.
    async fn remove_unit(
        &self,
        target: &ProvisioningTarget,
        destination: &Path,
    ) -> Result<ActionOutcome, BackendFailure>;
    /// Stop the supervisor, then end the terminal hosts of `state_dir` with
    /// the installed `farhelm_program`, then its private tmux server at the
    /// frozen socket. Planning removes and reloads the unit first; an
    /// inactive unit skips only the systemd command, since its hosts and its
    /// private server may survive. Failure to confirm every host stopped, or
    /// to end a reachable server, fails the step and permits retry.
    async fn stop(
        &self,
        target: &ProvisioningTarget,
        unit: &str,
        farhelm_program: &Path,
        state_dir: &Path,
        tmux_program: &Path,
        tmux_socket: &Path,
    ) -> Result<ActionOutcome, BackendFailure>;
    /// Delete one directory tree; an absent one is a skip, and one that is
    /// itself a symlink is refused rather than unlinked, because removing
    /// the link would leave the tree it points to behind.
    async fn remove_directory(
        &self,
        target: &ProvisioningTarget,
        path: &Path,
    ) -> Result<ActionOutcome, BackendFailure>;

    /// Read one unit file out of THIS machine's systemd user directory,
    /// or `None` when no such file exists.
    ///
    /// Takes no target on purpose: the only question it answers is about
    /// the helm's own machine, where `farhelm helm setup` — not the hosts
    /// panel — owns the units (D9). The panel never installs on the local
    /// row; the local row consults this only AFTER a probe found no
    /// supervisor (or to refuse an update), to choose what to tell the user:
    /// run `farhelm helm setup`, or leave the existing unit alone.
    ///
    /// `None` means one thing only: no such file. Every other outcome —
    /// an unreadable file, contents that are not UTF-8, a unit directory
    /// that cannot be located — is an error, because the caller reads
    /// `None` as "there is no unit here" and would tell the user to run
    /// setup over a unit it merely failed to read.
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
    /// Reused preparer names must never ride a previous sandbox's SSH master.
    pub(super) one_shot: bool,
    #[cfg(test)]
    pub(super) fail_before_rename: bool,
}

/// Process-creation seam that tests use to exercise production classifiers
/// without invoking a real SSH endpoint. Test launchers that stand in for
/// `ssh` go through `super::ssh_stand_in`, which gives the helm's pipes the
/// lifetimes a real shared connection gives them; the one exception is a
/// test that builds a deliberately unmeasured shape and says so.
pub(crate) trait CommandLauncher: Send + Sync {
    fn spawn(
        &self,
        command: &mut tokio::process::Command,
    ) -> std::io::Result<tokio::process::Child>;
}

/// Ordinary launcher: all lifecycle and stream policy stays with the caller.
pub(crate) struct SystemLauncher;

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
pub(crate) fn isolate_process_group(command: &mut tokio::process::Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.as_std_mut().process_group(0);
    }
}

impl SystemBackend {
    /// Remove only abandoned Farhelm staging files for one destination.
    ///
    /// The nonce is part of the filename so a killed install can leave a
    /// payload behind after its owning task disappears.  Matching the exact
    /// destination basename and canonical UUID shape keeps this best-effort
    /// sweep from treating an unrelated dotfile as ours.
    async fn cleanup_orphaned_temporaries(
        &self,
        target: &ProvisioningTarget,
        destination: &Path,
    ) -> Result<(), BackendFailure> {
        let Some(name) = destination.file_name().and_then(|name| name.to_str()) else {
            return Ok(());
        };
        let prefix = format!(".{name}.farhelm-tmp-");
        match target {
            ProvisioningTarget::Local => {
                let Some(parent) = destination.parent() else {
                    return Ok(());
                };
                let mut entries = match tokio::fs::read_dir(parent).await {
                    Ok(entries) => entries,
                    Err(error) => {
                        warn!(directory = %parent.display(), %error, "could not inspect Farhelm temporary files");
                        return Ok(());
                    }
                };
                loop {
                    let entry = match entries.next_entry().await {
                        Ok(Some(entry)) => entry,
                        Ok(None) => break,
                        Err(error) => {
                            warn!(directory = %parent.display(), %error, "could not finish inspecting Farhelm temporary files");
                            break;
                        }
                    };
                    let path = entry.path();
                    let Some(candidate) = path.file_name().and_then(|name| name.to_str()) else {
                        continue;
                    };
                    let Some(suffix) = candidate.strip_prefix(&prefix) else {
                        continue;
                    };
                    let Ok(uuid) = uuid::Uuid::parse_str(suffix) else {
                        continue;
                    };
                    if uuid.to_string() != suffix {
                        continue;
                    }
                    let metadata = match tokio::fs::symlink_metadata(&path).await {
                        Ok(metadata) => metadata,
                        Err(error) => {
                            warn!(path = %path.display(), %error, "could not inspect a Farhelm temporary file");
                            continue;
                        }
                    };
                    if !metadata.file_type().is_file() {
                        continue;
                    }
                    if let Err(error) = tokio::fs::remove_file(&path).await {
                        warn!(path = %path.display(), %error, "could not remove an orphaned Farhelm temporary file");
                    }
                }
            }
            ProvisioningTarget::Ssh { .. } => {
                let Some(parent) = destination.parent() else {
                    return Ok(());
                };
                let script = match orphan_cleanup_script(parent, &prefix) {
                    Ok(script) => script,
                    Err(error) => {
                        warn!(directory = %parent.display(), %error, "could not construct Farhelm temporary-file cleanup");
                        return Ok(());
                    }
                };
                if let Err(error) = self
                    .require_shell(target, &script, "cleaning orphaned Farhelm temporary files")
                    .await
                {
                    warn!(error = %error, "could not complete orphaned Farhelm temporary-file cleanup");
                }
            }
        }
        Ok(())
    }

    /// Production uses the real optional linger action.
    pub(super) fn new(control_dir: PathBuf) -> Self {
        Self {
            control_dir,
            linger: LingerBehavior::Real,
            launcher: Arc::new(SystemLauncher),
            runtime_units: false,
            one_shot: false,
            #[cfg(test)]
            fail_before_rename: false,
        }
    }

    /// Use a new SSH transport for every preparer command, including inspect
    /// and uploads. The normal host backend keeps its shared connection.
    pub(super) fn one_shot(control_dir: PathBuf) -> Self {
        Self {
            one_shot: true,
            ..Self::new(control_dir)
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
            one_shot: false,
            fail_before_rename: false,
        }
    }

    /// Build every remote operation with the selected SSH connection policy.
    /// Ordinary host work rides provisioning's own shared connection;
    /// preparation uses one-shot connections because its sandbox name recurs.
    ///
    /// The separate connection is what lets provisioning run while the helm
    /// is connected on a host whose sshd allows one session per connection:
    /// the supervisor connection's master spends that session on the
    /// long-lived proxy (see the `crate::ssh` module docs). Every caller in
    /// this file goes through here, including inspect and uploads, so no
    /// preparation step can accidentally reuse a previous sandbox's master.
    ///
    /// `access` is the target's: a Tensorlake sandbox is provisioned with
    /// Farhelm's own key, like every other connection to it, and the payload
    /// uploads and probes inherit that by coming through here too.
    pub(super) fn ssh_command(
        &self,
        destination: &str,
        access: crate::ssh::SshAccessKind,
        remote_command: String,
    ) -> anyhow::Result<tokio::process::Command> {
        let mut command = tokio::process::Command::new("ssh");
        let access = crate::ssh::SshAccess::resolve(access, &self.control_dir)?;
        if self.one_shot {
            let options = match &access {
                crate::ssh::SshAccess::UserConfig => Vec::new(),
                crate::ssh::SshAccess::Tensorlake(access) => {
                    access.oneshot_options().map_err(anyhow::Error::msg)?
                }
            };
            command.args(crate::ssh::ssh_oneshot_args(destination, &options));
        } else {
            command.args(crate::ssh::ssh_base_args(
                destination,
                &self.control_dir,
                crate::ssh::SharedConnection::Provisioning,
                &access,
            )?);
        }
        // ssh concatenates its trailing argv and reparses it remotely. Keep
        // the complete `sh -c` invocation in one shell-quoted string so the
        // script cannot absorb words from a destination or path.
        command.arg(format!(
            "sh -c {}",
            crate::ssh::shell_quote(&format!("{REMOTE_RUNTIME_PREFIX}{remote_command}"))
        ));
        Ok(command)
    }

    /// Fetch the user manager's `XDG_CONFIG_HOME` from the host and choose
    /// the unit directory it loads units from (see
    /// [`manager_unit_dir_from_probe`]).
    ///
    /// `show-environment` doubles as the "is there a usable manager" probe
    /// the reach check always made; `busctl` then supplies the value itself
    /// when the user bus answers, and `show-environment`'s own line stands in
    /// when it does not.
    ///
    /// Only the `XDG_CONFIG_HOME` line leaves the host. The manager's
    /// environment can hold credentials and values of any size, and the
    /// whole block would travel into diagnostics a failure renders for the
    /// hosts panel, and past the capture limit for host output. `busctl`'s
    /// pretty JSON puts each array element on a line of its own (a newline
    /// in a value is escaped), and `show-environment` prints one variable per
    /// line, so a line filter picks exactly the one entry.
    async fn manager_unit_dir_choice(
        &self,
        target: &ProbeTarget,
    ) -> Result<ManagerUnitDir, BackendFailure> {
        let busctl = crate::units::MANAGER_ENVIRONMENT_BUSCTL_ARGS
            .iter()
            .map(|arg| {
                crate::ssh::shell_quote(if *arg == "--json=short" {
                    "--json=pretty"
                } else {
                    arg
                })
            })
            .collect::<Vec<_>>()
            .join(" ");
        let script = format!(
            "if se=$(systemctl --user show-environment 2>/dev/null); then \
               if bj=$(busctl --user {busctl} 2>/dev/null); then \
                 printf 'busctl\\0'; \
                 printf '%s\\n' \"$bj\" | grep '^[[:space:]]*\"XDG_CONFIG_HOME=' | head -n 1; \
               else \
                 printf 'show-environment\\0'; \
                 printf '%s\\n' \"$se\" | grep '^XDG_CONFIG_HOME=' | head -n 1; \
               fi; \
               printf '\\0'; \
             else printf 'unavailable\\0\\0'; fi"
        );
        let output = self
            .run_shell(&target.transport, &script, COMMAND_TIMEOUT)
            .await?;
        if output.code != Some(0) {
            return Err(BackendFailure::new(
                "reading the systemd user manager's environment failed",
                output.stderr,
            ));
        }
        manager_unit_dir_from_probe(&output.stdout)
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
            ProvisioningTarget::Ssh {
                destination,
                access,
            } => self
                .ssh_command(destination, *access, script.to_string())
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
        self.require_shell_within(target, script, context, COMMAND_TIMEOUT)
            .await
    }

    /// [`Self::require_shell`] for a script that runs a command with its own,
    /// longer bound than [`COMMAND_TIMEOUT`].
    async fn require_shell_within(
        &self,
        target: &ProvisioningTarget,
        script: &str,
        context: &str,
        timeout: Duration,
    ) -> Result<CommandResult, BackendFailure> {
        let output = self.run_shell(target, script, timeout).await?;
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
                        hash: lower_hex(&digest.finalize()),
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
                // Read the file through stdin. GNU coreutils escapes a
                // backslash or newline in a filename in its output, so
                // passing the path as an argument would corrupt the digest
                // parser; this follows install.sh's sha256_of convention.
                // `stat -L` reads the mode of the file the hash describes:
                // `-e` and the digest follow a symlink, and without `-L` the
                // mode was the link's own (always 777 on Linux), so an
                // unchanged symlinked binary or unit was "repaired" by a
                // chmod through the link on every run, or failed with EPERM.
                // The local transport already reads the opened target's mode.
                // This shared path also installs credentials, so choose native
                // stat syntax here rather than relying on an earlier plan's OS.
                let output = self
                    .run_shell(
                        target,
                        &format!(
                            "if [ ! -e {path} ]; then exit 44; fi; \
                             {} && os=$(uname -s) && case \"$os\" in \
                             Darwin) stat -L -f '%Lp' -- {path} ;; \
                             *) stat -L -c '%a' -- {path} ;; esac",
                            remote_sha256sum(&path)
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
                    // BSD chmod ends option parsing at the mode. Its `--`
                    // must precede the mode, or it becomes a filename; GNU
                    // chmod accepts this ordering too.
                    &format!("chmod -- {mode:o} {}", shell_path(path)?),
                    "repairing installed permissions",
                )
                .await?;
                Ok(())
            }
        }
    }

    /// Remove exactly one Farhelm-owned file on either transport: a run's
    /// temporary or the managed GitHub file. Callers own the path's authority.
    async fn remove_owned_file(
        &self,
        target: &ProvisioningTarget,
        path: &Path,
    ) -> Result<(), BackendFailure> {
        match target {
            ProvisioningTarget::Local => match tokio::fs::remove_file(path).await {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(BackendFailure::new(
                    format!("removing owned file {}", path.display()),
                    error.to_string(),
                )),
            },
            ProvisioningTarget::Ssh { .. } => {
                self.require_shell(
                    target,
                    &format!("rm -f -- {}", shell_path(path)?),
                    "removing the remote owned file",
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
            refuse_setup_managed,
            expected,
        } = install;
        self.cleanup_orphaned_temporaries(target, destination)
            .await?;
        if expected.is_none()
            && let Some(installed) = self.metadata_on_target(target, destination).await?
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

        self.remove_owned_file(target, temporary).await?;
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
                    if let Some(expected) = expected {
                        // Absence includes symlinks: a dangling link is still an editor's
                        // destination, while unreadable metadata remains an ordinary error.
                        let present = match tokio::fs::symlink_metadata(destination).await {
                            Ok(_) => true,
                            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
                            Err(error) => {
                                return Err(BackendFailure::new(
                                    "inspecting settings destination",
                                    error.to_string(),
                                ));
                            }
                        };
                        let current = self.metadata_on_target(target, destination).await?;
                        if current.as_ref().map(|file| file.hash.as_str()) != expected
                            || (expected.is_none() && present)
                        {
                            return Err(BackendFailure::new(DESTINATION_CHANGED, ""));
                        }
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
                access,
            } => {
                async {
                    self.ssh_put(ssh_destination, *access, source, temporary)
                        .await?;
                    // Same first-line test as `units::is_managed`, run in the
                    // command that renames so no plan can outlive it.
                    // `head -n 1` for the same reasons the reach check uses
                    // it: a marker line without a newline still counts, and
                    // an unreadable destination refuses rather than passes.
                    let ownership_guard = if refuse_setup_managed {
                        format!(
                            "if [ -e {dest} ] || [ -L {dest} ]; then \
                               first_line=$(head -n 1 -- {dest}) || exit 78; \
                               if [ \"$first_line\" = {marker} ]; then \
                                 printf '%s\\n' 'refusing to replace a unit managed by farhelm helm setup' >&2; \
                                 exit 77; \
                               fi; \
                             fi; ",
                            dest = shell_path(destination)?,
                            marker = crate::ssh::shell_quote(crate::units::MANAGED_MARKER),
                        )
                    } else {
                        String::new()
                    };
                    // Status 79 is reserved for a generation mismatch. Hash/read failures
                    // use 78 so delivery never retries an ordinary transport or I/O error.
                    let generation_guard = match expected {
                        None => String::new(),
                        Some(None) => format!("[ ! -e {dest} ] && [ ! -L {dest} ] || exit 79; ", dest = shell_path(destination)?),
                        Some(Some(hash)) => format!(
                            "if [ ! -e {dest} ] && [ ! -L {dest} ]; then exit 79; fi; \
                             old=$({sha}) || exit 78; [ \"${{old%% *}}\" = {hash} ] || exit 79; ",
                            dest = shell_path(destination)?, sha = remote_sha256sum(&shell_path(destination)?),
                            hash = crate::ssh::shell_quote(hash)),
                    };
                    // Keep the checksum input on stdin. GNU coreutils escapes
                    // backslashes and newlines in argument-based output; the
                    // stdin form follows install.sh's sha256_of convention.
                    let output = self.run_shell(
                        target,
                        &format!(
                            "{ownership_guard}actual=$({}) || exit; \
                             [ \"${{actual%% *}}\" = {} ] || {{ \
                               printf '%s\\n' 'uploaded payload digest mismatch' >&2; exit 76; \
                             }}; {generation_guard}chmod -- {mode:o} {} && mv -f -- {} {}",
                            remote_sha256sum(&shell_path(temporary)?),
                            crate::ssh::shell_quote(source_hash),
                            shell_path(temporary)?,
                            shell_path(temporary)?,
                            shell_path(destination)?
                        ),
                        COMMAND_TIMEOUT,
                    )
                    .await?;
                    if output.code != Some(0) {
                        return Err(BackendFailure::new(
                            if output.code == Some(79) && expected.is_some() {
                                DESTINATION_CHANGED.to_owned()
                            } else {
                                format!("finishing the atomic {description} install: {}", output.failure_status())
                            }, output.stderr));
                    }
                    Ok(())
                }
                .await
            }
        };
        if let Err(primary) = install {
            if let Err(cleanup) = self.remove_owned_file(target, temporary).await {
                return Err(BackendFailure::new(
                    format!("{}; temporary cleanup also failed", primary.context),
                    format!("{}; {}", primary.stderr, cleanup.rendered()),
                ));
            }
            return Err(primary);
        }
        Ok(ActionOutcome::Completed)
    }

    /// Upload one remote binary to the plan's temporary and verify its bytes.
    /// An already-correct destination skips network transfer; the following
    /// install action still owns mode repair and the final skip record.
    async fn upload_source(
        &self,
        target: &ProvisioningTarget,
        source: &Path,
        source_hash: &str,
        destination: &Path,
        temporary: &Path,
    ) -> Result<ActionOutcome, BackendFailure> {
        let ProvisioningTarget::Ssh {
            destination: ssh_destination,
            access,
        } = target
        else {
            return Err(BackendFailure::new(
                "uploading a local payload",
                "local plans have no upload action",
            ));
        };
        // Remote provisioning now reports upload and install separately. Sweep
        // abandoned nonce files before creating this run's temporary so the
        // cleanup cannot remove the verified upload that the following install
        // action must consume.
        self.cleanup_orphaned_temporaries(target, destination)
            .await?;
        if let Some(installed) = self.metadata_on_target(target, destination).await?
            && installed.hash == source_hash
        {
            self.remove_owned_file(target, temporary).await?;
            return Ok(ActionOutcome::Skipped(format!(
                "{} already has the requested payload",
                destination.display()
            )));
        }
        self.remove_owned_file(target, temporary).await?;
        let upload = async {
            self.ssh_put(ssh_destination, *access, source, temporary)
                .await?;
            self.require_shell(
                target,
                &format!(
                    "actual=$({}) || exit; \
                     [ \"${{actual%% *}}\" = {} ] || {{ \
                       printf '%s\\n' 'uploaded payload digest mismatch' >&2; exit 76; \
                     }}",
                    remote_sha256sum(&shell_path(temporary)?),
                    crate::ssh::shell_quote(source_hash)
                ),
                "verifying the uploaded payload",
            )
            .await?;
            Ok::<(), BackendFailure>(())
        }
        .await;
        if let Err(primary) = upload {
            return Err(self
                .cleanup_temporary_failure(target, temporary, primary)
                .await);
        }
        Ok(ActionOutcome::Completed)
    }

    /// Install only the verified remote temporary, without a second upload.
    /// Rechecking the digest at this boundary protects the bytes across the
    /// separately reported upload and install actions.
    pub(super) async fn install_uploaded_source(
        &self,
        target: &ProvisioningTarget,
        source_hash: &str,
        destination: &Path,
        temporary: &Path,
        mode: u32,
    ) -> Result<ActionOutcome, BackendFailure> {
        // Every failure from here on happens with the uploaded temporary
        // (a complete payload, tens of megabytes) already on the host, so it
        // goes through `cleanup_temporary_failure`: returning early would
        // leave that file behind in the host's farhelm or bin directory on
        // one transient ssh failure. That includes the already-installed
        // branch's own removal: its failure earns the same one further
        // cleanup attempt. The helper stays outside this block so a failed
        // cleanup is reported, not retried recursively.
        let already_installed = async {
            let Some(installed) = self.metadata_on_target(target, destination).await? else {
                return Ok(None);
            };
            if installed.hash != source_hash {
                return Ok(None);
            }
            let repaired = installed.mode != mode;
            if repaired {
                self.set_target_mode(target, destination, mode).await?;
            }
            self.remove_owned_file(target, temporary).await?;
            Ok::<_, BackendFailure>(Some(ActionOutcome::Skipped(format!(
                "{} already has the requested payload{}",
                destination.display(),
                if repaired {
                    format!("; repaired mode to {mode:04o}")
                } else {
                    String::new()
                }
            ))))
        };
        match already_installed.await {
            Ok(Some(outcome)) => return Ok(outcome),
            Ok(None) => {}
            Err(primary) => {
                return Err(self
                    .cleanup_temporary_failure(target, temporary, primary)
                    .await);
            }
        }
        let install = self
            .require_shell(
                target,
                &format!(
                    "actual=$({}) || exit; \
                     [ \"${{actual%% *}}\" = {} ] || {{ \
                       printf '%s\\n' 'uploaded payload digest mismatch' >&2; exit 76; \
                     }}; chmod -- {mode:o} {} && mv -f -- {} {}",
                    remote_sha256sum(&shell_path(temporary)?),
                    crate::ssh::shell_quote(source_hash),
                    shell_path(temporary)?,
                    shell_path(temporary)?,
                    shell_path(destination)?
                ),
                "finishing the atomic payload install",
            )
            .await;
        if let Err(primary) = install {
            return Err(self
                .cleanup_temporary_failure(target, temporary, primary)
                .await);
        }
        Ok(ActionOutcome::Completed)
    }

    /// Preserve the first failure while reporting a failed nonce cleanup too.
    async fn cleanup_temporary_failure(
        &self,
        target: &ProvisioningTarget,
        temporary: &Path,
        primary: BackendFailure,
    ) -> BackendFailure {
        match self.remove_owned_file(target, temporary).await {
            Ok(()) => primary,
            Err(cleanup) => BackendFailure::new(
                format!("{}; temporary cleanup also failed", primary.context),
                format!("{}; {}", primary.stderr, cleanup.rendered()),
            ),
        }
    }

    /// Stream one local file to `remote` over the same `ssh` command every
    /// other provisioning step uses, by running `cat > <remote>` there with
    /// the file on its stdin. The same session reports the temporary file's
    /// size, so supervising an upload never needs a second remote command.
    ///
    /// This replaced an `sftp` upload. sftp parses its destination with its
    /// own grammar (`[user@]host[:path]`, the first colon ending the host), so
    /// an IPv6-literal or `ssh://user@host:port` registration dialed a
    /// different host for this one step than ssh did for all the others. One
    /// command builder means one host, and the remote side needs nothing
    /// beyond the `sh`, `cat`, `stat`, `sleep` and `kill` used here
    /// (no sftp subsystem). The pipe is byte-exact because the helm's ssh never
    /// requests a terminal.
    ///
    /// A successful return does not prove the bytes arrived intact: a local
    /// read that ends early looks like EOF to `cat`. The caller's digest check
    /// on the remote temporary is what proves the transfer. The remote file's
    /// byte growth is the only progress signal, reported on upload stdout.
    /// Local writes into ssh can run ahead of the host and cannot renew the
    /// deadline, including while the final buffered bytes drain remotely.
    async fn ssh_put(
        &self,
        destination: &str,
        access: crate::ssh::SshAccessKind,
        source: &Path,
        remote: &Path,
    ) -> Result<(), BackendFailure> {
        let source_bytes = tokio::fs::metadata(source)
            .await
            .map_err(|error| BackendFailure::new("reading staged payload size", error.to_string()))?
            .len();
        let mut command = self
            .ssh_command(destination, access, upload_script(remote)?)
            .map_err(|error| {
                BackendFailure::new("building the payload transfer command", error.to_string())
            })?;
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        isolate_process_group(&mut command);
        let mut input = tokio::fs::File::open(source).await.map_err(|error| {
            BackendFailure::new("opening the staged payload", error.to_string())
        })?;
        let mut child = self.launcher.spawn(&mut command).map_err(|error| {
            BackendFailure::new("spawning the payload transfer", error.to_string())
        })?;
        let mut stdin = child.stdin.take().expect("piped transfer stdin");
        // Feed stdin from its own task so the supervision loop below can watch
        // the remote file grow (and kill a stalled transfer) while the copy is
        // blocked on a full pipe. Dropping `stdin` at the end is the EOF that
        // lets the remote `cat` finish.
        let feeder = tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            let copied = tokio::io::copy(&mut input, &mut stdin).await;
            let flushed = stdin.shutdown().await;
            copied.and(flushed)
        });
        let output = capture_transfer_child(
            child,
            source_bytes,
            TRANSFER_IDLE_TIMEOUT,
            Some(feeder.abort_handle()),
        )
        .await;
        // Unless the copy succeeded, nothing will read the rest of the input,
        // and the feeder's write fails only once every holder of the pipe's
        // read end is gone. With OpenSSH that happens as soon as the ssh
        // process exits or is killed, shared connection or not (see
        // `ssh_stand_in`), so this cancel is defensive: a process that keeps
        // the pipe without reading it must not make the join below wait
        // forever. The capture's own error, or the exit status, is what gets
        // reported.
        if !matches!(&output, Ok(result) if result.code == Some(0)) {
            feeder.abort();
        }
        let fed = feeder.await;
        let output = output?;
        if output.code != Some(0) {
            return Err(BackendFailure::new(
                format!("transferring {} over ssh", source.display()),
                output.stderr,
            ));
        }
        match fed {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(error)) => Err(BackendFailure::new(
                format!("streaming {} to the host", source.display()),
                error.to_string(),
            )),
            Err(error) => Err(BackendFailure::new(
                "joining the payload transfer feeder",
                error.to_string(),
            )),
        }
    }

    /// The remote shell script an ssh probe runs: find the farhelm binary and
    /// exec its stdio proxy, or exit with the positive-absence status.
    ///
    /// `farhelm` and `state` are already shell-quoted. A bare `farhelm` is
    /// looked up on the remote PATH and then in Farhelm's two standard install
    /// locations, because a non-interactive ssh command does not read the
    /// login profile that usually puts `~/.local/bin` on PATH: the private
    /// ADD layout (`~/.local/lib/farhelm`) and the install script's
    /// `~/.local/bin`. Missing the latter reported a host that runs Farhelm
    /// from the standard installer as empty, and offered to install a second
    /// copy over it.
    pub(super) fn probe_script(farhelm: &str, state: Option<&str>) -> String {
        let state_arg = state
            .map(|path| format!(" --state-dir {path}"))
            .unwrap_or_default();
        format!(
            "printf '%s\\n' {marker} >&2; resolved=''; \
             if command -v {farhelm} >/dev/null 2>&1; then resolved=$(command -v {farhelm}); \
             elif [ {farhelm} = farhelm ] && [ -x \"$HOME/.local/lib/farhelm/farhelm\" ]; \
             then resolved=\"$HOME/.local/lib/farhelm/farhelm\"; \
             elif [ {farhelm} = farhelm ] && [ -x \"$HOME/.local/bin/farhelm\" ]; \
             then resolved=\"$HOME/.local/bin/farhelm\"; fi; \
             if [ -n \"$resolved\" ]; then printf '%s%s\\n' {resolved_prefix} \"$resolved\" >&2; \
             exec \"$resolved\" internal stdio{state_arg}; fi; exit {POSITIVE_ABSENCE_EXIT}",
            marker = crate::ssh::shell_quote(REMOTE_PROBE_MARKER),
            resolved_prefix = crate::ssh::shell_quote(REMOTE_RESOLVED_PREFIX),
        )
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
            ProvisioningTarget::Ssh {
                destination,
                access,
            } => {
                let farhelm = shell_path(&target.probe_farhelm)?;
                let state = target
                    .probe_state_dir
                    .as_ref()
                    .map(|path| shell_path(path))
                    .transpose()?;
                let script = Self::probe_script(&farhelm, state.as_deref());
                self.ssh_command(destination, *access, script)
                    .map_err(|error| {
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

/// Tensorlake's gateway does not run PAM. Ordinary ssh hosts benefit from
/// the same fallback; an existing runtime setting is always authoritative.
pub(super) const REMOTE_RUNTIME_PREFIX: &str = "if [ -z \"${XDG_RUNTIME_DIR-}\" ] && [ -d \"/run/user/$(id -u)\" ]; then XDG_RUNTIME_DIR=\"/run/user/$(id -u)\"; export XDG_RUNTIME_DIR; fi; ";

#[path = "tensorlake_backend.rs"]
mod tensorlake_backend;

/// Render the bounded remote sweep used for one destination directory.
pub(super) fn orphan_cleanup_script(parent: &Path, prefix: &str) -> anyhow::Result<String> {
    let parent = shell_path(parent)?;
    let prefix = crate::ssh::shell_quote(prefix);
    Ok(format!(
        "prefix={prefix}; for path in {parent}/\"$prefix\"*; do [ -f \"$path\" ] && [ ! -L \"$path\" ] || continue; base=${{path##*/}}; suffix=${{base#\"$prefix\"}}; printf '%s' \"$suffix\" | grep -Eq '^[0-9a-f]{{8}}-[0-9a-f]{{4}}-[0-9a-f]{{4}}-[0-9a-f]{{4}}-[0-9a-f]{{12}}$' || continue; rm -f -- \"$path\" || exit 1; done",
    ))
}

/// Bounded captured result of one local or remote host shell.
#[derive(Debug)]
pub(crate) struct CommandResult {
    pub(crate) code: Option<i32>,
    pub(crate) signal: Option<i32>,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: String,
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
        hash: lower_hex(&digest.finalize()),
    })
}

/// Final and temporary coordinates plus convergence policy for one artifact.
struct InstallDestination<'a> {
    path: &'a Path,
    temporary: &'a Path,
    mode: u32,
    description: &'a str,
    /// Refuse, on a remote host, to replace a destination whose first line is
    /// `farhelm helm setup`'s managed-by marker. Only the supervisor unit sets
    /// it. The reach check already refuses such a unit at planning time; this
    /// covers setup running on the host between planning and confirmation.
    /// The local branch has no such check because production never installs
    /// units locally (the panel hands the helm's own machine to setup).
    refuse_setup_managed: bool,
    /// Same three-state contract as `ProvisioningBackend::install_bytes_checked`.
    expected: Option<Option<&'a str>>,
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
#[derive(Default, Clone)]
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
/// long SSH banner falls relative to the retained prefix. Production probes
/// read through [`ProbeStderrReader`], which also keeps partial progress.
#[cfg(test)]
pub(super) async fn drain_probe_stderr<R>(
    stream: R,
    signal: tokio::sync::mpsc::UnboundedSender<DrainFailure>,
) -> ProbeStderr
where
    R: tokio::io::AsyncRead + Unpin,
{
    drain_probe_stderr_into(stream, signal, &std::sync::Mutex::default()).await
}

/// [`drain_probe_stderr`], also publishing everything recognized so far to
/// `progress` after each read, so an owner that stops waiting before EOF
/// still has the records that already arrived (see [`ProbeStderrReader`]).
async fn drain_probe_stderr_into<R>(
    mut stream: R,
    signal: tokio::sync::mpsc::UnboundedSender<DrainFailure>,
    progress: &std::sync::Mutex<ProbeStderr>,
) -> ProbeStderr
where
    R: tokio::io::AsyncRead + Unpin,
{
    let publish = |result: &ProbeStderr| {
        progress
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone_from(result);
    };
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
                publish(&result);
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
/// Output readers then get [`CAPTURE_PIPE_GRACE`]; after a normal exit, a
/// helper still holding a pipe when it expires is not killed.
/// The caller must isolate the command with [`isolate_process_group`] before
/// spawning so descendants cannot outlive a timed-out shell.
pub(crate) async fn capture_child(
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
                let _ = join_orphaned_readers([stdout_task, stderr_task]).await;
                return Err(BackendFailure::new(
                    format!("waiting for {context}"),
                    error.to_string(),
                ));
            }
        },
        failure = signal_rx.recv() => {
            let failure = failure.expect("stream drain signal sender disappeared");
            terminate_child(&mut child).await;
            let _ = join_orphaned_readers([stdout_task, stderr_task]).await;
            return Err(BackendFailure::new(
                format!("reading {context} {} ({})", failure.stream, failure.detail),
                String::from_utf8_lossy(&failure.prefix),
            ));
        }
        _ = tokio::time::sleep_until(deadline) => {
            terminate_child(&mut child).await;
            let _ = join_orphaned_readers([stdout_task, stderr_task]).await;
            return Err(BackendFailure::new(format!("{context} timed out"), ""));
        }
    };
    finish_child_output(status, stdout_task, stderr_task, &mut signal_rx, context).await
}

/// Upload through foreground `cat`, with file-size reports on the same SSH session.
///
/// A background `cat` loses its stdin under noninteractive POSIX shells, so only
/// the reporter runs in the background. Its sleeping child must inherit no SSH
/// output pipe: stopping the reporter then lets small uploads close immediately,
/// rather than waiting for the polling interval before a later command can run.
/// The final report covers bytes received after the last periodic sample. The
/// caller still checks the file's digest; size reports prove progress, not content.
/// The temporary is private from its first redirection, including in existing
/// shared directories: this path also carries agent tokens. The finishing
/// command applies the requested final mode for binaries and unit files.
pub(super) fn upload_script(remote: &Path) -> Result<String, BackendFailure> {
    let path = shell_path(remote)?;
    let interval = TRANSFER_PROGRESS_POLL.as_secs();
    Ok(format!(
        "case $(uname -s) in Darwin) size_format='-f %z' ;; *) size_format='-c %s' ;; esac; \
         report_size() {{ if test -e {path}; then stat $size_format -- {path} 2>/dev/null; fi; }}; \
         (while :; do report_size; sleep {interval} >/dev/null 2>&1; done) & \
         reporter=$!; \
         trap 'kill \"$reporter\" 2>/dev/null; wait \"$reporter\" 2>/dev/null' 0; \
         umask 077; cat > {path}; result=$?; report_size; exit \"$result\""
    ))
}

/// Retain only the latest complete size report while continuously draining stdout.
///
/// Invalid or overlong lines cannot arm or renew the stall deadline. A bounded
/// partial line also keeps a peer that never writes a newline from growing helm
/// memory indefinitely. A watch channel coalesces valid reports instead of queuing
/// one allocation per sample while the supervisor is busy elsewhere.
pub(super) async fn drain_transfer_sizes<R>(
    mut stream: R,
    sizes: tokio::sync::watch::Sender<Option<u64>>,
    signal: tokio::sync::mpsc::UnboundedSender<DrainFailure>,
) -> Vec<u8>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut buffer = [0_u8; 4096];
    let mut line = Vec::new();
    let mut overflow = false;
    loop {
        match stream.read(&mut buffer).await {
            Ok(0) => return Vec::new(),
            Ok(read) => {
                for byte in &buffer[..read] {
                    if *byte == b'\n' {
                        if !overflow
                            && let Some(bytes) = std::str::from_utf8(&line)
                                .ok()
                                .and_then(|text| text.trim().parse::<u64>().ok())
                        {
                            sizes.send_replace(Some(bytes));
                        }
                        line.clear();
                        overflow = false;
                    } else if line.len() < MAX_CHILD_STREAM_BYTES {
                        line.push(*byte);
                    } else {
                        overflow = true;
                    }
                }
            }
            Err(error) => {
                let _ = signal.send(DrainFailure {
                    stream: "stdout",
                    detail: error.to_string(),
                    prefix: line,
                });
                return Vec::new();
            }
        }
    }
}

/// Supervise remote-file growth without opening another command or blocking exit checks.
///
/// No transfer deadline runs before a complete size report proves the temporary
/// exists. Thereafter only growth within the expected source size renews it;
/// malformed output and local feeder activity do not. Closing stdout does not
/// stop the deadline: ssh may still be draining buffered input or waiting for the
/// remote command after the helm has written its final byte.
///
/// Every failure kills the child and then joins its stream readers within
/// [`CAPTURE_PIPE_GRACE`], like a normal exit's. That bound is what keeps a
/// stalled transfer from hanging (#3006): over a shared ssh connection the
/// master holds the session's stderr after the ssh process is killed, until
/// the remote command ends, and a stalled remote command may never end.
///
/// `input` is the task feeding the child's stdin, if one does, and every
/// failure cancels it first. This is defensive rather than load-bearing: as
/// measured (see `ssh_stand_in`), killing the ssh process already closes the
/// remote command's stdin and makes the feeder's next write fail.
pub(super) async fn capture_transfer_child(
    mut child: tokio::process::Child,
    source_bytes: u64,
    idle_timeout: Duration,
    input: Option<tokio::task::AbortHandle>,
) -> Result<CommandResult, BackendFailure> {
    let context = "the payload transfer";
    let stdout = child.stdout.take().expect("captured transfer stdout");
    let stderr = child.stderr.take().expect("captured transfer stderr");
    let (signal_tx, mut signal_rx) = tokio::sync::mpsc::unbounded_channel();
    let _signal_guard = signal_tx.clone();
    let (sizes_tx, mut sizes_rx) = tokio::sync::watch::channel(None);
    let stdout_task = tokio::spawn(drain_transfer_sizes(stdout, sizes_tx, signal_tx.clone()));
    let mut sizes_open = true;
    let stderr_task = tokio::spawn(drain_capped(stderr, "stderr", signal_tx));
    let mut deadline: Option<tokio::time::Instant> = None;
    let mut observed_bytes = 0;
    let status = loop {
        // Prefer an already completed child to a deadline that expired in the
        // same scheduling turn, as the ordinary command supervisor does.
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                stop_transfer(&mut child, stdout_task, stderr_task, input.as_ref()).await;
                return Err(BackendFailure::new(
                    format!("waiting for {context}"),
                    error.to_string(),
                ));
            }
        }
        if deadline.is_some_and(|at| tokio::time::Instant::now() >= at) {
            stop_transfer(&mut child, stdout_task, stderr_task, input.as_ref()).await;
            return Err(BackendFailure::new(format!("{context} stalled"), ""));
        }
        tokio::select! {
            result = child.wait() => match result {
                Ok(status) => break status,
                Err(error) => {
                    stop_transfer(&mut child, stdout_task, stderr_task, input.as_ref()).await;
                    return Err(BackendFailure::new(format!("waiting for {context}"), error.to_string()));
                }
            },
            failure = signal_rx.recv() => {
                let failure = failure.expect("stream drain signal sender disappeared");
                stop_transfer(&mut child, stdout_task, stderr_task, input.as_ref()).await;
                return Err(BackendFailure::new(
                    format!("reading {context} {} ({})", failure.stream, failure.detail),
                    String::from_utf8_lossy(&failure.prefix),
                ));
            },
            _ = async {
                if let Some(at) = deadline {
                    tokio::time::sleep_until(at).await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {
                stop_transfer(&mut child, stdout_task, stderr_task, input.as_ref()).await;
                return Err(BackendFailure::new(format!("{context} stalled"), ""));
            },
            changed = sizes_rx.changed(), if sizes_open => {
                if changed.is_err() {
                    sizes_open = false;
                    continue;
                }
                let bytes = *sizes_rx.borrow_and_update();
                if let Some(bytes) = bytes {
                    deadline.get_or_insert_with(|| tokio::time::Instant::now() + idle_timeout);
                    if bytes > observed_bytes && bytes <= source_bytes {
                        observed_bytes = bytes;
                        deadline = Some(tokio::time::Instant::now() + idle_timeout);
                    }
                }
            },
        }
    };
    finish_child_output(status, stdout_task, stderr_task, &mut signal_rx, context).await
}

/// End a failed transfer: cancel the feeder, kill the child's group, and join
/// its readers within the shared grace, so a pipe that stays open cannot hang
/// the failure. See [`capture_transfer_child`] for which part matters.
async fn stop_transfer(
    child: &mut tokio::process::Child,
    stdout_task: tokio::task::JoinHandle<Vec<u8>>,
    stderr_task: tokio::task::JoinHandle<Vec<u8>>,
    input: Option<&tokio::task::AbortHandle>,
) {
    if let Some(input) = input {
        input.abort();
    }
    terminate_child(child).await;
    let _ = join_orphaned_readers([stdout_task, stderr_task]).await;
}

/// Interpret the same bounded stream drains for fixed-deadline commands and
/// progress-supervised transfers, so neither path can hide an overflow.
async fn finish_child_output(
    status: std::process::ExitStatus,
    stdout_task: tokio::task::JoinHandle<Vec<u8>>,
    stderr_task: tokio::task::JoinHandle<Vec<u8>>,
    signal_rx: &mut tokio::sync::mpsc::UnboundedReceiver<DrainFailure>,
    context: &str,
) -> Result<CommandResult, BackendFailure> {
    // After a normal exit only a lingering holder's bytes can be cut off, but a
    // surfaced error beats silently missing stderr (the probe carries on instead).
    let [stdout, stderr] = join_orphaned_readers([stdout_task, stderr_task]).await;
    let failed = |stream, detail| format!("reading {context} {stream} ({detail}; {status})");
    let stdout = stdout.map_err(|detail| BackendFailure::new(failed("stdout", detail), ""))?;
    let stderr = stderr.map_err(|detail| BackendFailure::new(failed("stderr", detail), ""))?;
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

/// Join a gone child's stdout and stderr readers within one shared
/// [`CAPTURE_PIPE_GRACE`], aborting any reader still waiting for EOF so it
/// releases the pipe instead of waiting on as a detached task.
async fn join_orphaned_readers(
    readers: [tokio::task::JoinHandle<Vec<u8>>; 2],
) -> [Result<Vec<u8>, String>; 2] {
    let until = tokio::time::Instant::now() + CAPTURE_PIPE_GRACE;
    let mut joined = [Ok(Vec::new()), Ok(Vec::new())];
    for (slot, mut reader) in joined.iter_mut().zip(readers) {
        *slot = match tokio::time::timeout_at(until, &mut reader).await {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(_) => Err("still open after the command exited".to_string()),
        };
        reader.abort(); // Harmless once joined.
    }
    joined
}

/// Kill a child and every helper in its isolated process group, then reap
/// the direct child before releasing a provisioning lock.
pub(super) async fn terminate_child(child: &mut tokio::process::Child) {
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

/// A probe's stderr reader together with what it has recognized so far.
///
/// The probe's records (the wrapper's marker and its resolved binary) arrive
/// before the handshake, but stderr can stay open long after the probe is
/// stopped: a shared ssh connection's master holds the session's stderr until
/// the remote command ends, and an escaped helper can hold it indefinitely.
/// Waiting for EOF is therefore bounded, and an expired bound must not throw
/// away records that were read long before it. Losing them made a remote
/// probe whose remote command outlived the bound report "answered without
/// reporting its resolved binary", which refused remote uninstall outright
/// (#3005).
struct ProbeStderrReader {
    task: tokio::task::JoinHandle<ProbeStderr>,
    progress: Arc<std::sync::Mutex<ProbeStderr>>,
}

impl ProbeStderrReader {
    fn spawn<R>(stream: R, signal: tokio::sync::mpsc::UnboundedSender<DrainFailure>) -> Self
    where
        R: tokio::io::AsyncRead + Unpin + Send + 'static,
    {
        let progress = Arc::new(std::sync::Mutex::new(ProbeStderr::default()));
        let shared = Arc::clone(&progress);
        let task =
            tokio::spawn(async move { drain_probe_stderr_into(stream, signal, &shared).await });
        Self { task, progress }
    }

    /// Wait up to `bound` for stderr EOF. If it does not come, cancel the
    /// reader (it must not outlive the probe) and return what it had read.
    async fn finish(mut self, bound: Duration) -> ProbeStderr {
        let drained = tokio::time::timeout(bound, &mut self.task).await;
        // Aborting a joined task is harmless.
        self.task.abort();
        match drained {
            Ok(Ok(complete)) => complete,
            _ => self
                .progress
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
        }
    }
}

/// Terminate a protocol probe and retain its bounded stderr prefix.
///
/// Probe stdout belongs to the framed handshake, so this is the common cleanup
/// half for the independently drained diagnostic stream. Over a shared ssh
/// connection, stderr reaches EOF only once the remote command is done.
/// Killing the ssh process closes the remote command's stdin and stdout, so a
/// remote stdio proxy then ends on the EOF or on its next write, but how soon
/// that happens is up to the remote side. The two-second bound keeps a slow
/// remote command, or an escaped helper, from retaining a host planning slot;
/// see [`ProbeStderrReader`] for why expiry keeps what was read.
async fn stop_probe(child: &mut tokio::process::Child, stderr: ProbeStderrReader) -> ProbeStderr {
    terminate_child(child).await;
    stderr.finish(Duration::from_secs(2)).await
}

/// Only transport failures are eligible for positive-absence exit
/// classification. A decoded but malformed protocol reply remains a protocol
/// error even if the child happens to exit with the reserved status later.
fn handshake_io_failure(error: &std::io::Error) -> bool {
    ClosedBeforeHello::is_cause_of(error) || error.kind() != std::io::ErrorKind::InvalidData
}

#[async_trait]
impl ProvisioningBackend for SystemBackend {
    async fn remove_github_token(
        &self,
        target: &ProvisioningTarget,
        path: &Path,
    ) -> Result<(), BackendFailure> {
        self.remove_owned_file(target, path).await
    }

    async fn setup_github_git(&self, target: &ProvisioningTarget) -> Result<bool, BackendFailure> {
        let output = self.run_shell(target,
            "command -v gh >/dev/null 2>&1 || exit 42\ngh auth setup-git --force --hostname github.com >/dev/null 2>&1",
            COMMAND_TIMEOUT).await?;
        match output.code {
            Some(0) => Ok(true),
            Some(42) => Ok(false),
            _ => Err(BackendFailure::new(
                "could not configure GitHub's git helper",
                "",
            )),
        }
    }
    async fn host_home(&self, target: &ProvisioningTarget) -> Result<PathBuf, BackendFailure> {
        let failure = || BackendFailure::new("could not read the sandbox home", "");
        let output = self
            .run_shell(target, "printf '%s' \"$HOME\"", COMMAND_TIMEOUT)
            .await
            .map_err(|_| failure())?;
        if output.code != Some(0) {
            return Err(failure());
        }
        let home = PathBuf::from(String::from_utf8(output.stdout).map_err(|_| failure())?);
        if !home.is_absolute() {
            return Err(failure());
        }
        Ok(home)
    }
    async fn wait_tensorlake_ssh(&self, target: &ProvisioningTarget) -> Result<(), BackendFailure> {
        self.wait_sandbox_ssh(target).await
    }

    async fn prepare_tensorlake_sandbox(
        &self,
        target: &ProvisioningTarget,
    ) -> Result<PathBuf, BackendFailure> {
        self.prepare_sandbox(target).await
    }

    async fn read_small_file(
        &self,
        target: &ProvisioningTarget,
        path: &Path,
    ) -> Result<Option<Vec<u8>>, BackendFailure> {
        self.read_agent_file(target, path).await
    }

    async fn refuse_copied_supervisor(
        &self,
        target: &ProvisioningTarget,
        state_dir: &Path,
    ) -> Result<(), BackendFailure> {
        self.check_copied_state(target, state_dir).await
    }

    async fn finish_snapshot_install(
        &self,
        target: &ProvisioningTarget,
        unit: &str,
        state_dir: &Path,
    ) -> Result<(), BackendFailure> {
        self.finish_prepared_install(target, unit, state_dir).await
    }
    async fn probe(&self, target: &ProbeTarget) -> Result<ProbeObservation, BackendFailure> {
        let (mut child, remote) = self.spawn_probe(target).await?;
        // The probe's process group, kept for the path below where the child
        // has already been reaped and `child.id()` no longer answers.
        let group = child.id();
        let stdout = child.stdout.take().expect("piped probe stdout");
        let stdin = child.stdin.take().expect("piped probe stdin");
        let stderr = child.stderr.take().expect("piped probe stderr");
        let (stderr_signal_tx, mut stderr_signal_rx) = tokio::sync::mpsc::unbounded_channel();
        let _stderr_signal_guard = stderr_signal_tx.clone();
        let stderr_task = ProbeStderrReader::spawn(stderr, stderr_signal_tx);
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
                drop(writer);
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
        // Nothing is written after the hello, so stdin closes here. The
        // remote stdio proxy ends once it reads that EOF, but stopping the
        // probe delivers the same EOF to it, so this close does not decide
        // when a shared connection's master releases stderr (see
        // `stop_probe`).
        drop(writer);
        match handshake_result {
            Ok(Ok(ControlMsg::Hello {
                build_version,
                host_identity,
                database_schema,
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
                    database_schema,
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
                let skew =
                    VersionSkew::cause_of(&error).expect("guard established the skew payload");
                let (peer_build, database_schema) =
                    (skew.peer_build.clone(), skew.peer_database_schema);
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
                    database_schema,
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
                // The child is gone but a helper it started (an ssh
                // `ProxyCommand`, say) may not be, and it would hold the
                // stderr pipe the wait below needs closed: kill the rest of
                // the group now. The group id cannot have been reused while
                // any such member is alive, which is the only case the kill
                // is for.
                #[cfg(unix)]
                if let Some(pid) = group {
                    // SAFETY: `spawn_probe` isolated the child in its own
                    // process group, whose id is the child's pid.
                    unsafe {
                        libc::kill(-(pid as i32), libc::SIGKILL);
                    }
                }
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
                // Bounded even so, because this probe runs on a helm-owned
                // task that nothing cancels: a helper that left the group (one
                // that started its own session or process group) could
                // otherwise hold stderr open, just as in `stop_probe`.
                let stderr = stderr_task.finish(Duration::from_secs(2)).await;
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
        // The Linux inspection itself runs systemd and tmux commands. Choose
        // the OS before either, so remote Macs never probe those substrates.
        let system = self
            .require_shell(
                &target.transport,
                "uname -s",
                "identifying the host operating system",
            )
            .await?;
        match system.stdout.as_slice() {
            b"Linux\n" | b"Linux" => {},
            b"Darwin\n" | b"Darwin" => {
                let output = self.require_shell(&target.transport, MACOS_REACH_SH, "inspecting the Mac host").await?;
                return parse_macos_reach_output(&output.stdout);
            },
            _ => return Ok(ReachOutcome::Manual("automatic provisioning does not support this host operating system; run the supervisor manually".into())),
        }
        // Unit discovery must read the user manager's environment, not only
        // the login shell's. Those environments can disagree about
        // XDG_CONFIG_HOME, and writing to the shell's directory would leave
        // a valid-looking unit that this manager never searches.
        //
        // The environment is fetched by a first command and read here in
        // Rust (`manager_unit_dir_choice`), not with `sed` in the script
        // below: `show-environment` escapes values that need it, so a config
        // directory with a space came back as `$'…'` and was refused as
        // relative. The reach script then gets the chosen directory as a
        // quoted literal, and an empty one means `$HOME/.config/systemd/user`
        // with the shell's own HOME, as before.
        let (manager, unit_dir) = match self.manager_unit_dir_choice(target).await? {
            ManagerUnitDir::Unavailable => ("unavailable", String::new()),
            ManagerUnitDir::RelativeXdg => ("unsupported-xdg", String::new()),
            ManagerUnitDir::UnreadableXdg => {
                return Ok(ReachOutcome::Manual(
                    "the systemd user manager reports XDG_CONFIG_HOME in an escaped form, and \
                     busctl could not read it from the user bus, so Farhelm cannot determine its \
                     unit directory; run the supervisor manually on this host."
                        .to_string(),
                ));
            }
            ManagerUnitDir::Xdg(dir) => ("usable", dir),
            ManagerUnitDir::HomeDefault => ("usable", String::new()),
        };
        //
        // The last field reports whether that directory already holds a
        // supervisor unit `farhelm helm setup` wrote on the host itself
        // (first line exactly `units::MANAGED_MARKER`, the same test as
        // `units::is_managed`). Such a unit belongs to setup there, and ADD
        // and UPDATE refuse to replace it — see `parse_reach_output`. An
        // unmarked unit is provisioning's own, or a hand-written one the
        // user is expected to move aside, and is replaced as before.
        //
        // `head -n 1` rather than `read`: `read` fails on a last line with
        // no newline, and a marker-only file must still count as setup's.
        // A unit that exists but cannot be read fails the whole check
        // instead of passing as unmarked, because "could not tell" is not
        // evidence that provisioning owns it.
        let script = format!(
            "if [ -r /etc/os-release ]; then . /etc/os-release; fi; \
                      printf '%s\\0%s\\0%s\\0' 'farhelm-reach-v1' \"${{ID-}}\" \"${{HOME-}}\"; \
                      uname -m | tr -d '\\n'; printf '\\0'; \
                      if command -v tmux >/dev/null 2>&1; then command -v tmux | tr -d '\\n'; fi; \
                      printf '\\0'; \
                      if command -v tmux >/dev/null 2>&1; then tmux -V | tr -d '\\n'; fi; \
                      printf '\\0'; \
                      manager={manager}; unit_dir={unit_dir}; \
                      if [ \"$manager\" = usable ] && [ -z \"$unit_dir\" ]; then \
                        unit_dir=$HOME/.config/systemd/user; fi; \
                      printf '%s\\0%s\\0' \"$manager\" \"$unit_dir\"; \
                      unit_owner=''; unit_file=\"$unit_dir\"/{unit}; \
                      if [ -n \"$unit_dir\" ] && {{ [ -e \"$unit_file\" ] || [ -L \"$unit_file\" ]; }}; then \
                        first_line=$(head -n 1 -- \"$unit_file\") || {{ \
                          printf '%s\\n' \"cannot read $unit_file to check whether farhelm helm setup manages it\" >&2; \
                          exit 78; }}; \
                        if [ \"$first_line\" = {marker} ]; then unit_owner=setup; fi; \
                      fi; \
                      printf '%s\\0' \"$unit_owner\"",
            unit = crate::units::SUPERVISOR_UNIT_NAME,
            marker = crate::ssh::shell_quote(crate::units::MANAGED_MARKER),
            manager = manager,
            unit_dir = crate::ssh::shell_quote(&unit_dir),
        );
        let output = self
            .run_shell(&target.transport, &script, COMMAND_TIMEOUT)
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
                    // A shared directory that already exists is left exactly
                    // as it is; see `DirectorySpec`.
                    if directory.shared
                        && tokio::fs::metadata(&directory.path)
                            .await
                            .is_ok_and(|metadata| metadata.is_dir())
                    {
                        continue;
                    }
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
                // `install -d -m` chmods a directory that already exists, so a
                // shared one is only handed to it when it is missing.
                let commands = directories
                    .iter()
                    .map(|directory| {
                        let path = shell_path(&directory.path)?;
                        Ok(if directory.shared {
                            format!(
                                "{{ [ -d {path} ] || install -d -m {:o} -- {path}; }}",
                                directory.mode
                            )
                        } else {
                            format!("install -d -m {:o} -- {path}", directory.mode)
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .join(" && ");
                self.require_shell(target, &commands, "creating provisioning directories")
                    .await?;
            }
        }
        Ok(ActionOutcome::Completed)
    }

    async fn upload_path(
        &self,
        target: &ProvisioningTarget,
        _kind: PayloadKind,
        payload: &PreparedPayload,
        destination: &Path,
        temporary: &Path,
    ) -> Result<ActionOutcome, BackendFailure> {
        self.upload_source(
            target,
            payload.path(),
            &payload.hash,
            destination,
            temporary,
        )
        .await
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
        match target {
            ProvisioningTarget::Local => {
                self.install_source(
                    target,
                    payload.path(),
                    &payload.hash,
                    InstallDestination {
                        path: destination,
                        temporary,
                        mode,
                        description: "payload",
                        refuse_setup_managed: false,
                        expected: None,
                    },
                )
                .await
            }
            ProvisioningTarget::Ssh { .. } => {
                self.install_uploaded_source(target, &payload.hash, destination, temporary, mode)
                    .await
            }
        }
    }

    async fn install_bytes_checked(
        &self,
        target: &ProvisioningTarget,
        content: &[u8],
        destination: &Path,
        temporary: &Path,
        mode: u32,
        expected: Option<Option<&str>>,
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
                refuse_setup_managed: true,
                expected,
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
            crate::ssh::shell_quote(unit)
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
                let outcome = linger_failure_outcome(output.code, &output.stderr);
                if matches!(outcome, Ok(ActionOutcome::Degraded(_))) {
                    // The host's own words go to the log, not the step
                    // message: remote stderr is untrusted text, and the
                    // step only needs to say linger is off.
                    warn!(
                        code = ?output.code,
                        stderr = %linger_log_stderr(&output.stderr),
                        "enabling linger failed; the supervisor starts at login, not at boot"
                    );
                }
                outcome
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
        let unit = crate::ssh::shell_quote(unit);
        self.require_shell(
            target,
            &format!("systemctl --user restart -- {unit}"),
            "restarting the supervisor unit",
        )
        .await?;
        Ok(ActionOutcome::Completed)
    }

    async fn launch_agent(
        &self,
        target: &ProvisioningTarget,
        domain: &str,
        label: &str,
        plist: &Path,
        refresh: bool,
    ) -> Result<ActionOutcome, BackendFailure> {
        require_remote(target, "starting the launchd supervisor")?;
        let script = super::macos::start_script(domain, label, plist, refresh)?;
        self.require_shell(target, &script, "starting the launchd supervisor")
            .await?;
        Ok(ActionOutcome::Completed)
    }

    async fn stop_launch_agent(
        &self,
        target: &ProvisioningTarget,
        domain: &str,
        label: &str,
        farhelm: &Path,
        state: &Path,
    ) -> Result<ActionOutcome, BackendFailure> {
        require_remote(
            target,
            "stopping the launchd supervisor and its terminal hosts",
        )?;
        let script = super::macos::stop_script(domain, label, farhelm, state)?;
        self.require_shell_within(
            target,
            &script,
            "stopping the launchd supervisor and its terminal hosts",
            STOP_SUPERVISOR_TIMEOUT,
        )
        .await?;
        Ok(ActionOutcome::Completed)
    }

    /// Read-only, so it runs on either transport, though only UNINSTALL
    /// planning asks and that refuses the local row before it gets here.
    ///
    /// The output is NUL-separated: an existence flag, a symlink flag, a
    /// canonical path (empty when unresolvable) with a verified alternate
    /// home prefix normalized, that path before normalization, and an
    /// inside-`removed` flag, per asked path, then the default state
    /// directory and its five fields, then the unit's `ActiveState` and
    /// `FragmentPath`, and the fragment's five fields (no fragment prints
    /// two `0` flags, two empty paths and a `0` flag, through `%s` because
    /// a literal `\00` would read the second flag's digit as part of an
    /// octal escape), then the mount points at or below
    /// each asked path's two spellings ([`MOUNT_POINTS_SH`]), ended by an
    /// empty field. A failing `systemctl show` or unreadable mount table fails
    /// the whole command rather than reading as empty values. `readlink -f`
    /// rather than `realpath`: both GNU and BusyBox provide it.
    async fn inspect_uninstall(
        &self,
        target: &ProvisioningTarget,
        unit: &str,
        launchd_domain: Option<&str>,
        paths: &[&Path],
        removed: &Path,
    ) -> Result<UninstallInspection, BackendFailure> {
        // Only the home may have two names. For example, alex logs in with
        // /home/alex while systemd still loads its unit through
        // /srv/accounts/a1, a bind mount of that same home. readlink -f
        // leaves both names intact. The reach probe derives planned paths
        // from this same SSH login's HOME (or the manager's XDG_CONFIG_HOME),
        // so normalize every resolved path against that login home once.
        // This also catches a registered state directory spelled
        // /srv/accounts/a1/.local/lib/farhelm/data: without normalization,
        // the text containment check misses data inside the removed lib.
        //
        // Walk from the resolved path toward the root; the nearest home
        // identity wins if a home is itself mounted inside that home. Keep
        // the rewrite only if the TWO FULL PATHS are -ef. A non-recursive
        // home bind can expose different files below a nested mount: home
        // identity alone would then approve a unit uninstall won't remove.
        // Missing files and unsupported -ef tests retain today's spelling.
        // An absent/unresolvable home, or / as home, disables rewriting.
        //
        // Do not replace this with identity comparisons of arbitrary
        // folders or files. /srv/shared-units mounted at each account's
        // .config/systemd/user, or /opt/farhelm mounted at each account's
        // .local/lib/farhelm, must stay refused: removing them affects
        // everyone. Neither has a home ancestor to rewrite. A hard-linked
        // unit under .local/share/systemd/user remains a DIFFERENT path
        // after home normalization too; removing the planned unit would
        // leave that copy for systemd to start again. The comparisons in
        // service.rs and PlanLayout::plan_uninstall that accept a path
        // deliberately stay text comparisons over these normalized values,
        // not general identities; identity only ever adds a refusal (below).
        // The rewrite is per path, so a lib rewritten to the login home can
        // hold state left in the alias spelling (a nested mount the bind
        // lacks fails that state's -ef); the unnormalized spelling printed
        // beside each path lets planning still see that containment.
        //
        // Two spellings still miss the lib reached through one bind of the
        // home and the state through another, with a mount over a folder
        // above the install folder (~/install, say) visible through only
        // the home's own path: each keeps its bind's spelling, so neither
        // spelling of the state starts with either of the lib's, and no
        // mount lies under either lib spelling for MOUNT_POINTS_SH to
        // report, yet `rm -rf` of the lib reaches the data. So
        // each path also reports whether a folder on its unnormalized
        // resolution's way to the root is -ef the folder to be removed. That
        // identity only refuses (SPEC.md: uninstall "still refuses layouts it
        // cannot tell apart safely"); it never accepts.
        let mut script = format!("removed={}\n", shell_path(removed)?);
        script.push_str(
            r#"home=''
if [ -n "${HOME-}" ]; then
    home=$(readlink -f -- "$HOME" 2>/dev/null) || home=''
fi
resolve() {
    if [ -e "$1" ] || [ -L "$1" ]; then printf 1; else printf 0; fi
    printf '\0'
    if [ -L "$1" ]; then printf 1; else printf 0; fi
    printf '\0'
    resolved=$(readlink -f -- "$1" 2>/dev/null | tr -d '\n')
    raw=$resolved
    case "$home" in
        ''|/) ;;
        *)
            parent=$resolved
            suffix=''
            while [ -n "$parent" ]; do
                if [ "$parent" -ef "$home" ] 2>/dev/null; then
                    rewritten=$home$suffix
                    if [ "$rewritten" -ef "$resolved" ] 2>/dev/null; then
                        resolved=$rewritten
                    fi
                    break
                fi
                case "$parent" in /) break ;; esac
                suffix=/${parent##*/}$suffix
                case "$parent" in
                    */*) parent=${parent%/*} ;;
                    *) break ;;
                esac
                if [ -z "$parent" ]; then parent=/; fi
            done
            ;;
    esac
    under=0
    ancestor=$raw
    while [ -n "$ancestor" ]; do
        if [ "$ancestor" -ef "$removed" ] 2>/dev/null; then under=1; break; fi
        case "$ancestor" in
            /*/*) ancestor=${ancestor%/*} ;;
            /?*) ancestor=/ ;;
            *) break ;;
        esac
    done
    printf '%s\0%s\0%s\0' "$resolved" "$raw" "$under"
}
"#,
        );
        // Only mounts under each path's spellings (collected in "$@", empty
        // under `sh -c`) leave the host: a whole table can pass the 64 KiB cap.
        script.push_str(if launchd_domain.is_some() {
            super::macos::MAC_MOUNT_POINTS_SH
        } else {
            MOUNT_POINTS_SH
        });
        for path in paths {
            script.push_str(&format!("resolve {}; ", shell_path(path)?));
            script.push_str("set -- \"$@\" \"$resolved\" \"$raw\"; ");
        }
        let service_inspection = match launchd_domain {
            Some(domain) => super::macos::job_inspection_script(domain, unit)?,
            None => format!(
                "active=$(systemctl --user show -p ActiveState --value -- {unit}) || exit 1; \
                fragment=$(systemctl --user show -p FragmentPath --value -- {unit}) || exit 1;",
                unit = crate::ssh::shell_quote(unit)
            ),
        };
        script.push_str(&format!(
            "case \"${{XDG_STATE_HOME-}}\" in /*) state=\"$XDG_STATE_HOME/farhelm\" ;; \
               *) state=\"$HOME/.local/state/farhelm\" ;; esac; \
             printf '%s\\0' \"$state\"; resolve \"$state\"; \
             {service_inspection} \
             printf '%s\\0%s\\0' \"$active\" \"$fragment\"; \
             if [ -n \"$fragment\" ]; then resolve \"$fragment\"; else printf '%s\\0%s\\0\\0\\0%s\\0' 0 0 0; fi; \
             for p in \"$@\"; do if [ -n \"$p\" ]; then \
               mount_points \"$p\" '\\0' || exit 1; fi; done; printf '\\0'",
        ));
        let output = self
            .require_shell(target, &script, "inspecting the installation to remove")
            .await?;
        parse_uninstall_inspection(&output.stdout, paths).ok_or_else(|| {
            BackendFailure::new(
                "inspecting the installation to remove returned malformed output",
                String::from_utf8_lossy(&output.stdout),
            )
        })
    }

    async fn disable(
        &self,
        target: &ProvisioningTarget,
        unit: &str,
    ) -> Result<ActionOutcome, BackendFailure> {
        require_remote(target, "disabling the supervisor unit")?;
        // `--runtime` mirrors `enable_now`: the real-transport fixture links
        // its unit for this boot only, and only a runtime disable removes
        // that link. An implicit reload is harmless: uninstall deliberately
        // forgets process-only termination before stopping the unit. Its
        // explicit reload follows file removal for units still on disk.
        let runtime = if self.runtime_units { " --runtime" } else { "" };
        let unit = crate::ssh::shell_quote(unit);
        self.require_shell(
            target,
            &format!("systemctl --user{runtime} disable -- {unit}"),
            "disabling the supervisor unit",
        )
        .await?;
        Ok(ActionOutcome::Completed)
    }

    /// The marker test is the reach check's (first line exactly
    /// `units::MANAGED_MARKER`, read with `head` so a marker-only file with
    /// no newline still counts), repeated here in the same shell as the
    /// `rm`: setup may have taken the unit over since the plan was made, and
    /// a unit it cannot read fails the step instead of passing as unmarked.
    async fn remove_unit(
        &self,
        target: &ProvisioningTarget,
        destination: &Path,
    ) -> Result<ActionOutcome, BackendFailure> {
        require_remote(target, "removing the supervisor unit file")?;
        let script = format!(
            "f={file}; \
             if [ ! -e \"$f\" ] && [ ! -L \"$f\" ]; then printf absent; exit 0; fi; \
             first_line=$(head -n 1 -- \"$f\") || {{ \
               printf '%s\\n' \"cannot read $f to check whether farhelm helm setup manages it\" >&2; \
               exit 78; }}; \
             if [ \"$first_line\" = {marker} ]; then \
               printf '%s\\n' 'refusing to remove a unit managed by farhelm helm setup' >&2; \
               exit 79; fi; \
             rm -f -- \"$f\"",
            file = shell_path(destination)?,
            marker = crate::ssh::shell_quote(crate::units::MANAGED_MARKER),
        );
        let output = self
            .require_shell(target, &script, "removing the supervisor unit file")
            .await?;
        Ok(if output.stdout == b"absent" {
            ActionOutcome::Skipped(format!("{} was already gone", destination.display()))
        } else {
            ActionOutcome::Completed
        })
    }

    /// End the service, the state directory's terminal hosts and any tmux
    /// server that outlived its process-only policy. A retry may find the
    /// unit forgotten but its hosts or its tmux still alive, or all of them
    /// gone and the lib directory, with the installed `farhelm` and any
    /// private tmux client in it, already removed.
    async fn stop(
        &self,
        target: &ProvisioningTarget,
        unit: &str,
        farhelm_program: &Path,
        state_dir: &Path,
        tmux_program: &Path,
        tmux_socket: &Path,
    ) -> Result<ActionOutcome, BackendFailure> {
        const CONTEXT: &str = "stopping the supervisor, its terminal hosts and private tmux server";
        require_remote(target, CONTEXT)?;
        // Stop the supervisor before ending its hosts and its tmux server
        // directly. The daemon-reload before this step has already dropped
        // the removed unit's process-only stop policy, so this stop signals
        // the supervisor, its terminal hosts and the private tmux server in
        // its group together; the orderly terminal shutdown no longer runs
        // ahead of tmux here. The steps after it end what outlived the
        // supervisor, such as hosts and a server left running by a stop
        // under the old policy.
        //
        // Terminal hosts come first of those, through the installed
        // farhelm's `internal stop-terminal-hosts`, whose command line and
        // the meaning of its exit status 0 every release keeps (SPEC_impl.md,
        // "Terminal host"). That 0 is the only proof that every host is
        // stopped, so any other status, 2 included (a farhelm from before the
        // command), fails the step and leaves the host listed for a retry.
        // The command runs whenever the state directory has a `hosts`, of any
        // kind and whatever it holds: the command's contract has callers
        // decide by whether `hosts` exists and leaves judging its contents to
        // the command. Only a missing `hosts` (an install from before
        // terminal hosts, or one that never started any) skips it.
        //
        // That makes the retry after the lib removal, which deletes this
        // farhelm, depend on `hosts` being gone. So once the command has
        // confirmed every host stopped, the step removes what the hosts left
        // (socket files the command could not remove, which it reports only
        // as a note) and then `hosts` itself, and fails, while the farhelm is
        // still there to run again, if anything else is in it or the removal
        // fails. A `hosts` whose farhelm is already gone can only follow a
        // removal from outside; it fails the step with a way to end the hosts
        // by hand, never reading as success. The state directory is absolute,
        // as the command requires: planning refuses any other.
        //
        // Only exact socket-bound absence diagnostics excuse a failed tmux
        // command; permission, executable and protocol failures retain the host.
        // stat reports the lookup failure instead of collapsing all errors
        // into false, as shell existence predicates do. It receives one path
        // in the C locale: only the known GNU, uutils and BusyBox missing-path
        // diagnostics excuse bypassing a client (or a farhelm) that may already
        // have been removed on a lib-gone retry. Other lookup errors retain the host.
        // tmux leaves its socket behind on exit. Remove it while the frozen
        // client is still available, so a retry after lib removal can prove
        // absence without trying to execute that now-missing client.
        let script = format!(
            "export LC_ALL=C; socket={socket}; tmux={tmux}; farhelm={farhelm}; \
             data={data}; hosts=$data/{hosts}; changed=no; \
             state=$(systemctl --user show -p ActiveState --value -- {unit}) || exit 1; \
             case \"$state\" in inactive|failed) ;; \
               *) systemctl --user stop -- {unit} || exit 1; changed=yes ;; esac; \
             if inspection=$(stat -c '%F' -- \"$hosts\" 2>&1); then \
               if program=$(stat -L -c '%F' -- \"$farhelm\" 2>&1); then :; else status=$?; \
                 case \"$program\" in \
                   {missing}) printf '%s\\n' \"terminal hosts may still be running: $hosts exists, \
             but $farhelm, which would stop them, is gone; end every process whose command line \
             names a socket in $hosts, remove $hosts, and run uninstall again\" >&2; exit 1 ;; \
                   *) printf '%s\\n' \"$program\" >&2; exit \"$status\" ;; esac; fi; \
               if error=$(\"$farhelm\" internal stop-terminal-hosts --state-dir \"$data\" 2>&1); \
               then changed=yes; else status=$?; \
                 if [ -n \"$error\" ]; then printf '%s\\n' \"$error\" >&2; fi; \
                 printf '%s\\n' \"$farhelm internal stop-terminal-hosts exited with status $status; \
             terminal hosts in $hosts may still be running\" >&2; \
                 exit \"$status\"; fi; \
               others=$(find \"$hosts\" -mindepth 1 -maxdepth 1 ! -type s -print 2>&1) || {{ \
                 printf '%s\\n' \"$others\" \"cannot list $hosts to remove what its stopped terminal \
             hosts left\" >&2; exit 1; }}; \
               if [ -n \"$others\" ]; then \
                 printf '%s\\n' \"the terminal hosts stopped, but $hosts holds entries that are not \
             terminal host sockets, which uninstall does not remove; remove them and run uninstall \
             again:\" \"$others\" >&2; exit 1; fi; \
               removal=$(find \"$hosts\" -mindepth 1 -maxdepth 1 -type s \
             -exec rm -f -- {{}} \\; 2>&1); \
               if error=$(rmdir -- \"$hosts\" 2>&1); then :; else \
                 left=$(ls -A -- \"$hosts\" 2>&1); \
                 for line in \"$removal\" \"$error\"; do \
                   if [ -n \"$line\" ]; then printf '%s\\n' \"$line\" >&2; fi; done; \
                 printf '%s\\n' \"the terminal hosts stopped, but $hosts could not be removed; \
             left in it:\" \"$left\" >&2; exit 1; fi; \
             else status=$?; case \"$inspection\" in \
                 {missing}) ;; \
                 *) printf '%s\\n' \"$inspection\" >&2; exit \"$status\" ;; esac; fi; \
             if inspection=$(stat -L -c '%F' -- \"$socket\" 2>&1); then :; \
               else status=$?; case \"$inspection\" in \
                 {missing}) \
                   if [ \"$changed\" = no ]; then printf absent; fi; exit 0 ;; \
                 *) printf '%s\\n' \"$inspection\" >&2; exit \"$status\" ;; esac; fi; \
             if error=$(\"$tmux\" -S \"$socket\" kill-server 2>&1); then :; else status=$?; \
             case \"$error\" in \
               \"no server running on $socket\"|\"error connecting to $socket (No such file or directory)\") \
                 if [ \"$changed\" = no ]; then printf absent; fi ;; \
               *) printf '%s\\n' \"$error\" >&2; exit \"$status\" ;; esac; fi; \
             rm -f -- \"$socket\" || exit 1",
            unit = crate::ssh::shell_quote(unit),
            farhelm = shell_path(farhelm_program)?,
            data = shell_path(state_dir)?,
            hosts = farhelm_supervisor::hosts::HOSTS_DIR_NAME,
            missing = STAT_MISSING_PATH_PATTERNS,
            tmux = shell_path(tmux_program)?,
            socket = shell_path(tmux_socket)?,
        );
        let output = self
            .require_shell_within(target, &script, CONTEXT, STOP_SUPERVISOR_TIMEOUT)
            .await?;
        Ok(if output.stdout == b"absent" {
            ActionOutcome::Skipped(
                "the supervisor, its terminal hosts and private tmux server were already gone"
                    .to_string(),
            )
        } else {
            ActionOutcome::Completed
        })
    }

    /// Planning refuses a lib directory that is itself a symlink, and
    /// confirmation plans again, so a link made before the user confirmed
    /// is caught there. The test is repeated here, in the same shell as the
    /// `rm`, for a link made after that re-check, while the earlier steps
    /// (disable, unit removal, reload, stop) ran. A dangling link refuses
    /// too: `-L` sees it where `-e` does not. Planning's refusal of a mount
    /// on or inside the directory is repeated here for the same reason.
    async fn remove_directory(
        &self,
        target: &ProvisioningTarget,
        path: &Path,
    ) -> Result<ActionOutcome, BackendFailure> {
        require_remote(target, "removing Farhelm's lib directory")?;
        // This transport helper is also used after the host's binary is gone.
        // Detect the native mount table directly rather than persist OS state
        // in the host registry solely for a retry's filesystem guard.
        let system = self
            .require_shell(target, "uname -s", "identifying the host filesystem")
            .await?;
        let mounts = match system.stdout.as_slice() {
            b"Darwin\n" | b"Darwin" => super::macos::MAC_MOUNT_POINTS_SH,
            b"Linux\n" | b"Linux" => MOUNT_POINTS_SH,
            _ => {
                return Err(BackendFailure::new(
                    "unsupported host filesystem inspection",
                    "",
                ));
            }
        };
        let script = format!(
            "{mounts}d={dir}; \
             if [ ! -e \"$d\" ] && [ ! -L \"$d\" ]; then printf absent; exit 0; fi; \
             if [ -L \"$d\" ]; then \
               printf '%s\\n' \"refusing to remove $d: it has become a symbolic link since uninstall \
             last checked it, and removing it would leave the installation it points to in place; \
             nothing was removed\" >&2; exit 81; fi; \
             r=$(readlink -f -- \"$d\") && [ -n \"$r\" ] || exit 1; \
             inside=$(mount_points \"$r\" '\\n') || exit 1; \
             if [ -n \"$inside\" ]; then \
               printf '%s\\n' \"refusing to remove $d: something is mounted on or inside it ($inside), and \
             removing it would also delete the files that mount shows; nothing was removed\" >&2; \
             exit 82; fi; \
             rm -rf -- \"$d\"",
            dir = shell_path(path)?,
        );
        let output = self
            .require_shell(target, &script, "removing Farhelm's lib directory")
            .await?;
        Ok(if output.stdout == b"absent" {
            ActionOutcome::Skipped(format!("{} was already gone", path.display()))
        } else {
            ActionOutcome::Completed
        })
    }

    /// The helm's own process environment is the authority here, which is
    /// the one place in this file where that is true rather than a
    /// shortcut: this method asks about the machine the helm itself runs
    /// on, so the manager that would load the unit is the helm's own user
    /// manager. Remote hosts are asked over SSH by `inspect` instead, whose
    /// shell fragment is a separate copy of
    /// [`crate::units::user_unit_dir_for`]'s rule rather than a call into it
    /// (see the note there). The two differ for a relative
    /// `XDG_CONFIG_HOME`: this side falls back to `HOME`, the shell reports
    /// `unsupported-xdg`.
    ///
    /// Everything short of a confirmed absence is an error. A missing
    /// `HOME` with no absolute `XDG_CONFIG_HOME`, a file this process may
    /// not read, contents that are not UTF-8 — none of those mean "no unit
    /// exists", and answering `None` for them would have the panel tell the
    /// user to run setup over a unit it merely failed to inspect.
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

/// Shell function `mount_points DIR SEP`: print each mount point at or below
/// DIR, a non-empty resolved path, each followed by SEP (`'\0'` or `'\n'`).
///
/// Field 5 of `/proc/self/mountinfo` is a resolved path with `\ooo` octal
/// escapes for space, tab, newline and backslash, decoded here through
/// `printf %b`'s `\0ooo` form so it compares with `readlink -f` output.
/// Supported hosts are Linux, so an unreadable mount table fails the caller
/// rather than reading as "no mounts".
const MOUNT_POINTS_SH: &str = r#"mount_points() {
    [ -r /proc/self/mountinfo ] || {
        printf '%s\n' 'cannot read the mount table /proc/self/mountinfo' >&2
        return 1
    }
    while read -r _ _ _ _ point _; do
        case $point in *\\*)
            point=$(printf '%s' "$point" | sed 's/\\/\\0/g')
            point=$(printf '%b' "$point") ;;
        esac
        case $point in "$1" | "$1"/*) printf "%s$2" "$point" ;; esac
    done < /proc/self/mountinfo
}
"#;

/// Parse [`SystemBackend::inspect_uninstall`]'s output, or `None` when it
/// does not have exactly the fields asked for.
pub(super) fn parse_uninstall_inspection(
    output: &[u8],
    asked: &[&Path],
) -> Option<UninstallInspection> {
    fn host_path<'a>(
        fields: &mut impl Iterator<Item = &'a [u8]>,
        path: PathBuf,
    ) -> Option<HostPath> {
        fn flag<'a>(fields: &mut impl Iterator<Item = &'a [u8]>) -> Option<bool> {
            match fields.next()? {
                b"1" => Some(true),
                b"0" => Some(false),
                _ => None,
            }
        }
        let exists = flag(fields)?;
        let symlink = flag(fields)?;
        let canonical = fields.next()?;
        let raw = fields.next()?;
        let under_removed = flag(fields)?;
        let resolved = |field: &[u8]| {
            (!field.is_empty())
                .then(|| bytes_path(field).ok())
                .flatten()
        };
        Some(HostPath {
            path,
            exists,
            symlink,
            canonical: resolved(canonical),
            raw: resolved(raw),
            under_removed,
        })
    }
    let mut fields = output.split(|byte| *byte == 0);
    let paths = asked
        .iter()
        .map(|path| host_path(&mut fields, path.to_path_buf()))
        .collect::<Option<Vec<_>>>()?;
    let default_state = bytes_path(fields.next()?).ok()?;
    let default_state_dir = host_path(&mut fields, default_state)?;
    let unit_active_state = String::from_utf8(fields.next()?.to_vec()).ok()?;
    let fragment = bytes_path(fields.next()?).ok()?;
    let unit_fragment = host_path(&mut fields, fragment.clone())?;
    let unit_fragment = (!fragment.as_os_str().is_empty()).then_some(unit_fragment);
    // Mount points never print empty, so an empty field ends their list.
    let mounts = fields
        .by_ref()
        .take_while(|field| !field.is_empty())
        .map(|field| bytes_path(field).ok())
        .collect::<Option<Vec<_>>>()?;
    // The output ends with a NUL, which leaves one empty trailing field.
    if fields.next() != Some(b"") || fields.next().is_some() {
        return None;
    }
    Some(UninstallInspection {
        paths,
        default_state_dir,
        unit_active_state,
        unit_fragment,
        mounts,
    })
}

/// Refuse an UNINSTALL host action on the direct local transport.
///
/// Uninstall is planned only for remote rows (the helm's own machine is
/// `farhelm uninstall`'s), so a local plan cannot reach these actions; this
/// keeps the local executor from growing an implementation nothing can call.
fn require_remote(target: &ProvisioningTarget, what: &str) -> Result<(), BackendFailure> {
    match target {
        ProvisioningTarget::Ssh { .. } => Ok(()),
        ProvisioningTarget::Local => Err(BackendFailure::new(
            format!("{what} is not done on the helm's own machine; use farhelm uninstall there"),
            "",
        )),
    }
}

/// Encode a path as one remote shell word, refusing bytes that cannot cross
/// SSH's text command boundary without changing meaning.
pub(super) fn shell_path(path: &Path) -> Result<String, BackendFailure> {
    Ok(crate::ssh::shell_quote(&path_text(path)?))
}

/// Build a remote checksum command whose filename is the shell redirection
/// operand rather than a `sha256sum` argument.
pub(super) fn remote_sha256sum(path: &str) -> String {
    format!("sha256sum < {path}")
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
    lower_hex(&Sha256::digest(bytes))
}

/// Lowercase hex of a finished digest, the spelling `sha256sum` prints.
///
/// Written out by hand because sha2 0.11's digest output (a `hybrid_array`
/// array) no longer implements `LowerHex`, so `format!("{:x}", ...)` stopped
/// compiling.
fn lower_hex(digest: &[u8]) -> String {
    let mut out = String::with_capacity(digest.len() * 2);
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

/// The remote host's stderr from a degraded `loginctl enable-linger`, as the
/// helm's log line carries it: escaped and bounded by [`peer_text`].
///
/// The host is untrusted (SPEC.md's trust model), and the field is written
/// with the Display formatter, which would pass terminal control sequences
/// straight into the operator's terminal; SPEC_impl.md requires peer error
/// text to be escaped wherever it is logged. A function of its own so the
/// escaping can be tested without capturing the log.
pub(super) fn linger_log_stderr(stderr: &str) -> String {
    peer_text(stderr.trim())
}

/// What a failed `loginctl enable-linger` means for the run.
///
/// Linger is the optional step (SPEC.md Topology: an optional step that
/// cannot be done is reported and skipped), and it runs before an Update
/// restarts the supervisor onto its new binary. So any failure of the remote
/// command itself degrades the step, whatever loginctl said: a missing
/// loginctl, an unreachable system bus, or a refusal worded in a way nothing
/// here recognizes would otherwise fail every Update of that host, leaving it
/// on the old version with the new files already installed. Only a failure
/// of ssh itself (status 255, or no status) stays fatal, because then the
/// host was not reached at all and the steps after this one cannot work
/// either.
pub(super) fn linger_failure_outcome(
    code: Option<i32>,
    stderr: &str,
) -> Result<ActionOutcome, BackendFailure> {
    if linger_was_refused(code, stderr) {
        return Ok(ActionOutcome::Degraded(
            "linger was refused; starts at login, not at boot".to_string(),
        ));
    }
    match code {
        Some(code) if code != 0 && code != 255 => Ok(ActionOutcome::Degraded(format!(
            "linger could not be enabled (loginctl exited with status {code}); starts at login, \
             not at boot"
        ))),
        _ => Err(BackendFailure::new("enabling linger", stderr.to_string())),
    }
}

/// Recognize only a loginctl authorization refusal after the remote command
/// actually ran. SSH also reports authentication failures as permission
/// errors, commonly with status 255, so stderr text alone cannot justify
/// reporting a degraded boot-persistence outcome.
pub(super) fn linger_was_refused(code: Option<i32>, stderr: &str) -> bool {
    // Exit 255 belongs to ssh, not to the remote loginctl command. Requiring
    // a command exit code and its own diagnostic prevents an authentication
    // refusal containing "permission denied" from becoming a benign
    // best-effort linger degradation.
    if !code.is_some_and(|code| code != 0 && code != 255) {
        return false;
    }
    let lower = stderr.to_ascii_lowercase();
    let refusal = [
        "permission denied",
        "access denied",
        "authentication is required",
        "interactive authentication required",
        "not authorized",
    ]
    .iter()
    .any(|message| lower.contains(message));
    let loginctl_evidence = lower.contains("loginctl") || lower.contains("linger");
    refusal && loginctl_evidence
}

/// Where the reach check should look for the host's user units, as decided
/// from the manager's environment.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum ManagerUnitDir {
    /// No usable user manager answered.
    Unavailable,
    /// `XDG_CONFIG_HOME` is set but relative, which the reach check has
    /// always refused as undeterminable.
    RelativeXdg,
    /// `XDG_CONFIG_HOME` could be read only in `show-environment`'s escaped
    /// form (the user bus was unavailable to `busctl`).
    UnreadableXdg,
    /// An absolute `XDG_CONFIG_HOME`; the value is the unit directory.
    Xdg(String),
    /// No `XDG_CONFIG_HOME`: the host shell's `$HOME/.config/systemd/user`.
    HomeDefault,
}

/// Decode the environment probe's output: a source tag (`busctl`,
/// `show-environment` or `unavailable`) and that source's `XDG_CONFIG_HOME`
/// line, if any, each NUL-terminated.
///
/// The directory rule is the one the reach script applied in shell before
/// the environment moved here, kept identical: an absolute
/// `XDG_CONFIG_HOME` names `…/systemd/user`, a relative one is refused, and
/// none at all defers to the host shell's HOME.
///
/// A malformed record is reported without its contents: however carefully
/// the host filters, what came back is unvetted host output, and this
/// diagnostic is shown in the hosts panel.
pub(super) fn manager_unit_dir_from_probe(output: &[u8]) -> Result<ManagerUnitDir, BackendFailure> {
    use crate::units::{ManagerEnvironment, ManagerValue};
    let malformed = |detail: &str| {
        BackendFailure::new(
            "reading the systemd user manager's environment returned malformed output",
            detail.to_string(),
        )
    };
    let fields: Vec<&[u8]> = output.split(|byte| *byte == 0).collect();
    let [tag, text, rest] = fields.as_slice() else {
        return Err(malformed("expected a source tag and one value line"));
    };
    if !rest.is_empty() {
        return Err(malformed("unexpected output after the value line"));
    }
    let text = std::str::from_utf8(text).map_err(|_| malformed("the value line is not UTF-8"))?;
    let environment = match *tag {
        b"unavailable" => return Ok(ManagerUnitDir::Unavailable),
        b"busctl" => ManagerEnvironment::from_busctl_pretty_lines(text)
            .map_err(|_| malformed("busctl's value line is not a JSON string"))?,
        b"show-environment" => ManagerEnvironment::from_show_environment(text),
        _ => return Err(malformed("unknown source tag")),
    };
    Ok(match environment.get("XDG_CONFIG_HOME") {
        ManagerValue::Absent => ManagerUnitDir::HomeDefault,
        ManagerValue::Unreadable => ManagerUnitDir::UnreadableXdg,
        ManagerValue::Value(xdg) if xdg.starts_with('/') => {
            ManagerUnitDir::Xdg(format!("{xdg}/systemd/user"))
        }
        ManagerValue::Value(_) => ManagerUnitDir::RelativeXdg,
    })
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
    if fields.len() != 10 || fields[0] != REACH_RECORD_MARKER.as_bytes() || !fields[9].is_empty() {
        return Err(BackendFailure::new(
            "the provisioning reach check returned malformed output",
            String::from_utf8_lossy(output),
        ));
    }
    if fields[1].len() > 64 {
        return Err(BackendFailure::new(
            "the provisioning reach check returned malformed output",
            String::from_utf8_lossy(output),
        ));
    }
    // This value is shown in the confirmation plan. Validate it before it
    // can become a line in that plan; the ID remains informational and is
    // not treated as a distribution allowlist.
    let distro_id = String::from_utf8_lossy(fields[1]).into_owned();
    if distro_id.chars().any(char::is_control) {
        return Err(BackendFailure::new(
            "the provisioning reach check returned malformed output",
            String::from_utf8_lossy(output),
        ));
    }
    let arch_text = String::from_utf8_lossy(fields[3]);
    let arch = match arch_text.as_ref() {
        "x86_64" => PayloadTarget::LinuxX86_64,
        "aarch64" | "arm64" => PayloadTarget::LinuxAarch64,
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
    // Setup's unit is refused here, before any plan exists, so every
    // operation refuses it the same way, each in its own words (see
    // `ProvisioningOperation::setup_managed_refusal`). The unit write and the
    // unit removal re-check the marker (see `install_bytes` and
    // `remove_unit`), because a plan is confirmed some time after this
    // inspection.
    match fields[8] {
        b"" => {}
        b"setup" => return Ok(ReachOutcome::SetupManaged),
        _ => {
            return Err(BackendFailure::new(
                "the provisioning reach check returned malformed output",
                String::from_utf8_lossy(output),
            ));
        }
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
        launchd_domain: None,
        host_tmux: tmux_ok.then_some(tmux_path),
    }))
}

/// Mac discovery does not ask about tmux or systemd. A missing command is a
/// capability refusal, while the desktop app is an ownership refusal shared
/// by setup, Update and uninstall. The current helm owns this private record.
const MACOS_REACH_SH: &str = r#"
missing=''
for tool in launchctl readlink sha256sum stat install chmod mkdir mv cat head tr rm rmdir find ls grep id uname env sh mount sleep; do
    if ! command -v "$tool" >/dev/null 2>&1; then missing=$tool; break; fi
done
if [ -z "$missing" ] && ! readlink -f -- "$HOME" >/dev/null 2>&1; then missing='readlink -f'; fi
desktop=''
if [ -e "$HOME/Applications/Farhelm.app" ] || [ -e /Applications/Farhelm.app ]; then desktop=installed; fi
printf '%s\0%s\0' farhelm-reach-macos-v1 "${HOME-}"
uname -m | tr -d '\n'; printf '\0'
id -u | tr -d '\n'; printf '\0'
printf '%s\0%s\0' "$missing" "$desktop"
"#;

/// Validate all fields before any remote value becomes a plan path or a
/// launchctl domain. Intel Macs stay outside the representable payload set.
pub(super) fn parse_macos_reach_output(output: &[u8]) -> Result<ReachOutcome, BackendFailure> {
    let malformed = || BackendFailure::new("the Mac reach check returned malformed output", "");
    let fields: Vec<_> = output.split(|byte| *byte == 0).collect();
    if fields.len() != 7 || fields[0] != b"farhelm-reach-macos-v1" || !fields[6].is_empty() {
        return Err(malformed());
    }
    let text = fields
        .iter()
        .map(|field| std::str::from_utf8(field))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| malformed())?;
    if text.iter().any(|field| field.chars().any(char::is_control))
        || !matches!(text[5], "" | "installed")
    {
        return Err(malformed());
    }
    if text[5] == "installed" {
        return Ok(ReachOutcome::Manual("remote setup, Update and uninstall are not supported on a Mac with the Farhelm desktop app installed; update Farhelm from the desktop app on that Mac".into()));
    }
    if !matches!(text[2], "arm64" | "aarch64") {
        return Ok(ReachOutcome::Manual(format!(
            "automatic provisioning has no payload for Mac architecture {:?}",
            text[2]
        )));
    }
    if !text[4].is_empty() {
        return Ok(ReachOutcome::Manual(format!(
            "remote Mac provisioning requires the command {}",
            text[4]
        )));
    }
    let home = PathBuf::from(text[1]);
    if !home.is_absolute() || path_text(&home).is_err() {
        return Err(BackendFailure::new("the Mac reported an unusable HOME", ""));
    }
    let uid = text[3].parse::<u32>().map_err(|_| malformed())?;
    if text[3] != uid.to_string() {
        return Err(malformed());
    }
    Ok(ReachOutcome::Supported(Reach {
        user_unit_dir: home.join("Library/LaunchAgents"),
        home,
        arch: PayloadTarget::MacosAarch64,
        distro_id: "macOS".into(),
        needs_tmux: false,
        launchd_domain: Some(format!("user/{uid}")),
        host_tmux: None,
    }))
}

#[cfg(test)]
mod macos_reach_tests {
    use super::*;

    /// Match the private shell record, including its terminal empty field.
    fn record(home: &str, arch: &str, uid: &str, missing: &str, desktop: &str) -> Vec<u8> {
        [
            "farhelm-reach-macos-v1",
            home,
            arch,
            uid,
            missing,
            desktop,
            "",
        ]
        .join("\0")
        .into_bytes()
    }

    /// Apple silicon must choose Darwin bytes and freeze the SSH user's
    /// domain without requiring any tmux or Linux manager evidence.
    #[test]
    fn mac_reach_selects_darwin_payload_and_no_tmux() {
        let ReachOutcome::Supported(reach) =
            parse_macos_reach_output(&record("/Users/host", "arm64", "501", "", "")).unwrap()
        else {
            panic!("supported Mac refused")
        };
        assert_eq!(reach.arch, PayloadTarget::MacosAarch64);
        assert_eq!(reach.launchd_domain.as_deref(), Some("user/501"));
        assert_eq!(
            reach.user_unit_dir,
            Path::new("/Users/host/Library/LaunchAgents")
        );
        assert!(!reach.needs_tmux && reach.host_tmux.is_none());
    }

    /// Capability/ownership refusals must name the obstacle and never send
    /// a desktop-app owner toward starting a competing supervisor by hand.
    #[test]
    fn mac_reach_refuses_intel_missing_commands_and_desktop_ownership() {
        for (arch, missing, desktop, expected) in [
            ("x86_64", "", "", "no payload"),
            ("arm64", "sha256sum", "", "sha256sum"),
            ("arm64", "", "installed", "desktop app"),
        ] {
            let ReachOutcome::Manual(reason) =
                parse_macos_reach_output(&record("/Users/host", arch, "501", missing, desktop))
                    .unwrap()
            else {
                panic!("unsupported Mac accepted")
            };
            assert!(reason.contains(expected), "{reason}");
            assert!(!reason.contains("manually"));
        }
        for bytes in [
            record("/Users/host", "arm64", "0501", "", ""),
            record("/Users/host\nother", "arm64", "501", "", ""),
            record("relative", "arm64", "501", "", ""),
            b"truncated".to_vec(),
        ] {
            assert!(parse_macos_reach_output(&bytes).is_err());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stands in for an upload process that exits with status 7 at once while
    /// a detached holder (its own session, so outside the upload's process
    /// group) keeps stdin, stdout and stderr open without reading. The holder
    /// publishes its pid atomically so the test can end it.
    ///
    /// NOTE: This is a defensive shape, not one measured from OpenSSH. A live
    /// shared-connection client exits only once the session's output has
    /// closed (see `super::super::ssh_stand_in`), so over real ssh the exit
    /// and the held pipes do not coexist like this.
    #[cfg(target_os = "linux")]
    struct HeldUploadLauncher {
        holder_pid: PathBuf,
    }

    #[cfg(target_os = "linux")]
    impl CommandLauncher for HeldUploadLauncher {
        fn spawn(
            &self,
            command: &mut tokio::process::Command,
        ) -> std::io::Result<tokio::process::Child> {
            assert_eq!(command.as_std().get_program(), "ssh");
            // A non-interactive shell gives a background job /dev/null as stdin
            // unless it is redirected, and dash does not count `<&0` as one, so
            // the upload's stdin reaches the holder through fd 3.
            let script = format!(
                "exec 3<&0; setsid sh -c 'echo $$ > \"$1.tmp\" && mv \"$1.tmp\" \"$1\"; exec sleep 60' holder {} <&3 & exit 7",
                shell_words::quote(self.holder_pid.to_str().unwrap())
            );
            let mut child = tokio::process::Command::new("sh");
            child
                .args(["-c", &script])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            isolate_process_group(&mut child);
            child.spawn()
        }
    }

    /// Why: when an upload's process exits unsuccessfully while another
    /// process still holds its pipes, the feeder stays blocked writing input
    /// nobody reads. Joining it unconditionally would hang the upload, and so
    /// the whole install or update, instead of failing it. (The stall over
    /// the shared-connection stand-in, which follows measured OpenSSH, is
    /// `a_stalled_transfer_fails_while_a_shared_session_holds_stderr`.)
    ///
    /// Spec: with a payload larger than a pipe buffer, an upload that exits with
    /// status 7 while a holder keeps its pipes open returns an error within the
    /// reader grace plus a margin, rather than waiting on the feeder.
    #[cfg(target_os = "linux")]
    #[farhelm_testtrace::test]
    async fn a_failed_upload_does_not_wait_on_a_blocked_feeder() {
        let root = tempfile::tempdir().unwrap();
        let holder_pid = root.path().join("holder.pid");
        let source = root.path().join("payload");
        // Well past any pipe buffer, so the feeder blocks once the holder stops
        // nothing from reading.
        tokio::fs::write(&source, vec![b'x'; 4 * 1024 * 1024])
            .await
            .unwrap();
        let backend = SystemBackend {
            control_dir: root.path().to_path_buf(),
            linger: LingerBehavior::Simulated(Ok(())),
            launcher: Arc::new(HeldUploadLauncher {
                holder_pid: holder_pid.clone(),
            }),
            runtime_units: false,
            one_shot: false,
            fail_before_rename: false,
        };
        let result = tokio::time::timeout(
            CAPTURE_PIPE_GRACE + Duration::from_secs(10),
            backend.ssh_put(
                "held.example",
                crate::ssh::SshAccessKind::UserConfig,
                &source,
                &root.path().join("upload"),
            ),
        )
        .await;
        // End the holder whatever happened; its pid file is complete once present.
        if let Ok(text) = std::fs::read_to_string(&holder_pid)
            && let Ok(pid) = text.trim().parse::<i32>()
        {
            // SAFETY: the holder is this test's own fixture process.
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
        }
        let failure = result
            .expect("a failed upload must not wait on a feeder blocked by held pipes")
            .expect_err("an upload that exits with status 7 fails");
        assert!(
            failure.context.contains("payload transfer")
                || failure.context.contains("transferring"),
            "{failure:?}"
        );
    }

    /// The helper's connection progress cannot authorize provisioning or
    /// supply a resolved executable path. Those facts still come only from
    /// the wrapper's own records, even when both kinds share stderr.
    #[farhelm_testtrace::test]
    async fn connected_marker_does_not_replace_probe_wrapper_records() {
        let connected = format!("{}\n", farhelm_proto::io::SUPERVISOR_CONNECTED_MARKER);
        let (signal, mut failures) = tokio::sync::mpsc::unbounded_channel();
        let helper_only = drain_probe_stderr(connected.as_bytes(), signal).await;
        assert!(!helper_only.command_started);
        assert!(helper_only.resolved_farhelm.is_none());
        assert_eq!(helper_only.prefix, connected.as_bytes());
        assert!(failures.try_recv().is_err());

        let wrapped =
            format!("{REMOTE_PROBE_MARKER}\n{REMOTE_RESOLVED_PREFIX}/fixture/farhelm\n{connected}");
        let (signal, mut failures) = tokio::sync::mpsc::unbounded_channel();
        let with_wrapper = drain_probe_stderr(wrapped.as_bytes(), signal).await;
        assert!(with_wrapper.command_started);
        assert_eq!(
            with_wrapper.resolved_farhelm.as_deref(),
            Some(b"/fixture/farhelm".as_slice())
        );
        assert_eq!(with_wrapper.prefix, wrapped.as_bytes());
        assert!(failures.try_recv().is_err());
    }

    /// Reap an isolated child before pausing time, so OS scheduling cannot
    /// consume the virtual deadline meant only for its diagnostic reader.
    async fn reaped_probe() -> tokio::process::Child {
        let mut command = tokio::process::Command::new("true");
        command.kill_on_drop(true);
        isolate_process_group(&mut command);
        let mut child = command.spawn().expect("spawn isolated probe fixture");
        assert!(child.wait().await.expect("reap probe fixture").success());
        child
    }

    /// An open diagnostic stream must not retain planning capacity after
    /// the probe child is gone. EOF on the peer proves the reader was
    /// cancelled, rather than detached to keep waiting after the timeout.
    #[farhelm_testtrace::test]
    async fn stop_probe_bounds_and_cancels_an_open_stderr_reader() {
        let mut child = reaped_probe().await;
        let (mut producer, consumer) = tokio::io::duplex(64);
        let (signal, _failures) = tokio::sync::mpsc::unbounded_channel();
        let stderr_task = ProbeStderrReader::spawn(consumer, signal);
        assert!(
            !stderr_task.task.is_finished(),
            "the producer still holds stderr open"
        );
        tokio::time::pause();
        let started = tokio::time::Instant::now();
        let drained =
            tokio::time::timeout(Duration::from_secs(3), stop_probe(&mut child, stderr_task))
                .await
                .expect("an open stderr pipe must not keep the probe alive");
        // Timer ticks can round up: require the two-second wait without
        // reaching the three-second failure guard, rather than exact precision.
        assert_eq!(started.elapsed().as_secs(), 2);
        assert!(drained.prefix.is_empty());
        assert!(child.try_wait().unwrap().is_some());
        let mut byte = [0];
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), producer.read(&mut byte))
                .await
                .expect("the cancelled reader must release its pipe")
                .expect("read peer EOF"),
            0
        );
    }

    /// Normal cleanup still returns completed diagnostics; adding a bound
    /// must not turn an ordinary probe failure into an empty explanation.
    #[farhelm_testtrace::test]
    async fn stop_probe_retains_completed_stderr() {
        let mut child = reaped_probe().await;
        let (mut producer, consumer) = tokio::io::duplex(64);
        producer.write_all(b"retained diagnostic").await.unwrap();
        producer.shutdown().await.unwrap();
        let (signal, _failures) = tokio::sync::mpsc::unbounded_channel();
        let stderr_task = ProbeStderrReader::spawn(consumer, signal);
        let drained = stop_probe(&mut child, stderr_task).await;
        assert_eq!(drained.prefix, b"retained diagnostic");
    }

    /// Why: a shared ssh connection's master holds the probe session's stderr
    /// open after the probe's own ssh process is killed, until the remote
    /// command ends, so the bounded wait for EOF expires whenever the remote
    /// side takes longer than the bound. Discarding what was read by then
    /// made such a probe report "answered without reporting its resolved
    /// binary", refusing remote uninstall (#3005).
    ///
    /// Spec: when stderr stays open past the bound, `stop_probe` still returns
    /// the wrapper records and the diagnostic prefix that arrived before it.
    #[farhelm_testtrace::test]
    async fn stop_probe_keeps_records_read_before_its_bound_expires() {
        let mut child = reaped_probe().await;
        let (mut producer, consumer) = tokio::io::duplex(1024);
        let records = format!("{REMOTE_PROBE_MARKER}\n{REMOTE_RESOLVED_PREFIX}/fixture/farhelm\n");
        producer.write_all(records.as_bytes()).await.unwrap();
        let (signal, _failures) = tokio::sync::mpsc::unbounded_channel();
        let stderr_task = ProbeStderrReader::spawn(consumer, signal);
        // Premise: the reader has consumed both records while the producer,
        // standing in for the master, still holds stderr open.
        tokio::time::timeout(Duration::from_secs(10), async {
            while stderr_task
                .progress
                .lock()
                .unwrap()
                .resolved_farhelm
                .is_none()
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the reader recognizes the records written before the stop");
        assert!(!stderr_task.task.is_finished(), "stderr is still open");
        tokio::time::pause();
        let drained = stop_probe(&mut child, stderr_task).await;
        assert!(drained.command_started);
        assert_eq!(
            drained.resolved_farhelm.as_deref(),
            Some(b"/fixture/farhelm".as_slice())
        );
        assert_eq!(drained.prefix, records.as_bytes());
        drop(producer);
    }
}
