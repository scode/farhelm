//! Installation work that needs a backend and payload source, but no host row.
//!
//! Ordinary setup owns attachment and progress. Snapshot preparation instead
//! stops before attachment, so sharing only these actions keeps credentials,
//! tunnels and host bookkeeping out of the preparer's dependencies.

use super::backend::{
    ActionOutcome, BackendFailure, PreparedPayload, ProvisioningBackend, stage_payload,
};
use super::payloads::PayloadSource;
use super::plan::{PayloadKind, ProvisioningAction, ProvisioningPlan};
use std::collections::{HashMap, HashSet};

/// Finish every staged writer before returning the first failure in plan order.
///
/// `try_join_all` would drop siblings while their blocking cache-publication
/// tasks still write. Their future owns the cache lock, so dropping it could
/// admit a retry over an unfinished writer. No remote action may begin until
/// this function returns all payloads successfully.
pub(super) async fn prepare_payloads(
    payloads: &dyn PayloadSource,
    plan: &ProvisioningPlan,
) -> Result<HashMap<PayloadKind, PreparedPayload>, (usize, BackendFailure)> {
    let mut seen = HashSet::new();
    let preparations = plan
        .actions
        .iter()
        .enumerate()
        .filter_map(|(index, action)| match action {
            ProvisioningAction::UploadPayload { payload, arch, .. }
            | ProvisioningAction::InstallPayload { payload, arch, .. }
                if seen.insert(*payload) =>
            {
                Some((index, *payload, *arch))
            }
            _ => None,
        })
        .map(|(index, payload, arch)| async move {
            let source = payloads
                .path(payload, arch)
                .await
                .map_err(|error| (index, BackendFailure::new(format!("{error:#}"), "")))?;
            let staged = stage_payload(&source)
                .await
                .map_err(|error| (index, error))?;
            Ok::<_, (usize, BackendFailure)>((payload, staged))
        });
    let mut prepared = HashMap::new();
    let mut failure: Option<(usize, BackendFailure)> = None;
    for outcome in futures_util::future::join_all(preparations).await {
        match outcome {
            Ok((payload, staged)) => {
                prepared.insert(payload, staged);
            }
            Err((index, error)) => {
                if failure.as_ref().is_none_or(|(first, _)| index < *first) {
                    failure = Some((index, error));
                }
            }
        }
    }
    match failure {
        Some(failure) => Err(failure),
        None => Ok(prepared),
    }
}

/// Execute an Add installation action without any host connection authority.
///
/// Attach, Update and Uninstall actions are refused. The ordinary service
/// dispatches those itself; preparation explicitly omits attachment and must
/// fail if planning ever adds an action it does not understand.
pub(super) async fn execute_install_action(
    backend: &dyn ProvisioningBackend,
    plan: &ProvisioningPlan,
    action: &ProvisioningAction,
    prepared: &HashMap<PayloadKind, PreparedPayload>,
) -> Result<ActionOutcome, BackendFailure> {
    match action {
        ProvisioningAction::EnsureDirectories { directories } => {
            backend.ensure_directories(&plan.target, directories).await
        }
        ProvisioningAction::UploadPayload {
            payload,
            destination,
            temporary,
            ..
        } => {
            let source = prepared.get(payload).ok_or_else(|| {
                BackendFailure::new("the prepared payload disappeared before upload", "")
            })?;
            backend
                .upload_path(&plan.target, *payload, source, destination, temporary)
                .await
        }
        ProvisioningAction::InstallPayload {
            payload,
            destination,
            temporary,
            ..
        } => {
            let source = prepared.get(payload).ok_or_else(|| {
                BackendFailure::new("the prepared payload disappeared before installation", "")
            })?;
            backend
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
