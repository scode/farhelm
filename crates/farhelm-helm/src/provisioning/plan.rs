//! The confirmed plan is the only description of the work: confirmation and
//! execution walk the same frozen actions.

use super::backend::{BackendFailure, Reach, path_text};
use crate::store::{HostKind, HostRow};
use serde::Serialize;
use std::path::{Path, PathBuf};

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

/// One directory and the mode provisioning must converge on every rerun.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct DirectorySpec {
    pub(super) path: PathBuf,
    pub(super) mode: u32,
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
    pub(super) fn label(&self) -> &'static str {
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
    pub(super) operation: ProvisioningOperation,
    pub(super) target: ProvisioningTarget,
    pub(super) farhelm_path: PathBuf,
    pub(super) state_dir: PathBuf,
    pub(super) actions: Vec<ProvisioningAction>,
}

impl ProvisioningPlan {
    /// Render the plan without maintaining a second list of promises.
    pub(super) fn confirmation(&self) -> String {
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
    pub(super) fn plan(
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
    pub(super) fn plan_for_row(
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
pub(super) fn systemd_arg(path: &Path) -> Result<String, BackendFailure> {
    Ok(format!(
        "\"{}\"",
        path_text(path)?
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%")
    ))
}

const SUPERVISOR_UNIT_TEMPLATE: &str =
    include_str!("../../../../release/farhelm-supervisor.service.in");

/// Substitute the three reviewed unit-template fields without rescanning
/// inserted path text as template syntax.
///
/// A path may legally contain strings such as `@STATE_DIR@`. Appending each
/// replacement directly, instead of chaining `str::replace`, keeps that text
/// literal and preserves the path contract provisioning already accepts.
fn render_supervisor_unit_template(farhelm: &str, state_dir: &str, search: &str) -> String {
    let values = [
        ("@FARHELM@", farhelm),
        ("@STATE_DIR@", state_dir),
        ("@PATH@", search),
    ];
    let mut rendered = String::with_capacity(SUPERVISOR_UNIT_TEMPLATE.len() + 128);
    let mut rest = SUPERVISOR_UNIT_TEMPLATE;
    while let Some((offset, token, value)) = values
        .iter()
        .filter_map(|(token, value)| rest.find(token).map(|offset| (offset, *token, *value)))
        .min_by_key(|(offset, _, _)| *offset)
    {
        rendered.push_str(&rest[..offset]);
        rendered.push_str(value);
        rest = &rest[offset + token.len()..];
    }
    rendered.push_str(rest);
    rendered
}

/// Render the supervisor unit from the paths carried by the plan.
///
/// `release/farhelm-supervisor.service.in` is the canonical unit. Keeping its
/// fixed policy in one reviewed file prevents release packaging and remote
/// provisioning from quietly shipping different lifecycle behavior.
///
/// The existing tmux directory is retained because a user-manager process
/// does not necessarily inherit the login shell PATH that the reach check
/// used. The private payload directory stays first for hosts where Farhelm
/// supplies tmux itself. `KillMode=process` is equally deliberate: tmux owns
/// the durable sessions, so restarting their manager must stop only the
/// supervisor process rather than systemd's default whole control group.
pub(super) fn supervisor_unit(
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
    Ok(render_supervisor_unit_template(
        &systemd_arg(farhelm)?,
        &systemd_arg(state_dir)?,
        &search,
    ))
}
