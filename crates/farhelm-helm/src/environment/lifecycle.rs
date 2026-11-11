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
    change_with_manager(&state.manager, host, resume).await
}

/// Automatic starts use the same operation from an owned task. The actor
/// must return to its nudge wait before this operation takes its hold.
pub(crate) async fn change_with_manager(
    manager: &crate::manager::ConnectionManager,
    host: HostId,
    resume: bool,
) -> anyhow::Result<()> {
    let row = manager
        .store()
        .list_hosts()
        .await?
        .into_iter()
        .find(|row| row.id == host)
        .ok_or(crate::store::HostStoreError::HostNotFound(host))?;
    let binding = row.environment_binding().ok_or_else(|| {
        crate::SupervisorError::helm(
            ErrorKind::Conflict,
            "Suspend and Resume are available only for environment hosts".into(),
        )
    })?;
    let provider = manager
        .providers()
        .get(binding.provider())
        .map_err(|reason| crate::SupervisorError::helm(ErrorKind::Conflict, reason.0))?;
    if resume {
        provider.resume(manager, &binding).await
    } else {
        provider.suspend(manager, &binding).await
    }
}

/// Share connection withdrawal and recovery after provider validation. The
/// caller holds the provisioning lock and validates credentials first. The
/// transition reports whether it changed the resource; recording may supply
/// a confirmed display state before the hold is released.
pub(crate) async fn perform<T, R, RF>(
    manager: &crate::manager::ConnectionManager,
    row: &crate::store::HostRow,
    resume: bool,
    changing: crate::manager::HostState,
    transition: T,
    record: R,
) -> anyhow::Result<()>
where
    T: std::future::Future<Output = anyhow::Result<bool>>,
    R: FnOnce(bool) -> RF,
    RF: std::future::Future<Output = anyhow::Result<Option<crate::manager::HostState>>>,
{
    let host = row.id;
    let held = !resume
        || !manager
            .status(host)
            .is_some_and(|status| status.state.is_connected());
    if held {
        manager.hold_connection(host, changing.clone()).await;
        manager.transport().release_shared_connections(row).await;
    }
    let result = async {
        let transitioned = transition.await?;
        if resume && transitioned && !held {
            // The actor may still show the connection from before an
            // external suspension. A real resume must withdraw it too.
            manager.hold_connection(host, changing).await;
            manager.transport().release_shared_connections(row).await;
        }
        if let Some(confirmed) = record(transitioned).await? {
            manager.hold_connection(host, confirmed).await;
        }
        Ok(())
    }
    .await;
    if !resume
        || !manager
            .status(host)
            .is_some_and(|status| status.state.is_connected())
    {
        manager.release_hold(host, result.is_err());
        if resume && result.is_ok() {
            let _ = manager.retry_now_with_fresh_window(host).await?;
        }
    }
    result
}
