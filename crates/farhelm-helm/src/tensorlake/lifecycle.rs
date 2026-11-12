//! Explicit suspend/resume operations. The provisioning lock serializes them
//! with setup, updates and removal; the connection hold prevents a concurrent
//! retry from reopening an ssh channel during an explicit lifecycle request.
//! The held display state records intent until Tensorlake confirms the target;
//! after Resume releases the hold, ordinary connection progress takes over.

use super::api::{ApiKey, SandboxStatus, TensorlakeError};
use crate::{AppState, manager::HostState, store::HostId};
use axum::{
    extract::{Path, State},
    response::{IntoResponse, Response},
};
use farhelm_proto::{ErrorKind, tensorlake::LifecycleDirection};
use std::{sync::Arc, time::Duration};

/// Suspend only after every connection owned by this helm has been released.
/// Request cancellation cannot interrupt the owned task and strand its hold.
pub(crate) async fn suspend(
    State(state): State<Arc<AppState>>,
    Path(host): Path<HostId>,
) -> Response {
    answer(crate::run_owned(async move { change(&state, host, false).await }).await)
}

/// Resume explicitly, then let the actor establish a fresh connection. A 200
/// means Tensorlake reports running; connection readiness follows separately.
pub(crate) async fn resume(
    State(state): State<Arc<AppState>>,
    Path(host): Path<HostId>,
) -> Response {
    answer(crate::run_owned(async move { change(&state, host, true).await }).await)
}

/// Preserve the standard REST error classification and body for both verbs.
fn answer(result: anyhow::Result<()>) -> Response {
    match result {
        Ok(()) => axum::Json(serde_json::json!({})).into_response(),
        Err(error) => crate::http_error(error),
    }
}

/// Keep the registry binding stable across the API operation. Take the same
/// lock as removal before loading the row, and release any connection hold on
/// every outcome, including a storage failure after Tensorlake has answered.
/// An already-connected Resume first checks whether work is needed, preserving
/// live viewers when the sandbox is already running. A real resume still
/// withdraws that old connection before success, even if the actor missed an
/// external suspension.
pub(crate) async fn change(state: &AppState, host: HostId, resume: bool) -> anyhow::Result<()> {
    crate::environment::lifecycle::change(state, host, resume).await
}

/// Keep account validation ahead of connection withdrawal. The provider seam
/// delegates here after selection, and this function re-reads the binding under
/// the existing provisioning lock before any provider operation.
pub(super) async fn change_tensorlake(
    service: &super::Tensorlake,
    manager: &crate::manager::ConnectionManager,
    host: HostId,
    resume: bool,
) -> anyhow::Result<()> {
    let _lock = crate::environment::lifecycle::provision_lock_or_busy(manager, host)?;
    let row = manager
        .store()
        .list_hosts()
        .await?
        .into_iter()
        .find(|row| row.id == host)
        .ok_or(crate::store::HostStoreError::HostNotFound(host))?;
    let binding = row.tensorlake.as_ref().ok_or_else(|| {
        refusal(
            ErrorKind::Conflict,
            "Suspend and Resume are available only for Tensorlake hosts",
        )
    })?;
    let key = service
        .api_key()
        .await
        .map_err(|reason| refusal(ErrorKind::Conflict, &reason))?;
    let sandbox = binding.sandbox_id.as_deref().unwrap_or(&binding.sandbox);
    let changing = HostState::TensorlakeChanging {
        starting: false,
        direction: if resume {
            LifecycleDirection::Resuming
        } else {
            LifecycleDirection::Suspending
        },
        requested_at: now_ms(),
    };
    crate::environment::lifecycle::perform(
        manager,
        &row,
        resume,
        changing,
        transition(service, &key, sandbox, resume),
        |transitioned| async move {
            let suspended_at = if resume {
                None
            } else {
                Some(if transitioned {
                    now_ms()
                } else {
                    binding.suspended_at.unwrap_or_else(now_ms)
                })
            };
            if !manager
                .store()
                .set_tensorlake_suspended_at(host, suspended_at)
                .await?
            {
                return Err(refusal(
                    ErrorKind::Conflict,
                    "This Tensorlake host is no longer registered",
                ));
            }
            Ok(suspended_at.map(|observed_at| HostState::Suspended {
                observed_at,
                suspended_by_farhelm_at: Some(observed_at),
                resume_failed: None,
            }))
        },
    )
    .await
}

/// Reconcile conflicts with the actual status rather than guessing what a
/// vendor 409 means. The deadline includes each request, so a slow final GET
/// cannot extend the advertised wait indefinitely.
/// Returns whether a mutation was needed: repeated Suspend preserves its
/// original timestamp, while a sandbox resumed outside Farhelm gets a new one.
async fn transition(
    service: &super::Tensorlake,
    key: &ApiKey,
    sandbox: &str,
    resume: bool,
) -> anyhow::Result<bool> {
    let wait = Duration::from_secs(if resume { 120 } else { 60 });
    let deadline = tokio::time::Instant::now() + wait;
    let api = service.api();
    let result = tokio::time::timeout_at(deadline, async {
        let initial = api.get_sandbox(key, sandbox).await?;
        let target = if resume {
            SandboxStatus::Running
        } else {
            SandboxStatus::Suspended
        };
        if initial.status == target {
            return Ok(false);
        }
        let request = if resume {
            api.resume_sandbox(key, sandbox).await
        } else {
            api.suspend_sandbox(key, sandbox).await
        };
        match request {
            Ok(()) | Err(TensorlakeError::Conflict { .. }) => {}
            Err(error) => return Err(error),
        }
        loop {
            let info = api.get_sandbox(key, sandbox).await?;
            if info.status == target {
                return Ok(true);
            }
            if info.status == SandboxStatus::Terminated
                || (resume && info.status == SandboxStatus::ResumeFailed)
            {
                return Err(TensorlakeError::Transport(
                    info.error_details.unwrap_or_else(|| {
                        "The sandbox could not complete the requested operation".into()
                    }),
                ));
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    })
    .await;
    match result {
        Ok(Ok(changed)) => Ok(changed),
        Ok(Err(error)) => Err(refusal(
            ErrorKind::Unavailable,
            &format!(
                "Tensorlake could not {} this host: {error}",
                if resume { "resume" } else { "suspend" }
            ),
        )),
        Err(_) => Err(refusal(
            ErrorKind::Unavailable,
            &format!(
                "Tensorlake did not finish {} this host within {} seconds; try again",
                if resume { "resuming" } else { "suspending" },
                wait.as_secs()
            ),
        )),
    }
}

/// Keep actionable lifecycle refusals distinct from internal storage errors.
fn refusal(kind: ErrorKind, message: &str) -> anyhow::Error {
    anyhow::Error::new(crate::SupervisorError::helm(kind, message.to_owned()))
}

/// Display metadata only; status checks remain the authority for dialing.
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as i64)
}
