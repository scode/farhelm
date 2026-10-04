//! The confirmed plan is the only description of the work: confirmation and
//! execution walk the same frozen actions.

use super::backend::{BackendFailure, Reach};
use super::http::ProvisioningRequestError;
use crate::store::HostRow;
use crate::units::{SupervisorUnitInputs, render_supervisor_unit};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Whether a plan is converging an absent install, explicitly updating an
/// existing one, or removing what provisioning installed. ADD never turns into
/// UPDATE implicitly, and neither ever turns into UNINSTALL.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ProvisioningOperation {
    Add,
    Update,
    /// Remove the supervisor's service, its unit file and Farhelm's private
    /// lib directory from a remote host, keep the host's state directory, and
    /// forget the host. Only ever planned for a remote row; the helm's own
    /// machine is uninstalled with `farhelm uninstall`.
    Uninstall,
}

impl ProvisioningOperation {
    /// What the panel says instead of acting on a host whose supervisor unit
    /// carries `farhelm helm setup`'s managed-by marker.
    ///
    /// One refusal per operation rather than one string for all of them:
    /// setup owns that unit on the host (SPEC.md "Ownership during cleanup
    /// and provisioning"), so the remedy depends on what the user was trying
    /// to do. Installing or updating there is the host's own installer and
    /// setup; removing is the host's own `farhelm uninstall`, which knows how
    /// to take down a setup-owned service.
    pub(super) fn setup_managed_refusal(self) -> String {
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
/// from the local no-SSH path to SSH-to-self, or vice versa.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(super) enum ProvisioningTarget {
    Local,
    Ssh { destination: String },
}

/// Installation artifacts selected independently; callers must never use a
/// Farhelm executable to satisfy a tmux request or vice versa.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum PayloadKind {
    Farhelm,
    Tmux,
}

/// Architectures with release payloads. Reach inspection maps the remote
/// machine to one of these before confirmation, never during execution.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum PayloadArch {
    X86_64,
    Aarch64,
}

/// The bare architecture spelling (`x86_64`, `aarch64`) used in the
/// confirmation plan's host line — distinct from the full target triples
/// `assets.rs` uses for release filenames, which nobody wants to read in
/// the confirmation text.
impl std::fmt::Display for PayloadArch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::X86_64 => "x86_64",
            Self::Aarch64 => "aarch64",
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
/// template. Adding executor behavior therefore requires adding a line to
/// the plan text the same request authorizes — setup confirmation shows it,
/// and remote updates consume it sight unseen from the same rendering.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "step", rename_all = "kebab-case")]
pub(crate) enum ProvisioningAction {
    EnsureDirectories {
        directories: Vec<DirectorySpec>,
    },
    /// Remote plans transfer the verified snapshot to this nonce temporary
    /// before the install step can change the live executable. Local plans
    /// copy within their install action and have no upload step.
    UploadPayload {
        payload: PayloadKind,
        arch: PayloadArch,
        destination: PathBuf,
        temporary: PathBuf,
    },
    /// Install the local snapshot or the remote upload from this same plan.
    /// A remote install must not start another network transfer.
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
    /// Stop the supervisor. Its unit's `KillMode=process` means only the
    /// supervisor process goes; nothing else was running there, because
    /// uninstall refuses while any session or terminal tab is alive.
    StopSupervisor {
        unit: String,
    },
    /// Delete Farhelm's private lib directory: the `farhelm` binary and any
    /// private tmux provisioning put beside it. Planning refuses a layout in
    /// which the state directory lies inside it, so this never removes data.
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
    pub(super) fn label(&self) -> &'static str {
        match self {
            Self::EnsureDirectories { .. } => "create-directories",
            Self::UploadPayload {
                payload: PayloadKind::Farhelm,
                ..
            } => "upload-farhelm",
            Self::UploadPayload {
                payload: PayloadKind::Tmux,
                ..
            } => "upload-tmux",
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
            Self::DisableSupervisor { .. } => "disable-supervisor",
            Self::RemoveUnit { .. } => "remove-unit",
            Self::StopSupervisor { .. } => "stop-supervisor",
            Self::RemoveDirectory { .. } => "remove-directory",
            Self::ForgetHost => "forget-host",
        }
    }

    /// Describe only a user-visible change on the host.
    ///
    /// Transfer staging, digest checks, atomic renames, daemon reloads, and
    /// attaching the resulting supervisor are execution details. They remain
    /// in the frozen action list, but the add-host question names the durable
    /// host changes the user is deciding to authorize.
    fn confirmation_line(&self) -> Option<String> {
        match self {
            Self::EnsureDirectories { directories } => Some(
                directories
                    .iter()
                    .map(|directory| format!("create or reuse directory {}", directory.path.display()))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            Self::UploadPayload { .. } | Self::DaemonReload | Self::AttachSupervisor => None,
            Self::InstallPayload {
                payload,
                destination,
                ..
            } => Some(format!(
                "place {} at {}",
                match payload {
                    PayloadKind::Farhelm => "Farhelm",
                    PayloadKind::Tmux => "tmux",
                },
                destination.display()
            )),
            Self::WriteUnit {
                unit, destination, ..
            } => Some(format!("write user service {unit} at {}", destination.display())),
            Self::EnableSupervisor {
                unit,
                persistent_run,
                ..
            } => Some(format!("enable and start {unit}; {persistent_run}")),
            Self::EnableLinger { .. } => Some(
                "start the supervisor at boot when user lingering is enabled; if lingering is refused, start it at login instead"
                    .to_string(),
            ),
            Self::RestartSupervisor { unit } => Some(format!("restart {unit}")),
            Self::DisableSupervisor { unit } => {
                Some(format!("disable user service {unit} so it no longer starts"))
            }
            Self::RemoveUnit { destination, .. } => {
                Some(format!("remove its unit file {}", destination.display()))
            }
            Self::StopSupervisor { unit } => Some(format!("stop {unit}")),
            Self::RemoveDirectory { path } => Some(format!(
                "remove {} with the Farhelm binary and any private tmux in it",
                path.display()
            )),
            Self::ForgetHost => Some("remove this host from the host list".to_string()),
        }
    }
}

/// The exact value the confirmation text renders and execution later consumes.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct ProvisioningPlan {
    pub(super) operation: ProvisioningOperation,
    pub(super) target: ProvisioningTarget,
    pub(super) farhelm_path: PathBuf,
    pub(super) state_dir: PathBuf,
    pub(super) actions: Vec<ProvisioningAction>,
    /// The reach probe's validated `/etc/os-release` `ID`, empty when the
    /// host had none, and the payload architecture it matched — carried
    /// through purely for `confirmation()`'s host line. Validation happens
    /// at the reach-record boundary because this value becomes plan text;
    /// provisioning still gates on capabilities, not on this ID (see
    /// `Reach::distro_id`), so nothing here or in execution branches on it.
    pub(super) host_distro_id: String,
    pub(super) host_arch: PayloadArch,
}

impl ProvisioningPlan {
    /// Render the plan without maintaining a second list of promises.
    ///
    /// The heading names the destination, distribution, and architecture so
    /// distro-agnostic provisioning still gives the user a chance to notice
    /// an unexpected host before authorizing its durable changes.
    ///
    /// UNINSTALL's heading says so instead of "set up", and its text ends
    /// with what it keeps: the host's state directory is not an action, but
    /// the user is deciding whether to remove Farhelm and needs to know
    /// where the data stays and that deleting it is theirs to do.
    pub(super) fn confirmation(&self) -> String {
        let mut rendered = format!(
            "Farhelm will {} {} ({}, {}):\n",
            match self.operation {
                ProvisioningOperation::Add | ProvisioningOperation::Update => "set up",
                ProvisioningOperation::Uninstall => "uninstall from",
            },
            match &self.target {
                ProvisioningTarget::Local => "the local host".to_string(),
                ProvisioningTarget::Ssh { destination } => destination.clone(),
            },
            if self.host_distro_id.is_empty() {
                "unknown distribution"
            } else {
                &self.host_distro_id
            },
            self.host_arch,
        );
        for action in &self.actions {
            if let Some(line) = action.confirmation_line() {
                for line in line.lines() {
                    rendered.push_str("- ");
                    rendered.push_str(line);
                    rendered.push('\n');
                }
            }
        }
        if self.operation == ProvisioningOperation::Uninstall {
            rendered.push_str(&format!(
                "Kept: the host's Farhelm data in {}. Deleting that directory by hand removes the \
                 data too.\n",
                self.state_dir.display()
            ));
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
    /// The lib directory's canonical path when it still exists; `None`
    /// when it is already gone.
    pub(super) lib_dir: Option<PathBuf>,
    /// The binary the supervisor was dialed through, when one answered.
    pub(super) farhelm: Option<ResolvedPath>,
    /// The state directory the host uses, which uninstall keeps.
    pub(super) state_dir: ResolvedPath,
}

/// A path as named to the user, beside where it really leads on the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResolvedPath {
    pub(super) named: PathBuf,
    pub(super) canonical: PathBuf,
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
        let unit_path = unit_dir.join(&self.unit_name);
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
        // `lib_dir` is Farhelm's private directory, which is also where the
        // private tmux goes. An UPDATE may install the farhelm binary
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
        let remote = matches!(&target, ProvisioningTarget::Ssh { .. });
        let farhelm_temporary = temporary(&farhelm_path, farhelm_name);
        if remote {
            actions.push(ProvisioningAction::UploadPayload {
                payload: PayloadKind::Farhelm,
                arch: reach.arch,
                destination: farhelm_path.clone(),
                temporary: farhelm_temporary.clone(),
            });
        }
        actions.push(ProvisioningAction::InstallPayload {
            payload: PayloadKind::Farhelm,
            arch: reach.arch,
            destination: farhelm_path.clone(),
            temporary: farhelm_temporary,
        });
        // Whichever branch runs, `tmux_program` ends up naming the exact
        // executable the supervisor must drive — see `supervisor_unit`
        // for why a directory on PATH is not enough.
        let tmux_program = if reach.needs_tmux {
            let tmux_path = lib_dir.join("tmux");
            let tmux_temporary = temporary(
                &tmux_path,
                tmux_path.file_name().ok_or_else(|| {
                    BackendFailure::new(
                        format!(
                            "provisioning tmux destination {:?} has no file name",
                            tmux_path
                        ),
                        "",
                    )
                })?,
            );
            if remote {
                actions.push(ProvisioningAction::UploadPayload {
                    payload: PayloadKind::Tmux,
                    arch: reach.arch,
                    destination: tmux_path.clone(),
                    temporary: tmux_temporary.clone(),
                });
            }
            actions.push(ProvisioningAction::InstallPayload {
                payload: PayloadKind::Tmux,
                arch: reach.arch,
                destination: tmux_path.clone(),
                temporary: tmux_temporary,
            });
            tmux_path
        } else {
            reach
                .host_tmux
                .clone()
                .expect("reach accepted the host tmux, so it recorded which one")
        };
        let content = supervisor_unit(&farhelm_path, &state_dir, &tmux_program)?;
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
                .join(&self.unit_name),
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
    /// The order is disable, remove the unit file, stop, reload. Removing
    /// the file before stopping keeps the supervisor answering until nothing
    /// can start it again: until then the host stays connected and a retry
    /// goes through the ordinary checks, and after it the service lets a
    /// retry proceed without a connection. Stopping before reloading is what
    /// keeps the unit's `KillMode=process`: systemd forgets a removed unit's
    /// settings when the manager reloads, and stopping it then would end its
    /// whole control group, the sessions' tmux server included. (Verified
    /// against a real user manager while this was written: after removing
    /// the file and reloading, `KillMode` reads `control-group` and a stop
    /// empties the group.)
    ///
    /// Two refusals guard the `rm -rf` of the lib directory, both on the
    /// host's canonical paths rather than their spelling. The binary the
    /// host runs, when known, must be inside it, so the plan removes the
    /// installation actually in use (SPEC.md "Supported host setup": other
    /// setups get a clear refusal). And the host's state directory must not
    /// be inside it, so data is never removed with it.
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
            let inside = facts
                .lib_dir
                .as_ref()
                .is_some_and(|lib| binary.canonical.starts_with(lib));
            if !inside {
                return Err(refused(format!(
                    "the supervisor on this host runs {}, outside Farhelm's own directory {}; \
                     uninstall only removes a supervisor set up from the hosts panel",
                    binary.named.display(),
                    paths.lib_dir.display()
                )));
            }
        }
        if let Some(lib) = &facts.lib_dir
            && facts.state_dir.canonical.starts_with(lib)
        {
            return Err(refused(format!(
                "the host's Farhelm data directory {} is inside {}, which uninstall would remove; \
                 uninstall keeps the data, so it does not run on this layout",
                facts.state_dir.named.display(),
                paths.lib_dir.display()
            )));
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
        actions.push(ProvisioningAction::StopSupervisor {
            unit: self.unit_name.clone(),
        });
        actions.push(ProvisioningAction::DaemonReload);
        if facts.lib_dir.is_some() {
            actions.push(ProvisioningAction::RemoveDirectory {
                path: paths.lib_dir.clone(),
            });
        }
        actions.push(ProvisioningAction::ForgetHost);
        Ok(ProvisioningPlan {
            operation: ProvisioningOperation::Uninstall,
            target,
            farhelm_path: facts
                .farhelm
                .as_ref()
                .map(|binary| binary.named.clone())
                .unwrap_or_else(|| paths.lib_dir.join("farhelm")),
            state_dir: facts.state_dir.named.clone(),
            actions,
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
                // Only the binary's own path follows the registration. The
                // private tmux stays in Farhelm's private directory: the
                // registered binary often lives in a shared bin directory on
                // the user's PATH, where a `tmux` would overwrite or shadow
                // the user's own. The unit pins tmux by absolute path, so
                // nothing needs it next to the binary.
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
/// `tmux_program` is pinned into the unit as `FARHELM_TMUX`, and that is
/// the load-bearing part: PATH alone was not enough to express which tmux
/// provisioning approved. An OBSOLETE private tmux left by an earlier
/// install used to shadow an accepted host tmux — provisioning would
/// report success, skip the payload because the host binary cleared the
/// floor, and then restart the supervisor onto the below-floor leftover it
/// had just declined to replace. Naming the executable outright removes
/// the ambiguity, on both branches: the host binary when it was accepted,
/// the freshly installed payload when it was not.
///
/// The unit's PATH now follows from those two executables (see the shared
/// renderer) rather than from a separately supplied lib and host-tmux
/// directory. That is the same set in every case provisioning produces —
/// the layout puts `farhelm` in the lib directory, and an ACCEPTED host
/// tmux is the program being pinned — with one deliberate difference: when
/// provisioning installs its own tmux payload, the directory of the host's
/// REJECTED tmux no longer joins PATH. Nothing needs it there; the payload
/// answers `tmux` by name from the lib directory that comes first anyway.
pub(super) fn supervisor_unit(
    farhelm: &Path,
    state_dir: &Path,
    tmux_program: &Path,
) -> Result<String, BackendFailure> {
    render_supervisor_unit(&SupervisorUnitInputs {
        farhelm,
        state_dir,
        tmux: tmux_program,
    })
    .map_err(unit_failure)
}
