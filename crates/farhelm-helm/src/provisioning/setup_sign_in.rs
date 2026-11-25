//! Interactive preflight on the setup master, before any batch provisioning command.
//!
//! Planning may have to give the ordered UI channel back before SSH can finish asking questions. The pending reply
//! carries the id of the attempt that owns the question. Manual and automatic planning can join a registered
//! host's check. The automatic listener holds its outcome independently of the bounded desktop completion history.
//! An accepted run already owns its task and awaits the same SSH relay directly.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use axum::Json;
use axum::extract::{Path as ApiPath, State};
use axum::http::StatusCode;
use farhelm_proto::ui_channel::{SshSignInCompletion, SshSignInOutcome};
use tokio::sync::{oneshot, watch};
use tokio::time::Instant;

use super::backend::BackendFailure;
use super::plan::ProvisioningTarget;
use super::service::ProvisioningService;
use crate::AppState;
use crate::access::RemoteRoute;
use crate::ssh::{SharedConnection, SshAccessKind};
use crate::ssh_sign_in::{Attempt, BeforeQuestion, SignIn, SignInAccess, SignInNeeded};
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
    route: SetupRoute,
    /// A check observed by prepare may settle before the caller accepts the pending result.
    existing: Option<SetupOutcome>,
}

/// The exact registered entry and SSH settings that may share a setup sign-in. Drafts are always independent.
#[derive(Clone, Debug, PartialEq, Eq)]
struct SetupRoute {
    destination: String,
    access: SshAccessKind,
    entry: Option<(HostId, String)>,
}

/// Automatic cancellation releases the retry opportunity. A real sign-in failure consumes it.
#[derive(Clone, Debug)]
pub(super) enum SetupSettlement {
    Finished(SshSignInOutcome),
    Released,
}

type SetupOutcome = watch::Receiver<Option<SetupSettlement>>;

struct Completion {
    view: SshSignInCompletion,
    at: Instant,
}

/// A detached preflight can need cancellation before its first SSH question exists.
struct ActiveAttempt {
    attempt: Attempt,
    route: SetupRoute,
    cancel: oneshot::Sender<()>,
    outcome: watch::Sender<Option<SetupSettlement>>,
    manual: bool,
}

impl ActiveAttempt {
    /// Reuse both the question id and the result even if settlement overtakes the caller's reply.
    fn pending(&self) -> Pending {
        Pending {
            attempt: self.attempt.clone(),
            route: self.route.clone(),
            existing: Some(self.outcome.subscribe()),
        }
    }

    /// Host removal can cancel every registered route check. An Add draft has no host to remove.
    fn host(&self) -> Option<HostId> {
        self.route.entry.as_ref().map(|(host, _)| *host)
    }

    /// Publish the policy cancellation while this check is still covered by the active lock.
    /// A manual caller that prepared earlier can then distinguish it from an ordinary settlement.
    fn release(self) {
        self.outcome.send_replace(Some(SetupSettlement::Released));
        let _ = self.cancel.send(());
    }
}

/// An automatic listener's interest in a detached sign-in. Dropping it ends an auto-only check, including
/// before its first question. A delivered manual request can keep the check alive on its own.
pub(super) struct AutomaticSignIn {
    attempt: String,
    outcome: SetupOutcome,
    setup: Weak<SetupSignIn>,
}

impl AutomaticSignIn {
    /// A fleet revision follows settlement. The watch keeps the result even when many other checks finish.
    pub(super) fn outcome(&self) -> Option<SetupSettlement> {
        let closed = self.outcome.has_changed().is_err();
        self.outcome.borrow().clone().or_else(|| {
            closed.then(|| {
                SetupSettlement::Finished(SshSignInOutcome::Failed {
                    reason: "The SSH sign-in check ended without a result".into(),
                })
            })
        })
    }

    /// Await the same result as the listener without borrowing or owning its check.
    /// Only a successful authentication can dispatch an automatic continuation.
    pub(super) fn wait_ready(&self) -> impl Future<Output = bool> + Send + 'static {
        let mut outcome = self.outcome.clone();
        async move {
            outcome
                .wait_for(Option::is_some)
                .await
                .is_ok_and(|settled| {
                    matches!(
                        &*settled,
                        Some(SetupSettlement::Finished(SshSignInOutcome::Ready))
                    )
                })
        }
    }
}

impl Drop for AutomaticSignIn {
    fn drop(&mut self) {
        if let Some(setup) = self.setup.upgrade() {
            setup.cancel_automatic(&self.attempt);
        }
    }
}

pub(super) struct SetupSignIn {
    relay: Arc<SignIn>,
    program: PathBuf,
    connect_timeout: Duration,
    completed: Mutex<VecDeque<Completion>>,
    /// Cancel by attempt or by removed host. The same lock also arbitrates concurrent starts for a route.
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
        match transport.route() {
            Some(RemoteRoute::Ssh {
                destination,
                access,
            }) => access.asks_user().then_some((destination, *access)),
            Some(RemoteRoute::PodmanExec { .. }) | None => None,
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
        let route = SetupRoute {
            destination: destination.to_owned(),
            access,
            entry: host.map(|row| (row.id, row.entry_uuid.clone())),
        };
        if route.entry.is_some()
            && let Some(active) = self
                .active
                .lock()
                .expect("SSH setup active mutex")
                .values()
                .find(|active| active.route == route)
        {
            return Err(active.pending().into());
        }
        if self
            .relay
            .setup_ready(
                &self.program,
                destination,
                transport
                    .prepared_access()
                    .map_or(SignInAccess::Resolve(access), SignInAccess::Prepared),
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
            route,
            existing: None,
        }
        .into())
    }

    /// Activate the check once the pending HTTP reply wins its deadline. A racing winner's id is also the
    /// desktop's focus correlation, so give the caller the id of the running check.
    pub(super) fn start_manual(
        self: &Arc<Self>,
        pending: Pending,
        service: &Arc<ProvisioningService>,
    ) -> String {
        self.start(pending, service, true).0
    }

    /// Activate an eligible automatic check. Its lifetime belongs to the scan's pending state.
    pub(super) fn start_automatic(
        self: &Arc<Self>,
        pending: Pending,
        service: &Arc<ProvisioningService>,
    ) -> AutomaticSignIn {
        let (attempt, outcome) = self.start(pending, service, false);
        AutomaticSignIn {
            attempt,
            outcome,
            setup: Arc::downgrade(self),
        }
    }

    /// Decide who owns the prompt atomically across callers that both passed prepare before either one started.
    /// Publish cancellation before the task reads the row, so Remove can cancel it or make that lookup fail.
    fn start(
        self: &Arc<Self>,
        pending: Pending,
        service: &Arc<ProvisioningService>,
        manual: bool,
    ) -> (String, SetupOutcome) {
        let Pending {
            mut attempt,
            route,
            existing,
        } = pending;
        let mut active = self.active.lock().expect("SSH setup active mutex");
        if let Some(outcome) = existing {
            if let Some(joined) = active.get_mut(&attempt.id) {
                joined.manual |= manual;
            }
            if !matches!(*outcome.borrow(), Some(SetupSettlement::Released)) {
                return (attempt.id, outcome);
            }
            if let Some((host, _)) = &route.entry {
                attempt = Attempt::for_host(*host, SharedConnection::Provisioning);
            }
        }
        if route.entry.is_some()
            && let Some(joined) = active.values_mut().find(|active| active.route == route)
        {
            joined.manual |= manual;
            return (joined.attempt.id.clone(), joined.outcome.subscribe());
        }

        let setup = Arc::clone(self);
        let service = Arc::clone(service);
        let (send, cancel) = oneshot::channel();
        let (settled, outcome) = watch::channel(None);
        let id = attempt.id.clone();
        active.insert(
            id.clone(),
            ActiveAttempt {
                attempt: attempt.clone(),
                route: route.clone(),
                cancel: send,
                outcome: settled.clone(),
                manual,
            },
        );
        drop(active);
        tokio::spawn(async move {
            let wait = async {
                // The route guard keeps a Tailscale mode edit from overtaking this SSH process.
                let _guard = if let Some((host, entry)) = &route.entry {
                    let lookup = || async {
                        service.host_row(*host).await.map_err(|error| {
                            BackendFailure::new(
                                "Could not read the host for SSH sign-in",
                                error.to_string(),
                            )
                        })
                    };
                    lookup().await?;
                    let guard = service.manager.ssh_route_lock(*host).read_owned().await;
                    let row = lookup().await?;
                    if row.entry_uuid != *entry
                        || row.destination.as_deref() != Some(route.destination.as_str())
                        || row.ssh_access() != Some(route.access)
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
                    .wait(
                        &route.destination,
                        SignInAccess::Resolve(route.access),
                        &attempt,
                    )
                    .await
            };
            let result = tokio::select! {
                biased;
                _ = cancel => Err(BackendFailure::new("SSH sign-in was cancelled", "")),
                result = wait => result,
            };
            let outcome = match result {
                Ok(()) => SshSignInOutcome::Ready,
                Err(error) => SshSignInOutcome::Failed {
                    reason: error.rendered(),
                },
            };
            {
                // A caller must be able to join until the completion is already readable.
                let mut active = setup.active.lock().expect("SSH setup active mutex");
                let mut completed = setup.completed.lock().expect("SSH setup completion mutex");
                let now = Instant::now();
                completed.retain(|entry| now - entry.at < RETAIN);
                if completed.len() == MAX_COMPLETIONS {
                    completed.pop_front();
                }
                completed.push_back(Completion {
                    at: now,
                    view: SshSignInCompletion {
                        attempt: attempt.id.clone(),
                        outcome: outcome.clone(),
                    },
                });
                settled.send_if_modified(|state| {
                    if state.is_some() {
                        return false;
                    }
                    *state = Some(SetupSettlement::Finished(outcome));
                    true
                });
                active.remove(&attempt.id);
            }
            service.manager.events().bump();
        });
        (id, outcome)
    }

    /// Closing Add can abandon its own draft even before a question id exists. Registered checks belong to their host.
    fn cancel_draft(&self, attempt: &str) {
        let mut active = self.active.lock().expect("SSH setup active mutex");
        if active
            .get(attempt)
            .is_some_and(|active| active.host().is_none())
            && let Some(active) = active.remove(attempt)
        {
            let _ = active.cancel.send(());
        }
    }

    /// Retire automatic-only preflights at the committed opt-out, even if fleet wakes coalesce.
    pub(super) fn cancel_automatic_checks(&self) {
        let mut active = self.active.lock().expect("SSH setup active mutex");
        for (_, attempt) in active.extract_if(|_, attempt| !attempt.manual) {
            attempt.release();
        }
    }

    /// Cancellation and manual attachment use the same lock. A check already sponsored by a manual reply survives.
    fn cancel_automatic(&self, attempt: &str) {
        let mut active = self.active.lock().expect("SSH setup active mutex");
        if active.get(attempt).is_some_and(|active| !active.manual)
            && let Some(active) = active.remove(attempt)
        {
            active.release();
        }
    }

    /// Cancel the removed host's detached checks after its row is gone. Dropping their waits closes the questions
    /// and ends the SSH processes. Accepted provisioning runs own their lock and wait on their own tasks.
    pub(super) fn cancel_host(&self, host: HostId) {
        let mut active = self.active.lock().expect("SSH setup active mutex");
        for (_, attempt) in active.extract_if(|_, attempt| attempt.host() == Some(host)) {
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
        let access = transport
            .prepared_access()
            .map_or(SignInAccess::Resolve(access), SignInAccess::Prepared);
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
        access: SignInAccess<'_>,
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
