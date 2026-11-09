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
    let _lock = state.manager.try_host_provision_lock(host).ok_or_else(|| {
        refusal(
            ErrorKind::Conflict,
            "This host is busy; try again after its current operation finishes",
        )
    })?;
    let row = crate::hosts::registry_row(state, host).await?;
    let binding = row.tensorlake.as_ref().ok_or_else(|| {
        refusal(
            ErrorKind::Conflict,
            "Suspend and Resume are available only for Tensorlake hosts",
        )
    })?;
    let key = state
        .tensorlake
        .api_key()
        .await
        .map_err(|reason| refusal(ErrorKind::Conflict, &reason))?;
    let sandbox = binding.sandbox_id.as_deref().unwrap_or(&binding.sandbox);
    let changing = HostState::TensorlakeChanging {
        direction: if resume {
            LifecycleDirection::Resuming
        } else {
            LifecycleDirection::Suspending
        },
        requested_at: now_ms(),
    };
    // An already-running, connected host must keep its viewers when Resume
    // is repeated. Every other caller publishes its intent before the API
    // request, while the hold prevents a retry from racing the operation.
    let held_before_request = !resume
        || !state
            .manager
            .status(host)
            .is_some_and(|status| status.state.is_connected());
    if held_before_request {
        state.manager.hold_connection(host, changing.clone()).await;
        state
            .manager
            .transport()
            .release_shared_connections(&row)
            .await;
    }
    let result = async {
        let transitioned = transition(&state.tensorlake, &key, sandbox, resume).await?;
        if resume && transitioned && !held_before_request {
            // An external suspension may precede the actor's next refresh.
            // Its still-published Connected label then describes the old
            // channel, not recovery. Withdraw it before returning success,
            // and discard both masters so launchers must await a fresh token.
            state.manager.hold_connection(host, changing).await;
            state
                .manager
                .transport()
                .release_shared_connections(&row)
                .await;
        }
        let suspended_at = if resume {
            None
        } else {
            Some(if transitioned {
                now_ms()
            } else {
                binding.suspended_at.unwrap_or_else(now_ms)
            })
        };
        if !state
            .store
            .set_tensorlake_suspended_at(host, suspended_at)
            .await?
        {
            return Err(refusal(
                ErrorKind::Conflict,
                "This Tensorlake host is no longer registered",
            ));
        }
        if let Some(observed_at) = suspended_at {
            // Persist for recovery, and carry attribution in the published
            // state too: a host-list read may have captured the old row
            // before this write and then observe the new confirmation.
            state
                .manager
                .hold_connection(
                    host,
                    HostState::Suspended {
                        observed_at,
                        suspended_by_farhelm_at: Some(observed_at),
                        resume_failed: None,
                    },
                )
                .await;
        }
        Ok(())
    }
    .await;
    // Resume also clears a hold left by an earlier lifecycle operation. The
    // lock excludes another lifecycle caller, so it cannot release theirs.
    // A real resume above withdrew any pre-suspension connection. Only an
    // already-running sandbox with a connected actor can take the shortcut;
    // repeated Resume must leave those live viewers attached.
    if !resume
        || !state
            .manager
            .status(host)
            .is_some_and(|status| status.state.is_connected())
    {
        state.manager.release_hold(host, result.is_err());
        if resume && result.is_ok() {
            // Releasing a hold only wakes a live actor. Resume must also
            // revive one that died while the sandbox was unavailable.
            let _ = state.manager.retry_now_with_fresh_window(host).await?;
        }
    }
    result
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
