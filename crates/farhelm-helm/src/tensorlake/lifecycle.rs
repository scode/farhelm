//! Explicit suspend/resume operations. The provisioning lock serializes them
//! with setup, updates and removal; the connection hold prevents a concurrent
//! retry from reopening an ssh channel during an explicit lifecycle request.
//! The held display state records intent until Tensorlake confirms the target;
//! after Resume releases the hold, ordinary connection progress takes over.
//!
//! The two routes answer within the host reply bound (see [`bounded`]), while
//! the change itself runs to its end on a task of its own.

use super::api::{ApiKey, SandboxStatus, TensorlakeError};
use crate::{AppState, manager::HostState, store::HostId};
use axum::{
    extract::{Path, State},
    response::{IntoResponse, Response},
};
use farhelm_proto::{ErrorKind, tensorlake::LifecycleDirection};
use std::{sync::Arc, time::Duration};

/// `POST /api/hosts/{id}/suspend`: suspend an environment host, only after
/// every connection owned by this helm has been released. A 200 means the
/// provider confirmed the suspension; past the host reply bound the reply is
/// [`still_changing`]'s 504 instead (see [`bounded`]).
pub(crate) async fn suspend(
    State(state): State<Arc<AppState>>,
    Path(host): Path<HostId>,
) -> Response {
    bounded(state, host, false).await
}

/// `POST /api/hosts/{id}/resume`: resume explicitly, then let the actor
/// establish a fresh connection. A 200 means Tensorlake reports running;
/// connection readiness follows separately. Past the host reply bound the
/// reply is [`still_changing`]'s 504 instead (see [`bounded`]).
pub(crate) async fn resume(
    State(state): State<Arc<AppState>>,
    Path(host): Path<HostId>,
) -> Response {
    bounded(state, host, true).await
}

/// Run one route's Suspend or Resume on a task of its own, answering by
/// [`crate::sessions::HOST_REPLY_BOUND`].
///
/// The GUI is to send both over its UI channel, which admits one command at a time
/// and treats a command unanswered for 30 seconds as a broken connection,
/// while a Tensorlake Suspend may wait 60 seconds for the sandbox to settle
/// and a Resume 120 ([`transition`]). Tensorlake's refusals that come before
/// anything changes (a busy host, a host that is not an environment, no
/// account key) are local and answer as before. A container host checks its
/// container over SSH before it changes anything, inside the bound, so a slow
/// container host can meet the bound before that check ends; the 504's words
/// stay true then too. A change still running at the bound answers with
/// [`still_changing`]'s 504 and carries on under its provisioning lock and
/// connection hold, so a second click meanwhile meets the busy refusal. Its outcome lands in the host's pushed state: Suspended once
/// confirmed, a fresh connection after a Resume, or, after a failure, the same
/// reconnect any failed change ends in, its reason reaching only the helm's log.
/// The migration's orchestrator decided on 2026-11-15 that these routes take
/// this bound rather than the accepted form, because that outcome already
/// reaches the client through the pushed host state.
///
/// The routes sit in the router's cancellable group, outside its ownership
/// layer, so this task is also what keeps a dropped caller from cancelling
/// the change partway and stranding its hold.
async fn bounded(state: Arc<AppState>, host: HostId, resume: bool) -> Response {
    crate::sessions::within_host_reply_bound_saying(
        async move { answer(change(&state, host, resume).await) },
        still_changing(if resume { "resuming" } else { "suspending" }),
    )
    .await
}

/// The text of the 504 a host's Suspend or Resume answers when it is still
/// running at the host reply bound (see [`bounded`]); `doing` names the
/// change ("suspending" or "resuming").
///
/// It promises only what is true whichever way the change ends: that it goes
/// on, and where its result shows.
fn still_changing(doing: &str) -> String {
    format!(
        "{doing} the host did not finish within {} seconds; it carries on, and the host's state \
         in the hosts list and the session list will show the result",
        crate::sessions::HOST_REPLY_BOUND.as_secs()
    )
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
                    // The hold gives us Running evidence that can be newer
                    // than the registry row we read before parking the actor.
                    let running_seen_at = manager.running_seen_at(host);
                    binding
                        .suspended_at
                        .filter(|&stamp| running_seen_at.is_none_or(|seen| stamp >= seen))
                        .unwrap_or_else(now_ms)
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
/// Returns whether a mutation was needed. A new pause earns its own stamp.
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
