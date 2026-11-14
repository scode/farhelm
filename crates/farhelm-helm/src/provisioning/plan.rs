//! The confirmed plan is the only description of the work: confirmation and
//! execution walk the same frozen actions.

use super::backend::{BackendFailure, Reach};
use super::http::ProvisioningRequestError;
use crate::access::RemoteRoute;
use crate::store::HostRow;
use crate::units::{SupervisorUnitInputs, render_supervisor_unit};
use serde::Serialize;
use std::path::{Path, PathBuf};

// The operation and the plan-path shape live in `farhelm-proto`, because provisioning progress carries them to the
// GUI; the helm-only behavior on them is in the two traits below.
pub(crate) use farhelm_proto::provisioning::{PlanStepPath, ProvisioningOperation};

/// The helm's own wording for an operation, kept out of the shared type because only the helm phrases refusals.
pub(super) trait OperationRefusal {
    /// What the panel says instead of acting on a host whose supervisor unit
    /// carries `farhelm helm setup`'s managed-by marker.
    ///
    /// One refusal per operation rather than one string for all of them:
    /// setup owns that unit on the host (SPEC.md "Ownership during cleanup
    /// and provisioning"), so the remedy depends on what the user was trying
    /// to do. Installing or updating there is the host's own installer and
    /// setup; removing is the host's own `farhelm uninstall`, which knows how
    /// to take down a setup-owned service.
    fn setup_managed_refusal(self) -> String;
}

impl OperationRefusal for ProvisioningOperation {
    fn setup_managed_refusal(self) -> String {
        let unit = crate::units::SUPERVISOR_UNIT_NAME;
        match self {
            Self::Add | Self::Update => format!(
                "{unit} on this host is managed by farhelm helm setup there, so the hosts panel does \
                 not replace it. Update Farhelm on that host with its installer and farhelm helm \
                 setup, or move the unit aside to let the panel provision the host."
            ),
            Self::Uninstall => format!(
                "{unit} on this host is managed by farhelm helm setup there, so the hosts panel does \
                 not remove it. Remove Farhelm on that host by running farhelm uninstall there."
            ),
        }
    }
}

/// Transport facts retained in the plan so execution cannot silently switch
/// machines or access policies between confirmation and execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProvisioningTarget {
    Local,
    /// Remote filesystem operations and probes always execute through their
    /// route, including a container reached by a local Podman process.
    Remote(RemoteRoute),
}

impl Serialize for ProvisioningTarget {
    /// Confirmation plans retain the established flat SSH shape. Wrapping a
    /// route in Rust must not change their serialized comparison material.
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Local => {
                use serde::ser::SerializeStruct as _;
                let mut value = serializer.serialize_struct("ProvisioningTarget", 1)?;
                value.serialize_field("kind", "local")?;
                value.end()
            }
            Self::Remote(route) => route.serialize(serializer),
        }
    }
}

#[cfg(test)]
impl ProvisioningTarget {
    /// An ordinary ssh host reached with the user's own ssh setup, for the
    /// tests' plan and backend fixtures.
    pub(super) fn ssh(destination: impl Into<String>) -> Self {
        Self::Remote(RemoteRoute::Ssh {
            destination: destination.into(),
            access: crate::ssh::SshAccessKind::UserConfig,
        })
    }
}

#[cfg(test)]
mod target_serialization_tests {
    use super::*;

    /// Plans are frozen confirmation material. Route refactoring must retain
    /// the flat SSH tag and omit ordinary access exactly as earlier plans did.
    #[farhelm_testtrace::test]
    fn remote_routes_keep_existing_plan_json() {
        assert_eq!(
            serde_json::to_value(ProvisioningTarget::Local).unwrap(),
            serde_json::json!({"kind": "local"})
        );
        assert_eq!(
            serde_json::to_value(ProvisioningTarget::ssh("user@host")).unwrap(),
            serde_json::json!({"kind": "ssh", "destination": "user@host"})
        );
        assert_eq!(
            serde_json::to_value(ProvisioningTarget::Remote(crate::tensorlake::ssh::route(
                "sandbox"
            )))
            .unwrap(),
            serde_json::json!({"kind": "ssh", "destination": "sandbox", "access": "tensorlake"})
        );
    }
}

/// Release targets accepted before confirmation, with no unsupported OS/CPU pair.
///
/// The two Linux wire spellings predate OS-aware provisioning. Keeping them
/// avoids changing existing serialized plans while adding the Mac target.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Hash)]
pub enum PayloadTarget {
    #[serde(rename = "x86_64")]
    LinuxX86_64,
    #[serde(rename = "aarch64")]
    LinuxAarch64,
    #[serde(rename = "macos-aarch64")]
    MacosAarch64,
}

impl PayloadTarget {
    /// The target selects the OS-specific plan; architecture alone cannot.
    pub(super) fn is_macos(self) -> bool {
        matches!(self, Self::MacosAarch64)
    }
}

/// The bare architecture spelling (`x86_64`, `aarch64`) used in the
/// confirmation plan's host line — distinct from the full target triples
/// `assets.rs` uses for release filenames, which nobody wants to read in
/// the confirmation text.
impl std::fmt::Display for PayloadTarget {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::LinuxX86_64 => "x86_64",
            Self::LinuxAarch64 | Self::MacosAarch64 => "aarch64",
        })
    }
}

/// One directory a plan needs to exist, and whose permissions it may touch.
///
/// SPEC.md ("Ownership during cleanup and provisioning") splits these in two.
/// A directory dedicated to Farhelm (its private lib directory, the supervisor
/// state directory) converges on `mode` every run, so a rerun repairs drift.
/// A `shared` directory merely holds one of Farhelm's files next to other
/// things (the systemd user-unit directory, a registered binary's own bin
/// directory, possibly `$HOME` itself): it is created with `mode` only when it
/// is missing, and an existing one keeps exactly the permissions it had.
/// Chmodding it would expose or lock out everything else in it, and GNU
/// `install -d -m` does chmod an existing directory, which is how that used to
/// happen. If a shared directory's permissions prevent installation, the later
/// write fails and reports it; nothing here widens them.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct DirectorySpec {
    pub(super) path: PathBuf,
    pub(super) mode: u32,
    pub(super) shared: bool,
}

/// Every mutating or attaching action in the order the executor performs it.
///
/// Paths, unit contents, linger's conditional boot promise, and the
/// persistent-run statement live here rather than in a parallel confirmation
/// template. Each action becomes one user-facing step for confirmation and
/// progress. Adding executor behavior therefore also changes the plan text
/// the request authorizes; remote updates consume it sight unseen.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "step", rename_all = "kebab-case")]
pub(crate) enum ProvisioningAction {
    EnsureDirectories {
        directories: Vec<DirectorySpec>,
    },
    /// Remote plans transfer the verified snapshot of the `farhelm` binary,
    /// the one payload a plan installs, to this nonce temporary before the
    /// install step can change the live executable. Local plans copy within
    /// their install action and have no upload step.
    UploadPayload {
        arch: PayloadTarget,
        destination: PathBuf,
        temporary: PathBuf,
    },
    /// Install the local snapshot or the remote upload from this same plan.
    /// A remote install must not start another network transfer.
    InstallPayload {
        arch: PayloadTarget,
        destination: PathBuf,
        temporary: PathBuf,
    },
    WriteUnit {
        unit: String,
        destination: PathBuf,
        temporary: PathBuf,
        content: String,
    },
    /// launchd consumes an argv plist, independently of Linux unit syntax.
    WriteLaunchAgent {
        destination: PathBuf,
        temporary: PathBuf,
        content: String,
    },
    /// UPDATE refreshes the loaded registration as well as the plist on
    /// disk. Domain and label are fixed by inspection and confirmation.
    StartLaunchAgent {
        domain: String,
        label: String,
        plist: PathBuf,
        refresh: bool,
    },
    RemoveLaunchAgent {
        destination: PathBuf,
    },
    /// launchd leaves terminal hosts alive deliberately; uninstall ends
    /// them explicitly before removing the binary that performs that stop.
    StopLaunchAgent {
        domain: String,
        label: String,
        farhelm_program: PathBuf,
        state_dir: PathBuf,
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
    /// Delete the tmux that releases before this one installed in Farhelm's
    /// private lib directory on a Linux host whose own tmux was too old.
    /// Farhelm no longer uses it, but its directory comes first on sessions'
    /// `PATH`, so left in place it would keep answering `tmux` typed in a
    /// session. Every Linux Add and Update plans it, because the upgrade
    /// floor still lets Update reach hosts that carry one and an Add may run
    /// over an earlier install; an absent file is a skip, so a host that
    /// never had one passes it untouched. It runs once the new supervisor
    /// is running (after the start or restart), never while an earlier
    /// supervisor that may still drive that tmux is up.
    ///
    /// A removal that fails degrades the step rather than failing the run.
    /// The new supervisor already runs by then and nothing of the install
    /// depends on the file, so reporting Update or Add as failed, and asking
    /// for a rerun, would misstate a working host; the step's warning says
    /// the file may have been left in place instead (a timeout or a lost
    /// connection may still have removed it). Snapshot preparation skips the
    /// step: the sandbox image it installs into has no Farhelm installation
    /// (preparation refuses setup-managed installs and copied state), so it
    /// is not expected to carry an earlier tmux.
    RemoveBundledTmux {
        path: PathBuf,
    },
    AttachSupervisor,
    /// UNINSTALL's first host change: stop the user manager from starting
    /// the supervisor at boot or login, while leaving it running. Keeping it
    /// running until the unit file is gone is what lets a failed run be
    /// retried under the ordinary connected checks (see
    /// `PlanLayout::plan_uninstall`).
    DisableSupervisor {
        unit: String,
    },
    /// Delete the supervisor's unit file. The remote command re-checks
    /// setup's managed-by marker in the same shell that removes the file, as
    /// the unit write does, because the plan was confirmed some time after
    /// the host was inspected.
    RemoveUnit {
        unit: String,
        destination: PathBuf,
    },
    /// Stop the supervisor and every process left in its unit's control
    /// group, then end the state directory's terminal hosts, which reach
    /// what outlived the unit. Every path is frozen before confirmation,
    /// like the removal paths: the `farhelm` that stops the hosts (the
    /// plan's binary, still in place because the lib directory goes after
    /// this step) and the state directory whose hosts it ends.
    ///
    /// There is no tmux step. A private tmux server an earlier release left
    /// running in the unit's control group ends with the unit; one that
    /// outlived an already stopped unit survives uninstall, a residual the
    /// maintainer accepted when tmux was dropped (SPEC.md, uninstall).
    StopSupervisor {
        unit: String,
        farhelm_program: PathBuf,
        state_dir: PathBuf,
    },
    /// Delete Farhelm's private lib directory, with the `farhelm` binary in
    /// it. Planning refuses a layout in which the state directory lies
    /// inside it, so this never removes data.
    RemoveDirectory {
        path: PathBuf,
    },
    /// Delete the host's registry row, its cached sessions and the helm's
    /// connection to it. Helm-side, like `AttachSupervisor`, and last, so a
    /// failed run leaves the row in place to show what is left and to be
    /// retried.
    ForgetHost,
}

impl ProvisioningAction {
    /// Stable action identity shared by the frozen plan and its progress view.
    pub(super) fn label(&self) -> &'static str {
        match self {
            Self::EnsureDirectories { .. } => "create-directories",
            Self::UploadPayload { .. } => "upload-farhelm",
            Self::InstallPayload { .. } => "install-farhelm",
            Self::WriteUnit { .. } => "write-unit",
            Self::WriteLaunchAgent { .. } => "write-launch-agent",
            Self::StartLaunchAgent { refresh: false, .. } => "start-launch-agent",
            Self::StartLaunchAgent { refresh: true, .. } => "restart-launch-agent",
            Self::RemoveLaunchAgent { .. } => "remove-launch-agent",
            Self::StopLaunchAgent { .. } => "stop-launch-agent",
            Self::DaemonReload => "daemon-reload",
            Self::EnableSupervisor { .. } => "enable-supervisor",
            Self::EnableLinger { .. } => "enable-linger",
            Self::RestartSupervisor { .. } => "restart-supervisor",
            Self::RemoveBundledTmux { .. } => "remove-bundled-tmux",
            Self::AttachSupervisor => "attach-supervisor",
            Self::DisableSupervisor { .. } => "disable-supervisor",
            Self::RemoveUnit { .. } => "remove-unit",
            Self::StopSupervisor { .. } => "stop-supervisor",
            Self::RemoveDirectory { .. } => "remove-directory",
            Self::ForgetHost => "forget-host",
        }
    }

    /// Give every frozen action one user-facing step, including actions that
    /// the old confirmation text hid. The helm owns this wording so a client
    /// cannot promise a different run than the one it was asked to confirm.
    fn step(&self, home: &Path) -> PlanStep {
        let (name, paths, explanation): (&str, Vec<&Path>, Option<String>) = match self {
            Self::EnsureDirectories { directories } => (
                "Create directories",
                directories
                    .iter()
                    .map(|directory| directory.path.as_path())
                    .collect(),
                Some("Existing shared directories keep their permissions.".to_string()),
            ),
            Self::UploadPayload { temporary, .. } => {
                ("Upload the farhelm binary", vec![temporary], None)
            }
            Self::InstallPayload {
                destination,
                temporary,
                ..
            } => (
                "Install the farhelm binary",
                vec![destination, temporary],
                None,
            ),
            Self::WriteUnit {
                unit,
                destination,
                temporary,
                ..
            } => (
                "Write the systemd user unit",
                vec![destination, temporary],
                Some(format!("Unit: {unit}")),
            ),
            Self::DaemonReload => ("Reload systemd user units", vec![], None),
            Self::WriteLaunchAgent {
                destination,
                temporary,
                ..
            } => (
                "Write the per-user launchd job",
                vec![destination, temporary],
                None,
            ),
            Self::StartLaunchAgent {
                label,
                plist,
                refresh,
                ..
            } => (
                if *refresh {
                    "Restart the supervisor"
                } else {
                    "Start the supervisor"
                },
                vec![plist],
                Some(format!(
                    "Job: {label}. Runs now and restarts after a crash. After a reboot, choose Update in the host menu to start it. Automatic startup at screen login is unverified."
                )),
            ),
            Self::RemoveLaunchAgent { destination } => {
                ("Remove the launchd job's plist", vec![destination], None)
            }
            Self::StopLaunchAgent { label, .. } => (
                "Stop the supervisor and its terminal hosts",
                vec![],
                Some(format!("Job: {label}")),
            ),
            Self::EnableSupervisor {
                unit_path,
                persistent_run,
                ..
            } => (
                "Enable and start the supervisor",
                vec![unit_path],
                Some(persistent_run.clone()),
            ),
            Self::EnableLinger {
                boot_start_if_enabled,
                login_start_if_refused,
            } => (
                "Enable lingering, to start at boot",
                vec![],
                Some(format!(
                    "{boot_start_if_enabled}; if lingering is refused, {login_start_if_refused}"
                )),
            ),
            Self::RestartSupervisor { .. } => ("Restart the supervisor", vec![], None),
            Self::RemoveBundledTmux { path } => (
                "Remove the bundled tmux",
                vec![path],
                Some(
                    "Earlier releases installed their own tmux here; Farhelm no longer uses it, \
                     and tmux typed in a session afterwards no longer runs this copy."
                        .to_string(),
                ),
            ),
            Self::AttachSupervisor => ("Connect to the supervisor", vec![], None),
            Self::DisableSupervisor { .. } => ("Disable the supervisor", vec![], None),
            Self::RemoveUnit { destination, .. } => {
                ("Remove the systemd user unit", vec![destination], None)
            }
            Self::StopSupervisor { .. } => {
                ("Stop the supervisor and its terminal hosts", vec![], None)
            }
            Self::RemoveDirectory { path } => (
                "Remove the install directory",
                vec![path],
                Some(
                    "The Farhelm binary, and anything else in that directory, is removed."
                        .to_string(),
                ),
            ),
            Self::ForgetHost => ("Remove the host from the list", vec![], None),
        };
        PlanStep {
            id: self.label().to_string(),
            name: name.to_string(),
            paths: paths
                .into_iter()
                .map(|path| PlanStepPath::new(path, home))
                .collect(),
            explanation,
        }
    }
}

/// Building a plan path from the inspected host's home, which only the helm does.
trait StepPathFromHome {
    fn new(path: &Path, home: &Path) -> Self;
}

impl StepPathFromHome for PlanStepPath {
    /// Keep a path's full spelling for hover and copying, while shortening
    /// only paths that are actually inside this inspected host's home. `strip_prefix`
    /// checks path components, so a sibling whose name starts with the home directory
    /// is never shortened.
    fn new(path: &Path, home: &Path) -> Self {
        let full = path.display().to_string();
        let short = path.strip_prefix(home).map_or_else(
            |_| full.clone(),
            |relative| {
                if relative.as_os_str().is_empty() {
                    "~".to_string()
                } else {
                    format!("~/{}", relative.display())
                }
            },
        );
        Self { full, short }
    }
}

/// One frozen action as the user sees it before and during a run. The id
/// remains the executor's stable label; the name is the helm's public wording.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(super) struct PlanStep {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) paths: Vec<PlanStepPath>,
    pub(super) explanation: Option<String>,
}

/// The single ordered description used for confirmation and progress. Kept
/// data is a separate note because uninstall never acts on that directory.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(super) struct PlanSteps {
    pub(super) lead: String,
    pub(super) steps: Vec<PlanStep>,
    pub(super) kept: Option<String>,
}

#[cfg(test)]
mod step_path_tests {
    use super::*;

    /// A path whose first characters match the home is still outside it
    /// when the next path component differs. Shortening it would mislead the
    /// user about the host path a step actually touches.
    #[test]
    fn only_paths_within_the_host_home_get_tilde_spelling() {
        let home = Path::new("/home/host-user");
        assert_eq!(
            PlanStepPath::new(Path::new("/home/host-user/lib/farhelm"), home).short,
            "~/lib/farhelm"
        );
        assert_eq!(
            PlanStepPath::new(Path::new("/home/host-user-other/lib"), home).short,
            "/home/host-user-other/lib"
        );
    }
}

/// The frozen value both the confirmation text and the executor consume.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct ProvisioningPlan {
    pub(super) operation: ProvisioningOperation,
    pub(super) target: ProvisioningTarget,
    pub(super) farhelm_path: PathBuf,
    pub(super) state_dir: PathBuf,
    pub(super) actions: Vec<ProvisioningAction>,
    /// The inspected host's home directory, frozen with the actions so their
    /// path chips cannot be shortened against this client's own home.
    pub(super) host_home: PathBuf,
    /// The reach probe's validated Linux distribution ID (empty when absent)
    /// or `macOS`, and the native payload target it matched — carried
    /// through purely for `confirmation()`'s host line. Validation happens
    /// at the reach-record boundary because this value becomes plan text;
    /// provisioning still gates on capabilities, not on this ID (see
    /// `Reach::distro_id`), so nothing here or in execution branches on it.
    pub(super) host_distro_id: String,
    pub(super) host_arch: PayloadTarget,
}

impl ProvisioningPlan {
    /// Describe exactly the actions the run will carry out, in run order.
    ///
    /// The heading names the destination, distribution, and architecture so
    /// distro-agnostic provisioning still gives the user a chance to notice
    /// an unexpected host before authorizing its durable changes.
    ///
    /// UNINSTALL's kept-data note is separate: no action touches that path.
    pub(super) fn steps(&self) -> PlanSteps {
        let lead = format!(
            "Farhelm will {} {} ({}, {}):",
            match self.operation {
                ProvisioningOperation::Add | ProvisioningOperation::Update => "set up",
                ProvisioningOperation::Uninstall => "uninstall from",
            },
            match &self.target {
                ProvisioningTarget::Local => "the local host".to_string(),
                ProvisioningTarget::Remote(route) => route.destination().to_string(),
            },
            if self.host_distro_id.is_empty() {
                "unknown distribution"
            } else {
                &self.host_distro_id
            },
            self.host_arch,
        );
        let kept = (self.operation == ProvisioningOperation::Uninstall).then(|| {
            format!(
                "Kept: the host's Farhelm data in {}. Deleting that directory by hand removes the \
                 data too.",
                self.state_dir.display()
            )
        });
        PlanSteps {
            lead,
            steps: self
                .actions
                .iter()
                .map(|action| action.step(&self.host_home))
                .collect(),
            kept,
        }
    }

    /// Render copyable plan text from the same entries the dialogs and run
    /// use. A separate prose action list could drift from the run's steps.
    pub(super) fn confirmation(&self) -> String {
        let steps = self.steps();
        let mut rendered = format!("{}\n", steps.lead);
        for step in steps.steps {
            rendered.push_str(&format!("- {}\n", step.name));
            for path in step.paths {
                rendered.push_str(&format!("  {}\n", path.full));
            }
            if let Some(explanation) = step.explanation {
                rendered.push_str(&format!("  {explanation}\n"));
            }
        }
        if let Some(kept) = steps.kept {
            rendered.push_str(&format!("{kept}\n"));
        }
        rendered
    }
}

/// The concrete paths an UNINSTALL acts on, from [`PlanLayout::uninstall_paths`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UninstallPaths {
    pub(super) lib_dir: PathBuf,
    pub(super) unit_path: PathBuf,
    /// The layout's own state directory, when it overrides the host's.
    /// Otherwise a row with no recorded state directory uses whatever the
    /// supervisor's default resolves to on the host, which only the host
    /// can say (`XDG_STATE_HOME` included); see `UninstallInspection`.
    pub(super) state_dir_override: Option<PathBuf>,
}

/// What the host showed about the installation an UNINSTALL would remove,
/// resolved by the service from its inspection of the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UninstallFacts {
    /// Whether the supervisor's unit file is still there.
    pub(super) unit_file: bool,
    /// Where the lib directory leads when it still exists; `None` when it
    /// is already gone.
    pub(super) lib_dir: Option<ResolvedPath>,
    /// The binary the supervisor was dialed through, when one answered.
    pub(super) farhelm: Option<ResolvedPath>,
    /// The state directory the host uses, which uninstall keeps.
    pub(super) state_dir: ResolvedPath,
    /// Mount points under the asked paths (`UninstallInspection::mounts`).
    pub(super) mounts: Vec<PathBuf>,
}

/// A path as named to the user, beside where it really leads on the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResolvedPath {
    pub(super) named: PathBuf,
    /// The host's resolution with a verified home alias normalized.
    pub(super) canonical: PathBuf,
    /// The same resolution before that normalization (`HostPath::raw`).
    pub(super) raw: PathBuf,
    /// Whether the host found this path inside the lib directory by file
    /// identity, whatever either is called (`HostPath::under_removed`).
    pub(super) under_lib: bool,
}

#[derive(Debug, Clone)]
pub(super) struct PlanLayout {
    pub(super) local_state_dir: PathBuf,
    pub(super) override_lib_dir: Option<PathBuf>,
    pub(super) override_farhelm_path: Option<PathBuf>,
    pub(super) override_state_dir: Option<PathBuf>,
    pub(super) override_unit_dir: Option<PathBuf>,
    pub(super) unit_name: String,
}

impl PlanLayout {
    /// Production has one label; fixture unit names also isolate Mac jobs
    /// without adding another independently configurable identity.
    pub(super) fn launchd_label(&self) -> &str {
        if self.unit_name == crate::units::SUPERVISOR_UNIT_NAME {
            "io.farhelm.supervisor"
        } else {
            &self.unit_name
        }
    }

    /// Choose the service file from the same inspected target as its actions.
    fn service_file_name(&self, reach: &Reach) -> String {
        if reach.arch.is_macos() {
            format!("{}.plist", self.launchd_label())
        } else {
            self.unit_name.clone()
        }
    }
    /// Standard per-user paths, with the local supervisor sharing the helm's
    /// state directory as the local transport requires.
    pub(super) fn production(local_state_dir: PathBuf) -> Self {
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
    /// A destination without a file name is refused here instead of reaching
    /// the temporary-name builder, whose old unchecked assumption panicked on
    /// values such as `/` and `..`. That refusal is a configuration problem
    /// the user has to fix, not a host failure, so it is a typed
    /// [`ProvisioningRequestError::Refused`] (409), never a
    /// [`BackendFailure`] (which the HTTP layer answers as 502, the status a
    /// client may treat as transient and retry).
    pub(super) fn plan(
        &self,
        operation: ProvisioningOperation,
        target: ProvisioningTarget,
        reach: &Reach,
        run_nonce: &str,
    ) -> anyhow::Result<ProvisioningPlan> {
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
        let unit_path = unit_dir.join(self.service_file_name(reach));
        let farhelm_name = farhelm_path.file_name().ok_or_else(|| {
            anyhow::Error::new(ProvisioningRequestError::Refused(format!(
                "provisioning farhelm destination {:?} has no file name",
                farhelm_path
            )))
        })?;
        let unit_name = unit_path.file_name().ok_or_else(|| {
            anyhow::Error::new(ProvisioningRequestError::Refused(format!(
                "provisioning unit destination {:?} has no file name",
                unit_path
            )))
        })?;
        let temporary = |path: &Path, name: &std::ffi::OsStr| {
            path.with_file_name(format!(
                ".{}.farhelm-tmp-{run_nonce}",
                name.to_string_lossy()
            ))
        };
        // `lib_dir` is Farhelm's private directory, where earlier releases
        // also put their private tmux. An UPDATE may install the farhelm binary
        // elsewhere (the registered path, often a shared bin directory on the
        // user's PATH); that directory is ensured too, but nothing else of
        // Farhelm's is placed in it, which is why it is `shared` and never
        // chmodded (see `DirectorySpec`).
        let mut directories = vec![DirectorySpec {
            path: lib_dir.clone(),
            mode: 0o755,
            shared: false,
        }];
        if let Some(binary_dir) = farhelm_path.parent()
            && binary_dir != lib_dir
        {
            directories.push(DirectorySpec {
                path: binary_dir.to_path_buf(),
                mode: 0o755,
                shared: true,
            });
        }
        let mut actions = vec![ProvisioningAction::EnsureDirectories {
            directories: directories
                .into_iter()
                .chain([
                    DirectorySpec {
                        path: state_dir.clone(),
                        mode: 0o700,
                        shared: false,
                    },
                    DirectorySpec {
                        path: unit_dir,
                        mode: 0o755,
                        shared: true,
                    },
                ])
                .collect(),
        }];
        let remote = matches!(&target, ProvisioningTarget::Remote(_));
        let farhelm_temporary = temporary(&farhelm_path, farhelm_name);
        if remote {
            actions.push(ProvisioningAction::UploadPayload {
                arch: reach.arch,
                destination: farhelm_path.clone(),
                temporary: farhelm_temporary.clone(),
            });
        }
        actions.push(ProvisioningAction::InstallPayload {
            arch: reach.arch,
            destination: farhelm_path.clone(),
            temporary: farhelm_temporary,
        });
        if reach.arch.is_macos() {
            let domain = reach.launchd_domain.clone().ok_or_else(|| {
                BackendFailure::new("the Mac launchd domain was not inspected", "")
            })?;
            let label = self.launchd_label().to_string();
            actions.extend([
                ProvisioningAction::WriteLaunchAgent {
                    destination: unit_path.clone(),
                    temporary: temporary(&unit_path, unit_name),
                    content: super::macos::supervisor_plist(&farhelm_path, &state_dir, &label)?,
                },
                ProvisioningAction::StartLaunchAgent {
                    domain,
                    label,
                    plist: unit_path,
                    refresh: operation == ProvisioningOperation::Update,
                },
                ProvisioningAction::AttachSupervisor,
            ]);
            return Ok(ProvisioningPlan {
                operation,
                target,
                farhelm_path,
                state_dir,
                actions,
                host_home: reach.home.clone(),
                host_distro_id: reach.distro_id.clone(),
                host_arch: reach.arch,
            });
        }
        let content = supervisor_unit(&farhelm_path, &state_dir)?;
        actions.extend([
            ProvisioningAction::WriteUnit {
                unit: self.unit_name.clone(),
                destination: unit_path.clone(),
                temporary: temporary(&unit_path, unit_name),
                content,
            },
            ProvisioningAction::DaemonReload,
            ProvisioningAction::EnableSupervisor {
                unit: self.unit_name.clone(),
                unit_path: unit_path.clone(),
                persistent_run: "The supervisor runs persistently under the systemd user manager"
                    .to_string(),
            },
            ProvisioningAction::EnableLinger {
                boot_start_if_enabled: "The supervisor starts at boot if linger succeeds"
                    .to_string(),
                login_start_if_refused: "starts at login, not at boot".to_string(),
            },
        ]);
        if operation == ProvisioningOperation::Update {
            actions.push(ProvisioningAction::RestartSupervisor {
                unit: self.unit_name.clone(),
            });
        }
        actions.push(ProvisioningAction::RemoveBundledTmux {
            path: lib_dir.join("tmux"),
        });
        actions.push(ProvisioningAction::AttachSupervisor);
        Ok(ProvisioningPlan {
            operation,
            target,
            farhelm_path,
            state_dir,
            actions,
            host_home: reach.home.clone(),
            host_distro_id: reach.distro_id.clone(),
            host_arch: reach.arch,
        })
    }

    /// The paths an UNINSTALL would act on for a host with this reach,
    /// frozen from the same layout and overrides an install uses.
    ///
    /// Never re-derived at run time from the host's home: the real-transport
    /// test runs uninstall against the machine executing it with fixture
    /// overrides, and a re-derived path would name the real
    /// `~/.local/lib/farhelm` and `farhelm-supervisor.service`, which may be
    /// the maintainer's own install.
    pub(super) fn uninstall_paths(&self, reach: &Reach) -> UninstallPaths {
        UninstallPaths {
            lib_dir: self
                .override_lib_dir
                .clone()
                .unwrap_or_else(|| reach.home.join(".local/lib/farhelm")),
            unit_path: self
                .override_unit_dir
                .clone()
                .unwrap_or_else(|| reach.user_unit_dir.clone())
                .join(self.service_file_name(reach)),
            state_dir_override: self.override_state_dir.clone(),
        }
    }

    /// Freeze an UNINSTALL plan for a remote host, or refuse one that would
    /// remove something provisioning does not own.
    ///
    /// Only the removals still outstanding are planned: the unit file's two
    /// steps when it exists, the lib directory's when it exists. Stopping
    /// the unit, reloading the user manager and forgetting the host are
    /// always planned, because each tolerates having happened already. That
    /// is what makes a failed run retryable from wherever it stopped.
    ///
    /// The order is disable, remove the unit file, reload, stop. Removing
    /// the file before stopping keeps the supervisor answering until nothing
    /// can start it again: until then the host stays connected and a retry
    /// goes through the ordinary checks, and after it the service lets a
    /// retry proceed without a connection. Reloading first makes systemd
    /// forget the removed unit's `KillMode=process`, so stopping ends its
    /// whole control group, including its terminal hosts. This default
    /// control-group behavior was verified against a real user manager when
    /// uninstall was built. The stop then ends the state directory's
    /// terminal hosts, which an inactive unit can leave behind even after
    /// systemd has forgotten the unit.
    ///
    /// Three refusals guard the `rm -rf` of the lib directory, all on the
    /// host's symlink-resolved paths with verified alternate home prefixes
    /// normalized by the inspection. The binary the
    /// host runs, when known, must be inside it, so the plan removes the
    /// installation actually in use (SPEC.md "Supported host setup": other
    /// setups get a clear refusal). The host's state directory must not
    /// be inside it in either the normalized or the unnormalized spelling,
    /// nor by the host's file-identity check of the folders above it, so
    /// data is never removed with it. And no mount point may lie at or
    /// below it, so `rm -rf` cannot reach into another folder. The kept
    /// state path must also pass the remote command-line text check and be
    /// absolute, so a path the stop step could not name (to the shell, or to
    /// the terminal-host stop) refuses here, before confirmation, instead of
    /// failing the run after the unit is gone.
    pub(super) fn plan_uninstall(
        &self,
        target: ProvisioningTarget,
        reach: &Reach,
        paths: &UninstallPaths,
        facts: &UninstallFacts,
    ) -> anyhow::Result<ProvisioningPlan> {
        let refused =
            |message: String| anyhow::Error::new(ProvisioningRequestError::Refused(message));
        if !paths.lib_dir.is_absolute() || paths.lib_dir.parent().is_none() {
            return Err(refused(format!(
                "Farhelm's lib directory {} is not a directory uninstall can remove",
                paths.lib_dir.display()
            )));
        }
        if let Some(binary) = &facts.farhelm {
            // The inspection normalizes only aliases of the home: a
            // binary reached through that alias is inside the same lib,
            // while a shared /opt install stays outside and refused. See
            // inspect_uninstall in backend.rs for the full aliasing policy.
            let inside = facts
                .lib_dir
                .as_ref()
                .is_some_and(|lib| binary.canonical.starts_with(&lib.canonical));
            if !inside {
                return Err(refused(format!(
                    "the supervisor on this host runs {}, outside Farhelm's own directory {}; \
                     uninstall only removes a supervisor set up from the hosts panel",
                    binary.named.display(),
                    paths.lib_dir.display()
                )));
            }
        }
        // The stop step names the kept data directory in its script, and
        // the kept-data notice shows this path for copying, so it must pass
        // the same text check every remote command line applies. Checking it
        // here refuses before confirmation; left to the stop step, the run
        // would fail after it had already disabled and removed the unit. The
        // host itself reports its default state path, so nothing earlier
        // has checked that one.
        if let Err(error) = crate::units::path_text(&facts.state_dir.named) {
            return Err(refused(format!(
                "the host's Farhelm data directory has a path uninstall cannot pass to the host \
                 unchanged ({error:#}); uninstall keeps that data and names it in its steps and its \
                 notice, so it does not run on this layout"
            )));
        }
        // The stop step also names the directory to
        // `farhelm internal stop-terminal-hosts`, which takes only an
        // absolute path. Registration refuses a relative one, and the host
        // reports its default from an absolute `XDG_STATE_HOME` or from
        // `HOME`, so this refuses only an unusual host environment (a
        // relative `HOME`), and does so before confirmation instead of
        // failing the run after the unit is gone.
        if !facts.state_dir.named.is_absolute() {
            return Err(refused(format!(
                "the host's Farhelm data directory {} is not an absolute path, which uninstall \
                 needs to stop the terminals kept there; uninstall keeps that data, so it does \
                 not run on this layout",
                facts.state_dir.named.display()
            )));
        }
        // The same home normalization makes an aliased state path inside
        // the lib visible here, so removing the lib cannot remove that
        // data. General folder identity would also accept shared installs;
        // inspect_uninstall in backend.rs explains why that is forbidden.
        // That normalization is per path, so containment in either spelling
        // refuses: a layout they disagree on is one uninstall cannot tell
        // apart safely (SPEC.md, the home-alias exception).
        if let Some(lib) = &facts.lib_dir
            && (facts.state_dir.canonical.starts_with(&lib.canonical)
                || facts.state_dir.raw.starts_with(&lib.raw))
        {
            return Err(refused(format!(
                "the host's Farhelm data directory {} is inside {}, which uninstall would remove; \
                 uninstall keeps the data, so it does not run on this layout",
                facts.state_dir.named.display(),
                paths.lib_dir.display()
            )));
        }
        // Spellings cannot show every alias. Take two binds of the home,
        // the lib reached through one and the state through the other, and
        // a mount over a folder ABOVE the install folder (say ~/install,
        // which holds the lib) visible through only the home's own path.
        // Both keep their bind's spelling, so neither of the state's
        // spellings starts with either of the lib's, and no mount lies under
        // either lib spelling, so the mount check below cannot see it
        // either (a divergent mount BELOW the lib is that check's case). A
        // state that does not exist yet keeps its alias spelling the same
        // way. Only the host's -ef walk over the state's folders sees this
        // containment, so this check is not redundant with the mount check;
        // that identity only refuses.
        if facts.lib_dir.is_some() && facts.state_dir.under_lib {
            return Err(refused(format!(
                "the host's Farhelm data directory {} may be inside {}, which uninstall would \
                 remove: the data folder, or a folder above it, is that folder under another \
                 path, such as a second mount of the home folder; uninstall keeps the data, so \
                 it does not run on this layout",
                facts.state_dir.named.display(),
                paths.lib_dir.display()
            )));
        }
        // A home bound inside the lib, or a shared install mounted on it,
        // would lose its files to `rm -rf`. `rm --one-file-system` cannot
        // help: a same-filesystem bind keeps its device number, and BusyBox
        // `rm` lacks it.
        if let Some(lib) = &facts.lib_dir
            && let Some(mount) = facts
                .mounts
                .iter()
                .find(|mount| mount.starts_with(&lib.raw) || mount.starts_with(&lib.canonical))
        {
            return Err(refused(format!(
                "{} is mounted on or inside {}, which uninstall would remove; that would also \
                 delete the files the mount shows, so uninstall does not run while it is mounted",
                mount.display(),
                paths.lib_dir.display()
            )));
        }
        if reach.arch.is_macos() {
            let domain = reach.launchd_domain.clone().ok_or_else(|| {
                BackendFailure::new("the Mac launchd domain was not inspected", "")
            })?;
            let farhelm_path = facts
                .farhelm
                .as_ref()
                .map(|binary| binary.named.clone())
                .unwrap_or_else(|| paths.lib_dir.join("farhelm"));
            let mut actions = Vec::new();
            if facts.unit_file {
                actions.push(ProvisioningAction::RemoveLaunchAgent {
                    destination: paths.unit_path.clone(),
                });
            }
            actions.push(ProvisioningAction::StopLaunchAgent {
                domain,
                label: self.launchd_label().to_string(),
                farhelm_program: farhelm_path.clone(),
                state_dir: facts.state_dir.named.clone(),
            });
            if facts.lib_dir.is_some() {
                actions.push(ProvisioningAction::RemoveDirectory {
                    path: paths.lib_dir.clone(),
                });
            }
            actions.push(ProvisioningAction::ForgetHost);
            return Ok(ProvisioningPlan {
                operation: ProvisioningOperation::Uninstall,
                target,
                farhelm_path,
                state_dir: facts.state_dir.named.clone(),
                actions,
                host_home: reach.home.clone(),
                host_distro_id: reach.distro_id.clone(),
                host_arch: reach.arch,
            });
        }
        let mut actions = Vec::new();
        if facts.unit_file {
            actions.push(ProvisioningAction::DisableSupervisor {
                unit: self.unit_name.clone(),
            });
            actions.push(ProvisioningAction::RemoveUnit {
                unit: self.unit_name.clone(),
                destination: paths.unit_path.clone(),
            });
        }
        let farhelm_path = facts
            .farhelm
            .as_ref()
            .map(|binary| binary.named.clone())
            .unwrap_or_else(|| paths.lib_dir.join("farhelm"));
        actions.push(ProvisioningAction::DaemonReload);
        actions.push(ProvisioningAction::StopSupervisor {
            unit: self.unit_name.clone(),
            farhelm_program: farhelm_path.clone(),
            state_dir: facts.state_dir.named.clone(),
        });
        if facts.lib_dir.is_some() {
            actions.push(ProvisioningAction::RemoveDirectory {
                path: paths.lib_dir.clone(),
            });
        }
        actions.push(ProvisioningAction::ForgetHost);
        Ok(ProvisioningPlan {
            operation: ProvisioningOperation::Uninstall,
            target,
            farhelm_path,
            state_dir: facts.state_dir.named.clone(),
            actions,
            host_home: reach.home.clone(),
            host_distro_id: reach.distro_id.clone(),
            host_arch: reach.arch,
        })
    }

    /// UPDATE converges the installation the registry actually dials. A
    /// relative `remote_farhelm` is accepted at registration because SSH
    /// resolves it through PATH, but is refused here: installing to a guessed
    /// absolute layout would update a binary different from the one in use.
    /// Like [`Self::plan`]'s own refusals, that is a typed
    /// [`ProvisioningRequestError::Refused`]: the fix is the registration,
    /// not a retry.
    pub(super) fn plan_for_row(
        &self,
        row: &HostRow,
        target: ProvisioningTarget,
        reach: &Reach,
        run_nonce: &str,
    ) -> anyhow::Result<ProvisioningPlan> {
        let mut layout = self.clone();
        if row.kind.has_remote_install() {
            if let Some(farhelm) = &row.remote_farhelm {
                if !Path::new(farhelm).is_absolute() {
                    return Err(anyhow::Error::new(ProvisioningRequestError::Refused(
                        format!(
                            "updating in place needs an absolute remote_farhelm; registered value {farhelm:?} is resolved through the remote PATH instead",
                        ),
                    )));
                }
                // Only the binary's own path follows the registration: the
                // bundled tmux an earlier release left is removed from
                // Farhelm's private directory, where every release since
                // 0.18.0 put it, never from beside a registered binary that
                // may live in a shared bin directory next to the user's own
                // (where releases before 0.18.0 put theirs, deliberately
                // left alone since then).
                layout.override_farhelm_path = Some(PathBuf::from(farhelm));
            }
            if let Some(state_dir) = &row.remote_state_dir {
                layout.override_state_dir = Some(PathBuf::from(state_dir));
            }
        }
        layout.plan(ProvisioningOperation::Update, target, reach, run_nonce)
    }
}

/// Carry a unit-rendering refusal into provisioning's failure type.
///
/// Rendering never touches the host, so there is no host stderr to
/// preserve: the whole explanation is in the message.
fn unit_failure(error: anyhow::Error) -> BackendFailure {
    BackendFailure::new(format!("{error:#}"), "")
}

/// Render the supervisor unit from the paths carried by the plan.
///
/// A thin adapter over [`crate::units::render_supervisor_unit`], which
/// `farhelm helm setup` shares: one renderer means remote provisioning and
/// local setup cannot ship different lifecycle behaviour.
///
/// The unit is deliberately left UNMARKED — the managed-by marker means
/// "setup owns this file", and provisioning's remote units are owned by
/// the provisioning workflow instead.
///
/// The unit names no tmux and puts no tmux directory on `PATH`: the
/// supervisor starts none. Its `PATH` is the directory holding `farhelm`
/// and the base directories the shared renderer appends.
pub(super) fn supervisor_unit(farhelm: &Path, state_dir: &Path) -> Result<String, BackendFailure> {
    render_supervisor_unit(&SupervisorUnitInputs { farhelm, state_dir }).map_err(unit_failure)
}

#[cfg(test)]
mod macos_plan_tests {
    use super::*;

    /// A Mac supplies a launchd domain.
    fn reach() -> Reach {
        Reach {
            home: "/Users/host".into(),
            user_unit_dir: "/Users/host/Library/LaunchAgents".into(),
            arch: PayloadTarget::MacosAarch64,
            distro_id: "macOS".into(),
            launchd_domain: Some("user/501".into()),
        }
    }

    /// Setup/Update must expose the same frozen native actions in question
    /// and progress, with no hidden systemd work or reboot promise. A Mac
    /// never had a bundled tmux, so its plans carry no step removing one.
    #[test]
    fn mac_actions_are_explicit_and_linux_serialization_is_preserved() {
        assert_eq!(
            serde_json::to_string(&PayloadTarget::LinuxX86_64).unwrap(),
            "\"x86_64\""
        );
        assert_eq!(
            serde_json::to_string(&PayloadTarget::LinuxAarch64).unwrap(),
            "\"aarch64\""
        );
        let layout = PlanLayout::production("/local/state".into());
        for operation in [ProvisioningOperation::Add, ProvisioningOperation::Update] {
            let plan = layout
                .plan(
                    operation,
                    ProvisioningTarget::ssh("mac.example"),
                    &reach(),
                    "nonce",
                )
                .unwrap();
            assert_eq!(
                plan.actions
                    .iter()
                    .map(ProvisioningAction::label)
                    .collect::<Vec<_>>(),
                [
                    "create-directories",
                    "upload-farhelm",
                    "install-farhelm",
                    "write-launch-agent",
                    if operation == ProvisioningOperation::Update {
                        "restart-launch-agent"
                    } else {
                        "start-launch-agent"
                    },
                    "attach-supervisor"
                ]
            );
            let text = plan.confirmation();
            assert!(
                text.contains("io.farhelm.supervisor.plist")
                    && text.contains("restarts after a crash")
            );
            assert!(!text.contains("tmux") && !text.contains("systemd"));
        }
    }

    /// Mac uninstall keeps the shared containment guarantees but has no
    /// disable override or reload to run.
    #[test]
    fn mac_uninstall_removes_plist_before_stopping_and_keeps_data() {
        let layout = PlanLayout::production("/local/state".into());
        let reach = reach();
        let paths = layout.uninstall_paths(&reach);
        let resolved = |path: PathBuf| ResolvedPath {
            named: path.clone(),
            canonical: path.clone(),
            raw: path,
            under_lib: false,
        };
        let facts = UninstallFacts {
            unit_file: true,
            lib_dir: Some(resolved(paths.lib_dir.clone())),
            farhelm: Some(resolved(paths.lib_dir.join("farhelm"))),
            state_dir: resolved(reach.home.join(".local/state/farhelm")),
            mounts: vec![],
        };
        let plan = layout
            .plan_uninstall(
                ProvisioningTarget::ssh("mac.example"),
                &reach,
                &paths,
                &facts,
            )
            .unwrap();
        assert_eq!(
            plan.actions
                .iter()
                .map(ProvisioningAction::label)
                .collect::<Vec<_>>(),
            [
                "remove-launch-agent",
                "stop-launch-agent",
                "remove-directory",
                "forget-host"
            ]
        );
        let text = plan.confirmation();
        assert!(text.contains("Kept: the host's Farhelm data"));
        assert!(!text.contains("tmux") && !text.contains("systemd"));
    }
}
