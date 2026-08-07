//! Discovery-first supervisor provisioning and its host-scoped REST state.
//!
//! Provisioning is a convergence operation, not an installer transaction.
//! The confirmed [`ProvisioningPlan`] is the only description of the work:
//! the confirmation renderer walks its actions, and the executor walks the
//! same actions after confirmation. A failed action leaves every completed
//! action in place so rerunning can resume from content and hash comparisons.
//!
//! Transport is deliberately below that plan. Local setup executes and
//! copies directly; remote setup uses the user's `ssh` and `sftp`, sharing
//! the option-safe SSH prefix with the steady-state connection manager.

use crate::manager::{ConnectionManager, HostState, peer_text};
use crate::store::{
    DialedAs, FirstContactOutcome, HelmStore, HostId, HostKind, HostRow, HostStoreError,
};
use crate::{AppState, http_error};
use anyhow::{Context, bail};
use async_trait::async_trait;
use axum::extract::{Path as AxPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use farhelm_proto::ControlMsg;
use farhelm_proto::io::{ClosedBeforeHello, FrameReader, FrameWriter, handshake};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const PROBE_TIMEOUT: Duration = Duration::from_secs(15);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(60);
const ATTACH_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_CHILD_STREAM_BYTES: usize = 64 * 1024;
const POSITIVE_ABSENCE_EXIT: i32 = 75;
const REMOTE_PROBE_MARKER: &str = "farhelm-probe-command-started-v1";
const REMOTE_RESOLVED_PREFIX: &str = "farhelm-probe-resolved-v1:";
const REACH_RECORD_MARKER: &str = "farhelm-reach-v1";
const MAX_PENDING_PLANS: usize = 64;
const MAX_CONCURRENT_RUNS: usize = 4;
const PAYLOAD_COPY_BUFFER: usize = 64 * 1024;

/// Request-level refusals whose HTTP status must not depend on prose.
#[derive(Debug, thiserror::Error)]
enum ProvisioningRequestError {
    #[error("{0}")]
    InvalidProbe(String),
    #[error("the provisioning plan is unknown or has already been used; probe again")]
    UnknownPlan,
    #[error("host {0} already has a provisioning operation in flight")]
    Busy(HostId),
}

/// What the client wants discovery to inspect.
///
/// The tagged target keeps a remote host literally named `local` distinct
/// from the reserved direct-process transport.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProbeRequest {
    target: ProbeDestination,
    #[serde(default)]
    remote_farhelm: Option<String>,
    #[serde(default)]
    remote_state_dir: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum ProbeDestination {
    Local,
    Ssh { destination: String },
}

/// The opaque reference returned with a confirmed provisioning plan.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProvisionRequest {
    probe_id: String,
}

/// Discovery either registers an answering supervisor, offers one concrete
/// plan, or explains why this host remains a manual install.
#[derive(Debug, Serialize)]
#[serde(tag = "result", rename_all = "kebab-case")]
enum ProbeResponse {
    Discovered {
        host_id: HostId,
        build_version: String,
        host_identity: Option<String>,
    },
    Provisionable {
        probe_id: String,
        plan: ProvisioningPlan,
        confirmation: String,
    },
    Manual {
        reason: String,
    },
}

/// Identity returned before a long provision or update continues in the
/// background.
#[derive(Debug, Serialize)]
pub(crate) struct RunAccepted {
    host_id: HostId,
    run_id: String,
}

/// A frozen UPDATE plan. Posting its opaque id back to the same host route
/// consumes it exactly once and starts the run.
#[derive(Debug, Serialize)]
pub(crate) struct UpdatePlanResponse {
    probe_id: String,
    plan: ProvisioningPlan,
    confirmation: String,
}

/// Whether a plan is converging an absent install or explicitly updating an
/// existing one. ADD never turns into UPDATE implicitly.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ProvisioningOperation {
    Add,
    Update,
}

/// Transport facts retained in the plan so execution cannot silently switch
/// from the local no-SSH path to SSH-to-self, or vice versa.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum ProvisioningTarget {
    Local,
    Ssh { destination: String },
}

/// Installation artifacts selected independently; callers must never use a
/// Farhelm executable to satisfy a tmux request or vice versa.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PayloadKind {
    Farhelm,
    Tmux,
}

/// Architectures with release payloads. Reach inspection maps the remote
/// machine to one of these before confirmation, never during execution.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PayloadArch {
    X86_64,
    Aarch64,
}

/// One directory and the mode provisioning must converge on every rerun.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct DirectorySpec {
    path: PathBuf,
    mode: u32,
}

/// Every mutating or attaching action in the order the executor performs it.
///
/// Paths, unit contents, linger's conditional boot promise, and the
/// persistent-run statement live here rather than in a parallel confirmation
/// template. Adding executor behavior therefore requires adding something the
/// user will see before confirmation.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "step", rename_all = "kebab-case")]
pub(crate) enum ProvisioningAction {
    EnsureDirectories {
        directories: Vec<DirectorySpec>,
    },
    InstallPayload {
        payload: PayloadKind,
        arch: PayloadArch,
        destination: PathBuf,
        temporary: PathBuf,
    },
    WriteUnit {
        unit: String,
        destination: PathBuf,
        temporary: PathBuf,
        content: String,
    },
    DaemonReload,
    EnableSupervisor {
        unit: String,
        unit_path: PathBuf,
        persistent_run: String,
    },
    EnableLinger {
        boot_start_if_enabled: String,
        login_start_if_refused: String,
    },
    RestartSupervisor {
        unit: String,
    },
    AttachSupervisor,
}

impl ProvisioningAction {
    fn label(&self) -> &'static str {
        match self {
            Self::EnsureDirectories { .. } => "create-directories",
            Self::InstallPayload {
                payload: PayloadKind::Farhelm,
                ..
            } => "install-farhelm",
            Self::InstallPayload {
                payload: PayloadKind::Tmux,
                ..
            } => "install-tmux",
            Self::WriteUnit { .. } => "write-unit",
            Self::DaemonReload => "daemon-reload",
            Self::EnableSupervisor { .. } => "enable-supervisor",
            Self::EnableLinger { .. } => "enable-linger",
            Self::RestartSupervisor { .. } => "restart-supervisor",
            Self::AttachSupervisor => "attach-supervisor",
        }
    }

    fn confirmation_line(&self) -> String {
        match self {
            Self::EnsureDirectories { directories } => format!(
                "create or reuse directories {}",
                directories
                    .iter()
                    .map(|directory| {
                        format!("{} (mode {:04o})", directory.path.display(), directory.mode)
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::InstallPayload {
                payload,
                destination,
                temporary,
                ..
            } => format!(
                "install {payload:?} at {} via temporary file {} and atomic rename",
                destination.display(),
                temporary.display()
            ),
            Self::WriteUnit {
                unit,
                destination,
                temporary,
                ..
            } => format!(
                "write user unit {unit} at {} via temporary file {} and atomic rename",
                destination.display(),
                temporary.display()
            ),
            Self::DaemonReload => "reload the systemd user manager".to_string(),
            Self::EnableSupervisor {
                unit,
                persistent_run,
                ..
            } => format!("enable and start {unit}; {persistent_run}"),
            Self::EnableLinger {
                boot_start_if_enabled,
                login_start_if_refused,
            } => format!(
                "optionally enable linger: {boot_start_if_enabled}; if privilege is refused, \
                 continue and report that it {login_start_if_refused}"
            ),
            Self::RestartSupervisor { unit } => format!(
                "restart {unit}; tmux keeps existing sessions running during the supervisor restart"
            ),
            Self::AttachSupervisor => {
                "dial the supervisor and attach it to the already-registered host row".to_string()
            }
        }
    }
}

/// The exact value shown for confirmation and later consumed by execution.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct ProvisioningPlan {
    operation: ProvisioningOperation,
    target: ProvisioningTarget,
    farhelm_path: PathBuf,
    state_dir: PathBuf,
    actions: Vec<ProvisioningAction>,
}

impl ProvisioningPlan {
    /// Render the plan without maintaining a second list of promises.
    fn confirmation(&self) -> String {
        let mut rendered = format!(
            "Farhelm will perform these steps for {}:\n",
            match &self.target {
                ProvisioningTarget::Local => "the local host".to_string(),
                ProvisioningTarget::Ssh { destination } => destination.clone(),
            }
        );
        for (index, action) in self.actions.iter().enumerate() {
            rendered.push_str(&format!("{}. {}\n", index + 1, action.confirmation_line()));
        }
        rendered
    }
}

/// One action's observable progress. Completed runs remain readable through
/// the host-scoped GET until a later run replaces them.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum StepStatus {
    Pending,
    Running,
    Completed,
    Skipped,
    Degraded,
    Failed,
}

/// One action's retained status and bounded explanatory message.
#[derive(Debug, Clone, Serialize)]
struct StepView {
    step: String,
    status: StepStatus,
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum RunStatus {
    Running,
    Completed,
    Failed,
}

/// What `GET /api/hosts/{id}/provisioning` returns.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProvisioningView {
    host_id: HostId,
    run_id: Option<String>,
    operation: Option<ProvisioningOperation>,
    status: RunStatus,
    steps: Vec<StepView>,
    message: Option<String>,
}

impl ProvisioningView {
    fn idle(host_id: HostId) -> Self {
        Self {
            host_id,
            run_id: None,
            operation: None,
            status: RunStatus::Completed,
            steps: Vec::new(),
            message: Some("no provisioning run has been recorded for this host".to_string()),
        }
    }

    fn for_plan(host_id: HostId, run_id: String, plan: &ProvisioningPlan) -> Self {
        Self {
            host_id,
            run_id: Some(run_id),
            operation: Some(plan.operation),
            status: RunStatus::Running,
            steps: plan
                .actions
                .iter()
                .map(|action| StepView {
                    step: action.label().to_string(),
                    status: StepStatus::Pending,
                    message: None,
                })
                .collect(),
            message: None,
        }
    }
}

/// Probe-time transport plus the binary and state directory the stdio proxy
/// must use before any install plan exists.
#[derive(Debug, Clone)]
struct ProbeTarget {
    transport: ProvisioningTarget,
    probe_farhelm: PathBuf,
    probe_state_dir: Option<PathBuf>,
}

/// What completing the protocol hello proves about discovery.
#[derive(Debug)]
enum ProbeObservation {
    Supervisor {
        build_version: String,
        host_identity: Option<String>,
        dial_farhelm: PathBuf,
        dial_state_dir: Option<PathBuf>,
    },
    Absent,
}

/// Host facts needed to select payloads and render absolute install paths,
/// including the unit directory the running user manager actually searches.
#[derive(Debug, Clone)]
struct Reach {
    home: PathBuf,
    user_unit_dir: PathBuf,
    arch: PayloadArch,
    needs_tmux: bool,
    tmux_dir: Option<PathBuf>,
}

/// Supported hosts continue to a plan; every other host keeps the manual
/// supervisor path without turning platform mismatch into a setup failure.
#[derive(Debug, Clone)]
enum ReachOutcome {
    Supported(Reach),
    Manual(String),
}

/// Idempotent actions distinguish useful no-ops and optional degradation
/// from ordinary completion in the progress record.
#[derive(Debug)]
enum ActionOutcome {
    Completed,
    Skipped(String),
    Degraded(String),
}

/// A backend failure preserves the host's stderr separately so the REST
/// progress record can escape it before retention.
#[derive(Debug)]
struct BackendFailure {
    context: String,
    stderr: String,
}

impl std::fmt::Display for BackendFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.rendered())
    }
}

impl std::error::Error for BackendFailure {}

impl BackendFailure {
    fn new(context: impl Into<String>, stderr: impl Into<String>) -> Self {
        Self {
            context: context.into(),
            stderr: stderr.into(),
        }
    }

    fn rendered(&self) -> String {
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
trait ProvisioningBackend: Send + Sync {
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
}

/// Supplies host-architecture-specific artifacts without coupling item 6 to
/// item 8's release embedding.
pub trait PayloadSource: Send + Sync {
    fn path(&self, payload: PayloadKind, arch: PayloadArch) -> anyhow::Result<PathBuf>;
}

/// Development helms intentionally carry no cross-compiled install payloads.
struct NoPayloads;

impl PayloadSource for NoPayloads {
    fn path(&self, _payload: PayloadKind, _arch: PayloadArch) -> anyhow::Result<PathBuf> {
        bail!("this build carries no provisioning payloads")
    }
}

#[derive(Debug, Clone)]
struct PlanLayout {
    local_state_dir: PathBuf,
    override_lib_dir: Option<PathBuf>,
    override_farhelm_path: Option<PathBuf>,
    override_state_dir: Option<PathBuf>,
    override_unit_dir: Option<PathBuf>,
    unit_name: String,
}

impl PlanLayout {
    /// Standard per-user paths, with the local supervisor sharing the helm's
    /// state directory as the local transport requires.
    fn production(local_state_dir: PathBuf) -> Self {
        Self {
            local_state_dir,
            override_lib_dir: None,
            override_farhelm_path: None,
            override_state_dir: None,
            override_unit_dir: None,
            unit_name: "farhelm-supervisor.service".to_string(),
        }
    }

    /// Freeze every path, unit byte, and operation-specific action before
    /// confirmation; execution is not allowed to derive any of them later.
    fn plan(
        &self,
        operation: ProvisioningOperation,
        target: ProvisioningTarget,
        reach: &Reach,
        run_nonce: &str,
    ) -> Result<ProvisioningPlan, BackendFailure> {
        let lib_dir = self
            .override_lib_dir
            .clone()
            .unwrap_or_else(|| reach.home.join(".local/lib/farhelm"));
        let state_dir = self.override_state_dir.clone().unwrap_or_else(|| {
            if matches!(target, ProvisioningTarget::Local) {
                self.local_state_dir.clone()
            } else {
                reach.home.join(".local/state/farhelm")
            }
        });
        let unit_dir = self
            .override_unit_dir
            .clone()
            .unwrap_or_else(|| reach.user_unit_dir.clone());
        let farhelm_path = self
            .override_farhelm_path
            .clone()
            .unwrap_or_else(|| lib_dir.join("farhelm"));
        let unit_path = unit_dir.join(&self.unit_name);
        let temporary = |path: &Path| {
            path.with_file_name(format!(
                ".{}.farhelm-tmp-{run_nonce}",
                path.file_name()
                    .expect("provisioning destinations always have file names")
                    .to_string_lossy()
            ))
        };
        let mut actions = vec![ProvisioningAction::EnsureDirectories {
            directories: vec![
                DirectorySpec {
                    path: lib_dir.clone(),
                    mode: 0o755,
                },
                DirectorySpec {
                    path: state_dir.clone(),
                    mode: 0o700,
                },
                DirectorySpec {
                    path: unit_dir,
                    mode: 0o755,
                },
            ],
        }];
        actions.push(ProvisioningAction::InstallPayload {
            payload: PayloadKind::Farhelm,
            arch: reach.arch,
            destination: farhelm_path.clone(),
            temporary: temporary(&farhelm_path),
        });
        if reach.needs_tmux {
            let tmux_path = lib_dir.join("tmux");
            actions.push(ProvisioningAction::InstallPayload {
                payload: PayloadKind::Tmux,
                arch: reach.arch,
                destination: tmux_path.clone(),
                temporary: temporary(&tmux_path),
            });
        }
        let content = supervisor_unit(
            &farhelm_path,
            &state_dir,
            &lib_dir,
            reach.tmux_dir.as_deref(),
        )?;
        actions.extend([
            ProvisioningAction::WriteUnit {
                unit: self.unit_name.clone(),
                destination: unit_path.clone(),
                temporary: temporary(&unit_path),
                content,
            },
            ProvisioningAction::DaemonReload,
            ProvisioningAction::EnableSupervisor {
                unit: self.unit_name.clone(),
                unit_path: unit_path.clone(),
                persistent_run: "the supervisor runs persistently under the systemd user manager"
                    .to_string(),
            },
            ProvisioningAction::EnableLinger {
                boot_start_if_enabled: "the supervisor starts at boot if linger succeeds"
                    .to_string(),
                login_start_if_refused: "starts at login, not at boot".to_string(),
            },
        ]);
        if operation == ProvisioningOperation::Update {
            actions.push(ProvisioningAction::RestartSupervisor {
                unit: self.unit_name.clone(),
            });
        }
        actions.push(ProvisioningAction::AttachSupervisor);
        Ok(ProvisioningPlan {
            operation,
            target,
            farhelm_path,
            state_dir,
            actions,
        })
    }

    /// UPDATE converges the installation the registry actually dials. A
    /// custom binary or state override is not permission to create a second
    /// standard-layout supervisor beside it.
    fn plan_for_row(
        &self,
        row: &HostRow,
        target: ProvisioningTarget,
        reach: &Reach,
        run_nonce: &str,
    ) -> Result<ProvisioningPlan, BackendFailure> {
        let mut layout = self.clone();
        if row.kind == HostKind::Ssh {
            if let Some(farhelm) = &row.remote_farhelm {
                let farhelm = PathBuf::from(farhelm);
                layout.override_lib_dir = farhelm.parent().map(Path::to_path_buf);
                layout.override_farhelm_path = Some(farhelm);
            }
            if let Some(state_dir) = &row.remote_state_dir {
                layout.override_state_dir = Some(PathBuf::from(state_dir));
            }
        }
        layout.plan(ProvisioningOperation::Update, target, reach, run_nonce)
    }
}

/// Quote one path for systemd's ExecStart grammar.
fn systemd_arg(path: &Path) -> Result<String, BackendFailure> {
    Ok(format!(
        "\"{}\"",
        path_text(path)?
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%")
    ))
}

/// Render the supervisor unit from the paths carried by the plan.
///
/// The existing tmux directory is retained because a user-manager process
/// does not necessarily inherit the login shell PATH that the reach check
/// used. The private payload directory stays first for hosts where Farhelm
/// supplies tmux itself. `KillMode=process` is equally deliberate: tmux owns
/// the durable sessions, so restarting their manager must stop only the
/// supervisor process rather than systemd's default whole control group.
fn supervisor_unit(
    farhelm: &Path,
    state_dir: &Path,
    lib_dir: &Path,
    tmux_dir: Option<&Path>,
) -> Result<String, BackendFailure> {
    let mut search = vec![lib_dir.to_path_buf()];
    if let Some(tmux_dir) = tmux_dir
        && !search.iter().any(|path| path == tmux_dir)
    {
        search.push(tmux_dir.to_path_buf());
    }
    search.extend([
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
    ]);
    let search = search
        .iter()
        .map(|path| {
            let text = path_text(path)?;
            if text.contains(':') {
                return Err(BackendFailure::new(
                    format!("PATH component {text:?} contains ':'"),
                    "systemd Environment PATH cannot represent that component faithfully",
                ));
            }
            Ok(text)
        })
        .collect::<Result<Vec<_>, BackendFailure>>()?
        .join(":")
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%");
    Ok(format!(
        "[Unit]\nDescription=Farhelm supervisor\nAfter=default.target\n\n\
         [Service]\nType=simple\nExecStart={} supervisor run --state-dir {}\n\
         Environment=\"PATH={}\"\nKillMode=process\nRestart=on-failure\n\n\
         [Install]\nWantedBy=default.target\n",
        systemd_arg(farhelm)?,
        systemd_arg(state_dir)?,
        search
    ))
}

/// Bounded process-local state behind plan confirmation and progress reads.
#[derive(Default)]
struct ProvisioningMemory {
    plans: HashMap<String, PendingPlan>,
    plan_order: VecDeque<String>,
    runs: HashMap<HostId, ProvisioningView>,
    busy: std::collections::HashSet<HostId>,
    tasks: HashMap<HostId, tokio::task::JoinHandle<()>>,
}

/// A confirmed plan retains the registration inputs used before execution.
#[derive(Clone)]
struct PendingPlan {
    plan: ProvisioningPlan,
    confirmation: PendingConfirmation,
}

/// The facts a confirmation must revalidate before its first mutation.
#[derive(Clone)]
enum PendingConfirmation {
    Add {
        registration: ProbeRegistration,
        original_target: ProbeTarget,
    },
    Update {
        host: HostId,
        target: ProbeTarget,
        expected_identity: Option<String>,
        registration: ProbeRegistration,
    },
}

/// Confirmation either preserves the frozen executor plan or discovers that
/// ADD must stop and adopt an answering supervisor instead.
enum Revalidation {
    Execute,
    UseAsIs(String),
}

/// Registry input paired with a probe, separate from the executor's target
/// because registration owns SSH metadata while execution owns transport.
#[derive(Clone, PartialEq, Eq)]
enum ProbeRegistration {
    Local,
    Ssh {
        destination: String,
        remote_farhelm: Option<String>,
        remote_state_dir: Option<String>,
    },
}

/// The exact connection coordinates and identity proved by one completed
/// hello. Keeping this owned lets registration commit the same facts after
/// the probe process has been reaped.
struct DiscoveredDial {
    farhelm: PathBuf,
    state_dir: Option<PathBuf>,
    identity: Option<String>,
}

/// Process-local orchestration authority for probes and one run per host.
pub(crate) struct ProvisioningService {
    backend: Arc<dyn ProvisioningBackend>,
    payloads: Arc<dyn PayloadSource>,
    store: HelmStore,
    manager: Arc<ConnectionManager>,
    layout: PlanLayout,
    local_farhelm: PathBuf,
    memory: tokio::sync::Mutex<ProvisioningMemory>,
    run_slots: Arc<tokio::sync::Semaphore>,
    /// Fail the next durable-to-live registry handoff so tests can prove the
    /// database and actor set do not diverge after registration commits.
    #[cfg(test)]
    fail_registry_sync: std::sync::atomic::AtomicBool,
}

impl ProvisioningService {
    /// Production composition uses real process/file operations and the
    /// deliberately empty development payload source item 8 will replace.
    pub(crate) fn production(
        store: HelmStore,
        manager: Arc<ConnectionManager>,
        helm_state_dir: PathBuf,
    ) -> anyhow::Result<Arc<Self>> {
        let local_farhelm =
            std::env::current_exe().context("locating the running farhelm binary")?;
        Ok(Arc::new(Self {
            backend: Arc::new(SystemBackend::new(helm_state_dir.clone())),
            payloads: Arc::new(NoPayloads),
            store,
            manager,
            layout: PlanLayout::production(helm_state_dir),
            local_farhelm,
            memory: tokio::sync::Mutex::new(ProvisioningMemory::default()),
            run_slots: Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_RUNS)),
            #[cfg(test)]
            fail_registry_sync: std::sync::atomic::AtomicBool::new(false),
        }))
    }

    #[cfg(test)]
    fn injected(
        store: HelmStore,
        manager: Arc<ConnectionManager>,
        backend: Arc<dyn ProvisioningBackend>,
        payloads: Arc<dyn PayloadSource>,
        layout: PlanLayout,
        local_farhelm: PathBuf,
    ) -> Arc<Self> {
        Arc::new(Self {
            backend,
            payloads,
            store,
            manager,
            layout,
            local_farhelm,
            memory: tokio::sync::Mutex::new(ProvisioningMemory::default()),
            run_slots: Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_RUNS)),
            fail_registry_sync: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// Split one request into probe transport and eventual registry input.
    fn target(&self, request: &ProbeRequest) -> anyhow::Result<(ProbeTarget, ProbeRegistration)> {
        match &request.target {
            ProbeDestination::Local => {
                if request.remote_farhelm.is_some() || request.remote_state_dir.is_some() {
                    return Err(anyhow::Error::new(ProvisioningRequestError::InvalidProbe(
                        "the local probe does not accept remote_farhelm or remote_state_dir"
                            .to_string(),
                    )));
                }
                Ok((
                    ProbeTarget {
                        transport: ProvisioningTarget::Local,
                        probe_farhelm: self.local_farhelm.clone(),
                        probe_state_dir: Some(self.layout.local_state_dir.clone()),
                    },
                    ProbeRegistration::Local,
                ))
            }
            ProbeDestination::Ssh { destination } => {
                if !crate::store::destination_is_usable(destination) {
                    return Err(anyhow::Error::new(ProvisioningRequestError::InvalidProbe(
                        format!("{destination:?} is not a usable ssh destination"),
                    )));
                }
                let registration = ProbeRegistration::Ssh {
                    destination: destination.clone(),
                    remote_farhelm: request.remote_farhelm.clone(),
                    remote_state_dir: request.remote_state_dir.clone(),
                };
                Ok((
                    ProbeTarget {
                        transport: ProvisioningTarget::Ssh {
                            destination: destination.clone(),
                        },
                        probe_farhelm: PathBuf::from(
                            request.remote_farhelm.as_deref().unwrap_or("farhelm"),
                        ),
                        probe_state_dir: request.remote_state_dir.as_deref().map(PathBuf::from),
                    },
                    registration,
                ))
            }
        }
    }

    /// Complete discovery before either registering an answer or retaining a
    /// non-mutating plan for later confirmation.
    async fn probe(&self, mut request: ProbeRequest) -> anyhow::Result<ProbeResponse> {
        // A rerun against a row this helm already knows must probe the
        // installed path recorded at registration, not fall back to PATH.
        // A brand-new helm has no such row; the remote probe itself also
        // checks the standard flat install path for that recovery case.
        if let ProbeDestination::Ssh { destination } = &request.target
            && (request.remote_farhelm.is_none() || request.remote_state_dir.is_none())
            && let Some(row) = self
                .store
                .list_hosts()
                .await?
                .into_iter()
                .find(|row| row.destination.as_deref() == Some(destination.as_str()))
        {
            request.remote_farhelm = request.remote_farhelm.or(row.remote_farhelm);
            request.remote_state_dir = request.remote_state_dir.or(row.remote_state_dir);
        }
        let (target, registration) = self.target(&request)?;
        match self
            .backend
            .probe(&target)
            .await
            .map_err(anyhow::Error::new)?
        {
            ProbeObservation::Supervisor {
                build_version,
                host_identity,
                dial_farhelm,
                dial_state_dir,
            } => {
                let host_id = self
                    .register(
                        &registration,
                        None,
                        Some(DiscoveredDial {
                            farhelm: dial_farhelm,
                            state_dir: dial_state_dir,
                            identity: host_identity.clone(),
                        }),
                    )
                    .await?;
                Ok(ProbeResponse::Discovered {
                    host_id,
                    build_version,
                    host_identity,
                })
            }
            ProbeObservation::Absent => {
                let reach = match self
                    .backend
                    .inspect(&target)
                    .await
                    .map_err(anyhow::Error::new)?
                {
                    ReachOutcome::Supported(reach) => reach,
                    ReachOutcome::Manual(reason) => return Ok(ProbeResponse::Manual { reason }),
                };
                let probe_id = uuid::Uuid::new_v4().to_string();
                let plan = self.layout.plan(
                    ProvisioningOperation::Add,
                    target.transport.clone(),
                    &reach,
                    &probe_id,
                )?;
                let confirmation = plan.confirmation();
                let mut memory = self.memory.lock().await;
                while memory.plan_order.len() >= MAX_PENDING_PLANS {
                    if let Some(expired) = memory.plan_order.pop_front() {
                        memory.plans.remove(&expired);
                    }
                }
                memory.plan_order.push_back(probe_id.clone());
                memory.plans.insert(
                    probe_id.clone(),
                    PendingPlan {
                        plan: plan.clone(),
                        confirmation: PendingConfirmation::Add {
                            registration,
                            original_target: target,
                        },
                    },
                );
                Ok(ProbeResponse::Provisionable {
                    probe_id,
                    plan,
                    confirmation,
                })
            }
        }
    }

    /// Establish the host id and the exact dial configuration proved by the
    /// probe or chosen by a confirmed plan before a run starts.
    async fn register(
        &self,
        registration: &ProbeRegistration,
        plan: Option<&ProvisioningPlan>,
        discovered: Option<DiscoveredDial>,
    ) -> anyhow::Result<HostId> {
        let (host, inserted) = match registration {
            ProbeRegistration::Local => {
                let row = self
                    .store
                    .list_hosts()
                    .await?
                    .into_iter()
                    .find(|row| row.kind == HostKind::Local)
                    .context("the guaranteed local host row is missing")?;
                if let Some(identity) = discovered.and_then(|dial| dial.identity) {
                    match self
                        .store
                        .record_first_contact(row.id, &DialedAs::of(&row), &identity)
                        .await?
                    {
                        FirstContactOutcome::Recorded => {}
                        outcome => bail!(
                            "the local supervisor identity could not be registered: {outcome:?}"
                        ),
                    }
                }
                (row.id, false)
            }
            ProbeRegistration::Ssh {
                destination,
                remote_farhelm,
                remote_state_dir,
            } => {
                let discovered_identity = discovered
                    .as_ref()
                    .and_then(|dial| dial.identity.as_deref());
                let installed_farhelm = plan
                    .map(|plan| plan.farhelm_path.as_path())
                    .or_else(|| discovered.as_ref().map(|dial| dial.farhelm.as_path()));
                let installed_state = plan.map(|plan| plan.state_dir.as_path()).or_else(|| {
                    discovered
                        .as_ref()
                        .and_then(|dial| dial.state_dir.as_deref())
                });
                let farhelm = installed_farhelm
                    .map(path_text)
                    .transpose()?
                    .or_else(|| remote_farhelm.clone());
                let state_dir = installed_state
                    .map(path_text)
                    .transpose()?
                    .or_else(|| remote_state_dir.clone());
                self.store
                    .register_probed_ssh_host(
                        destination,
                        farhelm.as_deref(),
                        state_dir.as_deref(),
                        discovered_identity,
                    )
                    .await?
            }
        };
        let reconciled = async {
            self.sync_registry().await?;
            if !self.manager.retry_now(host).await? {
                bail!("the registered host actor was not available for retry");
            }
            anyhow::Ok(())
        }
        .await;
        if let Err(error) = reconciled {
            if inserted {
                let rollback = self.store.remove_ssh_host(host).await;
                self.manager.stop_actor(host).await;
                if let Err(rollback) = rollback {
                    return Err(error.context(format!(
                        "the new host row could not be reconciled, and rolling it back also failed ({rollback:#})"
                    )));
                }
            }
            return Err(error);
        }
        Ok(host)
    }

    /// Reconcile durable registry changes with the actor set. The test seam
    /// exists to prove that a newly inserted row is rolled back when this
    /// post-commit step fails.
    async fn sync_registry(&self) -> anyhow::Result<()> {
        #[cfg(test)]
        if self
            .fail_registry_sync
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            bail!("planted registry synchronization failure");
        }
        self.manager.sync_registry().await
    }

    /// Consume a confirmed plan exactly once, register first, then start ADD.
    async fn start_add(self: &Arc<Self>, request: ProvisionRequest) -> anyhow::Result<RunAccepted> {
        let mut pending = self.consume_plan(&request.probe_id).await?;
        let PendingConfirmation::Add { registration, .. } = &pending.confirmation else {
            return Err(anyhow::Error::new(ProvisioningRequestError::UnknownPlan));
        };
        let host = self
            .register(registration, Some(&pending.plan), None)
            .await?;
        let registered = registration_for_row(&self.host_row(host).await?)?;
        if let PendingConfirmation::Add { registration, .. } = &mut pending.confirmation {
            *registration = registered;
        }
        self.start_run(host, pending).await
    }

    /// Inspect an existing row and retain one exact UPDATE plan without
    /// changing either the registry or the host.
    async fn plan_update(&self, host: HostId) -> anyhow::Result<UpdatePlanResponse> {
        let row = self.host_row(host).await?;
        self.require_update_trusted(host)
            .map_err(anyhow::Error::new)?;
        let original_target = self.probe_target_for_row(&row);
        let observation = self
            .backend
            .probe(&original_target)
            .await
            .map_err(anyhow::Error::new)?;
        let mut effective_row = row.clone();
        let mut expected_identity = row.host_identity.clone();
        let target = match observation {
            ProbeObservation::Supervisor {
                host_identity,
                dial_farhelm,
                dial_state_dir,
                ..
            } => {
                if let (Some(recorded), Some(reported)) = (&row.host_identity, &host_identity)
                    && recorded != reported
                {
                    return Err(anyhow::Error::new(HostStoreError::IdentityMismatch {
                        host,
                        expected: recorded.clone(),
                        actual: Some(reported.clone()),
                    }));
                }
                expected_identity = expected_identity.or(host_identity);
                if row.kind == HostKind::Ssh {
                    effective_row.remote_farhelm = Some(path_text(&dial_farhelm)?);
                    effective_row.remote_state_dir =
                        dial_state_dir.as_deref().map(path_text).transpose()?;
                }
                ProbeTarget {
                    transport: original_target.transport.clone(),
                    probe_farhelm: dial_farhelm,
                    probe_state_dir: dial_state_dir,
                }
            }
            ProbeObservation::Absent => original_target,
        };
        let reach = match self
            .backend
            .inspect(&target)
            .await
            .map_err(anyhow::Error::new)?
        {
            ReachOutcome::Supported(reach) => reach,
            ReachOutcome::Manual(reason) => bail!(reason),
        };
        let probe_id = uuid::Uuid::new_v4().to_string();
        let plan = self.layout.plan_for_row(
            &effective_row,
            target.transport.clone(),
            &reach,
            &probe_id,
        )?;
        let confirmation = plan.confirmation();
        // Confirmation compares the row the user actually planned from.
        // Any resolved dial coordinates are committed only after that check.
        let registration = registration_for_row(&row)?;
        self.retain_plan(
            probe_id.clone(),
            PendingPlan {
                plan: plan.clone(),
                confirmation: PendingConfirmation::Update {
                    host,
                    target,
                    expected_identity,
                    registration,
                },
            },
        )
        .await;
        Ok(UpdatePlanResponse {
            probe_id,
            plan,
            confirmation,
        })
    }

    /// Consume one host-bound UPDATE plan and only then claim the run.
    async fn start_update(
        self: &Arc<Self>,
        host: HostId,
        request: ProvisionRequest,
    ) -> anyhow::Result<RunAccepted> {
        let pending = self.consume_plan(&request.probe_id).await?;
        if !matches!(
            pending.confirmation,
            PendingConfirmation::Update { host: planned, .. } if planned == host
        ) {
            return Err(anyhow::Error::new(ProvisioningRequestError::UnknownPlan));
        }
        self.start_run(host, pending).await
    }

    async fn consume_plan(&self, probe_id: &str) -> anyhow::Result<PendingPlan> {
        let mut memory = self.memory.lock().await;
        let pending = memory
            .plans
            .remove(probe_id)
            .ok_or_else(|| anyhow::Error::new(ProvisioningRequestError::UnknownPlan))?;
        memory.plan_order.retain(|id| id != probe_id);
        Ok(pending)
    }

    async fn retain_plan(&self, probe_id: String, pending: PendingPlan) {
        let mut memory = self.memory.lock().await;
        while memory.plan_order.len() >= MAX_PENDING_PLANS {
            if let Some(expired) = memory.plan_order.pop_front() {
                memory.plans.remove(&expired);
            }
        }
        memory.plan_order.push_back(probe_id.clone());
        memory.plans.insert(probe_id, pending);
    }

    async fn start_run(
        self: &Arc<Self>,
        host: HostId,
        pending: PendingPlan,
    ) -> anyhow::Result<RunAccepted> {
        let run_id = uuid::Uuid::new_v4().to_string();
        {
            let mut memory = self.memory.lock().await;
            if !memory.busy.insert(host) {
                return Err(anyhow::Error::new(ProvisioningRequestError::Busy(host)));
            }
            memory.runs.insert(
                host,
                ProvisioningView::for_plan(host, run_id.clone(), &pending.plan),
            );
        }
        let host_write = self.manager.host_write_lock(host).await;
        if let Err(error) = self.host_row(host).await {
            let mut memory = self.memory.lock().await;
            memory.busy.remove(&host);
            memory.runs.remove(&host);
            return Err(error);
        }
        self.manager.events().bump();
        let service = Arc::clone(self);
        let task = tokio::spawn(async move {
            let _host_write = host_write;
            let _slot = Arc::clone(&service.run_slots)
                .acquire_owned()
                .await
                .expect("the provisioning semaphore is never closed");
            service.revalidate_and_execute(host, pending).await;
        });
        self.memory.lock().await.tasks.insert(host, task);
        Ok(RunAccepted {
            host_id: host,
            run_id,
        })
    }

    /// Forget process-local progress alongside the durable row. Pending
    /// confirmation tokens for that host are removed as well.
    pub(crate) async fn forget_host(&self, host: HostId) {
        let mut memory = self.memory.lock().await;
        let task = memory.tasks.remove(&host);
        if let Some(task) = &task {
            task.abort();
        }
        memory.runs.remove(&host);
        let removed: std::collections::HashSet<String> = memory
            .plans
            .iter()
            .filter(|(_, pending)| {
                matches!(
                    &pending.confirmation,
                    PendingConfirmation::Update { host: planned, .. } if *planned == host
                )
            })
            .map(|(id, _)| id.clone())
            .collect();
        memory.plans.retain(|id, _| !removed.contains(id));
        memory.plan_order.retain(|id| !removed.contains(id));
        drop(memory);
        if let Some(task) = task {
            let _ = task.await;
        }
    }

    #[cfg(test)]
    async fn abort_run(&self, host: HostId) {
        let task = self.memory.lock().await.tasks.remove(&host);
        if let Some(task) = task {
            task.abort();
            let _ = task.await;
        }
        self.memory.lock().await.busy.remove(&host);
    }

    /// Re-probe after confirmation but before the first mutation. ADD uses
    /// an answering supervisor as-is; UPDATE refuses a changed identity.
    async fn revalidate_and_execute(&self, host: HostId, pending: PendingPlan) {
        let preflight = self.revalidate(host, &pending).await;
        match preflight {
            Ok(Revalidation::Execute) => self.execute(host, pending.plan).await,
            Ok(Revalidation::UseAsIs(message)) => self.finish_use_as_is(host, message).await,
            Err(error) => self.finish_preflight_failure(host, error.rendered()).await,
        }
    }

    async fn revalidate(
        &self,
        host: HostId,
        pending: &PendingPlan,
    ) -> Result<Revalidation, BackendFailure> {
        match &pending.confirmation {
            PendingConfirmation::Add {
                registration,
                original_target,
            } => {
                self.require_registration_unchanged(host, registration)
                    .await?;
                let mut observation = self.backend.probe(original_target).await?;
                let installed_target = ProbeTarget {
                    transport: pending.plan.target.clone(),
                    probe_farhelm: pending.plan.farhelm_path.clone(),
                    probe_state_dir: Some(pending.plan.state_dir.clone()),
                };
                if matches!(observation, ProbeObservation::Absent)
                    && matches!(original_target.transport, ProvisioningTarget::Ssh { .. })
                    && (original_target.probe_farhelm != installed_target.probe_farhelm
                        || original_target.probe_state_dir != installed_target.probe_state_dir)
                {
                    observation = self.backend.probe(&installed_target).await?;
                }
                if let ProbeObservation::Supervisor {
                    build_version,
                    host_identity,
                    dial_farhelm,
                    dial_state_dir,
                } = observation
                {
                    self.register(
                        registration,
                        None,
                        Some(DiscoveredDial {
                            farhelm: dial_farhelm,
                            state_dir: dial_state_dir,
                            identity: host_identity,
                        }),
                    )
                    .await
                    .map_err(|error| {
                        BackendFailure::new(
                            "registering the supervisor found at confirmation",
                            format!("{error:#}"),
                        )
                    })?;
                    return Ok(Revalidation::UseAsIs(format!(
                        "a supervisor answered during confirmation (build {build_version}); ADD used it as-is"
                    )));
                }
                Ok(Revalidation::Execute)
            }
            PendingConfirmation::Update {
                target,
                expected_identity,
                registration,
                ..
            } => {
                self.require_registration_unchanged(host, registration)
                    .await?;
                self.require_update_trusted(host)?;
                let observation = self.backend.probe(target).await?;
                let (plan, discovered) = match observation {
                    ProbeObservation::Supervisor {
                        host_identity,
                        dial_farhelm,
                        dial_state_dir,
                        ..
                    } => {
                        if expected_identity.is_some() && &host_identity != expected_identity {
                            return Err(BackendFailure::new(
                                "the supervisor identity changed after UPDATE planning",
                                format!(
                                    "expected {expected_identity:?}, reported {host_identity:?}; plan again"
                                ),
                            ));
                        }
                        (
                            None,
                            Some(DiscoveredDial {
                                farhelm: dial_farhelm,
                                state_dir: dial_state_dir,
                                identity: host_identity,
                            }),
                        )
                    }
                    ProbeObservation::Absent => (Some(&pending.plan), None),
                };
                self.register(registration, plan, discovered)
                    .await
                    .map_err(|error| {
                        BackendFailure::new(
                            "recording the confirmed UPDATE target",
                            format!("{error:#}"),
                        )
                    })?;
                Ok(Revalidation::Execute)
            }
        }
    }

    async fn require_registration_unchanged(
        &self,
        host: HostId,
        expected: &ProbeRegistration,
    ) -> Result<(), BackendFailure> {
        let row = self.host_row(host).await.map_err(|error| {
            BackendFailure::new("re-reading the confirmed host row", format!("{error:#}"))
        })?;
        let current = registration_for_row(&row).map_err(|error| {
            BackendFailure::new(
                "reading the confirmed host configuration",
                format!("{error:#}"),
            )
        })?;
        if &current != expected {
            return Err(BackendFailure::new(
                "the host configuration changed after the plan was shown",
                "discard this plan and plan again",
            ));
        }
        Ok(())
    }

    /// UPDATE cannot resolve an identity decision that the connection
    /// manager has deliberately frozen for the user. Refusing both while
    /// planning and immediately before mutation closes the confirmation
    /// window without treating a software update as implicit adoption.
    fn require_update_trusted(&self, host: HostId) -> Result<(), BackendFailure> {
        match self.manager.state(host) {
            Some(HostState::IdentityMismatch { recorded, reported }) => Err(BackendFailure::new(
                "UPDATE is refused while the host identity is frozen",
                format!(
                    "recorded {recorded:?}, reported {reported:?}; adopt the identity or fix the destination first"
                ),
            )),
            Some(HostState::Duplicate { twin, identity }) => Err(BackendFailure::new(
                "UPDATE is refused while the host duplicates another registry row",
                format!(
                    "identity {identity:?} belongs to host {twin}; resolve the duplicate first"
                ),
            )),
            _ => Ok(()),
        }
    }

    async fn finish_use_as_is(&self, host: HostId, message: String) {
        let mut memory = self.memory.lock().await;
        if let Some(run) = memory.runs.get_mut(&host) {
            run.status = RunStatus::Completed;
            run.message = Some(message.clone());
            for step in &mut run.steps {
                step.status = StepStatus::Skipped;
                step.message = Some(message.clone());
            }
        }
        memory.busy.remove(&host);
        drop(memory);
        self.manager.events().bump();
    }

    async fn finish_preflight_failure(&self, host: HostId, message: String) {
        let mut memory = self.memory.lock().await;
        if let Some(run) = memory.runs.get_mut(&host) {
            run.status = RunStatus::Failed;
            run.message = Some(message.clone());
            if let Some(step) = run.steps.first_mut() {
                step.status = StepStatus::Failed;
                step.message = Some(message);
            }
        }
        memory.busy.remove(&host);
        drop(memory);
        self.manager.events().bump();
    }

    /// Consume the plan in order, retaining every completed outcome and
    /// stopping at the first failure without rollback.
    async fn execute(&self, host: HostId, plan: ProvisioningPlan) {
        let prepared = match self.prepare_payloads(&plan).await {
            Ok(prepared) => prepared,
            Err((index, error)) => {
                self.fail_action(host, index, &plan.actions[index], error)
                    .await;
                return;
            }
        };
        for (index, action) in plan.actions.iter().enumerate() {
            self.set_step(host, index, StepStatus::Running, None).await;
            let outcome = self
                .execute_action(host, &plan, action, prepared.get(&index))
                .await;
            let (status, message) = match outcome {
                Ok(ActionOutcome::Completed) => (StepStatus::Completed, None),
                Ok(ActionOutcome::Skipped(message)) => (StepStatus::Skipped, Some(message)),
                Ok(ActionOutcome::Degraded(message)) => (StepStatus::Degraded, Some(message)),
                Err(error) => {
                    self.fail_action(host, index, action, error).await;
                    return;
                }
            };
            self.set_step(host, index, status, message).await;
        }
        let mut memory = self.memory.lock().await;
        if let Some(run) = memory.runs.get_mut(&host) {
            run.status = RunStatus::Completed;
            let degraded = run
                .steps
                .iter()
                .any(|step| step.status == StepStatus::Degraded);
            run.message = degraded.then(|| "starts at login, not at boot".to_string());
        }
        memory.busy.remove(&host);
        drop(memory);
        self.manager.events().bump();
    }

    /// Resolve and stage every payload before the first plan action runs.
    /// A missing release artifact is a planning/execution boundary failure,
    /// not permission to leave newly created directories on the host.
    async fn prepare_payloads(
        &self,
        plan: &ProvisioningPlan,
    ) -> Result<HashMap<usize, PreparedPayload>, (usize, BackendFailure)> {
        let mut prepared = HashMap::new();
        for (index, action) in plan.actions.iter().enumerate() {
            let ProvisioningAction::InstallPayload { payload, arch, .. } = action else {
                continue;
            };
            let source = self
                .payloads
                .path(*payload, *arch)
                .map_err(|error| (index, BackendFailure::new(format!("{error:#}"), "")))?;
            let staged = stage_payload(&source)
                .await
                .map_err(|error| (index, error))?;
            prepared.insert(index, staged);
        }
        Ok(prepared)
    }

    /// Retain one failed action and release the host claim without undoing
    /// any earlier completed steps.
    async fn fail_action(
        &self,
        host: HostId,
        index: usize,
        action: &ProvisioningAction,
        error: BackendFailure,
    ) {
        let message = format!(
            "step {} ({}) failed: {}; rerun provisioning to continue",
            index + 1,
            action.label(),
            error.rendered()
        );
        self.set_step(host, index, StepStatus::Failed, Some(message.clone()))
            .await;
        let mut memory = self.memory.lock().await;
        if let Some(run) = memory.runs.get_mut(&host) {
            run.status = RunStatus::Failed;
            run.message = Some(message);
        }
        memory.busy.remove(&host);
        drop(memory);
        self.manager.events().bump();
    }

    /// Dispatch one typed action. Attach is owned here because it joins the
    /// host registry/manager; every host-side action remains in the backend.
    async fn execute_action(
        &self,
        host: HostId,
        plan: &ProvisioningPlan,
        action: &ProvisioningAction,
        prepared: Option<&PreparedPayload>,
    ) -> Result<ActionOutcome, BackendFailure> {
        match action {
            ProvisioningAction::EnsureDirectories { directories } => {
                self.backend
                    .ensure_directories(&plan.target, directories)
                    .await
            }
            ProvisioningAction::InstallPayload {
                payload,
                destination,
                temporary,
                ..
            } => {
                let source = prepared.ok_or_else(|| {
                    BackendFailure::new("the prepared payload disappeared before installation", "")
                })?;
                self.backend
                    .install_path(
                        &plan.target,
                        *payload,
                        source,
                        destination,
                        temporary,
                        0o755,
                    )
                    .await
            }
            ProvisioningAction::WriteUnit {
                content,
                destination,
                temporary,
                ..
            } => {
                self.backend
                    .install_bytes(
                        &plan.target,
                        content.as_bytes(),
                        destination,
                        temporary,
                        0o644,
                    )
                    .await
            }
            ProvisioningAction::DaemonReload => self.backend.daemon_reload(&plan.target).await,
            ProvisioningAction::EnableSupervisor {
                unit, unit_path, ..
            } => self.backend.enable_now(&plan.target, unit, unit_path).await,
            ProvisioningAction::EnableLinger { .. } => {
                self.backend.enable_linger(&plan.target).await
            }
            ProvisioningAction::RestartSupervisor { unit } => {
                self.backend.restart(&plan.target, unit).await
            }
            ProvisioningAction::AttachSupervisor => {
                let previous_incarnation =
                    self.manager.status(host).map(|status| status.incarnation);
                self.manager.retry_now(host).await.map_err(|error| {
                    BackendFailure::new("requesting supervisor attach", error.to_string())
                })?;
                let attached = tokio::time::timeout(ATTACH_TIMEOUT, async {
                    loop {
                        let status = self.manager.status(host)?;
                        if status.state.is_connected()
                            && status.client.is_some()
                            && Some(status.incarnation) != previous_incarnation
                        {
                            return Some(status.state);
                        }
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    }
                })
                .await
                .map_err(|_| {
                    BackendFailure::new("waiting for the provisioned supervisor", "timed out")
                })?;
                attached.ok_or_else(|| {
                    BackendFailure::new(
                        "waiting for the provisioned supervisor",
                        "the registered host actor disappeared",
                    )
                })?;
                Ok(ActionOutcome::Completed)
            }
        }
    }

    /// Publish one progress transition through the fleet's no-data feed.
    async fn set_step(
        &self,
        host: HostId,
        index: usize,
        status: StepStatus,
        message: Option<String>,
    ) {
        let mut memory = self.memory.lock().await;
        if let Some(step) = memory
            .runs
            .get_mut(&host)
            .and_then(|run| run.steps.get_mut(index))
        {
            step.status = status;
            step.message = message;
        }
        drop(memory);
        self.manager.events().bump();
    }

    /// Read the retained run, returning an explicit idle view for a valid
    /// host that has never been provisioned by this helm process.
    async fn view(&self, host: HostId) -> anyhow::Result<ProvisioningView> {
        self.host_row(host).await?;
        Ok(self
            .memory
            .lock()
            .await
            .runs
            .get(&host)
            .cloned()
            .unwrap_or_else(|| ProvisioningView::idle(host)))
    }

    async fn host_row(&self, host: HostId) -> anyhow::Result<HostRow> {
        self.store
            .list_hosts()
            .await?
            .into_iter()
            .find(|row| row.id == host)
            .ok_or_else(|| anyhow::Error::new(HostStoreError::HostNotFound(host)))
    }

    fn probe_target_for_row(&self, row: &HostRow) -> ProbeTarget {
        match row.kind {
            HostKind::Local => ProbeTarget {
                transport: ProvisioningTarget::Local,
                probe_farhelm: self.local_farhelm.clone(),
                probe_state_dir: Some(self.layout.local_state_dir.clone()),
            },
            HostKind::Ssh => ProbeTarget {
                transport: ProvisioningTarget::Ssh {
                    destination: row.destination.clone().expect("ssh row destination"),
                },
                probe_farhelm: PathBuf::from(row.remote_farhelm.as_deref().unwrap_or("farhelm")),
                probe_state_dir: row.remote_state_dir.as_deref().map(PathBuf::from),
            },
        }
    }
}

fn registration_for_row(row: &HostRow) -> anyhow::Result<ProbeRegistration> {
    match row.kind {
        HostKind::Local => Ok(ProbeRegistration::Local),
        HostKind::Ssh => Ok(ProbeRegistration::Ssh {
            destination: row
                .destination
                .clone()
                .context("an ssh host row has no destination")?,
            remote_farhelm: row.remote_farhelm.clone(),
            remote_state_dir: row.remote_state_dir.clone(),
        }),
    }
}

/// `POST /api/hosts/probe` — discover and register as-is, or return the
/// exact plan a later confirmation may consume.
pub(crate) async fn probe_host(
    State(state): State<Arc<AppState>>,
    axum::Json(request): axum::Json<ProbeRequest>,
) -> Response {
    match state.provisioning.probe(request).await {
        Ok(response) => axum::Json(response).into_response(),
        Err(error) => provisioning_error(error),
    }
}

/// `POST /api/hosts/provision` — consume one confirmed ADD plan, register
/// its host first, and return before background convergence finishes.
pub(crate) async fn provision_host(
    State(state): State<Arc<AppState>>,
    axum::Json(request): axum::Json<ProvisionRequest>,
) -> Response {
    match state.provisioning.start_add(request).await {
        Ok(run) => (StatusCode::ACCEPTED, axum::Json(run)).into_response(),
        Err(error) => provisioning_error(error),
    }
}

/// `POST /api/hosts/{id}/update` — plan without a body, then consume the
/// returned opaque plan when the same route receives its confirmation body.
pub(crate) async fn update_host(
    State(state): State<Arc<AppState>>,
    AxPath(host): AxPath<HostId>,
    request: Option<axum::Json<ProvisionRequest>>,
) -> Response {
    match request {
        Some(axum::Json(request)) => match state.provisioning.start_update(host, request).await {
            Ok(run) => (StatusCode::ACCEPTED, axum::Json(run)).into_response(),
            Err(error) => provisioning_error(error),
        },
        None => match state.provisioning.plan_update(host).await {
            Ok(plan) => axum::Json(plan).into_response(),
            Err(error) => provisioning_error(error),
        },
    }
}

/// `GET /api/hosts/{id}/provisioning` — re-read progress after a feed bump.
pub(crate) async fn provisioning_state(
    State(state): State<Arc<AppState>>,
    AxPath(host): AxPath<HostId>,
) -> Response {
    match state.provisioning.view(host).await {
        Ok(view) => axum::Json(view).into_response(),
        Err(error) => http_error(error),
    }
}

/// Map provisioning's own typed refusals without parsing their diagnostics.
fn provisioning_error(error: anyhow::Error) -> Response {
    if error.is::<BackendFailure>() {
        return (StatusCode::BAD_GATEWAY, format!("{error:#}")).into_response();
    }
    if let Some(request) = error.downcast_ref::<ProvisioningRequestError>() {
        let status = match request {
            ProvisioningRequestError::InvalidProbe(_) => StatusCode::BAD_REQUEST,
            ProvisioningRequestError::UnknownPlan | ProvisioningRequestError::Busy(_) => {
                StatusCode::CONFLICT
            }
        };
        return (status, format!("{error:#}")).into_response();
    }
    http_error(error)
}

/// Optional linger is the one host action real transport tests must not run
/// against the developer's account.
#[derive(Clone)]
enum LingerBehavior {
    Real,
    #[cfg(test)]
    Simulated(Result<(), String>),
}

/// Production process and file operations for both supported transports.
struct SystemBackend {
    control_dir: PathBuf,
    linger: LingerBehavior,
    launcher: Arc<dyn CommandLauncher>,
    runtime_units: bool,
    #[cfg(test)]
    fail_before_rename: bool,
}

/// Process-creation seam that tests use to exercise production classifiers
/// without invoking a real SSH endpoint.
trait CommandLauncher: Send + Sync {
    fn spawn(
        &self,
        command: &mut tokio::process::Command,
    ) -> std::io::Result<tokio::process::Child>;
}

/// Ordinary launcher: all lifecycle and stream policy stays with the caller.
struct SystemLauncher;

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
fn isolate_process_group(command: &mut tokio::process::Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.as_std_mut().process_group(0);
    }
}

impl SystemBackend {
    /// Production uses the real optional linger action.
    fn new(control_dir: PathBuf) -> Self {
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
    fn with_simulated_linger(
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
    async fn require_shell(
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
    async fn metadata_on_target(
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
struct CommandResult {
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
struct TargetMetadata {
    hash: String,
    mode: u32,
}

/// One payload snapshot whose digest and install bytes come from the same
/// read. The temporary file stays alive until every plan action is done.
struct PreparedPayload {
    file: tempfile::NamedTempFile,
    hash: String,
}

impl PreparedPayload {
    /// Expose only the private snapshot, never the caller-controlled source
    /// path that may change after preflight.
    fn path(&self) -> &Path {
        self.file.path()
    }
}

/// Copy a payload through a fixed-size buffer while computing the digest of
/// those exact bytes. Later installation reads only this private snapshot,
/// so replacing the source path cannot change what the plan installs.
async fn stage_payload(source: &Path) -> Result<PreparedPayload, BackendFailure> {
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
struct DrainFailure {
    stream: &'static str,
    detail: String,
    prefix: Vec<u8>,
}

/// Probe stderr keeps a bounded diagnostic prefix while scanning every byte
/// for the wrapper records that decide remote absence and resolved dialing.
#[derive(Default)]
struct ProbeStderr {
    prefix: Vec<u8>,
    command_started: bool,
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
async fn drain_probe_stderr<R>(
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
async fn capture_child(
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
}

/// Encode a path as one remote shell word, refusing bytes that cannot cross
/// SSH's text command boundary without changing meaning.
fn shell_path(path: &Path) -> Result<String, BackendFailure> {
    Ok(shell_words::quote(&path_text(path)?).into_owned())
}

/// Preserve a path exactly at every text-only SSH, registry, and systemd
/// boundary. Rejecting before confirmation is safer than displaying one path
/// and later mutating a lossy approximation of it.
fn path_text(path: &Path) -> Result<String, BackendFailure> {
    let text = path.to_str().ok_or_else(|| {
        BackendFailure::new(
            format!("path {} is not valid UTF-8", path.to_string_lossy()),
            "ssh and systemd command paths are text",
        )
    })?;
    if text.chars().any(char::is_control) {
        return Err(BackendFailure::new(
            format!("path {text:?} contains a control character"),
            "",
        ));
    }
    Ok(text.to_string())
}

/// Encode one batch-mode sftp path. Sftp has its own quoting grammar and
/// cannot reuse shell quoting safely.
fn sftp_path(path: &Path) -> Result<String, BackendFailure> {
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
async fn set_mode(path: &Path, mode: u32) -> Result<(), BackendFailure> {
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
async fn set_mode(_path: &Path, _mode: u32) -> Result<(), BackendFailure> {
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
fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        write!(&mut out, "{byte:02x}").expect("writing to String cannot fail");
    }
    out
}

/// Parse tmux's user-facing version line conservatively; anything malformed
/// requests the private payload rather than assuming compatibility.
fn tmux_at_least_3_3(output: &str) -> bool {
    let version = output
        .strip_prefix("tmux ")
        .unwrap_or(output)
        .split(|character: char| !character.is_ascii_digit() && character != '.')
        .next()
        .unwrap_or_default();
    let mut parts = version.split('.');
    let major = parts.next().and_then(|part| part.parse::<u32>().ok());
    let minor = parts.next().and_then(|part| part.parse::<u32>().ok());
    matches!((major, minor), (Some(major), Some(minor)) if (major, minor) >= (3, 3))
}

fn linger_was_refused(code: Option<i32>, stderr: &str) -> bool {
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

fn parse_reach_output(output: &[u8]) -> Result<ReachOutcome, BackendFailure> {
    let fields: Vec<&[u8]> = output.split(|byte| *byte == 0).collect();
    if fields.len() != 9 || fields[0] != REACH_RECORD_MARKER.as_bytes() || !fields[8].is_empty() {
        return Err(BackendFailure::new(
            "the provisioning reach check returned malformed output",
            String::from_utf8_lossy(output),
        ));
    }
    let os = String::from_utf8_lossy(fields[1]);
    if os != "ubuntu" {
        return Ok(ReachOutcome::Manual(format!(
            "automatic provisioning supports Ubuntu only; this host reported /etc/os-release ID={os:?}. Run the supervisor manually."
        )));
    }
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
        tmux_at_least_3_3(&tmux) && tmux_path.is_absolute() && path_text(&tmux_path).is_ok();
    Ok(ReachOutcome::Supported(Reach {
        home,
        user_unit_dir,
        arch,
        needs_tmux: !tmux_ok,
        tmux_dir: tmux_ok
            .then(|| tmux_path.parent().map(Path::to_path_buf))
            .flatten(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rest_harness::{FleetBuilder, Harness, HostScript};
    use axum::body::{Body, to_bytes};
    use std::sync::Mutex;
    use tower::ServiceExt;

    /// A payload source whose bytes are irrelevant to the fake executor.
    struct FixedPayloads(PathBuf);

    impl PayloadSource for FixedPayloads {
        fn path(&self, _payload: PayloadKind, _arch: PayloadArch) -> anyhow::Result<PathBuf> {
            Ok(self.0.clone())
        }
    }

    /// A release fixture with Farhelm present but the required tmux artifact
    /// missing, used to prove whole-plan payload preflight.
    struct MissingTmuxPayload {
        farhelm: PathBuf,
        requested: Mutex<Vec<PayloadKind>>,
    }

    impl PayloadSource for MissingTmuxPayload {
        fn path(&self, payload: PayloadKind, _arch: PayloadArch) -> anyhow::Result<PathBuf> {
            self.requested.lock().unwrap().push(payload);
            match payload {
                PayloadKind::Farhelm => Ok(self.farhelm.clone()),
                PayloadKind::Tmux => bail!("the tmux payload is missing"),
            }
        }
    }

    struct ScriptLauncher {
        scripts: Mutex<VecDeque<String>>,
        programs: Mutex<Vec<String>>,
    }

    impl ScriptLauncher {
        fn new(scripts: impl IntoIterator<Item = String>) -> Arc<Self> {
            Arc::new(Self {
                scripts: Mutex::new(scripts.into_iter().collect()),
                programs: Mutex::new(Vec::new()),
            })
        }
    }

    impl CommandLauncher for ScriptLauncher {
        fn spawn(
            &self,
            command: &mut tokio::process::Command,
        ) -> std::io::Result<tokio::process::Child> {
            self.programs.lock().unwrap().push(
                command
                    .as_std()
                    .get_program()
                    .to_string_lossy()
                    .into_owned(),
            );
            let script = self
                .scripts
                .lock()
                .unwrap()
                .pop_front()
                .expect("one scripted child per launch");
            let mut scripted = tokio::process::Command::new("sh");
            scripted
                .args(["-c", &script])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            isolate_process_group(&mut scripted);
            scripted.spawn()
        }
    }

    struct RecordingLauncher {
        programs: Mutex<Vec<String>>,
    }

    impl RecordingLauncher {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                programs: Mutex::new(Vec::new()),
            })
        }
    }

    impl CommandLauncher for RecordingLauncher {
        fn spawn(
            &self,
            command: &mut tokio::process::Command,
        ) -> std::io::Result<tokio::process::Child> {
            self.programs.lock().unwrap().push(
                command
                    .as_std()
                    .get_program()
                    .to_string_lossy()
                    .into_owned(),
            );
            command.spawn()
        }
    }

    fn frame_script(frame: farhelm_proto::Frame) -> String {
        let mut bytes = Vec::new();
        frame.encode(&mut bytes).expect("encode scripted frame");
        let octal = bytes
            .iter()
            .map(|byte| format!("\\{byte:03o}"))
            .collect::<String>();
        format!("printf '{octal}'")
    }

    fn test_system_backend(root: &Path) -> SystemBackend {
        SystemBackend {
            control_dir: root.to_path_buf(),
            linger: LingerBehavior::Simulated(Ok(())),
            launcher: Arc::new(SystemLauncher),
            runtime_units: false,
            fail_before_rename: false,
        }
    }

    /// Exercise the production install path with the same immutable payload
    /// snapshot the orchestration layer supplies during a real run.
    async fn install_test_payload(
        backend: &SystemBackend,
        target: &ProvisioningTarget,
        source: &Path,
        destination: &Path,
        temporary: &Path,
        mode: u32,
    ) -> Result<ActionOutcome, BackendFailure> {
        let prepared = stage_payload(source).await?;
        backend
            .install_path(
                target,
                PayloadKind::Farhelm,
                &prepared,
                destination,
                temporary,
                mode,
            )
            .await
    }

    /// Recorder for orchestration tests, including planted failure, linger,
    /// registration-order, and in-flight barriers.
    struct FakeBackend {
        probe: Mutex<Option<Result<ProbeObservation, BackendFailure>>>,
        reach: Mutex<ReachOutcome>,
        inspect_failure: Mutex<Option<String>>,
        operations: Mutex<Vec<String>>,
        fail: Mutex<Option<String>>,
        skip: Mutex<Option<String>>,
        linger: Mutex<Option<Result<(), String>>>,
        registration: Mutex<Option<(HelmStore, String)>>,
        entered: tokio::sync::Notify,
        release: tokio::sync::Notify,
        block_first: std::sync::atomic::AtomicBool,
        stateful: std::sync::atomic::AtomicBool,
    }

    impl FakeBackend {
        fn absent(home: PathBuf) -> Arc<Self> {
            Arc::new(Self {
                probe: Mutex::new(Some(Ok(ProbeObservation::Absent))),
                reach: Mutex::new(ReachOutcome::Supported(Reach {
                    user_unit_dir: home.join(".config/systemd/user"),
                    home,
                    arch: PayloadArch::X86_64,
                    needs_tmux: false,
                    tmux_dir: Some(PathBuf::from("/usr/bin")),
                })),
                inspect_failure: Mutex::new(None),
                operations: Mutex::new(Vec::new()),
                fail: Mutex::new(None),
                skip: Mutex::new(None),
                linger: Mutex::new(Some(Ok(()))),
                registration: Mutex::new(None),
                entered: tokio::sync::Notify::new(),
                release: tokio::sync::Notify::new(),
                block_first: std::sync::atomic::AtomicBool::new(false),
                stateful: std::sync::atomic::AtomicBool::new(false),
            })
        }

        fn supervisor(home: PathBuf) -> Arc<Self> {
            let backend = Self::absent(home.clone());
            *backend.probe.lock().unwrap() = Some(Ok(ProbeObservation::Supervisor {
                build_version: "test-build".to_string(),
                host_identity: Some("test-identity".to_string()),
                dial_farhelm: home.join("farhelm"),
                dial_state_dir: Some(home.join("state")),
            }));
            backend
        }

        fn failing_probe(home: PathBuf, message: &str) -> Arc<Self> {
            let backend = Self::absent(home);
            *backend.probe.lock().unwrap() =
                Some(Err(BackendFailure::new("the ssh probe failed", message)));
            backend
        }

        fn manual(home: PathBuf, reason: &str) -> Arc<Self> {
            let backend = Self::absent(home);
            *backend.reach.lock().unwrap() = ReachOutcome::Manual(reason.to_string());
            backend
        }

        fn record(&self, operation: &str) -> Result<ActionOutcome, BackendFailure> {
            self.operations.lock().unwrap().push(operation.to_string());
            if self.fail.lock().unwrap().as_deref() == Some(operation) {
                return Err(BackendFailure::new(
                    format!("planted failure in {operation}"),
                    "\u{1b}[31mhost said no\nnext",
                ));
            }
            if self.skip.lock().unwrap().as_deref() == Some(operation) {
                return Ok(ActionOutcome::Skipped(format!(
                    "planted skip in {operation}"
                )));
            }
            Ok(ActionOutcome::Completed)
        }
    }

    #[async_trait]
    impl ProvisioningBackend for FakeBackend {
        async fn probe(&self, _target: &ProbeTarget) -> Result<ProbeObservation, BackendFailure> {
            self.probe
                .lock()
                .unwrap()
                .take()
                .unwrap_or(Ok(ProbeObservation::Absent))
        }

        async fn inspect(&self, _target: &ProbeTarget) -> Result<ReachOutcome, BackendFailure> {
            if let Some(message) = self.inspect_failure.lock().unwrap().take() {
                return Err(BackendFailure::new("planted inspection failure", message));
            }
            Ok(self.reach.lock().unwrap().clone())
        }

        async fn ensure_directories(
            &self,
            _target: &ProvisioningTarget,
            directories: &[DirectorySpec],
        ) -> Result<ActionOutcome, BackendFailure> {
            let registration = self.registration.lock().unwrap().take();
            if let Some((store, destination)) = registration {
                let registered = store
                    .list_hosts()
                    .await
                    .unwrap()
                    .iter()
                    .any(|row| row.destination.as_deref() == Some(destination.as_str()));
                assert!(
                    registered,
                    "the host row must exist before the first action"
                );
            }
            if self.stateful.load(std::sync::atomic::Ordering::SeqCst) {
                for directory in directories {
                    tokio::fs::create_dir_all(&directory.path)
                        .await
                        .map_err(|error| {
                            BackendFailure::new(
                                format!("creating stateful fixture {}", directory.path.display()),
                                error.to_string(),
                            )
                        })?;
                    set_mode(&directory.path, directory.mode).await?;
                }
            }
            let result = self.record("create-directories")?;
            if self
                .block_first
                .swap(false, std::sync::atomic::Ordering::SeqCst)
            {
                self.entered.notify_one();
                self.release.notified().await;
            }
            Ok(result)
        }

        async fn install_path(
            &self,
            _target: &ProvisioningTarget,
            kind: PayloadKind,
            payload: &PreparedPayload,
            destination: &Path,
            _temporary: &Path,
            _mode: u32,
        ) -> Result<ActionOutcome, BackendFailure> {
            if self.stateful.load(std::sync::atomic::Ordering::SeqCst) {
                tokio::fs::copy(payload.path(), destination)
                    .await
                    .map_err(|error| {
                        BackendFailure::new(
                            format!("installing stateful fixture {}", destination.display()),
                            error.to_string(),
                        )
                    })?;
            }
            self.record(match kind {
                PayloadKind::Farhelm => "install-farhelm",
                PayloadKind::Tmux => "install-tmux",
            })
        }

        async fn install_bytes(
            &self,
            _target: &ProvisioningTarget,
            content: &[u8],
            destination: &Path,
            _temporary: &Path,
            _mode: u32,
        ) -> Result<ActionOutcome, BackendFailure> {
            if self.stateful.load(std::sync::atomic::Ordering::SeqCst) {
                tokio::fs::write(destination, content)
                    .await
                    .map_err(|error| {
                        BackendFailure::new(
                            format!("writing stateful fixture {}", destination.display()),
                            error.to_string(),
                        )
                    })?;
            }
            self.record("write-unit")
        }

        async fn daemon_reload(
            &self,
            _target: &ProvisioningTarget,
        ) -> Result<ActionOutcome, BackendFailure> {
            self.record("daemon-reload")
        }

        async fn enable_now(
            &self,
            _target: &ProvisioningTarget,
            _unit: &str,
            _unit_path: &Path,
        ) -> Result<ActionOutcome, BackendFailure> {
            self.record("enable-supervisor")
        }

        async fn enable_linger(
            &self,
            _target: &ProvisioningTarget,
        ) -> Result<ActionOutcome, BackendFailure> {
            self.record("enable-linger")?;
            match self.linger.lock().unwrap().take().unwrap_or(Ok(())) {
                Ok(()) => Ok(ActionOutcome::Completed),
                Err(message) => Ok(ActionOutcome::Degraded(format!(
                    "linger was refused ({message}); starts at login, not at boot"
                ))),
            }
        }

        async fn restart(
            &self,
            _target: &ProvisioningTarget,
            _unit: &str,
        ) -> Result<ActionOutcome, BackendFailure> {
            self.record("restart-supervisor")
        }
    }

    /// Build an isolated scripted helm whose local actor can reconnect after
    /// provisioning's attach step. No process, unit, or real user path is
    /// involved in these contract tests.
    async fn harness() -> Harness {
        let harness = FleetBuilder::new()
            .await
            .local(HostScript::default())
            .await
            .start()
            .await;
        let local = harness.store.list_hosts().await.unwrap()[0].id;
        harness.await_refreshed(local).await;
        harness
    }

    fn layout(root: &Path) -> PlanLayout {
        PlanLayout {
            local_state_dir: root.join("state"),
            override_lib_dir: Some(root.join("lib")),
            override_farhelm_path: None,
            override_state_dir: Some(root.join("state")),
            override_unit_dir: Some(root.join("units")),
            unit_name: format!("farhelm-provisioning-test-{}.service", uuid::Uuid::new_v4()),
        }
    }

    fn service(
        harness: &Harness,
        backend: Arc<FakeBackend>,
        root: &Path,
    ) -> Arc<ProvisioningService> {
        let payload = root.join("payload");
        std::fs::write(&payload, b"farhelm test payload").expect("write fixed payload fixture");
        ProvisioningService::injected(
            harness.store.clone(),
            Arc::clone(&harness.manager),
            backend,
            Arc::new(FixedPayloads(payload)),
            layout(root),
            PathBuf::from("/test/farhelm"),
        )
    }

    async fn wait_finished(service: &ProvisioningService, host: HostId) -> ProvisioningView {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let view = service.view(host).await.unwrap();
                if matches!(view.status, RunStatus::Completed | RunStatus::Failed) {
                    return view;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("provisioning run did not finish")
    }

    /// A successful discovery is use-as-is: it creates the SSH registry row
    /// and returns the peer's hello without constructing an install plan.
    #[tokio::test]
    async fn probe_registers_an_answering_supervisor_as_is() {
        let (builder, expected_host) = FleetBuilder::new()
            .await
            .ssh(
                "user@example",
                HostScript {
                    reachable: false,
                    identity: Some("test-identity".to_string()),
                    ..HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        harness
            .await_state(expected_host, |state| {
                matches!(state, HostState::Unreachable { .. })
            })
            .await;
        harness.fleet.edit(expected_host, |script| {
            script.reachable = true;
        });
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::supervisor(root.path().to_path_buf());
        let provisioner = service(&harness, backend.clone(), root.path());
        let response = provisioner
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "user@example".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        assert!(matches!(
            response,
            ProbeResponse::Discovered {
                host_id,
                build_version,
                host_identity: Some(identity),
                ..
            } if host_id == expected_host
                && build_version == "test-build"
                && identity == "test-identity"
        ));
        assert!(
            harness
                .store
                .list_hosts()
                .await
                .unwrap()
                .iter()
                .any(|row| row.destination.as_deref() == Some("user@example"))
        );
        let row = harness
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.destination.as_deref() == Some("user@example"))
            .unwrap();
        assert_eq!(
            row.remote_farhelm.as_deref(),
            Some(root.path().join("farhelm").to_str().unwrap())
        );
        assert_eq!(
            row.remote_state_dir.as_deref(),
            Some(root.path().join("state").to_str().unwrap())
        );
        assert_eq!(row.host_identity.as_deref(), Some("test-identity"));
        harness
            .await_refreshed_as(expected_host, "test-identity", 0)
            .await;
        let client = harness
            .manager
            .status(expected_host)
            .and_then(|status| status.client)
            .expect("discovery must leave an operable manager client");
        assert!(client.list_sessions().await.unwrap().sessions.is_empty());
        assert!(backend.operations.lock().unwrap().is_empty());
    }

    /// Positive absence may offer a plan, but probing itself must not create
    /// a registry row, directory, unit, or payload transfer.
    #[tokio::test]
    async fn positive_absence_returns_a_concrete_plan_without_touching_the_host() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let service = service(&harness, backend.clone(), root.path());
        let before = harness.store.list_hosts().await.unwrap();
        let response = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "user@absent".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable {
            probe_id,
            plan,
            confirmation,
        } = response
        else {
            panic!("positive absence must offer provisioning")
        };
        assert_eq!(harness.store.list_hosts().await.unwrap(), before);
        assert!(backend.operations.lock().unwrap().is_empty());
        assert!(confirmation.contains("starts at boot if linger succeeds"));
        assert!(confirmation.contains("starts at login, not at boot"));
        let ProvisioningAction::WriteUnit { unit, .. } = &plan.actions[2] else {
            panic!("the third action must write the unit")
        };
        let farhelm = root.path().join("lib/farhelm");
        let unit_path = root.path().join("units").join(unit);
        let expected = format!(
            "Farhelm will perform these steps for user@absent:\n\
             1. create or reuse directories {} (mode 0755), {} (mode 0700), {} (mode 0755)\n\
             2. install Farhelm at {} via temporary file {} and atomic rename\n\
             3. write user unit {unit} at {} via temporary file {} and atomic rename\n\
             4. reload the systemd user manager\n\
             5. enable and start {unit}; the supervisor runs persistently under the systemd user manager\n\
             6. optionally enable linger: the supervisor starts at boot if linger succeeds; if privilege is refused, continue and report that it starts at login, not at boot\n\
             7. dial the supervisor and attach it to the already-registered host row\n",
            root.path().join("lib").display(),
            root.path().join("state").display(),
            root.path().join("units").display(),
            farhelm.display(),
            root.path()
                .join(format!("lib/.farhelm.farhelm-tmp-{probe_id}"))
                .display(),
            unit_path.display(),
            root.path()
                .join("units")
                .join(format!(".{unit}.farhelm-tmp-{probe_id}"))
                .display(),
        );
        assert_eq!(confirmation, expected);
    }

    /// Unsupported hosts stay on the manual-run path without retaining
    /// confirmation authority or touching either registry or executor.
    #[tokio::test]
    async fn manual_fallback_retains_nothing_and_mutates_nothing() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::manual(root.path().to_path_buf(), "unsupported fixture");
        let service = service(&harness, backend.clone(), root.path());
        let before = harness.store.list_hosts().await.unwrap();

        let response = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "manual.example".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();

        assert!(matches!(
            response,
            ProbeResponse::Manual { reason } if reason == "unsupported fixture"
        ));
        assert_eq!(harness.store.list_hosts().await.unwrap(), before);
        assert!(backend.operations.lock().unwrap().is_empty());
        let memory = service.memory.lock().await;
        assert!(memory.plans.is_empty());
        assert!(memory.runs.is_empty());
        assert!(memory.busy.is_empty());
    }

    /// A development build resolves its absent payload before directories or
    /// any other host state changes, then releases the host for another run.
    #[tokio::test]
    async fn no_payloads_fails_before_the_first_mutation_and_releases_the_host() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let service = ProvisioningService::injected(
            harness.store.clone(),
            Arc::clone(&harness.manager),
            backend.clone(),
            Arc::new(NoPayloads),
            layout(root.path()),
            PathBuf::from("/test/farhelm"),
        );

        for attempt in 0..2 {
            let response = service
                .probe(ProbeRequest {
                    target: ProbeDestination::Local,
                    remote_farhelm: None,
                    remote_state_dir: None,
                })
                .await
                .unwrap();
            let ProbeResponse::Provisionable { probe_id, .. } = response else {
                panic!("attempt {attempt} must retain a plan")
            };
            let accepted = service
                .start_add(ProvisionRequest { probe_id })
                .await
                .expect("the released host must admit the retry");
            let failed = wait_finished(&service, accepted.host_id).await;
            assert_eq!(failed.status, RunStatus::Failed);
            assert!(failed.message.as_deref().is_some_and(|message| {
                message.contains("this build carries no provisioning payloads")
            }));
            assert_eq!(failed.steps[0].status, StepStatus::Pending);
            assert_eq!(failed.steps[1].status, StepStatus::Failed);
            assert!(backend.operations.lock().unwrap().is_empty());
            assert!(service.memory.lock().await.busy.is_empty());
        }
    }

    /// Preflight visits every required payload before mutating, even when an
    /// earlier payload exists and only the private-tmux artifact is missing.
    #[tokio::test]
    async fn payload_preflight_validates_the_complete_required_set() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let farhelm = root.path().join("farhelm-payload");
        tokio::fs::write(&farhelm, b"farhelm").await.unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        {
            let mut reach = backend.reach.lock().unwrap();
            let ReachOutcome::Supported(reach) = &mut *reach else {
                panic!("fixture must be supported")
            };
            reach.needs_tmux = true;
            reach.tmux_dir = None;
        }
        let payloads = Arc::new(MissingTmuxPayload {
            farhelm,
            requested: Mutex::new(Vec::new()),
        });
        let service = ProvisioningService::injected(
            harness.store.clone(),
            Arc::clone(&harness.manager),
            backend.clone(),
            Arc::clone(&payloads) as Arc<dyn PayloadSource>,
            layout(root.path()),
            PathBuf::from("/test/farhelm"),
        );
        let response = service
            .probe(ProbeRequest {
                target: ProbeDestination::Local,
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { probe_id, .. } = response else {
            panic!("missing tmux still yields a confirmable plan")
        };
        let accepted = service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        let failed = wait_finished(&service, accepted.host_id).await;
        assert_eq!(failed.status, RunStatus::Failed);
        assert!(failed.message.unwrap().contains("tmux payload is missing"));
        assert_eq!(
            payloads.requested.lock().unwrap().as_slice(),
            [PayloadKind::Farhelm, PayloadKind::Tmux]
        );
        assert!(backend.operations.lock().unwrap().is_empty());
    }

    /// A failed post-insert actor reconciliation rolls back only the new row,
    /// leaving the consumed confirmation recoverable through a fresh probe.
    #[tokio::test]
    async fn failed_registry_sync_rolls_back_a_newly_registered_host() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let service = service(&harness, backend.clone(), root.path());
        let response = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "rollback.example".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { probe_id, .. } = response else {
            panic!("absence must produce a plan")
        };
        service
            .fail_registry_sync
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let error = service
            .start_add(ProvisionRequest { probe_id })
            .await
            .expect_err("the planted reconciliation failure must surface");
        assert!(
            error
                .to_string()
                .contains("planted registry synchronization failure")
        );
        assert!(
            harness
                .store
                .list_hosts()
                .await
                .unwrap()
                .iter()
                .all(|row| row.destination.as_deref() != Some("rollback.example"))
        );
        assert!(backend.operations.lock().unwrap().is_empty());

        let retry = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "rollback.example".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        assert!(matches!(retry, ProbeResponse::Provisionable { .. }));
    }

    /// Confirming an already-registered but absent SSH host replaces stale
    /// dial coordinates with the installation paths before attachment.
    #[tokio::test]
    async fn confirmed_add_repairs_stale_registered_paths() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let host = harness
            .store
            .add_ssh_host(
                "stale.example",
                Some("/stale/farhelm"),
                Some("/stale/state"),
            )
            .await
            .unwrap();
        harness.manager.sync_registry().await.unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let service = service(&harness, backend, root.path());
        let response = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "stale.example".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { probe_id, .. } = response else {
            panic!("the stopped registered host must offer recovery")
        };
        let accepted = service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        assert_eq!(accepted.host_id, host);
        assert_eq!(
            wait_finished(&service, host).await.status,
            RunStatus::Completed
        );
        let repaired = service.host_row(host).await.unwrap();
        assert_eq!(
            repaired.remote_farhelm.as_deref(),
            root.path().join("lib/farhelm").to_str()
        );
        assert_eq!(
            repaired.remote_state_dir.as_deref(),
            root.path().join("state").to_str()
        );
    }

    /// A remote literally named `local` remains an SSH destination; transport
    /// choice comes from the request tag rather than hostname spelling.
    #[tokio::test]
    async fn local_hostname_does_not_alias_the_local_transport() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let service = service(&harness, backend, root.path());
        let response = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "local".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { plan, .. } = response else {
            panic!("the absent SSH host must produce a plan")
        };
        assert!(matches!(
            plan.target,
            ProvisioningTarget::Ssh { destination } if destination == "local"
        ));
    }

    /// UPDATE previews the installation coordinates already stored on the
    /// row and leaves both registry and host untouched until confirmation.
    #[tokio::test]
    async fn update_plan_uses_registered_paths_without_mutation() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let farhelm = root.path().join("custom/bin/farhelm");
        let state_dir = root.path().join("custom/state");
        let host = harness
            .store
            .add_ssh_host("custom.example", farhelm.to_str(), state_dir.to_str())
            .await
            .unwrap();
        harness.manager.sync_registry().await.unwrap();
        let before = harness.store.list_hosts().await.unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let service = service(&harness, backend.clone(), root.path());

        let preview = service.plan_update(host).await.unwrap();

        assert_eq!(preview.plan.farhelm_path, farhelm);
        assert_eq!(preview.plan.state_dir, state_dir);
        assert_eq!(harness.store.list_hosts().await.unwrap(), before);
        assert!(backend.operations.lock().unwrap().is_empty());
    }

    /// Manual and backend planning failures retain no UPDATE claim; once
    /// inspection recovers, the same host can immediately produce a plan.
    #[tokio::test]
    async fn update_planning_failures_release_the_host_for_retry() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let host = harness.store.list_hosts().await.unwrap()[0].id;
        let backend = FakeBackend::manual(root.path().to_path_buf(), "manual only");
        let service = service(&harness, backend.clone(), root.path());

        assert!(service.plan_update(host).await.is_err());
        let idle = service.view(host).await.unwrap();
        assert_eq!(idle.status, RunStatus::Completed);
        assert!(idle.run_id.is_none());
        *backend.reach.lock().unwrap() = ReachOutcome::Supported(Reach {
            home: root.path().to_path_buf(),
            user_unit_dir: root.path().join("units"),
            arch: PayloadArch::X86_64,
            needs_tmux: false,
            tmux_dir: Some(PathBuf::from("/usr/bin")),
        });
        *backend.inspect_failure.lock().unwrap() = Some("inspection broke".to_string());
        assert!(service.plan_update(host).await.is_err());
        assert!(service.memory.lock().await.busy.is_empty());

        let recovered = service.plan_update(host).await.unwrap();
        assert!(!recovered.probe_id.is_empty());
    }

    /// UPDATE is not an identity-resolution mechanism: both frozen mismatch
    /// and duplicate rows are refused before host inspection begins.
    #[tokio::test]
    async fn update_refuses_identity_freeze_states() {
        let (builder, mismatch_host) = FleetBuilder::new()
            .await
            .ssh(
                "mismatch.example",
                HostScript {
                    identity: Some("identity-old".to_string()),
                    ..HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        harness
            .await_refreshed_as(mismatch_host, "identity-old", 0)
            .await;
        let harness = harness
            .restart_with(|fleet| {
                fleet.edit(mismatch_host, |script| {
                    script.identity = Some("identity-new".to_string());
                });
            })
            .await;
        harness
            .await_state(mismatch_host, |state| {
                matches!(state, HostState::IdentityMismatch { .. })
            })
            .await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let mismatch_service = service(&harness, backend.clone(), root.path());
        let error = mismatch_service
            .plan_update(mismatch_host)
            .await
            .expect_err("identity mismatch must refuse UPDATE");
        assert!(error.to_string().contains("identity is frozen"));
        assert!(backend.operations.lock().unwrap().is_empty());

        let shared = || HostScript {
            identity: Some("shared-identity".to_string()),
            ..HostScript::default()
        };
        let (builder, first) = FleetBuilder::new()
            .await
            .ssh("first.example", shared())
            .await;
        let (builder, second) = builder.ssh("second.example", shared()).await;
        let harness = builder.start().await;
        let duplicate = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                for host in [first, second] {
                    if matches!(
                        harness.manager.state(host),
                        Some(HostState::Duplicate { .. })
                    ) {
                        return host;
                    }
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("one shared identity row must freeze as duplicate");
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let service = service(&harness, backend.clone(), root.path());
        let error = service
            .plan_update(duplicate)
            .await
            .expect_err("duplicate identity must refuse UPDATE");
        assert!(
            error
                .to_string()
                .contains("duplicates another registry row")
        );
        assert!(backend.operations.lock().unwrap().is_empty());
    }

    /// UPDATE binds the confirmed mutation to the identity observed during
    /// planning and refuses a different answer in the final preflight.
    #[tokio::test]
    async fn confirmed_update_rechecks_identity_before_mutation() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let host = harness
            .store
            .add_ssh_host("identity.example", None, None)
            .await
            .unwrap();
        let row = harness
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == host)
            .unwrap();
        harness
            .store
            .record_first_contact(host, &DialedAs::of(&row), "expected-identity")
            .await
            .unwrap();
        harness.manager.sync_registry().await.unwrap();
        let backend = FakeBackend::supervisor(root.path().to_path_buf());
        *backend.probe.lock().unwrap() = Some(Ok(ProbeObservation::Supervisor {
            build_version: "planned".to_string(),
            host_identity: Some("expected-identity".to_string()),
            dial_farhelm: root.path().join("lib/farhelm"),
            dial_state_dir: Some(root.path().join("state")),
        }));
        let service = service(&harness, backend.clone(), root.path());
        let plan = service.plan_update(host).await.unwrap();
        *backend.probe.lock().unwrap() = Some(Ok(ProbeObservation::Supervisor {
            build_version: "changed".to_string(),
            host_identity: Some("other-identity".to_string()),
            dial_farhelm: root.path().join("lib/farhelm"),
            dial_state_dir: Some(root.path().join("state")),
        }));
        let accepted = service
            .start_update(
                host,
                ProvisionRequest {
                    probe_id: plan.probe_id,
                },
            )
            .await
            .unwrap();
        let failed = wait_finished(&service, accepted.host_id).await;
        assert_eq!(failed.status, RunStatus::Failed);
        assert!(failed.message.unwrap().contains("identity changed"));
        assert!(backend.operations.lock().unwrap().is_empty());
    }

    /// Confirmation closes the absence race: a supervisor that starts after
    /// planning is registered and used as-is without any plan action.
    #[tokio::test]
    async fn confirmed_add_reprobes_and_uses_a_new_supervisor_as_is() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let service = service(&harness, backend.clone(), root.path());
        let response = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "race.example".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { probe_id, .. } = response else {
            panic!("initial absence must return a plan")
        };
        *backend.probe.lock().unwrap() = Some(Ok(ProbeObservation::Supervisor {
            build_version: "late-build".to_string(),
            host_identity: Some("late-identity".to_string()),
            dial_farhelm: root.path().join("late/farhelm"),
            dial_state_dir: Some(root.path().join("late/state")),
        }));
        let accepted = service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        let view = wait_finished(&service, accepted.host_id).await;
        assert_eq!(view.status, RunStatus::Completed);
        assert!(
            view.steps
                .iter()
                .all(|step| step.status == StepStatus::Skipped)
        );
        assert!(backend.operations.lock().unwrap().is_empty());
        let row = service.host_row(accepted.host_id).await.unwrap();
        assert_eq!(row.host_identity.as_deref(), Some("late-identity"));
        assert_eq!(
            row.remote_farhelm.as_deref(),
            Some(root.path().join("late/farhelm").to_str().unwrap())
        );
    }

    /// An SSH/auth failure is an error with the host's diagnostic, never the
    /// absence result that unlocks setup.
    #[tokio::test]
    async fn ssh_failure_never_offers_provisioning() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::failing_probe(root.path().to_path_buf(), "Permission denied");
        let service = service(&harness, backend, root.path());
        let before = harness.store.list_hosts().await.unwrap();
        let error = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "user@denied".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .expect_err("an SSH failure must not become an offer");
        assert!(format!("{error:#}").contains("Permission denied"));
        assert_eq!(harness.store.list_hosts().await.unwrap(), before);
    }

    /// Confirmation registers the destination before the first plan action,
    /// and a second operation for that host is refused while the first waits.
    #[tokio::test]
    async fn registration_precedes_the_run_and_busy_hosts_refuse_a_second_run() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        backend
            .block_first
            .store(true, std::sync::atomic::Ordering::SeqCst);
        *backend.fail.lock().unwrap() = Some("daemon-reload".to_string());
        *backend.registration.lock().unwrap() =
            Some((harness.store.clone(), "user@ordered".to_string()));
        let service = service(&harness, backend.clone(), root.path());
        let first = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "user@ordered".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { probe_id, .. } = first else {
            panic!("expected plan")
        };
        let accepted = service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        backend.entered.notified().await;
        let second = service
            .probe(ProbeRequest {
                target: ProbeDestination::Ssh {
                    destination: "user@ordered".to_string(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { probe_id, .. } = second else {
            panic!("expected second plan")
        };
        let error = service
            .start_add(ProvisionRequest { probe_id })
            .await
            .expect_err("the in-flight host must refuse another run");
        assert!(error.to_string().contains("in flight"));
        let progress = service.view(accepted.host_id).await.unwrap();
        assert_eq!(progress.run_id.as_deref(), Some(accepted.run_id.as_str()));
        backend.release.notify_one();
        let _ = wait_finished(&service, accepted.host_id).await;
    }

    /// Failure names the exact action and retains control-escaped host stderr;
    /// completed actions are not rolled back.
    #[tokio::test]
    async fn a_failing_step_is_reported_with_escaped_host_stderr() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        backend
            .stateful
            .store(true, std::sync::atomic::Ordering::SeqCst);
        *backend.fail.lock().unwrap() = Some("daemon-reload".to_string());
        let service = service(&harness, backend.clone(), root.path());
        let response = service
            .probe(ProbeRequest {
                target: ProbeDestination::Local,
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { probe_id, .. } = response else {
            panic!("expected plan")
        };
        let accepted = service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        let view = wait_finished(&service, accepted.host_id).await;
        assert_eq!(view.status, RunStatus::Failed);
        let message = view.message.unwrap();
        assert!(message.contains("step 4 (daemon-reload) failed"));
        assert!(message.contains("\\u{1b}[31mhost said no\\nnext"));
        assert_eq!(
            backend.operations.lock().unwrap().as_slice(),
            [
                "create-directories",
                "install-farhelm",
                "write-unit",
                "daemon-reload"
            ]
        );
        assert!(root.path().join("lib").is_dir());
        assert!(root.path().join("state").is_dir());
        assert_eq!(
            std::fs::read(root.path().join("lib/farhelm")).unwrap(),
            b"farhelm test payload"
        );
        let unit_paths = std::fs::read_dir(root.path().join("units"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        assert_eq!(unit_paths.len(), 1);
        assert!(
            std::fs::read_to_string(&unit_paths[0])
                .unwrap()
                .contains("KillMode=process")
        );

        *backend.fail.lock().unwrap() = None;
        let retry = service
            .probe(ProbeRequest {
                target: ProbeDestination::Local,
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { probe_id, .. } = retry else {
            panic!("the failed run must release the host for recovery")
        };
        let accepted = service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        assert_eq!(
            wait_finished(&service, accepted.host_id).await.status,
            RunStatus::Completed
        );
    }

    /// Linger success keeps the boot promise; privilege refusal is a
    /// completed degradation with the exact login-only wording.
    #[tokio::test]
    async fn conditional_linger_reports_both_outcomes() {
        for refused in [false, true] {
            let harness = harness().await;
            let root = tempfile::tempdir().unwrap();
            let backend = FakeBackend::absent(root.path().to_path_buf());
            *backend.linger.lock().unwrap() = Some(if refused {
                Err("permission denied".to_string())
            } else {
                Ok(())
            });
            let service = service(&harness, backend, root.path());
            let response = service
                .probe(ProbeRequest {
                    target: ProbeDestination::Local,
                    remote_farhelm: None,
                    remote_state_dir: None,
                })
                .await
                .unwrap();
            let ProbeResponse::Provisionable { probe_id, .. } = response else {
                panic!("expected plan")
            };
            let accepted = service
                .start_add(ProvisionRequest { probe_id })
                .await
                .unwrap();
            let view = wait_finished(&service, accepted.host_id).await;
            assert_eq!(view.status, RunStatus::Completed);
            assert_eq!(
                view.message.as_deref(),
                refused.then_some("starts at login, not at boot")
            );
        }
    }

    /// Probe and confirmed ADD share the host-scoped progress reader:
    /// provision returns 202 with an identity, then GET exposes that run.
    #[tokio::test]
    async fn rest_surface_starts_and_rereads_a_host_scoped_run() {
        let mut harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let service = service(&harness, backend, root.path());
        harness.state = Arc::new(AppState::with_provisioning(
            Arc::clone(&harness.manager),
            harness.store.clone(),
            Arc::clone(&service),
        ));
        let router = harness.router();
        let probe = router
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/hosts/probe")
                    .header("host", "127.0.0.1:7433")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"target":{"kind":"local"}}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(probe.status(), StatusCode::OK);
        let probe: serde_json::Value =
            serde_json::from_slice(&to_bytes(probe.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        let probe_id = probe["probe_id"].as_str().unwrap();
        let provision = router
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/hosts/provision")
                    .header("host", "127.0.0.1:7433")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({ "probe_id": probe_id }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(provision.status(), StatusCode::ACCEPTED);
        let accepted: serde_json::Value =
            serde_json::from_slice(&to_bytes(provision.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        let host = accepted["host_id"].as_i64().unwrap();
        let run = accepted["run_id"].as_str().unwrap().to_string();
        let finished = wait_finished(&service, host).await;
        assert_eq!(finished.status, RunStatus::Completed);
        let progress = router
            .oneshot(
                axum::http::Request::builder()
                    .method("GET")
                    .uri(format!("/api/hosts/{host}/provisioning"))
                    .header("host", "127.0.0.1:7433")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(progress.status(), StatusCode::OK);
        let progress: serde_json::Value =
            serde_json::from_slice(&to_bytes(progress.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(progress["run_id"], run);
        assert_eq!(progress["status"], "completed");
    }

    /// A valid host with no retained operation has a stable, explicit idle
    /// JSON shape rather than masquerading as missing progress.
    #[tokio::test]
    async fn progress_route_returns_the_idle_shape_before_any_run() {
        let mut harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let service = service(
            &harness,
            FakeBackend::absent(root.path().to_path_buf()),
            root.path(),
        );
        harness.state = Arc::new(AppState::with_provisioning(
            Arc::clone(&harness.manager),
            harness.store.clone(),
            service,
        ));
        let host = harness.store.list_hosts().await.unwrap()[0].id;
        let response = harness
            .router()
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/api/hosts/{host}/provisioning"))
                    .header("host", "127.0.0.1:7433")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let progress: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(progress["host_id"], host);
        assert!(progress["run_id"].is_null());
        assert!(progress["operation"].is_null());
        assert_eq!(progress["status"], "completed");
        assert_eq!(progress["steps"], serde_json::json!([]));
        assert_eq!(
            progress["message"],
            "no provisioning run has been recorded for this host"
        );
    }

    /// UPDATE uses the router-level plan/confirm handshake, returns a 202 run
    /// identity only after confirmation, exposes progress, and refuses a
    /// second confirmed plan while the host is busy.
    #[tokio::test]
    async fn update_route_plans_confirms_and_enforces_in_flight_exclusion() {
        let mut harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        let service = service(&harness, backend.clone(), root.path());
        let state = Arc::new(AppState::with_provisioning(
            Arc::clone(&harness.manager),
            harness.store.clone(),
            Arc::clone(&service),
        ));
        harness.state = Arc::clone(&state);
        let router = harness.router();
        let host = harness.store.list_hosts().await.unwrap()[0].id;

        async fn plan(router: &axum::Router, host: HostId) -> String {
            let response = router
                .clone()
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri(format!("/api/hosts/{host}/update"))
                        .header("host", "127.0.0.1:7433")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let value: serde_json::Value =
                serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                    .unwrap();
            value["probe_id"].as_str().unwrap().to_string()
        }

        let first = plan(&router, host).await;
        let second = plan(&router, host).await;
        assert!(
            backend.operations.lock().unwrap().is_empty(),
            "UPDATE planning mutated the host before confirmation"
        );
        backend
            .block_first
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let confirm = |probe_id: &str| {
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/api/hosts/{host}/update"))
                .header("host", "127.0.0.1:7433")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "probe_id": probe_id }).to_string(),
                ))
                .unwrap()
        };
        let accepted = router.clone().oneshot(confirm(&first)).await.unwrap();
        assert_eq!(accepted.status(), StatusCode::ACCEPTED);
        let accepted: serde_json::Value =
            serde_json::from_slice(&to_bytes(accepted.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(accepted["host_id"], host);
        backend.entered.notified().await;
        let busy = router.clone().oneshot(confirm(&second)).await.unwrap();
        assert_eq!(busy.status(), StatusCode::CONFLICT);
        let progress = router
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/api/hosts/{host}/provisioning"))
                    .header("host", "127.0.0.1:7433")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let progress: serde_json::Value =
            serde_json::from_slice(&to_bytes(progress.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(progress["operation"], "update");
        assert_eq!(progress["status"], "running");
        backend.release.notify_one();
        assert_eq!(
            wait_finished(&service, host).await.status,
            RunStatus::Completed
        );
    }

    /// Host removal waits behind an in-flight run's write authority, then
    /// purges retained progress and unconsumed UPDATE confirmations with the
    /// durable row instead of leaving process-local ghosts.
    #[tokio::test]
    async fn removal_serializes_with_runs_and_purges_provisioning_memory() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let host = harness
            .store
            .add_ssh_host("remove.example", Some("farhelm"), Some("/tmp/state"))
            .await
            .unwrap();
        harness.manager.sync_registry().await.unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        backend
            .block_first
            .store(true, std::sync::atomic::Ordering::SeqCst);
        *backend.fail.lock().unwrap() = Some("daemon-reload".to_string());
        let service = service(&harness, backend.clone(), root.path());
        let first = service.plan_update(host).await.unwrap();
        let retained = service.plan_update(host).await.unwrap();
        let _accepted = service
            .start_update(
                host,
                ProvisionRequest {
                    probe_id: first.probe_id,
                },
            )
            .await
            .unwrap();
        backend.entered.notified().await;
        let state = Arc::new(AppState::with_provisioning(
            Arc::clone(&harness.manager),
            harness.store.clone(),
            Arc::clone(&service),
        ));
        let mut removal = tokio::spawn(async move {
            crate::hosts::remove_host(State(state), AxPath(host))
                .await
                .into_response()
        });
        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut removal)
                .await
                .is_err(),
            "removal bypassed the in-flight host write authority"
        );

        backend.release.notify_one();
        assert_eq!(removal.await.unwrap().status(), StatusCode::OK);
        let memory = service.memory.lock().await;
        assert!(!memory.runs.contains_key(&host));
        assert!(!memory.busy.contains(&host));
        assert!(!memory.plans.contains_key(&retained.probe_id));
        drop(memory);
        assert!(
            harness
                .store
                .list_hosts()
                .await
                .unwrap()
                .iter()
                .all(|row| row.id != host)
        );
    }

    /// Typed provisioning failures keep their router-level status mapping:
    /// invalid input 400, consumed plans and busy hosts 409, backend failures
    /// 502, and missing hosts 404.
    #[tokio::test]
    async fn router_maps_all_provisioning_error_classes() {
        let mut harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::failing_probe(root.path().to_path_buf(), "denied");
        let service = service(&harness, backend, root.path());
        let state = Arc::new(AppState::with_provisioning(
            Arc::clone(&harness.manager),
            harness.store.clone(),
            service,
        ));
        harness.state = Arc::clone(&state);
        let router = harness.router();
        let post_probe = |body: serde_json::Value| {
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/hosts/probe")
                .header("host", "127.0.0.1:7433")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap()
        };
        let invalid = router
            .clone()
            .oneshot(post_probe(serde_json::json!({
                "target": { "kind": "ssh", "destination": "-bad" }
            })))
            .await
            .unwrap();
        assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
        let backend = router
            .clone()
            .oneshot(post_probe(serde_json::json!({
                "target": { "kind": "ssh", "destination": "host" }
            })))
            .await
            .unwrap();
        assert_eq!(backend.status(), StatusCode::BAD_GATEWAY);
        let unknown = router
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/hosts/provision")
                    .header("host", "127.0.0.1:7433")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"probe_id":"unknown"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unknown.status(), StatusCode::CONFLICT);
        let missing = router
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/hosts/999999/update")
                    .header("host", "127.0.0.1:7433")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn tmux_floor_accepts_suffixes_but_not_older_or_malformed_versions() {
        assert!(tmux_at_least_3_3("tmux 3.3a"));
        assert!(tmux_at_least_3_3("tmux 4.0"));
        assert!(!tmux_at_least_3_3("tmux 3.2"));
        assert!(!tmux_at_least_3_3("not installed"));
    }

    /// The production classifier requires both the private marker and exit
    /// 75; authentication failure, unmarked 75, malformed hello, and skew
    /// remain errors even when process creation itself succeeds.
    #[tokio::test]
    async fn production_probe_classifier_distinguishes_positive_absence() {
        let root = tempfile::tempdir().unwrap();
        let local_target = ProbeTarget {
            transport: ProvisioningTarget::Local,
            probe_farhelm: PathBuf::from("scripted-local"),
            probe_state_dir: None,
        };
        let mut local_backend = test_system_backend(root.path());
        local_backend.launcher = ScriptLauncher::new(["exit 75".to_string()]);
        assert!(matches!(
            local_backend.probe(&local_target).await,
            Ok(ProbeObservation::Absent)
        ));

        let target = ProbeTarget {
            transport: ProvisioningTarget::Ssh {
                destination: "scripted.example".to_string(),
            },
            probe_farhelm: PathBuf::from("farhelm"),
            probe_state_dir: None,
        };
        let malformed = farhelm_proto::Frame {
            kind: farhelm_proto::FrameKind::Control,
            channel: 0,
            body: br#"{"not":"hello"}"#.to_vec(),
        };
        let skew = farhelm_proto::Frame::control(&ControlMsg::Hello {
            protocol_version: farhelm_proto::PROTOCOL_VERSION + 1,
            build_version: "future".to_string(),
            role: "supervisor".to_string(),
            host_identity: None,
            auth: None,
        });
        let cases = [
            (
                format!("printf '%s\\n' '{REMOTE_PROBE_MARKER}' >&2; exit 75"),
                true,
            ),
            (
                "printf 'Permission denied\\n' >&2; exit 255".to_string(),
                false,
            ),
            (
                format!(
                    "head -c {} /dev/zero | tr '\\0' x >&2; \
                     printf '\\n%s\\n' '{REMOTE_PROBE_MARKER}' >&2; exit 75",
                    MAX_CHILD_STREAM_BYTES + 4096
                ),
                true,
            ),
            ("exit 75".to_string(), false),
            (
                format!(
                    "printf '%s\\n' '{REMOTE_PROBE_MARKER}' >&2; {}; exit 75",
                    frame_script(malformed)
                ),
                false,
            ),
            (frame_script(skew), false),
        ];
        for (script, absent) in cases {
            let launcher = ScriptLauncher::new([script]);
            let backend = SystemBackend {
                control_dir: root.path().to_path_buf(),
                linger: LingerBehavior::Simulated(Ok(())),
                launcher,
                runtime_units: false,
                fail_before_rename: false,
            };
            let result = backend.probe(&target).await;
            assert_eq!(
                matches!(result, Ok(ProbeObservation::Absent)),
                absent,
                "unexpected classifier result: {result:?}"
            );
            if !absent {
                assert!(result.is_err());
            }
        }
    }

    /// Child capture fails closed on either unbounded peer output or a
    /// deadline, returning only after the offending process has been killed
    /// and reaped.
    #[tokio::test]
    async fn child_capture_bounds_output_and_reaps_timeouts() {
        let spawn = |script: &str| {
            let mut command = tokio::process::Command::new("sh");
            command
                .args(["-c", script])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            isolate_process_group(&mut command);
            command.spawn().unwrap()
        };

        let overflow = capture_child(
            spawn("head -c 70000 /dev/zero"),
            Duration::from_secs(5),
            "overflow fixture",
        )
        .await
        .expect_err("output beyond the cap must terminate the child");
        assert!(overflow.context.contains("exceeded"));

        let timeout = capture_child(
            spawn("sleep 300"),
            Duration::from_millis(20),
            "timeout fixture",
        )
        .await
        .expect_err("a timed-out child must be terminated");
        assert!(timeout.context.contains("timed out"));
    }

    /// Probe diagnostics stop retaining bytes at the cap but keep draining,
    /// so an oversized producer reaches EOF and a later marker is observed.
    #[tokio::test]
    async fn oversized_probe_stderr_never_blocks_its_producer() {
        let (mut producer, consumer) = tokio::io::duplex(1024);
        let bytes = vec![b'x'; MAX_CHILD_STREAM_BYTES + 4096];
        let writer = tokio::spawn(async move {
            producer.write_all(&bytes).await.unwrap();
            producer.write_all(b"\n").await.unwrap();
            producer
                .write_all(REMOTE_PROBE_MARKER.as_bytes())
                .await
                .unwrap();
            producer.write_all(b"\n").await.unwrap();
            producer.shutdown().await.unwrap();
        });
        let (signal, mut failures) = tokio::sync::mpsc::unbounded_channel();
        let drained =
            tokio::time::timeout(Duration::from_secs(2), drain_probe_stderr(consumer, signal))
                .await
                .expect("the bounded reader must continue draining to EOF");
        tokio::time::timeout(Duration::from_secs(2), writer)
            .await
            .expect("the oversized producer must finish")
            .unwrap();
        assert_eq!(drained.prefix.len(), MAX_CHILD_STREAM_BYTES);
        assert!(drained.command_started);
        assert!(failures.try_recv().is_err());
    }

    /// Silent command failures still identify whether the host returned an
    /// exit code or was terminated by a signal.
    #[tokio::test]
    async fn required_shell_diagnostics_always_include_termination_status() {
        let root = tempfile::tempdir().unwrap();
        let backend = test_system_backend(root.path());
        let exited = backend
            .require_shell(&ProvisioningTarget::Local, "exit 7", "silent failure")
            .await
            .expect_err("the nonzero shell must fail");
        assert!(exited.rendered().contains("exit status 7"));

        #[cfg(unix)]
        {
            let signalled = backend
                .require_shell(
                    &ProvisioningTarget::Local,
                    "kill -TERM $$",
                    "signal failure",
                )
                .await
                .expect_err("the signalled shell must fail");
            assert!(signalled.rendered().contains("terminated by signal 15"));
        }
    }

    /// Reach parsing refuses malformed fields and unsupported platforms,
    /// selects both payload architectures, and requests tmux when absent or
    /// below the supported floor.
    #[test]
    fn reach_output_parser_covers_platform_and_tool_boundaries() {
        let supported = |os: &str, arch: &str, tmux_path: &str, tmux: &str, manager: &str| {
            format!(
                "{REACH_RECORD_MARKER}\0{os}\0/home/test\0{arch}\0{tmux_path}\0{tmux}\0{manager}\0/home/test/.config/systemd/user\0"
            )
        };
        assert!(matches!(
            parse_reach_output(
                supported("debian", "x86_64", "/usr/bin/tmux", "tmux 3.3", "usable").as_bytes()
            )
            .unwrap(),
            ReachOutcome::Manual(_)
        ));
        assert!(matches!(
            parse_reach_output(supported("ubuntu", "riscv64", "", "", "usable").as_bytes())
                .unwrap(),
            ReachOutcome::Manual(_)
        ));
        assert!(parse_reach_output(b"too\0short\0").is_err());
        let clean = supported("ubuntu", "x86_64", "/usr/bin/tmux", "tmux 3.4", "usable");
        assert!(parse_reach_output(format!("shell banner{clean}").as_bytes()).is_err());
        assert!(parse_reach_output(format!("{clean}trailing output").as_bytes()).is_err());
        assert!(matches!(
            parse_reach_output(supported("ubuntu", "x86_64", "", "", "unavailable").as_bytes())
                .unwrap(),
            ReachOutcome::Manual(_)
        ));
        let ReachOutcome::Supported(missing) =
            parse_reach_output(supported("ubuntu", "x86_64", "", "", "usable").as_bytes()).unwrap()
        else {
            panic!("x86_64 Ubuntu with a user manager is supported")
        };
        assert!(missing.needs_tmux);
        let ReachOutcome::Supported(old) = parse_reach_output(
            supported("ubuntu", "aarch64", "/usr/bin/tmux", "tmux 3.2", "usable").as_bytes(),
        )
        .unwrap() else {
            panic!("aarch64 Ubuntu with a user manager is supported")
        };
        assert_eq!(old.arch, PayloadArch::Aarch64);
        assert!(old.needs_tmux);
        let ReachOutcome::Supported(relative_tmux) = parse_reach_output(
            supported("ubuntu", "x86_64", "bin/tmux", "tmux 3.4", "usable").as_bytes(),
        )
        .unwrap() else {
            panic!("an otherwise supported host remains provisionable")
        };
        assert!(relative_tmux.needs_tmux);
        assert_eq!(
            relative_tmux.user_unit_dir,
            PathBuf::from("/home/test/.config/systemd/user")
        );
        assert!(matches!(
            parse_reach_output(
                supported("ubuntu", "x86_64", "/usr/bin/tmux", "tmux 3.4", "unsupported-xdg")
                    .as_bytes()
            )
            .unwrap(),
            ReachOutcome::Manual(reason) if reason.contains("XDG_CONFIG_HOME")
        ));
        #[cfg(unix)]
        {
            let mut non_utf8_home = format!("{REACH_RECORD_MARKER}\0ubuntu\0/home/").into_bytes();
            non_utf8_home.push(0xff);
            non_utf8_home.extend_from_slice(
                b"\0x86_64\0/usr/bin/tmux\0tmux 3.4\0usable\0/home/test/.config/systemd/user\0",
            );
            assert!(matches!(
                parse_reach_output(&non_utf8_home).unwrap(),
                ReachOutcome::Manual(reason)
                    if reason.contains("HOME") && reason.contains("explicit paths")
            ));
        }
    }

    /// A host needing private tmux receives that payload before the unit, and
    /// the rendered unit searches the isolated payload directory first.
    #[test]
    fn tmux_payload_plan_is_concrete_and_ordered() {
        let root = tempfile::tempdir().unwrap();
        let plan = layout(root.path())
            .plan(
                ProvisioningOperation::Add,
                ProvisioningTarget::Ssh {
                    destination: "host".to_string(),
                },
                &Reach {
                    home: root.path().join("home"),
                    user_unit_dir: root.path().join("home/.config/systemd/user"),
                    arch: PayloadArch::X86_64,
                    needs_tmux: true,
                    tmux_dir: None,
                },
                "nonce",
            )
            .unwrap();
        assert!(matches!(
            &plan.actions[2],
            ProvisioningAction::InstallPayload {
                payload: PayloadKind::Tmux,
                arch: PayloadArch::X86_64,
                destination,
                ..
            } if destination == &root.path().join("lib/tmux")
        ));
        let ProvisioningAction::WriteUnit { content, .. } = &plan.actions[3] else {
            panic!("the unit follows both payloads")
        };
        assert!(content.contains(&format!("PATH={}", root.path().join("lib").display())));
    }

    /// Production layout has no test overrides: local and SSH plans use the
    /// documented library, state, unit, and nonce-scoped temporary paths.
    #[test]
    fn production_layout_uses_the_exact_deployment_paths() {
        let home = PathBuf::from("/home/provisioned");
        let user_units = PathBuf::from("/xdg/systemd/user");
        let local_state = PathBuf::from("/helm/state");
        let reach = Reach {
            home: home.clone(),
            user_unit_dir: user_units.clone(),
            arch: PayloadArch::X86_64,
            needs_tmux: false,
            tmux_dir: Some(PathBuf::from("/usr/bin")),
        };
        let layout = PlanLayout::production(local_state.clone());

        for (target, expected_state) in [
            (ProvisioningTarget::Local, local_state),
            (
                ProvisioningTarget::Ssh {
                    destination: "host.example".to_string(),
                },
                home.join(".local/state/farhelm"),
            ),
        ] {
            let plan = layout
                .plan(ProvisioningOperation::Add, target, &reach, "nonce")
                .unwrap();
            let lib = home.join(".local/lib/farhelm");
            let farhelm = lib.join("farhelm");
            let unit = user_units.join("farhelm-supervisor.service");
            assert_eq!(plan.farhelm_path, farhelm);
            assert_eq!(plan.state_dir, expected_state);
            let ProvisioningAction::EnsureDirectories { directories } = &plan.actions[0] else {
                panic!("production plan must begin with directories")
            };
            assert_eq!(directories[0].path, lib);
            assert_eq!(directories[1].path, expected_state);
            assert_eq!(directories[2].path, user_units);
            assert!(matches!(
                &plan.actions[1],
                ProvisioningAction::InstallPayload { destination, temporary, .. }
                    if destination == &farhelm
                        && temporary == &lib.join(".farhelm.farhelm-tmp-nonce")
            ));
            assert!(matches!(
                &plan.actions[2],
                ProvisioningAction::WriteUnit { unit: name, destination, temporary, .. }
                    if name == "farhelm-supervisor.service"
                        && destination == &unit
                        && temporary == &user_units.join(
                            ".farhelm-supervisor.service.farhelm-tmp-nonce"
                        )
            ));
        }
    }

    /// Execution resolves each install action through its own payload kind,
    /// so the private-tmux branch cannot receive the Farhelm executable.
    #[tokio::test]
    async fn tmux_install_uses_the_distinct_tmux_fixture() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        {
            let mut outcome = backend.reach.lock().unwrap();
            let ReachOutcome::Supported(reach) = &mut *outcome else {
                panic!("the fixture starts supported")
            };
            reach.needs_tmux = true;
            reach.tmux_dir = None;
        }
        std::fs::write(root.path().join("farhelm-payload"), b"farhelm").unwrap();
        std::fs::write(root.path().join("tmux-payload"), b"tmux").unwrap();
        let payloads = Arc::new(MutablePayload {
            farhelm: Mutex::new(root.path().join("farhelm-payload")),
            tmux: root.path().join("tmux-payload"),
        });
        let service = ProvisioningService::injected(
            harness.store.clone(),
            Arc::clone(&harness.manager),
            backend.clone(),
            payloads,
            layout(root.path()),
            PathBuf::from("/test/farhelm"),
        );
        let response = service
            .probe(ProbeRequest {
                target: ProbeDestination::Local,
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { probe_id, .. } = response else {
            panic!("the missing private tmux must produce a plan")
        };
        let accepted = service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        assert_eq!(
            wait_finished(&service, accepted.host_id).await.status,
            RunStatus::Completed
        );
        let operations = backend.operations.lock().unwrap();
        assert!(
            operations
                .iter()
                .any(|operation| operation == "install-farhelm")
        );
        assert!(
            operations
                .iter()
                .any(|operation| operation == "install-tmux")
        );
    }

    /// Pending plans retain exactly the newest 64 opaque tokens.
    #[tokio::test]
    async fn pending_plan_cache_evicts_only_the_oldest_at_65() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let service = service(
            &harness,
            FakeBackend::absent(root.path().to_path_buf()),
            root.path(),
        );
        let mut ids = Vec::new();
        for index in 0..65 {
            let response = service
                .probe(ProbeRequest {
                    target: ProbeDestination::Ssh {
                        destination: format!("host-{index}.example"),
                    },
                    remote_farhelm: None,
                    remote_state_dir: None,
                })
                .await
                .unwrap();
            let ProbeResponse::Provisionable { probe_id, .. } = response else {
                panic!("absence must retain a plan")
            };
            ids.push(probe_id);
        }
        assert!(service.consume_plan(&ids[0]).await.is_err());
        assert!(service.consume_plan(&ids[1]).await.is_ok());
        assert!(service.consume_plan(ids.last().unwrap()).await.is_ok());
    }

    /// Systemd specifiers are escaped literally and unrepresentable paths
    /// fail before a plan can be offered.
    #[test]
    fn systemd_rendering_is_fallible_and_escapes_percent() {
        let unit = supervisor_unit(
            Path::new("/tmp/%h/farhelm"),
            Path::new("/tmp/state"),
            Path::new("/tmp/%h"),
            None,
        )
        .unwrap();
        assert!(unit.contains("/tmp/%%h/farhelm"));
        assert!(unit.contains("PATH=/tmp/%%h:"));
        assert!(unit.contains("KillMode=process"));
        assert!(
            supervisor_unit(
                Path::new("/tmp/farhelm"),
                Path::new("/tmp/state\nother"),
                Path::new("/tmp"),
                None,
            )
            .is_err()
        );
        assert!(
            supervisor_unit(
                Path::new("/tmp/farhelm"),
                Path::new("/tmp/state"),
                Path::new("/tmp/with:colon"),
                None,
            )
            .is_err()
        );
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            let non_utf8 = PathBuf::from(std::ffi::OsString::from_vec(vec![
                b'/', b't', b'm', b'p', b'/', 0xff,
            ]));
            assert!(
                supervisor_unit(
                    Path::new("/tmp/farhelm"),
                    &non_utf8,
                    Path::new("/tmp"),
                    None,
                )
                .is_err()
            );
        }
    }

    /// Systemd argument escaping handles each special character in its own
    /// grammar rather than borrowing shell quoting rules.
    #[test]
    fn systemd_argument_rendering_covers_every_supported_escape() {
        for (path, expected) in [
            ("/tmp/a b", "\"/tmp/a b\""),
            ("/tmp/a\"b", "\"/tmp/a\\\"b\""),
            ("/tmp/a\\b", "\"/tmp/a\\\\b\""),
            ("/tmp/%h", "\"/tmp/%%h\""),
        ] {
            assert_eq!(systemd_arg(Path::new(path)).unwrap(), expected);
        }
    }

    /// Remote shell words round-trip hostile but representable paths and
    /// reject bytes that cannot cross the text command boundary unchanged.
    #[test]
    fn remote_shell_path_encoding_round_trips_and_rejects_text_hazards() {
        for path in ["/tmp/a b", "/tmp/it's-here", "-leading-dash"] {
            let encoded = shell_path(Path::new(path)).unwrap();
            let output = std::process::Command::new("sh")
                .args(["-c", &format!("set -- {encoded}; printf '%s' \"$1\"")])
                .output()
                .unwrap();
            assert!(output.status.success());
            assert_eq!(output.stdout, path.as_bytes());
        }
        assert!(shell_path(Path::new("/tmp/line\nbreak")).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            let path = PathBuf::from(std::ffi::OsString::from_vec(vec![b'/', 0xff]));
            assert!(shell_path(&path).is_err());
        }
    }

    /// SFTP batch paths use double-quote escaping of their own and reject
    /// record-breaking bytes before a batch is written.
    #[test]
    fn sftp_batch_path_encoding_has_an_independent_grammar() {
        for (path, expected) in [
            ("/tmp/a b", "\"/tmp/a b\""),
            ("/tmp/a\"b", "\"/tmp/a\\\"b\""),
            ("/tmp/a\\b", "\"/tmp/a\\\\b\""),
        ] {
            assert_eq!(sftp_path(Path::new(path)).unwrap(), expected);
        }
        for rejected in ["/tmp/a\nb", "/tmp/a\rb", "/tmp/a\0b"] {
            assert!(sftp_path(Path::new(rejected)).is_err());
        }
    }

    /// The locale-stable production linger classifier degrades only known
    /// authorization refusals; unrelated command failures remain fatal.
    #[test]
    fn linger_classifier_separates_refusal_from_failure() {
        assert!(linger_was_refused(Some(1), "Access denied"));
        assert!(linger_was_refused(
            Some(1),
            "Interactive authentication required"
        ));
        assert!(!linger_was_refused(Some(1), "Failed to connect to bus"));
        assert!(!linger_was_refused(Some(0), "Access denied"));
    }

    /// Remote absence has a dedicated exit while inspection failures retain
    /// stderr instead of being collapsed into `None`.
    #[tokio::test]
    async fn remote_metadata_distinguishes_absence_from_hash_failure() {
        let root = tempfile::tempdir().unwrap();
        let target = ProvisioningTarget::Ssh {
            destination: "scripted.example".to_string(),
        };
        let absent = ScriptLauncher::new(["exit 44".to_string()]);
        let mut backend = test_system_backend(root.path());
        backend.launcher = absent;
        assert!(
            backend
                .metadata_on_target(&target, Path::new("/missing"))
                .await
                .unwrap()
                .is_none()
        );

        let broken =
            ScriptLauncher::new(["printf 'sha256sum missing\\n' >&2; exit 127".to_string()]);
        backend.launcher = broken;
        let error = backend
            .metadata_on_target(&target, Path::new("/unreadable"))
            .await
            .expect_err("hash failures are not absence");
        assert!(error.rendered().contains("sha256sum missing"));
    }

    /// Local convergence streams hashes, installs one immutable payload
    /// snapshot, refuses unsafe temporary state, and preserves installed
    /// bytes across every pre-rename failure.
    #[tokio::test]
    async fn local_atomic_install_converges_binary_and_unit_content() {
        let root = tempfile::tempdir().unwrap();
        let mut backend = test_system_backend(root.path());
        let target = ProvisioningTarget::Local;
        for (name, mode) in [("farhelm", 0o755), ("unit.service", 0o644)] {
            let destination = root.path().join(name);
            let temporary = root.path().join(format!(".{name}.tmp"));
            let source = root.path().join(format!("{name}.source"));
            tokio::fs::write(&source, b"first").await.unwrap();
            install_test_payload(&backend, &target, &source, &destination, &temporary, mode)
                .await
                .unwrap();
            set_mode(&destination, 0o600).await.unwrap();
            assert!(matches!(
                install_test_payload(&backend, &target, &source, &destination, &temporary, mode)
                    .await
                    .unwrap(),
                ActionOutcome::Skipped(_)
            ));
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                tokio::fs::metadata(&destination)
                    .await
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o7777,
                mode
            );
            tokio::fs::write(&source, b"second").await.unwrap();
            install_test_payload(&backend, &target, &source, &destination, &temporary, mode)
                .await
                .unwrap();
            assert_eq!(tokio::fs::read(&destination).await.unwrap(), b"second");
            tokio::fs::write(&source, b"third").await.unwrap();
            backend.fail_before_rename = true;
            assert!(
                install_test_payload(&backend, &target, &source, &destination, &temporary, mode)
                    .await
                    .is_err()
            );
            backend.fail_before_rename = false;
            assert_eq!(tokio::fs::read(&destination).await.unwrap(), b"second");
            assert!(!temporary.exists());
        }

        let source = root.path().join("snapshot-source");
        let destination = root.path().join("snapshot-destination");
        let temporary = root.path().join(".snapshot-destination.tmp");
        tokio::fs::write(&source, b"validated bytes").await.unwrap();
        let prepared = stage_payload(&source).await.unwrap();
        tokio::fs::write(&source, b"replacement bytes")
            .await
            .unwrap();
        backend
            .install_path(
                &target,
                PayloadKind::Farhelm,
                &prepared,
                &destination,
                &temporary,
                0o755,
            )
            .await
            .unwrap();
        assert_eq!(
            tokio::fs::read(&destination).await.unwrap(),
            b"validated bytes"
        );

        #[cfg(unix)]
        {
            let victim = root.path().join("symlink-victim");
            let destination = root.path().join("symlink-destination");
            let temporary = root.path().join(".symlink-destination.tmp");
            let source = root.path().join("symlink-source");
            tokio::fs::write(&victim, b"victim").await.unwrap();
            tokio::fs::write(&source, b"installed").await.unwrap();
            std::os::unix::fs::symlink(&victim, &temporary).unwrap();
            install_test_payload(&backend, &target, &source, &destination, &temporary, 0o755)
                .await
                .unwrap();
            assert_eq!(tokio::fs::read(&victim).await.unwrap(), b"victim");
            assert_eq!(tokio::fs::read(&destination).await.unwrap(), b"installed");
        }

        let blocked_temporary = root.path().join("blocked-temporary");
        let blocked_destination = root.path().join("blocked-destination");
        let blocked_source = root.path().join("blocked-source");
        tokio::fs::create_dir(&blocked_temporary).await.unwrap();
        tokio::fs::write(&blocked_destination, b"preserved")
            .await
            .unwrap();
        tokio::fs::write(&blocked_source, b"replacement")
            .await
            .unwrap();
        let error = install_test_payload(
            &backend,
            &target,
            &blocked_source,
            &blocked_destination,
            &blocked_temporary,
            0o755,
        )
        .await
        .expect_err("a non-file temporary cannot be ignored as absent");
        assert!(error.context.contains("removing temporary file"));
        assert_eq!(
            tokio::fs::read(&blocked_destination).await.unwrap(),
            b"preserved"
        );

        let large = root.path().join("large-installed-artifact");
        let large_bytes = vec![b'x'; PAYLOAD_COPY_BUFFER * 3 + 17];
        tokio::fs::write(&large, &large_bytes).await.unwrap();
        let metadata = backend
            .metadata_on_target(&target, &large)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(metadata.hash, hex_sha256(&large_bytes));

        let unit = root.path().join("content.service");
        let unit_temporary = root.path().join(".content.service.tmp");
        backend
            .install_bytes(&target, b"first unit", &unit, &unit_temporary, 0o644)
            .await
            .unwrap();
        assert!(matches!(
            backend
                .install_bytes(&target, b"first unit", &unit, &unit_temporary, 0o644,)
                .await
                .unwrap(),
            ActionOutcome::Skipped(_)
        ));
        backend
            .install_bytes(&target, b"second unit", &unit, &unit_temporary, 0o644)
            .await
            .unwrap();
        backend.fail_before_rename = true;
        assert!(
            backend
                .install_bytes(&target, b"third unit", &unit, &unit_temporary, 0o644,)
                .await
                .is_err()
        );
        assert_eq!(tokio::fs::read(&unit).await.unwrap(), b"second unit");
        assert!(!unit_temporary.exists());
    }

    /// Step transitions publish feed revisions and retain running/pending,
    /// completed/skipped/degraded, and failed states for rereads.
    #[tokio::test]
    async fn per_step_progress_and_feed_transitions_are_observable() {
        let harness = harness().await;
        let root = tempfile::tempdir().unwrap();
        let backend = FakeBackend::absent(root.path().to_path_buf());
        *backend.skip.lock().unwrap() = Some("install-farhelm".to_string());
        *backend.linger.lock().unwrap() = Some(Err("permission denied".to_string()));
        backend
            .block_first
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let provisioner = service(&harness, backend.clone(), root.path());
        let mut revisions = harness.manager.events().subscribe();
        let initial_revision = *revisions.borrow_and_update();
        let response = provisioner
            .probe(ProbeRequest {
                target: ProbeDestination::Local,
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { probe_id, .. } = response else {
            panic!("expected plan")
        };
        let accepted = provisioner
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        backend.entered.notified().await;
        let running = provisioner.view(accepted.host_id).await.unwrap();
        assert_eq!(running.steps[0].status, StepStatus::Running);
        assert!(
            running.steps[1..]
                .iter()
                .all(|step| step.status == StepStatus::Pending)
        );
        revisions.changed().await.unwrap();
        assert!(*revisions.borrow_and_update() > initial_revision);
        backend.release.notify_one();
        let completed = wait_finished(&provisioner, accepted.host_id).await;
        assert!(
            completed
                .steps
                .iter()
                .any(|step| step.status == StepStatus::Completed)
        );
        assert!(
            completed
                .steps
                .iter()
                .any(|step| step.status == StepStatus::Skipped)
        );
        assert!(
            completed
                .steps
                .iter()
                .any(|step| step.status == StepStatus::Degraded)
        );

        let failing = FakeBackend::absent(root.path().to_path_buf());
        *failing.fail.lock().unwrap() = Some("daemon-reload".to_string());
        let failing_service = service(&harness, failing, root.path());
        let response = failing_service
            .probe(ProbeRequest {
                target: ProbeDestination::Local,
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        let ProbeResponse::Provisionable { probe_id, .. } = response else {
            panic!("expected plan")
        };
        let accepted = failing_service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        let failed = wait_finished(&failing_service, accepted.host_id).await;
        assert!(
            failed
                .steps
                .iter()
                .any(|step| step.status == StepStatus::Failed)
        );
    }

    /// Per-kind sources prevent a private-tmux plan from ever substituting
    /// the supervisor executable merely because both artifacts share an
    /// architecture.
    struct MutablePayload {
        farhelm: Mutex<PathBuf>,
        tmux: PathBuf,
    }

    impl PayloadSource for MutablePayload {
        fn path(&self, payload: PayloadKind, _arch: PayloadArch) -> anyhow::Result<PathBuf> {
            Ok(match payload {
                PayloadKind::Farhelm => self.farhelm.lock().unwrap().clone(),
                PayloadKind::Tmux => self.tmux.clone(),
            })
        }
    }

    /// Failure-safe cleanup for every real user unit created below.
    ///
    /// Drop uses only the nonce-named unit and the fixture's own tmux socket.
    /// It never broad-matches Farhelm units or paths, and therefore remains
    /// safe on assertion failure as well as on the happy path.
    struct UnitGuard {
        unit: String,
        unit_path: PathBuf,
        state_dir: PathBuf,
        cleaned: bool,
    }

    impl UnitGuard {
        /// Finish teardown only after every fixture-owned resource is proven
        /// absent; callers must surface failure instead of trusting Drop.
        fn cleanup(&mut self) -> anyhow::Result<()> {
            let disable = std::process::Command::new("systemctl")
                .args(["--user", "--runtime", "disable", "--now", "--", &self.unit])
                .output()
                .context("stopping the nonce provisioning unit")?;
            if !disable.status.success()
                && !String::from_utf8_lossy(&disable.stderr).contains("does not exist")
            {
                bail!(
                    "stopping nonce unit {} failed: {}",
                    self.unit,
                    String::from_utf8_lossy(&disable.stderr)
                );
            }
            match std::fs::remove_file(&self.unit_path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error).context("removing the fixture-owned unit file"),
            }
            let reload = std::process::Command::new("systemctl")
                .args(["--user", "daemon-reload"])
                .output()
                .context("reloading after nonce unit removal")?;
            if !reload.status.success() {
                bail!(
                    "reloading after nonce unit removal failed: {}",
                    String::from_utf8_lossy(&reload.stderr)
                );
            }
            let _ = std::process::Command::new("tmux")
                .arg("-S")
                .arg(self.state_dir.join("tmux.sock"))
                .arg("kill-server")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();

            let active = std::process::Command::new("systemctl")
                .args(["--user", "is-active", "--", &self.unit])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .context("verifying nonce unit inactivity")?;
            let enabled = std::process::Command::new("systemctl")
                .args(["--user", "is-enabled", "--", &self.unit])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .context("verifying nonce unit disablement")?;
            let tmux_alive = std::process::Command::new("tmux")
                .arg("-S")
                .arg(self.state_dir.join("tmux.sock"))
                .arg("has-session")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .context("verifying nonce tmux teardown")?;
            if active.success()
                || enabled.success()
                || self.unit_path.exists()
                || tmux_alive.success()
            {
                bail!(
                    "nonce teardown verification failed: active={}, enabled={}, file={}, tmux={}",
                    active.success(),
                    enabled.success(),
                    self.unit_path.exists(),
                    tmux_alive.success()
                );
            }
            self.cleaned = true;
            Ok(())
        }
    }

    /// Exercise an error return while the checked teardown guard is live.
    fn planted_failure(mut guard: UnitGuard) -> anyhow::Result<()> {
        guard.cleanup()?;
        bail!("planted failure")
    }

    impl Drop for UnitGuard {
        fn drop(&mut self) {
            if self.cleaned {
                return;
            }
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "--runtime", "disable", "--now", "--", &self.unit])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = std::fs::remove_file(&self.unit_path);
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "daemon-reload"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = std::process::Command::new("tmux")
                .arg("-S")
                .arg(self.state_dir.join("tmux.sock"))
                .arg("kill-server")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            eprintln!(
                "provisioning test teardown fell back to Drop for nonce unit {}; explicit cleanup did not complete",
                self.unit
            );
        }
    }

    async fn user_manager_available() -> bool {
        tokio::process::Command::new("systemctl")
            .args(["--user", "show-environment"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .is_ok_and(|status| status.success())
    }

    async fn self_ssh_available() -> bool {
        tokio::process::Command::new("ssh")
            .args([
                "-o",
                "BatchMode=yes",
                "-o",
                "StrictHostKeyChecking=yes",
                "-o",
                "ConnectTimeout=10",
                "localhost",
                "true",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .is_ok_and(|status| status.success())
    }

    /// Resolve the executable payload contract for real provisioning tests.
    ///
    /// `FARHELM_TEST_BINARY` lets clean or nonstandard target layouts name a
    /// known artifact. The conventional workspace debug path remains a
    /// convenience for the CI sequence, which builds binaries before tests.
    fn debug_farhelm() -> Option<PathBuf> {
        if let Some(configured) = std::env::var_os("FARHELM_TEST_BINARY") {
            let configured = PathBuf::from(configured);
            assert!(
                configured.is_file(),
                "FARHELM_TEST_BINARY does not name a file: {}",
                configured.display()
            );
            return Some(configured);
        }
        let test_binary = std::env::current_exe().expect("locate the test binary");
        let debug_dir = test_binary
            .parent()
            .and_then(Path::parent)
            .expect("the test binary lives under target/debug/deps");
        let payload = debug_dir.join(format!("farhelm{}", std::env::consts::EXE_SUFFIX));
        payload.is_file().then_some(payload)
    }

    /// Locate a real tmux payload independently of Farhelm. The normal
    /// localhost reach check uses this installation directly, while the
    /// source remains available if the private-payload branch is selected.
    fn debug_tmux() -> Option<PathBuf> {
        let output = std::process::Command::new("sh")
            .args(["-c", "command -v tmux"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let path = PathBuf::from(String::from_utf8(output.stdout).ok()?.trim());
        path.is_absolute().then_some(path)
    }

    async fn wait_real_run(
        service: &ProvisioningService,
        host: HostId,
    ) -> anyhow::Result<ProvisioningView> {
        tokio::time::timeout(Duration::from_secs(60), async {
            loop {
                let view = service.view(host).await?;
                if matches!(view.status, RunStatus::Completed | RunStatus::Failed) {
                    return Ok(view);
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .context("real provisioning run did not finish")?
    }

    /// Wait through the manager's intentional disconnect/retry window until
    /// a test can issue a command on the current supervisor incarnation.
    async fn wait_real_client(
        manager: &ConnectionManager,
        host: HostId,
    ) -> Arc<crate::client::SupervisorClient> {
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                if let Some(client) = manager.status(host).and_then(|status| status.client) {
                    return client;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("the supervisor did not expose a connected client")
    }

    /// Prove both halves of session survival: tmux still reports a live
    /// process, and the post-restart supervisor can drive its terminal.
    async fn assert_session_operable(client: &crate::client::SupervisorClient, session_id: &str) {
        let listing = client.list_sessions().await.expect("list live session");
        let session = listing
            .sessions
            .iter()
            .find(|session| session.id == session_id)
            .expect("the tmux-held session must remain listed");
        assert!(
            session.status.is_live(),
            "the retained session is not running: {:?}",
            session.status
        );

        let (channel, mut terminal) = client
            .attach(session_id, 80, 24)
            .await
            .expect("attach to the retained session");
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                match terminal.recv().await {
                    Some(crate::client::TermEvent::ReplayComplete) => break,
                    Some(crate::client::TermEvent::Data(_)) => {}
                    Some(crate::client::TermEvent::Detached(reason)) => {
                        panic!("retained terminal detached during replay: {reason}")
                    }
                    None => panic!("retained terminal ended during replay"),
                }
            }
        })
        .await
        .expect("retained terminal replay did not complete");

        let token = format!("farhelmoperable{}", uuid::Uuid::new_v4().simple());
        let marker = token.to_ascii_uppercase();
        client
            .send_input(
                channel,
                format!("printf '%s\\n' '{token}' | tr '[:lower:]' '[:upper:]'\n").into_bytes(),
            )
            .await;
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut output = Vec::new();
            loop {
                match terminal.recv().await {
                    Some(crate::client::TermEvent::Data(bytes)) => {
                        output.extend(bytes);
                        if String::from_utf8_lossy(&output).contains(&marker) {
                            break;
                        }
                    }
                    Some(crate::client::TermEvent::ReplayComplete) => {}
                    Some(crate::client::TermEvent::Detached(reason)) => {
                        panic!("retained terminal detached before accepting input: {reason}")
                    }
                    None => panic!("retained terminal ended before accepting input"),
                }
            }
        })
        .await
        .expect("retained terminal did not echo post-restart input");
        client.detach(channel).await;
    }

    /// Exercise the complete installer against a real user manager, either
    /// through direct local process/file operations or through ssh+sftp to
    /// localhost. All install/state paths are fixture-owned and linger is a
    /// simulated action. The unit file stays below the fixture root and the
    /// user manager sees it only through a nonce-scoped runtime link removed
    /// by [`UnitGuard`] on every exit.
    async fn real_provisioning_case(use_ssh: bool, update: bool) {
        let test_name = if use_ssh {
            "provisioning_over_ssh_to_localhost"
        } else {
            "provisioning_over_the_direct_local_transport"
        };
        if !user_manager_available().await {
            eprintln!("SKIPPED {test_name}: no usable systemd user manager exists on this host");
            return;
        }
        if use_ssh && !self_ssh_available().await {
            eprintln!(
                "SKIPPED {test_name}: passwordless, already-trusted ssh localhost is unavailable"
            );
            return;
        }

        let Some(payload) = debug_farhelm() else {
            eprintln!(
                "SKIPPED {test_name}: no Farhelm payload exists at the workspace debug path; build it first or set FARHELM_TEST_BINARY"
            );
            return;
        };
        let Some(tmux_payload) = debug_tmux() else {
            eprintln!("SKIPPED {test_name}: no absolute tmux executable is available");
            return;
        };
        let root = tempfile::tempdir().expect("isolated provisioning root");
        let lib_dir = root.path().join("lib");
        let supervisor_state = root.path().join("supervisor-state");
        let probe_state = root.path().join("probe-state-never-created");
        let probe_farhelm = root.path().join("probe-farhelm-never-created");
        let unit_dir = root.path().join("units");
        let helm_state = if use_ssh {
            root.path().join("helm-state")
        } else {
            supervisor_state.clone()
        };
        tokio::fs::create_dir_all(&helm_state).await.unwrap();
        let unit = format!("farhelm-provisioning-test-{}.service", uuid::Uuid::new_v4());
        let unit_path = unit_dir.join(&unit);
        let mut guard = UnitGuard {
            unit: unit.clone(),
            unit_path: unit_path.clone(),
            state_dir: supervisor_state.clone(),
            cleaned: false,
        };
        let store = HelmStore::open(&helm_state.join("helm.db")).await.unwrap();
        let manager = ConnectionManager::start(
            store.clone(),
            Arc::new(crate::manager::SystemTransport::new(&helm_state)),
            crate::manager::Cadence::default(),
        )
        .await
        .unwrap();
        let payloads = Arc::new(MutablePayload {
            farhelm: Mutex::new(payload.clone()),
            tmux: tmux_payload,
        });
        let launcher = RecordingLauncher::new();
        let mut system_backend =
            SystemBackend::with_simulated_linger(helm_state.clone(), Ok(()), true);
        system_backend.launcher = Arc::clone(&launcher) as Arc<dyn CommandLauncher>;
        let service = ProvisioningService::injected(
            store.clone(),
            Arc::clone(&manager),
            Arc::new(system_backend),
            Arc::clone(&payloads) as Arc<dyn PayloadSource>,
            PlanLayout {
                local_state_dir: supervisor_state.clone(),
                override_lib_dir: Some(lib_dir.clone()),
                override_farhelm_path: None,
                override_state_dir: Some(supervisor_state.clone()),
                override_unit_dir: Some(unit_dir),
                unit_name: unit.clone(),
            },
            payload.clone(),
        );
        let request = ProbeRequest {
            target: if use_ssh {
                ProbeDestination::Ssh {
                    destination: "localhost".to_string(),
                }
            } else {
                ProbeDestination::Local
            },
            remote_farhelm: use_ssh.then(|| {
                probe_farhelm
                    .to_str()
                    .expect("temp path is UTF-8")
                    .to_string()
            }),
            remote_state_dir: use_ssh.then(|| {
                probe_state
                    .to_str()
                    .expect("temp path is UTF-8")
                    .to_string()
            }),
        };

        let response = service.probe(request).await.unwrap();
        let ProbeResponse::Provisionable { probe_id, plan, .. } = response else {
            panic!("an absent isolated supervisor must produce a plan")
        };
        assert_eq!(
            matches!(plan.target, ProvisioningTarget::Ssh { .. }),
            use_ssh,
            "the local case must never silently become SSH-to-self"
        );
        let accepted = service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        let completed = match wait_real_run(&service, accepted.host_id).await {
            Ok(completed) => completed,
            Err(error) => {
                service.abort_run(accepted.host_id).await;
                guard.cleanup().expect("cleanup after timed-out ADD");
                panic!("{error:#}");
            }
        };
        assert_eq!(completed.status, RunStatus::Completed, "{completed:?}");
        assert!(lib_dir.join("farhelm").is_file());
        assert!(unit_path.is_file());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o7777;
            assert_eq!(mode(&lib_dir), 0o755);
            assert_eq!(mode(&supervisor_state), 0o700);
            assert_eq!(mode(unit_path.parent().unwrap()), 0o755);
        }
        let active = tokio::process::Command::new("systemctl")
            .args(["--user", "is-active", "--", &unit])
            .output()
            .await
            .unwrap();
        assert!(
            active.status.success(),
            "the nonce unit is not active: {}",
            String::from_utf8_lossy(&active.stderr)
        );
        assert!(
            manager
                .state(accepted.host_id)
                .is_some_and(|state| state.is_connected())
        );
        if !use_ssh {
            assert!(
                launcher
                    .programs
                    .lock()
                    .unwrap()
                    .iter()
                    .all(|program| { !program.ends_with("ssh") && !program.ends_with("sftp") }),
                "the direct local case launched ssh or sftp"
            );
        }

        // ADD reruns discovery and uses the answering supervisor as-is. It
        // does not reinterpret an explicit retry as permission to update.
        let previous_run = completed.run_id;
        let rerun = service
            .probe(ProbeRequest {
                target: if use_ssh {
                    ProbeDestination::Ssh {
                        destination: "localhost".to_string(),
                    }
                } else {
                    ProbeDestination::Local
                },
                remote_farhelm: None,
                remote_state_dir: None,
            })
            .await
            .unwrap();
        assert!(matches!(rerun, ProbeResponse::Discovered { .. }));
        assert_eq!(
            service.view(accepted.host_id).await.unwrap().run_id,
            previous_run
        );

        let client = wait_real_client(&manager, accepted.host_id).await;
        let cwd = root.path().to_str().expect("temp path is UTF-8");
        let session = client
            .create_session(cwd, "/bin/sh", None, 80, 24)
            .await
            .expect("create an operable session through the provisioned host");

        if update {
            let newer = root.path().join("farhelm-newer");
            let mut bytes = tokio::fs::read(&payload).await.unwrap();
            bytes.extend_from_slice(b"farhelm-test-newer-payload");
            tokio::fs::write(&newer, &bytes).await.unwrap();
            set_mode(&newer, 0o755).await.unwrap();
            *payloads.farhelm.lock().unwrap() = newer;
            let update_plan = service.plan_update(accepted.host_id).await.unwrap();
            let update_run = service
                .start_update(
                    accepted.host_id,
                    ProvisionRequest {
                        probe_id: update_plan.probe_id,
                    },
                )
                .await
                .unwrap();
            let completed = match wait_real_run(&service, update_run.host_id).await {
                Ok(completed) => completed,
                Err(error) => {
                    service.abort_run(update_run.host_id).await;
                    guard.cleanup().expect("cleanup after timed-out UPDATE");
                    panic!("{error:#}");
                }
            };
            assert_eq!(completed.status, RunStatus::Completed, "{completed:?}");
            assert_eq!(
                hex_sha256(&tokio::fs::read(lib_dir.join("farhelm")).await.unwrap()),
                hex_sha256(&bytes),
                "UPDATE must converge to the newer payload"
            );
        }
        let client = wait_real_client(&manager, accepted.host_id).await;
        assert_session_operable(&client, &session.id).await;
        client.delete_session(&session.id).await.unwrap();

        service.abort_run(accepted.host_id).await;
        manager.shutdown();
        drop(service);
        drop(manager);
        drop(store);
        guard.cleanup().expect("checked nonce resource teardown");
        drop(guard);
        assert!(!unit_path.exists(), "the nonce unit file survived teardown");
        let inactive = tokio::process::Command::new("systemctl")
            .args(["--user", "is-active", "--", &unit])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .unwrap();
        assert!(!inactive.success(), "the nonce unit survived teardown");
        let root_path = root.path().to_path_buf();
        drop(root);
        assert!(
            !root_path.exists(),
            "the temporary install/state root survived teardown"
        );
    }

    /// The CI-shaped transport proof: real ssh and sftp into isolated paths,
    /// then an SSH UPDATE that preserves and operates a tmux-held session.
    #[tokio::test]
    async fn provisioning_and_update_over_ssh_to_localhost_preserve_an_operable_session() {
        real_provisioning_case(true, true).await;
    }

    /// The local path performs no SSH, and the explicit UPDATE replaces the
    /// payload, restarts only the supervisor, and preserves its tmux session.
    #[tokio::test]
    async fn local_provisioning_and_update_preserve_a_running_session() {
        real_provisioning_case(false, true).await;
    }

    /// A planted failure is followed by checked teardown of a real active
    /// runtime unit and tmux server, then verified again from the outer scope.
    #[tokio::test]
    async fn teardown_guard_runs_on_failure() {
        if !user_manager_available().await {
            eprintln!(
                "SKIPPED teardown_guard_runs_on_failure: no usable systemd user manager exists on this host"
            );
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let unit = format!("farhelm-provisioning-test-{}.service", uuid::Uuid::new_v4());
        let unit_path = root.path().join(&unit);
        std::fs::write(
            &unit_path,
            "[Service]\nType=simple\nExecStart=/usr/bin/sleep 300\n",
        )
        .unwrap();
        let activated = std::process::Command::new("systemctl")
            .args(["--user", "--runtime", "enable", "--now", "--"])
            .arg(&unit_path)
            .output()
            .unwrap();
        assert!(
            activated.status.success(),
            "failed to activate nonce unit: {}",
            String::from_utf8_lossy(&activated.stderr)
        );
        let tmux_socket = root.path().join("tmux.sock");
        let tmux_started = std::process::Command::new("tmux")
            .arg("-S")
            .arg(&tmux_socket)
            .args(["new-session", "-d", "/usr/bin/sleep", "300"])
            .output()
            .unwrap();
        assert!(
            tmux_started.status.success(),
            "failed to start nonce tmux server: {}",
            String::from_utf8_lossy(&tmux_started.stderr)
        );
        assert!(
            planted_failure(UnitGuard {
                unit: unit.clone(),
                unit_path: unit_path.clone(),
                state_dir: root.path().to_path_buf(),
                cleaned: false,
            })
            .is_err()
        );
        assert!(
            !unit_path.exists(),
            "the unit file survived failure teardown"
        );
        let active = std::process::Command::new("systemctl")
            .args(["--user", "is-active", "--", &unit])
            .status()
            .unwrap();
        let enabled = std::process::Command::new("systemctl")
            .args(["--user", "is-enabled", "--", &unit])
            .status()
            .unwrap();
        let tmux_alive = std::process::Command::new("tmux")
            .arg("-S")
            .arg(tmux_socket)
            .arg("has-session")
            .status()
            .unwrap();
        assert!(!active.success());
        assert!(!enabled.success());
        assert!(!tmux_alive.success());
    }
}
