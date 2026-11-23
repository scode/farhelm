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
use crate::ssh::{SharedConnection, SshAccessKind};
use crate::ssh_sign_in::{Attempt, BeforeQuestion, SignIn, SignInNeeded};
use crate::store::{HostId, HostRow};

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
    access: SshAccessKind,
    entry: Option<(HostId, String)>,
}

struct Completion {
    view: SshSignInCompletion,
    at: Instant,
}

/// A detached preflight can need cancellation before its first SSH question exists.
struct ActiveAttempt {
    host: Option<HostId>,
    cancel: oneshot::Sender<()>,
}

pub(super) struct SetupSignIn {
    relay: Arc<SignIn>,
    program: PathBuf,
    connect_timeout: Duration,
    completed: Mutex<VecDeque<Completion>>,
    /// Cancel from the pending reply through cleanup, by attempt for Add and by host for Remove.
    active: Mutex<HashMap<String, ActiveAttempt>>,
}

impl SetupSignIn {
    pub(super) fn new(relay: Arc<SignIn>, program: PathBuf) -> Self {
        Self {
            relay,
            program,
            connect_timeout: CONNECT,
            completed: Mutex::default(),
            active: Mutex::default(),
        }
    }

    /// Keep the real child deadline in the test while replacing the production time budget.
    #[cfg(test)]
    pub(super) fn with_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Setup sign-in uses the same SSH access policy as the first batch command, including its master.
    pub(super) fn destination(transport: &ProvisioningTarget) -> Option<(&str, SshAccessKind)> {
        match transport {
            ProvisioningTarget::Remote(RemoteRoute::Ssh {
                destination,
                access,
            }) => access.asks_user().then_some((destination, *access)),
            ProvisioningTarget::Remote(RemoteRoute::PodmanExec { .. })
            | ProvisioningTarget::Local => None,
        }
    }

    /// Return pending when the setup master needs a new connection. The caller can finish its normal planning
    /// on a successful reissue, rechecking all admission and confirmation facts then.
    pub(super) async fn prepare(
        &self,
        transport: &ProvisioningTarget,
        host: Option<&HostRow>,
    ) -> anyhow::Result<()> {
        let Some((destination, access)) = Self::destination(transport) else {
            return Ok(());
        };
        if self
            .relay
            .setup_ready(
                &self.program,
                destination,
                access,
                Instant::now() + self.connect_timeout,
            )
            .await?
        {
            return Ok(());
        }
        let attempt = match host {
            Some(host) => Attempt::for_host(host.id, SharedConnection::Provisioning),
            None => Attempt::for_draft(),
        };
        Err(Pending {
            attempt,
            destination: destination.to_owned(),
            access,
            entry: host.map(|row| (row.id, row.entry_uuid.clone())),
        }
        .into())
    }

    /// Start the preflight after sending its correlation in the command reply path. The result remains readable
    /// whether it finished before or after the channel next projects its collections. Publish cancellation before
    /// the task reads the registered row, so removal can either cancel the task or make its first lookup fail.
    pub(super) fn start(self: &Arc<Self>, pending: Pending, state: Arc<AppState>) {
        let setup = Arc::clone(self);
        let (send, cancel) = oneshot::channel();
        self.active.lock().expect("SSH setup active mutex").insert(
            pending.attempt.id.clone(),
            ActiveAttempt {
                host: pending.entry.as_ref().map(|(host, _)| *host),
                cancel: send,
            },
        );
        tokio::spawn(async move {
            let wait = async {
                // The route guard keeps a Tailscale mode edit from overtaking this SSH process.
                let _guard = if let Some((host, entry)) = &pending.entry {
                    let lookup = || async {
                        state.provisioning.host_row(*host).await.map_err(|error| {
                            BackendFailure::new(
                                "Could not read the host for SSH sign-in",
                                error.to_string(),
                            )
                        })
                    };
                    lookup().await?;
                    let guard = state.manager.ssh_route_lock(*host).read_owned().await;
                    let row = lookup().await?;
                    if row.entry_uuid != *entry
                        || row.destination.as_deref() != Some(pending.destination.as_str())
                        || row.ssh_access() != Some(pending.access)
                    {
                        return Err(BackendFailure::new(
                            "The SSH connection settings changed. Start the check again.",
                            "",
                        ));
                    }
                    Some(guard)
                } else {
                    None
                };
                setup
                    .wait(&pending.destination, pending.access, &pending.attempt)
                    .await
            };
            let result = tokio::select! {
                biased;
                _ = cancel => Err(BackendFailure::new("SSH sign-in was cancelled", "")),
                result = wait => result,
            };
            setup
                .active
                .lock()
                .expect("SSH setup active mutex")
                .remove(&pending.attempt.id);
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
            state.manager.events().bump();
        });
    }

    /// Closing Add can abandon its own draft even before a question id exists. Registered checks belong to their host.
    fn cancel_draft(&self, attempt: &str) {
        let mut active = self.active.lock().expect("SSH setup active mutex");
        if active
            .get(attempt)
            .is_some_and(|active| active.host.is_none())
            && let Some(active) = active.remove(attempt)
        {
            let _ = active.cancel.send(());
        }
    }

    /// Cancel the removed host's detached checks after its row is gone. Dropping their waits closes the questions
    /// and ends the SSH processes. Accepted provisioning runs own their lock and wait on their own tasks.
    pub(super) fn cancel_host(&self, host: HostId) {
        let mut active = self.active.lock().expect("SSH setup active mutex");
        for (_, attempt) in active.extract_if(|_, attempt| attempt.host == Some(host)) {
            let _ = attempt.cancel.send(());
        }
    }

    /// Accepted work already has a progress record and can wait on its task. A failure from this check enters the
    /// same preflight failure path as an unreachable or changed supervisor.
    pub(super) async fn establish_for_run(
        &self,
        transport: &ProvisioningTarget,
        host: i64,
    ) -> Result<(), BackendFailure> {
        let Some((destination, access)) = Self::destination(transport) else {
            return Ok(());
        };
        self.wait(
            destination,
            access,
            &Attempt::for_host(host, SharedConnection::Provisioning),
        )
        .await
    }

    async fn wait(
        &self,
        destination: &str,
        access: SshAccessKind,
        attempt: &Attempt,
    ) -> Result<(), BackendFailure> {
        self.relay
            .establish(
                self.program.as_path(),
                destination,
                access,
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
