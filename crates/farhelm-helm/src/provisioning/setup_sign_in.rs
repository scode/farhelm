//! Interactive preflight on the setup master, before any batch provisioning command.
//!
//! Planning may have to give the ordered UI channel back before SSH can finish asking questions. The pending reply
//! carries an opaque attempt id. Only its terminal outcome is retained for the reprobe. An accepted run already has
//! progress and an owned task, so it awaits exactly the same SSH relay without retaining a second result.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Json;
use axum::extract::{Path as ApiPath, State};
use axum::http::StatusCode;
use farhelm_proto::ui_channel::{SshSignInCompletion, SshSignInOutcome};
use tokio::sync::oneshot;
use tokio::time::Instant;

use super::backend::BackendFailure;
use super::plan::ProvisioningTarget;
use crate::AppState;
use crate::access::RemoteRoute;
use crate::feed::FleetEvents;
use crate::ssh::SharedConnection;
use crate::ssh_sign_in::{Attempt, BeforeQuestion, SignIn, SignInNeeded};

const CONNECT: Duration = Duration::from_secs(15);
const RETAIN: Duration = Duration::from_secs(180);
const MAX_COMPLETIONS: usize = 128;

/// This job has passed planning's local checks but has not launched anything. Activate it only from the response
/// that actually reaches the caller, since the existing reply bound can leave a late planning task to finish.
#[derive(Clone, Debug, thiserror::Error)]
#[error("SSH setup sign-in is pending")]
pub(super) struct Pending {
    pub(super) attempt: Attempt,
    destination: String,
}

struct Completion {
    view: SshSignInCompletion,
    at: Instant,
}

pub(super) struct SetupSignIn {
    relay: Arc<SignIn>,
    program: PathBuf,
    connect_timeout: Duration,
    completed: Mutex<VecDeque<Completion>>,
    /// Add can close before there is a question id. Keep its cancel sender from the pending reply through cleanup.
    drafts: Mutex<HashMap<String, oneshot::Sender<()>>>,
}

impl SetupSignIn {
    pub(super) fn new(relay: Arc<SignIn>, program: PathBuf) -> Self {
        Self {
            relay,
            program,
            connect_timeout: CONNECT,
            completed: Mutex::default(),
            drafts: Mutex::default(),
        }
    }

    /// Keep the real child deadline in the test while replacing the production time budget.
    #[cfg(test)]
    pub(super) fn with_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// The ordinary SSH case asks before the first batch command. Local, Tensorlake and container execution
    /// have their own transport policies and keep their existing execution paths.
    pub(super) fn destination(transport: &ProvisioningTarget) -> Option<&str> {
        match transport {
            ProvisioningTarget::Remote(RemoteRoute::Ssh {
                destination,
                access,
            }) => access.asks_user().then_some(destination),
            ProvisioningTarget::Remote(RemoteRoute::PodmanExec { .. })
            | ProvisioningTarget::Local => None,
        }
    }

    /// Return pending when the setup master needs a new connection. The caller can finish its normal planning
    /// on a successful reissue, rechecking all admission and confirmation facts then.
    pub(super) async fn prepare(
        &self,
        transport: &ProvisioningTarget,
        host: Option<i64>,
    ) -> anyhow::Result<()> {
        let Some(destination) = Self::destination(transport) else {
            return Ok(());
        };
        if self
            .relay
            .setup_ready(
                &self.program,
                destination,
                Instant::now() + self.connect_timeout,
            )
            .await?
        {
            return Ok(());
        }
        let attempt = match host {
            Some(host) => Attempt::for_host(host, SharedConnection::Provisioning),
            None => Attempt::for_draft(),
        };
        Err(Pending {
            attempt,
            destination: destination.to_owned(),
        }
        .into())
    }

    /// Start the preflight after sending its correlation in the command reply path. The result remains readable
    /// whether it finished before or after the channel next projects its collections.
    pub(super) fn start(self: &Arc<Self>, pending: Pending, events: Arc<FleetEvents>) {
        let setup = Arc::clone(self);
        let cancel = pending.attempt.is_draft().then(|| {
            let (send, recv) = oneshot::channel();
            self.drafts
                .lock()
                .expect("SSH setup draft mutex")
                .insert(pending.attempt.id.clone(), send);
            recv
        });
        tokio::spawn(async move {
            let wait = setup.wait(&pending.destination, &pending.attempt);
            let result = if let Some(cancel) = cancel {
                let result = tokio::select! {
                    biased;
                    _ = cancel => Err(BackendFailure::new("SSH sign-in was cancelled", "")),
                    result = wait => result,
                };
                setup
                    .drafts
                    .lock()
                    .expect("SSH setup draft mutex")
                    .remove(&pending.attempt.id);
                result
            } else {
                wait.await
            };
            let outcome = match result {
                Ok(()) => SshSignInOutcome::Ready,
                Err(error) => SshSignInOutcome::Failed {
                    reason: error.rendered(),
                },
            };
            {
                let mut completed = setup.completed.lock().expect("SSH setup completion mutex");
                let now = Instant::now();
                completed.retain(|entry| now - entry.at < RETAIN);
                if completed.len() == MAX_COMPLETIONS {
                    completed.pop_front();
                }
                completed.push_back(Completion {
                    at: now,
                    view: SshSignInCompletion {
                        attempt: pending.attempt.id,
                        outcome,
                    },
                });
            }
            events.bump();
        });
    }

    /// Only a draft may be abandoned without a question id. An already finished check has nothing left to stop.
    fn cancel_draft(&self, attempt: &str) {
        if let Some(cancel) = self
            .drafts
            .lock()
            .expect("SSH setup draft mutex")
            .remove(attempt)
        {
            let _ = cancel.send(());
        }
    }

    /// Accepted work already has a progress record and can wait on its task. A failure from this check enters the
    /// same preflight failure path as an unreachable or changed supervisor.
    pub(super) async fn establish_for_run(
        &self,
        transport: &ProvisioningTarget,
        host: i64,
    ) -> Result<(), BackendFailure> {
        let Some(destination) = Self::destination(transport) else {
            return Ok(());
        };
        self.wait(
            destination,
            &Attempt::for_host(host, SharedConnection::Provisioning),
        )
        .await
    }

    async fn wait(&self, destination: &str, attempt: &Attempt) -> Result<(), BackendFailure> {
        self.relay
            .establish(
                self.program.as_path(),
                destination,
                attempt,
                self.connect_timeout,
                &mut |_| {},
            )
            .await
            .map_err(|error| {
                if error.is::<SignInNeeded>() {
                    BackendFailure::new(
                        "SSH sign-in stopped before the host was connected. Try again.",
                        "",
                    )
                } else if let Some(failure) = error.downcast_ref::<BeforeQuestion>() {
                    let stderr = failure
                        .snapshot
                        .stderr_tail
                        .iter()
                        .map(|line| String::from_utf8_lossy(&line.bytes))
                        .collect::<Vec<_>>()
                        .join(" / ");
                    let detail = if failure.timed_out {
                        "the supervisor probe timed out"
                    } else {
                        "SSH could not connect or log in to the host"
                    };
                    BackendFailure::new(detail, stderr)
                } else {
                    BackendFailure::new(error.to_string(), "")
                }
            })
    }

    fn list(&self) -> Vec<SshSignInCompletion> {
        let now = Instant::now();
        self.completed
            .lock()
            .expect("SSH setup completion mutex")
            .iter()
            .filter(|entry| now - entry.at < RETAIN)
            .map(|entry| entry.view.clone())
            .collect()
    }
}

pub(crate) async fn list(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let completions = state
        .provisioning
        .setup_sign_in
        .as_ref()
        .map_or_else(Vec::new, |setup| setup.list());
    Json(serde_json::json!({ "completions": completions }))
}

/// Closing Add must also stop a still-silent SSH process. The attempt id is already known from its 202.
pub(crate) async fn cancel(
    State(state): State<Arc<AppState>>,
    ApiPath(attempt): ApiPath<String>,
) -> StatusCode {
    if let Some(setup) = &state.provisioning.setup_sign_in {
        setup.cancel_draft(&attempt);
    }
    StatusCode::NO_CONTENT
}
