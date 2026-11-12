//! Installation work that needs a backend and payload source, but no host row.
//!
//! Ordinary setup owns attachment and progress. Snapshot preparation instead
//! stops before attachment, so sharing only these actions keeps credentials,
//! tunnels and host bookkeeping out of the preparer's dependencies.

use super::backend::{
    ActionOutcome, BackendFailure, PreparedPayload, ProvisioningBackend, stage_payload,
};
use super::payloads::PayloadSource;
use super::plan::{ProvisioningAction, ProvisioningPlan};

/// Resolve and stage the plan's one payload, the `farhelm` binary for the
/// inspected target, before any host action runs.
///
/// `None` for a plan that installs nothing (uninstall). Every Add and
/// Update plan uploads or installs exactly one payload, so the first such
/// action names it; a failure carries that action's index so progress
/// reports it on the step the user is looking at, while nothing on the host
/// has been touched yet.
pub(super) async fn prepare_payloads(
    payloads: &dyn PayloadSource,
    plan: &ProvisioningPlan,
) -> Result<Option<PreparedPayload>, (usize, BackendFailure)> {
    let Some((index, arch)) =
        plan.actions
            .iter()
            .enumerate()
            .find_map(|(index, action)| match action {
                ProvisioningAction::UploadPayload { arch, .. }
                | ProvisioningAction::InstallPayload { arch, .. } => Some((index, *arch)),
                _ => None,
            })
    else {
        return Ok(None);
    };
    let source = payloads
        .path(arch)
        .await
        .map_err(|error| (index, BackendFailure::new(format!("{error:#}"), "")))?;
    stage_payload(&source)
        .await
        .map(Some)
        .map_err(|error| (index, error))
}

/// Execute an Add installation action without any host connection authority.
///
/// Attach, Update-only and Uninstall actions are refused. The ordinary
/// service dispatches those itself; preparation explicitly omits attachment
/// and must fail if planning ever adds an action it does not understand.
///
/// The bundled tmux's removal is refused here too, although Add plans it.
/// The ordinary service runs it itself because a failed removal only warns
/// there, and snapshot preparation skips it: the sandbox image it installs
/// into has no Farhelm installation (preparation refuses setup-managed
/// installs and copied state), so it is not expected to carry an earlier
/// tmux.
pub(super) async fn execute_install_action(
    backend: &dyn ProvisioningBackend,
    plan: &ProvisioningPlan,
    action: &ProvisioningAction,
    prepared: Option<&PreparedPayload>,
) -> Result<ActionOutcome, BackendFailure> {
    match action {
        ProvisioningAction::EnsureDirectories { directories } => {
            backend.ensure_directories(&plan.target, directories).await
        }
        ProvisioningAction::UploadPayload {
            destination,
            temporary,
            ..
        } => {
            let source = prepared.ok_or_else(|| {
                BackendFailure::new("the prepared payload disappeared before upload", "")
            })?;
            backend
                .upload_path(&plan.target, source, destination, temporary)
                .await
        }
        ProvisioningAction::InstallPayload {
            destination,
            temporary,
            ..
        } => {
            let source = prepared.ok_or_else(|| {
                BackendFailure::new("the prepared payload disappeared before installation", "")
            })?;
            backend
                .install_path(&plan.target, source, destination, temporary, 0o755)
                .await
        }
        ProvisioningAction::WriteUnit {
            content,
            destination,
            temporary,
            ..
        } => {
            backend
                .install_bytes(
                    &plan.target,
                    content.as_bytes(),
                    destination,
                    temporary,
                    0o644,
                )
                .await
        }
        ProvisioningAction::DaemonReload => backend.daemon_reload(&plan.target).await,
        ProvisioningAction::EnableSupervisor {
            unit, unit_path, ..
        } => backend.enable_now(&plan.target, unit, unit_path).await,
        ProvisioningAction::EnableLinger { .. } => backend.enable_linger(&plan.target).await,
        _ => Err(BackendFailure::new(
            format!("{} is not a snapshot installation action", action.label()),
            "",
        )),
    }
}
