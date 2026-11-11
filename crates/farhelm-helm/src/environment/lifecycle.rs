//! Explicit environment operations select the provider from the stored binding.
//! Each provider retains operation validation before taking connection holds.
//! The Tensorlake adapter keeps its existing lifecycle orchestration so a
//! missing account key cannot disconnect an otherwise usable supervisor.

use crate::{AppState, store::HostId};
use farhelm_proto::ErrorKind;

/// Dispatch the ordinary host Suspend/Resume path without changing its locking
/// or cancellation lifetime. The provider re-reads the row under its existing
/// provisioning lock before changing a resource.
pub(crate) async fn change(state: &AppState, host: HostId, resume: bool) -> anyhow::Result<()> {
    let row = crate::hosts::registry_row(state, host).await?;
    let binding = row.environment_binding().ok_or_else(|| {
        crate::SupervisorError::helm(
            ErrorKind::Conflict,
            "Suspend and Resume are available only for Tensorlake hosts".into(),
        )
    })?;
    let provider = state
        .providers
        .get(binding.provider())
        .map_err(|reason| crate::SupervisorError::helm(ErrorKind::Conflict, reason.0))?;
    if resume {
        provider.resume(state, &binding).await
    } else {
        provider.suspend(state, &binding).await
    }
}
