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
//!   ([`Tensorlake::delete_sandbox`]); the store refuses to forget the row
//!   without the proof this returns.

use super::Tensorlake;
use super::api::{ApiKey, SandboxStatus, TensorlakeApi, TensorlakeError};
use crate::manager::{SandboxCheck, SandboxStatusSource};
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

/// Proof that a Tensorlake host's sandbox is gone: terminated (or never
/// found) and its snapshots deleted.
/// [`crate::store::HelmStore::remove_host_row`] forgets a Tensorlake row
/// only when handed one for that very row, so no code path can forget a
/// sandbox host while its sandbox may still bill (D6). The field and the
/// teardown constructor are private to this module. The only other
/// constructor, [`Self::never_created`], is reserved for registration
/// rollback before any create request can have reached Tensorlake.
#[derive(Debug)]
pub(crate) struct SandboxDeleted {
    host: HostId,
}

impl SandboxDeleted {
    /// Roll back a newly registered row before its first create request.
    /// Callers must not use this once Tensorlake may have received a create.
    pub(crate) fn never_created(host: HostId) -> Self {
        Self { host }
    }
    /// Attest that `host`'s sandbox and its snapshots are gone. Only after
    /// Tensorlake has said so.
    fn attest(host: HostId) -> Self {
        Self { host }
    }

    /// The host this proof is for.
    pub(crate) fn host(&self) -> HostId {
        self.host
    }
}

#[async_trait::async_trait]
impl SandboxStatusSource for Tensorlake {
    async fn check(&self, sandbox: &str) -> SandboxCheck {
        self.check_sandbox(sandbox).await
    }

    async fn connected(
        &self,
        row: &HostRow,
        client: std::sync::Arc<crate::client::SupervisorClient>,
        events: std::sync::Arc<crate::feed::FleetEvents>,
    ) -> crate::manager::ConnectedResources {
        if row.tensorlake.is_none() {
            return crate::manager::ConnectedResources::default();
        }
        // Registration precedes every credential read, and the returned lease
        // covers the later Connected publication as well as serving itself.
        let lease = self.0.delivery.enter(row);
        self.0.delivery.connect(row, &events).await;
        crate::manager::ConnectedResources {
            _tunnels: self.start_tunnels(row, client, events).await,
            _credentials: Some(lease),
        }
    }
}

impl Tensorlake {
    /// What Tensorlake says about `sandbox` right now, for the actor's dial
    /// gate. Never fails: anything that keeps Tensorlake from answering (no
    /// key saved, a rejected key, the network) is [`SandboxCheck::Unavailable`]
    /// with a sentence saying why, and the actor decides what that means.
    pub(crate) async fn check_sandbox(&self, sandbox: &str) -> SandboxCheck {
        let key = match self.api_key().await {
            Ok(key) => key,
            Err(reason) => return SandboxCheck::Unavailable(reason),
        };
        match tokio::time::timeout(STATUS_CHECK_TIMEOUT, self.0.api.get_sandbox(&key, sandbox))
            .await
        {
            Ok(Ok(info)) => SandboxCheck::Status {
                status: info.status,
                error_details: info.error_details,
            },
            Ok(Err(TensorlakeError::NotFound { .. })) => SandboxCheck::Missing,
            Ok(Err(error)) => SandboxCheck::Unavailable(error.to_string()),
            Err(_) => SandboxCheck::Unavailable(format!(
                "Tensorlake did not answer within {} seconds",
                STATUS_CHECK_TIMEOUT.as_secs()
            )),
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
    pub(crate) async fn delete_sandbox(&self, row: &HostRow) -> anyhow::Result<SandboxDeleted> {
        let Some(binding) = row.tensorlake.as_ref() else {
            anyhow::bail!("host {} is not a Tensorlake host", row.id);
        };
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
        // row has it once a create answered; before that, the name still
        // resolves, unless the sandbox never came to exist at all.
        let id = match &binding.sandbox_id {
            Some(id) => id.clone(),
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
                    match self
                        .0
                        .store
                        .set_tensorlake_sandbox_id(row.id, &info.id)
                        .await
                    {
                        Ok(true) => info.id,
                        Ok(false) => anyhow::bail!(
                            "host {} is no longer a Tensorlake host in the registry",
                            row.id
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
                    return Ok(SandboxDeleted::attest(row.id));
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
        Ok(SandboxDeleted::attest(row.id))
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
