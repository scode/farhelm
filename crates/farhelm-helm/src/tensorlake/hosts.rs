//! Tensorlake hosts against Tensorlake's API: the status check a host's
//! connection actor makes before every dial, and removal, which deletes the
//! sandbox.
//!
//! A Tensorlake host is an ssh host whose machine Farhelm created (see
//! [`crate::store::HostKind::Tensorlake`]). Two things about that machine are
//! only knowable from Tensorlake:
//!
//! - Whether it is running. A suspended sandbox refuses new ssh connections
//!   and hangs new sessions on an old one, so the actor asks first and shows
//!   the host as Suspended instead of dialing ([`Tensorlake::check_sandbox`]).
//! - How to make it stop billing. Removing the host terminates the sandbox and
//!   deletes the snapshots it left behind before the row is forgotten
//!   ([`EnvironmentProvider::destroy`]); the store refuses to forget the row
//!   without the proof this returns.

use super::Tensorlake;
use super::api::{ApiKey, SandboxStatus, TensorlakeApi, TensorlakeError};
use crate::environment::{
    ConnectedResources, Destroyed, EnvironmentBinding, EnvironmentProvider, EnvironmentStatus,
    ProviderHandle, StatusUnanswered,
};
use crate::store::{HostId, HostRow};
use farhelm_proto::ErrorKind;
use std::time::Duration;

/// How long the status check before a dial may take. Shorter than the
/// client's own call timeout because it runs inside the actor's 20-second
/// attempt budget, which the dial itself needs most of. A check that runs
/// out of time counts as "Tensorlake could not answer", which dials anyway.
const STATUS_CHECK_TIMEOUT: Duration = Duration::from_secs(10);

/// The bound on each API call a removal makes. A removal answers its request
/// directly (SPEC.md asks removal to answer promptly), so each call gets a
/// bound of its own and the whole removal stays around a minute at worst.
const REMOVAL_CALL_TIMEOUT: Duration = Duration::from_secs(20);

/// How long a removal waits for Tensorlake to report the sandbox terminated
/// before giving up (the row stays, and removing again resumes). Waiting at
/// all matters: a suspend that was in flight when the sandbox was terminated
/// can still be writing its snapshot, and sweeping before it exists would
/// leave that snapshot billing.
const TERMINATION_WAIT: Duration = Duration::from_secs(30);

/// How often a removal asks whether the sandbox has terminated.
const TERMINATION_POLL: Duration = Duration::from_secs(1);

/// Confirmed cleanup or a never-sent create permits forgetting this host row.
/// Successful cleanup is attested only inside this provider module. The
/// registration rollback exception requires its caller to establish that no
/// sandbox ID or outstanding create is recorded. Neither case establishes
/// absence of unrecorded legacy or power-loss leftovers in the account.
#[derive(Debug)]
pub(crate) struct SandboxDeleted {
    host: HostId,
}

impl SandboxDeleted {
    /// Roll back a registration after establishing that no create was recorded.
    pub(crate) fn never_created(host: HostId) -> Self {
        Self { host }
    }

    /// Issue successful-cleanup evidence only after the provider confirms it.
    fn attest(host: HostId) -> Self {
        Self { host }
    }

    /// Bind the shared removal gate to this proof's original registry row.
    pub(crate) fn host(&self) -> HostId {
        self.host
    }
}

/// Translate vendor states once, at the provider boundary. Suspending already
/// forbids dialing, while pending/snapshotting remain ordinary ladder failures.
pub(crate) fn environment_status(
    sandbox: &str,
    status: SandboxStatus,
    error_details: Option<String>,
) -> EnvironmentStatus {
    match status {
        SandboxStatus::Running => EnvironmentStatus::Running,
        SandboxStatus::Suspended | SandboxStatus::Suspending => EnvironmentStatus::Suspended {
            resume_failed: None,
        },
        SandboxStatus::ResumeFailed => EnvironmentStatus::Suspended {
            resume_failed: Some(
                error_details.unwrap_or_else(|| "Tensorlake gave no reason".into()),
            ),
        },
        SandboxStatus::Pending => EnvironmentStatus::Changing(format!(
            "sandbox {sandbox} is starting; Farhelm connects once it is running"
        )),
        SandboxStatus::Snapshotting => EnvironmentStatus::Changing(format!(
            "sandbox {sandbox} is taking a snapshot; Farhelm connects once it is running"
        )),
        SandboxStatus::Unknown => EnvironmentStatus::Unknown(format!(
            "sandbox {sandbox} is in a state Farhelm does not know; Farhelm connects once it is running"
        )),
        SandboxStatus::Terminated => missing(sandbox),
    }
}

/// Definitive absence carries the existing host-removal remedy.
pub(crate) fn missing(sandbox: &str) -> EnvironmentStatus {
    EnvironmentStatus::Missing(format!(
        "Tensorlake has no running sandbox {sandbox}; it was deleted outside Farhelm. Delete \
         this host to remove it from the list"
    ))
}

/// Keep vendor context in the provider so the actor can append it unchanged.
pub(crate) fn unanswered(reason: String) -> StatusUnanswered {
    StatusUnanswered(format!(
        "Farhelm could not ask Tensorlake whether the sandbox is running: {reason}"
    ))
}

#[async_trait::async_trait]
impl EnvironmentProvider for Tensorlake {
    async fn status(
        &self,
        binding: &EnvironmentBinding,
    ) -> Result<EnvironmentStatus, StatusUnanswered> {
        let ProviderHandle::Tensorlake(binding) = &binding.handle else {
            return Err(unanswered(
                "the registry binding belongs to another provider".into(),
            ));
        };
        self.check_sandbox(&binding.sandbox).await
    }

    async fn suspend(
        &self,
        state: &crate::AppState,
        binding: &EnvironmentBinding,
    ) -> anyhow::Result<()> {
        super::lifecycle::change_tensorlake(self, state, binding.host, false).await
    }

    async fn resume(
        &self,
        state: &crate::AppState,
        binding: &EnvironmentBinding,
    ) -> anyhow::Result<()> {
        super::lifecycle::change_tensorlake(self, state, binding.host, true).await
    }

    async fn destroy(&self, binding: &EnvironmentBinding) -> anyhow::Result<Destroyed> {
        let ProviderHandle::Tensorlake(tensorlake) = &binding.handle else {
            anyhow::bail!("the registry binding belongs to another provider");
        };
        self.delete_binding(binding.host, tensorlake).await
    }

    async fn connected(
        &self,
        row: &HostRow,
        client: std::sync::Arc<crate::client::SupervisorClient>,
        events: std::sync::Arc<crate::feed::FleetEvents>,
    ) -> ConnectedResources {
        if row.tensorlake.is_none() {
            return ConnectedResources::default();
        }
        // Registration precedes every credential read, and the returned lease
        // covers the later Connected publication as well as serving itself.
        let lease = self.0.delivery.enter(row);
        self.0.delivery.connect(row, &events).await;
        ConnectedResources {
            _tunnels: self.start_tunnels(row, client, events).await,
            _credentials: Some(lease),
        }
    }
}

impl Tensorlake {
    /// What Tensorlake says about `sandbox` right now, for the actor's dial
    /// gate. Failed reads carry a complete sentence while successful unknown
    /// states stay answers. The actor applies the provider's static policy.
    pub(crate) async fn check_sandbox(
        &self,
        sandbox: &str,
    ) -> Result<EnvironmentStatus, StatusUnanswered> {
        let key = match self.api_key().await {
            Ok(key) => key,
            Err(reason) => return Err(unanswered(reason)),
        };
        match tokio::time::timeout(STATUS_CHECK_TIMEOUT, self.0.api.get_sandbox(&key, sandbox))
            .await
        {
            Ok(Ok(info)) => Ok(environment_status(sandbox, info.status, info.error_details)),
            Ok(Err(TensorlakeError::NotFound { .. })) => Ok(missing(sandbox)),
            Ok(Err(error)) => Err(unanswered(error.to_string())),
            Err(_) => Err(unanswered(format!(
                "Tensorlake did not answer within {} seconds",
                STATUS_CHECK_TIMEOUT.as_secs()
            ))),
        }
    }

    /// Terminate a Tensorlake host's sandbox and delete every snapshot whose
    /// source is that sandbox, returning the proof the store needs to forget
    /// the row ([`crate::store::HelmStore::remove_host_row`]).
    ///
    /// Idempotent, so a removal that failed or was interrupted partway is
    /// finished by the next one: a sandbox Tensorlake no longer has counts
    /// as terminated, a snapshot it no longer has as deleted, and a row that
    /// never sent a create needs nothing deleted. An unanswered create
    /// retains the row until its name resolves or the builder's shared
    /// settle window makes a missing lookup conclusive. Any other failure
    /// leaves the row in place and says why.
    ///
    /// There is deliberately no way around a missing API key (D6): without
    /// one Farhelm cannot delete the sandbox, and forgetting the row anyway
    /// would leave it billing with nothing in Farhelm pointing at it.
    #[cfg(test)]
    pub(crate) async fn delete_sandbox(&self, row: &HostRow) -> anyhow::Result<Destroyed> {
        let Some(binding) = row.tensorlake.as_ref() else {
            anyhow::bail!("host {} is not a Tensorlake host", row.id);
        };
        self.delete_binding(row.id, binding).await
    }

    /// Preserve recorded ownership through every retry. The saved ID survives
    /// name disappearance and the pending marker bounds ambiguous creates.
    async fn delete_binding(
        &self,
        host: HostId,
        binding: &crate::store::TensorlakeBinding,
    ) -> anyhow::Result<Destroyed> {
        let key = self.api_key().await.map_err(|_| {
            refusal(
                ErrorKind::Conflict,
                "Farhelm needs the Tensorlake API key to delete this sandbox; add it in \
                 Settings → Tensorlake, then remove the host again"
                    .to_string(),
            )
        })?;
        let api = self.0.api.as_ref();
        let sandbox = binding.sandbox.as_str();
        // The id is what finds the snapshots once the sandbox is gone. The
        // row has it once a create answered. Before that, only an outstanding
        // create permits recovery by name: a never-sent row may collide with
        // another helm's sandbox, which removing this entry must leave alone.
        let id = match &binding.sandbox_id {
            Some(id) => id.clone(),
            None if binding.pending_create_at.is_none() => {
                return Ok(Destroyed::Tensorlake(SandboxDeleted::never_created(host)));
            }
            None => match bounded(api.get_sandbox(&key, sandbox)).await {
                Ok(info) => {
                    // Saved before anything is terminated: a terminated
                    // sandbox's name may stop resolving, and a removal that
                    // fails after the terminate (a refused snapshot delete,
                    // a crash) must still find its snapshots on the retry.
                    // Only the saved id can; without it the retry would take
                    // the "never existed" answer below and leave them
                    // billing. If the id cannot be saved, nothing is
                    // terminated.
                    match self.0.store.set_tensorlake_sandbox_id(host, &info.id).await {
                        Ok(true) => info.id,
                        Ok(false) => anyhow::bail!(
                            "host {} is no longer a Tensorlake host in the registry",
                            host
                        ),
                        Err(error) => {
                            return Err(error.context(format!(
                                "Farhelm could not record the id of sandbox {sandbox}, so it did \
                                 not delete the sandbox; remove the host again"
                            )));
                        }
                    }
                }
                Err(TensorlakeError::NotFound { .. }) => {
                    if binding.pending_create_at.is_some_and(|sent| {
                        super::recipes::now_secs() < sent + super::build::CREATE_SETTLE_SECS
                    }) {
                        return Err(refusal(ErrorKind::Unavailable,
                            "a sandbox Farhelm asked Tensorlake for may still be starting; the host stays in the list. Try Delete sandbox again in a few minutes".into()));
                    }
                    return Ok(Destroyed::Tensorlake(SandboxDeleted::attest(host)));
                }
                Err(error) => return Err(api_failure("look up", sandbox, error)),
            },
        };
        match bounded(api.terminate_sandbox(&key, &id)).await {
            Ok(()) | Err(TensorlakeError::NotFound { .. }) => {}
            Err(error) => return Err(api_failure("terminate", sandbox, error)),
        }
        wait_until_terminated(api, &key, sandbox, &id).await?;
        let snapshots = bounded(api.list_snapshots(&key))
            .await
            .map_err(|error| api_failure("list the snapshots of", sandbox, error))?;
        for snapshot in snapshots
            .snapshots
            .iter()
            .filter(|snapshot| snapshot.sandbox_id.as_deref() == Some(id.as_str()))
        {
            match bounded(api.delete_snapshot(&key, &snapshot.id)).await {
                Ok(()) | Err(TensorlakeError::NotFound { .. }) => {}
                Err(error) => {
                    return Err(refusal(
                        ErrorKind::Unavailable,
                        format!(
                            "Farhelm terminated sandbox {sandbox} but could not delete its \
                             snapshot {}: {error}. Remove the host again to finish",
                            snapshot.id
                        ),
                    ));
                }
            }
        }
        Ok(Destroyed::Tensorlake(SandboxDeleted::attest(host)))
    }

    /// The saved API key, or a sentence saying why there is none.
    pub(crate) async fn api_key(&self) -> Result<ApiKey, String> {
        let config = self
            .0
            .store
            .tensorlake_config()
            .await
            .map_err(|_| "Farhelm could not read its Tensorlake settings".to_string())?;
        config
            .api_key()
            .ok_or_else(|| "no Tensorlake API key is saved in Settings → Tensorlake".to_string())
    }
}

/// Poll until Tensorlake reports the sandbox terminated or no longer has it,
/// within [`TERMINATION_WAIT`].
pub(super) async fn wait_until_terminated(
    api: &dyn TensorlakeApi,
    key: &ApiKey,
    sandbox: &str,
    id: &str,
) -> anyhow::Result<()> {
    let deadline = tokio::time::Instant::now() + TERMINATION_WAIT;
    loop {
        match bounded(api.get_sandbox(key, id)).await {
            Ok(info) if info.status == SandboxStatus::Terminated => return Ok(()),
            Err(TensorlakeError::NotFound { .. }) => return Ok(()),
            Ok(_) => {}
            Err(error) => return Err(api_failure("check on", sandbox, error)),
        }
        if tokio::time::Instant::now() + TERMINATION_POLL > deadline {
            return Err(refusal(
                ErrorKind::Unavailable,
                format!(
                    "Tensorlake has not finished terminating sandbox {sandbox} after {} \
                     seconds; remove the host again in a moment",
                    TERMINATION_WAIT.as_secs()
                ),
            ));
        }
        tokio::time::sleep(TERMINATION_POLL).await;
    }
}

/// One API call under [`REMOVAL_CALL_TIMEOUT`]; running out of time is a
/// transport failure like any other unanswered call.
pub(super) async fn bounded<T>(
    call: impl std::future::Future<Output = Result<T, TensorlakeError>>,
) -> Result<T, TensorlakeError> {
    tokio::time::timeout(REMOVAL_CALL_TIMEOUT, call)
        .await
        .unwrap_or_else(|_| {
            Err(TensorlakeError::Transport(format!(
                "Tensorlake did not answer within {} seconds",
                REMOVAL_CALL_TIMEOUT.as_secs()
            )))
        })
}

/// A refusal the REST edge reports with `kind` and `message` as they are.
fn refusal(kind: ErrorKind, message: String) -> anyhow::Error {
    anyhow::Error::new(crate::SupervisorError::helm(kind, message))
}

/// A failed call during removal, worded for the person removing the host:
/// what Farhelm was doing, Tensorlake's own reason, and that the host stays.
fn api_failure(doing: &str, sandbox: &str, error: TensorlakeError) -> anyhow::Error {
    refusal(
        ErrorKind::Unavailable,
        format!(
            "Farhelm could not {doing} sandbox {sandbox}: {error}. The host stays in the list \
             until its sandbox is deleted; remove it again to retry"
        ),
    )
}

/// Planting Tensorlake host rows for tests elsewhere in the crate. None of
/// these tests needs the add flow: they start from rows written through the
/// store's own registration, independently of provisioning.
#[cfg(test)]
pub(crate) mod test_support {
    use crate::store::{HelmStore, HostId, NewHostEntry, NewTensorlakeHost};
    use farhelm_proto::tensorlake::recipes::RecipeSize;

    /// The destination Tensorlake's gateway gives a sandbox named `sandbox`.
    pub(crate) fn destination(sandbox: &str) -> String {
        crate::tensorlake::ssh::destination(sandbox)
    }

    /// Register a Tensorlake host named `name` (its alias) for `sandbox`,
    /// made from recipe `recipe` at 2 CPUs, 4 GB and 20 GB, and record
    /// `sandbox_id` when given, as a create that answered would.
    pub(crate) async fn plant(
        store: &HelmStore,
        name: &str,
        sandbox: &str,
        recipe: &str,
        sandbox_id: Option<&str>,
    ) -> HostId {
        let host = store
            .register_tensorlake_host(NewTensorlakeHost {
                destination: destination(sandbox),
                entry: NewHostEntry::new(Some(name)).expect("a valid name"),
                sandbox: sandbox.to_string(),
                recipe: recipe.to_string(),
                resources: RecipeSize {
                    cpus: 2,
                    memory_gb: 4,
                    disk_gb: 20,
                },
            })
            .await
            .expect("register the Tensorlake host");
        if let Some(id) = sandbox_id {
            assert!(
                store
                    .set_tensorlake_sandbox_id(host, id)
                    .await
                    .expect("record the sandbox id")
            );
        }
        host
    }
}

#[cfg(test)]
#[path = "hosts_tests.rs"]
mod tests;
